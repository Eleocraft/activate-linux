use x11rb::connection::Connection;
use x11rb::errors::{ReplyError, ReplyOrIdError};
use x11rb::image::Image;
use x11rb::protocol::render::{self, ConnectionExt as _, PictType};
use x11rb::protocol::shape::SK;
use x11rb::protocol::xfixes::ConnectionExt as _;
use x11rb::protocol::xproto::Window;
use x11rb::protocol::xproto::{AtomEnum, ColormapAlloc};
use x11rb::protocol::xproto::{ConfigureWindowAux, Visualtype};
use x11rb::protocol::xproto::{ConnectionExt, StackMode};
use x11rb::protocol::xproto::{CreateGCAux, Visualid};
use x11rb::protocol::xproto::{CreateWindowAux, WindowClass};
use x11rb::protocol::xproto::{EventMask, PropMode};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::wayland;

x11rb::atom_manager! {
    pub AtomCollection: AtomCollectionCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
        _NET_WM_NAME,
        _NET_WM_STATE,
        _NET_WM_STATE_ABOVE,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DIALOG,
        UTF8_STRING,
    }
}

fn init_tracing() {
    use tracing_subscriber::prelude::*;

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}

/// A rust version of XCB's `xcb_visualtype_t` struct. This is used in a FFI-way.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct XcbVisualtypeT {
    pub visual_id: u32,
    pub class: u8,
    pub bits_per_rgb_value: u8,
    pub colormap_entries: u16,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub pad0: [u8; 4],
}

impl From<Visualtype> for XcbVisualtypeT {
    fn from(value: Visualtype) -> XcbVisualtypeT {
        XcbVisualtypeT {
            visual_id: value.visual_id,
            class: value.class.into(),
            bits_per_rgb_value: value.bits_per_rgb_value,
            colormap_entries: value.colormap_entries,
            red_mask: value.red_mask,
            green_mask: value.green_mask,
            blue_mask: value.blue_mask,
            pad0: [0; 4],
        }
    }
}

fn choose_visual(conn: &impl Connection, screen_num: usize) -> Result<(u8, Visualid), ReplyError> {
    let depth = 32;
    let screen = &conn.setup().roots[screen_num];

    // Try to use XRender to find a visual with alpha support
    let has_render = conn
        .extension_information(render::X11_EXTENSION_NAME)?
        .is_some();
    if has_render {
        let formats = conn.render_query_pict_formats()?.reply()?;
        // Find the ARGB32 format that must be supported.
        let format = formats
            .formats
            .iter()
            .filter(|info| (info.type_, info.depth) == (PictType::DIRECT, depth))
            .filter(|info| {
                let d = info.direct;
                (d.red_mask, d.green_mask, d.blue_mask, d.alpha_mask) == (0xff, 0xff, 0xff, 0xff)
            })
            .find(|info| {
                let d = info.direct;
                (d.red_shift, d.green_shift, d.blue_shift, d.alpha_shift) == (16, 8, 0, 24)
            });
        if let Some(format) = format {
            // Now we need to find the visual that corresponds to this format
            if let Some(visual) = formats.screens[screen_num]
                .depths
                .iter()
                .flat_map(|d| &d.visuals)
                .find(|v| v.format == format.id)
            {
                return Ok((format.depth, visual.visual));
            }
        }
    }
    Ok((screen.root_depth, screen.root_visual))
}

fn composite_manager_running(
    conn: &impl Connection,
    screen_num: usize,
) -> Result<bool, ReplyError> {
    let atom = format!("_NET_WM_CM_S{}", screen_num);
    let atom = conn.intern_atom(false, atom.as_bytes())?.reply()?.atom;
    let owner = conn.get_selection_owner(atom)?.reply()?;
    Ok(owner.owner != x11rb::NONE)
}

fn create_window<C>(
    conn: &C,
    screen: &x11rb::protocol::xproto::Screen,
    atoms: &AtomCollection,
    (width, height): (u16, u16),
    depth: u8,
    visual_id: Visualid,
) -> Result<Window, ReplyOrIdError>
where
    C: Connection,
{
    let x = (screen.width_in_pixels - width) as i16;
    let y = (screen.height_in_pixels - height) as i16;
    let window = conn.generate_id()?;
    let colormap = conn.generate_id()?;
    conn.create_colormap(ColormapAlloc::NONE, colormap, screen.root, visual_id)?;
    let win_aux = CreateWindowAux::new()
        .override_redirect(1)
        .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY)
        .background_pixel(x11rb::NONE)
        .border_pixel(screen.black_pixel)
        .colormap(colormap);
    conn.create_window(
        depth,
        window,
        screen.root,
        x,
        y,
        width,
        height,
        0,
        WindowClass::INPUT_OUTPUT,
        visual_id,
        &win_aux,
    )?;

    conn.change_property32(
        PropMode::REPLACE,
        window,
        atoms.WM_PROTOCOLS,
        AtomEnum::ATOM,
        &[atoms.WM_DELETE_WINDOW],
    )?;

    conn.change_property32(
        PropMode::REPLACE,
        window,
        atoms._NET_WM_STATE,
        AtomEnum::ATOM,
        &[atoms._NET_WM_STATE_ABOVE],
    )?;

    conn.change_property32(
        PropMode::REPLACE,
        window,
        atoms._NET_WM_WINDOW_TYPE,
        AtomEnum::ATOM,
        &[atoms._NET_WM_WINDOW_TYPE_DIALOG],
    )?;

    conn.map_window(window)?;
    Ok(window)
}

pub fn x11_main(
    header: Option<String>,
    caption: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    let (conn, screen_num) = RustConnection::connect(None)?;

    conn.xfixes_query_version(5, 0)?;

    let screen = &conn.setup().roots[screen_num];

    let width: u16 = 335;
    let height: u16 = 110;

    let (depth, visualid) = choose_visual(&conn, screen_num)?;
    println!("Using visual {:#x} with depth {}", visualid, depth);

    let transparency = composite_manager_running(&conn, screen_num)?;
    println!(
        "Composite manager running / working transparency: {:?}",
        transparency
    );
    let atoms = AtomCollection::new(&conn)?.reply()?;
    let window = create_window(&conn, screen, &atoms, (width, height), depth, visualid)?;

    let region = conn.generate_id()?;
    conn.xfixes_create_region(region, &[])?;
    conn.xfixes_set_window_shape_region(window, SK::INPUT, 0, 0, region)?;
    conn.xfixes_destroy_region(region)?;

    conn.configure_window(
        window,
        &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
    )?;

    let gc = conn.generate_id()?;
    conn.create_gc(
        gc,
        window,
        &CreateGCAux::default().foreground(screen.white_pixel),
    )?;

    let (mut image, _) = Image::get(&conn, window, 0, 0, width, height)?;

    let image_width = image.width() as usize;
    let data = image.data_mut();

    let font = fontdue::Font::from_bytes(
        include_bytes!("../resources/Roboto[wdth,wght].ttf") as &[u8],
        fontdue::FontSettings::default(),
    )
    .unwrap();

    wayland::rasterize_string(
        &font,
        header.as_deref().unwrap_or("Activate Linux"),
        28.0,
        0,
        data,
        image_width,
    );
    wayland::rasterize_string(
        &font,
        caption
            .as_deref()
            .unwrap_or("Go to Settings to activate Linux."),
        16.0,
        32,
        data,
        image_width,
    );

    image.put(&conn, window, gc, 0, 0)?;

    conn.flush()?;
    loop {
        match conn.wait_for_event() {
            Ok(e) => {
                println!("Event: {e:?}")
            }
            Err(e) => eprintln!("{e:?}"),
        }
    }
}

use fontdue::{
    layout::{CoordinateSystem, Layout, TextStyle},
    Font,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{
            Anchor, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
    Connection, QueueHandle,
};

struct SimpleLayer {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    width: u32,
    height: u32,
    layer: LayerSurface,
    font: fontdue::Font,
}

impl CompositorHandler for SimpleLayer {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Not needed for this example.
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        // Not needed for this example.
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        self.draw(qh);
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // Not needed for this example.
    }
}

impl OutputHandler for SimpleLayer {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for SimpleLayer {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {}

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 == 0 || configure.new_size.1 == 0 {
            self.width = 256;
            self.height = 256;
        } else {
            self.width = configure.new_size.0;
            self.height = configure.new_size.1;
        }
        self.draw(qh);
    }
}

impl ShmHandler for SimpleLayer {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

pub fn rasterize_string(
    font: &Font,
    text: &str,
    px: f32,
    y_pos: usize,
    canvas: &mut [u8],
    canvas_stride: usize,
) {
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.append(&[font], &TextStyle::new(text, px, 0));

    let bottom_alignment_y = layout
        .glyphs()
        .iter()
        .map(|g| font.horizontal_line_metrics(g.key.px).unwrap().ascent - g.y)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();

    for glyph in layout.glyphs() {
        let glyph_x = glyph.x.round() as usize;
        let glyph_y = bottom_alignment_y
            - (font.horizontal_line_metrics(glyph.key.px).unwrap().ascent - glyph.y);
        let glyph_y = glyph_y.round() as usize;

        let (_, coverage) = font.rasterize(glyph.parent, px);
        for x in 0..glyph.width {
            for y in 0..glyph.height {
                let canvas_idx = (x + glyph_x) * 4 + (y + y_pos + glyph_y) * canvas_stride * 4;
                let glyph_pixel = coverage[x + y * glyph.width] / 2;
                canvas[canvas_idx] = glyph_pixel;
                canvas[canvas_idx + 1] = glyph_pixel;
                canvas[canvas_idx + 2] = glyph_pixel;
                canvas[canvas_idx + 3] = glyph_pixel;
            }
        }
    }
}

impl SimpleLayer {
    pub fn draw(&mut self, qh: &QueueHandle<Self>) {
        let width = self.width;
        let height = self.height;
        let stride = self.width as i32 * 4;

        let (buffer, canvas) = self
            .pool
            .create_buffer(
                width as i32,
                height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("create buffer");

        rasterize_string(
            &self.font,
            "Activate Linux",
            28.0,
            0,
            canvas,
            self.width as usize,
        );

        rasterize_string(
            &self.font,
            "Go to Settings to activate Linux.",
            16.0,
            32,
            canvas,
            self.width as usize,
        );

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);

        self.layer
            .wl_surface()
            .frame(qh, self.layer.wl_surface().clone());

        buffer
            .attach_to(self.layer.wl_surface())
            .expect("buffer attach");
        self.layer.commit();
    }
}

delegate_compositor!(SimpleLayer);
delegate_output!(SimpleLayer);
delegate_shm!(SimpleLayer);
delegate_layer!(SimpleLayer);
delegate_registry!(SimpleLayer);

impl ProvidesRegistryState for SimpleLayer {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

pub fn wayland_main() {
    env_logger::init();

    let conn = Connection::connect_to_env().unwrap();
    let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor is not available");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer shell is not available");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");

    let region = Region::new(&compositor).unwrap();
    let surface = compositor.create_surface(&qh);
    surface.set_input_region(Some(region.wl_region()));
    region.wl_region().destroy();

    let width = 335;
    let height = 110;

    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, None::<String>, None);
    layer.set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
    layer.set_size(width, height);
    layer.commit();
    let pool = SlotPool::new((width * height * 4) as usize, &shm).expect("Failed to create pool");

    let font = fontdue::Font::from_bytes(
        include_bytes!("../resources/Roboto[wdth,wght].ttf") as &[u8],
        fontdue::FontSettings::default(),
    )
    .unwrap();

    let mut simple_layer = SimpleLayer {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        font,
        pool,
        width,
        height,
        layer,
    };

    loop {
        event_queue.blocking_dispatch(&mut simple_layer).unwrap();
    }
}

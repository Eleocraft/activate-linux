mod wayland;
mod x11;

fn main() {
    match std::env::var("WAYLAND_DISPLAY") {
        Ok(_) => {
            wayland::wayland_main();
        }
        Err(_) => {
            x11::x11_main().unwrap();
        }
    }
}

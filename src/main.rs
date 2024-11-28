mod wayland;
mod x11;

fn main() {
    let mut header: Option<String> = None;
    let mut caption: Option<String> = None;
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--header" || arg == "-H" {
            header = args.next()
        }

        if arg == "--caption" || arg == "-c" {
            caption = args.next()
        }

        if arg == "--help" || arg == "-h" {
            println!("Usage: activate-linux [options]");
            println!(
                "\t -H '{{}}' or --header '{{}}' to set the header (Default: \"Activate Linux\")"
            );
            println!("\t -c '{{}}' or --caption '{{}}' to set the caption (Default: \"Go to Settings to activate Linux.\")");
            println!("\t -h or --help to show this message");
            return;
        }
    }
    match std::env::var("WAYLAND_DISPLAY") {
        Ok(_) => {
            wayland::wayland_main(header, caption);
        }
        Err(_) => {
            x11::x11_main(header, caption).unwrap();
        }
    }
}

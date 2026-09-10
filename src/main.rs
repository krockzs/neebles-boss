use std::process::Command;

const VERSION: &str = "1.0.0";
const UI_PATH: &str = "/opt/neebles/client/ui/neebles-ui";

fn print_version() {
    println!("N.E.E.B.L.E.S. Boss {}", VERSION);
}

fn print_help() {
    println!("N.E.E.B.L.E.S. Boss {}", VERSION);
    println!();
    println!("Usage:");
    println!("  neebles");
    println!("  neebles --version");
    println!("  neebles --help");
}

fn launch_ui() {
    match Command::new(UI_PATH).spawn() {
        Ok(_) => {}
        Err(error) => {
            eprintln!("N.E.E.B.L.E.S.: could not launch UI at {UI_PATH}: {error}");
            std::process::exit(1);
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        None => launch_ui(),
        Some("--version") | Some("-V") => print_version(),
        Some("--help") | Some("-h") => print_help(),
        Some(other) => {
            eprintln!("N.E.E.B.L.E.S.: unknown argument: {other}");
            std::process::exit(2);
        }
    }
}

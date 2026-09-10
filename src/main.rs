const VERSION: &str = "0.0.1";

fn print_version() {
    println!("N.E.E.B.L.E.S. Boss {}", VERSION);
}

fn print_help() {
    println!("N.E.E.B.L.E.S. Boss {}", VERSION);
    println!();
    println!("Usage:");
    println!("  neebles --version");
    println!("  neebles --help");
}

fn main() {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("--version") | Some("-V") => print_version(),
        Some("--help") | Some("-h") | None => print_help(),
        Some(other) => {
            eprintln!("N.E.E.B.L.E.S.: unknown argument: {other}");
            std::process::exit(2);
        }
    }
}

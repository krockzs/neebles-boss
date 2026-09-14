mod cli;
mod config;
mod dependencies;
mod dispatcher;
mod ipc;
mod languages;
mod local_installer;
mod modules;
mod notifications;
mod privileges;
mod request;
mod tray;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    std::process::exit(cli::run(args));
}

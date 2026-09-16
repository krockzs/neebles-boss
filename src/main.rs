mod cli;
mod config;
mod contracts;
mod dependencies;
mod dispatcher;
mod ipc;
mod languages;
mod local_installer;
mod module_ipc;
mod modules;
mod notifications;
mod privileges;
mod request;
mod runtime_identity;
mod settings;
mod stage0;
mod surface_state;
mod tray;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    std::process::exit(cli::run(args));
}

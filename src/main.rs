mod cli;
mod config;
mod contracts;
mod dispatcher;
mod external;
mod ipc;
mod languages;
mod module_ipc;
mod modules;
pub mod nightmare;
mod notifications;
mod privileges;
mod request;
mod runtime_identity;
mod settings;
mod surface_state;
mod tray;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    std::process::exit(cli::run(args));
}

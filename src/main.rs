mod cli;
mod config;
mod dependencies;
mod dispatcher;
mod languages;
mod modules;
mod notifications;
mod privileges;
mod request;

pub const VERSION: &str = "1.0.1";

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    std::process::exit(cli::run(args));
}

mod cli;
mod config;
mod contracts;
mod dispatcher;
mod external;
mod ipc;
mod languages;
mod lifecycle;
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

    let (authority_supply, args) =
        match neebles_backend::domestic_authority_supply_process::
            split_authority_supply_process_argument(args)
        {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!("N.E.E.B.L.E.S. Boss process input rejected: {error}");
                std::process::exit(2);
            }
        };

    if let Some(authority_supply) = authority_supply {
        if let Err(error) =
            neebles_backend::domestic_authority_supply_process::initialize_authority_supply_process(
                authority_supply,
            )
        {
            eprintln!("N.E.E.B.L.E.S. Boss AuthoritySupply rejected: {error}");
            std::process::exit(2);
        }
    }

    std::process::exit(cli::run(args));
}

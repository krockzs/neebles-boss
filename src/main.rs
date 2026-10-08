mod boss_update;
mod cli;
mod config;
mod contracts;
mod critical_update;
mod dispatcher;
mod external;
mod ipc;
mod languages;
mod lifecycle;
mod lifecycle_arsenal;
mod lifecycle_available_capabilities;
mod lifecycle_battlefield;
mod lifecycle_capabilities;
mod lifecycle_communication;
mod lifecycle_control;
mod lifecycle_dag_executor;
mod lifecycle_event;
mod lifecycle_execution;
mod lifecycle_executor;
mod lifecycle_failure;
mod lifecycle_fire_control;
mod lifecycle_flow;
mod lifecycle_governor_binding;
mod lifecycle_governor_bridge;
mod lifecycle_governor_runtime;
mod lifecycle_human;
mod lifecycle_intelligence;
mod lifecycle_ir;
mod lifecycle_object_transition_executor;
mod lifecycle_objects;
mod lifecycle_observer;
mod lifecycle_operation;
mod lifecycle_orchestration;
mod lifecycle_pipeline;
mod lifecycle_process;
mod lifecycle_require;
mod lifecycle_require_human;
mod lifecycle_resolver;
mod lifecycle_result;
mod lifecycle_state;
mod lifecycle_telemetry;
mod lifecycle_transition_executor;
mod module_dependency_policy;
mod module_ipc;
mod module_preinstall;
mod modules;
mod network_boundary;
pub mod nightmare;
mod notification_presenter;
mod notifications;
mod privileges;
mod request;
mod runtime_identity;
mod settings;
mod surface_content;
mod surface_contract;
mod surface_projection;
mod surface_requirement_resolver;
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

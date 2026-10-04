use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

pub fn command(world_name: &str, arguments: &[OsString]) -> Result<Command, String> {
    let registry =
        neebles_backend::domestic_authority_supply_process::process_supplied_authority_registry()?;

    let grants = neebles_backend::domestic_authority_supply::build_authority_grant_set(
        registry,
        ["platform.filesystem_boundary", "system.dns_resolver_config"],
    )?;

    let platform_descriptor = grants.descriptor_path(registry, "platform.filesystem_boundary")?;

    let dns_descriptor = grants.descriptor_path(registry, "system.dns_resolver_config")?;

    let platform =
        neebles_backend::domestic_platform_authority::load_platform_authority_descriptor(
            &platform_descriptor,
            "platform.filesystem_boundary",
        )?;

    let dns =
        neebles_backend::domestic_external_data_authority::load_external_data_authority_descriptor(
            &dns_descriptor,
            "system.dns_resolver_config",
        )?;

    let runtime_manifest =
        neebles_backend::domestic_runtime_authority::current_boss_runtime_manifest()?;

    let plan =
        neebles_backend::domestic_boundary_execution::compose_materialized_boundary_execution_plan(
            &runtime_manifest,
            world_name,
            &platform,
            &[dns],
            &std::collections::BTreeMap::new(),
            true,
            true,
            true,
            Some(Path::new("/tmp")),
        )?;

    neebles_backend::domestic_boundary_execution::build_pure_materialized_boundary_execution_command(
        &plan, arguments,
    )
}

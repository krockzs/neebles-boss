use std::path::PathBuf;

const DOMESTIC_WORKSPACE_AUTHORITY: &str = "neebles.domestic_workspace";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleMaterialTerritory {
    pub package_pool: PathBuf,
    pub essential_package_pool: PathBuf,
    pub material_root: PathBuf,
    pub runtime_lease_root: PathBuf,
}

pub fn resolve_module_material_territory() -> Result<ModuleMaterialTerritory, String> {
    let registry = crate::domestic_authority_supply_process::process_supplied_authority_registry()?;

    let grants = crate::domestic_authority_supply::build_authority_grant_set(
        registry,
        [DOMESTIC_WORKSPACE_AUTHORITY],
    )?;

    let descriptor_path = grants.descriptor_path(registry, DOMESTIC_WORKSPACE_AUTHORITY)?;

    let descriptor =
        crate::domestic_writable_data_authority::load_writable_data_authority_descriptor(
            &descriptor_path,
            DOMESTIC_WORKSPACE_AUTHORITY,
        )?;

    let modules = descriptor.root.join("modules");
    let packages = modules.join("packages");
    let essentials = packages.join("essentials");
    let material = modules.join("material");
    let runtime_leases = modules.join("runtime-leases");

    let packages_grant = crate::domestic_writable_data_authority::grant_writable_data_subpath(
        &descriptor,
        &packages,
        &packages,
    )?;

    let essentials_grant = crate::domestic_writable_data_authority::grant_writable_data_subpath(
        &descriptor,
        &essentials,
        &essentials,
    )?;

    let material_grant = crate::domestic_writable_data_authority::grant_writable_data_subpath(
        &descriptor,
        &material,
        &material,
    )?;

    let runtime_lease_grant = crate::domestic_writable_data_authority::grant_writable_data_subpath(
        &descriptor,
        &runtime_leases,
        &runtime_leases,
    )?;

    Ok(ModuleMaterialTerritory {
        package_pool: packages_grant.source,
        essential_package_pool: essentials_grant.source,
        material_root: material_grant.source,
        runtime_lease_root: runtime_lease_grant.source,
    })
}

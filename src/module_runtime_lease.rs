use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::module_material_binding::{load_material_binding, MaterialBinding};
use crate::module_material_territory::{
    resolve_module_material_territory, ModuleMaterialTerritory,
};
use crate::module_materialization::{materialize_runtime_layers, RuntimeMaterializationReport};

const RUNTIME_MANIFEST: &str = "domestic-runtime.json";

#[derive(Debug)]
pub struct ModuleRuntimeLease {
    temporary: tempfile::TempDir,
    module: String,
    version: String,
    custom_revision: String,
    manifest_path: PathBuf,
    rootfs_path: PathBuf,
    report: RuntimeMaterializationReport,
}

impl ModuleRuntimeLease {
    pub fn module(&self) -> &str {
        &self.module
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn custom_revision(&self) -> &str {
        &self.custom_revision
    }

    pub fn root(&self) -> &Path {
        self.temporary.path()
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn rootfs_path(&self) -> &Path {
        &self.rootfs_path
    }

    pub fn report(&self) -> &RuntimeMaterializationReport {
        &self.report
    }
}

fn ensure_runtime_lease_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute() {
        return Err(format!(
            "module runtime lease root must be absolute: {}",
            root.display()
        ));
    }

    fs::create_dir_all(root).map_err(|error| {
        format!(
            "could not create module runtime lease root {}: {error}",
            root.display()
        )
    })?;

    let metadata = fs::symlink_metadata(root).map_err(|error| {
        format!(
            "could not inspect module runtime lease root {}: {error}",
            root.display()
        )
    })?;

    if !metadata.file_type().is_dir() {
        return Err(format!(
            "module runtime lease root is not a directory: {}",
            root.display()
        ));
    }

    Ok(())
}

fn write_runtime_manifest(payload: &[u8], target: &Path) -> Result<(), String> {
    if fs::symlink_metadata(target).is_ok() {
        return Err(format!(
            "module runtime lease manifest destination already exists: {}",
            target.display()
        ));
    }

    fs::write(target, payload).map_err(|error| {
        format!(
            "could not write module runtime lease manifest {}: {error}",
            target.display()
        )
    })?;

    fs::set_permissions(target, fs::Permissions::from_mode(0o644)).map_err(|error| {
        format!(
            "could not set module runtime lease manifest mode {}: {error}",
            target.display()
        )
    })
}

fn create_runtime_lease_from_binding(
    binding: MaterialBinding,
    essential_package_pool: &Path,
    module_package_pool: &Path,
    runtime_lease_root: &Path,
) -> Result<ModuleRuntimeLease, String> {
    ensure_runtime_lease_root(runtime_lease_root)?;

    let temporary = tempfile::Builder::new()
        .prefix(&format!(".neebles-runtime-lease-{}-", binding.module))
        .tempdir_in(runtime_lease_root)
        .map_err(|error| {
            format!(
                "could not create module runtime lease under {}: {error}",
                runtime_lease_root.display()
            )
        })?;

    let rootfs_path = temporary.path().join("rootfs");

    let report = materialize_runtime_layers(
        &binding.essential_layer,
        essential_package_pool,
        &binding.module_recipe,
        module_package_pool,
        &rootfs_path,
    )?;

    let manifest_path = temporary.path().join(RUNTIME_MANIFEST);

    write_runtime_manifest(&binding.runtime_manifest_payload, &manifest_path)?;

    crate::domestic_runtime_authority::validate_materialized_runtime_manifest(&manifest_path)?;

    let resolved_root =
        crate::domestic_runtime_authority::resolve_materialized_runtime_root(&manifest_path)?;

    let expected_root = fs::canonicalize(&rootfs_path).map_err(|error| {
        format!(
            "could not canonicalize module runtime lease rootfs {}: {error}",
            rootfs_path.display()
        )
    })?;

    if resolved_root != expected_root {
        return Err(format!(
            "module runtime lease manifest resolves unexpected root: expected={} actual={}",
            expected_root.display(),
            resolved_root.display()
        ));
    }

    Ok(ModuleRuntimeLease {
        temporary,
        module: binding.module,
        version: binding.version,
        custom_revision: binding.custom_revision,
        manifest_path,
        rootfs_path,
        report,
    })
}

pub fn create_module_runtime_lease(module_id: &str) -> Result<ModuleRuntimeLease, String> {
    let territory = resolve_module_material_territory()?;

    create_module_runtime_lease_in_territory(module_id, &territory)
}

pub fn create_module_runtime_lease_in_territory(
    module_id: &str,
    territory: &ModuleMaterialTerritory,
) -> Result<ModuleRuntimeLease, String> {
    let binding = load_material_binding(&territory.material_root, module_id)?;

    create_runtime_lease_from_binding(
        binding,
        &territory.essential_package_pool,
        &territory.package_pool,
        &territory.runtime_lease_root,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::module_material::{MaterialLayer, MaterialRecipe};

    fn runtime_payload() -> Vec<u8> {
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "1",
            "name": "neebles-domestic-runtime",
            "root": "rootfs",
            "worlds": {
                "fixture.runtime": {
                    "categories": {
                        "runtime_paths": {
                            "category": "runtime_paths",
                            "value": "fixture-runtime",
                            "declared_targets": ["."],
                            "resolved_targets": ["."]
                        }
                    }
                }
            }
        }))
        .expect("runtime fixture must serialize")
    }

    fn binding(payload: Vec<u8>) -> MaterialBinding {
        MaterialBinding {
            module: "fixture".to_string(),
            version: "1.0.0".to_string(),
            custom_revision: "c".repeat(40),

            essential_layer: MaterialLayer {
                packages: Vec::new(),
                entries: Vec::new(),
            },

            module_recipe: MaterialRecipe {
                module: "fixture".to_string(),
                version: "1.0.0".to_string(),
                packages: Vec::new(),
                entries: Vec::new(),
            },

            runtime_manifest_payload: payload,
            construction_payload: serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name": "neebles-domestic-construction",
                "subject": "fixture",
                "steps": [
                    {
                        "id": "runtime",
                        "runtime_authority": "modules.runtime",
                        "world": "modules.fixture",
                        "execution": "foreground",
                        "session": false
                    }
                ]
            }))
            .expect("Construction fixture must serialize"),

            path: PathBuf::from("/fixture/material/fixture"),
        }
    }

    fn pools(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let essential = root.join("packages/essentials");

        let modules = root.join("packages");

        let leases = root.join("runtime-leases");

        fs::create_dir_all(&essential).expect("Essential fixture pool");

        fs::create_dir_all(&modules).expect("module fixture pool");

        fs::create_dir_all(&leases).expect("lease fixture root");

        (essential, modules, leases)
    }

    #[test]
    fn runtime_lease_owns_ephemeral_rootfs_until_drop() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let (essential, modules, leases) = pools(temporary.path());

        let lease = create_runtime_lease_from_binding(
            binding(runtime_payload()),
            &essential,
            &modules,
            &leases,
        )
        .expect("runtime lease must materialize");

        let lease_root = lease.root().to_path_buf();

        assert!(lease.rootfs_path().is_dir());
        assert!(lease.manifest_path().is_file());

        drop(lease);

        assert!(
            !lease_root.exists(),
            "runtime rootfs survived lease destruction"
        );
    }

    #[test]
    fn runtime_lease_preserves_authenticated_manifest_bytes() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let (essential, modules, leases) = pools(temporary.path());

        let payload = runtime_payload();

        let lease = create_runtime_lease_from_binding(
            binding(payload.clone()),
            &essential,
            &modules,
            &leases,
        )
        .expect("runtime lease must materialize");

        assert_eq!(
            fs::read(lease.manifest_path()).expect("lease manifest must read"),
            payload
        );
    }

    #[test]
    fn concurrent_runtime_leases_are_physically_independent() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let (essential, modules, leases) = pools(temporary.path());

        let first = create_runtime_lease_from_binding(
            binding(runtime_payload()),
            &essential,
            &modules,
            &leases,
        )
        .expect("first runtime lease");

        let second = create_runtime_lease_from_binding(
            binding(runtime_payload()),
            &essential,
            &modules,
            &leases,
        )
        .expect("second runtime lease");

        assert_ne!(first.root(), second.root());

        let first_root = first.root().to_path_buf();

        let second_root = second.root().to_path_buf();

        drop(first);

        assert!(!first_root.exists());
        assert!(second_root.exists());

        drop(second);

        assert!(!second_root.exists());
    }
}

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::module_material_binding::{
    load_installed_material_binding, InstalledMaterialBinding, MaterialBinding,
};
use crate::module_material_territory::{
    resolve_module_material_territory, ModuleMaterialTerritory,
};
use crate::module_materialization::{materialize_runtime_layers, RuntimeMaterializationReport};

const RUNTIME_MANIFEST: &str = "domestic-runtime.json";

#[derive(Debug)]
pub struct ModuleRuntimeLease {
    temporary: Option<tempfile::TempDir>,
    lease_root: PathBuf,
    module: String,
    version: String,
    custom_revision: String,
    manifest_path: PathBuf,
    rootfs_path: PathBuf,
    report: Option<RuntimeMaterializationReport>,
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
        &self.lease_root
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn rootfs_path(&self) -> &Path {
        &self.rootfs_path
    }

    pub fn report(&self) -> Option<&RuntimeMaterializationReport> {
        self.report.as_ref()
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
        lease_root: temporary.path().to_path_buf(),
        temporary: Some(temporary),
        module: binding.module,
        version: binding.version,
        custom_revision: binding.custom_revision,
        manifest_path,
        rootfs_path,
        report: Some(report),
    })
}

/// Select a logical rootfs from the authenticated Construction contract. No
/// caller-supplied physical paths or fallback to another rootfs are permitted.
pub fn create_module_runtime_lease_for_rootfs(
    module_id: &str,
    rootfs_id: &str,
) -> Result<ModuleRuntimeLease, String> {
    let territory = resolve_module_material_territory()?;
    create_module_runtime_lease_in_territory_for_rootfs(module_id, rootfs_id, &territory)
}

fn validate_bound_rootfs_selection(
    binding: &InstalledMaterialBinding,
    rootfs_id: &str,
) -> Result<(), String> {
    if !crate::module_material::valid_module_id(rootfs_id) {
        return Err(format!("invalid runtime rootfs identity: {rootfs_id}"));
    }
    let raw = std::str::from_utf8(binding.construction_payload())
        .map_err(|error| format!("bound Construction is not UTF-8: {error}"))?;
    let declaration = crate::domestic_construction::DomesticConstructionDeclaration::parse(raw)?;
    if declaration.subject != binding.module() {
        return Err(format!(
            "bound Construction belongs to {} rather than {}",
            declaration.subject,
            binding.module()
        ));
    }
    if !declaration.rootfs.iter().any(|entry| entry.id == rootfs_id) {
        return Err(format!("undeclared runtime rootfs identity: {rootfs_id}"));
    }
    // Schema 3 binds every logical rootfs independently to its certified
    // recipe. Schema 2 has a single historic recipe, so an explicit
    // declaration is still mandatory, but no rootfs name is inferred.
    match binding {
        InstalledMaterialBinding::MultiRootfs(multi) => {
            if !multi.rootfs_recipes.contains_key(rootfs_id) {
                return Err(format!(
                    "no authenticated recipe for runtime rootfs: {rootfs_id}"
                ));
            }
        }
        InstalledMaterialBinding::Legacy(_) => {
            if declaration.rootfs.len() != 1 {
                return Err("schema-2 recipe cannot authenticate multiple rootfs".to_string());
            }
        }
    }
    Ok(())
}

/// Resolve a previously published, certified persistent rootfs. Runtime never
/// downloads or extracts packages; missing or mismatched material fails closed.
fn persistent_runtime_lease_from_binding(
    binding: InstalledMaterialBinding,
    installed_directory: &Path,
    rootfs_id: &str,
) -> Result<ModuleRuntimeLease, String> {
    if !crate::module_material::valid_module_id(rootfs_id) {
        return Err(format!("invalid runtime rootfs id: {rootfs_id}"));
    }
    let installed_directory = fs::canonicalize(installed_directory)
        .map_err(|e| format!("cannot resolve installed module directory: {e}"))?;
    let root = installed_directory.join("rootfs").join(rootfs_id);
    let root_metadata = fs::symlink_metadata(&root)
        .map_err(|e| format!("persistent rootfs {rootfs_id} is not installed: {e}"))?;
    if !root_metadata.file_type().is_dir() {
        return Err(format!(
            "persistent rootfs {rootfs_id} must be a real directory"
        ));
    }
    let manifest_path = root.join(RUNTIME_MANIFEST);
    let metadata = fs::symlink_metadata(&manifest_path)
        .map_err(|e| format!("missing persistent runtime manifest: {e}"))?;
    if !metadata.file_type().is_file() {
        return Err("persistent runtime manifest must be a regular file".to_string());
    }
    let actual = fs::read(&manifest_path)
        .map_err(|e| format!("cannot read persistent runtime manifest: {e}"))?;
    if actual != binding.runtime_manifest_payload() {
        return Err("persistent runtime manifest differs from authenticated binding".to_string());
    }
    crate::domestic_runtime_authority::validate_materialized_runtime_manifest(&manifest_path)?;
    let rootfs_path = root.join("rootfs");
    let metadata = fs::symlink_metadata(&rootfs_path)
        .map_err(|e| format!("persistent runtime rootfs missing: {e}"))?;
    if !metadata.file_type().is_dir() {
        return Err("persistent runtime rootfs must be a real directory".to_string());
    }
    let resolved =
        crate::domestic_runtime_authority::resolve_materialized_runtime_root(&manifest_path)?;
    let expected = fs::canonicalize(&rootfs_path)
        .map_err(|e| format!("cannot resolve persistent rootfs: {e}"))?;
    if expected != resolved {
        return Err("persistent runtime manifest resolves outside its rootfs".to_string());
    }
    Ok(ModuleRuntimeLease {
        temporary: None,
        lease_root: root,
        module: binding.module().to_string(),
        version: binding.version().to_string(),
        custom_revision: binding.custom_revision().to_string(),
        manifest_path,
        rootfs_path,
        report: None, // No materialization occurs when creating a persistent lease.
    })
}

/// Locate a published module without relying on its directory having the same
/// spelling as its manifest identity. This code is shared by the library runtime;
/// modules.rs (binary) has its own installation resolver.
fn locate_installed_module(module_id: &str) -> Result<PathBuf, String> {
    if !crate::module_material::valid_module_id(module_id) {
        return Err(format!("invalid module identity: {module_id}"));
    }
    let root = std::env::var_os("NEEBLES_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/opt/neebles"))
        .join("modules");
    locate_installed_module_in_root(module_id, &root)
}

/// Resolve the same production inventory without changing NEEBLES_ROOT in tests.
fn locate_installed_module_in_root(module_id: &str, root: &Path) -> Result<PathBuf, String> {
    if !crate::module_material::valid_module_id(module_id) {
        return Err(format!("invalid module identity: {module_id}"));
    }
    let mut matched: Option<PathBuf> = None;
    let entries = fs::read_dir(&root)
        .map_err(|e| format!("cannot inspect installed module inventory: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot inspect installed module entry: {e}"))?;
        let path = entry.path();
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let manifest = path.join("manifest.json");
        let meta = match fs::symlink_metadata(&manifest) {
            Ok(meta) if meta.file_type().is_file() => meta,
            _ => continue,
        };
        let _ = meta;
        let raw = fs::read(&manifest)
            .map_err(|e| format!("cannot read published module manifest: {e}"))?;
        let value: serde_json::Value = serde_json::from_slice(&raw)
            .map_err(|e| format!("invalid published module manifest: {e}"))?;
        if value.get("name").and_then(serde_json::Value::as_str) != Some(module_id) {
            continue;
        }
        if matched.replace(path).is_some() {
            return Err(format!("duplicate installed module identity: {module_id}"));
        }
    }
    matched.ok_or_else(|| format!("module is not installed: {module_id}"))
}

pub fn create_module_runtime_lease_in_territory_for_rootfs(
    module_id: &str,
    rootfs_id: &str,
    territory: &ModuleMaterialTerritory,
) -> Result<ModuleRuntimeLease, String> {
    let installed_directory = locate_installed_module(module_id)?;
    create_module_runtime_lease_from_installed_in_territory(
        module_id,
        rootfs_id,
        territory,
        &installed_directory,
    )
}

/// The production resolution tail, extracted so its installed-module location
/// can be supplied by the caller and independently certified without global
/// NEEBLES_ROOT mutation in concurrent Rust tests.
fn create_module_runtime_lease_from_installed_in_territory(
    module_id: &str,
    rootfs_id: &str,
    territory: &ModuleMaterialTerritory,
    installed_directory: &Path,
) -> Result<ModuleRuntimeLease, String> {
    let binding = load_installed_material_binding(&territory.material_root, module_id)?;
    validate_bound_rootfs_selection(&binding, rootfs_id)?;
    persistent_runtime_lease_from_binding(binding, installed_directory, rootfs_id)
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
                "rootfs": [{"id":"caca-de-gato"}],
                "steps": [
                    {
                        "id": "runtime",
                        "runtime_authority": "modules.runtime",
                        "rootfs": "caca-de-gato",
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

    #[test]
    fn rootfs_selection_requires_declaration_without_defaults() {
        let fixture = binding(runtime_payload());
        assert!(validate_bound_rootfs_selection(
            &InstalledMaterialBinding::Legacy(fixture.clone()),
            "caca-de-gato"
        )
        .is_ok());
        assert!(validate_bound_rootfs_selection(
            &InstalledMaterialBinding::Legacy(fixture.clone()),
            "principal"
        )
        .is_err());
        assert!(validate_bound_rootfs_selection(
            &InstalledMaterialBinding::Legacy(fixture.clone()),
            "../unsafe"
        )
        .is_err());
    }

    #[test]
    fn rootfs_selection_requires_exact_bound_declaration() {
        let mut fixture = binding(runtime_payload());
        fixture.construction_payload = serde_json::to_vec(&serde_json::json!({
            "schema": "1", "name": "neebles-domestic-construction",
            "subject": "fixture",
            "rootfs": [{"id": "vnc"}],
            "steps": [{"id": "open", "runtime_authority": "modules.runtime",
                "rootfs": "vnc", "world": "modules.fixture",
                "execution": "foreground", "session": false}]
        }))
        .unwrap();
        assert!(validate_bound_rootfs_selection(
            &InstalledMaterialBinding::Legacy(fixture.clone()),
            "vnc"
        )
        .is_ok());
        assert!(validate_bound_rootfs_selection(
            &InstalledMaterialBinding::Legacy(fixture.clone()),
            "ssh"
        )
        .is_err());
        fixture.module = "foreign".to_string();
        assert!(validate_bound_rootfs_selection(
            &InstalledMaterialBinding::Legacy(fixture.clone()),
            "vnc"
        )
        .is_err());
    }

    #[test]
    fn schema_three_requires_exact_authenticated_recipe_and_declaration() {
        use crate::module_material_binding::MaterialBindingV3;
        let legacy = binding(runtime_payload());
        let mut multi = MaterialBindingV3 {
            module: legacy.module.clone(),
            version: legacy.version.clone(),
            custom_revision: legacy.custom_revision.clone(),
            essential_layer: legacy.essential_layer.clone(),
            rootfs_recipes: std::collections::BTreeMap::new(),
            runtime_manifest_payload: legacy.runtime_manifest_payload.clone(),
            construction_payload: legacy.construction_payload.clone(),
            path: legacy.path.clone(),
        };
        assert!(validate_bound_rootfs_selection(
            &InstalledMaterialBinding::MultiRootfs(multi.clone()),
            "caca-de-gato"
        )
        .is_err());
        multi
            .rootfs_recipes
            .insert("caca-de-gato".to_string(), legacy.module_recipe.clone());
        assert!(validate_bound_rootfs_selection(
            &InstalledMaterialBinding::MultiRootfs(multi.clone()),
            "caca-de-gato"
        )
        .is_ok());
        assert!(validate_bound_rootfs_selection(
            &InstalledMaterialBinding::MultiRootfs(multi),
            "unlisted"
        )
        .is_err());
    }

    #[test]
    fn schema_two_refuses_ambiguous_multi_rootfs_declaration() {
        let mut legacy = binding(runtime_payload());
        let mut declaration: serde_json::Value =
            serde_json::from_slice(&legacy.construction_payload).unwrap();
        declaration["rootfs"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"id":"second"}));
        legacy.construction_payload = serde_json::to_vec(&declaration).unwrap();
        assert!(validate_bound_rootfs_selection(
            &InstalledMaterialBinding::Legacy(legacy),
            "caca-de-gato"
        )
        .is_err());
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
    fn persistent_runtime_lease_reuses_installed_material_without_extraction() {
        let temporary = tempfile::tempdir().unwrap();
        let candidate = temporary.path().join("installed");
        fs::create_dir(&candidate).unwrap();
        let input = binding(runtime_payload());
        let first = crate::module_persistent_rootfs::prepare_persistent_rootfs(
            &candidate,
            "principal",
            &input.essential_layer,
            temporary.path(),
            &input.module_recipe,
            temporary.path(),
            &input.runtime_manifest_payload,
        )
        .unwrap();
        let lease_a = persistent_runtime_lease_from_binding(
            InstalledMaterialBinding::Legacy(input.clone()),
            &candidate,
            "principal",
        )
        .unwrap();
        let lease_b = persistent_runtime_lease_from_binding(
            InstalledMaterialBinding::Legacy(input),
            &candidate,
            "principal",
        )
        .unwrap();
        assert_eq!(lease_a.rootfs_path(), lease_b.rootfs_path());
        assert_eq!(lease_a.rootfs_path(), first.rootfs_path);
        assert!(lease_a.report().is_none());
        drop(lease_a);
        drop(lease_b);
        assert!(
            first.rootfs_path.exists(),
            "persistent rootfs must outlive runtime leases"
        );
    }

    #[test]
    fn persistent_runtime_rejects_manifest_tampering() {
        let temporary = tempfile::tempdir().unwrap();
        let candidate = temporary.path().join("installed");
        fs::create_dir(&candidate).unwrap();
        let input = binding(runtime_payload());
        let first = crate::module_persistent_rootfs::prepare_persistent_rootfs(
            &candidate,
            "principal",
            &input.essential_layer,
            temporary.path(),
            &input.module_recipe,
            temporary.path(),
            &input.runtime_manifest_payload,
        )
        .unwrap();
        fs::write(&first.manifest_path, b"{}").unwrap();
        assert!(persistent_runtime_lease_from_binding(
            InstalledMaterialBinding::Legacy(input),
            &candidate,
            "principal"
        )
        .is_err());
    }

    #[test]
    fn schema_three_selects_two_independent_persistent_runtime_roots() {
        use crate::module_material_binding::{InstalledMaterialBinding, MaterialBindingV3};
        use std::collections::BTreeMap;

        let temporary = tempfile::tempdir().unwrap();
        let installed = temporary.path().join("installed");
        fs::create_dir(&installed).unwrap();
        let legacy = binding(runtime_payload());
        let mut construction: serde_json::Value =
            serde_json::from_slice(&legacy.construction_payload).unwrap();
        construction["rootfs"] = serde_json::json!([
            {"id":"vnc"}, {"id":"ssh"}
        ]);
        construction["steps"] = serde_json::json!([
            {"id":"open-vnc", "runtime_authority":"modules.runtime",
             "rootfs":"vnc", "world":"modules.fixture",
             "execution":"foreground", "session":false},
            {"id":"open-ssh", "runtime_authority":"modules.runtime",
             "rootfs":"ssh", "world":"modules.fixture",
             "execution":"foreground", "session":false}
        ]);
        let mut recipes = BTreeMap::new();
        recipes.insert("vnc".to_string(), legacy.module_recipe.clone());
        recipes.insert("ssh".to_string(), legacy.module_recipe.clone());
        let bound = InstalledMaterialBinding::MultiRootfs(MaterialBindingV3 {
            module: legacy.module.clone(),
            version: legacy.version.clone(),
            custom_revision: legacy.custom_revision.clone(),
            essential_layer: legacy.essential_layer.clone(),
            rootfs_recipes: recipes,
            runtime_manifest_payload: legacy.runtime_manifest_payload.clone(),
            construction_payload: serde_json::to_vec(&construction).unwrap(),
            path: legacy.path.clone(),
        });
        for id in ["vnc", "ssh"] {
            crate::module_persistent_rootfs::prepare_persistent_rootfs(
                &installed,
                id,
                &legacy.essential_layer,
                temporary.path(),
                &legacy.module_recipe,
                temporary.path(),
                &legacy.runtime_manifest_payload,
            )
            .unwrap();
            validate_bound_rootfs_selection(&bound, id).unwrap();
        }
        assert!(validate_bound_rootfs_selection(&bound, "principal").is_err());
        assert!(validate_bound_rootfs_selection(&bound, "../escape").is_err());

        let vnc = persistent_runtime_lease_from_binding(bound.clone(), &installed, "vnc").unwrap();
        let ssh = persistent_runtime_lease_from_binding(bound.clone(), &installed, "ssh").unwrap();
        assert_ne!(vnc.rootfs_path(), ssh.rootfs_path());
        assert!(vnc.rootfs_path().starts_with(installed.join("rootfs/vnc")));
        assert!(ssh.rootfs_path().starts_with(installed.join("rootfs/ssh")));
        assert!(vnc.report().is_none() && ssh.report().is_none());
        drop(vnc);
        drop(ssh);
        assert!(installed.join("rootfs/vnc/rootfs").is_dir());
        assert!(installed.join("rootfs/ssh/rootfs").is_dir());

        let ssh_manifest = installed.join("rootfs/ssh/domestic-runtime.json");
        fs::write(&ssh_manifest, b"{}").unwrap();
        assert!(persistent_runtime_lease_from_binding(bound.clone(), &installed, "ssh").is_err());
        assert!(persistent_runtime_lease_from_binding(bound, &installed, "vnc").is_ok());
    }

    #[test]
    fn schema_three_installed_binding_selects_independent_rootfs_and_fails_closed() {
        use crate::module_material_binding::{
            prepare_material_binding_v3, MaterialBindingV3Input, RootfsBindingInput,
        };
        let temp = tempfile::tempdir().unwrap();
        let module_dir = temp.path().join("published-module");
        let material_root = temp.path().join("material");
        fs::create_dir(&module_dir).unwrap();
        let runtime = runtime_payload();
        let mut construction: serde_json::Value =
            serde_json::from_slice(&binding(runtime.clone()).construction_payload).unwrap();
        construction["rootfs"] = serde_json::json!([{"id":"alpha"},{"id":"beta"}]);
        construction["steps"] = serde_json::json!([
            {"id":"open-alpha", "runtime_authority":"modules.runtime",
             "rootfs":"alpha", "world":"modules.fixture",
             "execution":"foreground", "session":false},
            {"id":"open-beta", "runtime_authority":"modules.runtime",
             "rootfs":"beta", "world":"modules.fixture",
             "execution":"foreground", "session":false}
        ]);
        let construction = serde_json::to_vec(&construction).unwrap();
        let rootfs: Vec<_> = ["alpha", "beta"]
            .into_iter()
            .map(|id| RootfsBindingInput {
                id: id.to_owned(),
                packages_payload: Vec::new(),
                manifest_payload: serde_json::json!({
                    "module":"fixture", "version":"1.0.0", "rootfs":id, "entries":[]
                })
                .to_string()
                .into_bytes(),
            })
            .collect();
        let input = MaterialBindingV3Input {
            module: "fixture".into(),
            version: "1.0.0".into(),
            custom_revision: "c".repeat(40),
            essential_packages_payload: Vec::new(),
            essential_manifest_payload: br#"{"entries":[]}"#.to_vec(),
            runtime_manifest_payload: runtime.clone(),
            construction_payload: construction,
            rootfs,
        };
        prepare_material_binding_v3(&material_root, input)
            .unwrap()
            .activate_install()
            .unwrap();
        let legacy = binding(runtime.clone());
        for id in ["alpha", "beta"] {
            crate::module_persistent_rootfs::prepare_persistent_rootfs(
                &module_dir,
                id,
                &legacy.essential_layer,
                temp.path(),
                &legacy.module_recipe,
                temp.path(),
                &runtime,
            )
            .unwrap();
        }
        let territory = ModuleMaterialTerritory {
            material_root: material_root.clone(),
            package_pool: temp.path().join("packages"),
            essential_package_pool: temp.path().join("essentials"),
            runtime_lease_root: temp.path().join("leases"),
        };
        let alpha = create_module_runtime_lease_from_installed_in_territory(
            "fixture",
            "alpha",
            &territory,
            &module_dir,
        )
        .unwrap();
        let beta = create_module_runtime_lease_from_installed_in_territory(
            "fixture",
            "beta",
            &territory,
            &module_dir,
        )
        .unwrap();
        assert_ne!(alpha.rootfs_path(), beta.rootfs_path());
        assert_eq!(alpha.version(), "1.0.0");
        assert_eq!(beta.custom_revision(), "c".repeat(40));
        assert!(create_module_runtime_lease_from_installed_in_territory(
            "fixture",
            "missing",
            &territory,
            &module_dir,
        )
        .is_err());
        assert!(create_module_runtime_lease_from_installed_in_territory(
            "foreign",
            "alpha",
            &territory,
            &module_dir,
        )
        .is_err());
        fs::write(module_dir.join("rootfs/beta/domestic-runtime.json"), b"{}").unwrap();
        assert!(create_module_runtime_lease_from_installed_in_territory(
            "fixture",
            "beta",
            &territory,
            &module_dir,
        )
        .is_err());
        assert!(create_module_runtime_lease_from_installed_in_territory(
            "fixture",
            "alpha",
            &territory,
            &module_dir,
        )
        .is_ok());
    }

    #[test]
    fn production_locator_resolves_manifest_identity_not_directory_spelling() {
        let temp = tempfile::tempdir().unwrap();
        let inventory = temp.path().join("modules");
        fs::create_dir(&inventory).unwrap();
        let renamed = inventory.join("any-physical-directory");
        fs::create_dir(&renamed).unwrap();
        fs::write(renamed.join("manifest.json"), br#"{"name":"fixture"}"#).unwrap();
        assert_eq!(
            locate_installed_module_in_root("fixture", &inventory).unwrap(),
            renamed
        );
        assert!(locate_installed_module_in_root("unknown", &inventory).is_err());
        assert!(locate_installed_module_in_root("../fixture", &inventory).is_err());
    }

    #[test]
    fn production_locator_rejects_duplicate_identity_and_ignores_symlink_entries() {
        let temp = tempfile::tempdir().unwrap();
        let inventory = temp.path().join("modules");
        fs::create_dir(&inventory).unwrap();
        let first = inventory.join("one");
        let second = inventory.join("two");
        for path in [&first, &second] {
            fs::create_dir(path).unwrap();
            fs::write(path.join("manifest.json"), br#"{"name":"fixture"}"#).unwrap();
        }
        assert!(locate_installed_module_in_root("fixture", &inventory).is_err());
        fs::remove_dir_all(&second).unwrap();
        std::os::unix::fs::symlink(&first, &second).unwrap();
        assert_eq!(
            locate_installed_module_in_root("fixture", &inventory).unwrap(),
            first
        );
        fs::remove_dir_all(&first).unwrap();
        assert!(locate_installed_module_in_root("fixture", &inventory).is_err());
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

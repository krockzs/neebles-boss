//! Persistent rootfs materialization in an already staged module directory.
//! The caller owns installation/update publication and rollback of that directory.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::module_material::{valid_module_id, MaterialLayer, MaterialRecipe};
use crate::module_materialization::{materialize_runtime_layers, RuntimeMaterializationReport};

#[derive(Debug)]
pub struct PreparedPersistentRootfs {
    pub id: String,
    pub manifest_path: PathBuf,
    pub rootfs_path: PathBuf,
    pub report: RuntimeMaterializationReport,
}

/// Materialize a certified environment beneath a module *candidate*, never a live tree.
/// The caller must ensure `staged_module_dir` is its private, unpublished staging directory.
/// Publication is a rename of the completed environment, not a partial live write.
pub fn prepare_persistent_rootfs(
    staged_module_dir: &Path,
    id: &str,
    essential: &MaterialLayer,
    essential_pool: &Path,
    module: &MaterialRecipe,
    module_pool: &Path,
    runtime_manifest_payload: &[u8],
) -> Result<PreparedPersistentRootfs, String> {
    if !valid_module_id(id) {
        return Err(format!("invalid persistent rootfs id: {id}"));
    }
    if !staged_module_dir.is_absolute() {
        return Err("staged module directory must be absolute".to_string());
    }
    let metadata = fs::symlink_metadata(staged_module_dir)
        .map_err(|e| format!("cannot inspect staged module directory: {e}"))?;
    if !metadata.file_type().is_dir() {
        return Err("staged module directory must be a real directory".to_string());
    }
    let environments = staged_module_dir.join("rootfs");
    if let Ok(metadata) = fs::symlink_metadata(&environments) {
        if !metadata.file_type().is_dir() {
            return Err("rootfs territory must be a real directory".to_string());
        }
    } else {
        fs::create_dir(&environments)
            .map_err(|e| format!("cannot create rootfs territory: {e}"))?;
    }
    let destination = environments.join(id);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(format!(
            "persistent rootfs already exists: {}",
            destination.display()
        ));
    }
    let staged = tempfile::Builder::new()
        .prefix(".neebles-rootfs-candidate-")
        .tempdir_in(&environments)
        .map_err(|e| format!("cannot stage persistent rootfs: {e}"))?;
    let composed = staged.path().join("rootfs");
    let report =
        materialize_runtime_layers(essential, essential_pool, module, module_pool, &composed)?;
    let manifest_path = staged.path().join("domestic-runtime.json");
    fs::write(&manifest_path, runtime_manifest_payload)
        .map_err(|e| format!("cannot write persistent runtime manifest: {e}"))?;
    fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o644))
        .map_err(|e| format!("cannot set runtime manifest permissions: {e}"))?;
    crate::domestic_runtime_authority::validate_materialized_runtime_manifest(&manifest_path)?;
    let actual =
        crate::domestic_runtime_authority::resolve_materialized_runtime_root(&manifest_path)?;
    let expected = fs::canonicalize(&composed)
        .map_err(|e| format!("cannot canonicalize composed rootfs: {e}"))?;
    if actual != expected {
        return Err(format!("runtime manifest root mismatch for rootfs {id}"));
    }
    fs::rename(staged.path(), &destination)
        .map_err(|e| format!("cannot publish prepared rootfs {id}: {e}"))?;
    Ok(PreparedPersistentRootfs {
        id: id.to_string(),
        manifest_path: destination.join("domestic-runtime.json"),
        rootfs_path: destination.join("rootfs"),
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn runtime_payload() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema": "1", "name": "neebles-domestic-runtime", "root": "rootfs",
            "worlds": { "fixture.runtime": { "categories": {
                "runtime_paths": { "category": "runtime_paths", "value": "fixture-runtime",
                    "declared_targets": ["."], "resolved_targets": ["."] }
            } } }
        }))
        .unwrap()
    }
    fn build(root: &Path, id: &str, payload: &[u8]) -> Result<PreparedPersistentRootfs, String> {
        let module_dir = root.join("candidate");
        fs::create_dir_all(&module_dir).unwrap();
        let essential = MaterialLayer {
            packages: vec![],
            entries: vec![],
        };
        let module = MaterialRecipe {
            module: "fixture".into(),
            version: "1".into(),
            packages: vec![],
            entries: vec![],
        };
        prepare_persistent_rootfs(&module_dir, id, &essential, root, &module, root, payload)
    }
    #[test]
    fn prepared_rootfs_survives_return_and_can_coexist() {
        let tmp = tempfile::tempdir().unwrap();
        let first = build(tmp.path(), "principal", &runtime_payload()).unwrap();
        let second = build(tmp.path(), "vnc", &runtime_payload()).unwrap();
        assert!(first.rootfs_path.is_dir());
        assert!(second.rootfs_path.is_dir());
        assert_ne!(first.rootfs_path, second.rootfs_path);
        assert!(first.manifest_path.is_file());
        assert!(second.manifest_path.is_file());
    }
    #[test]
    fn rejects_duplicate_and_unsafe_ids() {
        let tmp = tempfile::tempdir().unwrap();
        build(tmp.path(), "principal", &runtime_payload()).unwrap();
        assert!(build(tmp.path(), "principal", &runtime_payload()).is_err());
        assert!(build(tmp.path(), "../escape", &runtime_payload()).is_err());
    }
    #[test]
    fn invalid_manifest_does_not_publish_candidate() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(build(tmp.path(), "broken", b"not-json").is_err());
        assert!(!tmp.path().join("candidate/rootfs/broken").exists());
    }
    /// 08D: opt-in, real materializer rehearsal against local CUSTOM V2.
    /// Run only with NEEBLES_08D_CANDIDATE set to a fresh Downloads directory.
    #[test]
    #[ignore = "08D requires the Ryzen local package pools and an explicit Downloads candidate"]
    fn multirootfs_08d_test_module_real_materialization() {
        use crate::domestic_construction::DomesticConstructionDeclaration;
        use crate::module_material::{parse_material_layer, parse_material_recipe};
        let home = Path::new("/home/thomyorke/NEEBLES");
        let custom = home.join("neebles-custom");
        let module = home.join("neebles-test-module");
        let output =
            std::env::var_os("NEEBLES_08D_CANDIDATE").expect("08D requires NEEBLES_08D_CANDIDATE");
        let candidate = PathBuf::from(output);
        assert!(candidate.is_absolute(), "08D candidate must be absolute");
        assert!(
            candidate.starts_with("/home/thomyorke/Downloads/"),
            "08D writes only beneath Downloads"
        );
        assert!(!candidate.exists(), "08D refuses an existing destination");

        let construction = fs::read_to_string(module.join("construction.json")).unwrap();
        let declaration = DomesticConstructionDeclaration::parse(&construction).unwrap();
        assert_eq!(declaration.subject, "test-module");
        assert_eq!(
            declaration.rootfs.len(),
            1,
            "Test Module owns exactly one rootfs"
        );
        assert_eq!(declaration.rootfs[0].id, "python3.13-tk");
        assert_eq!(
            declaration.rootfs[0].domination_world.as_deref(),
            Some("modules.python3.13-tk")
        );

        let base = custom.join("runtime/manifests/modules");
        let essentials = parse_material_layer(
            "Essential",
            &fs::read_to_string(base.join("essentials.packages.tsv")).unwrap(),
            &fs::read(base.join("essentials.manifest.json")).unwrap(),
        )
        .unwrap();
        let manifest = fs::read(module.join("rootfs/python3.13-tk.manifest.json")).unwrap();
        let version = serde_json::from_slice::<serde_json::Value>(&manifest).unwrap()["version"]
            .as_str()
            .unwrap()
            .to_string();
        let recipe = parse_material_recipe(
            "test-module",
            &version,
            &fs::read_to_string(module.join("rootfs/python3.13-tk.packages.tsv")).unwrap(),
            &manifest,
        )
        .unwrap();
        assert_eq!(essentials.packages.len(), 59);
        assert_eq!(recipe.packages.len(), 32);
        let essential_pool = custom.join("runtime/modules/packages/essentials");
        let module_pool = custom.join("runtime/modules/packages");
        assert!(
            essential_pool.is_dir() && module_pool.is_dir(),
            "package pool missing"
        );
        let runtime = fs::read(custom.join("runtime/modules/domestic-runtime.json")).unwrap();

        fs::create_dir(&candidate).expect("08D creates only its private candidate");
        let built = prepare_persistent_rootfs(
            &candidate,
            "python3.13-tk",
            &essentials,
            &essential_pool,
            &recipe,
            &module_pool,
            &runtime,
        )
        .expect("08D real persistent rootfs materialization failed");
        assert!(built.rootfs_path.join("usr/bin/python3.13").exists());
        assert!(built.manifest_path.is_file());
        assert_eq!(fs::read_dir(candidate.join("rootfs")).unwrap().count(), 1);
        eprintln!("08D_ROOTFS={}", built.rootfs_path.display());
        eprintln!("08D_MANIFEST={}", built.manifest_path.display());
        eprintln!("08D_REPORT={:?}", built.report);
    }
}

use crate::module_material::{
    parse_material_layer, parse_material_recipe, valid_module_id, MaterialLayer, MaterialRecipe,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const BINDING_SCHEMA: u32 = 2;
const BINDING_INDEX: &str = "binding.json";
const ESSENTIAL_PACKAGES: &str = "essentials.packages.tsv";
const ESSENTIAL_MANIFEST: &str = "essentials.manifest.json";
const MODULE_PACKAGES: &str = "module.packages.tsv";
const MODULE_MANIFEST: &str = "module.manifest.json";
const RUNTIME_MANIFEST: &str = "domestic-runtime.json";
const CONSTRUCTION: &str = "construction.json";

#[derive(Debug, Clone)]
pub struct MaterialBindingInput {
    pub module: String,
    pub version: String,
    pub custom_revision: String,
    pub essential_packages_payload: Vec<u8>,
    pub essential_manifest_payload: Vec<u8>,
    pub module_packages_payload: Vec<u8>,
    pub module_manifest_payload: Vec<u8>,
    pub runtime_manifest_payload: Vec<u8>,
    pub construction_payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct MaterialBinding {
    pub module: String,
    pub version: String,
    pub custom_revision: String,
    pub essential_layer: MaterialLayer,
    pub module_recipe: MaterialRecipe,
    pub runtime_manifest_payload: Vec<u8>,
    pub construction_payload: Vec<u8>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MaterialBindingHashes {
    essentials_packages: String,
    essentials_manifest: String,
    module_packages: String,
    module_manifest: String,
    runtime_manifest: String,
    construction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct MaterialBindingIndex {
    schema: u32,
    module: String,
    version: String,
    custom_revision: String,
    sha256: MaterialBindingHashes,
}

#[derive(Debug)]
pub struct PreparedMaterialBinding {
    temporary: tempfile::TempDir,
    staged_path: PathBuf,
    active_path: PathBuf,
}

fn valid_revision(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256_bytes(payload: &[u8]) -> String {
    Sha256::digest(payload)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn ensure_material_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute() {
        return Err(format!(
            "material binding root must be absolute: {}",
            root.display()
        ));
    }

    fs::create_dir_all(root).map_err(|error| {
        format!(
            "could not create material binding root {}: {error}",
            root.display()
        )
    })?;

    let metadata = fs::symlink_metadata(root).map_err(|error| {
        format!(
            "could not inspect material binding root {}: {error}",
            root.display()
        )
    })?;

    if !metadata.file_type().is_dir() {
        return Err(format!(
            "material binding root is not a directory: {}",
            root.display()
        ));
    }

    Ok(())
}

fn write_binding_file(path: &Path, payload: &[u8]) -> Result<(), String> {
    fs::write(path, payload).map_err(|error| {
        format!(
            "could not write material binding file {}: {error}",
            path.display()
        )
    })?;

    fs::set_permissions(path, fs::Permissions::from_mode(0o644)).map_err(|error| {
        format!(
            "could not set material binding file mode {}: {error}",
            path.display()
        )
    })
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "could not inspect material binding file {}: {error}",
            path.display()
        )
    })?;

    if !metadata.file_type().is_file() {
        return Err(format!(
            "material binding entry is not a regular file: {}",
            path.display()
        ));
    }

    fs::read(path).map_err(|error| {
        format!(
            "could not read material binding file {}: {error}",
            path.display()
        )
    })
}

fn validate_runtime_manifest_payload(payload: &[u8]) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|error| format!("invalid material binding runtime manifest JSON: {error}"))?;

    if !value.is_object() {
        return Err("material binding runtime manifest must be a JSON object".to_string());
    }

    Ok(())
}

fn expected_binding_entries() -> BTreeSet<String> {
    [
        BINDING_INDEX,
        ESSENTIAL_PACKAGES,
        ESSENTIAL_MANIFEST,
        MODULE_PACKAGES,
        MODULE_MANIFEST,
        RUNTIME_MANIFEST,
        CONSTRUCTION,
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn validate_exact_binding_directory(path: &Path) -> Result<(), String> {
    let mut actual = BTreeSet::new();

    let entries = fs::read_dir(path).map_err(|error| {
        format!(
            "could not read material binding directory {}: {error}",
            path.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "could not read material binding directory entry {}: {error}",
                path.display()
            )
        })?;

        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "material binding filename is not UTF-8".to_string())?;

        actual.insert(name);
    }

    let expected = expected_binding_entries();

    if actual != expected {
        return Err(format!(
            "material binding directory membership mismatch: expected={expected:?} actual={actual:?}"
        ));
    }

    Ok(())
}

fn verify_hash(name: &str, payload: &[u8], expected: &str) -> Result<(), String> {
    let actual = sha256_bytes(payload);

    if actual != expected {
        return Err(format!(
            "material binding sha256 mismatch for {name}: expected={expected} actual={actual}"
        ));
    }

    Ok(())
}

pub fn prepare_material_binding(
    material_root: &Path,
    input: MaterialBindingInput,
) -> Result<PreparedMaterialBinding, String> {
    ensure_material_root(material_root)?;

    if !valid_module_id(&input.module) {
        return Err(format!(
            "invalid module id for material binding: {}",
            input.module
        ));
    }

    if input.version.trim().is_empty() {
        return Err("material binding version cannot be empty".to_string());
    }

    if !valid_revision(&input.custom_revision) {
        return Err(format!(
            "invalid CUSTOM V2 revision for material binding: {}",
            input.custom_revision
        ));
    }

    let essential_packages_text = std::str::from_utf8(&input.essential_packages_payload)
        .map_err(|error| format!("Essential package selector is not UTF-8: {error}"))?;

    let module_packages_text = std::str::from_utf8(&input.module_packages_payload)
        .map_err(|error| format!("module package selector is not UTF-8: {error}"))?;

    parse_material_layer(
        "Essential",
        essential_packages_text,
        &input.essential_manifest_payload,
    )?;

    parse_material_recipe(
        &input.module,
        &input.version,
        module_packages_text,
        &input.module_manifest_payload,
    )?;

    validate_runtime_manifest_payload(&input.runtime_manifest_payload)?;

    let active_path = material_root.join(&input.module);

    let temporary = tempfile::Builder::new()
        .prefix(".neebles-material-binding-")
        .tempdir_in(material_root)
        .map_err(|error| {
            format!(
                "could not create material binding staging under {}: {error}",
                material_root.display()
            )
        })?;

    let staged_path = temporary.path().join("binding");

    fs::create_dir(&staged_path).map_err(|error| {
        format!(
            "could not create material binding staging directory {}: {error}",
            staged_path.display()
        )
    })?;

    fs::set_permissions(&staged_path, fs::Permissions::from_mode(0o755)).map_err(|error| {
        format!(
            "could not set material binding staging mode {}: {error}",
            staged_path.display()
        )
    })?;

    write_binding_file(
        &staged_path.join(ESSENTIAL_PACKAGES),
        &input.essential_packages_payload,
    )?;

    write_binding_file(
        &staged_path.join(ESSENTIAL_MANIFEST),
        &input.essential_manifest_payload,
    )?;

    write_binding_file(
        &staged_path.join(MODULE_PACKAGES),
        &input.module_packages_payload,
    )?;

    write_binding_file(
        &staged_path.join(MODULE_MANIFEST),
        &input.module_manifest_payload,
    )?;

    write_binding_file(
        &staged_path.join(RUNTIME_MANIFEST),
        &input.runtime_manifest_payload,
    )?;

    write_binding_file(&staged_path.join(CONSTRUCTION), &input.construction_payload)?;

    let index = MaterialBindingIndex {
        schema: BINDING_SCHEMA,
        module: input.module,
        version: input.version,
        custom_revision: input.custom_revision.to_ascii_lowercase(),
        sha256: MaterialBindingHashes {
            essentials_packages: sha256_bytes(&input.essential_packages_payload),
            essentials_manifest: sha256_bytes(&input.essential_manifest_payload),
            module_packages: sha256_bytes(&input.module_packages_payload),
            module_manifest: sha256_bytes(&input.module_manifest_payload),
            runtime_manifest: sha256_bytes(&input.runtime_manifest_payload),
            construction: sha256_bytes(&input.construction_payload),
        },
    };

    let index_payload = serde_json::to_vec_pretty(&index)
        .map_err(|error| format!("could not serialize material binding index: {error}"))?;

    write_binding_file(&staged_path.join(BINDING_INDEX), &index_payload)?;

    validate_exact_binding_directory(&staged_path)?;

    Ok(PreparedMaterialBinding {
        temporary,
        staged_path,
        active_path,
    })
}

impl PreparedMaterialBinding {
    pub fn staged_path(&self) -> &Path {
        &self.staged_path
    }

    pub fn active_path(&self) -> &Path {
        &self.active_path
    }

    pub fn activate_install(self) -> Result<PathBuf, String> {
        if fs::symlink_metadata(&self.active_path).is_ok() {
            return Err(format!(
                "material binding already active: {}",
                self.active_path.display()
            ));
        }

        fs::rename(&self.staged_path, &self.active_path).map_err(|error| {
            format!(
                "could not activate material binding {} -> {}: {error}",
                self.staged_path.display(),
                self.active_path.display()
            )
        })?;

        drop(self.temporary);

        Ok(self.active_path)
    }
}

pub fn load_material_binding(
    material_root: &Path,
    module_id: &str,
) -> Result<MaterialBinding, String> {
    ensure_material_root(material_root)?;

    if !valid_module_id(module_id) {
        return Err(format!(
            "invalid module id for material binding load: {module_id}"
        ));
    }

    let path = material_root.join(module_id);

    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        format!(
            "could not inspect active material binding {}: {error}",
            path.display()
        )
    })?;

    if !metadata.file_type().is_dir() {
        return Err(format!(
            "active material binding is not a directory: {}",
            path.display()
        ));
    }

    validate_exact_binding_directory(&path)?;

    let index_payload = read_regular_file(&path.join(BINDING_INDEX))?;

    let index: MaterialBindingIndex = serde_json::from_slice(&index_payload)
        .map_err(|error| format!("invalid material binding index JSON: {error}"))?;

    if index.schema != BINDING_SCHEMA {
        return Err(format!(
            "unsupported material binding schema: {}",
            index.schema
        ));
    }

    if index.module != module_id {
        return Err(format!(
            "material binding module mismatch: expected={module_id} actual={}",
            index.module
        ));
    }

    if !valid_module_id(&index.module) {
        return Err(format!(
            "invalid module id in material binding index: {}",
            index.module
        ));
    }

    if index.version.trim().is_empty() {
        return Err("material binding index version cannot be empty".to_string());
    }

    if !valid_revision(&index.custom_revision) {
        return Err(format!(
            "invalid CUSTOM V2 revision in material binding index: {}",
            index.custom_revision
        ));
    }

    let essential_packages = read_regular_file(&path.join(ESSENTIAL_PACKAGES))?;
    let essential_manifest = read_regular_file(&path.join(ESSENTIAL_MANIFEST))?;
    let module_packages = read_regular_file(&path.join(MODULE_PACKAGES))?;
    let module_manifest = read_regular_file(&path.join(MODULE_MANIFEST))?;
    let runtime_manifest = read_regular_file(&path.join(RUNTIME_MANIFEST))?;
    let construction = read_regular_file(&path.join(CONSTRUCTION))?;

    verify_hash(
        ESSENTIAL_PACKAGES,
        &essential_packages,
        &index.sha256.essentials_packages,
    )?;

    verify_hash(
        ESSENTIAL_MANIFEST,
        &essential_manifest,
        &index.sha256.essentials_manifest,
    )?;

    verify_hash(
        MODULE_PACKAGES,
        &module_packages,
        &index.sha256.module_packages,
    )?;

    verify_hash(
        MODULE_MANIFEST,
        &module_manifest,
        &index.sha256.module_manifest,
    )?;

    verify_hash(
        RUNTIME_MANIFEST,
        &runtime_manifest,
        &index.sha256.runtime_manifest,
    )?;

    verify_hash(CONSTRUCTION, &construction, &index.sha256.construction)?;

    let essential_packages_text = std::str::from_utf8(&essential_packages)
        .map_err(|error| format!("stored Essential package selector is not UTF-8: {error}"))?;

    let module_packages_text = std::str::from_utf8(&module_packages)
        .map_err(|error| format!("stored module package selector is not UTF-8: {error}"))?;

    let essential_layer =
        parse_material_layer("Essential", essential_packages_text, &essential_manifest)?;

    let module_recipe = parse_material_recipe(
        &index.module,
        &index.version,
        module_packages_text,
        &module_manifest,
    )?;

    validate_runtime_manifest_payload(&runtime_manifest)?;

    Ok(MaterialBinding {
        module: index.module,
        version: index.version,
        custom_revision: index.custom_revision,
        essential_layer,
        module_recipe,
        runtime_manifest_payload: runtime_manifest,
        construction_payload: construction,
        path,
    })
}

#[derive(Debug)]
pub struct StagedMaterialBindingRemoval {
    temporary: Option<tempfile::TempDir>,
    staged_path: PathBuf,
    active_path: PathBuf,
    committed: bool,
}

pub fn stage_material_binding_removal(
    material_root: &Path,
    module_id: &str,
) -> Result<Option<StagedMaterialBindingRemoval>, String> {
    ensure_material_root(material_root)?;

    if !valid_module_id(module_id) {
        return Err(format!(
            "invalid module id for material binding removal: {module_id}"
        ));
    }

    let active_path = material_root.join(module_id);

    let metadata = match fs::symlink_metadata(&active_path) {
        Ok(metadata) => metadata,

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }

        Err(error) => {
            return Err(format!(
                "could not inspect active material binding {}: {error}",
                active_path.display()
            ));
        }
    };

    if !metadata.file_type().is_dir() {
        return Err(format!(
            "active material binding is not a directory: {}",
            active_path.display()
        ));
    }

    let temporary = tempfile::Builder::new()
        .prefix(".neebles-material-binding-remove-")
        .tempdir_in(material_root)
        .map_err(|error| {
            format!(
                "could not create material binding removal staging under {}: {error}",
                material_root.display()
            )
        })?;

    let staged_path = temporary.path().join("binding");

    fs::rename(&active_path, &staged_path).map_err(|error| {
        format!(
            "could not stage material binding removal {} -> {}: {error}",
            active_path.display(),
            staged_path.display()
        )
    })?;

    Ok(Some(StagedMaterialBindingRemoval {
        temporary: Some(temporary),
        staged_path,
        active_path,
        committed: false,
    }))
}

impl StagedMaterialBindingRemoval {
    pub fn active_path(&self) -> &Path {
        &self.active_path
    }

    pub fn staged_path(&self) -> &Path {
        &self.staged_path
    }

    pub fn restore(mut self) -> Result<(), String> {
        if fs::symlink_metadata(&self.active_path).is_ok() {
            return Err(format!(
                "cannot restore material binding because active path exists: {}",
                self.active_path.display()
            ));
        }

        fs::rename(&self.staged_path, &self.active_path).map_err(|error| {
            format!(
                "could not restore material binding {} -> {}: {error}",
                self.staged_path.display(),
                self.active_path.display()
            )
        })?;

        self.committed = true;

        drop(self.temporary.take());

        Ok(())
    }

    pub fn finalize(mut self) {
        self.committed = true;

        drop(self.temporary.take());
    }
}

impl Drop for StagedMaterialBindingRemoval {
    fn drop(&mut self) {
        if self.committed || fs::symlink_metadata(&self.staged_path).is_err() {
            return;
        }

        if fs::symlink_metadata(&self.active_path).is_ok() {
            eprintln!(
                "N.E.E.B.L.E.S.: material binding rollback refused because active path already exists: {}",
                self.active_path.display()
            );

            return;
        }

        if let Err(error) = fs::rename(&self.staged_path, &self.active_path) {
            eprintln!(
                "N.E.E.B.L.E.S.: CRITICAL: could not automatically restore material binding {} -> {}: {error}",
                self.staged_path.display(),
                self.active_path.display()
            );

            return;
        }

        self.committed = true;

        drop(self.temporary.take());
    }
}

#[derive(Debug)]
pub struct ActivatedMaterialBindingUpdate {
    temporary: Option<tempfile::TempDir>,
    previous_path: Option<PathBuf>,
    active_path: PathBuf,
    finalized: bool,
}

fn rollback_activated_material_binding_update(
    active_path: &Path,
    previous_path: Option<&Path>,
    temporary_root: &Path,
) -> Result<(), String> {
    let failed_path = temporary_root.join("failed-new-binding");

    if fs::symlink_metadata(&failed_path).is_ok() {
        return Err(format!(
            "material binding rollback staging path already exists: {}",
            failed_path.display()
        ));
    }

    let active_present = match fs::symlink_metadata(active_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir() {
                return Err(format!(
                    "active material binding is not a directory during rollback: {}",
                    active_path.display()
                ));
            }

            true
        }

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,

        Err(error) => {
            return Err(format!(
                "could not inspect active material binding during rollback {}: {error}",
                active_path.display()
            ));
        }
    };

    if active_present {
        fs::rename(active_path, &failed_path).map_err(|error| {
            format!(
                "could not move failed material binding aside {} -> {}: {error}",
                active_path.display(),
                failed_path.display()
            )
        })?;
    }

    let Some(previous_path) = previous_path else {
        return Ok(());
    };

    let previous_metadata = fs::symlink_metadata(previous_path).map_err(|error| {
        format!(
            "previous material binding is unavailable for rollback {}: {error}",
            previous_path.display()
        )
    })?;

    if !previous_metadata.file_type().is_dir() {
        return Err(format!(
            "previous material binding is not a directory: {}",
            previous_path.display()
        ));
    }

    if let Err(error) = fs::rename(previous_path, active_path) {
        if active_present {
            let _ = fs::rename(&failed_path, active_path);
        }

        return Err(format!(
            "could not restore previous material binding {} -> {}: {error}",
            previous_path.display(),
            active_path.display()
        ));
    }

    Ok(())
}

impl PreparedMaterialBinding {
    pub fn activate_update(self) -> Result<ActivatedMaterialBindingUpdate, String> {
        let previous_path = match fs::symlink_metadata(&self.active_path) {
            Ok(metadata) => {
                if !metadata.file_type().is_dir() {
                    return Err(format!(
                        "current material binding for update is not a directory: {}",
                        self.active_path.display()
                    ));
                }

                let previous_path = self.temporary.path().join("previous-binding");

                fs::rename(&self.active_path, &previous_path).map_err(|error| {
                    format!(
                        "could not stage previous material binding {} -> {}: {error}",
                        self.active_path.display(),
                        previous_path.display()
                    )
                })?;

                Some(previous_path)
            }

            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,

            Err(error) => {
                return Err(format!(
                    "could not inspect current material binding for update {}: {error}",
                    self.active_path.display()
                ));
            }
        };

        if let Err(error) = fs::rename(&self.staged_path, &self.active_path) {
            if let Some(previous_path) = previous_path.as_ref() {
                let rollback_error = fs::rename(previous_path, &self.active_path).err();

                return match rollback_error {
                    None => Err(format!(
                        "could not activate updated material binding {} -> {}: {error}; previous binding was restored",
                        self.staged_path.display(),
                        self.active_path.display()
                    )),

                    Some(rollback_error) => Err(format!(
                        "CRITICAL: could not activate updated material binding {} -> {}: {error}; previous binding rollback also failed: {rollback_error}",
                        self.staged_path.display(),
                        self.active_path.display()
                    )),
                };
            }

            return Err(format!(
                "could not activate first material binding during module update {} -> {}: {error}",
                self.staged_path.display(),
                self.active_path.display()
            ));
        }

        Ok(ActivatedMaterialBindingUpdate {
            temporary: Some(self.temporary),
            previous_path,
            active_path: self.active_path,
            finalized: false,
        })
    }
}

impl ActivatedMaterialBindingUpdate {
    pub fn active_path(&self) -> &Path {
        &self.active_path
    }

    pub fn rollback(mut self) -> Result<(), String> {
        let temporary_root = self
            .temporary
            .as_ref()
            .ok_or_else(|| "material binding update staging is unavailable".to_string())?
            .path()
            .to_path_buf();

        rollback_activated_material_binding_update(
            &self.active_path,
            self.previous_path.as_deref(),
            &temporary_root,
        )?;

        self.finalized = true;

        drop(self.temporary.take());

        Ok(())
    }

    pub fn finalize(mut self) {
        self.finalized = true;

        drop(self.temporary.take());
    }
}

impl Drop for ActivatedMaterialBindingUpdate {
    fn drop(&mut self) {
        if self.finalized {
            return;
        }

        let Some(temporary) = self.temporary.as_ref() else {
            return;
        };

        if let Err(error) = rollback_activated_material_binding_update(
            &self.active_path,
            self.previous_path.as_deref(),
            temporary.path(),
        ) {
            eprintln!(
                "N.E.E.B.L.E.S.: CRITICAL: automatic material binding update rollback failed: {error}"
            );

            return;
        }

        self.finalized = true;

        drop(self.temporary.take());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha(value: &str) -> String {
        value.repeat(64)
    }

    fn fixture() -> MaterialBindingInput {
        let essential_sha = sha("a");
        let module_sha = sha("b");

        let essential_packages =
            format!("base-files\t13.8\tamd64\tbase-files.deb\t{essential_sha}\n");

        let essential_manifest = serde_json::json!({
            "entries": [
                {
                    "path": "packages/base-files.deb",
                    "type": "file",
                    "mode": "0o664",
                    "size": 10,
                    "sha256": essential_sha
                },
                {
                    "path": "rootfs/usr",
                    "type": "directory",
                    "mode": "0o755"
                }
            ]
        });

        let module_packages = format!("fixture\t1.0\tamd64\tfixture.deb\t{module_sha}\n");

        let module_manifest = serde_json::json!({
            "module": "fixture",
            "version": "1.0.0",
            "entries": [
                {
                    "path": "packages/fixture.deb",
                    "type": "file",
                    "mode": "0o664",
                    "size": 20,
                    "sha256": module_sha
                },
                {
                    "path": "rootfs/usr/bin",
                    "type": "directory",
                    "mode": "0o755"
                }
            ]
        });

        let runtime_manifest = serde_json::json!({
            "schema": "1",
            "name": "neebles-domestic-runtime",
            "root": "rootfs",
            "worlds": {}
        });

        MaterialBindingInput {
            module: "fixture".to_string(),
            version: "1.0.0".to_string(),
            custom_revision: "c".repeat(40),
            essential_packages_payload: essential_packages.into_bytes(),
            essential_manifest_payload: serde_json::to_vec_pretty(&essential_manifest)
                .expect("Essential manifest fixture must serialize"),
            module_packages_payload: module_packages.into_bytes(),
            module_manifest_payload: serde_json::to_vec_pretty(&module_manifest)
                .expect("module manifest fixture must serialize"),
            runtime_manifest_payload: serde_json::to_vec_pretty(&runtime_manifest)
                .expect("runtime manifest fixture must serialize"),
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
        }
    }

    #[test]
    fn binding_roundtrip_preserves_authenticated_material() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let material_root = temporary.path().join("material");

        let prepared =
            prepare_material_binding(&material_root, fixture()).expect("binding must prepare");

        assert!(!prepared.active_path().exists());

        let active = prepared.activate_install().expect("binding must activate");

        assert!(active.is_dir());
        assert!(!active.join("rootfs").exists());

        let loaded = load_material_binding(&material_root, "fixture").expect("binding must load");

        assert_eq!(loaded.module, "fixture");
        assert_eq!(loaded.version, "1.0.0");
        assert_eq!(loaded.custom_revision, "c".repeat(40));
        assert_eq!(loaded.essential_layer.packages.len(), 1);
        assert_eq!(loaded.module_recipe.packages.len(), 1);
        assert_eq!(loaded.path, active);
    }

    #[test]
    fn binding_load_rejects_tampered_authority_blob() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let material_root = temporary.path().join("material");

        let active = prepare_material_binding(&material_root, fixture())
            .expect("binding must prepare")
            .activate_install()
            .expect("binding must activate");

        fs::write(active.join(RUNTIME_MANIFEST), br#"{"tampered":true}"#)
            .expect("fixture tamper must write");

        let error = load_material_binding(&material_root, "fixture")
            .expect_err("tampered binding must fail");

        assert!(error.contains("sha256 mismatch"));
    }

    #[test]
    fn binding_load_rejects_tampered_construction() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let material_root = temporary.path().join("material");

        let active = prepare_material_binding(&material_root, fixture())
            .expect("binding must prepare")
            .activate_install()
            .expect("binding must activate");

        fs::write(active.join(CONSTRUCTION), br#"{"tampered":true}"#)
            .expect("Construction tamper fixture must write");

        let error = load_material_binding(&material_root, "fixture")
            .expect_err("tampered Construction must fail");

        assert!(error.contains("sha256 mismatch"));
        assert!(error.contains(CONSTRUCTION));
    }

    #[test]
    fn binding_load_rejects_untracked_files() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let material_root = temporary.path().join("material");

        let active = prepare_material_binding(&material_root, fixture())
            .expect("binding must prepare")
            .activate_install()
            .expect("binding must activate");

        fs::write(active.join("foreign.txt"), b"foreign").expect("foreign fixture must write");

        let error = load_material_binding(&material_root, "fixture")
            .expect_err("binding with foreign file must fail");

        assert!(error.contains("membership mismatch"));
    }

    #[test]
    fn install_activation_refuses_to_replace_active_binding() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let material_root = temporary.path().join("material");

        prepare_material_binding(&material_root, fixture())
            .expect("first binding must prepare")
            .activate_install()
            .expect("first binding must activate");

        let second = prepare_material_binding(&material_root, fixture())
            .expect("second binding must prepare");

        let error = second
            .activate_install()
            .expect_err("install activation must not replace active binding");

        assert!(error.contains("already active"));

        let loaded = load_material_binding(&material_root, "fixture")
            .expect("original binding must survive");

        assert_eq!(loaded.module, "fixture");
    }

    #[test]
    fn staged_removal_restores_binding_when_not_committed() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let material_root = temporary.path().join("material");

        let active = prepare_material_binding(&material_root, fixture())
            .expect("binding must prepare")
            .activate_install()
            .expect("binding must activate");

        {
            let removal = stage_material_binding_removal(&material_root, "fixture")
                .expect("removal staging must succeed")
                .expect("active binding must stage");

            assert!(!active.exists());
            assert!(removal.staged_path().exists());
        }

        let loaded = load_material_binding(&material_root, "fixture")
            .expect("uncommitted removal must restore binding");

        assert_eq!(loaded.custom_revision, "c".repeat(40));
    }

    #[test]
    fn finalized_removal_leaves_no_active_binding() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let material_root = temporary.path().join("material");

        let active = prepare_material_binding(&material_root, fixture())
            .expect("binding must prepare")
            .activate_install()
            .expect("binding must activate");

        let removal = stage_material_binding_removal(&material_root, "fixture")
            .expect("removal staging must succeed")
            .expect("active binding must stage");

        removal.finalize();

        assert!(!active.exists());
    }

    #[test]
    fn update_drop_restores_previous_binding() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let material_root = temporary.path().join("material");

        prepare_material_binding(&material_root, fixture())
            .expect("original binding must prepare")
            .activate_install()
            .expect("original binding must activate");

        let mut next = fixture();
        next.custom_revision = "d".repeat(40);

        {
            let update = prepare_material_binding(&material_root, next)
                .expect("updated binding must prepare")
                .activate_update()
                .expect("updated binding must activate");

            let loaded = load_material_binding(&material_root, "fixture")
                .expect("updated binding must load");

            assert_eq!(loaded.custom_revision, "d".repeat(40));

            drop(update);
        }

        let restored = load_material_binding(&material_root, "fixture")
            .expect("previous binding must be restored");

        assert_eq!(restored.custom_revision, "c".repeat(40));
    }

    #[test]
    fn finalized_update_keeps_new_binding() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let material_root = temporary.path().join("material");

        prepare_material_binding(&material_root, fixture())
            .expect("original binding must prepare")
            .activate_install()
            .expect("original binding must activate");

        let mut next = fixture();
        next.custom_revision = "d".repeat(40);

        let update = prepare_material_binding(&material_root, next)
            .expect("updated binding must prepare")
            .activate_update()
            .expect("updated binding must activate");

        update.finalize();

        let loaded = load_material_binding(&material_root, "fixture")
            .expect("finalized updated binding must load");

        assert_eq!(loaded.custom_revision, "d".repeat(40));
    }

    #[test]
    fn update_without_previous_drop_removes_new_binding() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let material_root = temporary.path().join("material");

        let mut next = fixture();
        next.custom_revision = "d".repeat(40);

        {
            let update = prepare_material_binding(&material_root, next)
                .expect("first binding during update must prepare")
                .activate_update()
                .expect("first binding during update must activate");

            let loaded = load_material_binding(&material_root, "fixture")
                .expect("new binding must be active");

            assert_eq!(loaded.custom_revision, "d".repeat(40));

            drop(update);
        }

        assert!(
            !material_root.join("fixture").exists(),
            "failed legacy update left a new binding active"
        );
    }

    #[test]
    fn finalized_update_without_previous_keeps_new_binding() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let material_root = temporary.path().join("material");

        let mut next = fixture();
        next.custom_revision = "d".repeat(40);

        let update = prepare_material_binding(&material_root, next)
            .expect("first binding during update must prepare")
            .activate_update()
            .expect("first binding during update must activate");

        update.finalize();

        let loaded = load_material_binding(&material_root, "fixture")
            .expect("finalized first binding must remain active");

        assert_eq!(loaded.custom_revision, "d".repeat(40));
    }
}

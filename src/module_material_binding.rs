use crate::module_material::{
    parse_material_layer, parse_material_recipe, parse_rootfs_material_recipe, valid_module_id,
    MaterialLayer, MaterialRecipe,
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

/// A single independently identified rootfs delta, supplied as authenticated bytes.
/// This is the schema-3 preparation contract; schema-2 disk bindings are unchanged.
#[derive(Debug, Clone)]
pub struct RootfsBindingInput {
    pub id: String,
    pub packages_payload: Vec<u8>,
    pub manifest_payload: Vec<u8>,
}

/// Validate all rootfs recipes as one closed set before any persistence or
/// download. Prevent duplicate identities and incompatible SHA authorities.
pub fn validate_rootfs_binding_recipes(
    module: &str,
    version: &str,
    rootfs: &[RootfsBindingInput],
) -> Result<std::collections::BTreeMap<String, MaterialRecipe>, String> {
    if !valid_module_id(module) || version.trim().is_empty() {
        return Err("invalid module identity/version for rootfs binding".to_string());
    }
    if rootfs.is_empty() {
        return Err("rootfs binding must declare at least one environment".to_string());
    }
    let mut recipes = std::collections::BTreeMap::new();
    let mut package_authority: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    for item in rootfs {
        if !valid_module_id(&item.id) {
            return Err(format!("invalid rootfs binding id: {}", item.id));
        }
        if recipes.contains_key(&item.id) {
            return Err(format!("duplicate rootfs binding id: {}", item.id));
        }
        let selector = std::str::from_utf8(&item.packages_payload)
            .map_err(|error| format!("rootfs package selector is not UTF-8: {error}"))?;
        let recipe = parse_rootfs_material_recipe(
            module,
            version,
            &item.id,
            selector,
            &item.manifest_payload,
        )?;
        for package in &recipe.packages {
            match package_authority.get(&package.filename) {
                Some(previous) if previous != &package.sha256 => {
                    return Err(format!(
                        "package filename has incompatible SHA256 across rootfs: {}",
                        package.filename
                    ));
                }
                _ => {
                    package_authority.insert(package.filename.clone(), package.sha256.clone());
                }
            }
        }
        recipes.insert(item.id.clone(), recipe);
    }
    Ok(recipes)
}

/// Require the recipes to cover precisely the rootfs identities named by
/// Construction. Empty declarations are invalid; no implicit identity exists.
/// This checks a closed set; no rootfs can be silently omitted or injected.
pub fn validate_construction_rootfs_binding(
    module: &str,
    version: &str,
    construction_payload: &[u8],
    rootfs: &[RootfsBindingInput],
) -> Result<std::collections::BTreeMap<String, MaterialRecipe>, String> {
    let raw = std::str::from_utf8(construction_payload)
        .map_err(|error| format!("construction binding is not UTF-8: {error}"))?;
    let declaration = crate::domestic_construction::DomesticConstructionDeclaration::parse(raw)?;
    if declaration.subject != module {
        return Err(format!(
            "construction binding subject mismatch: expected={module} actual={}",
            declaration.subject
        ));
    }
    let recipes = validate_rootfs_binding_recipes(module, version, rootfs)?;
    if declaration.rootfs.is_empty() {
        return Err("rootfs binding requires explicit Construction declarations".to_string());
    }
    let declared: BTreeSet<String> = declaration
        .rootfs
        .iter()
        .map(|item| item.id.clone())
        .collect();
    let provided: BTreeSet<String> = recipes.keys().cloned().collect();
    if declared != provided {
        return Err(format!(
            "rootfs binding membership mismatch: declared={declared:?} supplied={provided:?}"
        ));
    }
    Ok(recipes)
}

/// Schema-3 recipe inventory, independent from any module-specific rootfs name.
/// Serialized names are derived solely from the authenticated declaration.
/// This is a format contract; schema-2 activation remains unchanged until the
/// schema-3 reader/writer transaction is connected in a subsequent patch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RootfsBindingIndexEntry {
    pub packages_file: String,
    pub manifest_file: String,
    pub packages_sha256: String,
    pub manifest_sha256: String,
}

/// An authenticated, closed membership index for arbitrary rootfs identities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RootfsBindingIndex {
    pub schema: u32,
    pub rootfs: std::collections::BTreeMap<String, RootfsBindingIndexEntry>,
}

/// Reject all unexpected rootfs files and any changed recipe payload.
/// Caller must validate the signed/controlled index provenance separately.
pub fn verify_rootfs_binding_index(
    index: &RootfsBindingIndex,
    rootfs: &[RootfsBindingInput],
) -> Result<(), String> {
    if index.schema != 3 || index.rootfs.is_empty() || index.rootfs.len() != rootfs.len() {
        return Err("rootfs binding schema or membership mismatch".to_string());
    }
    let mut seen = BTreeSet::new();
    for item in rootfs {
        if !valid_module_id(&item.id) || !seen.insert(item.id.clone()) {
            return Err("invalid or repeated rootfs binding identity".to_string());
        }
        let expected = index
            .rootfs
            .get(&item.id)
            .ok_or_else(|| format!("missing rootfs binding index entry: {}", item.id))?;
        let (packages_file, manifest_file) = rootfs_binding_filenames(&item.id)?;
        if expected.packages_file != packages_file || expected.manifest_file != manifest_file {
            return Err(format!("rootfs binding index path mismatch: {}", item.id));
        }
        verify_hash(
            &packages_file,
            &item.packages_payload,
            &expected.packages_sha256,
        )?;
        verify_hash(
            &manifest_file,
            &item.manifest_payload,
            &expected.manifest_sha256,
        )?;
    }
    Ok(())
}

fn rootfs_binding_filenames(id: &str) -> Result<(String, String), String> {
    if !valid_module_id(id) {
        return Err(format!("invalid rootfs binding id: {id}"));
    }
    Ok((
        format!("rootfs.{id}.packages.tsv"),
        format!("rootfs.{id}.manifest.json"),
    ))
}

pub fn build_rootfs_binding_index(
    module: &str,
    version: &str,
    construction_payload: &[u8],
    rootfs: &[RootfsBindingInput],
) -> Result<RootfsBindingIndex, String> {
    validate_construction_rootfs_binding(module, version, construction_payload, rootfs)?;
    let mut entries = std::collections::BTreeMap::new();
    for item in rootfs {
        let (packages_file, manifest_file) = rootfs_binding_filenames(&item.id)?;
        entries.insert(
            item.id.clone(),
            RootfsBindingIndexEntry {
                packages_file,
                manifest_file,
                packages_sha256: sha256_bytes(&item.packages_payload),
                manifest_sha256: sha256_bytes(&item.manifest_payload),
            },
        );
    }
    Ok(RootfsBindingIndex {
        schema: 3,
        rootfs: entries,
    })
}

/// Write a closed schema-3 recipe inventory into a fresh directory using a
/// same-parent staging rename. This is deliberately separate from schema-2
/// active bindings until activation/rollback is upgraded in one transaction.
pub fn publish_rootfs_recipe_inventory(
    target: &Path,
    index: &RootfsBindingIndex,
    rootfs: &[RootfsBindingInput],
) -> Result<(), String> {
    verify_rootfs_binding_index(index, rootfs)?;
    let parent = target
        .parent()
        .ok_or_else(|| "rootfs inventory needs parent".to_string())?;
    if !parent.is_absolute() || !parent.is_dir() {
        return Err("rootfs inventory parent must be an existing absolute directory".to_string());
    }
    if fs::symlink_metadata(target).is_ok() {
        return Err(format!(
            "rootfs inventory already exists: {}",
            target.display()
        ));
    }
    let staging = tempfile::Builder::new()
        .prefix(".neebles-rootfs-recipes-")
        .tempdir_in(parent)
        .map_err(|error| format!("rootfs inventory staging failed: {error}"))?;
    let directory = staging.path().join("recipes");
    fs::create_dir(&directory)
        .map_err(|error| format!("rootfs inventory creation failed: {error}"))?;
    for item in rootfs {
        let (packages, manifest) = rootfs_binding_filenames(&item.id)?;
        write_binding_file(&directory.join(packages), &item.packages_payload)?;
        write_binding_file(&directory.join(manifest), &item.manifest_payload)?;
    }
    let payload = serde_json::to_vec_pretty(index)
        .map_err(|error| format!("rootfs inventory index encoding failed: {error}"))?;
    write_binding_file(&directory.join(BINDING_INDEX), &payload)?;
    // Fail before publication if a read-back finds unexpected entries/bytes.
    load_rootfs_recipe_inventory(&directory)?;
    if fs::symlink_metadata(target).is_ok() {
        return Err("rootfs inventory target appeared during staging".to_string());
    }
    fs::rename(&directory, target)
        .map_err(|error| format!("rootfs inventory publication failed: {error}"))?;
    Ok(())
}

/// Read schema-3 bytes only if the complete directory membership and all
/// payload hashes match. Never follows symlinks for recipe files.
pub fn load_rootfs_recipe_inventory(
    directory: &Path,
) -> Result<(RootfsBindingIndex, Vec<RootfsBindingInput>), String> {
    if !fs::symlink_metadata(directory)
        .map_err(|error| format!("rootfs inventory directory inspection failed: {error}"))?
        .file_type()
        .is_dir()
    {
        return Err("rootfs inventory is not a regular directory".to_string());
    }
    let index_payload = read_regular_file(&directory.join(BINDING_INDEX))?;
    let index: RootfsBindingIndex = serde_json::from_slice(&index_payload)
        .map_err(|error| format!("invalid rootfs inventory index: {error}"))?;
    if index.schema != 3 || index.rootfs.is_empty() {
        return Err("invalid rootfs inventory schema or empty membership".to_string());
    }
    let mut expected: BTreeSet<String> = [BINDING_INDEX.to_string()].into_iter().collect();
    let mut items = Vec::with_capacity(index.rootfs.len());
    for id in index.rootfs.keys() {
        let (packages, manifest) = rootfs_binding_filenames(id)?;
        expected.insert(packages.clone());
        expected.insert(manifest.clone());
        items.push(RootfsBindingInput {
            id: id.clone(),
            packages_payload: read_regular_file(&directory.join(packages))?,
            manifest_payload: read_regular_file(&directory.join(manifest))?,
        });
    }
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("rootfs inventory enumeration failed: {error}"))?
    {
        let entry = entry.map_err(|error| format!("rootfs inventory entry failed: {error}"))?;
        actual.insert(
            entry
                .file_name()
                .into_string()
                .map_err(|_| "rootfs inventory contains non-UTF8 filename".to_string())?,
        );
    }
    if actual != expected {
        return Err(format!(
            "rootfs inventory unexpected membership: expected={expected:?} actual={actual:?}"
        ));
    }
    verify_rootfs_binding_index(&index, &items)?;
    Ok((index, items))
}

/// Load an inventory only when its recipes match the expected authenticated
/// Construction membership, module identity and candidate version.
/// A valid checksum alone is not authority to bind recipes to a different module.
pub fn load_verified_rootfs_recipe_inventory(
    directory: &Path,
    module: &str,
    version: &str,
    construction_payload: &[u8],
) -> Result<(RootfsBindingIndex, Vec<RootfsBindingInput>), String> {
    let (index, items) = load_rootfs_recipe_inventory(directory)?;
    validate_construction_rootfs_binding(module, version, construction_payload, &items)?;
    Ok((index, items))
}

/// Couple the published inventory to its authenticated Construction declaration.
/// Publication remains independent from schema-2 active binding transactions.
pub fn publish_verified_rootfs_recipe_inventory(
    target: &Path,
    module: &str,
    version: &str,
    construction_payload: &[u8],
    rootfs: &[RootfsBindingInput],
) -> Result<(), String> {
    let index = build_rootfs_binding_index(module, version, construction_payload, rootfs)?;
    publish_rootfs_recipe_inventory(target, &index, rootfs)?;
    // Do not report publication success without verifying the expected owner.
    load_verified_rootfs_recipe_inventory(target, module, version, construction_payload)?;
    Ok(())
}

/// Full schema-3 active binding. Uses the existing PreparedMaterialBinding
/// activation/update/rollback mechanisms, without any reserved rootfs identity.
#[derive(Debug, Clone)]
pub struct MaterialBindingV3Input {
    pub module: String,
    pub version: String,
    pub custom_revision: String,
    pub essential_packages_payload: Vec<u8>,
    pub essential_manifest_payload: Vec<u8>,
    pub runtime_manifest_payload: Vec<u8>,
    pub construction_payload: Vec<u8>,
    pub rootfs: Vec<RootfsBindingInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveMaterialBindingV3Index {
    schema: u32,
    module: String,
    version: String,
    custom_revision: String,
    essentials_packages_sha256: String,
    essentials_manifest_sha256: String,
    runtime_manifest_sha256: String,
    construction_sha256: String,
    rootfs: RootfsBindingIndex,
}

#[derive(Debug, Clone)]
pub struct MaterialBindingV3 {
    pub module: String,
    pub version: String,
    pub custom_revision: String,
    pub essential_layer: MaterialLayer,
    pub rootfs_recipes: std::collections::BTreeMap<String, MaterialRecipe>,
    pub runtime_manifest_payload: Vec<u8>,
    pub construction_payload: Vec<u8>,
    pub path: PathBuf,
}

/// Prepare a complete schema-3 binding in the same transaction structure
/// already used by install/update. No activation is performed here.
pub fn prepare_material_binding_v3(
    material_root: &Path,
    input: MaterialBindingV3Input,
) -> Result<PreparedMaterialBinding, String> {
    ensure_material_root(material_root)?;
    if !valid_module_id(&input.module)
        || input.version.trim().is_empty()
        || !valid_revision(&input.custom_revision)
    {
        return Err("invalid active schema-3 binding identity/revision".to_string());
    }
    let essential_selector = std::str::from_utf8(&input.essential_packages_payload)
        .map_err(|error| format!("invalid Essential selector UTF-8: {error}"))?;
    parse_material_layer(
        "Essential",
        essential_selector,
        &input.essential_manifest_payload,
    )?;
    validate_runtime_manifest_payload(&input.runtime_manifest_payload)?;
    let rootfs_index = build_rootfs_binding_index(
        &input.module,
        &input.version,
        &input.construction_payload,
        &input.rootfs,
    )?;
    let index = ActiveMaterialBindingV3Index {
        schema: 3,
        module: input.module.clone(),
        version: input.version.clone(),
        custom_revision: input.custom_revision.to_ascii_lowercase(),
        essentials_packages_sha256: sha256_bytes(&input.essential_packages_payload),
        essentials_manifest_sha256: sha256_bytes(&input.essential_manifest_payload),
        runtime_manifest_sha256: sha256_bytes(&input.runtime_manifest_payload),
        construction_sha256: sha256_bytes(&input.construction_payload),
        rootfs: rootfs_index,
    };
    let temporary = tempfile::Builder::new()
        .prefix(".neebles-material-binding-")
        .tempdir_in(material_root)
        .map_err(|error| format!("schema-3 binding staging failed: {error}"))?;
    let staged_path = temporary.path().join("binding");
    fs::create_dir(&staged_path).map_err(|error| error.to_string())?;
    fs::set_permissions(&staged_path, fs::Permissions::from_mode(0o755))
        .map_err(|error| error.to_string())?;
    for (name, payload) in [
        (ESSENTIAL_PACKAGES, &input.essential_packages_payload),
        (ESSENTIAL_MANIFEST, &input.essential_manifest_payload),
        (RUNTIME_MANIFEST, &input.runtime_manifest_payload),
        (CONSTRUCTION, &input.construction_payload),
    ] {
        write_binding_file(&staged_path.join(name), payload)?;
    }
    for item in &input.rootfs {
        let (selector, manifest) = rootfs_binding_filenames(&item.id)?;
        write_binding_file(&staged_path.join(selector), &item.packages_payload)?;
        write_binding_file(&staged_path.join(manifest), &item.manifest_payload)?;
    }
    write_binding_file(
        &staged_path.join(BINDING_INDEX),
        &serde_json::to_vec_pretty(&index).map_err(|error| error.to_string())?,
    )?;
    // Authenticate the complete staged candidate before making activation possible.
    load_material_binding_v3_directory(&staged_path, &input.module)?;
    Ok(PreparedMaterialBinding {
        temporary,
        staged_path,
        active_path: material_root.join(&input.module),
    })
}

fn load_material_binding_v3_directory(
    path: &Path,
    module_id: &str,
) -> Result<MaterialBindingV3, String> {
    if !fs::symlink_metadata(path)
        .map_err(|error| error.to_string())?
        .file_type()
        .is_dir()
    {
        return Err("active schema-3 binding must be a directory".to_string());
    }
    let index: ActiveMaterialBindingV3Index =
        serde_json::from_slice(&read_regular_file(&path.join(BINDING_INDEX))?)
            .map_err(|error| format!("invalid active schema-3 binding index: {error}"))?;
    if index.schema != 3
        || index.module != module_id
        || !valid_module_id(module_id)
        || index.version.trim().is_empty()
        || !valid_revision(&index.custom_revision)
    {
        return Err("invalid active schema-3 binding owner/version/revision".to_string());
    }
    let mut expected: BTreeSet<String> = [
        BINDING_INDEX,
        ESSENTIAL_PACKAGES,
        ESSENTIAL_MANIFEST,
        RUNTIME_MANIFEST,
        CONSTRUCTION,
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    if index.rootfs.schema != 3 || index.rootfs.rootfs.is_empty() {
        return Err("invalid active schema-3 rootfs inventory".to_string());
    }
    let mut rootfs = Vec::new();
    for id in index.rootfs.rootfs.keys() {
        let (packages, manifest) = rootfs_binding_filenames(id)?;
        expected.insert(packages.clone());
        expected.insert(manifest.clone());
        rootfs.push(RootfsBindingInput {
            id: id.clone(),
            packages_payload: read_regular_file(&path.join(packages))?,
            manifest_payload: read_regular_file(&path.join(manifest))?,
        });
    }
    let actual = fs::read_dir(path)
        .map_err(|error| error.to_string())?
        .map(|item| {
            item.map_err(|error| error.to_string()).and_then(|entry| {
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "non-UTF8 binding filename".to_string())
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if actual != expected {
        return Err(format!("active schema-3 directory membership mismatch: expected={expected:?} actual={actual:?}"));
    }
    verify_rootfs_binding_index(&index.rootfs, &rootfs)?;
    let essentials_packages = read_regular_file(&path.join(ESSENTIAL_PACKAGES))?;
    let essentials_manifest = read_regular_file(&path.join(ESSENTIAL_MANIFEST))?;
    let runtime_manifest = read_regular_file(&path.join(RUNTIME_MANIFEST))?;
    let construction = read_regular_file(&path.join(CONSTRUCTION))?;
    verify_hash(
        ESSENTIAL_PACKAGES,
        &essentials_packages,
        &index.essentials_packages_sha256,
    )?;
    verify_hash(
        ESSENTIAL_MANIFEST,
        &essentials_manifest,
        &index.essentials_manifest_sha256,
    )?;
    verify_hash(
        RUNTIME_MANIFEST,
        &runtime_manifest,
        &index.runtime_manifest_sha256,
    )?;
    verify_hash(CONSTRUCTION, &construction, &index.construction_sha256)?;
    validate_runtime_manifest_payload(&runtime_manifest)?;
    let essential_layer = parse_material_layer(
        "Essential",
        std::str::from_utf8(&essentials_packages).map_err(|error| error.to_string())?,
        &essentials_manifest,
    )?;
    let rootfs_recipes =
        validate_construction_rootfs_binding(module_id, &index.version, &construction, &rootfs)?;
    Ok(MaterialBindingV3 {
        module: index.module,
        version: index.version,
        custom_revision: index.custom_revision,
        essential_layer,
        rootfs_recipes,
        runtime_manifest_payload: runtime_manifest,
        construction_payload: construction,
        path: path.to_path_buf(),
    })
}

/// Load an installed, authenticated schema-3 binding. Schema-2 bindings
/// continue to use load_material_binding without migration or reinterpretation.
pub fn load_material_binding_v3(
    material_root: &Path,
    module_id: &str,
) -> Result<MaterialBindingV3, String> {
    ensure_material_root(material_root)?;
    if !valid_module_id(module_id) {
        return Err("invalid module identity for schema-3 binding".to_string());
    }
    load_material_binding_v3_directory(&material_root.join(module_id), module_id)
}

/// Installed material binding, dispatched by the authenticated on-disk schema.
/// No inferred rootfs identity and no reinterpretation between schema versions.
#[derive(Debug, Clone)]
pub enum InstalledMaterialBinding {
    Legacy(MaterialBinding),
    MultiRootfs(MaterialBindingV3),
}

impl InstalledMaterialBinding {
    pub fn module(&self) -> &str {
        match self {
            Self::Legacy(binding) => &binding.module,
            Self::MultiRootfs(binding) => &binding.module,
        }
    }

    pub fn version(&self) -> &str {
        match self {
            Self::Legacy(binding) => &binding.version,
            Self::MultiRootfs(binding) => &binding.version,
        }
    }

    pub fn custom_revision(&self) -> &str {
        match self {
            Self::Legacy(binding) => &binding.custom_revision,
            Self::MultiRootfs(binding) => &binding.custom_revision,
        }
    }

    pub fn runtime_manifest_payload(&self) -> &[u8] {
        match self {
            Self::Legacy(binding) => &binding.runtime_manifest_payload,
            Self::MultiRootfs(binding) => &binding.runtime_manifest_payload,
        }
    }

    pub fn construction_payload(&self) -> &[u8] {
        match self {
            Self::Legacy(binding) => &binding.construction_payload,
            Self::MultiRootfs(binding) => &binding.construction_payload,
        }
    }
}

/// Inspect only the on-disk schema number and delegate all authority checks to
/// the corresponding complete, strict reader. Never fall back after failure.
pub fn load_installed_material_binding(
    material_root: &Path,
    module_id: &str,
) -> Result<InstalledMaterialBinding, String> {
    ensure_material_root(material_root)?;
    if !valid_module_id(module_id) {
        return Err(format!(
            "invalid installed binding module identity: {module_id}"
        ));
    }
    let path = material_root.join(module_id);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("cannot inspect installed binding: {error}"))?;
    if !metadata.file_type().is_dir() {
        return Err("installed binding must be a real directory".to_string());
    }
    let payload = read_regular_file(&path.join(BINDING_INDEX))?;
    #[derive(Deserialize)]
    struct SchemaProbe {
        schema: u32,
    }
    let probe: SchemaProbe = serde_json::from_slice(&payload)
        .map_err(|error| format!("invalid installed binding schema index: {error}"))?;
    match probe.schema {
        2 => load_material_binding(material_root, module_id).map(InstalledMaterialBinding::Legacy),
        3 => load_material_binding_v3(material_root, module_id)
            .map(InstalledMaterialBinding::MultiRootfs),
        other => Err(format!(
            "unsupported installed material binding schema: {other}"
        )),
    }
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

#[cfg(test)]
mod multirootfs_binding_contract_tests {
    use super::*;

    fn item(id: &str) -> RootfsBindingInput {
        RootfsBindingInput {
            id: id.to_string(),
            packages_payload: Vec::new(),
            manifest_payload:
                serde_json::json!({"module":"connect", "version":"1.0", "rootfs":id, "entries":[]})
                    .to_string()
                    .into_bytes(),
        }
    }

    #[test]
    fn binds_multiple_arbitrary_rootfs_without_technology_branches() {
        let parsed =
            validate_rootfs_binding_recipes("connect", "1.0", &[item("motor_a"), item("motor_b")])
                .unwrap();
        assert!(parsed.contains_key("motor_a"));
        assert!(parsed.contains_key("motor_b"));
    }

    #[test]
    fn duplicate_and_unsafe_rootfs_are_rejected() {
        assert!(
            validate_rootfs_binding_recipes("connect", "1.0", &[item("x"), item("x")]).is_err()
        );
        assert!(validate_rootfs_binding_recipes("connect", "1.0", &[item("../escape")]).is_err());
    }

    #[test]
    fn mismatched_recipe_identity_and_empty_set_fail_closed() {
        assert!(validate_rootfs_binding_recipes("connect", "1.0", &[]).is_err());
        assert!(validate_rootfs_binding_recipes("connect", "2.0", &[item("x")]).is_err());
    }
}

#[cfg(test)]
mod multirootfs_construction_binding_tests {
    use super::*;

    fn construction(ids: &[&str]) -> Vec<u8> {
        let declared: Vec<_> = ids.iter().map(|id| serde_json::json!({"id": id})).collect();
        serde_json::json!({
            "schema": "1", "name": "neebles-domestic-construction", "subject": "connect",
            "rootfs": declared,
            "steps": [{"id":"open", "runtime_authority":"modules.runtime",
                "rootfs": if ids.is_empty() { "unbound" } else { ids[0] },
                "world":"modules.connect", "execution":"persistent", "session":false}]
        })
        .to_string()
        .into_bytes()
    }

    fn item(id: &str) -> RootfsBindingInput {
        RootfsBindingInput {
            id: id.into(),
            packages_payload: Vec::new(),
            manifest_payload: serde_json::json!({
                "module":"connect", "version":"1.0", "rootfs":id, "entries":[]
            })
            .to_string()
            .into_bytes(),
        }
    }

    #[test]
    fn declared_and_supplied_arbitrary_rootfs_match_exactly() {
        let recipes = validate_construction_rootfs_binding(
            "connect",
            "1.0",
            &construction(&["motor_a", "motor_b"]),
            &[item("motor_b"), item("motor_a")],
        )
        .unwrap();
        assert_eq!(recipes.len(), 2);
    }

    #[test]
    fn missing_or_extra_recipe_is_rejected() {
        assert!(validate_construction_rootfs_binding(
            "connect",
            "1.0",
            &construction(&["motor_a", "motor_b"]),
            &[item("motor_a")]
        )
        .is_err());
        assert!(validate_construction_rootfs_binding(
            "connect",
            "1.0",
            &construction(&["motor_a"]),
            &[item("motor_a"), item("motor_b")]
        )
        .is_err());
    }

    #[test]
    fn no_implicit_rootfs_is_ever_supplied() {
        assert!(validate_construction_rootfs_binding(
            "connect",
            "1.0",
            &construction(&[]),
            &[item("principal")]
        )
        .is_err());
        assert!(validate_construction_rootfs_binding(
            "connect",
            "1.0",
            &construction(&["caca-de-perro"]),
            &[item("caca-de-perro")]
        )
        .is_ok());
    }

    #[test]
    fn different_module_identity_is_rejected() {
        assert!(validate_construction_rootfs_binding(
            "another",
            "1.0",
            &construction(&["motor_a"]),
            &[item("motor_a")]
        )
        .is_err());
    }
}

#[cfg(test)]
mod multirootfs_index_contract_tests {
    use super::*;

    fn item(id: &str) -> RootfsBindingInput {
        RootfsBindingInput {
            id: id.to_string(),
            packages_payload: format!("{id}-packages").into_bytes(),
            manifest_payload: format!("{id}-manifest").into_bytes(),
        }
    }

    #[test]
    fn schema_three_accepts_arbitrary_names_without_reserved_rootfs() {
        let rootfs = vec![item("caca-de-perro"), item("caca-de-gato")];
        let entries = rootfs
            .iter()
            .map(|r| {
                let (packages_file, manifest_file) = rootfs_binding_filenames(&r.id).unwrap();
                (
                    r.id.clone(),
                    RootfsBindingIndexEntry {
                        packages_file,
                        manifest_file,
                        packages_sha256: sha256_bytes(&r.packages_payload),
                        manifest_sha256: sha256_bytes(&r.manifest_payload),
                    },
                )
            })
            .collect();
        let index = RootfsBindingIndex {
            schema: 3,
            rootfs: entries,
        };
        assert!(verify_rootfs_binding_index(&index, &rootfs).is_ok());
        assert!(verify_rootfs_binding_index(&index, &[rootfs[0].clone()]).is_err());
    }

    #[test]
    fn schema_three_rejects_changed_hash_and_rewritten_path() {
        let rootfs = vec![item("motor-x")];
        let (packages_file, manifest_file) = rootfs_binding_filenames("motor-x").unwrap();
        let mut index = RootfsBindingIndex {
            schema: 3,
            rootfs: [(
                "motor-x".to_string(),
                RootfsBindingIndexEntry {
                    packages_file,
                    manifest_file,
                    packages_sha256: sha256_bytes(&rootfs[0].packages_payload),
                    manifest_sha256: sha256_bytes(&rootfs[0].manifest_payload),
                },
            )]
            .into_iter()
            .collect(),
        };
        assert!(verify_rootfs_binding_index(&index, &rootfs).is_ok());
        let mut changed = rootfs.clone();
        changed[0].manifest_payload.push(42);
        assert!(verify_rootfs_binding_index(&index, &changed).is_err());
        index.rootfs.get_mut("motor-x").unwrap().packages_file = "other.tsv".to_string();
        assert!(verify_rootfs_binding_index(&index, &rootfs).is_err());
    }
}

#[cfg(test)]
mod multirootfs_disk_inventory_tests {
    use super::*;

    fn item(id: &str) -> RootfsBindingInput {
        RootfsBindingInput {
            id: id.to_string(),
            packages_payload: format!("{id}\n").into_bytes(),
            manifest_payload: format!("{{\"id\":\"{id}\"}}").into_bytes(),
        }
    }

    fn index(items: &[RootfsBindingInput]) -> RootfsBindingIndex {
        let rootfs = items
            .iter()
            .map(|item| {
                let (packages_file, manifest_file) = rootfs_binding_filenames(&item.id).unwrap();
                (
                    item.id.clone(),
                    RootfsBindingIndexEntry {
                        packages_file,
                        manifest_file,
                        packages_sha256: sha256_bytes(&item.packages_payload),
                        manifest_sha256: sha256_bytes(&item.manifest_payload),
                    },
                )
            })
            .collect();
        RootfsBindingIndex { schema: 3, rootfs }
    }

    #[test]
    fn multiple_opaque_ids_survive_disk_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("inventory");
        let items = vec![item("caca-de-gato"), item("otro-motor")];
        publish_rootfs_recipe_inventory(&target, &index(&items), &items).unwrap();
        let (_, loaded) = load_rootfs_recipe_inventory(&target).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded.iter().any(|i| i.id == "otro-motor"));
        assert!(publish_rootfs_recipe_inventory(&target, &index(&items), &items).is_err());
    }

    #[test]
    fn tampered_and_untracked_files_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("inventory");
        let items = vec![item("libre")];
        publish_rootfs_recipe_inventory(&target, &index(&items), &items).unwrap();
        fs::write(target.join("rootfs.libre.packages.tsv"), b"changed").unwrap();
        assert!(load_rootfs_recipe_inventory(&target).is_err());
        fs::write(
            target.join("rootfs.libre.packages.tsv"),
            &items[0].packages_payload,
        )
        .unwrap();
        fs::write(target.join("extra"), b"x").unwrap();
        assert!(load_rootfs_recipe_inventory(&target).is_err());
    }
}

#[cfg(test)]
mod multirootfs_owner_verified_inventory_tests {
    use super::*;

    fn construction() -> Vec<u8> {
        serde_json::json!({
            "schema":"1", "name":"neebles-domestic-construction", "subject":"connect",
            "rootfs":[{"id":"gato"},{"id":"perro"}],
            "steps":[{"id":"open","runtime_authority":"modules.runtime",
                "rootfs":"gato", "world":"modules.connect",
                "execution":"persistent", "session":false}]
        })
        .to_string()
        .into_bytes()
    }

    fn items() -> Vec<RootfsBindingInput> {
        ["gato", "perro"]
            .into_iter()
            .map(|id| RootfsBindingInput {
                id: id.to_string(),
                packages_payload: Vec::new(),
                manifest_payload: serde_json::json!({
                    "module":"connect", "version":"1.0", "rootfs":id, "entries":[]
                })
                .to_string()
                .into_bytes(),
            })
            .collect()
    }

    #[test]
    fn matching_owner_and_declaration_survive_disk_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("inventory");
        publish_verified_rootfs_recipe_inventory(
            &target,
            "connect",
            "1.0",
            &construction(),
            &items(),
        )
        .unwrap();
        assert_eq!(
            load_verified_rootfs_recipe_inventory(&target, "connect", "1.0", &construction())
                .unwrap()
                .1
                .len(),
            2
        );
        assert!(
            load_verified_rootfs_recipe_inventory(&target, "otro", "1.0", &construction()).is_err()
        );
        assert!(
            load_verified_rootfs_recipe_inventory(&target, "connect", "2.0", &construction())
                .is_err()
        );
    }

    #[test]
    fn missing_declared_recipe_does_not_publish() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("inventory");
        assert!(publish_verified_rootfs_recipe_inventory(
            &target,
            "connect",
            "1.0",
            &construction(),
            &items()[..1]
        )
        .is_err());
        assert!(!target.exists());
    }
}

#[cfg(test)]
mod active_schema_three_binding_tests {
    use super::*;

    fn fixture() -> MaterialBindingV3Input {
        let rootfs = ["gato", "perro"]
            .iter()
            .map(|id| RootfsBindingInput {
                id: id.to_string(),
                packages_payload: vec![],
                manifest_payload: serde_json::json!({
                    "module":"connect", "version":"1.0", "rootfs":id, "entries":[]
                })
                .to_string()
                .into_bytes(),
            })
            .collect();
        MaterialBindingV3Input {
            module: "connect".into(),
            version: "1.0".into(),
            custom_revision: "a".repeat(40),
            essential_packages_payload: vec![],
            essential_manifest_payload: br#"{"entries":[]}"#.to_vec(),
            runtime_manifest_payload: br#"{"worlds":{}}"#.to_vec(),
            construction_payload: serde_json::json!({
                "schema":"1", "name":"neebles-domestic-construction",
                "subject":"connect", "rootfs":[{"id":"gato"},{"id":"perro"}],
                "steps":[{"id":"open", "runtime_authority":"modules.runtime",
                    "rootfs":"gato", "world":"modules.connect",
                    "execution":"persistent", "session":false}]
            })
            .to_string()
            .into_bytes(),
            rootfs,
        }
    }

    #[test]
    fn schema_three_activates_with_generic_transaction_and_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("material");
        let prepared = prepare_material_binding_v3(&root, fixture()).unwrap();
        assert!(!root.join("connect").exists());
        prepared.activate_install().unwrap();
        let loaded = load_material_binding_v3(&root, "connect").unwrap();
        assert_eq!(loaded.rootfs_recipes.len(), 2);
        assert!(loaded.rootfs_recipes.contains_key("gato"));
        assert!(loaded.rootfs_recipes.contains_key("perro"));
        assert!(load_material_binding(&root, "connect").is_err());
    }

    #[test]
    fn schema_three_update_rollback_restores_active_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("material");
        prepare_material_binding_v3(&root, fixture())
            .unwrap()
            .activate_install()
            .unwrap();
        let original = fs::read(root.join("connect").join(BINDING_INDEX)).unwrap();
        let mut changed = fixture();
        changed.custom_revision = "b".repeat(40);
        let updated = prepare_material_binding_v3(&root, changed).unwrap();
        let activated = updated.activate_update().unwrap();
        assert_eq!(
            load_material_binding_v3(&root, "connect")
                .unwrap()
                .custom_revision,
            "b".repeat(40)
        );
        activated.rollback().unwrap();
        assert_eq!(
            fs::read(root.join("connect").join(BINDING_INDEX)).unwrap(),
            original
        );
    }

    #[test]
    fn schema_three_update_changed_rootfs_membership_rollback_restores_original_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("material");
        prepare_material_binding_v3(&root, fixture())
            .unwrap()
            .activate_install()
            .unwrap();
        let original = load_material_binding_v3(&root, "connect").unwrap();
        let original_index = fs::read(root.join("connect").join(BINDING_INDEX)).unwrap();
        let original_manifest =
            fs::read(root.join("connect").join("rootfs.perro.manifest.json")).unwrap();

        let mut next = fixture();
        next.version = "2.0".into();
        next.custom_revision = "b".repeat(40);
        next.rootfs.push(RootfsBindingInput {
            id: "pato".into(),
            packages_payload: vec![],
            manifest_payload:
                br#"{"module":"connect","version":"2.0","rootfs":"pato","entries":[]}"#.to_vec(),
        });
        for recipe in &mut next.rootfs {
            recipe.manifest_payload = serde_json::json!({
                "module":"connect", "version":"2.0", "rootfs":recipe.id, "entries":[]
            })
            .to_string()
            .into_bytes();
        }
        next.construction_payload = serde_json::json!({
            "schema":"1", "name":"neebles-domestic-construction", "subject":"connect",
            "rootfs":[{"id":"gato"},{"id":"perro"},{"id":"pato"}],
            "steps":[{"id":"open", "runtime_authority":"modules.runtime", "rootfs":"pato",
                "world":"modules.connect", "execution":"persistent", "session":false}]
        })
        .to_string()
        .into_bytes();

        let activated = prepare_material_binding_v3(&root, next)
            .unwrap()
            .activate_update()
            .unwrap();
        let active = load_installed_material_binding(&root, "connect").unwrap();
        assert_eq!(active.version(), "2.0");
        assert_eq!(
            load_material_binding_v3(&root, "connect")
                .unwrap()
                .rootfs_recipes
                .len(),
            3
        );
        assert!(root
            .join("connect")
            .join("rootfs.pato.manifest.json")
            .exists());

        activated.rollback().unwrap();
        let restored = load_material_binding_v3(&root, "connect").unwrap();
        assert_eq!(restored.version, original.version);
        assert_eq!(
            restored.rootfs_recipes.keys().collect::<Vec<_>>(),
            original.rootfs_recipes.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            fs::read(root.join("connect").join(BINDING_INDEX)).unwrap(),
            original_index
        );
        assert_eq!(
            fs::read(root.join("connect").join("rootfs.perro.manifest.json")).unwrap(),
            original_manifest
        );
        assert!(!root
            .join("connect")
            .join("rootfs.pato.manifest.json")
            .exists());
    }

    #[test]
    fn schema_three_update_changed_rootfs_membership_finalize_keeps_only_new_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("material");
        prepare_material_binding_v3(&root, fixture())
            .unwrap()
            .activate_install()
            .unwrap();

        let mut next = fixture();
        next.version = "2.0".into();
        next.custom_revision = "c".repeat(40);
        next.rootfs.retain(|recipe| recipe.id == "gato");
        next.rootfs[0].manifest_payload =
            br#"{"module":"connect","version":"2.0","rootfs":"gato","entries":[]}"#.to_vec();
        next.construction_payload = serde_json::json!({
            "schema":"1", "name":"neebles-domestic-construction", "subject":"connect",
            "rootfs":[{"id":"gato"}],
            "steps":[{"id":"open", "runtime_authority":"modules.runtime", "rootfs":"gato",
                "world":"modules.connect", "execution":"persistent", "session":false}]
        })
        .to_string()
        .into_bytes();

        prepare_material_binding_v3(&root, next)
            .unwrap()
            .activate_update()
            .unwrap()
            .finalize();
        let installed = load_installed_material_binding(&root, "connect").unwrap();
        assert_eq!(installed.version(), "2.0");
        let binding = load_material_binding_v3(&root, "connect").unwrap();
        assert_eq!(binding.rootfs_recipes.len(), 1);
        assert!(binding.rootfs_recipes.contains_key("gato"));
        assert!(!root
            .join("connect")
            .join("rootfs.perro.manifest.json")
            .exists());
    }

    #[test]
    fn schema_three_uninstall_staging_restores_entire_authenticated_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("material");
        prepare_material_binding_v3(&root, fixture())
            .unwrap()
            .activate_install()
            .unwrap();
        let active = root.join("connect");
        let original: std::collections::BTreeMap<_, _> = fs::read_dir(&active)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name(), fs::read(entry.path()).unwrap())
            })
            .collect();
        let staged = stage_material_binding_removal(&root, "connect")
            .unwrap()
            .unwrap();
        assert!(!active.exists());
        assert!(load_installed_material_binding(&root, "connect").is_err());
        staged.restore().unwrap();
        let restored: std::collections::BTreeMap<_, _> = fs::read_dir(&active)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (entry.file_name(), fs::read(entry.path()).unwrap())
            })
            .collect();
        assert_eq!(restored, original);
        assert_eq!(
            load_material_binding_v3(&root, "connect")
                .unwrap()
                .rootfs_recipes
                .len(),
            2
        );
    }

    #[test]
    fn schema_three_uninstall_finalization_removes_every_rootfs_binding_entry() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("material");
        prepare_material_binding_v3(&root, fixture())
            .unwrap()
            .activate_install()
            .unwrap();
        let staged = stage_material_binding_removal(&root, "connect")
            .unwrap()
            .unwrap();
        let staged_path = staged.staged_path().to_path_buf();
        assert!(staged_path.join("rootfs.gato.manifest.json").exists());
        assert!(staged_path.join("rootfs.perro.manifest.json").exists());
        staged.finalize();
        assert!(!root.join("connect").exists());
        assert!(!staged_path.exists());
        assert!(stage_material_binding_removal(&root, "connect")
            .unwrap()
            .is_none());
    }

    #[test]
    fn schema_three_uninstall_restore_refuses_active_collision_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("material");
        prepare_material_binding_v3(&root, fixture())
            .unwrap()
            .activate_install()
            .unwrap();
        let staged = stage_material_binding_removal(&root, "connect")
            .unwrap()
            .unwrap();
        let active = root.join("connect");
        fs::create_dir(&active).unwrap();
        fs::write(active.join("foreign"), b"unchanged").unwrap();
        assert!(staged.restore().is_err());
        assert_eq!(fs::read(active.join("foreign")).unwrap(), b"unchanged");
        assert!(load_installed_material_binding(&root, "connect").is_err());
    }

    #[test]
    fn installed_reader_dispatches_schema_three_with_opaque_identities() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("material");
        prepare_material_binding_v3(&root, fixture())
            .unwrap()
            .activate_install()
            .unwrap();
        let result = load_installed_material_binding(&root, "connect").unwrap();
        assert_eq!(result.module(), "connect");
        assert_eq!(result.version(), "1.0");
        assert_eq!(result.custom_revision(), "a".repeat(40));
        assert!(matches!(result, InstalledMaterialBinding::MultiRootfs(_)));
    }

    #[test]
    fn installed_reader_does_not_fallback_from_invalid_schema_three() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("material");
        prepare_material_binding_v3(&root, fixture())
            .unwrap()
            .activate_install()
            .unwrap();
        let index = root.join("connect").join(BINDING_INDEX);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&index).unwrap()).unwrap();
        value["schema"] = serde_json::json!(99);
        fs::write(&index, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(load_installed_material_binding(&root, "connect").is_err());
        value["schema"] = serde_json::json!(3);
        fs::write(&index, serde_json::to_vec(&value).unwrap()).unwrap();
        fs::write(
            root.join("connect").join("rootfs.gato.manifest.json"),
            b"broken",
        )
        .unwrap();
        assert!(load_installed_material_binding(&root, "connect").is_err());
    }

    #[test]
    fn schema_three_rejects_tampering_and_untracked_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("material");
        prepare_material_binding_v3(&root, fixture())
            .unwrap()
            .activate_install()
            .unwrap();
        let path = root.join("connect");
        fs::write(path.join("rogue"), b"no").unwrap();
        assert!(load_material_binding_v3(&root, "connect").is_err());
        fs::remove_file(path.join("rogue")).unwrap();
        fs::write(path.join("rootfs.perro.manifest.json"), b"{}").unwrap();
        assert!(load_material_binding_v3(&root, "connect").is_err());
    }
}

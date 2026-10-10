use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use url::Url;

use neebles_backend::domestic_construction::DomesticConstructionDeclaration;
use neebles_backend::module_material::{
    parse_material_layer, parse_material_recipe, valid_module_id, PackageRequirement,
};
use neebles_backend::module_material_binding::{
    MaterialBindingInput, MaterialBindingV3Input, PreparedMaterialBinding, RootfsBindingInput,
};

const CUSTOM_REPOSITORY: &str = "krockzs/neebles-custom";
const CUSTOM_REPOSITORY_GIT: &str = "https://github.com/krockzs/neebles-custom.git";
const CUSTOM_BRANCH_CANDIDATES: [&str; 2] = ["main", "master"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulePreinstallReport {
    pub module: String,
    pub custom_revision: String,
    pub required: usize,
    pub reused: usize,
    pub downloaded: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedModulePackages {
    pub(crate) report: ModulePreinstallReport,
    pub(crate) binding_input: MaterialBindingInput,
    pub(crate) material_root: PathBuf,
    // Explicit binding format. A future multi-recipe downloader supplies V3;
    // an old V2 recipe is never silently interpreted as N independent recipes.
    pub(crate) binding_format: CandidateBindingFormat,
}

#[derive(Debug, Clone)]
pub(crate) enum CandidateBindingFormat {
    LegacyV2,
    MultiRootfsV3(Vec<RootfsBindingInput>),
}

impl PreparedModulePackages {
    pub(crate) fn prepare_binding(&self) -> Result<PreparedMaterialBinding, String> {
        match &self.binding_format {
            CandidateBindingFormat::LegacyV2 => {
                neebles_backend::module_material_binding::prepare_material_binding(
                    &self.material_root,
                    self.binding_input.clone(),
                )
            }
            CandidateBindingFormat::MultiRootfsV3(rootfs) => {
                let old = &self.binding_input;
                neebles_backend::module_material_binding::prepare_material_binding_v3(
                    &self.material_root,
                    MaterialBindingV3Input {
                        module: old.module.clone(),
                        version: old.version.clone(),
                        custom_revision: old.custom_revision.clone(),
                        essential_packages_payload: old.essential_packages_payload.clone(),
                        essential_manifest_payload: old.essential_manifest_payload.clone(),
                        runtime_manifest_payload: old.runtime_manifest_payload.clone(),
                        construction_payload: old.construction_payload.clone(),
                        rootfs: rootfs.clone(),
                    },
                )
            }
        }
    }
}

fn resolve_custom_revision() -> Result<String, String> {
    let mut failures = Vec::new();

    for branch in CUSTOM_BRANCH_CANDIDATES {
        let reference = format!("refs/heads/{branch}");

        let arguments = [
            OsString::from("ls-remote"),
            OsString::from(CUSTOM_REPOSITORY_GIT),
            OsString::from(&reference),
        ];

        let output = match crate::network_boundary::command("boss.git", &arguments) {
            Ok(mut command) => match command.output() {
                Ok(output) => output,
                Err(error) => {
                    failures.push(format!("{branch}: {error}"));
                    continue;
                }
            },
            Err(error) => {
                failures.push(format!("{branch}: {error}"));
                continue;
            }
        };

        if !output.status.success() {
            failures.push(format!(
                "{branch}: git ls-remote returned {}",
                output.status
            ));
            continue;
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|error| format!("invalid CUSTOM git revision output: {error}"))?;

        let Some(revision) = stdout.split_whitespace().next() else {
            failures.push(format!("{branch}: no revision returned"));
            continue;
        };

        if revision.len() != 40 || !revision.bytes().all(|value| value.is_ascii_hexdigit()) {
            failures.push(format!("{branch}: invalid revision returned: {revision}"));
            continue;
        }

        return Ok(revision.to_ascii_lowercase());
    }

    Err(format!(
        "could not resolve canonical N.E.E.B.L.E.S. CUSTOM revision: {}",
        failures.join(" | ")
    ))
}

fn custom_raw_url(revision: &str, path: &str) -> Result<String, String> {
    if revision.len() != 40 || !revision.bytes().all(|value| value.is_ascii_hexdigit()) {
        return Err(format!("invalid CUSTOM raw revision: {revision}"));
    }

    if path.is_empty() || path.starts_with('/') || path.ends_with('/') {
        return Err(format!("invalid CUSTOM raw path: {path}"));
    }

    for segment in path.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(format!("invalid CUSTOM raw path segment in: {path}"));
        }

        if segment.chars().any(char::is_control) {
            return Err(format!(
                "CUSTOM raw path contains control characters: {path:?}"
            ));
        }
    }

    let mut url = Url::parse("https://raw.githubusercontent.com/")
        .map_err(|error| format!("could not construct CUSTOM raw URL base: {error}"))?;

    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "CUSTOM raw URL base cannot accept path segments".to_string())?;

        segments.pop_if_empty();

        for segment in CUSTOM_REPOSITORY.split('/') {
            segments.push(segment);
        }

        segments.push(revision);

        for segment in path.split('/') {
            segments.push(segment);
        }
    }

    Ok(url.to_string())
}

fn fetch_remote_bytes(revision: &str, path: &str, timeout_seconds: u32) -> Result<Vec<u8>, String> {
    let url = custom_raw_url(revision, path)?;

    let arguments = [
        OsString::from("-fsSL"),
        OsString::from("--max-time"),
        OsString::from(timeout_seconds.to_string()),
        OsString::from(&url),
    ];

    let output = crate::network_boundary::command("boss.curl", &arguments)?
        .output()
        .map_err(|error| format!("could not fetch module material metadata: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "module material metadata download failed for {path}: {}",
            output.status
        ));
    }

    Ok(output.stdout)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("could not open {}: {error}", path.display()))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];

    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("could not hash {}: {error}", path.display()))?;

        if count == 0 {
            break;
        }

        hasher.update(&buffer[..count]);
    }

    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn verify_package(path: &Path, expected_sha256: &str) -> Result<bool, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(false);
        }
        Err(error) => {
            return Err(format!(
                "could not inspect module package {}: {error}",
                path.display()
            ));
        }
    };

    if !metadata.file_type().is_file() {
        return Err(format!(
            "module package path exists but is not a regular file: {}",
            path.display()
        ));
    }

    let actual = sha256_file(path)?;

    if actual != expected_sha256 {
        return Err(format!(
            "module package sha256 mismatch: {}",
            path.display()
        ));
    }

    Ok(true)
}

fn download_package(
    revision: &str,
    pool: &Path,
    remote_pool: &str,
    requirement: &PackageRequirement,
) -> Result<bool, String> {
    let target = pool.join(&requirement.filename);

    if verify_package(&target, &requirement.sha256)? {
        return Ok(false);
    }

    let temporary = tempfile::NamedTempFile::new_in(pool).map_err(|error| {
        format!(
            "could not create module package staging file in {}: {error}",
            pool.display()
        )
    })?;

    let output = temporary
        .reopen()
        .map_err(|error| format!("could not reopen module package staging file: {error}"))?;

    let remote_path = format!("{remote_pool}/{}", requirement.filename);

    let url = custom_raw_url(revision, &remote_path)?;

    let arguments = [
        OsString::from("-fsSL"),
        OsString::from("--max-time"),
        OsString::from("300"),
        OsString::from(&url),
    ];

    let status = crate::network_boundary::command("boss.curl", &arguments)?
        .stdout(Stdio::from(output))
        .status()
        .map_err(|error| {
            format!(
                "could not download module package '{}': {error}",
                requirement.filename
            )
        })?;

    if !status.success() {
        return Err(format!(
            "module package '{}' download failed with status {}",
            requirement.filename, status
        ));
    }

    if !verify_package(temporary.path(), &requirement.sha256)? {
        return Err(format!(
            "downloaded module package disappeared before verification: {}",
            requirement.filename
        ));
    }

    match temporary.persist_noclobber(&target) {
        Ok(_) => Ok(true),

        Err(error) => {
            let persist_error = error.error.to_string();
            drop(error.file);

            if verify_package(&target, &requirement.sha256)? {
                return Ok(false);
            }

            Err(format!(
                "could not publish module package '{}': {}",
                requirement.filename, persist_error
            ))
        }
    }
}

fn ensure_package_requirements(
    revision: &str,
    pool: &Path,
    remote_pool: &str,
    requirements: &[PackageRequirement],
) -> Result<(usize, usize), String> {
    let mut reused = 0usize;
    let mut downloaded = 0usize;

    for requirement in requirements {
        let target = pool.join(&requirement.filename);

        if verify_package(&target, &requirement.sha256)? {
            reused += 1;
            continue;
        }

        if download_package(revision, pool, remote_pool, requirement)? {
            downloaded += 1;
        } else {
            reused += 1;
        }
    }

    Ok((reused, downloaded))
}

/// Fail closed while a declaration cannot yet be mapped to independently
/// authenticated CUSTOM V2 package recipes. Every identity is module-declared.
fn validate_preinstall_rootfs_recipe_contract(
    declaration: &DomesticConstructionDeclaration,
) -> Result<(), String> {
    match declaration.rootfs.as_slice() {
        [ _only ] => Ok(()),
        others => Err(format!(
            "preinstall cannot authenticate independent rootfs recipes yet: subject={} declared={}; publish multi-rootfs material bindings before enabling this declaration",
            declaration.subject,
            others.iter().map(|rootfs| rootfs.id.as_str()).collect::<Vec<_>>().join(",")
        )),
    }
}

/// CUSTOM recipe layout is structural; only module and rootfs names come from
/// the authenticated Construction declaration. No rootfs name is reserved.
fn rootfs_recipe_paths(module: &str, rootfs: &str) -> Result<(String, String), String> {
    if !valid_module_id(module) || !valid_module_id(rootfs) {
        return Err("invalid module/rootfs recipe path identity".to_string());
    }
    let stem = format!("runtime/manifests/modules/rootfs/{module}/{rootfs}");
    Ok((
        format!("{stem}.packages.tsv"),
        format!("{stem}.manifest.json"),
    ))
}

/// Read the complete recipe set at one pinned CUSTOM revision and authenticate
/// every identity before any package download or publication.
fn fetch_rootfs_recipes(
    revision: &str,
    module: &str,
    version: &str,
    construction_payload: &[u8],
) -> Result<
    (
        Vec<RootfsBindingInput>,
        BTreeMap<String, neebles_backend::module_material::MaterialRecipe>,
    ),
    String,
> {
    let construction_text = std::str::from_utf8(construction_payload)
        .map_err(|error| format!("Construction is not UTF-8: {error}"))?;
    let declaration = DomesticConstructionDeclaration::parse(construction_text)?;
    if declaration.subject != module || declaration.rootfs.is_empty() {
        return Err("Construction does not declare rootfs for this module".to_string());
    }
    let mut rootfs = Vec::with_capacity(declaration.rootfs.len());
    for entry in &declaration.rootfs {
        let (packages_path, manifest_path) = rootfs_recipe_paths(module, &entry.id)?;
        rootfs.push(RootfsBindingInput {
            id: entry.id.clone(),
            packages_payload: fetch_remote_bytes(revision, &packages_path, 30)?,
            manifest_payload: fetch_remote_bytes(revision, &manifest_path, 30)?,
        });
    }
    let recipes = neebles_backend::module_material_binding::validate_construction_rootfs_binding(
        module,
        version,
        construction_payload,
        &rootfs,
    )?;
    Ok((rootfs, recipes))
}

/// Shared pool filenames must have one SHA authority across Essentials and
/// every declared rootfs, including recipes not yet materialized.
fn unique_rootfs_packages(
    essentials: &[PackageRequirement],
    recipes: &BTreeMap<String, neebles_backend::module_material::MaterialRecipe>,
) -> Result<Vec<PackageRequirement>, String> {
    let essential_names: std::collections::BTreeSet<&str> =
        essentials.iter().map(|p| p.filename.as_str()).collect();
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for recipe in recipes.values() {
        for requirement in &recipe.packages {
            if essential_names.contains(requirement.filename.as_str()) {
                return Err(format!(
                    "rootfs package repeats Essential: {}",
                    requirement.filename
                ));
            }
            if let Some(previous) =
                names.insert(requirement.filename.clone(), requirement.sha256.clone())
            {
                if previous != requirement.sha256 {
                    return Err(format!(
                        "conflicting rootfs package SHA256: {}",
                        requirement.filename
                    ));
                }
            }
        }
    }
    Ok(names
        .into_iter()
        .map(|(filename, sha256)| PackageRequirement { filename, sha256 })
        .collect())
}

pub(crate) fn ensure_module_packages_for_candidate(
    module_id: &str,
    candidate_version: &str,
) -> Result<PreparedModulePackages, String> {
    if !valid_module_id(module_id) {
        return Err(format!("invalid module id for preinstall: {module_id}"));
    }

    if candidate_version.trim().is_empty() {
        return Err("preinstall candidate version cannot be empty".to_string());
    }

    let revision = resolve_custom_revision()?;

    let essentials_payload = fetch_remote_bytes(
        &revision,
        "runtime/manifests/modules/essentials.packages.tsv",
        30,
    )?;

    let essentials_text = std::str::from_utf8(&essentials_payload)
        .map_err(|error| format!("Essential package selector is not UTF-8: {error}"))?;

    let essentials_manifest_payload = fetch_remote_bytes(
        &revision,
        "runtime/manifests/modules/essentials.manifest.json",
        30,
    )?;

    let essential_layer =
        parse_material_layer("Essential", essentials_text, &essentials_manifest_payload)?;

    let runtime_manifest_payload =
        fetch_remote_bytes(&revision, "runtime/modules/domestic-runtime.json", 30)?;

    let construction_path = format!("runtime/construction/{module_id}.json");
    let construction_payload = fetch_remote_bytes(&revision, &construction_path, 30)?;

    let construction_text = std::str::from_utf8(&construction_payload)
        .map_err(|error| format!("module Construction declaration is not UTF-8: {error}"))?;

    let construction = DomesticConstructionDeclaration::parse(construction_text)?;

    if construction.subject != module_id {
        return Err(format!(
            "module Construction subject mismatch: expected={module_id} actual={}",
            construction.subject
        ));
    }

    // Every nonempty declared rootfs set uses one authenticated V3 contract,
    // including a module that declares exactly one environment. The identity
    // and cardinality come exclusively from Construction, never from Boss.
    // No legacy recipe fallback: a missing V3 recipe fails before publication.
    let (rootfs_bindings, recipes) = fetch_rootfs_recipes(
        &revision,
        module_id,
        candidate_version,
        &construction_payload,
    )?;
    let requirements = unique_rootfs_packages(&essential_layer.packages, &recipes)?;

    let territory =
        neebles_backend::module_material_territory::resolve_module_material_territory()?;

    let (essential_reused, essential_downloaded) = ensure_package_requirements(
        &revision,
        &territory.essential_package_pool,
        "runtime/modules/packages/essentials",
        &essential_layer.packages,
    )?;

    let (module_reused, module_downloaded) = ensure_package_requirements(
        &revision,
        &territory.package_pool,
        "runtime/modules/packages",
        &requirements,
    )?;

    let binding_input = MaterialBindingInput {
        module: module_id.to_string(),
        version: candidate_version.to_string(),
        custom_revision: revision.clone(),
        essential_packages_payload: essentials_payload,
        essential_manifest_payload: essentials_manifest_payload,
        module_packages_payload: Vec::new(),
        module_manifest_payload: Vec::new(),
        runtime_manifest_payload: runtime_manifest_payload.clone(),
        construction_payload: construction_payload.clone(),
    };

    Ok(PreparedModulePackages {
        report: ModulePreinstallReport {
            module: module_id.to_string(),
            custom_revision: revision,
            required: essential_layer.packages.len() + requirements.len(),
            reused: essential_reused + module_reused,
            downloaded: essential_downloaded + module_downloaded,
        },
        binding_input,
        material_root: territory.material_root,
        binding_format: CandidateBindingFormat::MultiRootfsV3(rootfs_bindings),
    })
}

/// Stage every explicitly declared environment inside the unpublished module
/// candidate. The enclosing install/update transaction owns publication and
/// cleanup; a failure here must never publish a partially built module.
pub(crate) fn stage_declared_rootfs(
    prepared: &PreparedModulePackages,
    candidate_directory: &Path,
) -> Result<(), String> {
    let input = &prepared.binding_input;
    let construction_raw = std::str::from_utf8(&input.construction_payload)
        .map_err(|e| format!("Construction is not UTF-8: {e}"))?;
    let declaration = DomesticConstructionDeclaration::parse(construction_raw)?;
    if declaration.subject != input.module || declaration.rootfs.is_empty() {
        return Err("staged rootfs Construction identity/membership mismatch".to_string());
    }
    let essential_selector = std::str::from_utf8(&input.essential_packages_payload)
        .map_err(|e| format!("Essential selector is not UTF-8: {e}"))?;
    let essential = parse_material_layer(
        "Essential",
        essential_selector,
        &input.essential_manifest_payload,
    )?;

    // Authenticate the entire closed recipe set before creating even the first
    // rootfs. No inferred default ID and no best-effort partial installation.
    let recipes: BTreeMap<String, neebles_backend::module_material::MaterialRecipe> =
        match &prepared.binding_format {
            CandidateBindingFormat::LegacyV2 => {
                validate_preinstall_rootfs_recipe_contract(&declaration)?;
                let module_selector = std::str::from_utf8(&input.module_packages_payload)
                    .map_err(|e| format!("module selector is not UTF-8: {e}"))?;
                let recipe = parse_material_recipe(
                    &input.module,
                    &input.version,
                    module_selector,
                    &input.module_manifest_payload,
                )?;
                BTreeMap::from([(declaration.rootfs[0].id.clone(), recipe)])
            }
            CandidateBindingFormat::MultiRootfsV3(rootfs) => {
                neebles_backend::module_material_binding::validate_construction_rootfs_binding(
                    &input.module,
                    &input.version,
                    &input.construction_payload,
                    rootfs,
                )?
            }
        };

    let territory =
        neebles_backend::module_material_territory::resolve_module_material_territory()?;
    stage_authenticated_rootfs_set(
        candidate_directory,
        &declaration,
        &recipes,
        &essential,
        &territory.essential_package_pool,
        &territory.package_pool,
        &input.runtime_manifest_payload,
    )
}

/// Recheck the exact authenticated physical set at the transaction boundary.
/// This closes the window between staging and module publication/deactivation.
pub(crate) fn verify_candidate_rootfs_before_publication(
    prepared: &PreparedModulePackages,
    candidate_directory: &Path,
) -> Result<(), String> {
    let input = &prepared.binding_input;
    let raw = std::str::from_utf8(&input.construction_payload)
        .map_err(|e| format!("Construction is not UTF-8: {e}"))?;
    let declaration = DomesticConstructionDeclaration::parse(raw)?;
    if declaration.subject != input.module || declaration.rootfs.is_empty() {
        return Err("prepublication rootfs Construction identity mismatch".into());
    }
    match &prepared.binding_format {
        CandidateBindingFormat::LegacyV2 => {
            validate_preinstall_rootfs_recipe_contract(&declaration)?;
        }
        CandidateBindingFormat::MultiRootfsV3(rootfs) => {
            neebles_backend::module_material_binding::validate_construction_rootfs_binding(
                &input.module,
                &input.version,
                &input.construction_payload,
                rootfs,
            )?;
        }
    }
    verify_staged_rootfs_inventory(
        candidate_directory,
        &declaration,
        &input.runtime_manifest_payload,
    )?;
    let essential_selector = std::str::from_utf8(&input.essential_packages_payload)
        .map_err(|e| format!("Essential selector is not UTF-8: {e}"))?;
    let essential = parse_material_layer(
        "Essential",
        essential_selector,
        &input.essential_manifest_payload,
    )?;
    let recipes = match &prepared.binding_format {
        CandidateBindingFormat::LegacyV2 => {
            let selector = std::str::from_utf8(&input.module_packages_payload)
                .map_err(|e| format!("module selector is not UTF-8: {e}"))?;
            let recipe = parse_material_recipe(
                &input.module,
                &input.version,
                selector,
                &input.module_manifest_payload,
            )?;
            BTreeMap::from([(declaration.rootfs[0].id.clone(), recipe)])
        }
        CandidateBindingFormat::MultiRootfsV3(rootfs) => {
            neebles_backend::module_material_binding::validate_construction_rootfs_binding(
                &input.module,
                &input.version,
                &input.construction_payload,
                rootfs,
            )?
        }
    };
    for entry in &declaration.rootfs {
        let recipe = recipes
            .get(&entry.id)
            .ok_or_else(|| format!("missing authenticated rootfs recipe: {}", entry.id))?;
        neebles_backend::module_materialization::verify_composed_runtime_rootfs(
            &candidate_directory
                .join("rootfs")
                .join(&entry.id)
                .join("rootfs"),
            &essential,
            recipe,
        )?;
    }
    Ok(())
}

/// Stage the entire authenticated set or remove only the environments created
/// by this invocation. No partial rootfs set is returned as a successful candidate.
fn stage_authenticated_rootfs_set(
    candidate_directory: &Path,
    declaration: &DomesticConstructionDeclaration,
    recipes: &BTreeMap<String, neebles_backend::module_material::MaterialRecipe>,
    essential: &neebles_backend::module_material::MaterialLayer,
    essential_pool: &Path,
    module_pool: &Path,
    runtime_manifest_payload: &[u8],
) -> Result<(), String> {
    let territory = candidate_directory.join("rootfs");
    // Reject altered candidate parents and unexpected recipes before writing.
    let candidate_metadata = fs::symlink_metadata(candidate_directory)
        .map_err(|error| format!("invalid rootfs candidate: {error}"))?;
    if !candidate_metadata.file_type().is_dir() {
        return Err("rootfs candidate must be a real directory".to_string());
    }
    match fs::symlink_metadata(&territory) {
        Ok(metadata) if !metadata.file_type().is_dir() => {
            return Err("rootfs territory must be a real directory".to_string())
        }
        Ok(_) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(format!("cannot inspect rootfs territory: {error}")),
    }
    let declared: std::collections::BTreeSet<&str> = declaration
        .rootfs
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    if declared.len() != declaration.rootfs.len()
        || declared.len() != recipes.len()
        || recipes.keys().any(|id| !declared.contains(id.as_str()))
    {
        return Err("rootfs staging requires exact declared recipe membership".to_string());
    }
    // Refuse collisions up front; never remove anything predating this call.
    for entry in &declaration.rootfs {
        if !recipes.contains_key(&entry.id) {
            return Err(format!("missing authenticated rootfs recipe: {}", entry.id));
        }
        if fs::symlink_metadata(territory.join(&entry.id)).is_ok() {
            return Err(format!("rootfs candidate already exists: {}", entry.id));
        }
    }
    let mut created: Vec<PathBuf> = Vec::new();
    for entry in &declaration.rootfs {
        let recipe = recipes.get(&entry.id).expect("checked before staging");
        match neebles_backend::module_persistent_rootfs::prepare_persistent_rootfs(
            candidate_directory,
            &entry.id,
            essential,
            essential_pool,
            recipe,
            module_pool,
            runtime_manifest_payload,
        ) {
            Ok(built) => {
                created.push(territory.join(&entry.id));
                eprintln!(
                    "N.E.E.B.L.E.S.: staged rootfs '{}' at {}",
                    entry.id,
                    built.rootfs_path.display()
                );
            }
            Err(error) => {
                let mut cleanup_errors = Vec::new();
                for path in created.iter().rev() {
                    if let Err(cleanup) = fs::remove_dir_all(path) {
                        cleanup_errors.push(format!("{}: {cleanup}", path.display()));
                    }
                }
                if !cleanup_errors.is_empty() {
                    return Err(format!(
                        "rootfs staging failed: {error}; cleanup failed: {}",
                        cleanup_errors.join(" | ")
                    ));
                }
                return Err(format!("rootfs staging failed for '{}': {error}", entry.id));
            }
        }
    }
    if let Err(error) =
        verify_staged_rootfs_inventory(candidate_directory, declaration, runtime_manifest_payload)
    {
        let mut cleanup_errors = Vec::new();
        for path in created.iter().rev() {
            if let Err(cleanup) = fs::remove_dir_all(path) {
                cleanup_errors.push(format!("{}: {cleanup}", path.display()));
            }
        }
        if !cleanup_errors.is_empty() {
            return Err(format!(
                "rootfs inventory verification failed: {error}; cleanup failed: {}",
                cleanup_errors.join(" | ")
            ));
        }
        return Err(format!("rootfs inventory verification failed: {error}"));
    }
    Ok(())
}

/// Verify the physically materialized set before a candidate may be published.
/// An additional rootfs, changed runtime manifest or symlink is not tolerated.
fn verify_staged_rootfs_inventory(
    candidate_directory: &Path,
    declaration: &DomesticConstructionDeclaration,
    runtime_manifest_payload: &[u8],
) -> Result<(), String> {
    let territory = candidate_directory.join("rootfs");
    let metadata = fs::symlink_metadata(&territory)
        .map_err(|e| format!("cannot inspect staged rootfs territory: {e}"))?;
    if !metadata.file_type().is_dir() {
        return Err("staged rootfs territory is not a real directory".into());
    }
    let declared: std::collections::BTreeSet<&str> = declaration
        .rootfs
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    if declared.is_empty() || declared.len() != declaration.rootfs.len() {
        return Err("invalid staged rootfs declaration membership".into());
    }
    let mut observed = std::collections::BTreeSet::new();
    for entry in fs::read_dir(&territory)
        .map_err(|e| format!("cannot enumerate staged rootfs territory: {e}"))?
    {
        let entry = entry.map_err(|e| format!("cannot read staged rootfs entry: {e}"))?;
        let id = entry
            .file_name()
            .into_string()
            .map_err(|_| "non-UTF-8 staged rootfs entry".to_string())?;
        if !declared.contains(id.as_str()) || !observed.insert(id.clone()) {
            return Err(format!("undeclared staged rootfs entry: {id}"));
        }
        let environment = entry.path();
        let environment_type = fs::symlink_metadata(&environment)
            .map_err(|e| format!("cannot inspect rootfs '{id}': {e}"))?;
        if !environment_type.file_type().is_dir() {
            return Err(format!("rootfs '{id}' must be a real directory"));
        }
        let rootfs_type = fs::symlink_metadata(environment.join("rootfs"))
            .map_err(|e| format!("rootfs '{id}' payload missing: {e}"))?;
        if !rootfs_type.file_type().is_dir() {
            return Err(format!("rootfs '{id}' payload must be a real directory"));
        }
        let manifest = environment.join("domestic-runtime.json");
        let manifest_type = fs::symlink_metadata(&manifest)
            .map_err(|e| format!("rootfs '{id}' runtime manifest missing: {e}"))?;
        if !manifest_type.file_type().is_file() {
            return Err(format!(
                "rootfs '{id}' runtime manifest must be a regular file"
            ));
        }
        if fs::read(&manifest)
            .map_err(|e| format!("cannot read rootfs '{id}' runtime manifest: {e}"))?
            != runtime_manifest_payload
        {
            return Err(format!(
                "rootfs '{id}' runtime manifest differs from authenticated material"
            ));
        }
    }
    if observed.len() != declared.len() {
        return Err("staged rootfs inventory is incomplete".into());
    }
    Ok(())
}

#[cfg(test)]
mod custom_raw_url_tests {
    use super::custom_raw_url;

    const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

    fn build(path: &str) -> String {
        custom_raw_url(REVISION, path).expect("CUSTOM raw URL must build")
    }

    #[test]
    fn normal_debian_filename_remains_semantically_unchanged() {
        let url =
            build("runtime/modules/packages/essentials/base-files_13.8+deb13u7~fixture_amd64.deb");

        assert_eq!(
            url,
            "https://raw.githubusercontent.com/krockzs/neebles-custom/0123456789abcdef0123456789abcdef01234567/runtime/modules/packages/essentials/base-files_13.8+deb13u7~fixture_amd64.deb"
        );
    }

    #[test]
    fn literal_percent_epoch_is_not_decoded_by_the_url() {
        let url =
            build("runtime/modules/packages/essentials/bsdutils_1%3a2.41.5-0+deb13u1_amd64.deb");

        assert_eq!(
            url,
            "https://raw.githubusercontent.com/krockzs/neebles-custom/0123456789abcdef0123456789abcdef01234567/runtime/modules/packages/essentials/bsdutils_1%253a2.41.5-0+deb13u1_amd64.deb"
        );
    }

    #[test]
    fn encoded_slash_text_cannot_become_a_path_separator() {
        let url = build("runtime/modules/packages/fixture_%2f_payload.deb");

        assert!(url.contains("fixture_%252f_payload.deb"), "{url}");
        assert!(!url.contains("fixture_%2f_payload.deb"), "{url}");
    }

    #[test]
    fn encoded_dot_segments_cannot_become_traversal() {
        let url = build("runtime/modules/packages/fixture_%2e%2e_payload.deb");

        assert!(url.contains("fixture_%252e%252e_payload.deb"), "{url}");
    }

    #[test]
    fn query_fragment_and_space_are_data_inside_the_segment() {
        let url = build("runtime/modules/packages/name #part?.deb");
        let parsed = url::Url::parse(&url).expect("generated URL must parse");

        assert!(url.contains("name%20%23part%3F.deb"), "{url}");
        assert!(
            parsed.query().is_none(),
            "query must not escape package filename"
        );
        assert!(
            parsed.fragment().is_none(),
            "fragment must not escape package filename"
        );
    }

    #[test]
    fn unicode_is_encoded_as_segment_data() {
        let url = build("runtime/modules/packages/módulo_ñ.deb");

        assert!(url.contains("m%C3%B3dulo_%C3%B1.deb"), "{url}");
    }

    #[test]
    fn backslash_cannot_become_url_path_structure() {
        let url = build("runtime/modules/packages/name\\payload.deb");
        let parsed = url::Url::parse(&url).expect("generated URL must parse");

        assert!(url.contains("name%5Cpayload.deb"), "{url}");
        assert_eq!(
            parsed.path_segments().expect("hierarchical URL").count(),
            7,
            "backslash must remain inside one package path segment"
        );
    }

    #[test]
    fn raw_colon_ampersand_equals_semicolon_and_at_remain_path_data() {
        let url = build("runtime/modules/packages/name:@&=;.deb");
        let parsed = url::Url::parse(&url).expect("generated URL must parse");

        assert!(parsed.query().is_none());
        assert!(parsed.fragment().is_none());
        assert_eq!(parsed.path_segments().expect("hierarchical URL").count(), 7);
    }

    #[test]
    fn revision_cannot_inject_url_structure() {
        assert!(custom_raw_url("main/../../escape", "runtime/modules/packages/a.deb").is_err());
        assert!(custom_raw_url(
            "0123456789abcdef0123456789abcdef0123456g",
            "runtime/modules/packages/a.deb"
        )
        .is_err());
    }

    #[test]
    fn structural_path_ambiguity_is_rejected() {
        for path in [
            "",
            "/runtime/modules/packages/a.deb",
            "runtime/modules/packages/a.deb/",
            "runtime//modules/packages/a.deb",
            "runtime/./packages/a.deb",
            "runtime/../packages/a.deb",
        ] {
            assert!(
                custom_raw_url(REVISION, path).is_err(),
                "path should be rejected: {path:?}"
            );
        }
    }

    #[test]
    fn control_characters_are_rejected() {
        for path in [
            "runtime/modules/packages/a\n.deb",
            "runtime/modules/packages/a\0.deb",
        ] {
            assert!(
                custom_raw_url(REVISION, path).is_err(),
                "control path should be rejected: {path:?}"
            );
        }
    }
    fn percent_hex_value(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => panic!("invalid percent hex byte: {byte:?}"),
        }
    }

    fn decode_serialized_segment(value: &str) -> String {
        let bytes = value.as_bytes();
        let mut decoded = Vec::with_capacity(bytes.len());
        let mut index = 0usize;

        while index < bytes.len() {
            if bytes[index] == b'%' {
                assert!(
                    index + 2 < bytes.len(),
                    "truncated percent escape in serialized segment: {value:?}"
                );

                decoded.push(
                    (percent_hex_value(bytes[index + 1]) << 4)
                        | percent_hex_value(bytes[index + 2]),
                );

                index += 3;
            } else {
                decoded.push(bytes[index]);
                index += 1;
            }
        }

        String::from_utf8(decoded)
            .unwrap_or_else(|error| panic!("decoded segment is not UTF-8 for {value:?}: {error}"))
    }

    fn assert_filename_round_trip(filename: &str) {
        let path = format!("runtime/modules/packages/{filename}");
        let generated = custom_raw_url(REVISION, &path)
            .unwrap_or_else(|error| panic!("URL rejected filename {filename:?}: {error}"));

        let parsed = url::Url::parse(&generated).unwrap_or_else(|error| {
            panic!("generated URL does not parse for {filename:?}: {error}")
        });

        assert_eq!(parsed.scheme(), "https", "scheme changed for {filename:?}");
        assert_eq!(
            parsed.host_str(),
            Some("raw.githubusercontent.com"),
            "host changed for {filename:?}: {generated}"
        );

        assert!(
            parsed.query().is_none(),
            "filename escaped into query for {filename:?}: {generated}"
        );

        assert!(
            parsed.fragment().is_none(),
            "filename escaped into fragment for {filename:?}: {generated}"
        );

        let segments = parsed
            .path_segments()
            .expect("raw GitHub URL must be hierarchical")
            .collect::<Vec<_>>();

        assert_eq!(
            segments.len(),
            7,
            "filename changed URL path structure for {filename:?}: {segments:?}"
        );

        assert_eq!(segments[0], "krockzs");
        assert_eq!(segments[1], "neebles-custom");
        assert_eq!(segments[2], REVISION);
        assert_eq!(segments[3], "runtime");
        assert_eq!(segments[4], "modules");
        assert_eq!(segments[5], "packages");

        let decoded = decode_serialized_segment(segments[6]);

        assert_eq!(
            decoded, filename,
            "filename failed exact one-layer URL round-trip: original={filename:?} serialized={:?}",
            segments[6]
        );
    }

    #[test]
    fn every_printable_ascii_filename_character_round_trips() {
        let mut tested = 0usize;

        for code in 0x20u32..=0x7eu32 {
            let character = char::from_u32(code).expect("printable ASCII");

            if character == '/' {
                continue;
            }

            let filename = format!("fixture{character}payload.deb");
            assert_filename_round_trip(&filename);
            tested += 1;
        }

        eprintln!("printable ASCII filename cases: {tested}");
        assert_eq!(tested, 94);
    }

    #[test]
    fn every_percent_hex_triplet_remains_literal_filename_text() {
        let mut tested = 0usize;

        for value in 0u16..=255u16 {
            let upper = format!("fixture%{value:02X}payload.deb");
            let lower = format!("fixture%{value:02x}payload.deb");

            assert_filename_round_trip(&upper);
            assert_filename_round_trip(&lower);

            tested += 2;
        }

        eprintln!("literal percent escape-looking cases: {tested}");
        assert_eq!(tested, 512);
    }

    #[test]
    fn every_ascii_control_character_is_rejected() {
        let mut tested = 0usize;

        for code in (0u32..=31u32).chain(std::iter::once(127u32)) {
            let character = char::from_u32(code).expect("ASCII control character");
            let path = format!("runtime/modules/packages/fixture{character}payload.deb");

            assert!(
                custom_raw_url(REVISION, &path).is_err(),
                "ASCII control character U+{code:04X} was accepted"
            );

            tested += 1;
        }

        eprintln!("ASCII control rejection cases: {tested}");
        assert_eq!(tested, 33);
    }

    #[test]
    fn combinatorial_sensitive_filename_matrix_round_trips() {
        let tokens = [
            "%3a", "%3A", "%2f", "%2F", "%2e", "%2E", "%2e%2e", "%2E%2E", "%00", "%0a", "%0A",
            "%0d", "%0D", "%25", "%ff", "%FF", "%", "%z", "%zz", "+", "~", "#", "?", " ", ":", "@",
            "&", "=", ";", "\\", "[", "]", "(", ")", "'", "\"", "<", ">", "`", "{", "}", "|", "^",
            ",", "!", "$", "*", "é", "ñ", "中", "😀", "\u{0301}", "\u{200f}",
        ];

        let mut singles = 0usize;
        let mut pairs = 0usize;
        let mut triples = 0usize;

        for first in tokens {
            let filename = format!("fixture{first}payload.deb");
            assert_filename_round_trip(&filename);
            singles += 1;
        }

        for first in tokens {
            for second in tokens {
                let filename = format!("fixture{first}{second}payload.deb");
                assert_filename_round_trip(&filename);
                pairs += 1;
            }
        }

        for first in tokens {
            for second in tokens {
                for third in tokens {
                    let filename = format!("fixture{first}{second}{third}payload.deb");
                    assert_filename_round_trip(&filename);
                    triples += 1;
                }
            }
        }

        eprintln!("sensitive token singles: {singles}");
        eprintln!("sensitive token ordered pairs: {pairs}");
        eprintln!("sensitive token ordered triples: {triples}");
        eprintln!("sensitive token total: {}", singles + pairs + triples);

        assert_eq!(singles, tokens.len());
        assert_eq!(pairs, tokens.len() * tokens.len());
        assert_eq!(triples, tokens.len() * tokens.len() * tokens.len());
    }

    #[test]
    fn revision_validation_matrix_is_fail_closed() {
        let lower = "0123456789abcdef0123456789abcdef01234567";
        let upper = "0123456789ABCDEF0123456789ABCDEF01234567";

        assert!(custom_raw_url(lower, "runtime/modules/packages/a.deb").is_ok());
        assert!(custom_raw_url(upper, "runtime/modules/packages/a.deb").is_ok());

        let invalid = [
            "",
            "0123456789abcdef0123456789abcdef0123456",
            "0123456789abcdef0123456789abcdef012345678",
            "0123456789abcdef0123456789abcdef0123456g",
            "0123456789abcdef0123456789abcdef0123456/",
            "0123456789abcdef0123456789abcdef0123456?",
            "0123456789abcdef0123456789abcdef0123456#",
            "0123456789abcdef0123456789abcdef0123456%",
        ];

        for revision in invalid {
            assert!(
                custom_raw_url(revision, "runtime/modules/packages/a.deb").is_err(),
                "invalid revision was accepted: {revision:?}"
            );
        }
    }
}

#[cfg(test)]
mod multirootfs_preinstall_contract_tests {
    use super::*;

    fn construction(rootfs: serde_json::Value, selected: &str) -> DomesticConstructionDeclaration {
        let raw = serde_json::json!({
            "schema": "1", "name": "neebles-domestic-construction",
            "subject": "fixture",
            "rootfs": rootfs,
            "steps": [{ "id": "run", "runtime_authority": "modules.runtime",
                "rootfs": selected, "world": "modules.fixture",
                "execution": "foreground", "session": false }]
        })
        .to_string();
        DomesticConstructionDeclaration::parse(&raw).expect("valid Construction fixture")
    }

    #[test]
    fn preinstall_accepts_arbitrary_declared_rootfs() {
        let fixture = construction(serde_json::json!([{"id":"caca-de-gato"}]), "caca-de-gato");
        assert!(validate_preinstall_rootfs_recipe_contract(&fixture).is_ok());
    }

    #[test]
    fn preinstall_accepts_any_second_identity() {
        let fixture = construction(serde_json::json!([{"id": "motor-x"}]), "motor-x");
        assert!(validate_preinstall_rootfs_recipe_contract(&fixture).is_ok());
    }

    #[test]
    fn preinstall_rejects_unbound_additional_rootfs() {
        let fixture = construction(
            serde_json::json!([
                {"id": "gato"}, {"id": "perro"}
            ]),
            "perro",
        );
        assert!(validate_preinstall_rootfs_recipe_contract(&fixture).is_err());
    }
}

#[cfg(test)]
mod multi_recipe_acquisition_tests {
    use super::{rootfs_recipe_paths, unique_rootfs_packages};
    use neebles_backend::module_material::{MaterialRecipe, PackageRequirement};
    use std::collections::BTreeMap;

    fn recipe(filename: &str, sha: &str) -> MaterialRecipe {
        MaterialRecipe {
            module: "sample".into(),
            version: "1".into(),
            packages: vec![PackageRequirement {
                filename: filename.into(),
                sha256: sha.into(),
            }],
            entries: vec![],
        }
    }

    #[test]
    fn rootfs_urls_derive_from_opaque_declarations() {
        let (a, b) = rootfs_recipe_paths("modulo-x", "caca-de-gato").unwrap();
        assert_eq!(
            a,
            "runtime/manifests/modules/rootfs/modulo-x/caca-de-gato.packages.tsv"
        );
        assert_eq!(
            b,
            "runtime/manifests/modules/rootfs/modulo-x/caca-de-gato.manifest.json"
        );
        assert!(rootfs_recipe_paths("modulo-x", "../escape").is_err());
    }

    #[test]
    fn shared_packages_download_once_and_essential_overlap_fails() {
        let mut recipes = BTreeMap::new();
        recipes.insert("gato".into(), recipe("one.deb", "abc"));
        recipes.insert("perro".into(), recipe("one.deb", "abc"));
        assert_eq!(unique_rootfs_packages(&[], &recipes).unwrap().len(), 1);
        assert!(unique_rootfs_packages(
            &[PackageRequirement {
                filename: "one.deb".into(),
                sha256: "abc".into()
            }],
            &recipes
        )
        .is_err());
        recipes.insert("perro".into(), recipe("one.deb", "different"));
        assert!(unique_rootfs_packages(&[], &recipes).is_err());
    }
}

#[cfg(test)]
mod multirootfs_staging_atomicity_tests {
    use super::*;
    use neebles_backend::module_material::{MaterialLayer, MaterialRecipe, PackageRequirement};

    fn declaration() -> DomesticConstructionDeclaration {
        let raw = serde_json::json!({
            "schema":"1", "name":"neebles-domestic-construction", "subject":"fixture",
            "rootfs":[{"id":"gato"},{"id":"perro"}],
            "steps":[{"id":"run", "runtime_authority":"modules.runtime",
                "rootfs":"perro", "world":"modules.fixture",
                "execution":"foreground", "session":false}]
        })
        .to_string();
        DomesticConstructionDeclaration::parse(&raw).unwrap()
    }

    fn runtime_payload() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema":"1", "name":"neebles-domestic-runtime", "root":"rootfs",
            "worlds":{"fixture.runtime":{"categories":{
                "runtime_paths":{"category":"runtime_paths","value":"fixture-runtime",
                    "declared_targets":["."],"resolved_targets":["."]}
            }}}
        }))
        .unwrap()
    }

    fn recipe() -> MaterialRecipe {
        MaterialRecipe {
            module: "fixture".into(),
            version: "1".into(),
            packages: vec![],
            entries: vec![],
        }
    }

    fn single_declaration() -> DomesticConstructionDeclaration {
        let raw = serde_json::json!({
            "schema":"1", "name":"neebles-domestic-construction", "subject":"fixture",
            "rootfs":[{"id":"gato"}],
            "steps":[{"id":"run", "runtime_authority":"modules.runtime",
                "rootfs":"gato", "world":"modules.fixture",
                "execution":"foreground", "session":false}]
        })
        .to_string();
        DomesticConstructionDeclaration::parse(&raw).unwrap()
    }

    fn prepared_fixture(payload: Vec<u8>) -> PreparedModulePackages {
        PreparedModulePackages {
            report: ModulePreinstallReport {
                module: "fixture".into(),
                custom_revision: "revision".into(),
                required: 0,
                reused: 0,
                downloaded: 0,
            },
            material_root: PathBuf::new(),
            binding_format: CandidateBindingFormat::LegacyV2,
            binding_input: MaterialBindingInput {
                module: "fixture".into(),
                version: "1".into(),
                custom_revision: "revision".into(),
                essential_packages_payload: Vec::new(),
                essential_manifest_payload: br#"{"entries":[]}"#.to_vec(),
                module_packages_payload: Vec::new(),
                module_manifest_payload: br#"{"module":"fixture","version":"1","entries":[]}"#
                    .to_vec(),
                construction_payload: serde_json::json!({
                    "schema":"1", "name":"neebles-domestic-construction", "subject":"fixture",
                    "rootfs":[{"id":"gato"}],
                    "steps":[{"id":"run", "runtime_authority":"modules.runtime",
                        "rootfs":"gato", "world":"modules.fixture",
                        "execution":"foreground", "session":false}]
                })
                .to_string()
                .into_bytes(),
                runtime_manifest_payload: payload,
            },
        }
    }

    // Transaction-level fixture: the same prepared inventory drives physical
    // staging, schema-3 binding preparation and post-publication loading.
    fn prepared_v3_fixture(material_root: PathBuf) -> PreparedModulePackages {
        let mut prepared = prepared_fixture(runtime_payload());
        prepared.material_root = material_root;
        prepared.binding_input.custom_revision = "a".repeat(40);
        prepared.binding_input.essential_manifest_payload = br#"{"entries":[]}"#.to_vec();
        prepared.binding_input.construction_payload = serde_json::json!({
            "schema":"1", "name":"neebles-domestic-construction", "subject":"fixture",
            "rootfs":[{"id":"gato"},{"id":"perro"}],
            "steps":[{"id":"run", "runtime_authority":"modules.runtime",
                "rootfs":"perro", "world":"modules.fixture",
                "execution":"foreground", "session":false}]
        })
        .to_string()
        .into_bytes();
        prepared.binding_format = CandidateBindingFormat::MultiRootfsV3(
            ["gato", "perro"]
                .iter()
                .map(|id| RootfsBindingInput {
                    id: (*id).to_string(),
                    packages_payload: Vec::new(),
                    manifest_payload: serde_json::json!({
                        "module":"fixture", "version":"1", "rootfs":id, "entries":[]
                    })
                    .to_string()
                    .into_bytes(),
                })
                .collect(),
        );
        prepared
    }

    #[test]
    fn schema_three_physical_candidate_and_active_binding_agree_after_publish() {
        let tmp = tempfile::tempdir().unwrap();
        let prepared = prepared_v3_fixture(tmp.path().join("material"));
        let candidate = tmp.path().join("unpublished");
        let installed = tmp.path().join("installed");
        fs::create_dir(&candidate).unwrap();
        stage_declared_rootfs_for_test(&prepared, &candidate, tmp.path()).unwrap();
        let binding = prepared.prepare_binding().unwrap();
        verify_candidate_rootfs_before_publication(&prepared, &candidate).unwrap();
        assert!(!installed.exists());
        assert!(!prepared.material_root.join("fixture").exists());
        fs::rename(&candidate, &installed).unwrap();
        binding.activate_install().unwrap();
        let active = neebles_backend::module_material_binding::load_installed_material_binding(
            &prepared.material_root,
            "fixture",
        )
        .unwrap();
        match active {
            neebles_backend::module_material_binding::InstalledMaterialBinding::MultiRootfs(v3) => {
                assert_eq!(v3.rootfs_recipes.len(), 2);
                for id in ["gato", "perro"] {
                    assert!(v3.rootfs_recipes.contains_key(id));
                    assert!(installed.join("rootfs").join(id).join("rootfs").is_dir());
                    assert_eq!(
                        fs::read(
                            installed
                                .join("rootfs")
                                .join(id)
                                .join("domestic-runtime.json")
                        )
                        .unwrap(),
                        prepared.binding_input.runtime_manifest_payload
                    );
                }
            }
            _ => panic!("schema-3 candidate unexpectedly activated legacy binding"),
        }
    }

    #[test]
    fn schema_three_single_rootfs_publishes_one_environment_and_one_recipe() {
        let tmp = tempfile::tempdir().unwrap();
        let mut prepared = prepared_v3_fixture(tmp.path().join("material"));
        prepared.binding_input.construction_payload = serde_json::json!({
            "schema":"1", "name":"neebles-domestic-construction", "subject":"fixture",
            "rootfs":[{"id":"solo"}],
            "steps":[{"id":"run", "runtime_authority":"modules.runtime",
                "rootfs":"solo", "world":"modules.fixture",
                "execution":"foreground", "session":false}]
        })
        .to_string()
        .into_bytes();
        prepared.binding_format = CandidateBindingFormat::MultiRootfsV3(vec![RootfsBindingInput {
            id: "solo".into(),
            packages_payload: Vec::new(),
            manifest_payload: serde_json::json!({
                "module":"fixture", "version":"1", "rootfs":"solo", "entries":[]
            })
            .to_string()
            .into_bytes(),
        }]);
        let candidate = tmp.path().join("candidate");
        let installed = tmp.path().join("installed");
        fs::create_dir(&candidate).unwrap();
        stage_declared_rootfs_for_test(&prepared, &candidate, tmp.path()).unwrap();
        verify_candidate_rootfs_before_publication(&prepared, &candidate).unwrap();
        fs::rename(&candidate, &installed).unwrap();
        prepared
            .prepare_binding()
            .unwrap()
            .activate_install()
            .unwrap();
        let active = neebles_backend::module_material_binding::load_installed_material_binding(
            &prepared.material_root,
            "fixture",
        )
        .unwrap();
        match active {
            neebles_backend::module_material_binding::InstalledMaterialBinding::MultiRootfs(v3) => {
                assert_eq!(v3.rootfs_recipes.len(), 1);
                assert!(v3.rootfs_recipes.contains_key("solo"));
                assert!(installed.join("rootfs/solo/rootfs").is_dir());
                assert!(!installed.join("rootfs/gato").exists());
                assert!(!installed.join("rootfs/perro").exists());
            }
            _ => panic!("a single rootfs must publish a V3 binding"),
        }
    }

    #[test]
    fn schema_three_binding_activation_collision_preserves_previous_and_candidate_can_compensate() {
        let tmp = tempfile::tempdir().unwrap();
        let prepared = prepared_v3_fixture(tmp.path().join("material"));
        let candidate = tmp.path().join("candidate");
        let installed = tmp.path().join("installed");
        fs::create_dir(&candidate).unwrap();
        stage_declared_rootfs_for_test(&prepared, &candidate, tmp.path()).unwrap();
        let first = prepared.prepare_binding().unwrap();
        first.activate_install().unwrap();
        let prior = neebles_backend::module_material_binding::load_installed_material_binding(
            &prepared.material_root,
            "fixture",
        )
        .unwrap();
        let second = prepared.prepare_binding().unwrap();
        verify_candidate_rootfs_before_publication(&prepared, &candidate).unwrap();
        fs::rename(&candidate, &installed).unwrap();
        assert!(second.activate_install().is_err());
        // Mirror the install transaction's compensation of the newly published
        // module; an already-active binding must remain unchanged.
        fs::remove_dir_all(&installed).unwrap();
        assert!(!installed.exists());
        let restored = neebles_backend::module_material_binding::load_installed_material_binding(
            &prepared.material_root,
            "fixture",
        )
        .unwrap();
        assert_eq!(restored.custom_revision(), prior.custom_revision());
    }

    fn stage_declared_rootfs_for_test(
        prepared: &PreparedModulePackages,
        candidate: &Path,
        package_pool: &Path,
    ) -> Result<(), String> {
        let raw = std::str::from_utf8(&prepared.binding_input.construction_payload)
            .map_err(|error| error.to_string())?;
        let declared = DomesticConstructionDeclaration::parse(raw)?;
        let rootfs = match &prepared.binding_format {
            CandidateBindingFormat::MultiRootfsV3(rootfs) => rootfs,
            _ => return Err("expected schema-3 fixture".into()),
        };
        let recipes =
            neebles_backend::module_material_binding::validate_construction_rootfs_binding(
                &prepared.binding_input.module,
                &prepared.binding_input.version,
                &prepared.binding_input.construction_payload,
                rootfs,
            )?;
        stage_authenticated_rootfs_set(
            candidate,
            &declared,
            &recipes,
            &MaterialLayer {
                packages: vec![],
                entries: vec![],
            },
            package_pool,
            package_pool,
            &prepared.binding_input.runtime_manifest_payload,
        )
    }

    #[test]
    fn prepublication_recheck_accepts_untampered_staged_set() {
        let tmp = tempfile::tempdir().unwrap();
        let candidate = tmp.path().join("candidate");
        fs::create_dir(&candidate).unwrap();
        let payload = runtime_payload();
        let recipes = BTreeMap::from([("gato".into(), recipe())]);
        let essential = MaterialLayer {
            packages: vec![],
            entries: vec![],
        };
        stage_authenticated_rootfs_set(
            &candidate,
            &single_declaration(),
            &recipes,
            &essential,
            tmp.path(),
            tmp.path(),
            &payload,
        )
        .unwrap();
        assert!(
            verify_candidate_rootfs_before_publication(&prepared_fixture(payload), &candidate)
                .is_ok()
        );
    }

    #[test]
    fn prepublication_recheck_rejects_physical_tamper() {
        let tmp = tempfile::tempdir().unwrap();
        let candidate = tmp.path().join("candidate");
        fs::create_dir(&candidate).unwrap();
        let payload = runtime_payload();
        let recipes = BTreeMap::from([("gato".into(), recipe())]);
        let essential = MaterialLayer {
            packages: vec![],
            entries: vec![],
        };
        stage_authenticated_rootfs_set(
            &candidate,
            &single_declaration(),
            &recipes,
            &essential,
            tmp.path(),
            tmp.path(),
            &payload,
        )
        .unwrap();
        fs::write(
            candidate.join("rootfs/gato/domestic-runtime.json"),
            b"modified",
        )
        .unwrap();
        assert!(
            verify_candidate_rootfs_before_publication(&prepared_fixture(payload), &candidate)
                .is_err()
        );
    }

    #[test]
    fn two_independent_rootfs_materialize_in_one_unpublished_candidate() {
        let tmp = tempfile::tempdir().unwrap();
        let candidate = tmp.path().join("candidate");
        fs::create_dir(&candidate).unwrap();
        let recipes = BTreeMap::from([("gato".into(), recipe()), ("perro".into(), recipe())]);
        let essential = MaterialLayer {
            packages: vec![],
            entries: vec![],
        };
        stage_authenticated_rootfs_set(
            &candidate,
            &declaration(),
            &recipes,
            &essential,
            tmp.path(),
            tmp.path(),
            &runtime_payload(),
        )
        .unwrap();
        assert!(candidate.join("rootfs/gato/rootfs").is_dir());
        assert!(candidate.join("rootfs/perro/rootfs").is_dir());
    }

    #[test]
    fn physical_inventory_rejects_extra_and_missing_rootfs() {
        let tmp = tempfile::tempdir().unwrap();
        let candidate = tmp.path().join("candidate");
        fs::create_dir(&candidate).unwrap();
        let recipes = BTreeMap::from([("gato".into(), recipe()), ("perro".into(), recipe())]);
        let essential = MaterialLayer {
            packages: vec![],
            entries: vec![],
        };
        let payload = runtime_payload();
        stage_authenticated_rootfs_set(
            &candidate,
            &declaration(),
            &recipes,
            &essential,
            tmp.path(),
            tmp.path(),
            &payload,
        )
        .unwrap();
        fs::create_dir(candidate.join("rootfs/intruso")).unwrap();
        assert!(verify_staged_rootfs_inventory(&candidate, &declaration(), &payload).is_err());
        fs::remove_dir(candidate.join("rootfs/intruso")).unwrap();
        fs::remove_dir_all(candidate.join("rootfs/perro")).unwrap();
        assert!(verify_staged_rootfs_inventory(&candidate, &declaration(), &payload).is_err());
    }

    #[test]
    fn physical_inventory_rejects_tampered_manifest_and_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let candidate = tmp.path().join("candidate");
        fs::create_dir(&candidate).unwrap();
        let recipes = BTreeMap::from([("gato".into(), recipe()), ("perro".into(), recipe())]);
        let essential = MaterialLayer {
            packages: vec![],
            entries: vec![],
        };
        let payload = runtime_payload();
        stage_authenticated_rootfs_set(
            &candidate,
            &declaration(),
            &recipes,
            &essential,
            tmp.path(),
            tmp.path(),
            &payload,
        )
        .unwrap();
        let manifest = candidate.join("rootfs/gato/domestic-runtime.json");
        fs::write(&manifest, b"modified").unwrap();
        assert!(verify_staged_rootfs_inventory(&candidate, &declaration(), &payload).is_err());
        fs::write(&manifest, &payload).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let original = candidate.join("rootfs/perro/rootfs");
            fs::remove_dir(&original).unwrap();
            symlink(candidate.join("rootfs/gato/rootfs"), &original).unwrap();
            assert!(verify_staged_rootfs_inventory(&candidate, &declaration(), &payload).is_err());
        }
    }

    #[test]
    fn undeclared_recipe_rejected_before_any_rootfs_is_created() {
        let tmp = tempfile::tempdir().unwrap();
        let candidate = tmp.path().join("candidate");
        fs::create_dir(&candidate).unwrap();
        let recipes = BTreeMap::from([
            ("gato".into(), recipe()),
            ("perro".into(), recipe()),
            ("intruso".into(), recipe()),
        ]);
        let essential = MaterialLayer {
            packages: vec![],
            entries: vec![],
        };
        assert!(stage_authenticated_rootfs_set(
            &candidate,
            &declaration(),
            &recipes,
            &essential,
            tmp.path(),
            tmp.path(),
            &runtime_payload()
        )
        .is_err());
        assert!(!candidate.join("rootfs").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_rootfs_territory_rejected_without_touching_target() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let candidate = tmp.path().join("candidate");
        let external = tmp.path().join("external");
        fs::create_dir(&candidate).unwrap();
        fs::create_dir(&external).unwrap();
        symlink(&external, candidate.join("rootfs")).unwrap();
        let recipes = BTreeMap::from([("gato".into(), recipe()), ("perro".into(), recipe())]);
        let essential = MaterialLayer {
            packages: vec![],
            entries: vec![],
        };
        assert!(stage_authenticated_rootfs_set(
            &candidate,
            &declaration(),
            &recipes,
            &essential,
            tmp.path(),
            tmp.path(),
            &runtime_payload()
        )
        .is_err());
        assert!(fs::read_dir(&external).unwrap().next().is_none());
    }

    #[test]
    fn failure_in_second_rootfs_removes_the_first_without_publishing() {
        let tmp = tempfile::tempdir().unwrap();
        let candidate = tmp.path().join("candidate");
        fs::create_dir(&candidate).unwrap();
        let mut bad = recipe();
        bad.packages.push(PackageRequirement {
            filename: "missing.deb".into(),
            sha256: "0".repeat(64),
        });
        let recipes = BTreeMap::from([("gato".into(), recipe()), ("perro".into(), bad)]);
        let essential = MaterialLayer {
            packages: vec![],
            entries: vec![],
        };
        assert!(stage_authenticated_rootfs_set(
            &candidate,
            &declaration(),
            &recipes,
            &essential,
            tmp.path(),
            tmp.path(),
            &runtime_payload()
        )
        .is_err());
        assert!(!candidate.join("rootfs/gato").exists());
        assert!(!candidate.join("rootfs/perro").exists());
    }
}

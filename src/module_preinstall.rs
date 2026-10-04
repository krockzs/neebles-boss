use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::module_material::{
    parse_material_recipe, valid_module_id, MaterialRecipe, PackageRequirement,
};

const CUSTOM_REPOSITORY: &str = "krockzs/neebles-custom";
const CUSTOM_REPOSITORY_GIT: &str = "https://github.com/krockzs/neebles-custom.git";
const CUSTOM_BRANCH_CANDIDATES: [&str; 2] = ["main", "master"];
const DOMESTIC_WORKSPACE_AUTHORITY: &str = "neebles.domestic_workspace";

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
    pub(crate) recipe: MaterialRecipe,
    pub(crate) package_pool: PathBuf,
    pub(crate) shared_rootfs: PathBuf,
    pub(crate) runtime_manifest_payload: Vec<u8>,
    pub(crate) shared_runtime_manifest: PathBuf,
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

fn custom_raw_url(revision: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{CUSTOM_REPOSITORY}/{revision}/{path}")
}

fn fetch_remote_bytes(revision: &str, path: &str, timeout_seconds: u32) -> Result<Vec<u8>, String> {
    let url = custom_raw_url(revision, path);

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

fn authorized_material_territory() -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let registry =
        neebles_backend::domestic_authority_supply_process::process_supplied_authority_registry()?;

    let grants = neebles_backend::domestic_authority_supply::build_authority_grant_set(
        registry,
        [DOMESTIC_WORKSPACE_AUTHORITY],
    )?;

    let descriptor_path = grants.descriptor_path(registry, DOMESTIC_WORKSPACE_AUTHORITY)?;

    let descriptor =
        neebles_backend::domestic_writable_data_authority::load_writable_data_authority_descriptor(
            &descriptor_path,
            DOMESTIC_WORKSPACE_AUTHORITY,
        )?;

    let modules = descriptor.root.join("modules");
    let packages = modules.join("packages");
    let rootfs = modules.join("rootfs");
    let runtime_manifest = modules.join("domestic-runtime.json");

    let packages_grant =
        neebles_backend::domestic_writable_data_authority::grant_writable_data_subpath(
            &descriptor,
            &packages,
            &packages,
        )?;

    let rootfs_grant =
        neebles_backend::domestic_writable_data_authority::grant_writable_data_subpath(
            &descriptor,
            &rootfs,
            &rootfs,
        )?;

    let runtime_manifest_grant =
        neebles_backend::domestic_writable_data_authority::grant_writable_data_subpath(
            &descriptor,
            &runtime_manifest,
            &runtime_manifest,
        )?;

    Ok((
        packages_grant.source,
        rootfs_grant.source,
        runtime_manifest_grant.source,
    ))
}

fn download_package(
    revision: &str,
    pool: &Path,
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

    let remote_path = format!("runtime/modules/packages/{}", requirement.filename);

    let url = custom_raw_url(revision, &remote_path);

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

    fs::set_permissions(
        temporary.path(),
        fs::Permissions::from_mode(requirement.mode),
    )
    .map_err(|error| {
        format!(
            "could not apply module package mode to '{}': {error}",
            requirement.filename
        )
    })?;

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

pub(crate) fn ensure_module_packages(module_id: &str) -> Result<PreparedModulePackages, String> {
    if !valid_module_id(module_id) {
        return Err(format!("invalid module id for preinstall: {module_id}"));
    }

    let installed = crate::modules::installed_module_manifest(module_id)?;

    let revision = resolve_custom_revision()?;

    let packages_path = format!("runtime/manifests/modules/{module_id}.packages.tsv");

    let manifest_path = format!("runtime/manifests/modules/{module_id}.manifest.json");

    let packages_payload = fetch_remote_bytes(&revision, &packages_path, 30)?;

    let packages_text = String::from_utf8(packages_payload)
        .map_err(|error| format!("module package selector is not UTF-8: {error}"))?;

    let manifest_payload = fetch_remote_bytes(&revision, &manifest_path, 30)?;

    let recipe = parse_material_recipe(
        module_id,
        &installed.version,
        &packages_text,
        &manifest_payload,
    )?;

    let requirements = &recipe.packages;

    let runtime_manifest_payload =
        fetch_remote_bytes(&revision, "runtime/modules/domestic-runtime.json", 30)?;

    let (pool, shared_rootfs, shared_runtime_manifest) = authorized_material_territory()?;

    let mut reused = 0usize;
    let mut downloaded = 0usize;

    for requirement in requirements {
        let target = pool.join(&requirement.filename);

        if verify_package(&target, &requirement.sha256)? {
            reused += 1;
            continue;
        }

        if download_package(&revision, &pool, requirement)? {
            downloaded += 1;
        } else {
            reused += 1;
        }
    }

    Ok(PreparedModulePackages {
        report: ModulePreinstallReport {
            module: module_id.to_string(),
            custom_revision: revision,
            required: requirements.len(),
            reused,
            downloaded,
        },
        recipe,
        package_pool: pool,
        shared_rootfs,
        runtime_manifest_payload,
        shared_runtime_manifest,
    })
}

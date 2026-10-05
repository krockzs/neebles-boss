use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use neebles_backend::module_material::{
    parse_material_layer, parse_material_recipe, valid_module_id, PackageRequirement,
};
use neebles_backend::module_material_binding::MaterialBindingInput;

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

pub(crate) fn ensure_module_packages(module_id: &str) -> Result<PreparedModulePackages, String> {
    if !valid_module_id(module_id) {
        return Err(format!("invalid module id for preinstall: {module_id}"));
    }

    let installed = crate::modules::installed_module_manifest(module_id)?;

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

    let packages_path = format!("runtime/manifests/modules/{module_id}.packages.tsv");
    let manifest_path = format!("runtime/manifests/modules/{module_id}.manifest.json");

    let packages_payload = fetch_remote_bytes(&revision, &packages_path, 30)?;

    let packages_text = std::str::from_utf8(&packages_payload)
        .map_err(|error| format!("module package selector is not UTF-8: {error}"))?;

    let manifest_payload = fetch_remote_bytes(&revision, &manifest_path, 30)?;

    let recipe = parse_material_recipe(
        module_id,
        &installed.version,
        packages_text,
        &manifest_payload,
    )?;

    for requirement in &recipe.packages {
        if essential_layer
            .packages
            .iter()
            .any(|essential| essential.filename.as_str() == requirement.filename.as_str())
        {
            return Err(format!(
                "module package delta repeats Essential package: {}",
                requirement.filename
            ));
        }
    }

    let runtime_manifest_payload =
        fetch_remote_bytes(&revision, "runtime/modules/domestic-runtime.json", 30)?;

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
        &recipe.packages,
    )?;

    let binding_input = MaterialBindingInput {
        module: module_id.to_string(),
        version: installed.version.clone(),
        custom_revision: revision.clone(),
        essential_packages_payload: essentials_payload,
        essential_manifest_payload: essentials_manifest_payload,
        module_packages_payload: packages_payload,
        module_manifest_payload: manifest_payload,
        runtime_manifest_payload: runtime_manifest_payload.clone(),
    };

    Ok(PreparedModulePackages {
        report: ModulePreinstallReport {
            module: module_id.to_string(),
            custom_revision: revision,
            required: essential_layer.packages.len() + recipe.packages.len(),
            reused: essential_reused + module_reused,
            downloaded: essential_downloaded + module_downloaded,
        },
        binding_input,
        material_root: territory.material_root,
    })
}

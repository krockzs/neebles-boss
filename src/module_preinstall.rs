use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

const CUSTOM_REPOSITORY: &str = "krockzs/neebles-custom";
const CUSTOM_REPOSITORY_GIT: &str = "https://github.com/krockzs/neebles-custom.git";
const CUSTOM_BRANCH_CANDIDATES: [&str; 3] = ["neebles-custom", "main", "master"];
const DOMESTIC_WORKSPACE_AUTHORITY: &str = "neebles.domestic_workspace";

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageSelector {
    filename: String,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageRequirement {
    filename: String,
    sha256: String,
    mode: u32,
}

#[derive(Debug, Deserialize)]
struct MaterialManifest {
    module: String,
    version: String,
    entries: Vec<MaterialManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct MaterialManifestEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    mode: String,
    sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulePreinstallReport {
    pub module: String,
    pub custom_revision: String,
    pub required: usize,
    pub reused: usize,
    pub downloaded: usize,
}

fn valid_module_id(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|value| value.is_ascii_hexdigit())
}

fn valid_single_filename(value: &str) -> bool {
    if value.is_empty() || value == "." || value == ".." {
        return false;
    }

    let path = Path::new(value);

    !path.is_absolute()
        && path.components().count() == 1
        && path.file_name().and_then(|name| name.to_str()) == Some(value)
}

fn parse_mode(value: &str) -> Result<u32, String> {
    let digits = value
        .strip_prefix("0o")
        .ok_or_else(|| format!("material mode must use 0o notation: {value}"))?;

    let mode = u32::from_str_radix(digits, 8)
        .map_err(|error| format!("invalid material mode '{value}': {error}"))?;

    if mode > 0o7777 {
        return Err(format!("material mode is outside supported range: {value}"));
    }

    Ok(mode)
}

fn parse_packages_tsv(payload: &str) -> Result<BTreeMap<String, PackageSelector>, String> {
    let mut packages = BTreeMap::new();

    for (index, raw) in payload.lines().enumerate() {
        let line = raw.trim();

        if line.is_empty() {
            continue;
        }

        let fields = line.split('\t').collect::<Vec<_>>();

        if fields.len() != 5 {
            return Err(format!(
                "invalid module package selector line {}",
                index + 1
            ));
        }

        let package = fields[0].trim();
        let version = fields[1].trim();
        let arch = fields[2].trim();
        let filename = fields[3].trim();
        let sha256 = fields[4].trim();

        if package.is_empty() || version.is_empty() || arch.is_empty() {
            return Err(format!(
                "incomplete module package selector line {}",
                index + 1
            ));
        }

        if !valid_single_filename(filename) {
            return Err(format!("unsafe module package filename: {filename}"));
        }

        if !valid_sha256(sha256) {
            return Err(format!("invalid module package sha256 for {filename}"));
        }

        let selector = PackageSelector {
            filename: filename.to_string(),
            sha256: sha256.to_ascii_lowercase(),
        };

        if packages.insert(filename.to_string(), selector).is_some() {
            return Err(format!("duplicate module package filename: {filename}"));
        }
    }

    Ok(packages)
}

fn validate_material_recipe(
    module_id: &str,
    module_version: &str,
    selectors: &BTreeMap<String, PackageSelector>,
    payload: &[u8],
) -> Result<Vec<PackageRequirement>, String> {
    let manifest: MaterialManifest = serde_json::from_slice(payload)
        .map_err(|error| format!("invalid module material manifest JSON: {error}"))?;

    if manifest.module != module_id {
        return Err(format!(
            "module material manifest identity mismatch: expected={module_id} actual={}",
            manifest.module
        ));
    }

    if manifest.version != module_version {
        return Err(format!(
            "module material manifest version mismatch: module={module_version} material={}",
            manifest.version
        ));
    }

    let mut entries = BTreeMap::<String, &MaterialManifestEntry>::new();

    for entry in &manifest.entries {
        let Some(filename) = entry.path.strip_prefix("packages/") else {
            continue;
        };

        if !valid_single_filename(filename) {
            return Err(format!(
                "unsafe package path in module material manifest: {}",
                entry.path
            ));
        }

        if entries.insert(filename.to_string(), entry).is_some() {
            return Err(format!(
                "duplicate package entry in module material manifest: {filename}"
            ));
        }
    }

    let declared = selectors.keys().cloned().collect::<Vec<_>>();
    let manifested = entries.keys().cloned().collect::<Vec<_>>();

    if declared != manifested {
        return Err(format!(
            "module package membership mismatch: selector={declared:?} manifest={manifested:?}"
        ));
    }

    let mut requirements = Vec::with_capacity(selectors.len());

    for selector in selectors.values() {
        let entry = entries.get(&selector.filename).ok_or_else(|| {
            format!(
                "module material manifest is missing package {}",
                selector.filename
            )
        })?;

        if entry.kind != "file" {
            return Err(format!(
                "module package entry is not a file: {}",
                selector.filename
            ));
        }

        let manifest_sha = entry
            .sha256
            .as_deref()
            .ok_or_else(|| {
                format!(
                    "module package manifest sha256 missing: {}",
                    selector.filename
                )
            })?
            .to_ascii_lowercase();

        if manifest_sha != selector.sha256 {
            return Err(format!(
                "module package sha256 authority mismatch: {}",
                selector.filename
            ));
        }

        requirements.push(PackageRequirement {
            filename: selector.filename.clone(),
            sha256: selector.sha256.clone(),
            mode: parse_mode(&entry.mode)?,
        });
    }

    Ok(requirements)
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

fn authorized_package_pool() -> Result<PathBuf, String> {
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

    let pool = descriptor.root.join("modules").join("packages");

    let grant = neebles_backend::domestic_writable_data_authority::grant_writable_data_subpath(
        &descriptor,
        &pool,
        &pool,
    )?;

    Ok(grant.source)
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

pub fn ensure_module_packages(module_id: &str) -> Result<ModulePreinstallReport, String> {
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

    let selectors = parse_packages_tsv(&packages_text)?;

    let manifest_payload = fetch_remote_bytes(&revision, &manifest_path, 30)?;

    let requirements =
        validate_material_recipe(module_id, &installed.version, &selectors, &manifest_payload)?;

    let pool = authorized_package_pool()?;

    let mut reused = 0usize;
    let mut downloaded = 0usize;

    for requirement in &requirements {
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

    Ok(ModulePreinstallReport {
        module: module_id.to_string(),
        custom_revision: revision,
        required: requirements.len(),
        reused,
        downloaded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    #[test]
    fn package_selector_carries_filename_and_sha() {
        let payload = format!(
            "python3.13-minimal\t3.13.5\tamd64\tpython.deb\t{}\n",
            sha('a')
        );

        let parsed = parse_packages_tsv(&payload).expect("valid package selector must parse");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed["python.deb"].sha256, sha('a'));
    }

    #[test]
    fn package_selector_rejects_path_escape() {
        let payload = format!(
            "python3.13-minimal\t3.13.5\tamd64\t../python.deb\t{}\n",
            sha('a')
        );

        let error =
            parse_packages_tsv(&payload).expect_err("package selector path escape must fail");

        assert!(error.contains("unsafe module package filename"));
    }

    #[test]
    fn material_recipe_requires_same_package_sha() {
        let payload = format!(
            r#"{{
                "module": "fixture",
                "version": "1.0.0",
                "entries": [
                    {{
                        "path": "packages/python.deb",
                        "type": "file",
                        "mode": "0o664",
                        "sha256": "{}"
                    }}
                ]
            }}"#,
            sha('b')
        );

        let selectors =
            parse_packages_tsv(&format!("python\t1\tamd64\tpython.deb\t{}\n", sha('a')))
                .expect("fixture selector must parse");

        let error = validate_material_recipe("fixture", "1.0.0", &selectors, payload.as_bytes())
            .expect_err("divergent package sha must fail");

        assert!(error.contains("sha256 authority mismatch"));
    }

    #[test]
    fn material_recipe_accepts_matching_membership() {
        let expected_sha = sha('a');

        let payload = format!(
            r#"{{
                "module": "fixture",
                "version": "1.0.0",
                "entries": [
                    {{
                        "path": "packages/python.deb",
                        "type": "file",
                        "mode": "0o664",
                        "sha256": "{expected_sha}"
                    }}
                ]
            }}"#
        );

        let selectors =
            parse_packages_tsv(&format!("python\t1\tamd64\tpython.deb\t{expected_sha}\n"))
                .expect("fixture selector must parse");

        let requirements =
            validate_material_recipe("fixture", "1.0.0", &selectors, payload.as_bytes())
                .expect("matching material recipe must validate");

        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].filename, "python.deb");
        assert_eq!(requirements[0].mode, 0o664);
    }
}

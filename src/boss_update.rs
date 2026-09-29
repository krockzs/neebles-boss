use flate2::read::GzDecoder;
use semver::Version;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tar::Archive;
use tempfile::TempDir;
use url::Url;

pub const BOSS_BOOTSTRAP_URL: &str =
    "https://github.com/krockzs/neebles-boss/releases/latest/download/bootstrap.json";

#[derive(Debug, Clone, Deserialize)]
struct Bootstrap {
    schema: u32,
    channel: String,
    stable: StableRelease,
}

#[derive(Debug, Clone, Deserialize)]
struct StableRelease {
    version: String,
    base_url: String,

    #[serde(default)]
    assets: BTreeMap<String, ReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct ReleaseAsset {
    file: String,
    sha256: String,
}

fn curl_bytes(url: &str) -> Result<Vec<u8>, String> {
    let arguments = [
        OsString::from("-fsSL"),
        OsString::from("--max-time"),
        OsString::from("30"),
        OsString::from(url),
    ];

    let output = crate::network_boundary::command("boss.curl", &arguments)?
        .output()
        .map_err(|error| format!("could not execute Boss release request: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "Boss release request failed with status {}",
            output.status
        ));
    }

    Ok(output.stdout)
}

fn validate_bootstrap(bootstrap: &Bootstrap) -> Result<(), String> {
    if bootstrap.schema != 2 {
        return Err(format!(
            "unsupported Boss bootstrap schema: {}",
            bootstrap.schema
        ));
    }

    if bootstrap.channel != "stable" {
        return Err(format!(
            "unsupported Boss release channel: {}",
            bootstrap.channel
        ));
    }

    Version::parse(bootstrap.stable.version.trim()).map_err(|error| {
        format!(
            "invalid Boss stable version '{}': {error}",
            bootstrap.stable.version
        )
    })?;

    let base = Url::parse(bootstrap.stable.base_url.trim())
        .map_err(|error| format!("invalid Boss stable base_url: {error}"))?;

    if base.scheme() != "https" {
        return Err("Boss stable base_url must use https".to_string());
    }

    Ok(())
}

fn fetch_bootstrap() -> Result<Bootstrap, String> {
    let raw = curl_bytes(BOSS_BOOTSTRAP_URL)?;

    let bootstrap: Bootstrap = serde_json::from_slice(&raw)
        .map_err(|error| format!("invalid Boss bootstrap JSON: {error}"))?;

    validate_bootstrap(&bootstrap)?;

    Ok(bootstrap)
}

fn update_is_available(bootstrap: &Bootstrap) -> Result<bool, String> {
    let installed = Version::parse(crate::VERSION).map_err(|error| {
        format!(
            "invalid installed Boss version '{}': {error}",
            crate::VERSION
        )
    })?;

    let remote = Version::parse(bootstrap.stable.version.trim()).map_err(|error| {
        format!(
            "invalid remote Boss version '{}': {error}",
            bootstrap.stable.version
        )
    })?;

    Ok(remote > installed)
}

pub fn status_json() -> Result<Value, String> {
    let bootstrap = fetch_bootstrap()?;

    Ok(json!({
        "installed_version":
            crate::VERSION,

        "remote_version":
            bootstrap.stable.version,

        "update_available":
            update_is_available(
                &bootstrap
            )?,
    }))
}

fn required_asset<'a>(bootstrap: &'a Bootstrap, key: &str) -> Result<&'a ReleaseAsset, String> {
    bootstrap
        .stable
        .assets
        .get(key)
        .ok_or_else(|| format!("Boss release is missing required asset '{key}'"))
}

fn validate_asset(key: &str, asset: &ReleaseAsset) -> Result<(), String> {
    let file = asset.file.trim();

    let digest = asset.sha256.trim();

    if file.is_empty() {
        return Err(format!("Boss release asset '{key}' has empty filename"));
    }

    let path = Path::new(file);

    if path.is_absolute() || path.components().count() != 1 {
        return Err(format!(
            "Boss release asset '{key}' has unsafe filename '{}'",
            asset.file
        ));
    }

    if digest.len() != 64 || !digest.bytes().all(|value| value.is_ascii_hexdigit()) {
        return Err(format!("Boss release asset '{key}' has invalid sha256"));
    }

    Ok(())
}

fn digest_hex(bytes: impl AsRef<[u8]>) -> String {
    let mut output = String::with_capacity(64);

    for byte in bytes.as_ref() {
        output.push_str(&format!("{byte:02x}"));
    }

    output
}

fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let mut file = File::open(path).map_err(|error| {
        format!(
            "could not open Boss update asset {}: {error}",
            path.display()
        )
    })?;

    let mut hasher = Sha256::new();

    let mut buffer = [0u8; 1024 * 1024];

    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            format!(
                "could not hash Boss update asset {}: {error}",
                path.display()
            )
        })?;

        if count == 0 {
            break;
        }

        hasher.update(&buffer[..count]);
    }

    let actual = digest_hex(hasher.finalize());

    if actual != expected.to_ascii_lowercase() {
        return Err(format!(
            "Boss update asset sha256 mismatch for {}",
            path.display()
        ));
    }

    Ok(())
}

fn download_asset(bootstrap: &Bootstrap, key: &str, destination: &Path) -> Result<PathBuf, String> {
    let asset = required_asset(bootstrap, key)?;

    validate_asset(key, asset)?;

    let mut base = Url::parse(bootstrap.stable.base_url.trim())
        .map_err(|error| format!("invalid Boss stable base_url: {error}"))?;

    if base.scheme() != "https" {
        return Err("Boss stable base_url must use https".to_string());
    }

    if !base.path().ends_with('/') {
        let updated = base.path().to_string() + "/";

        base.set_path(&updated);
    }

    let url = base
        .join(asset.file.trim())
        .map_err(|error| format!("could not compose Boss asset URL for '{key}': {error}"))?;

    let target = destination.join(asset.file.trim());

    let output = File::create(&target).map_err(|error| {
        format!(
            "could not create Boss update asset {}: {error}",
            target.display()
        )
    })?;

    let arguments = [
        OsString::from("-fsSL"),
        OsString::from("--max-time"),
        OsString::from("300"),
        OsString::from(url.as_str()),
    ];

    let status = crate::network_boundary::command("boss.curl", &arguments)?
        .stdout(Stdio::from(output))
        .status()
        .map_err(|error| format!("could not download Boss asset '{key}': {error}"))?;

    if !status.success() {
        return Err(format!(
            "Boss asset '{key}' download failed with status {status}"
        ));
    }

    verify_sha256(&target, asset.sha256.trim())?;

    Ok(target)
}

fn parse_critical_update_manifest(path: &Path) -> Result<Vec<String>, String> {
    let payload = fs::read(path).map_err(|error| {
        format!(
            "could not read Boss Critical Update manifest {}: {error}",
            path.display()
        )
    })?;

    if payload.is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_slice::<Vec<String>>(&payload).map_err(|error| {
        format!("Boss Critical Update manifest must be an array of strings: {error}")
    })
}

fn extract_runtime_archive(archive_path: &Path, destination: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("could not create Boss runtime staging directory: {error}"))?;

    let file = File::open(archive_path)
        .map_err(|error| format!("could not open Boss runtime archive: {error}"))?;

    let mut archive = Archive::new(GzDecoder::new(file));

    let entries = archive
        .entries()
        .map_err(|error| format!("could not read Boss runtime archive: {error}"))?;

    let mut manifest_seen = false;

    let mut root_seen = false;

    for entry in entries {
        let mut entry =
            entry.map_err(|error| format!("could not read Boss runtime archive entry: {error}"))?;

        let path = entry
            .path()
            .map_err(|error| format!("invalid Boss runtime archive path: {error}"))?
            .into_owned();

        if path == Path::new("domestic-runtime.json") {
            manifest_seen = true;
        }

        if path == Path::new("rootfs") || path.starts_with("rootfs") {
            root_seen = true;
        }

        let unpacked = entry
            .unpack_in(destination)
            .map_err(|error| format!("could not extract Boss runtime archive entry: {error}"))?;

        if !unpacked {
            return Err("Boss runtime archive attempted to escape staging".to_string());
        }
    }

    if !manifest_seen {
        return Err("Boss runtime archive does not contain domestic-runtime.json".to_string());
    }

    if !root_seen {
        return Err("Boss runtime archive does not contain rootfs".to_string());
    }

    let manifest = destination.join("domestic-runtime.json");

    if !manifest.is_file() {
        return Err("Boss runtime manifest was not materialized".to_string());
    }

    Ok(manifest)
}

fn make_executable(path: &Path) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("could not inspect executable {}: {error}", path.display()))?;

    let mut permissions = metadata.permissions();

    permissions.set_mode(0o755);

    fs::set_permissions(path, permissions)
        .map_err(|error| format!("could not make {} executable: {error}", path.display()))
}

fn execute_critical_update_instructions(instructions: &[String]) -> Result<usize, String> {
    if instructions.is_empty() {
        return Ok(0);
    }

    let nightmare = crate::critical_update::NightmareConsumer::load()?;

    crate::critical_update::execute_instructions(instructions, &[&nightmare])
}

pub fn execute_update_json() -> Result<Value, String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("Boss Critical Update requires administrative authorization".to_string());
    }

    let bootstrap = fetch_bootstrap()?;

    if !update_is_available(&bootstrap)? {
        return Err("Boss Critical Update requested but no newer release is available".to_string());
    }

    let staging = TempDir::new()
        .map_err(|error| format!("could not create Boss Critical Update staging: {error}"))?;

    let backend = download_asset(&bootstrap, "backend", staging.path())?;

    let ui = download_asset(&bootstrap, "ui", staging.path())?;

    let auth_agent = download_asset(&bootstrap, "auth_agent", staging.path())?;

    let client_data = download_asset(&bootstrap, "client_data", staging.path())?;

    let install = download_asset(&bootstrap, "install", staging.path())?;

    let runtime_resolver = download_asset(&bootstrap, "runtime_resolver", staging.path())?;

    let runtime_archive = download_asset(&bootstrap, "runtime_archive", staging.path())?;

    let critical_update_manifest = download_asset(&bootstrap, "critical_update", staging.path())?;

    let critical_update_instructions = parse_critical_update_manifest(&critical_update_manifest)?;

    make_executable(&install)?;

    make_executable(&runtime_resolver)?;

    let runtime_manifest =
        extract_runtime_archive(&runtime_archive, &staging.path().join("runtime"))?;

    let authority_supply =
        neebles_backend::domestic_authority_supply_process::authority_supply_process_state()?
            .supply_path();

    if !authority_supply.is_file() {
        return Err("Boss Critical Update supplied AuthoritySupply is unavailable".to_string());
    }

    let status = std::process::Command::new(&install)
        .arg(&backend)
        .arg(&ui)
        .arg(&client_data)
        .arg(&auth_agent)
        .arg(&runtime_resolver)
        .arg(&runtime_manifest)
        .arg(authority_supply)
        .status()
        .map_err(|error| format!("could not execute Boss Critical Update installer: {error}"))?;

    if !status.success() {
        return Err(format!(
            "Boss Critical Update installer failed with status {status}"
        ));
    }

    let critical_update_executed =
        execute_critical_update_instructions(&critical_update_instructions)?;

    Ok(json!({
        "updated": true,
        "from_version":
            crate::VERSION,
        "to_version":
            bootstrap.stable.version,

        "critical_update_instructions": critical_update_instructions.len(),
        "critical_update_executed": critical_update_executed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bootstrap(version: &str) -> Bootstrap {
        Bootstrap {
            schema: 2,

            channel: "stable".to_string(),

            stable: StableRelease {
                version: version.to_string(),

                base_url: "https://example.invalid/release".to_string(),

                assets: BTreeMap::new(),
            },
        }
    }

    #[test]
    fn current_version_is_valid_semver() {
        assert!(Version::parse(crate::VERSION).is_ok());
    }

    #[test]
    fn newer_version_is_available() {
        assert!(update_is_available(&test_bootstrap("999.0.0")).unwrap());
    }

    #[test]
    fn same_version_is_not_available() {
        assert!(!update_is_available(&test_bootstrap(crate::VERSION)).unwrap());
    }

    #[test]
    fn bootstrap_rejects_non_https_base() {
        let mut value = test_bootstrap(crate::VERSION);

        value.stable.base_url = "http://example.invalid/release".to_string();

        assert!(validate_bootstrap(&value).is_err());
    }

    #[test]
    fn asset_rejects_path_escape() {
        let asset = ReleaseAsset {
            file: "../evil".to_string(),

            sha256: "0".repeat(64),
        };

        assert!(validate_asset("evil", &asset).is_err());
    }

    #[test]
    fn asset_rejects_invalid_digest() {
        let asset = ReleaseAsset {
            file: "file".to_string(),

            sha256: "bad".to_string(),
        };

        assert!(validate_asset("file", &asset).is_err());
    }

    #[test]
    fn critical_update_manifest_accepts_empty_file() {
        let directory = tempfile::tempdir().unwrap();

        let path = directory.path().join("manifest.json");

        std::fs::write(&path, []).unwrap();

        let instructions = parse_critical_update_manifest(&path).unwrap();

        assert!(instructions.is_empty());
    }

    #[test]
    fn critical_update_manifest_preserves_opaque_strings() {
        let directory = tempfile::tempdir().unwrap();

        let path = directory.path().join("manifest.json");

        std::fs::write(&path, br#"["orden.uno","orden.dos"]"#).unwrap();

        let instructions = parse_critical_update_manifest(&path).unwrap();

        assert_eq!(
            instructions,
            vec!["orden.uno".to_string(), "orden.dos".to_string(),]
        );
    }

    #[test]
    fn critical_update_manifest_rejects_typed_entries() {
        let directory = tempfile::tempdir().unwrap();

        let path = directory.path().join("manifest.json");

        std::fs::write(&path, br#"[{"nightmare":"x"}]"#).unwrap();

        assert!(parse_critical_update_manifest(&path).is_err());
    }

    #[test]
    fn digest_hex_is_exact() {
        assert_eq!(digest_hex([0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    }
}

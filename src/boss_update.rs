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

// The update executor is privileged and must never take a home path from the UI.
// The only desktop identity accepted is the previously root-owned Boss runtime identity.
fn publish_verified_boss_update_receipt(
    from_version: &str,
    to_version: &str,
) -> Result<(), String> {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;

    let identity_path = Path::new("/etc/neebles/runtime.env");
    let identity_meta = fs::symlink_metadata(identity_path)
        .map_err(|e| format!("Boss receipt runtime identity metadata: {e}"))?;
    if !identity_meta.file_type().is_file()
        || identity_meta.uid() != 0
        || identity_meta.mode() & 0o022 != 0
    {
        return Err("Boss receipt refused untrusted runtime identity".to_owned());
    }
    let identity = fs::read_to_string(identity_path)
        .map_err(|e| format!("Boss receipt runtime identity: {e}"))?;
    let unique_decimal = |key: &str| -> Result<u32, String> {
        let prefix = format!("{key}=");
        let values: Vec<_> = identity
            .lines()
            .filter_map(|l| l.strip_prefix(&prefix))
            .collect();
        if values.len() != 1
            || values[0].is_empty()
            || !values[0].bytes().all(|b| b.is_ascii_digit())
        {
            return Err(format!(
                "Boss receipt identity {key} must occur once as decimal"
            ));
        }
        values[0]
            .parse::<u32>()
            .map_err(|e| format!("Boss receipt {key}: {e}"))
    };
    let uid = unique_decimal("NEEBLES_DESKTOP_UID")?;
    let gid = unique_decimal("NEEBLES_DESKTOP_GID")?;
    if uid == 0 {
        return Err("Boss receipt cannot target root desktop".to_owned());
    }

    // Resolve the canonical home through the system user database, not environment variables.
    let passwd = unsafe { libc::getpwuid(uid) };
    if passwd.is_null() {
        return Err("Boss receipt desktop account missing".to_owned());
    }
    let account = unsafe { &*passwd };
    if account.pw_gid != gid || account.pw_dir.is_null() {
        return Err("Boss receipt desktop account identity mismatch".to_owned());
    }
    let home = unsafe { std::ffi::CStr::from_ptr(account.pw_dir) }.to_bytes();
    let home = PathBuf::from(std::ffi::OsStr::from_bytes(home));
    if !home.is_absolute() || home == Path::new("/") {
        return Err("Boss receipt unsafe desktop home".to_owned());
    }
    let owned_dir = |path: &Path| -> Result<(), String> {
        let meta = fs::symlink_metadata(path)
            .map_err(|e| format!("Boss receipt directory {}: {e}", path.display()))?;
        if !meta.file_type().is_dir() || meta.uid() != uid || meta.mode() & 0o022 != 0 {
            return Err(format!(
                "Boss receipt directory untrusted: {}",
                path.display()
            ));
        }
        Ok(())
    };
    owned_dir(&home)?;
    let config = home.join(".config");
    owned_dir(&config)?;
    let neebles = config.join("neebles");
    if !neebles.exists() {
        fs::create_dir(&neebles).map_err(|e| format!("Boss receipt create directory: {e}"))?;
        let c_path = std::ffi::CString::new(neebles.as_os_str().as_bytes())
            .map_err(|_| "Boss receipt invalid config path".to_owned())?;
        if unsafe { libc::chown(c_path.as_ptr(), uid, gid) } != 0 {
            return Err(format!(
                "Boss receipt chown directory: {}",
                std::io::Error::last_os_error()
            ));
        }
        fs::set_permissions(&neebles, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("Boss receipt chmod directory: {e}"))?;
    }
    owned_dir(&neebles)?;
    let destination = neebles.join("boss-update-pending.json");
    if fs::symlink_metadata(&destination).is_ok() {
        let meta = fs::symlink_metadata(&destination)
            .map_err(|e| format!("Boss receipt existing record: {e}"))?;
        if !meta.file_type().is_file() || meta.uid() != uid {
            return Err("Boss receipt existing record is not trusted".to_owned());
        }
    }
    let payload = serde_json::to_vec(&json!({
        "schema": 1,
        "target": "boss",
        "from_version": from_version,
        "to_version": to_version,
    }))
    .map_err(|e| format!("Boss receipt JSON serialization: {e}"))?;
    let mut staged = tempfile::NamedTempFile::new_in(&neebles)
        .map_err(|e| format!("Boss receipt staging: {e}"))?;
    staged
        .write_all(&payload)
        .map_err(|e| format!("Boss receipt write: {e}"))?;
    staged
        .as_file()
        .sync_all()
        .map_err(|e| format!("Boss receipt sync: {e}"))?;
    if unsafe { libc::fchown(staged.as_file().as_raw_fd(), uid, gid) } != 0 {
        return Err(format!(
            "Boss receipt fchown: {}",
            std::io::Error::last_os_error()
        ));
    }
    staged
        .persist(&destination)
        .map_err(|e| format!("Boss receipt atomic publish: {}", e.error))?;
    Ok(())
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

    // Confirm the installed product, not merely the installer's exit status.
    // The path is the authenticated Boss entrypoint created by install.sh.
    let installed_output = std::process::Command::new("/opt/neebles/client/bin/neebles")
        .arg("--version")
        .output()
        .map_err(|error| {
            format!("could not inspect installed Boss version after Critical Update: {error}")
        })?;

    if !installed_output.status.success() {
        return Err(format!(
            "installed Boss version command failed after Critical Update: {}",
            installed_output.status
        ));
    }

    let installed_stdout = String::from_utf8(installed_output.stdout)
        .map_err(|error| format!("installed Boss version output is not UTF-8: {error}"))?;

    let installed_version = installed_stdout
        .split_whitespace()
        .last()
        .ok_or_else(|| "installed Boss version command returned empty output".to_string())?;

    if installed_version != bootstrap.stable.version.as_str() {
        return Err(format!(
            "Boss Critical Update version mismatch: expected {}, installed {}",
            bootstrap.stable.version, installed_version
        ));
    }

    // Publish only after installer, Critical Update instructions and installed
    // version verification all succeeded. Older UI does not need to know this format.
    publish_verified_boss_update_receipt(crate::VERSION, &bootstrap.stable.version)?;

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

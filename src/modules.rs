use crate::config;
use crate::dependencies::{self, DependencySet};
use crate::languages;
use crate::privileges;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MODULE_SCHEMA_VERSION: u32 = 3;
pub const MODULE_LANGUAGE_SCHEMA_VERSION: u32 = 1;
pub const MODULE_NOTIFICATIONS_PROTOCOL_VERSION: u32 = 1;
pub const REGISTRY_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommandLifecycle {
    #[default]
    Oneshot,
    Tracked,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandContract {
    #[serde(default)]
    pub requires_root: bool,

    #[serde(default)]
    pub lifecycle: CommandLifecycle,

    #[serde(default)]
    pub launcher: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayContract {
    #[serde(default = "default_tray_protocol")]
    pub protocol: u32,

    pub icon: String,

    pub provider: String,
}

fn default_tray_protocol() -> u32 {
    crate::tray::protocol::TRAY_PROTOCOL_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationContract {
    #[serde(default = "default_notifications_protocol")]
    pub protocol: u32,
}

fn default_notifications_protocol() -> u32 {
    MODULE_NOTIFICATIONS_PROTOCOL_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    #[serde(default = "default_schema")]
    pub schema: u32,
    pub name: String,
    #[serde(default)]
    pub version: String,
    pub entrypoint: String,
    #[serde(default)]
    pub commands: BTreeMap<String, CommandContract>,
    #[serde(default)]
    pub dependencies: DependencySet,

    #[serde(default)]
    pub tray: Option<TrayContract>,

    #[serde(default)]
    pub notifications: Option<NotificationContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleLanguageEntry {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleLanguageManifest {
    pub schema: u32,

    pub default: String,

    pub languages: Vec<ModuleLanguageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryModule {
    pub repo: String,

    pub commit: String,

    #[serde(default)]
    pub version: Option<String>,

    #[serde(default)]
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default = "default_schema")]
    pub schema: u32,
    #[serde(default)]
    pub modules: BTreeMap<String, RegistryModule>,
}

fn default_schema() -> u32 {
    1
}

fn valid_git_commit(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn validate_registry(registry: &Registry) -> Result<(), String> {
    if registry.schema != REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported N.E.E.B.L.E.S. registry schema {}; expected {}",
            registry.schema, REGISTRY_SCHEMA_VERSION
        ));
    }

    for (name, module) in &registry.modules {
        if !valid_module_id(name) {
            return Err(format!("invalid module id in registry: {name}"));
        }

        if module.repo.trim().is_empty() {
            return Err(format!("module '{}' registry repo cannot be empty", name));
        }

        if !valid_git_commit(module.commit.trim()) {
            return Err(format!(
                "module '{}' registry commit '{}' is not a full 40-character Git commit SHA",
                name, module.commit
            ));
        }

        if let Some(version) = module.version.as_deref() {
            let version = version.trim();

            if version.is_empty() {
                return Err(format!(
                    "module '{}' registry version cannot be empty",
                    name
                ));
            }

            Version::parse(version).map_err(|error| {
                format!(
                    "module '{}' registry version '{}' is not valid SemVer: {error}",
                    name, version
                )
            })?;
        }

        if let Some(folder) = module.folder.as_deref() {
            if !valid_module_id(folder) {
                return Err(format!(
                    "invalid module folder '{}' in registry for '{}'",
                    folder, name
                ));
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct ResolvedTrayContract {
    pub protocol: u32,
    pub module_version: String,
    pub icon: String,
    pub provider: String,
}

fn resolve_module_contract_path(
    module_dir: &Path,
    value: &str,
    field: &str,
) -> Result<PathBuf, String> {
    let value = value.trim();

    if value.is_empty() {
        return Err(format!("module tray {field} cannot be empty"));
    }

    let relative = Path::new(value);

    if relative.is_absolute() {
        return Err(format!(
            "module tray {field} must be relative to the module directory"
        ));
    }

    for component in relative.components() {
        use std::path::Component;

        match component {
            Component::Normal(_) | Component::CurDir => {}

            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "module tray {field} contains an invalid path: {value}"
                ));
            }
        }
    }

    let candidate = module_dir.join(relative);

    let canonical = candidate.canonicalize().map_err(|error| {
        format!(
            "could not resolve module tray {field} {}: {error}",
            candidate.display()
        )
    })?;

    let canonical_module = module_dir.canonicalize().map_err(|error| {
        format!(
            "could not resolve module directory {}: {error}",
            module_dir.display()
        )
    })?;

    if !canonical.starts_with(&canonical_module) {
        return Err(format!("module tray {field} escapes module directory"));
    }

    if !canonical.is_file() {
        return Err(format!(
            "module tray {field} is not a file: {}",
            canonical.display()
        ));
    }

    Ok(canonical)
}

pub fn installed_module_manifest(name: &str) -> Result<ModuleManifest, String> {
    if !valid_module_id(name) {
        return Err(format!("invalid module id: {name}"));
    }

    let module_dir = find_module_dir(name)?;

    read_manifest(&module_dir.join("manifest.json"))
}

fn resolve_tray_contract_from(
    module_dir: &Path,
    manifest: &ModuleManifest,
) -> Result<ResolvedTrayContract, String> {
    let tray = manifest.tray.as_ref().ok_or_else(|| {
        format!(
            "module '{}' does not declare a tray capability",
            manifest.name
        )
    })?;

    if tray.protocol != crate::tray::protocol::TRAY_PROTOCOL_VERSION {
        return Err(format!(
            "module '{}' declares unsupported tray protocol {}; expected {}",
            manifest.name,
            tray.protocol,
            crate::tray::protocol::TRAY_PROTOCOL_VERSION
        ));
    }

    let icon = resolve_module_contract_path(module_dir, &tray.icon, "icon")?;

    let provider = resolve_module_contract_path(module_dir, &tray.provider, "provider")?;

    let metadata = fs::metadata(&provider).map_err(|error| {
        format!(
            "could not inspect module '{}' tray provider {}: {error}",
            manifest.name,
            provider.display()
        )
    })?;

    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!(
            "module '{}' tray provider is not executable: {}",
            manifest.name,
            provider.display()
        ));
    }

    Ok(ResolvedTrayContract {
        protocol: tray.protocol,
        module_version: manifest.version.clone(),
        icon: icon.display().to_string(),
        provider: provider.display().to_string(),
    })
}

pub fn resolved_tray_contract(name: &str) -> Result<ResolvedTrayContract, String> {
    let module_dir = find_module_dir(name)?;

    let manifest = installed_module_manifest(name)?;

    resolve_tray_contract_from(&module_dir, &manifest)
}

const MODULE_ICON_EXTENSIONS: &[&str] = &["svg", "png", "webp", "jpg", "jpeg"];

static TRANSACTION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn transaction_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();

    let counter = TRANSACTION_COUNTER.fetch_add(1, Ordering::Relaxed);

    format!("{}-{}-{}", std::process::id(), timestamp, counter)
}

struct ModuleInstallStaging {
    path: PathBuf,
    committed: bool,
}

impl ModuleInstallStaging {
    fn prepare(path: PathBuf) -> Result<Self, String> {
        if path.exists() {
            return Err(format!(
                "module staging path already exists and will not be removed automatically: {}",
                path.display()
            ));
        }

        Ok(Self {
            path,
            committed: false,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn commit(mut self, destination: &Path) -> Result<(), String> {
        fs::rename(&self.path, destination).map_err(|error| {
            format!(
                "could not move module staging directory {} into {}: {error}",
                self.path.display(),
                destination.display()
            )
        })?;

        self.committed = true;

        Ok(())
    }
}

impl Drop for ModuleInstallStaging {
    fn drop(&mut self) {
        if self.committed || !self.path.exists() {
            return;
        }

        if let Err(error) = fs::remove_dir_all(&self.path) {
            eprintln!(
                "N.E.E.B.L.E.S.: could not clean module staging directory {}: {error}",
                self.path.display()
            );
        }
    }
}

fn checkout_registry_commit(
    name: &str,
    entry: &RegistryModule,
    destination: &Path,
) -> Result<(), String> {
    if !dependencies::command_exists("git") {
        return Err("git is required by Boss to retrieve modules".to_string());
    }

    let expected = entry.commit.trim().to_ascii_lowercase();

    if !valid_git_commit(&expected) {
        return Err(format!(
            "module '{}' has invalid registry commit '{}'",
            name, entry.commit
        ));
    }

    let status = Command::new("git")
        .arg("init")
        .arg(destination)
        .status()
        .map_err(|error| {
            format!(
                "could not initialize staged repository for module '{}': {error}",
                name
            )
        })?;

    if !status.success() {
        return Err(format!(
            "git init failed while staging module '{}' with status {}",
            name, status
        ));
    }

    let status = Command::new("git")
        .arg("-C")
        .arg(destination)
        .args(["remote", "add", "origin"])
        .arg(&entry.repo)
        .status()
        .map_err(|error| format!("could not configure module '{}' repository: {error}", name))?;

    if !status.success() {
        return Err(format!(
            "git remote add failed while staging module '{}' with status {}",
            name, status
        ));
    }

    /*
     * Fetch the immutable object named by the registry.
     * We intentionally do not resolve or trust branch tip.
     */
    let status = Command::new("git")
        .arg("-C")
        .arg(destination)
        .args(["fetch", "--depth", "1", "origin"])
        .arg(&expected)
        .status()
        .map_err(|error| {
            format!(
                "could not fetch pinned commit for module '{}': {error}",
                name
            )
        })?;

    if !status.success() {
        return Err(format!(
            "git fetch failed for pinned module '{}' commit {} with status {}",
            name, expected, status
        ));
    }

    let status = Command::new("git")
        .arg("-C")
        .arg(destination)
        .args(["checkout", "--detach", "FETCH_HEAD"])
        .status()
        .map_err(|error| {
            format!(
                "could not checkout pinned module '{}' commit: {error}",
                name
            )
        })?;

    if !status.success() {
        return Err(format!(
            "git checkout failed for pinned module '{}' commit {} with status {}",
            name, expected, status
        ));
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(destination)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|error| {
            format!(
                "could not verify checked out commit for module '{}': {error}",
                name
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "git rev-parse failed while verifying module '{}'",
            name
        ));
    }

    let actual = String::from_utf8(output.stdout)
        .map_err(|error| {
            format!(
                "invalid git rev-parse output for module '{}': {error}",
                name
            )
        })?
        .trim()
        .to_ascii_lowercase();

    if actual != expected {
        return Err(format!(
            "module '{}' integrity failure: registry requires commit {}, but staged repository is {}",
            name,
            expected,
            actual
        ));
    }

    Ok(())
}

fn local_module_icon(module_dir: &Path) -> String {
    for extension in MODULE_ICON_EXTENSIONS {
        let candidate = module_dir.join(format!("icon.{extension}"));

        if candidate.is_file() {
            if let Ok(path) = candidate.canonicalize() {
                return format!("file://{}", path.display());
            }
        }
    }

    String::new()
}

fn github_raw_base(repo: &str, revision: &str) -> Option<String> {
    let repo = repo
        .strip_prefix("https://github.com/")?
        .trim_end_matches(".git")
        .trim_end_matches('/');

    Some(format!(
        "https://raw.githubusercontent.com/{repo}/{revision}"
    ))
}

fn remote_module_icon(repo: &str, commit: &str) -> String {
    let Some(base) = github_raw_base(repo, commit) else {
        return String::new();
    };

    for extension in MODULE_ICON_EXTENSIONS {
        let url = format!("{base}/icon.{extension}");

        let status = Command::new("curl")
            .args(["-fsIL", "--max-time", "5", url.as_str()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        if matches!(status, Ok(value) if value.success()) {
            return url;
        }
    }

    String::new()
}

pub fn neebles_root() -> PathBuf {
    env::var("NEEBLES_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt/neebles"))
}

pub fn modules_root() -> PathBuf {
    neebles_root().join("modules")
}

fn runtime_identity() -> String {
    /*
     * Privileged re-execution must keep operating on the
     * runtime state of the original desktop user.
     */
    if let Ok(value) = env::var("NEEBLES_RUNTIME_IDENTITY") {
        let value = value.trim();

        if !value.is_empty() && value.chars().all(|character| character.is_ascii_digit()) {
            return value.to_string();
        }
    }

    /*
     * Normal desktop session:
     * XDG_RUNTIME_DIR is normally /run/user/<uid>.
     */
    if let Ok(value) = env::var("XDG_RUNTIME_DIR") {
        if let Some(name) = Path::new(&value)
            .file_name()
            .and_then(|value| value.to_str())
        {
            if !name.is_empty() && name.chars().all(|character| character.is_ascii_digit()) {
                return name.to_string();
            }
        }
    }

    /*
     * Final deterministic fallback.
     * Never use USER: sudo changes USER to root while the
     * runtime ownership we care about is UID-based.
     */
    unsafe { libc::geteuid() }.to_string()
}

fn runtime_modules_root() -> PathBuf {
    env::temp_dir()
        .join(format!("neebles-runtime-{}", runtime_identity()))
        .join("modules")
}

fn runtime_marker_path(name: &str) -> PathBuf {
    runtime_modules_root().join(format!("{name}.pid"))
}

fn clear_runtime_marker(name: &str) {
    let _ = fs::remove_file(runtime_marker_path(name));
}

fn clear_runtime_marker_if_pid(name: &str, pid: u32) {
    let path = runtime_marker_path(name);

    let matches = fs::read_to_string(&path)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        == Some(pid);

    if matches {
        let _ = fs::remove_file(path);
    }
}

fn write_runtime_marker(name: &str, pid: u32) -> Result<(), String> {
    let root = runtime_modules_root();

    fs::create_dir_all(&root).map_err(|error| {
        format!(
            "could not create module runtime directory {}: {error}",
            root.display()
        )
    })?;

    let path = runtime_marker_path(name);

    fs::write(&path, format!("{pid}\n")).map_err(|error| {
        format!(
            "could not write module runtime marker {}: {error}",
            path.display()
        )
    })
}

fn probe_module_pid(name: &str) -> Result<Option<u32>, String> {
    let path = runtime_marker_path(name);

    let raw = match fs::read_to_string(&path) {
        Ok(value) => value,

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }

        Err(error) => {
            return Err(format!(
                "could not read module runtime marker {}: {error}",
                path.display()
            ));
        }
    };

    let pid = raw.trim().parse::<u32>().map_err(|error| {
        format!(
            "module '{}' has an invalid runtime marker {}: {error}",
            name,
            path.display()
        )
    })?;

    let process_path = PathBuf::from(format!("/proc/{pid}"));

    if !process_path.exists() {
        clear_runtime_marker_if_pid(name, pid);

        return Ok(None);
    }

    /*
     * From here onward a live PID exists.
     *
     * Failure to prove ownership must NEVER be converted
     * into "not running". That would allow destructive
     * operations to proceed while an unknown process may
     * still be using the installed module.
     */

    let module_dir = find_module_dir(name).map_err(|error| {
        format!(
            "module '{}' runtime process {} exists, but its installation cannot be resolved: {}",
            name, pid, error
        )
    })?;

    let manifest = read_manifest(&module_dir.join("manifest.json")).map_err(|error| {
        format!(
            "module '{}' runtime process {} exists, but its manifest cannot be verified: {}",
            name, pid, error
        )
    })?;

    let expected = resolve_entrypoint(&module_dir, &manifest.entrypoint).map_err(|error| {
        format!(
            "module '{}' runtime process {} exists, but its entrypoint cannot be verified: {}",
            name, pid, error
        )
    })?;

    /*
     * Native executable.
     */
    let proc_exe = PathBuf::from(format!("/proc/{pid}/exe"));

    if let Ok(actual_exe) = fs::read_link(&proc_exe) {
        if let Ok(actual_exe) = actual_exe.canonicalize() {
            if actual_exe == expected {
                return Ok(Some(pid));
            }
        }
    }

    /*
     * Interpreted entrypoint:
     * Bash/Python/Node/etc.
     */
    let cmdline =
        fs::read(
            format!("/proc/{pid}/cmdline")
        )
        .map_err(|error| {
            format!(
                "module '{}' runtime process {} exists, but its command line cannot be inspected: {error}",
                name,
                pid
            )
        })?;

    let expected_bytes = expected.as_os_str().as_bytes();

    let matches_entrypoint = cmdline
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .any(|argument| argument == expected_bytes);

    if matches_entrypoint {
        return Ok(Some(pid));
    }

    /*
     * PID exists, marker exists, but ownership cannot be
     * proven. Preserve the marker and fail closed.
     */
    Err(format!(
        "module '{}' runtime marker points to live process {}, but Boss cannot verify that the process still belongs to the declared entrypoint",
        name,
        pid
    ))
}

pub fn module_running(name: &str) -> bool {
    match probe_module_pid(name) {
        Ok(Some(_)) => true,
        Ok(None) => false,

        /*
         * Unknown runtime state is treated as running in
         * non-destructive views. This prevents the UI from
         * presenting an unsafe false "stopped" state.
         */
        Err(_) => true,
    }
}

fn stop_module(name: &str) -> Result<(), String> {
    let Some(pid) = probe_module_pid(name)? else {
        return Ok(());
    };

    /*
     * Graceful shutdown first.
     */
    let status = Command::new("kill")
        .args(["-TERM", pid.to_string().as_str()])
        .status()
        .map_err(|error| format!("could not stop module '{name}' process {pid}: {error}"))?;

    if !status.success() {
        return Err(format!(
            "could not send SIGTERM to module '{name}' process {pid}"
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

    while probe_module_pid(name)? == Some(pid) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if probe_module_pid(name)? != Some(pid) {
        clear_runtime_marker_if_pid(name, pid);
        return Ok(());
    }

    /*
     * A tracked process that ignores SIGTERM must not
     * block Boss lifecycle forever.
     */
    let status = Command::new("kill")
        .args(["-KILL", pid.to_string().as_str()])
        .status()
        .map_err(|error| format!("could not force-stop module '{name}' process {pid}: {error}"))?;

    if !status.success() {
        return Err(format!(
            "could not send SIGKILL to module '{name}' process {pid}"
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);

    while probe_module_pid(name)? == Some(pid) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if probe_module_pid(name)? == Some(pid) {
        return Err(format!(
            "module '{name}' process {pid} did not terminate after SIGKILL"
        ));
    }

    clear_runtime_marker_if_pid(name, pid);

    Ok(())
}

fn tray_runtime_marker_path(name: &str) -> PathBuf {
    runtime_modules_root().join(format!("{name}.tray.pid"))
}

fn clear_tray_runtime_marker_if_pid(name: &str, pid: u32) {
    let path = tray_runtime_marker_path(name);

    let matches = fs::read_to_string(&path)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        == Some(pid);

    if matches {
        let _ = fs::remove_file(path);
    }
}

fn write_tray_runtime_marker(name: &str, pid: u32) -> Result<(), String> {
    let root = runtime_modules_root();

    fs::create_dir_all(&root).map_err(|error| {
        format!(
            "could not create module runtime directory {}: {error}",
            root.display()
        )
    })?;

    let path = tray_runtime_marker_path(name);

    fs::write(&path, format!("{pid}\n")).map_err(|error| {
        format!(
            "could not write tray provider runtime marker {}: {error}",
            path.display()
        )
    })
}

fn probe_tray_provider_pid(name: &str) -> Result<Option<u32>, String> {
    let path = tray_runtime_marker_path(name);

    let raw = match fs::read_to_string(&path) {
        Ok(value) => value,

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }

        Err(error) => {
            return Err(format!(
                "could not read tray provider runtime marker {}: {error}",
                path.display()
            ));
        }
    };

    let pid = raw.trim().parse::<u32>().map_err(|error| {
        format!(
            "module '{}' has an invalid tray provider runtime marker {}: {error}",
            name,
            path.display()
        )
    })?;

    let process_path = PathBuf::from(format!("/proc/{pid}"));

    if !process_path.exists() {
        clear_tray_runtime_marker_if_pid(name, pid);

        return Ok(None);
    }

    /*
     * A live PID exists.
     *
     * From this point onward every inability to prove
     * provider ownership is an UNKNOWN/UNSAFE runtime
     * state, never "not running".
     */
    let contract =
        resolved_tray_contract(name)
            .map_err(|error| {
                format!(
                    "module '{}' tray provider process {} exists, but its tray contract cannot be verified: {}",
                    name,
                    pid,
                    error
                )
            })?;

    let expected =
        PathBuf::from(
            &contract.provider
        )
        .canonicalize()
        .map_err(|error| {
            format!(
                "module '{}' tray provider process {} exists, but its provider path cannot be canonicalized: {error}",
                name,
                pid
            )
        })?;

    /*
     * Native provider.
     */
    let proc_exe = PathBuf::from(format!("/proc/{pid}/exe"));

    if let Ok(actual_exe) = fs::read_link(&proc_exe) {
        if let Ok(actual_exe) = actual_exe.canonicalize() {
            if actual_exe == expected {
                return Ok(Some(pid));
            }
        }
    }

    /*
     * Interpreted provider:
     * Python/Bash/Node/etc.
     */
    let cmdline =
        fs::read(
            format!("/proc/{pid}/cmdline")
        )
        .map_err(|error| {
            format!(
                "module '{}' tray provider process {} exists, but its command line cannot be inspected: {error}",
                name,
                pid
            )
        })?;

    let expected_bytes = expected.as_os_str().as_bytes();

    let matches_provider = cmdline
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .any(|argument| argument == expected_bytes);

    if matches_provider {
        return Ok(Some(pid));
    }

    /*
     * Preserve the marker. A live PID with unverified
     * ownership requires explicit intervention.
     */
    Err(format!(
        "module '{}' tray provider marker points to live process {}, but Boss cannot verify that the process still belongs to the declared provider",
        name,
        pid
    ))
}

pub fn tray_provider_running(name: &str) -> bool {
    match probe_tray_provider_pid(name) {
        Ok(Some(_)) => true,
        Ok(None) => false,

        /*
         * Unknown provider state is reported as running
         * to avoid presenting an unsafe false "stopped"
         * state.
         */
        Err(_) => true,
    }
}

pub fn verify_tray_provider_process(name: &str, pid: u32) -> Result<(), String> {
    let contract = resolved_tray_contract(name)?;

    let expected = PathBuf::from(&contract.provider)
        .canonicalize()
        .map_err(|error| {
            format!(
                "could not canonicalize tray provider for module '{}': {error}",
                name
            )
        })?;

    /*
     * Native binary:
     *
     * /proc/<pid>/exe points directly to the
     * executable declared by the module.
     */
    let proc_exe = PathBuf::from(format!("/proc/{pid}/exe"));

    if let Ok(actual_exe) = fs::read_link(&proc_exe) {
        if let Ok(actual_exe) = actual_exe.canonicalize() {
            if actual_exe == expected {
                return Ok(());
            }
        }
    }

    /*
     * Interpreted provider:
     *
     * Python, Bash, Node, etc. expose the runtime
     * through /proc/<pid>/exe. The provider script
     * must therefore appear as one exact argv entry.
     */
    let cmdline = fs::read(format!("/proc/{pid}/cmdline")).map_err(|error| {
        format!(
            "could not inspect tray provider process {} for module '{}': {error}",
            pid, name
        )
    })?;

    let expected_bytes = expected.as_os_str().as_bytes();

    let matches_provider = cmdline
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .any(|argument| argument == expected_bytes);

    if matches_provider {
        return Ok(());
    }

    Err(format!(
        "process {} is not the tray provider declared by module '{}'",
        pid, name
    ))
}

pub fn start_tray_provider(name: &str) -> Result<bool, String> {
    if !config::module_enabled(name)? {
        return Ok(false);
    }

    let module_dir = find_module_dir(name)?;

    let manifest = read_manifest(&module_dir.join("manifest.json"))?;

    /*
     * No tray capability means there is no tray
     * lifecycle to manage. This is not an error.
     */
    if manifest.tray.is_none() {
        return Ok(false);
    }

    if tray_provider_running(name) {
        return Ok(false);
    }

    let socket_path = crate::tray::protocol::socket_path();

    /*
     * The provider belongs to the Tray Manager
     * lifecycle. If no manager socket exists yet,
     * enabling/installing the module remains valid;
     * the provider will be started when tray serve
     * comes online.
     */
    if !socket_path.exists() {
        return Ok(false);
    }

    let contract = resolved_tray_contract(name)?;

    let provider = PathBuf::from(&contract.provider);

    let metadata = fs::metadata(&provider).map_err(|error| {
        format!(
            "could not inspect tray provider {}: {error}",
            provider.display()
        )
    })?;

    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!(
            "module '{}' tray provider is not executable: {}",
            name,
            provider.display()
        ));
    }

    let config_path = config::config_path()?;

    let requested_language = config::load_or_initialize()?.language;

    let language = resolve_module_language(&module_dir, &requested_language)?;

    let mut command = Command::new(&provider);

    command
        .current_dir(&module_dir)
        .env("NEEBLES_LANGUAGE", language)
        .env("NEEBLES_CALLER", "tray-manager")
        .env("NEEBLES_MODULE", name)
        .env("NEEBLES_CONFIG", config_path)
        .env("NEEBLES_TRAY_SOCKET", socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = command.spawn().map_err(|error| {
        format!(
            "could not launch tray provider for module '{}': {error}",
            name
        )
    })?;

    let pid = child.id();

    if let Err(error) = write_tray_runtime_marker(name, pid) {
        let _ = child.kill();
        let _ = child.wait();

        return Err(error);
    }

    let owned_name = name.to_string();

    /*
     * Reap the provider when it exits and remove
     * only the marker belonging to this exact PID.
     * This avoids zombies and avoids an old provider
     * deleting a marker belonging to a newer one.
     */
    std::thread::spawn(move || {
        let _ = child.wait();

        clear_tray_runtime_marker_if_pid(&owned_name, pid);
    });

    Ok(true)
}

pub fn stop_tray_provider(name: &str) -> Result<bool, String> {
    let Some(pid) = probe_tray_provider_pid(name)? else {
        return Ok(false);
    };

    let status = Command::new("kill")
        .args(["-TERM", pid.to_string().as_str()])
        .status()
        .map_err(|error| {
            format!(
                "could not stop tray provider for module '{}' process {}: {error}",
                name, pid
            )
        })?;

    if !status.success() {
        return Err(format!(
            "could not send SIGTERM to tray provider for module '{}' process {}",
            name, pid
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

    while probe_tray_provider_pid(name)? == Some(pid) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if probe_tray_provider_pid(name)? != Some(pid) {
        clear_tray_runtime_marker_if_pid(name, pid);
        return Ok(true);
    }

    /*
     * A stale or broken provider must not permanently
     * block reconciliation.
     */
    let status = Command::new("kill")
        .args(["-KILL", pid.to_string().as_str()])
        .status()
        .map_err(|error| {
            format!(
                "could not force-stop tray provider for module '{}' process {}: {error}",
                name, pid
            )
        })?;

    if !status.success() {
        return Err(format!(
            "could not send SIGKILL to tray provider for module '{}' process {}",
            name, pid
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);

    while probe_tray_provider_pid(name)? == Some(pid) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if probe_tray_provider_pid(name)? == Some(pid) {
        return Err(format!(
            "tray provider for module '{}' process {} did not terminate after SIGKILL",
            name, pid
        ));
    }

    clear_tray_runtime_marker_if_pid(name, pid);

    Ok(true)
}

pub fn start_enabled_tray_providers() -> Result<(), String> {
    let root = modules_root();

    if !root.exists() {
        return Ok(());
    }

    let mut errors = Vec::new();

    for entry in fs::read_dir(&root)
        .map_err(|error| format!("could not read {}: {error}", root.display()))?
    {
        let entry =
            entry.map_err(|error| format!("could not read module directory entry: {error}"))?;

        if !entry.path().is_dir() {
            continue;
        }

        let manifest_path = entry.path().join("manifest.json");

        if !manifest_path.exists() {
            continue;
        }

        let manifest = match read_manifest(&manifest_path) {
            Ok(manifest) => manifest,

            Err(error) => {
                errors.push(error);
                continue;
            }
        };

        if manifest.tray.is_none() {
            continue;
        }

        match config::module_enabled(&manifest.name) {
            Ok(true) => {}

            Ok(false) => continue,

            Err(error) => {
                errors.push(error);
                continue;
            }
        }

        if let Err(error) = start_tray_provider(&manifest.name) {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "one or more tray providers could not be started: {}",
            errors.join(" | ")
        ))
    }
}

fn registry_url() -> Result<String, String> {
    /*
     * Explicit override is respected exactly.
     * Useful for development/testing and alternate registries.
     */
    if let Ok(value) = env::var("NEEBLES_MODULES_REGISTRY") {
        if !value.trim().is_empty() {
            return Ok(value);
        }
    }

    /*
     * Never read the production registry through raw/main.
     *
     * raw.githubusercontent.com may keep the branch URL cached
     * for several minutes after main has advanced.
     *
     * Boss first resolves the exact main commit and then reads
     * registry/modules.json from that immutable commit SHA.
     */
    if !dependencies::command_exists("git") {
        return Err(
            "git is required by Boss to resolve the current module registry revision".to_string(),
        );
    }

    let output = Command::new("git")
        .args([
            "ls-remote",
            "https://github.com/krockzs/neebles-boss.git",
            "refs/heads/main",
        ])
        .output()
        .map_err(|error| format!("could not resolve N.E.E.B.L.E.S. registry revision: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "could not resolve N.E.E.B.L.E.S. registry revision: {}",
            output.status
        ));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        format!("invalid git ls-remote output while resolving registry: {error}")
    })?;

    let commit = stdout.split_whitespace().next().ok_or_else(|| {
        "git ls-remote returned no revision for N.E.E.B.L.E.S. Boss main".to_string()
    })?;

    if commit.len() != 40
        || !commit
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(format!(
            "invalid N.E.E.B.L.E.S. registry revision returned by git: {commit}"
        ));
    }

    Ok(format!(
        "https://raw.githubusercontent.com/krockzs/neebles-boss/{commit}/registry/modules.json"
    ))
}

pub fn fetch_registry() -> Result<Registry, String> {
    if !dependencies::command_exists("curl") {
        return Err("curl is required by Boss to read the remote module registry".to_string());
    }

    let url = registry_url()?;
    let output = Command::new("curl")
        .args(["-fsSL", url.as_str()])
        .output()
        .map_err(|error| format!("could not start curl: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "could not read N.E.E.B.L.E.S. module registry: {}",
            output.status
        ));
    }

    let registry: Registry = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid N.E.E.B.L.E.S. module registry: {error}"))?;

    validate_registry(&registry)?;

    Ok(registry)
}

pub fn installed_modules_json() -> Result<Value, String> {
    let mut result = Vec::new();
    let root = modules_root();
    if !root.exists() {
        return Ok(json!([]));
    }

    for entry in fs::read_dir(&root)
        .map_err(|error| format!("could not read {}: {error}", root.display()))?
    {
        let entry =
            entry.map_err(|error| format!("could not read module directory entry: {error}"))?;
        if !entry.path().is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }
        let manifest = read_manifest(&manifest_path)?;
        let enabled = config::module_enabled(&manifest.name)?;
        let icon = local_module_icon(&entry.path());
        let running = module_running(&manifest.name);

        let launcher_action = launcher_action(&manifest)?;

        let mut item = json!({
            "name": manifest.name,
            "version": manifest.version,
            "enabled": enabled,
            "running": running,
            "path": entry.path(),
            "icon": icon,
            "launcher_action": launcher_action,
            "notifications": manifest.notifications.as_ref().map(|contract| {
                json!({
                    "protocol": contract.protocol,
                })
            }),
        });

        if let Some(tray) = manifest.tray.as_ref() {
            item.as_object_mut()
                .expect("module JSON must be an object")
                .insert(
                    "tray".to_string(),
                    json!({
                        "protocol": tray.protocol,
                        "icon": tray.icon,
                        "provider": tray.provider,
                    }),
                );
        }

        result.push(item);
    }

    result.sort_by(|a, b| {
        a.get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .cmp(b.get("name").and_then(Value::as_str).unwrap_or_default())
    });
    Ok(Value::Array(result))
}

pub fn available_modules_json() -> Result<Value, String> {
    let registry = fetch_registry()?;
    let installed = installed_names()?;
    let mut result = Vec::new();

    for (name, module) in registry.modules {
        let is_installed = installed.contains(&name);

        let icon = remote_module_icon(&module.repo, &module.commit);

        result.push(json!({
            "name": name,
            "version": module.version.unwrap_or_default(),
            "repo": module.repo,
            "icon": icon,
            "installed": is_installed,
        }));
    }

    Ok(Value::Array(result))
}

fn installed_names() -> Result<HashSet<String>, String> {
    let value = installed_modules_json()?;
    Ok(value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("name").and_then(Value::as_str).map(str::to_string))
        .collect())
}

pub fn install(name: &str) -> Result<(), String> {
    let registry = fetch_registry()?;
    let mut visiting = HashSet::new();
    install_internal(name, &registry, &mut visiting)
}

fn ensure_minimum_module_version(name: &str, minimum_version: Option<&str>) -> Result<(), String> {
    let Some(minimum_version) = minimum_version else {
        return Ok(());
    };

    let minimum_version = Version::parse(minimum_version.trim()).map_err(|error| {
        format!(
            "invalid minimum version '{}' requested for module '{}': {error}",
            minimum_version, name
        )
    })?;

    let manifest = installed_module_manifest(name)?;

    let installed_version = Version::parse(manifest.version.trim()).map_err(|error| {
        format!(
            "installed module '{}' has invalid semantic version '{}': {error}",
            name, manifest.version
        )
    })?;

    if installed_version < minimum_version {
        return Err(format!(
            "module '{}' version {} is installed but version {} or newer is required",
            name, installed_version, minimum_version
        ));
    }

    Ok(())
}

fn resolve_module_dependency(
    dependency: &dependencies::ModuleDependency,
    registry: &Registry,
    visiting: &mut HashSet<String>,
) -> Result<(), String> {
    /*
     * Installed dependencies are not blindly accepted:
     * their declared version must still satisfy the contract.
     */
    if find_module_dir(&dependency.name).is_err() {
        if let Err(error) = install_internal(&dependency.name, registry, visiting) {
            /*
             * install_internal removes itself on success.
             * On failure the caller must release the
             * traversal marker as well, especially for an
             * optional dependency whose failure does not
             * abort the whole parent installation.
             */
            visiting.remove(&dependency.name);

            return Err(error);
        }
    }

    ensure_minimum_module_version(&dependency.name, dependency.minimum_version.as_deref())
}

fn install_internal(
    name: &str,
    registry: &Registry,
    visiting: &mut HashSet<String>,
) -> Result<(), String> {
    if find_module_dir(name).is_ok() {
        return Ok(());
    }

    if !visiting.insert(name.to_string()) {
        return Err(format!(
            "circular N.E.E.B.L.E.S. module dependency detected at '{name}'"
        ));
    }

    let entry = registry
        .modules
        .get(name)
        .ok_or_else(|| format!("module '{name}' does not exist in the N.E.E.B.L.E.S. registry"))?;

    if !valid_module_id(name) {
        return Err(format!("invalid module id in registry: {name}"));
    }

    if !dependencies::command_exists("git") {
        return Err("git is required by Boss to install modules from repositories".to_string());
    }

    let temp_root = neebles_root().join("shared/tmp");
    fs::create_dir_all(&temp_root)
        .map_err(|error| format!("could not create {}: {error}", temp_root.display()))?;
    let temp = temp_root.join(format!("install-{name}-{}", transaction_id()));

    let staging = ModuleInstallStaging::prepare(temp)?;

    checkout_registry_commit(name, entry, staging.path())?;

    let manifest_path = staging.path().join("manifest.json");
    let manifest = read_manifest(&manifest_path)?;
    if manifest.name != name {
        return Err(format!(
            "module manifest name '{}' does not match registry id '{name}'",
            manifest.name
        ));
    }

    if let Some(expected_version) = entry.version.as_deref() {
        if !expected_version.is_empty() && manifest.version != expected_version {
            return Err(format!(
                "module '{}' manifest version '{}' does not match registry version '{}'",
                name, manifest.version, expected_version
            ));
        }
    }

    if manifest.tray.is_some() {
        resolve_tray_contract_from(staging.path(), &manifest)?;
    }

    dependencies::resolve_system_dependencies(&manifest.dependencies.system)?;

    for dependency in &manifest.dependencies.modules {
        match resolve_module_dependency(dependency, registry, visiting) {
            Ok(()) => {}

            Err(error) if dependency.required => {
                return Err(format!(
                    "required module dependency '{}' could not be resolved: {error}",
                    dependency.name
                ));
            }

            Err(error) => eprintln!(
                "N.E.E.B.L.E.S.: optional module dependency '{}' could not be resolved: {error}",
                dependency.name
            ),
        }
    }

    let folder = entry.folder.as_deref().unwrap_or(name);
    if !valid_module_id(folder) {
        return Err(format!("invalid module folder in registry: {folder}"));
    }
    let destination = modules_root().join(folder);
    fs::create_dir_all(modules_root())
        .map_err(|error| format!("could not create {}: {error}", modules_root().display()))?;
    if destination.exists() {
        return Err(format!(
            "module destination already exists: {}",
            destination.display()
        ));
    }
    staging.commit(&destination)?;

    visiting.remove(name);

    Ok(())
}

pub fn uninstall(name: &str) -> Result<(), String> {
    if probe_module_pid(name)?.is_some() {
        return Err(format!("module '{name}' is currently running"));
    }

    let path = find_module_dir(name)?;

    fs::remove_dir_all(&path)
        .map_err(|error| format!("could not remove module {}: {error}", path.display()))
}

pub fn update(name: &str, close_running: bool) -> Result<(), String> {
    if probe_module_pid(name)?.is_some() {
        if !close_running {
            return Err(format!("module '{name}' is currently running"));
        }

        /*
         * Authorization has already been granted before
         * this privileged backend process starts.
         *
         * Only now may Boss close the running application.
         */
        stop_module(name)?;

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);

        while probe_module_pid(name)?.is_some() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        if probe_module_pid(name)?.is_some() {
            return Err(format!("module '{name}' did not close in time"));
        }
    }

    let current_path = find_module_dir(name)?;

    let registry = fetch_registry()?;

    let entry = registry
        .modules
        .get(name)
        .ok_or_else(|| format!("module '{name}' does not exist in the N.E.E.B.L.E.S. registry"))?;

    if !dependencies::command_exists("git") {
        return Err("git is required by Boss to update modules from repositories".to_string());
    }

    /*
     * Stage the candidate INSIDE modules_root().
     *
     * That keeps the final rename on the same filesystem,
     * allowing the commit/rollback operation to remain
     * atomic at filesystem rename level.
     */
    let root = modules_root();

    fs::create_dir_all(&root)
        .map_err(|error| format!("could not create module root {}: {error}", root.display()))?;

    let transaction = transaction_id();

    let staging_path = root.join(format!(".neebles-update-{name}-{transaction}"));

    let backup_path = root.join(format!(".neebles-backup-{name}-{transaction}"));

    /*
     * Transaction paths are unique.
     *
     * Boss must never destroy a pre-existing staging or
     * backup directory merely because its name resembles
     * an old transaction. Such a directory may contain
     * recovery data from a previous failure.
     */
    let staging = ModuleInstallStaging::prepare(staging_path.clone())?;

    if backup_path.exists() {
        return Err(format!(
            "transactional backup path already exists and will not be removed automatically: {}",
            backup_path.display()
        ));
    }

    /*
     * Clone the candidate instead of mutating the current
     * checkout with git reset.
     */
    checkout_registry_commit(name, entry, staging.path())?;

    /*
     * read_manifest() is now the schema-3 gate.
     *
     * Before touching the installed copy this validates:
     * - schema
     * - name format
     * - semantic version
     * - entrypoint
     * - commands
     * - lifecycle values through serde
     * - launcher contract
     * - tray contract
     * - notifications protocol
     * - language contract
     * - dependency declarations
     * - safe paths
     */
    let manifest = read_manifest(&staging.path().join("manifest.json"))?;

    if manifest.name != name {
        return Err(format!(
            "staged module manifest name '{}' does not match registry id '{name}'",
            manifest.name
        ));
    }

    if let Some(expected_version) = entry.version.as_deref() {
        if !expected_version.trim().is_empty() && manifest.version != expected_version {
            return Err(format!(
                "module '{}' staged version '{}' does not match registry version '{}'",
                name, manifest.version, expected_version
            ));
        }
    }

    /*
     * Resolve every dependency while the old module is
     * still intact.
     */
    dependencies::resolve_system_dependencies(&manifest.dependencies.system)?;

    let mut visiting = HashSet::new();
    visiting.insert(name.to_string());

    for dependency in &manifest.dependencies.modules {
        match resolve_module_dependency(
            dependency,
            &registry,
            &mut visiting,
        ) {
            Ok(()) => {}

            Err(error) if dependency.required => {
                return Err(format!(
                    "required module dependency '{}' could not be resolved before updating '{}': {error}",
                    dependency.name,
                    name
                ));
            }

            Err(error) => eprintln!(
                "N.E.E.B.L.E.S.: optional module dependency '{}' could not be resolved before updating '{}': {error}",
                dependency.name,
                name
            ),
        }
    }

    /*
     * Candidate is valid.
     *
     * From this point forward we perform the smallest
     * possible filesystem transaction:
     *
     * current -> backup
     * staged  -> current
     */
    fs::rename(&current_path, &backup_path).map_err(|error| {
        format!(
            "could not move current module '{}' into transactional backup {}: {error}",
            name,
            backup_path.display()
        )
    })?;

    match staging.commit(&current_path) {
        Ok(()) => {}

        Err(commit_error) => {
            /*
             * The new version could not take the live path.
             * Restore the previous known-good copy.
             */
            match fs::rename(&backup_path, &current_path) {
                Ok(()) => {
                    return Err(format!(
                        "could not commit staged update for module '{}': {}; previous version was restored",
                        name,
                        commit_error
                    ));
                }

                Err(rollback_error) => {
                    return Err(format!(
                        "CRITICAL: could not commit staged update for module '{}': {}; rollback also failed: {}; previous module remains at {}",
                        name,
                        commit_error,
                        rollback_error,
                        backup_path.display()
                    ));
                }
            }
        }
    }

    /*
     * The new copy is now live.
     *
     * Remove the old known-good copy only after the swap
     * completed successfully.
     */
    if let Err(error) = fs::remove_dir_all(&backup_path) {
        return Err(format!(
            "module '{}' update was committed successfully, but transactional backup {} could not be removed: {error}",
            name,
            backup_path.display()
        ));
    }

    Ok(())
}
pub fn set_enabled(name: &str, enabled: bool) -> Result<(), String> {
    let _ = find_module_dir(name)?;

    if !enabled {
        stop_tray_provider(name)?;
        stop_module(name)?;

        config::set_module_enabled(name, false)?;

        return Ok(());
    }

    config::set_module_enabled(name, true)?;

    if let Err(error) = start_tray_provider(name) {
        /*
         * Enabling is one operation. Do not leave
         * Boss saying "enabled" when the declared
         * tray lifecycle could not be started.
         */
        let _ = config::set_module_enabled(name, false);

        return Err(error);
    }

    Ok(())
}

fn resolve_module_language(module_dir: &Path, requested: &str) -> Result<String, String> {
    let manifest_path = module_dir.join("languages").join("manifest.json");

    /*
     * Schema 3 modules always have a validated language
     * contract. No implicit legacy behavior exists here.
     */
    validate_module_language_contract(module_dir)?;

    let raw = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "could not read module language manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    let manifest: ModuleLanguageManifest = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid module language manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    let requested = languages::normalize_locale(requested);

    if let Some(language) = manifest
        .languages
        .iter()
        .find(|language| languages::normalize_locale(&language.code) == requested)
    {
        return Ok(language.code.clone());
    }

    let normalized_default = languages::normalize_locale(manifest.default.trim());

    let language = manifest
        .languages
        .iter()
        .find(|language| languages::normalize_locale(&language.code) == normalized_default)
        .ok_or_else(|| {
            format!(
                "module language manifest {} defines invalid default '{}'",
                manifest_path.display(),
                manifest.default
            )
        })?;

    Ok(language.code.clone())
}

pub fn execute(name: &str, args: &[String], caller: &str) -> Result<i32, String> {
    if !config::module_enabled(name)? {
        return Err(format!("module '{name}' is disabled"));
    }

    let module_dir = find_module_dir(name)?;

    let manifest = read_manifest(&module_dir.join("manifest.json"))?;

    let action = args.first().map(String::as_str).unwrap_or("default");

    let contract = manifest
        .commands
        .get(action)
        .ok_or_else(|| format!("module '{}' does not declare command '{}'", name, action))?;

    let track_runtime = contract.lifecycle == CommandLifecycle::Tracked;

    if track_runtime && module_running(name) {
        return Err(format!("module '{name}' is already running"));
    }

    privileges::ensure_root(contract.requires_root)?;

    let entrypoint = resolve_entrypoint(&module_dir, &manifest.entrypoint)?;

    let requested_language = config::load_or_initialize()?.language;

    let language = resolve_module_language(&module_dir, &requested_language)?;

    let config_path = config::config_path()?;

    let mut command = Command::new(&entrypoint);

    command
        .args(args)
        .current_dir(&module_dir)
        .env("NEEBLES_LANGUAGE", language)
        .env("NEEBLES_CALLER", caller)
        .env("NEEBLES_MODULE", name)
        .env("NEEBLES_CONFIG", config_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = command
        .spawn()
        .map_err(|error| format!("could not launch module '{name}': {error}"))?;

    if track_runtime {
        if let Err(error) = write_runtime_marker(name, child.id()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    }

    let result = child
        .wait()
        .map_err(|error| format!("could not wait for module '{name}': {error}"));

    if track_runtime {
        clear_runtime_marker(name);
    }

    let status = result?;

    Ok(status.code().unwrap_or(1))
}
pub fn find_module_dir(name: &str) -> Result<PathBuf, String> {
    let direct = modules_root().join(name);
    if direct.join("manifest.json").exists() {
        let manifest = read_manifest(&direct.join("manifest.json"))?;
        if manifest.name == name {
            return Ok(direct);
        }
    }

    let root = modules_root();
    if root.exists() {
        for entry in fs::read_dir(&root)
            .map_err(|error| format!("could not read {}: {error}", root.display()))?
        {
            let entry =
                entry.map_err(|error| format!("could not read module directory entry: {error}"))?;
            let manifest_path = entry.path().join("manifest.json");
            if !manifest_path.exists() {
                continue;
            }
            let manifest = read_manifest(&manifest_path)?;
            if manifest.name == name {
                return Ok(entry.path());
            }
        }
    }

    Err(format!("module '{name}' is not installed"))
}

fn validate_module_language_contract(module_dir: &Path) -> Result<(), String> {
    let manifest_path = module_dir.join("languages").join("manifest.json");

    if !manifest_path.is_file() {
        return Err(format!(
            "module schema {} requires a language manifest: {}",
            MODULE_SCHEMA_VERSION,
            manifest_path.display()
        ));
    }

    let raw = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "could not read module language manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    let manifest: ModuleLanguageManifest = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid module language manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    if manifest.schema != MODULE_LANGUAGE_SCHEMA_VERSION {
        return Err(format!(
            "module language manifest {} declares unsupported schema {}; expected {}",
            manifest_path.display(),
            manifest.schema,
            MODULE_LANGUAGE_SCHEMA_VERSION
        ));
    }

    if manifest.languages.is_empty() {
        return Err(format!(
            "module language manifest {} declares no languages",
            manifest_path.display()
        ));
    }

    let mut normalized_languages = HashSet::new();

    for language in &manifest.languages {
        let code = language.code.trim();

        if code.is_empty() {
            return Err(format!(
                "module language manifest {} contains an empty language code",
                manifest_path.display()
            ));
        }

        let normalized = languages::normalize_locale(code);

        if normalized.is_empty() {
            return Err(format!(
                "module language manifest {} contains invalid language code '{}'",
                manifest_path.display(),
                language.code
            ));
        }

        if !normalized_languages.insert(normalized.clone()) {
            return Err(format!(
                "module language manifest {} declares duplicate language '{}'",
                manifest_path.display(),
                normalized
            ));
        }
    }

    let default = manifest.default.trim();

    if default.is_empty() {
        return Err(format!(
            "module language manifest {} must declare a default language",
            manifest_path.display()
        ));
    }

    let normalized_default = languages::normalize_locale(default);

    if !normalized_languages.contains(&normalized_default) {
        return Err(format!(
            "module language manifest {} defines default '{}' but that language is not declared",
            manifest_path.display(),
            manifest.default
        ));
    }

    Ok(())
}

fn resolve_module_file(module_dir: &Path, value: &str, field: &str) -> Result<PathBuf, String> {
    let value = value.trim();

    if value.is_empty() {
        return Err(format!("module {field} cannot be empty"));
    }

    let relative = Path::new(value);

    if relative.is_absolute() {
        return Err(format!(
            "module {field} must be relative to the module directory"
        ));
    }

    for component in relative.components() {
        use std::path::Component;

        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("module {field} contains an invalid path: {value}"));
            }
        }
    }

    let candidate = module_dir.join(relative);

    let canonical = candidate.canonicalize().map_err(|error| {
        format!(
            "could not resolve module {field} {}: {error}",
            candidate.display()
        )
    })?;

    let canonical_module = module_dir.canonicalize().map_err(|error| {
        format!(
            "could not resolve module directory {}: {error}",
            module_dir.display()
        )
    })?;

    if !canonical.starts_with(&canonical_module) {
        return Err(format!("module {field} escapes module directory"));
    }

    if !canonical.is_file() {
        return Err(format!(
            "module {field} is not a file: {}",
            canonical.display()
        ));
    }

    Ok(canonical)
}

fn launcher_action(manifest: &ModuleManifest) -> Result<Option<String>, String> {
    let actions: Vec<&String> = manifest
        .commands
        .iter()
        .filter_map(|(name, command)| command.launcher.then_some(name))
        .collect();

    match actions.as_slice() {
        [] => Ok(None),
        [action] => Ok(Some((*action).clone())),
        _ => Err(format!(
            "module '{}' declares more than one launcher command",
            manifest.name
        )),
    }
}

pub fn installed_module_launcher_action(name: &str) -> Result<Option<String>, String> {
    let manifest = installed_module_manifest(name)?;
    launcher_action(&manifest)
}

fn validate_module_manifest(module_dir: &Path, manifest: &ModuleManifest) -> Result<(), String> {
    if manifest.schema != MODULE_SCHEMA_VERSION {
        return Err(format!(
            "module '{}' declares unsupported schema {}; expected {}",
            manifest.name, manifest.schema, MODULE_SCHEMA_VERSION
        ));
    }

    if !valid_module_id(&manifest.name) {
        return Err(format!("invalid module id: {}", manifest.name));
    }

    if manifest.version.trim().is_empty() {
        return Err(format!("module '{}' must declare a version", manifest.name));
    }

    Version::parse(manifest.version.trim()).map_err(|error| {
        format!(
            "module '{}' declares invalid semantic version '{}': {error}",
            manifest.name, manifest.version
        )
    })?;

    for dependency in &manifest.dependencies.modules {
        if !valid_module_id(&dependency.name) {
            return Err(format!(
                "module '{}' declares invalid module dependency id '{}'",
                manifest.name, dependency.name
            ));
        }

        if dependency.name == manifest.name {
            return Err(format!(
                "module '{}' cannot depend on itself",
                manifest.name
            ));
        }

        if let Some(minimum_version) = dependency.minimum_version.as_deref() {
            let minimum_version = minimum_version.trim();

            if minimum_version.is_empty() {
                return Err(format!(
                    "module '{}' declares an empty minimum_version for dependency '{}'",
                    manifest.name, dependency.name
                ));
            }

            Version::parse(minimum_version).map_err(|error| {
                format!(
                    "module '{}' declares invalid minimum_version '{}' for dependency '{}': {error}",
                    manifest.name, minimum_version, dependency.name
                )
            })?;
        }
    }

    let entrypoint = resolve_module_file(module_dir, &manifest.entrypoint, "entrypoint")?;

    let metadata = fs::metadata(&entrypoint).map_err(|error| {
        format!(
            "could not inspect module '{}' entrypoint {}: {error}",
            manifest.name,
            entrypoint.display()
        )
    })?;

    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!(
            "module '{}' entrypoint is not executable: {}",
            manifest.name,
            entrypoint.display()
        ));
    }

    if manifest.commands.is_empty() {
        return Err(format!(
            "module '{}' must declare at least one command",
            manifest.name
        ));
    }

    for action in manifest.commands.keys() {
        if !valid_module_id(action) {
            return Err(format!(
                "module '{}' declares invalid command id '{}'",
                manifest.name, action
            ));
        }
    }

    let _ = launcher_action(manifest)?;

    if let Some(tray) = &manifest.tray {
        if tray.protocol != crate::tray::protocol::TRAY_PROTOCOL_VERSION {
            return Err(format!(
                "module '{}' declares unsupported tray protocol {}; expected {}",
                manifest.name,
                tray.protocol,
                crate::tray::protocol::TRAY_PROTOCOL_VERSION
            ));
        }

        let _ = resolve_module_contract_path(module_dir, &tray.icon, "icon")?;
        let _ = resolve_module_contract_path(module_dir, &tray.provider, "provider")?;
    }

    if let Some(notifications) = &manifest.notifications {
        if notifications.protocol != MODULE_NOTIFICATIONS_PROTOCOL_VERSION {
            return Err(format!(
                "module '{}' declares unsupported notifications protocol {}; expected {}",
                manifest.name, notifications.protocol, MODULE_NOTIFICATIONS_PROTOCOL_VERSION
            ));
        }
    }

    validate_module_language_contract(module_dir)?;

    Ok(())
}

fn read_manifest(path: &Path) -> Result<ModuleManifest, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("could not read module manifest {}: {error}", path.display()))?;

    let manifest: ModuleManifest = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid module manifest {}: {error}", path.display()))?;

    let module_dir = path.parent().ok_or_else(|| {
        format!(
            "module manifest has no parent directory: {}",
            path.display()
        )
    })?;

    validate_module_manifest(module_dir, &manifest)?;

    Ok(manifest)
}

fn resolve_entrypoint(module_dir: &Path, entrypoint: &str) -> Result<PathBuf, String> {
    resolve_module_file(module_dir, entrypoint, "entrypoint")
}

fn valid_module_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

#[allow(dead_code)]
fn _language_contract_example() -> Result<String, String> {
    Ok(languages::load_manifest()?.default)
}

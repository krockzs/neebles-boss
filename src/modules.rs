use crate::config;
use crate::dependencies::{self, DependencySet};
use crate::languages;
use crate::privileges;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandContract {
    #[serde(default)]
    pub requires_root: bool,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleLanguageEntry {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleLanguageManifest {
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub languages: Vec<ModuleLanguageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryModule {
    pub repo: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
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

struct ModuleInstallStaging {
    path: PathBuf,
    committed: bool,
}

impl ModuleInstallStaging {
    fn prepare(path: PathBuf) -> Result<Self, String> {
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|error| {
                format!(
                    "could not clear module staging directory {}: {error}",
                    path.display()
                )
            })?;
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

fn github_raw_base(repo: &str, branch: &str) -> Option<String> {
    let repo = repo
        .strip_prefix("https://github.com/")?
        .trim_end_matches(".git")
        .trim_end_matches('/');

    Some(format!("https://raw.githubusercontent.com/{repo}/{branch}"))
}

fn remote_module_icon(repo: &str, branch: Option<&str>) -> String {
    let branch = branch.unwrap_or("main");

    let Some(base) = github_raw_base(repo, branch) else {
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
    if let Ok(value) = env::var("PKEXEC_UID") {
        if !value.trim().is_empty() {
            return value;
        }
    }

    if let Ok(value) = env::var("XDG_RUNTIME_DIR") {
        if let Some(name) = Path::new(&value)
            .file_name()
            .and_then(|value| value.to_str())
        {
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }

    env::var("USER")
        .unwrap_or_else(|_| "default".to_string())
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
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

fn module_pid(name: &str) -> Option<u32> {
    let path = runtime_marker_path(name);

    let raw = fs::read_to_string(&path).ok()?;

    let pid = match raw.trim().parse::<u32>() {
        Ok(pid) => pid,
        Err(_) => {
            clear_runtime_marker(name);
            return None;
        }
    };

    let process_path = PathBuf::from(format!("/proc/{pid}"));

    if !process_path.exists() {
        clear_runtime_marker(name);
        return None;
    }

    Some(pid)
}

pub fn module_running(name: &str) -> bool {
    module_pid(name).is_some()
}

fn stop_module(name: &str) -> Result<(), String> {
    let Some(pid) = module_pid(name) else {
        return Ok(());
    };

    /*
     * Boss owns module lifecycle.
     *
     * SIGTERM gives the application an opportunity
     * to close normally instead of killing it abruptly.
     */
    let status = Command::new("kill")
        .args(["-TERM", pid.to_string().as_str()])
        .status()
        .map_err(|error| format!("could not stop module '{name}' process {pid}: {error}"))?;

    if !status.success() {
        return Err(format!("could not stop module '{name}' process {pid}"));
    }

    Ok(())
}

fn tray_runtime_marker_path(name: &str) -> PathBuf {
    runtime_modules_root().join(format!("{name}.tray.pid"))
}

fn clear_tray_runtime_marker(name: &str) {
    let _ = fs::remove_file(tray_runtime_marker_path(name));
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

fn tray_provider_pid(name: &str) -> Option<u32> {
    let path = tray_runtime_marker_path(name);

    let raw = fs::read_to_string(&path).ok()?;

    let pid = match raw.trim().parse::<u32>() {
        Ok(pid) => pid,

        Err(_) => {
            clear_tray_runtime_marker(name);

            return None;
        }
    };

    let process_path = PathBuf::from(format!("/proc/{pid}"));

    if !process_path.exists() {
        clear_tray_runtime_marker(name);

        return None;
    }

    /*
     * Do not trust a PID marker by itself.
     *
     * Linux may reuse process ids. Verify that the
     * process command line still references the
     * provider declared by this module.
     */
    let contract = match resolved_tray_contract(name) {
        Ok(contract) => contract,

        Err(_) => {
            clear_tray_runtime_marker(name);

            return None;
        }
    };

    let cmdline = match fs::read(format!("/proc/{pid}/cmdline")) {
        Ok(value) => value,

        Err(_) => {
            clear_tray_runtime_marker(name);

            return None;
        }
    };

    let provider = contract.provider.as_bytes();

    let belongs_to_provider = cmdline
        .split(|byte| *byte == 0)
        .any(|argument| argument == provider);

    if !belongs_to_provider {
        clear_tray_runtime_marker(name);

        return None;
    }

    Some(pid)
}

pub fn tray_provider_running(name: &str) -> bool {
    tray_provider_pid(name).is_some()
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
    let Some(pid) = tray_provider_pid(name) else {
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
            "could not stop tray provider for module '{}' process {}",
            name, pid
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

    while tray_provider_running(name) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if tray_provider_running(name) {
        return Err(format!(
            "tray provider for module '{}' did not close in time",
            name
        ));
    }

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

    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid N.E.E.B.L.E.S. module registry: {error}"))
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

        let mut item = json!({
            "name": manifest.name,
            "version": manifest.version,
            "enabled": enabled,
            "running": running,
            "path": entry.path(),
            "icon": icon,
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

        let icon = remote_module_icon(&module.repo, module.branch.as_deref());

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
    let temp = temp_root.join(format!("install-{name}-{}", std::process::id()));

    let staging = ModuleInstallStaging::prepare(temp)?;

    let mut clone = Command::new("git");
    clone.arg("clone").arg("--depth").arg("1");
    if let Some(branch) = &entry.branch {
        clone.arg("--branch").arg(branch);
    }
    let status = clone
        .arg(&entry.repo)
        .arg(staging.path())
        .status()
        .map_err(|error| format!("could not start git clone for '{name}': {error}"))?;
    if !status.success() {
        return Err(format!(
            "git clone failed for module '{name}' with status {status}"
        ));
    }

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
        match install_internal(&dependency.name, registry, visiting) {
            Ok(()) => {}
            Err(error) if dependency.required => return Err(error),
            Err(error) => eprintln!(
                "N.E.E.B.L.E.S.: optional module dependency '{}' could not be installed: {error}",
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
    if module_running(name) {
        return Err(format!("module '{name}' is currently running"));
    }

    let path = find_module_dir(name)?;

    fs::remove_dir_all(&path)
        .map_err(|error| format!("could not remove module {}: {error}", path.display()))
}

pub fn update(name: &str, close_running: bool) -> Result<(), String> {
    if module_running(name) {
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

        while module_running(name) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        if module_running(name) {
            return Err(format!("module '{name}' did not close in time"));
        }
    }

    let path = find_module_dir(name)?;

    if !path.join(".git").exists() {
        return Err(format!("module '{name}' is not backed by a git checkout"));
    }

    let registry = fetch_registry()?;

    let entry = registry
        .modules
        .get(name)
        .ok_or_else(|| format!("module '{name}' does not exist in the N.E.E.B.L.E.S. registry"))?;

    let branch = entry.branch.as_deref().unwrap_or("main");

    let fetch_status = Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["fetch", "--depth", "1", "origin", branch])
        .status()
        .map_err(|error| format!("could not fetch update for '{name}': {error}"))?;

    if !fetch_status.success() {
        return Err(format!(
            "git fetch failed for module '{name}' with status {fetch_status}"
        ));
    }

    let reset_status = Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["reset", "--hard", "FETCH_HEAD"])
        .status()
        .map_err(|error| format!("could not apply update for '{name}': {error}"))?;

    if !reset_status.success() {
        return Err(format!(
            "git reset failed for module '{name}' with status {reset_status}"
        ));
    }

    let manifest = read_manifest(&path.join("manifest.json"))?;

    if let Some(expected_version) = entry.version.as_deref() {
        if !expected_version.is_empty() && manifest.version != expected_version {
            return Err(format!(
                "module '{name}' updated but manifest version '{}' does not match registry version '{}'",
                manifest.version,
                expected_version
            ));
        }
    }

    if manifest.tray.is_some() {
        resolve_tray_contract_from(&path, &manifest)?;
    }

    dependencies::resolve_system_dependencies(&manifest.dependencies.system)?;

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
     * Legacy modules may not implement the N.E.E.B.L.E.S.
     * language contract yet. Preserve the historical behavior
     * and pass the Boss language unchanged.
     */
    if !manifest_path.exists() {
        return Ok(requested.to_string());
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

    if manifest.languages.is_empty() {
        return Err(format!(
            "module language manifest {} declares no languages",
            manifest_path.display()
        ));
    }

    let requested = languages::normalize_locale(requested);

    if let Some(language) = manifest
        .languages
        .iter()
        .find(|language| languages::normalize_locale(&language.code) == requested)
    {
        return Ok(language.code.clone());
    }

    let default = manifest.default.trim();

    if !default.is_empty() {
        let normalized_default = languages::normalize_locale(default);

        if let Some(language) = manifest
            .languages
            .iter()
            .find(|language| languages::normalize_locale(&language.code) == normalized_default)
        {
            return Ok(language.code.clone());
        }

        return Err(format!(
            "module language manifest {} defines default '{}' but that language is not declared",
            manifest_path.display(),
            manifest.default
        ));
    }

    Ok(manifest.languages[0].code.clone())
}

pub fn execute(name: &str, args: &[String], caller: &str) -> Result<i32, String> {
    if !config::module_enabled(name)? {
        return Err(format!("module '{name}' is disabled"));
    }

    let module_dir = find_module_dir(name)?;

    let manifest = read_manifest(&module_dir.join("manifest.json"))?;

    let action = args.first().map(String::as_str).unwrap_or("default");

    let track_runtime = action == "open";

    if track_runtime && module_running(name) {
        return Err(format!("module '{name}' is already running"));
    }

    let requires_root = manifest
        .commands
        .get(action)
        .map(|contract| contract.requires_root)
        .unwrap_or(false);

    privileges::ensure_root(requires_root)?;

    let entrypoint = resolve_entrypoint(&module_dir, &manifest.entrypoint);

    if !entrypoint.exists() {
        return Err(format!(
            "module entrypoint does not exist: {}",
            entrypoint.display()
        ));
    }

    let requested_language = config::load_or_initialize()?.language;

    let language = resolve_module_language(&module_dir, &requested_language)?;

    let config_path = config::config_path()?;

    let mut command = Command::new(&entrypoint);

    command
        .args(args)
        .current_dir(&module_dir)
        .env("NEEBLES_LANGUAGE", language)
        .env("NEEBLES_CALLER", caller)
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

fn read_manifest(path: &Path) -> Result<ModuleManifest, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("could not read module manifest {}: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("invalid module manifest {}: {error}", path.display()))
}

fn resolve_entrypoint(module_dir: &Path, entrypoint: &str) -> PathBuf {
    let path = PathBuf::from(entrypoint);
    if path.is_absolute() {
        path
    } else {
        module_dir.join(path)
    }
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

use crate::config;
use crate::dependencies::{self, DependencySet};
use crate::languages;
use crate::privileges;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandContract {
    #[serde(default)]
    pub requires_root: bool,
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

const MODULE_ICON_EXTENSIONS: &[&str] = &[
    "svg",
    "png",
    "webp",
    "jpg",
    "jpeg",
];

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

    Some(format!(
        "https://raw.githubusercontent.com/{repo}/{branch}"
    ))
}

fn remote_module_icon(repo: &str, branch: Option<&str>) -> String {
    let branch = branch.unwrap_or("main");

    let Some(base) = github_raw_base(repo, branch) else {
        return String::new();
    };

    for extension in MODULE_ICON_EXTENSIONS {
        let url = format!("{base}/icon.{extension}");

        let status = Command::new("curl")
            .args([
                "-fsIL",
                "--max-time",
                "5",
                url.as_str(),
            ])
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
            if character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_')
            {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn runtime_modules_root() -> PathBuf {
    env::temp_dir()
        .join(format!(
            "neebles-runtime-{}",
            runtime_identity()
        ))
        .join("modules")
}

fn runtime_marker_path(name: &str) -> PathBuf {
    runtime_modules_root()
        .join(format!("{name}.pid"))
}

fn clear_runtime_marker(name: &str) {
    let _ = fs::remove_file(
        runtime_marker_path(name)
    );
}

fn write_runtime_marker(
    name: &str,
    pid: u32,
) -> Result<(), String> {
    let root = runtime_modules_root();

    fs::create_dir_all(&root)
        .map_err(|error| {
            format!(
                "could not create module runtime directory {}: {error}",
                root.display()
            )
        })?;

    let path = runtime_marker_path(name);

    fs::write(
        &path,
        format!("{pid}\n"),
    )
    .map_err(|error| {
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

    let process_path =
        PathBuf::from(format!("/proc/{pid}"));

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
        .args([
            "-TERM",
            pid.to_string().as_str(),
        ])
        .status()
        .map_err(|error| {
            format!(
                "could not stop module '{name}' process {pid}: {error}"
            )
        })?;

    if !status.success() {
        return Err(format!(
            "could not stop module '{name}' process {pid}"
        ));
    }

    Ok(())
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
            "git is required by Boss to resolve the current module registry revision"
                .to_string()
        );
    }

    let output = Command::new("git")
        .args([
            "ls-remote",
            "https://github.com/krockzs/neebles-boss.git",
            "refs/heads/main",
        ])
        .output()
        .map_err(|error| {
            format!(
                "could not resolve N.E.E.B.L.E.S. registry revision: {error}"
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "could not resolve N.E.E.B.L.E.S. registry revision: {}",
            output.status
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| {
            format!(
                "invalid git ls-remote output while resolving registry: {error}"
            )
        })?;

    let commit = stdout
        .split_whitespace()
        .next()
        .ok_or_else(|| {
            "git ls-remote returned no revision for N.E.E.B.L.E.S. Boss main"
                .to_string()
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
        return Err(format!("could not read N.E.E.B.L.E.S. module registry: {}", output.status));
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
        let entry = entry.map_err(|error| format!("could not read module directory entry: {error}"))?;
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

        result.push(json!({
            "name": manifest.name,
            "version": manifest.version,
            "enabled": enabled,
            "running": running,
            "path": entry.path(),
            "icon": icon,
        }));
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

        let icon = remote_module_icon(
            &module.repo,
            module.branch.as_deref(),
        );

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

fn install_internal(name: &str, registry: &Registry, visiting: &mut HashSet<String>) -> Result<(), String> {
    if find_module_dir(name).is_ok() {
        return Ok(());
    }

    if !visiting.insert(name.to_string()) {
        return Err(format!("circular N.E.E.B.L.E.S. module dependency detected at '{name}'"));
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
    if temp.exists() {
        fs::remove_dir_all(&temp)
            .map_err(|error| format!("could not clear {}: {error}", temp.display()))?;
    }

    let mut clone = Command::new("git");
    clone.arg("clone").arg("--depth").arg("1");
    if let Some(branch) = &entry.branch {
        clone.arg("--branch").arg(branch);
    }
    let status = clone
        .arg(&entry.repo)
        .arg(&temp)
        .status()
        .map_err(|error| format!("could not start git clone for '{name}': {error}"))?;
    if !status.success() {
        return Err(format!("git clone failed for module '{name}' with status {status}"));
    }

    let manifest_path = temp.join("manifest.json");
    let manifest = read_manifest(&manifest_path)?;
    if manifest.name != name {
        return Err(format!(
            "module manifest name '{}' does not match registry id '{name}'",
            manifest.name
        ));
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
        return Err(format!("module destination already exists: {}", destination.display()));
    }
    fs::rename(&temp, &destination).map_err(|error| {
        format!(
            "could not move module '{}' into {}: {error}",
            name,
            destination.display()
        )
    })?;

    visiting.remove(name);
    Ok(())
}

pub fn uninstall(name: &str) -> Result<(), String> {
    if module_running(name) {
        return Err(format!(
            "module '{name}' is currently running"
        ));
    }

    let path = find_module_dir(name)?;

    fs::remove_dir_all(&path)
        .map_err(|error| {
            format!(
                "could not remove module {}: {error}",
                path.display()
            )
        })
}

pub fn update(
    name: &str,
    close_running: bool,
) -> Result<(), String> {
    if module_running(name) {
        if !close_running {
            return Err(format!(
                "module '{name}' is currently running"
            ));
        }

        /*
         * Authorization has already been granted before
         * this privileged backend process starts.
         *
         * Only now may Boss close the running application.
         */
        stop_module(name)?;

        let deadline =
            std::time::Instant::now()
            + std::time::Duration::from_secs(15);

        while module_running(name)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(
                std::time::Duration::from_millis(200)
            );
        }

        if module_running(name) {
            return Err(format!(
                "module '{name}' did not close in time"
            ));
        }
    }

    let path = find_module_dir(name)?;

    if !path.join(".git").exists() {
        return Err(format!(
            "module '{name}' is not backed by a git checkout"
        ));
    }

    let registry = fetch_registry()?;

    let entry = registry
        .modules
        .get(name)
        .ok_or_else(|| {
            format!(
                "module '{name}' does not exist in the N.E.E.B.L.E.S. registry"
            )
        })?;

    let branch =
        entry.branch.as_deref().unwrap_or("main");

    let fetch_status = Command::new("git")
        .arg("-C")
        .arg(&path)
        .args([
            "fetch",
            "--depth",
            "1",
            "origin",
            branch,
        ])
        .status()
        .map_err(|error| {
            format!(
                "could not fetch update for '{name}': {error}"
            )
        })?;

    if !fetch_status.success() {
        return Err(format!(
            "git fetch failed for module '{name}' with status {fetch_status}"
        ));
    }

    let reset_status = Command::new("git")
        .arg("-C")
        .arg(&path)
        .args([
            "reset",
            "--hard",
            "FETCH_HEAD",
        ])
        .status()
        .map_err(|error| {
            format!(
                "could not apply update for '{name}': {error}"
            )
        })?;

    if !reset_status.success() {
        return Err(format!(
            "git reset failed for module '{name}' with status {reset_status}"
        ));
    }

    let manifest =
        read_manifest(&path.join("manifest.json"))?;

    if let Some(expected_version) =
        entry.version.as_deref()
    {
        if !expected_version.is_empty()
            && manifest.version != expected_version
        {
            return Err(format!(
                "module '{name}' updated but manifest version '{}' does not match registry version '{}'",
                manifest.version,
                expected_version
            ));
        }
    }

    dependencies::resolve_system_dependencies(
        &manifest.dependencies.system
    )
}
pub fn set_enabled(
    name: &str,
    enabled: bool,
) -> Result<(), String> {
    let _ = find_module_dir(name)?;

    if !enabled {
        stop_module(name)?;
    }

    config::set_module_enabled(
        name,
        enabled,
    )?;

    Ok(())
}

pub fn execute(
    name: &str,
    args: &[String],
    caller: &str,
) -> Result<i32, String> {
    if !config::module_enabled(name)? {
        return Err(format!(
            "module '{name}' is disabled"
        ));
    }

    let module_dir = find_module_dir(name)?;

    let manifest =
        read_manifest(&module_dir.join("manifest.json"))?;

    let action =
        args.first()
            .map(String::as_str)
            .unwrap_or("default");

    let track_runtime = action == "open";

    if track_runtime && module_running(name) {
        return Err(format!(
            "module '{name}' is already running"
        ));
    }

    let requires_root = manifest
        .commands
        .get(action)
        .map(|contract| contract.requires_root)
        .unwrap_or(false);

    privileges::ensure_root(requires_root)?;

    let entrypoint =
        resolve_entrypoint(
            &module_dir,
            &manifest.entrypoint,
        );

    if !entrypoint.exists() {
        return Err(format!(
            "module entrypoint does not exist: {}",
            entrypoint.display()
        ));
    }

    let language =
        config::load_or_initialize()?.language;

    let config_path =
        config::config_path()?;

    let mut command =
        Command::new(&entrypoint);

    command
        .args(args)
        .current_dir(&module_dir)
        .env("NEEBLES_LANGUAGE", language)
        .env("NEEBLES_CALLER", caller)
        .env("NEEBLES_CONFIG", config_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child =
        command.spawn()
            .map_err(|error| {
                format!(
                    "could not launch module '{name}': {error}"
                )
            })?;

    if track_runtime {
        if let Err(error) =
            write_runtime_marker(
                name,
                child.id(),
            )
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    }

    let result =
        child.wait()
            .map_err(|error| {
                format!(
                    "could not wait for module '{name}': {error}"
                )
            });

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
            let entry = entry.map_err(|error| format!("could not read module directory entry: {error}"))?;
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
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
}

#[allow(dead_code)]
fn _language_contract_example() -> Result<String, String> {
    Ok(languages::load_manifest()?.default)
}

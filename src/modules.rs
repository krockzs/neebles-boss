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

fn registry_url() -> String {
    env::var("NEEBLES_MODULES_REGISTRY").unwrap_or_else(|_| {
        concat!(
            "https:",
            "//raw.githubusercontent.com/krockzs/neebles-boss/main/registry/modules.json"
        )
        .to_string()
    })
}

pub fn fetch_registry() -> Result<Registry, String> {
    if !dependencies::command_exists("curl") {
        return Err("curl is required by Boss to read the remote module registry".to_string());
    }

    let url = registry_url();
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

        result.push(json!({
            "name": manifest.name,
            "version": manifest.version,
            "enabled": enabled,
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

    /*
     * A newly installed module is active by default.
     * This also clears a stale disabled state left by an
     * earlier installation of the same module.
     */
    config::set_module_enabled(name, true)?;

    visiting.remove(name);
    Ok(())
}

pub fn uninstall(name: &str) -> Result<(), String> {
    let path = find_module_dir(name)?;
    fs::remove_dir_all(&path)
        .map_err(|error| format!("could not remove module {}: {error}", path.display()))
}

pub fn update(name: &str) -> Result<(), String> {
    let path = find_module_dir(name)?;
    if !path.join(".git").exists() {
        return Err(format!("module '{name}' is not backed by a git checkout"));
    }
    let status = Command::new("git")
        .arg("-C")
        .arg(&path)
        .args(["pull", "--ff-only"])
        .status()
        .map_err(|error| format!("could not start git pull for '{name}': {error}"))?;
    if !status.success() {
        return Err(format!("git pull failed for module '{name}' with status {status}"));
    }
    let manifest = read_manifest(&path.join("manifest.json"))?;
    dependencies::resolve_system_dependencies(&manifest.dependencies.system)
}

pub fn set_enabled(name: &str, enabled: bool) -> Result<(), String> {
    let _ = find_module_dir(name)?;
    config::set_module_enabled(name, enabled)?;
    Ok(())
}

pub fn execute(name: &str, args: &[String], caller: &str) -> Result<i32, String> {
    if !config::module_enabled(name)? {
        return Err(format!("module '{name}' is disabled"));
    }

    let module_dir = find_module_dir(name)?;
    let manifest = read_manifest(&module_dir.join("manifest.json"))?;
    let action = args.first().map(String::as_str).unwrap_or("default");
    let requires_root = manifest
        .commands
        .get(action)
        .map(|contract| contract.requires_root)
        .unwrap_or(false);

    privileges::ensure_root(requires_root)?;

    let entrypoint = resolve_entrypoint(&module_dir, &manifest.entrypoint);
    if !entrypoint.exists() {
        return Err(format!("module entrypoint does not exist: {}", entrypoint.display()));
    }

    let language = config::load_or_initialize()?.language;
    let config_path = config::config_path()?;
    let status = Command::new(&entrypoint)
        .args(args)
        .current_dir(&module_dir)
        .env("NEEBLES_LANGUAGE", language)
        .env("NEEBLES_CALLER", caller)
        .env("NEEBLES_CONFIG", config_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("could not launch module '{name}': {error}"))?;

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

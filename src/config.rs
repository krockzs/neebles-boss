use crate::languages;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BossConfig {
    pub language: String,
    pub tray_enabled: bool,
    pub launcher_enabled: bool,
    pub normal_notifications: bool,
    #[serde(default)]
    pub disabled_modules: Vec<String>,
    #[serde(default)]
    pub hidden_tray_modules: Vec<String>,
    #[serde(default)]
    pub hidden_launcher_modules: Vec<String>,
    #[serde(default)]
    pub module_update_notifications: BTreeMap<String, String>,
}

impl BossConfig {
    pub fn initial() -> Self {
        Self {
            language: languages::detect_initial_language(),
            tray_enabled: true,
            launcher_enabled: true,
            normal_notifications: true,
            disabled_modules: Vec::new(),
            hidden_tray_modules: Vec::new(),
            hidden_launcher_modules: Vec::new(),
            module_update_notifications: BTreeMap::new(),
        }
    }
}

fn nonempty_env_path(key: &str) -> Option<PathBuf> {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

pub fn config_path() -> Result<PathBuf, String> {
    if let Some(path) = nonempty_env_path("NEEBLES_CONFIG") {
        return Ok(path);
    }

    if let Some(path) = nonempty_env_path("XDG_CONFIG_HOME") {
        return Ok(path.join("neebles/boss.json"));
    }

    if let Some(path) = nonempty_env_path("HOME") {
        return Ok(path.join(".config/neebles/boss.json"));
    }

    Err("could not determine user configuration directory".to_string())
}

pub fn load_or_initialize() -> Result<BossConfig, String> {
    let path = config_path()?;
    if path.exists() {
        let raw = fs::read_to_string(&path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        return serde_json::from_str(&raw)
            .map_err(|error| format!("invalid Boss config {}: {error}", path.display()));
    }

    let config = BossConfig::initial();
    save(&config)?;
    Ok(config)
}

pub fn save(config: &BossConfig) -> Result<(), String> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    let data = serde_json::to_string_pretty(config)
        .map_err(|error| format!("could not serialize Boss config: {error}"))?;
    fs::write(&path, format!("{data}\n"))
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

pub fn set_language(code: &str) -> Result<BossConfig, String> {
    if !languages::is_supported(code) {
        return Err(format!("unsupported N.E.E.B.L.E.S. language: {code}"));
    }
    let mut config = load_or_initialize()?;
    config.language = languages::canonical_supported_code(code)?;
    save(&config)?;
    Ok(config)
}

pub fn set_bool(key: &str, value: bool) -> Result<BossConfig, String> {
    let mut config = load_or_initialize()?;
    match key {
        "tray_enabled" => config.tray_enabled = value,
        "launcher_enabled" => config.launcher_enabled = value,
        "normal_notifications" => config.normal_notifications = value,
        _ => return Err(format!("unknown Boss config key: {key}")),
    }
    save(&config)?;
    Ok(config)
}

pub fn set_module_enabled(name: &str, enabled: bool) -> Result<BossConfig, String> {
    let mut config = load_or_initialize()?;
    config.disabled_modules.retain(|item| item != name);
    if !enabled {
        config.disabled_modules.push(name.to_string());
        config.disabled_modules.sort();
        config.disabled_modules.dedup();
    }
    save(&config)?;
    Ok(config)
}

pub fn module_enabled(name: &str) -> Result<bool, String> {
    let config = load_or_initialize()?;
    Ok(!config.disabled_modules.iter().any(|item| item == name))
}

pub fn set_module_visibility(
    surface: &str,
    name: &str,
    visible: bool,
) -> Result<BossConfig, String> {
    let mut config = load_or_initialize()?;

    let hidden = match surface {
        "tray" => &mut config.hidden_tray_modules,
        "launcher" => &mut config.hidden_launcher_modules,
        _ => return Err(format!("unknown module visibility surface: {surface}")),
    };

    hidden.retain(|item| item != name);

    if !visible {
        hidden.push(name.to_string());
        hidden.sort();
        hidden.dedup();
    }

    save(&config)?;
    Ok(config)
}

pub fn module_visible(surface: &str, name: &str) -> Result<bool, String> {
    let config = load_or_initialize()?;

    let hidden = match surface {
        "tray" => &config.hidden_tray_modules,
        "launcher" => &config.hidden_launcher_modules,
        _ => return Err(format!("unknown module visibility surface: {surface}")),
    };

    Ok(!hidden.iter().any(|item| item == name))
}

pub fn mark_module_update_notified(name: &str, version: &str) -> Result<BossConfig, String> {
    let mut config = load_or_initialize()?;

    config
        .module_update_notifications
        .insert(name.to_string(), version.to_string());

    save(&config)?;
    Ok(config)
}

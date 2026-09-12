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
            module_update_notifications: BTreeMap::new(),
        }
    }
}

pub fn config_path() -> Result<PathBuf, String> {
    if let Ok(value) = env::var("NEEBLES_CONFIG") {
        return Ok(PathBuf::from(value));
    }

    if let Ok(value) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(value).join("neebles/boss.json"));
    }

    if let Ok(value) = env::var("HOME") {
        return Ok(PathBuf::from(value).join(".config/neebles/boss.json"));
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
    config.language = code.to_string();
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


pub fn mark_module_update_notified(
    name: &str,
    version: &str,
) -> Result<BossConfig, String> {
    let mut config = load_or_initialize()?;

    config
        .module_update_notifications
        .insert(name.to_string(), version.to_string());

    save(&config)?;
    Ok(config)
}

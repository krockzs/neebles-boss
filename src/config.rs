use crate::{languages, modules, settings};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use std::collections::BTreeMap;
use std::env;
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

pub fn config_path() -> Result<PathBuf, String> {
    if let Ok(value) = env::var("NEEBLES_CONFIG") {
        let value = value.trim();

        if value.is_empty() {
            return Err("NEEBLES_CONFIG is explicitly set but empty".to_string());
        }

        return Ok(PathBuf::from(value));
    }

    Ok(settings::boss_settings_path(&modules::neebles_root()))
}

pub fn default_json() -> Result<Value, String> {
    let language = languages::detect_initial_language()?;

    let default = json!({
        "launcher": {
            "enabled": "true",
            "hidden_modules": "[]"
        },

        "tray": {
            "enabled": "true",
            "hidden_modules": "[]"
        },

        "ui": {
            "language": language,
            "normal_notifications": "true",
            "disabled_modules": "[]",
            "module_update_notifications": "{}"
        }
    });

    settings::validate_default(&default)?;

    Ok(default)
}

fn local_settings() -> Result<(Value, Value), String> {
    let path = config_path()?;

    let default = default_json()?;

    let local = settings::update_from_default(&path, &default)?;

    Ok((local, default))
}

fn string(local: &Value, default: &Value, path: &str) -> Result<String, String> {
    settings::get_effective_path(local, default, path)
}

/*
 * From this point down, config.rs is a CONSUMER.
 *
 * Settings itself still knows only String.
 *
 * These helpers interpret individual Boss-owned Strings only
 * because the existing Boss runtime needs concrete decisions.
 */

fn boss_bool(local: &Value, default: &Value, path: &str) -> Result<bool, String> {
    match string(local, default, path)?.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),

        value => Err(format!(
            "Boss setting '{}' contains invalid value '{}'",
            path, value
        )),
    }
}

fn boss_list(local: &Value, default: &Value, path: &str) -> Result<Vec<String>, String> {
    let value = string(local, default, path)?;

    serde_json::from_str(&value).map_err(|error| {
        format!(
            "Boss setting '{}' contains invalid String data: {error}",
            path
        )
    })
}

fn boss_map(
    local: &Value,
    default: &Value,
    path: &str,
) -> Result<BTreeMap<String, String>, String> {
    let value = string(local, default, path)?;

    serde_json::from_str(&value).map_err(|error| {
        format!(
            "Boss setting '{}' contains invalid String data: {error}",
            path
        )
    })
}

fn string_list(value: &[String]) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!("could not serialize Boss String: {error}"))
}

fn string_map(value: &BTreeMap<String, String>) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!("could not serialize Boss String: {error}"))
}

pub fn load_or_initialize() -> Result<BossConfig, String> {
    let (local, default) = local_settings()?;

    Ok(BossConfig {
        language: string(&local, &default, "ui.language")?,

        tray_enabled: boss_bool(&local, &default, "tray.enabled")?,

        launcher_enabled: boss_bool(&local, &default, "launcher.enabled")?,

        normal_notifications: boss_bool(&local, &default, "ui.normal_notifications")?,

        disabled_modules: boss_list(&local, &default, "ui.disabled_modules")?,

        hidden_tray_modules: boss_list(&local, &default, "tray.hidden_modules")?,

        hidden_launcher_modules: boss_list(&local, &default, "launcher.hidden_modules")?,

        module_update_notifications: boss_map(&local, &default, "ui.module_update_notifications")?,
    })
}

pub fn set_language(code: &str) -> Result<BossConfig, String> {
    let canonical = languages::resolve_language(code)?
        .ok_or_else(|| format!("unsupported N.E.E.B.L.E.S. language: {code}"))?;

    let path = config_path()?;

    let default = default_json()?;

    settings::update_from_default(&path, &default)?;

    settings::set_path(&path, &default, "ui.language", canonical)?;

    load_or_initialize()
}

pub fn set_bool(key: &str, value: bool) -> Result<BossConfig, String> {
    let path_name = match key {
        "tray_enabled" => "tray.enabled",

        "launcher_enabled" => "launcher.enabled",

        "normal_notifications" => "ui.normal_notifications",

        _ => {
            return Err(format!("unknown Boss config key: {key}"));
        }
    };

    let path = config_path()?;

    let default = default_json()?;

    settings::update_from_default(&path, &default)?;

    settings::set_path(&path, &default, path_name, value.to_string())?;

    load_or_initialize()
}

pub fn set_module_enabled(name: &str, enabled: bool) -> Result<BossConfig, String> {
    let (local, default) = local_settings()?;

    let mut modules = boss_list(&local, &default, "ui.disabled_modules")?;

    modules.retain(|item| item != name);

    if !enabled {
        modules.push(name.to_string());

        modules.sort();
        modules.dedup();
    }

    settings::set_path(
        &config_path()?,
        &default,
        "ui.disabled_modules",
        string_list(&modules)?,
    )?;

    load_or_initialize()
}

pub fn module_enabled(name: &str) -> Result<bool, String> {
    let (local, default) = local_settings()?;

    let modules = boss_list(&local, &default, "ui.disabled_modules")?;

    Ok(!modules.iter().any(|item| item == name))
}

fn visibility_path(surface: &str) -> Result<&'static str, String> {
    match surface {
        "tray" => Ok("tray.hidden_modules"),

        "launcher" => Ok("launcher.hidden_modules"),

        _ => Err(format!("unknown module visibility surface: {surface}")),
    }
}

pub fn set_module_visibility(
    surface: &str,
    name: &str,
    visible: bool,
) -> Result<BossConfig, String> {
    let path_name = visibility_path(surface)?;

    let (local, default) = local_settings()?;

    let mut hidden = boss_list(&local, &default, path_name)?;

    hidden.retain(|item| item != name);

    if !visible {
        hidden.push(name.to_string());

        hidden.sort();
        hidden.dedup();
    }

    settings::set_path(&config_path()?, &default, path_name, string_list(&hidden)?)?;

    load_or_initialize()
}

pub fn module_visible(surface: &str, name: &str) -> Result<bool, String> {
    let path_name = visibility_path(surface)?;

    let (local, default) = local_settings()?;

    let hidden = boss_list(&local, &default, path_name)?;

    Ok(!hidden.iter().any(|item| item == name))
}

pub fn mark_module_update_notified(name: &str, version: &str) -> Result<BossConfig, String> {
    let (local, default) = local_settings()?;

    let mut notifications = boss_map(&local, &default, "ui.module_update_notifications")?;

    notifications.insert(name.to_string(), version.to_string());

    settings::set_path(
        &config_path()?,
        &default,
        "ui.module_update_notifications",
        string_map(&notifications)?,
    )?;

    load_or_initialize()
}

pub fn module_has_user_state(name: &str) -> Result<bool, String> {
    let (local, default) = local_settings()?;

    let disabled = boss_list(&local, &default, "ui.disabled_modules")?;

    let hidden_tray = boss_list(&local, &default, "tray.hidden_modules")?;

    let hidden_launcher = boss_list(&local, &default, "launcher.hidden_modules")?;

    Ok(disabled.iter().any(|item| item == name)
        || hidden_tray.iter().any(|item| item == name)
        || hidden_launcher.iter().any(|item| item == name))
}

pub fn remove_module_transient_state(name: &str) -> Result<BossConfig, String> {
    let (local, default) = local_settings()?;

    let mut notifications = boss_map(&local, &default, "ui.module_update_notifications")?;

    notifications.remove(name);

    settings::set_path(
        &config_path()?,
        &default,
        "ui.module_update_notifications",
        string_map(&notifications)?,
    )?;

    load_or_initialize()
}

pub fn remove_module_state(name: &str) -> Result<BossConfig, String> {
    let (local, default) = local_settings()?;

    let mut disabled = boss_list(&local, &default, "ui.disabled_modules")?;

    let mut hidden_tray = boss_list(&local, &default, "tray.hidden_modules")?;

    let mut hidden_launcher = boss_list(&local, &default, "launcher.hidden_modules")?;

    let mut notifications = boss_map(&local, &default, "ui.module_update_notifications")?;

    disabled.retain(|item| item != name);

    hidden_tray.retain(|item| item != name);

    hidden_launcher.retain(|item| item != name);

    notifications.remove(name);

    let path = config_path()?;

    settings::set_path(
        &path,
        &default,
        "ui.disabled_modules",
        string_list(&disabled)?,
    )?;

    settings::set_path(
        &path,
        &default,
        "tray.hidden_modules",
        string_list(&hidden_tray)?,
    )?;

    settings::set_path(
        &path,
        &default,
        "launcher.hidden_modules",
        string_list(&hidden_launcher)?,
    )?;

    settings::set_path(
        &path,
        &default,
        "ui.module_update_notifications",
        string_map(&notifications)?,
    )?;

    load_or_initialize()
}

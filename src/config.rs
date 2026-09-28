use crate::{languages, modules, settings};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BossConfig {
    pub language: String,
    pub tray_enabled: bool,
    pub launcher_enabled: bool,
    pub normal_notifications: bool,
    pub telemetry_enabled: bool,

    #[serde(default)]
    pub modules: BTreeMap<String, BTreeMap<String, bool>>,

    #[serde(default)]
    pub disabled_modules: Vec<String>,

    #[serde(default)]
    pub tray_item_visibility: BTreeMap<String, BTreeMap<String, bool>>,

    #[serde(default)]
    pub launcher_item_visibility: BTreeMap<String, BTreeMap<String, bool>>,

    #[serde(default)]
    pub module_update_notifications: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BossSeed {
    tray_enabled: bool,
    launcher_enabled: bool,
    normal_notifications: bool,
    telemetry_enabled: bool,
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

fn seed_path() -> Result<PathBuf, String> {
    Ok(languages::client_root()?.join("config/defaults.json"))
}

fn schema_json() -> Result<Value, String> {
    let schema = json!({
        "launcher": {
            "enabled": "true",
            "item_visibility": "{}"
        },

        "tray": {
            "enabled": "true",
            "item_visibility": "{}"
        },

        "telemetry": {
            "enabled": "false"
        },

        "modules": "{}",

        "ui": {
            "language": "es_CL",
            "normal_notifications": "true",
            "disabled_modules": "[]",
            "module_update_notifications": "{}"
        }
    });

    settings::validate_default(&schema)?;

    Ok(schema)
}

fn seed_local_json_from(seed: BossSeed, language: String) -> Result<Value, String> {
    let local = json!({
        "launcher": {
            "enabled": seed.launcher_enabled.to_string()
        },

        "tray": {
            "enabled": seed.tray_enabled.to_string()
        },

        "telemetry": {
            "enabled": seed.telemetry_enabled.to_string()
        },

        "ui": {
            "language": language,
            "normal_notifications": seed.normal_notifications.to_string()
        }
    });

    settings::validate_local_against_default(&local, &schema_json()?)?;

    Ok(local)
}

fn seed_local_json() -> Result<Value, String> {
    let path = seed_path()?;

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("could not read Boss seed {}: {error}", path.display()))?;

    let seed: BossSeed = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid Boss seed {}: {error}", path.display()))?;

    let language = languages::detect_initial_language()?;

    seed_local_json_from(seed, language)
}

fn load_or_seed_local<F>(path: &Path, seed: F) -> Result<Value, String>
where
    F: FnOnce() -> Result<Value, String>,
{
    if path.exists() {
        let local = settings::load(path)?;

        settings::validate_local_against_default(&local, &schema_json()?)?;

        return Ok(local);
    }

    let local = seed()?;

    settings::save(path, &local)?;

    Ok(local)
}

fn local_settings() -> Result<Value, String> {
    let path = config_path()?;

    load_or_seed_local(&path, seed_local_json)
}

fn local_string(local: &Value, path: &str) -> Result<Option<String>, String> {
    let mut current = local;

    for part in path.split('.') {
        let object = current
            .as_object()
            .ok_or_else(|| format!("Boss setting path '{}' crosses a non-object value", path))?;

        let Some(next) = object.get(part) else {
            return Ok(None);
        };

        current = next;
    }

    current
        .as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| format!("Boss setting '{}' does not contain a String", path))
}

fn required_string(local: &Value, path: &str) -> Result<String, String> {
    local_string(local, path)?
        .ok_or_else(|| format!("Boss settings are missing required path '{}'", path))
}

fn boss_bool(local: &Value, path: &str) -> Result<bool, String> {
    match required_string(local, path)?.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),

        value => Err(format!(
            "Boss setting '{}' contains invalid value '{}'",
            path, value
        )),
    }
}

fn boss_list(local: &Value, path: &str) -> Result<Vec<String>, String> {
    let Some(value) = local_string(local, path)? else {
        return Ok(Vec::new());
    };

    serde_json::from_str(&value).map_err(|error| {
        format!(
            "Boss setting '{}' contains invalid String data: {error}",
            path
        )
    })
}

fn boss_map(local: &Value, path: &str) -> Result<BTreeMap<String, String>, String> {
    let Some(value) = local_string(local, path)? else {
        return Ok(BTreeMap::new());
    };

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

fn surface_item_visibility(
    local: &Value,
    path: &str,
) -> Result<BTreeMap<String, BTreeMap<String, bool>>, String> {
    let Some(value) = local_string(local, path)? else {
        return Ok(BTreeMap::new());
    };

    serde_json::from_str(&value).map_err(|error| {
        format!(
            "Boss setting '{}' contains invalid surface item visibility data: {error}",
            path
        )
    })
}

fn string_surface_item_visibility(
    value: &BTreeMap<String, BTreeMap<String, bool>>,
) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!("could not serialize Boss surface item visibility: {error}"))
}

fn module_states(local: &Value) -> Result<BTreeMap<String, BTreeMap<String, bool>>, String> {
    let Some(value) = local_string(local, "modules")? else {
        return Ok(BTreeMap::new());
    };

    serde_json::from_str(&value)
        .map_err(|error| format!("Boss setting modules contains invalid String data: {error}"))
}

fn write_setting(path: &str, value: String) -> Result<String, String> {
    let file = config_path()?;

    local_settings()?;

    settings::set_local_path(&file, &schema_json()?, path, value)
}

fn write_module_states(
    modules: &BTreeMap<String, BTreeMap<String, bool>>,
) -> Result<BossConfig, String> {
    let encoded = serde_json::to_string(modules)
        .map_err(|error| format!("could not serialize Boss module object states: {error}"))?;

    write_setting("modules", encoded)?;

    load_or_initialize()
}

pub fn reconcile_module_object_states(
    name: &str,
    contract: &crate::lifecycle::LifecycleContract,
) -> Result<BTreeMap<String, bool>, String> {
    let config = load_or_initialize()?;

    let mut modules = config.modules.clone();
    let previous = modules.remove(name).unwrap_or_default();
    let mut canonical = BTreeMap::<String, bool>::new();

    for (object_id, object) in &contract.objects {
        let Some(initial_active) = object.initial_active else {
            continue;
        };

        canonical.insert(
            object_id.clone(),
            previous.get(object_id).copied().unwrap_or(initial_active),
        );
    }

    if !canonical.is_empty() {
        modules.insert(name.to_string(), canonical.clone());
    }

    if modules != config.modules {
        write_module_states(&modules)?;
    }

    Ok(canonical)
}

pub fn update_module_object_state(
    name: &str,
    object_id: &str,
    active: bool,
) -> Result<BossConfig, String> {
    let config = load_or_initialize()?;
    let mut modules = config.modules;

    let states = modules
        .get_mut(name)
        .ok_or_else(|| format!("module '{name}' has no canonical object state"))?;

    if !states.contains_key(object_id) {
        return Err(format!(
            "module '{name}' object '{object_id}' has no canonical state"
        ));
    }

    states.insert(object_id.to_string(), active);

    write_module_states(&modules)
}

pub fn load_or_initialize() -> Result<BossConfig, String> {
    let local = local_settings()?;

    Ok(BossConfig {
        language: required_string(&local, "ui.language")?,

        tray_enabled: boss_bool(&local, "tray.enabled")?,

        launcher_enabled: boss_bool(&local, "launcher.enabled")?,

        normal_notifications: boss_bool(&local, "ui.normal_notifications")?,

        telemetry_enabled: boss_bool(&local, "telemetry.enabled")?,

        modules: module_states(&local)?,

        disabled_modules: boss_list(&local, "ui.disabled_modules")?,

        tray_item_visibility: surface_item_visibility(&local, "tray.item_visibility")?,

        launcher_item_visibility: surface_item_visibility(&local, "launcher.item_visibility")?,

        module_update_notifications: boss_map(&local, "ui.module_update_notifications")?,
    })
}

pub fn settings_snapshot() -> Result<Value, String> {
    local_settings()
}

pub fn setting_value(path: &str) -> Result<String, String> {
    let local = local_settings()?;

    required_string(&local, path)
}

pub fn set_setting_value(path: &str, value: String) -> Result<String, String> {
    write_setting(path, value)
}

pub fn set_language(code: &str) -> Result<BossConfig, String> {
    let canonical = languages::resolve_language(code)?
        .ok_or_else(|| format!("unsupported N.E.E.B.L.E.S. language: {code}"))?;

    write_setting("ui.language", canonical)?;

    load_or_initialize()
}

pub fn set_bool(key: &str, value: bool) -> Result<BossConfig, String> {
    let path = match key {
        "tray_enabled" => "tray.enabled",

        "launcher_enabled" => "launcher.enabled",

        "normal_notifications" => "ui.normal_notifications",

        "telemetry_enabled" => "telemetry.enabled",

        _ => {
            return Err(format!("unknown Boss config key: {key}"));
        }
    };

    write_setting(path, value.to_string())?;

    load_or_initialize()
}

pub fn set_module_enabled(name: &str, enabled: bool) -> Result<BossConfig, String> {
    let local = local_settings()?;

    let mut modules = boss_list(&local, "ui.disabled_modules")?;

    modules.retain(|item| item != name);

    if !enabled {
        modules.push(name.to_string());

        modules.sort();
        modules.dedup();
    }

    write_setting("ui.disabled_modules", string_list(&modules)?)?;

    load_or_initialize()
}

pub fn module_enabled(name: &str) -> Result<bool, String> {
    let local = local_settings()?;

    let modules = boss_list(&local, "ui.disabled_modules")?;

    Ok(!modules.iter().any(|item| item == name))
}

fn surface_item_visibility_path(surface: &str) -> Result<&'static str, String> {
    match surface {
        "tray" => Ok("tray.item_visibility"),

        "launcher" => Ok("launcher.item_visibility"),

        _ => Err(format!(
            "unknown surface item visibility surface: {surface}"
        )),
    }
}

pub fn set_surface_item_visibility(
    surface: &str,
    module: &str,
    item_id: &str,
    visible: bool,
) -> Result<BossConfig, String> {
    let path = surface_item_visibility_path(surface)?;

    let local = local_settings()?;

    let mut visibility = surface_item_visibility(&local, path)?;

    visibility
        .entry(module.to_string())
        .or_default()
        .insert(item_id.to_string(), visible);

    write_setting(path, string_surface_item_visibility(&visibility)?)?;

    load_or_initialize()
}

pub fn surface_item_visibility_override(
    surface: &str,
    module: &str,
    item_id: &str,
) -> Result<Option<bool>, String> {
    let path = surface_item_visibility_path(surface)?;

    let local = local_settings()?;

    let visibility = surface_item_visibility(&local, path)?;

    Ok(visibility
        .get(module)
        .and_then(|items| items.get(item_id))
        .copied())
}

pub fn mark_module_update_notified(name: &str, version: &str) -> Result<BossConfig, String> {
    let local = local_settings()?;

    let mut notifications = boss_map(&local, "ui.module_update_notifications")?;

    notifications.insert(name.to_string(), version.to_string());

    write_setting(
        "ui.module_update_notifications",
        string_map(&notifications)?,
    )?;

    load_or_initialize()
}

pub fn module_has_user_state(name: &str) -> Result<bool, String> {
    let local = local_settings()?;

    let disabled = boss_list(&local, "ui.disabled_modules")?;

    let tray_visibility = surface_item_visibility(&local, "tray.item_visibility")?;

    let launcher_visibility = surface_item_visibility(&local, "launcher.item_visibility")?;

    Ok(disabled.iter().any(|item| item == name)
        || tray_visibility.contains_key(name)
        || launcher_visibility.contains_key(name))
}

pub fn remove_module_transient_state(name: &str) -> Result<BossConfig, String> {
    let local = local_settings()?;

    let mut notifications = boss_map(&local, "ui.module_update_notifications")?;

    let mut object_states = module_states(&local)?;

    notifications.remove(name);
    object_states.remove(name);

    write_setting(
        "ui.module_update_notifications",
        string_map(&notifications)?,
    )?;

    write_setting(
        "modules",
        serde_json::to_string(&object_states)
            .map_err(|error| format!("could not serialize Boss module object states: {error}"))?,
    )?;

    load_or_initialize()
}

pub fn remove_module_state(name: &str) -> Result<BossConfig, String> {
    let local = local_settings()?;

    let mut disabled = boss_list(&local, "ui.disabled_modules")?;

    let mut tray_visibility = surface_item_visibility(&local, "tray.item_visibility")?;

    let mut launcher_visibility = surface_item_visibility(&local, "launcher.item_visibility")?;

    let mut notifications = boss_map(&local, "ui.module_update_notifications")?;

    let mut object_states = module_states(&local)?;

    disabled.retain(|item| item != name);

    tray_visibility.remove(name);
    launcher_visibility.remove(name);
    notifications.remove(name);
    object_states.remove(name);

    write_setting("ui.disabled_modules", string_list(&disabled)?)?;

    write_setting(
        "tray.item_visibility",
        string_surface_item_visibility(&tray_visibility)?,
    )?;

    write_setting(
        "launcher.item_visibility",
        string_surface_item_visibility(&launcher_visibility)?,
    )?;

    write_setting(
        "ui.module_update_notifications",
        string_map(&notifications)?,
    )?;

    write_setting(
        "modules",
        serde_json::to_string(&object_states)
            .map_err(|error| format!("could not serialize Boss module object states: {error}"))?,
    )?;

    load_or_initialize()
}

#[cfg(test)]
mod boss_seed_persistence_tests {
    use super::*;

    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_path(label: &str) -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);

        std::env::temp_dir()
            .join(format!(
                "neebles-boss-seed-{}-{}-{}",
                label,
                std::process::id(),
                id
            ))
            .join("local_settings_boss.json")
    }

    #[test]
    fn fresh_birth_contains_only_base_boss_state() {
        let local = seed_local_json_from(
            BossSeed {
                tray_enabled: true,
                launcher_enabled: true,
                normal_notifications: true,
                telemetry_enabled: false,
            },
            "es_CL".to_string(),
        )
        .unwrap();

        assert_eq!(
            local_string(&local, "tray.enabled").unwrap().as_deref(),
            Some("true")
        );

        assert_eq!(
            local_string(&local, "launcher.enabled").unwrap().as_deref(),
            Some("true")
        );

        assert_eq!(
            local_string(&local, "ui.normal_notifications")
                .unwrap()
                .as_deref(),
            Some("true")
        );

        assert_eq!(
            local_string(&local, "telemetry.enabled")
                .unwrap()
                .as_deref(),
            Some("false")
        );

        assert_eq!(
            local_string(&local, "ui.language").unwrap().as_deref(),
            Some("es_CL")
        );

        assert!(local_string(&local, "modules").unwrap().is_none());

        assert!(local_string(&local, "tray.item_visibility")
            .unwrap()
            .is_none());

        assert!(local_string(&local, "launcher.item_visibility")
            .unwrap()
            .is_none());

        assert!(local_string(&local, "ui.disabled_modules")
            .unwrap()
            .is_none());

        assert!(local_string(&local, "ui.module_update_notifications")
            .unwrap()
            .is_none());
    }

    #[test]
    fn existing_local_is_never_reseeded_or_rewritten() {
        let path = temp_path("preserve-existing");

        let parent = path.parent().unwrap();

        fs::create_dir_all(parent).unwrap();

        let existing = json!({
            "launcher": {
                "enabled": "false"
            },

            "tray": {
                "enabled": "false",
                "item_visibility":
                    "{\"demo\":{\"indicator\":false}}"
            },

            "telemetry": {
                "enabled": "true"
            },

            "modules":
                "{\"demo\":{\"worker\":false}}",

            "ui": {
                "language": "en_US",
                "normal_notifications": "false",
                "disabled_modules":
                    "[\"demo\"]"
            }
        });

        settings::save(&path, &existing).unwrap();

        let before = fs::read(&path).unwrap();

        let loaded = load_or_seed_local(&path, || {
            Err("seed must not run when Boss local settings already exist".to_string())
        })
        .unwrap();

        let after = fs::read(&path).unwrap();

        assert_eq!(loaded, existing);

        assert_eq!(after, before);

        let _ = fs::remove_dir_all(parent.parent().unwrap());
    }

    #[test]
    fn full_local_writer_keeps_values_even_when_equal_to_schema() {
        let path = temp_path("full-writer");
        std::fs::create_dir_all(
            path
                .parent()
                .expect("temporary Boss settings path must have a parent"),
        )
        .expect("temporary Boss settings directory must be created");

        let local = seed_local_json_from(
            BossSeed {
                tray_enabled: true,
                launcher_enabled: true,
                normal_notifications: true,
                telemetry_enabled: false,
            },
            "es_CL".to_string(),
        )
        .unwrap();

        settings::save(&path, &local).unwrap();

        settings::set_local_path(
            &path,
            &schema_json().unwrap(),
            "tray.enabled",
            "true".to_string(),
        )
        .unwrap();

        let reloaded = settings::load(&path).unwrap();

        assert_eq!(
            local_string(&reloaded, "tray.enabled").unwrap().as_deref(),
            Some("true")
        );

        let _ = fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }
}

#[cfg(test)]
mod module_state_tests {
    use super::*;

    #[test]
    fn module_states_accept_multiple_modules_and_objects() {
        let raw = r#"{
            "module-a": {
                "indicator": true,
                "worker": false,
                "whatever": true
            },
            "module-b": {
                "overlay": false
            }
        }"#;

        let parsed: BTreeMap<String, BTreeMap<String, bool>> = serde_json::from_str(raw).unwrap();

        assert_eq!(parsed["module-a"]["indicator"], true);
        assert_eq!(parsed["module-a"]["worker"], false);
        assert_eq!(parsed["module-b"]["overlay"], false);
    }

    #[test]
    fn module_states_reject_non_boolean_object_state() {
        let raw = r#"{
            "module-a": {
                "worker": "true"
            }
        }"#;

        let parsed = serde_json::from_str::<BTreeMap<String, BTreeMap<String, bool>>>(raw);

        assert!(parsed.is_err());
    }

    #[test]
    fn module_states_round_trip_without_interpreting_object_ids() {
        let mut objects = BTreeMap::new();
        objects.insert("tray".to_string(), true);
        objects.insert("pepe".to_string(), false);
        objects.insert("renderer-x".to_string(), true);

        let mut modules = BTreeMap::new();
        modules.insert("synthetic-module".to_string(), objects);

        let encoded = serde_json::to_string(&modules).unwrap();

        let decoded: BTreeMap<String, BTreeMap<String, bool>> =
            serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded, modules);
    }
}

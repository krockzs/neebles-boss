use crate::{languages, modules, settings};
use crate::surface_content::SurfaceContentItem;
use crate::surface_projection::SurfaceRequirements;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub type FeatureInventory =
    BTreeMap<String, BTreeMap<String, SurfaceRequirements>>;

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
    pub features: FeatureInventory,

    #[serde(default)]
    pub disabled_modules: Vec<String>,

    #[serde(default)]
    pub tray_module_visibility: BTreeMap<String, bool>,

    #[serde(default)]
    pub launcher_module_visibility: BTreeMap<String, bool>,

    /*
     * Compatibility input from Boss 1.0.29.
     *
     * These maps are migration material only.
     * They are never current presentation authority.
     */
    #[serde(default, skip_serializing)]
    pub tray_item_visibility: BTreeMap<String, BTreeMap<String, bool>>,

    #[serde(default, skip_serializing)]
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
            "module_visibility": "{}",
            "item_visibility": "{}"
        },

        "tray": {
            "enabled": "true",
            "module_visibility": "{}",
            "item_visibility": "{}"
        },

        "telemetry": {
            "enabled": "false"
        },

        "modules": "{}",

        "features": "{}",

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

fn surface_module_visibility(
    local: &Value,
    path: &str,
) -> Result<BTreeMap<String, bool>, String> {
    let Some(value) = local_string(local, path)? else {
        return Ok(BTreeMap::new());
    };

    serde_json::from_str(&value).map_err(|error| {
        format!(
            "Boss setting {} contains invalid surface module visibility data: {error}",
            path
        )
    })
}

fn string_surface_module_visibility(
    value: &BTreeMap<String, bool>,
) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| {
            format!(
                "could not serialize Boss surface module visibility: {error}"
            )
        })
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

fn feature_inventory(local: &Value) -> Result<FeatureInventory, String> {
    let Some(value) = local_string(local, "features")? else {
        return Ok(BTreeMap::new());
    };

    serde_json::from_str(&value).map_err(|error| {
        format!(
            "Boss setting features contains invalid String data: {error}"
        )
    })
}

fn string_feature_inventory(
    value: &FeatureInventory,
) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!(
            "could not serialize Boss feature inventory: {error}"
        ))
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

pub(crate) fn canonical_module_features(
    name: &str,
    content: &[SurfaceContentItem],
) -> Result<BTreeMap<String, SurfaceRequirements>, String> {
    let mut canonical = BTreeMap::new();

    for item in content.iter().filter(|item| item.surface() == "ui") {
        if item.identity().owner_module() != name {
            return Err(format!(
                "feature SurfaceContent owner '{}' does not match module '{}'",
                item.identity().owner_module(),
                name
            ));
        }

        if canonical
            .insert(
                item.identity().item_id().to_string(),
                item.requirements().clone(),
            )
            .is_some()
        {
            return Err(format!(
                "duplicate feature SurfaceContent item '{}' for module '{}'",
                item.identity().item_id(),
                name
            ));
        }
    }

    Ok(canonical)
}

fn write_feature_inventory(
    features: &FeatureInventory,
) -> Result<BossConfig, String> {
    write_setting(
        "features",
        string_feature_inventory(features)?,
    )?;

    load_or_initialize()
}

fn feature_inventory_replacement(
    current: &FeatureInventory,
    canonical: &FeatureInventory,
) -> Option<FeatureInventory> {
    if current == canonical {
        None
    } else {
        Some(canonical.clone())
    }
}

pub(crate) fn replace_feature_inventory(
    canonical: &FeatureInventory,
) -> Result<BossConfig, String> {
    let config = load_or_initialize()?;

    let Some(replacement) =
        feature_inventory_replacement(&config.features, canonical)
    else {
        return Ok(config);
    };

    write_feature_inventory(&replacement)
}

pub fn reconcile_module_features(
    name: &str,
    content: &[SurfaceContentItem],
) -> Result<BTreeMap<String, SurfaceRequirements>, String> {
    let config = load_or_initialize()?;
    let canonical = canonical_module_features(name, content)?;
    let mut features = config.features.clone();

    features.remove(name);

    if !canonical.is_empty() {
        features.insert(name.to_string(), canonical.clone());
    }

    if features != config.features {
        write_feature_inventory(&features)?;
    }

    Ok(canonical)
}

pub fn remove_module_features(name: &str) -> Result<BossConfig, String> {
    let config = load_or_initialize()?;
    let mut features = config.features.clone();

    if features.remove(name).is_none() {
        return Ok(config);
    }

    write_feature_inventory(&features)
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

        features: feature_inventory(&local)?,

        disabled_modules: boss_list(&local, "ui.disabled_modules")?,

        tray_module_visibility:
            surface_module_visibility(
                &local,
                "tray.module_visibility",
            )?,

        launcher_module_visibility:
            surface_module_visibility(
                &local,
                "launcher.module_visibility",
            )?,

        tray_item_visibility:
            surface_item_visibility(
                &local,
                "tray.item_visibility",
            )?,

        launcher_item_visibility:
            surface_item_visibility(
                &local,
                "launcher.item_visibility",
            )?,

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

fn surface_module_visibility_path(
    surface: &str,
) -> Result<String, String> {
    match surface {
        "tray" =>
            Ok("tray.module_visibility".to_string()),

        "launcher" =>
            Ok("launcher.module_visibility".to_string()),

        _ => Err(format!(
            "unknown surface module visibility surface: {surface}"
        )),
    }
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

pub fn set_surface_module_visibility(
    surface: &str,
    module: &str,
    visible: bool,
) -> Result<BossConfig, String> {
    let path =
        surface_module_visibility_path(surface)?;

    let legacy_path =
        surface_item_visibility_path(surface)?;

    let local = local_settings()?;

    let mut visibility =
        surface_module_visibility(
            &local,
            &path,
        )?;

    visibility.insert(
        module.to_string(),
        visible,
    );

    write_setting(
        &path,
        string_surface_module_visibility(
            &visibility
        )?,
    )?;

    /*
     * Once this new whole-module switch is touched,
     * the legacy per-item state for this module/surface
     * is retired.
     */
    let mut legacy =
        surface_item_visibility(
            &local,
            legacy_path,
        )?;

    if legacy.remove(module).is_some() {
        write_setting(
            legacy_path,
            string_surface_item_visibility(
                &legacy
            )?,
        )?;
    }

    load_or_initialize()
}

pub fn effective_surface_module_visibility(
    config: &BossConfig,
    surface: &str,
    module: &str,
    declared_default: bool,
) -> Result<bool, String> {
    let (module_visibility, legacy_visibility) =
        match surface {
            "tray" => (
                &config.tray_module_visibility,
                &config.tray_item_visibility,
            ),

            "launcher" => (
                &config.launcher_module_visibility,
                &config.launcher_item_visibility,
            ),

            _ => {
                return Err(format!(
                    "unknown surface module visibility surface: {surface}"
                ));
            }
        };

    if let Some(visible) =
        module_visibility.get(module)
    {
        return Ok(*visible);
    }

    /*
     * Boss 1.0.29 migration:
     * any visible old item means the module is visible;
     * all old items false means the module is hidden.
     */
    if let Some(items) =
        legacy_visibility.get(module)
    {
        if !items.is_empty() {
            return Ok(
                items.values()
                    .any(|visible| *visible)
            );
        }
    }

    Ok(declared_default)
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

    let tray_module_visibility =
        surface_module_visibility(
            &local,
            "tray.module_visibility",
        )?;

    let launcher_module_visibility =
        surface_module_visibility(
            &local,
            "launcher.module_visibility",
        )?;

    let tray_legacy_visibility =
        surface_item_visibility(
            &local,
            "tray.item_visibility",
        )?;

    let launcher_legacy_visibility =
        surface_item_visibility(
            &local,
            "launcher.item_visibility",
        )?;

    Ok(
        disabled.iter().any(|item| item == name)
        || tray_module_visibility.contains_key(name)
        || launcher_module_visibility.contains_key(name)
        || tray_legacy_visibility.contains_key(name)
        || launcher_legacy_visibility.contains_key(name)
    )
}

pub fn remove_module_transient_state(name: &str) -> Result<BossConfig, String> {
    let local = local_settings()?;

    let mut notifications = boss_map(&local, "ui.module_update_notifications")?;

    let mut object_states = module_states(&local)?;

    let mut features = feature_inventory(&local)?;

    notifications.remove(name);
    object_states.remove(name);
    features.remove(name);

    write_setting(
        "ui.module_update_notifications",
        string_map(&notifications)?,
    )?;

    write_setting(
        "modules",
        serde_json::to_string(&object_states)
            .map_err(|error| format!("could not serialize Boss module object states: {error}"))?,
    )?;

    write_setting(
        "features",
        string_feature_inventory(&features)?,
    )?;

    load_or_initialize()
}

pub fn remove_module_state(name: &str) -> Result<BossConfig, String> {
    let local = local_settings()?;

    let mut disabled = boss_list(&local, "ui.disabled_modules")?;

    let mut tray_module_visibility =
        surface_module_visibility(
            &local,
            "tray.module_visibility",
        )?;

    let mut launcher_module_visibility =
        surface_module_visibility(
            &local,
            "launcher.module_visibility",
        )?;

    let mut tray_visibility =
        surface_item_visibility(
            &local,
            "tray.item_visibility",
        )?;

    let mut launcher_visibility =
        surface_item_visibility(
            &local,
            "launcher.item_visibility",
        )?;

    let mut notifications = boss_map(&local, "ui.module_update_notifications")?;

    let mut object_states = module_states(&local)?;

    let mut features = feature_inventory(&local)?;

    disabled.retain(|item| item != name);

    tray_module_visibility.remove(name);
    launcher_module_visibility.remove(name);
    tray_visibility.remove(name);
    launcher_visibility.remove(name);
    notifications.remove(name);
    object_states.remove(name);
    features.remove(name);

    write_setting("ui.disabled_modules", string_list(&disabled)?)?;

    write_setting(
        "tray.module_visibility",
        string_surface_module_visibility(
            &tray_module_visibility
        )?,
    )?;

    write_setting(
        "launcher.module_visibility",
        string_surface_module_visibility(
            &launcher_module_visibility
        )?,
    )?;

    write_setting(
        "tray.item_visibility",
        string_surface_item_visibility(
            &tray_visibility
        )?,
    )?;

    write_setting(
        "launcher.item_visibility",
        string_surface_item_visibility(
            &launcher_visibility
        )?,
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

    write_setting(
        "features",
        string_feature_inventory(&features)?,
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

        assert!(local_string(&local, "tray.module_visibility")
            .unwrap()
            .is_none());

        assert!(local_string(&local, "launcher.module_visibility")
            .unwrap()
            .is_none());

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

        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn full_local_writer_keeps_values_even_when_equal_to_schema() {
        let path = temp_path("full-writer");
        std::fs::create_dir_all(
            path.parent()
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

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}

#[cfg(test)]
mod surface_module_visibility_tests {
    use super::*;

    fn config_with_visibility(
        module_visibility: BTreeMap<String, bool>,
        legacy_visibility:
            BTreeMap<String, BTreeMap<String, bool>>,
    ) -> BossConfig {
        BossConfig {
            language: "es_CL".to_string(),
            tray_enabled: true,
            launcher_enabled: true,
            normal_notifications: true,
            telemetry_enabled: false,
            modules: BTreeMap::new(),
            features: BTreeMap::new(),
            disabled_modules: Vec::new(),
            tray_module_visibility:
                module_visibility,
            launcher_module_visibility:
                BTreeMap::new(),
            tray_item_visibility:
                legacy_visibility,
            launcher_item_visibility:
                BTreeMap::new(),
            module_update_notifications:
                BTreeMap::new(),
        }
    }

    #[test]
    fn module_visibility_override_beats_legacy_items() {
        let mut module_visibility =
            BTreeMap::new();

        module_visibility.insert(
            "demo".to_string(),
            false,
        );

        let mut items = BTreeMap::new();

        items.insert(
            "open".to_string(),
            true,
        );

        let mut legacy = BTreeMap::new();

        legacy.insert(
            "demo".to_string(),
            items,
        );

        let config = config_with_visibility(
            module_visibility,
            legacy,
        );

        assert!(
            !effective_surface_module_visibility(
                &config,
                "tray",
                "demo",
                true,
            )
            .unwrap()
        );
    }

    #[test]
    fn legacy_items_collapse_to_whole_module_visibility() {
        let mut items = BTreeMap::new();

        items.insert(
            "open".to_string(),
            false,
        );

        items.insert(
            "other".to_string(),
            true,
        );

        let mut legacy = BTreeMap::new();

        legacy.insert(
            "demo".to_string(),
            items,
        );

        let config = config_with_visibility(
            BTreeMap::new(),
            legacy,
        );

        assert!(
            effective_surface_module_visibility(
                &config,
                "tray",
                "demo",
                false,
            )
            .unwrap()
        );
    }

    #[test]
    fn absent_user_state_preserves_declared_default() {
        let config = config_with_visibility(
            BTreeMap::new(),
            BTreeMap::new(),
        );

        assert!(
            effective_surface_module_visibility(
                &config,
                "tray",
                "demo",
                true,
            )
            .unwrap()
        );

        assert!(
            !effective_surface_module_visibility(
                &config,
                "tray",
                "demo",
                false,
            )
            .unwrap()
        );
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

#[cfg(test)]
mod feature_materialization_tests {
    use super::*;

    use crate::lifecycle::LifecycleContract;
    use crate::surface_projection::{
        SurfaceProjection,
        SurfaceProjectionItem,
        SurfaceRequirementState,
    };

    #[test]
    fn features_schema_is_one_string_leaf() {
        let schema = schema_json().unwrap();

        assert_eq!(
            schema.get("features"),
            Some(&Value::String("{}".to_string()))
        );
    }

    #[test]
    fn feature_inventory_string_round_trip_preserves_requirements() {
        let expected: FeatureInventory = serde_json::from_value(
            serde_json::json!({
                "module.alpha": {
                    "notify.button": {
                        "self": "active",
                        "modules": {
                            "module.beta": "open"
                        }
                    }
                }
            })
        )
        .unwrap();

        let encoded = string_feature_inventory(&expected).unwrap();
        let local = serde_json::json!({
            "features": encoded
        });

        assert_eq!(
            feature_inventory(&local).unwrap(),
            expected
        );
    }

    #[test]
    fn feature_materialization_keeps_only_ui_surface_items() {
        let lifecycle = LifecycleContract::default();
        let requirements: SurfaceRequirements = serde_json::from_value(
            serde_json::json!({
                "self": "open",
                "modules": {
                    "module.beta": "active"
                }
            })
        )
        .unwrap();

        let mut projection = SurfaceProjection::new();

        projection
            .register_for_lifecycle(
                &lifecycle,
                SurfaceProjectionItem::with_data_and_requirements(
                    "notify.button",
                    "module.alpha",
                    "ui",
                    None,
                    None,
                    true,
                    BTreeMap::from([(
                        "control".to_string(),
                        "button".to_string(),
                    )]),
                    requirements,
                )
                .unwrap(),
            )
            .unwrap();

        projection
            .register_for_lifecycle(
                &lifecycle,
                SurfaceProjectionItem::with_data(
                    "open.launcher",
                    "module.alpha",
                    "launcher",
                    None,
                    None,
                    true,
                    BTreeMap::from([(
                        "control".to_string(),
                        "button".to_string(),
                    )]),
                )
                .unwrap(),
            )
            .unwrap();

        let content = crate::surface_content::resolve_all(&projection);
        let canonical =
            canonical_module_features("module.alpha", &content).unwrap();

        assert_eq!(canonical.len(), 1);
        assert!(!canonical.contains_key("open.launcher"));

        let require = canonical.get("notify.button").unwrap();

        assert_eq!(
            require.self_state(),
            Some(SurfaceRequirementState::Open)
        );

        assert_eq!(
            require.modules().get("module.beta"),
            Some(&SurfaceRequirementState::Active)
        );
    }

    #[test]
    fn feature_materialization_rejects_wrong_owner() {
        let lifecycle = LifecycleContract::default();
        let mut projection = SurfaceProjection::new();

        projection
            .register_for_lifecycle(
                &lifecycle,
                SurfaceProjectionItem::with_data(
                    "notify.button",
                    "module.beta",
                    "ui",
                    None,
                    None,
                    true,
                    BTreeMap::from([(
                        "control".to_string(),
                        "button".to_string(),
                    )]),
                )
                .unwrap(),
            )
            .unwrap();

        let content = crate::surface_content::resolve_all(&projection);

        let error =
            canonical_module_features("module.alpha", &content)
                .unwrap_err();

        assert!(error.contains("does not match module"));
    }

    #[test]
    fn exact_feature_inventory_replacement_drops_ghost_owner() {
        let current: FeatureInventory = BTreeMap::from([
            (
                "module.alpha".to_string(),
                BTreeMap::new(),
            ),
            (
                "ghost.module".to_string(),
                BTreeMap::new(),
            ),
        ]);

        let canonical: FeatureInventory = BTreeMap::from([(
            "module.alpha".to_string(),
            BTreeMap::new(),
        )]);

        let replacement =
            feature_inventory_replacement(&current, &canonical)
                .expect("different inventories must replace exactly");

        assert_eq!(replacement, canonical);
        assert!(!replacement.contains_key("ghost.module"));
    }
}

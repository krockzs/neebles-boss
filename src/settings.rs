use serde_json::{Map, Value};

use std::fs;
use std::path::{Path, PathBuf};

fn ensure_object(value: &Value, label: &str) -> Result<(), String> {
    if !value.is_object() {
        return Err(format!("{label} must be a JSON object"));
    }

    Ok(())
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }

    let data = serde_json::to_string_pretty(value)
        .map_err(|error| format!("could not serialize settings: {error}"))?;

    let temp = path.with_extension("json.tmp");

    fs::write(&temp, format!("{data}\n"))
        .map_err(|error| format!("could not write {}: {error}", temp.display()))?;

    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);

        format!("could not commit settings {}: {error}", path.display())
    })
}

pub fn load(path: &Path) -> Result<Value, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;

    let value: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid settings JSON {}: {error}", path.display()))?;

    ensure_object(&value, "settings")?;

    Ok(value)
}

pub fn load_or_create(path: &Path, default: &Value) -> Result<Value, String> {
    ensure_object(default, "settings default")?;

    if path.exists() {
        return load(path);
    }

    write_json(path, default)?;

    Ok(default.clone())
}

pub fn save(path: &Path, value: &Value) -> Result<(), String> {
    ensure_object(value, "settings")?;
    write_json(path, value)
}

fn merge_compatible(current: &Value, new_default: &Value) -> Value {
    match (current, new_default) {
        (Value::Object(current_map), Value::Object(default_map)) => {
            let mut result = Map::new();

            for (key, default_value) in default_map {
                let value = match current_map.get(key) {
                    Some(current_value) => match (current_value, default_value) {
                        (Value::Object(_), Value::Object(_)) => {
                            merge_compatible(current_value, default_value)
                        }

                        _ if std::mem::discriminant(current_value)
                            == std::mem::discriminant(default_value) =>
                        {
                            current_value.clone()
                        }

                        _ => default_value.clone(),
                    },

                    None => default_value.clone(),
                };

                result.insert(key.clone(), value);
            }

            Value::Object(result)
        }

        _ => new_default.clone(),
    }
}

pub fn update_from_default(path: &Path, new_default: &Value) -> Result<Value, String> {
    ensure_object(new_default, "settings default")?;

    if !path.exists() {
        write_json(path, new_default)?;
        return Ok(new_default.clone());
    }

    let current = load(path)?;
    let updated = merge_compatible(&current, new_default);

    write_json(path, &updated)?;

    Ok(updated)
}

fn same_value_kind(current: &Value, default: &Value) -> bool {
    default.is_null() || std::mem::discriminant(current) == std::mem::discriminant(default)
}

fn split_setting_path(path: &str) -> Result<Vec<&str>, String> {
    let parts = path.split('.').map(str::trim).collect::<Vec<_>>();

    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(format!("invalid settings path: {path}"));
    }

    Ok(parts)
}

pub fn get_path(value: &Value, path: &str) -> Result<Value, String> {
    let parts = split_setting_path(path)?;
    let mut current = value;

    for part in parts {
        let object = current
            .as_object()
            .ok_or_else(|| format!("settings path '{}' crosses a non-object value", path))?;

        current = object
            .get(part)
            .ok_or_else(|| format!("unknown settings path: {path}"))?;
    }

    Ok(current.clone())
}

pub fn set_path(
    file: &Path,
    default: &Value,
    path: &str,
    new_value: Value,
) -> Result<Value, String> {
    ensure_object(default, "settings default")?;

    let parts = split_setting_path(path)?;
    let mut current = load_or_create(file, default)?;

    let mut current_node = &mut current;
    let mut default_node = default;

    for (index, part) in parts.iter().enumerate() {
        let default_object = default_node
            .as_object()
            .ok_or_else(|| format!("settings path '{}' crosses a non-object default", path))?;

        let default_value = default_object
            .get(*part)
            .ok_or_else(|| format!("unknown settings path: {path}"))?;

        let is_last = index + 1 == parts.len();

        let current_object = current_node
            .as_object_mut()
            .ok_or_else(|| format!("settings path '{}' crosses a non-object value", path))?;

        if is_last {
            if !same_value_kind(&new_value, default_value) {
                return Err(format!(
                    "settings value type does not match default for path '{}'",
                    path
                ));
            }

            current_object.insert((*part).to_string(), new_value);

            save(file, &current)?;

            return Ok(current);
        }

        if !default_value.is_object() {
            return Err(format!(
                "settings path '{}' continues beyond a non-object default",
                path
            ));
        }

        if !current_object.contains_key(*part) {
            current_object.insert((*part).to_string(), default_value.clone());
        }

        current_node = current_object.get_mut(*part).unwrap();

        default_node = default_value;
    }

    Err(format!("could not update settings path: {path}"))
}

pub fn differs_from_default(current: &Value, default: &Value) -> Result<bool, String> {
    ensure_object(current, "settings")?;
    ensure_object(default, "settings default")?;

    Ok(current != default)
}

pub fn is_effectively_empty(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.values().all(is_effectively_empty),

        Value::Array(values) => values.is_empty(),

        Value::Null => true,

        Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

pub fn settings_root(neebles_root: &Path) -> PathBuf {
    neebles_root.join("shared/settings")
}

pub fn boss_settings_path(neebles_root: &Path) -> PathBuf {
    settings_root(neebles_root).join("local_settings_boss.json")
}

pub fn module_settings_path(neebles_root: &Path, module: &str) -> PathBuf {
    settings_root(neebles_root).join(format!("local_settings_{module}.json"))
}

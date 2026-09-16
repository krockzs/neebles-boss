use serde_json::{Map, Value};

use std::collections::BTreeMap;
use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

fn ensure_object(value: &Value, label: &str) -> Result<(), String> {
    if !value.is_object() {
        return Err(format!("{label} must be a JSON object"));
    }

    Ok(())
}

fn ensure_settings_directory_policy(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;

    let (uid, gid) = crate::runtime_identity::desktop_identity()?;

    let raw_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        format!(
            "settings directory path contains an invalid NUL byte: {}",
            path.display()
        )
    })?;

    let result = unsafe { libc::chown(raw_path.as_ptr(), uid, gid) };

    if result != 0 {
        return Err(format!(
            "could not assign settings directory {} to desktop user {uid}:{gid}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }

    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
        format!(
            "could not secure settings directory {}: {error}",
            path.display()
        )
    })
}

fn apply_settings_file_policy(path: &Path, parent: &Path) -> Result<(), String> {
    let parent_metadata = fs::metadata(parent)
        .map_err(|error| format!("could not inspect {}: {error}", parent.display()))?;

    let uid = parent_metadata.uid();
    let gid = parent_metadata.gid();

    let raw_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        format!(
            "settings path contains an invalid NUL byte: {}",
            path.display()
        )
    })?;

    let result = unsafe { libc::chown(raw_path.as_ptr(), uid as libc::uid_t, gid as libc::gid_t) };

    if result != 0 {
        return Err(format!(
            "could not assign settings file {} to {uid}:{gid}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("could not secure settings file {}: {error}", path.display()))
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("settings path has no parent: {}", path.display()))?;

    ensure_settings_directory_policy(parent)?;

    let data = serde_json::to_string_pretty(value)
        .map_err(|error| format!("could not serialize settings: {error}"))?;

    let temp = path.with_extension("json.tmp");

    fs::write(&temp, format!("{data}\n"))
        .map_err(|error| format!("could not write {}: {error}", temp.display()))?;

    if let Err(error) = apply_settings_file_policy(&temp, parent) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }

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

pub fn save(path: &Path, value: &Value) -> Result<(), String> {
    ensure_object(value, "settings")?;

    write_json(path, value)
}

fn validate_string_tree(value: &Value, path: &str) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };

                validate_string_tree(child, &child_path)?;
            }

            Ok(())
        }

        Value::String(_) => Ok(()),

        _ => Err(format!(
            "settings leaf '{}' must be a String",
            if path.is_empty() { "<root>" } else { path }
        )),
    }
}

pub fn validate_default(default: &Value) -> Result<(), String> {
    ensure_object(default, "settings default")?;

    validate_string_tree(default, "")?;

    if let Some(hardcoded) = default.as_object().and_then(|root| root.get("hardcoded")) {
        let hardcoded = hardcoded
            .as_object()
            .ok_or_else(|| "settings 'hardcoded' must be an object".to_string())?;

        for (key, value) in hardcoded {
            if !value.is_string() {
                return Err(format!("hardcoded '{}' must be a String", key));
            }
        }
    }

    Ok(())
}

pub fn validate_module_default(default: &Value) -> Result<(), String> {
    validate_default(default)
}

fn split_setting_path(path: &str) -> Result<Vec<&str>, String> {
    let parts = path.split('.').map(str::trim).collect::<Vec<_>>();

    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(format!("invalid settings path: {path}"));
    }

    Ok(parts)
}

fn get_path_ref<'a>(value: &'a Value, path: &str) -> Result<&'a Value, String> {
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

    Ok(current)
}

fn get_optional_path_ref<'a>(value: &'a Value, parts: &[&str]) -> Option<&'a Value> {
    let mut current = value;

    for part in parts {
        current = current.as_object()?.get(*part)?;
    }

    Some(current)
}

fn hardcoded_map(default: &Value) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();

    let Some(hardcoded) = default.as_object().and_then(|root| root.get("hardcoded")) else {
        return Ok(result);
    };

    let hardcoded = hardcoded
        .as_object()
        .ok_or_else(|| "settings 'hardcoded' must be an object".to_string())?;

    for (key, value) in hardcoded {
        let value = value
            .as_str()
            .ok_or_else(|| format!("hardcoded '{}' must be a String", key))?;

        result.insert(key.clone(), value.to_string());
    }

    Ok(result)
}

pub fn resolve_hardcoded(value: &str, hardcoded: &BTreeMap<String, String>) -> String {
    let mut resolved = value.to_string();

    for (key, replacement) in hardcoded {
        resolved = resolved.replace(&format!("${{{key}}}"), replacement);
    }

    resolved
}

fn local_template(default: &Value) -> Result<Value, String> {
    validate_default(default)?;

    let default = default
        .as_object()
        .ok_or_else(|| "settings default must be a JSON object".to_string())?;

    let mut local = Map::new();

    for (key, value) in default {
        if key == "hardcoded" {
            continue;
        }

        if value.is_object() {
            local.insert(key.clone(), Value::Object(Map::new()));
        }
    }

    Ok(Value::Object(local))
}

fn ensure_root_namespaces(local: &mut Value, default: &Value) -> Result<(), String> {
    let local = local
        .as_object_mut()
        .ok_or_else(|| "local settings must be a JSON object".to_string())?;

    let default = default
        .as_object()
        .ok_or_else(|| "settings default must be a JSON object".to_string())?;

    for (key, value) in default {
        if key == "hardcoded" {
            continue;
        }

        if value.is_object() {
            local
                .entry(key.clone())
                .or_insert_with(|| Value::Object(Map::new()));
        }
    }

    Ok(())
}

pub fn load_or_create(path: &Path, default: &Value) -> Result<Value, String> {
    validate_default(default)?;

    if path.exists() {
        let local = load(path)?;

        validate_local(&local, default)?;

        return Ok(local);
    }

    let local = local_template(default)?;

    write_json(path, &local)?;

    Ok(local)
}

fn validate_local_node(
    local: &Value,
    default: &Value,
    full_path: &str,
    root: bool,
) -> Result<(), String> {
    let local = local
        .as_object()
        .ok_or_else(|| format!("local settings '{}' must be an object", full_path))?;

    let default = default
        .as_object()
        .ok_or_else(|| format!("settings default '{}' must be an object", full_path))?;

    for (key, local_value) in local {
        if root && key == "hardcoded" {
            return Err("local settings cannot override hardcoded".to_string());
        }

        let default_value = default.get(key).ok_or_else(|| {
            if full_path.is_empty() {
                format!("unknown local settings path: {key}")
            } else {
                format!("unknown local settings path: {}.{}", full_path, key)
            }
        })?;

        let child_path = if full_path.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", full_path, key)
        };

        match (local_value, default_value) {
            (Value::Object(_), Value::Object(_)) => {
                validate_local_node(local_value, default_value, &child_path, false)?;
            }

            (Value::String(_), Value::String(_)) => {}

            (Value::String(_), _) => {
                return Err(format!(
                    "local settings path '{}' does not point to a String default",
                    child_path
                ));
            }

            _ => {
                return Err(format!(
                    "local settings leaf '{}' must be a String",
                    child_path
                ));
            }
        }
    }

    Ok(())
}

pub fn validate_local(local: &Value, default: &Value) -> Result<(), String> {
    ensure_object(local, "local settings")?;

    validate_default(default)?;

    validate_local_node(local, default, "", true)
}

pub fn get_effective_path(local: &Value, default: &Value, path: &str) -> Result<String, String> {
    validate_local(local, default)?;

    let parts = split_setting_path(path)?;

    let default_value = get_path_ref(default, path)?
        .as_str()
        .ok_or_else(|| format!("settings path '{}' does not point to a String", path))?;

    if parts.first().copied() == Some("hardcoded") {
        return Ok(default_value.to_string());
    }

    let selected = match get_optional_path_ref(local, &parts) {
        Some(value) => value
            .as_str()
            .ok_or_else(|| format!("local settings path '{}' must be a String", path))?,

        None => default_value,
    };

    let hardcoded = hardcoded_map(default)?;

    Ok(resolve_hardcoded(selected, &hardcoded))
}

fn effective_node(
    local: Option<&Value>,
    default: &Value,
    hardcoded: &BTreeMap<String, String>,
    root: bool,
) -> Result<Value, String> {
    match default {
        Value::Object(default_map) => {
            let local_map = local.and_then(Value::as_object);

            let mut result = Map::new();

            for (key, default_value) in default_map {
                if root && key == "hardcoded" {
                    result.insert(key.clone(), default_value.clone());

                    continue;
                }

                let local_value = local_map.and_then(|map| map.get(key));

                result.insert(
                    key.clone(),
                    effective_node(local_value, default_value, hardcoded, false)?,
                );
            }

            Ok(Value::Object(result))
        }

        Value::String(default_value) => {
            let selected = match local {
                Some(Value::String(value)) => value.as_str(),

                Some(_) => {
                    return Err("local settings leaf must be a String".to_string());
                }

                None => default_value.as_str(),
            };

            Ok(Value::String(resolve_hardcoded(selected, hardcoded)))
        }

        _ => Err("settings default may contain only objects and String leaves".to_string()),
    }
}

pub fn effective_settings(local: &Value, default: &Value) -> Result<Value, String> {
    validate_local(local, default)?;

    let hardcoded = hardcoded_map(default)?;

    effective_node(Some(local), default, &hardcoded, true)
}

fn insert_local_string(
    current: &mut Value,
    parts: &[&str],
    new_value: String,
    full_path: &str,
) -> Result<(), String> {
    let object = current.as_object_mut().ok_or_else(|| {
        format!(
            "settings path '{}' crosses a non-object local value",
            full_path
        )
    })?;

    if parts.len() == 1 {
        object.insert(parts[0].to_string(), Value::String(new_value));

        return Ok(());
    }

    let child = object
        .entry(parts[0].to_string())
        .or_insert_with(|| Value::Object(Map::new()));

    if !child.is_object() {
        return Err(format!(
            "settings path '{}' crosses a non-object local value",
            full_path
        ));
    }

    insert_local_string(child, &parts[1..], new_value, full_path)
}

fn remove_local_path(current: &mut Value, parts: &[&str]) -> Result<bool, String> {
    let object = current
        .as_object_mut()
        .ok_or_else(|| "local settings path crosses a non-object value".to_string())?;

    if parts.len() == 1 {
        object.remove(parts[0]);

        return Ok(object.is_empty());
    }

    if let Some(child) = object.get_mut(parts[0]) {
        if !child.is_object() {
            return Err("local settings path crosses a non-object value".to_string());
        }

        if remove_local_path(child, &parts[1..])? {
            object.remove(parts[0]);
        }
    }

    Ok(object.is_empty())
}

pub fn set_path(
    file: &Path,
    default: &Value,
    path: &str,
    new_value: String,
) -> Result<String, String> {
    validate_default(default)?;

    let parts = split_setting_path(path)?;

    if parts.first().copied() == Some("hardcoded") {
        return Err(
            "hardcoded values are declared by the module and cannot be overridden locally"
                .to_string(),
        );
    }

    let default_value = get_path_ref(default, path)?
        .as_str()
        .ok_or_else(|| format!("settings path '{}' does not point to a String", path))?;

    let mut local = load_or_create(file, default)?;

    if new_value == default_value {
        remove_local_path(&mut local, &parts)?;
    } else {
        insert_local_string(&mut local, &parts, new_value, path)?;
    }

    ensure_root_namespaces(&mut local, default)?;

    validate_local(&local, default)?;

    save(file, &local)?;

    get_effective_path(&local, default, path)
}

fn reconcile_node(local: &mut Value, default: &Value, root: bool) -> Result<(), String> {
    let local_object = local
        .as_object_mut()
        .ok_or_else(|| "local settings must be an object".to_string())?;

    let default_object = default
        .as_object()
        .ok_or_else(|| "settings default must be an object".to_string())?;

    let keys = local_object.keys().cloned().collect::<Vec<_>>();

    for key in keys {
        if root && key == "hardcoded" {
            local_object.remove(&key);
            continue;
        }

        let Some(default_value) = default_object.get(&key) else {
            local_object.remove(&key);
            continue;
        };

        let remove = match local_object.get_mut(&key) {
            Some(Value::Object(local_child)) if default_value.is_object() => {
                let mut child = Value::Object(std::mem::take(local_child));

                reconcile_node(&mut child, default_value, false)?;

                let empty = child
                    .as_object()
                    .map(|object| object.is_empty())
                    .unwrap_or(true);

                if !empty {
                    local_object.insert(key.clone(), child);
                }

                empty
            }

            Some(Value::String(local_value)) => match default_value.as_str() {
                Some(default_value) => local_value == default_value,

                None => true,
            },

            Some(_) => true,

            None => false,
        };

        if remove {
            local_object.remove(&key);
        }
    }

    Ok(())
}

pub fn update_from_default(path: &Path, new_default: &Value) -> Result<Value, String> {
    validate_default(new_default)?;

    if !path.exists() {
        let local = local_template(new_default)?;

        write_json(path, &local)?;

        return Ok(local);
    }

    let mut local = load(path)?;

    reconcile_node(&mut local, new_default, true)?;

    ensure_root_namespaces(&mut local, new_default)?;

    validate_local(&local, new_default)?;

    write_json(path, &local)?;

    Ok(local)
}

fn has_local_values_inner(value: &Value) -> Result<bool, String> {
    match value {
        Value::Object(map) => {
            for child in map.values() {
                if has_local_values_inner(child)? {
                    return Ok(true);
                }
            }

            Ok(false)
        }

        Value::String(_) => Ok(true),

        _ => Err("local settings may contain only objects and String leaves".to_string()),
    }
}

pub fn has_local_values(value: &Value) -> Result<bool, String> {
    ensure_object(value, "local settings")?;

    has_local_values_inner(value)
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

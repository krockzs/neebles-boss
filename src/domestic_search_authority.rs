use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomesticSearchAuthorityDescriptor {
    pub name: String,
    pub descriptor_path: PathBuf,
    pub domestic_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct DomesticSearchAuthorityRegistry {
    entries: BTreeMap<String, PathBuf>,
}

impl DomesticSearchAuthorityRegistry {
    pub fn load(registry_path: &Path) -> Result<Self, String> {
        if !registry_path.is_absolute() {
            return Err(format!(
                "search authority registry path must be absolute: {}",
                registry_path.display()
            ));
        }

        let raw = fs::read_to_string(registry_path).map_err(|error| {
            format!(
                "could not read search authority registry {}: {error}",
                registry_path.display()
            )
        })?;

        let value: Value = serde_json::from_str(&raw).map_err(|error| {
            format!(
                "invalid search authority registry JSON {}: {error}",
                registry_path.display()
            )
        })?;

        if value.get("schema").and_then(Value::as_str) != Some("1") {
            return Err("search authority registry schema must be String 1".to_string());
        }

        if value.get("name").and_then(Value::as_str) != Some("domestic-search-authority-registry") {
            return Err("invalid search authority registry name".to_string());
        }

        let entries = value
            .get("entries")
            .and_then(Value::as_array)
            .ok_or_else(|| "search authority registry entries must be an array".to_string())?;

        let mut result = BTreeMap::<String, PathBuf>::new();

        for entry in entries {
            let object = entry
                .as_object()
                .ok_or_else(|| "search authority registry entry must be an object".to_string())?;

            let authority_name = object
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| "search authority registry entry name must be String".to_string())?
                .trim()
                .to_string();

            if authority_name.is_empty() {
                return Err("search authority registry entry name cannot be empty".to_string());
            }

            let location = object
                .get("location")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!("search authority {authority_name} location must be String")
                })?
                .trim();

            let descriptor_path = PathBuf::from(location);

            if !descriptor_path.is_absolute() {
                return Err(format!(
                    "search authority {authority_name} descriptor location must be absolute: {}",
                    descriptor_path.display()
                ));
            }

            if result
                .insert(authority_name.clone(), descriptor_path)
                .is_some()
            {
                return Err(format!(
                    "duplicate search authority registry entry: {authority_name}"
                ));
            }
        }

        Ok(Self { entries: result })
    }

    pub fn descriptor_path(&self, authority_name: &str) -> Result<&Path, String> {
        self.entries
            .get(authority_name)
            .map(PathBuf::as_path)
            .ok_or_else(|| format!("unknown domestic search authority: {authority_name}"))
    }

    pub fn load_descriptor(
        &self,
        authority_name: &str,
    ) -> Result<DomesticSearchAuthorityDescriptor, String> {
        let descriptor_path = self.descriptor_path(authority_name)?;

        let raw = fs::read_to_string(descriptor_path).map_err(|error| {
            format!(
                "could not read domestic search authority {} at {}: {error}",
                authority_name,
                descriptor_path.display()
            )
        })?;

        let value: Value = serde_json::from_str(&raw).map_err(|error| {
            format!(
                "invalid domestic search authority JSON {}: {error}",
                descriptor_path.display()
            )
        })?;

        if value.get("schema").and_then(Value::as_str) != Some("1") {
            return Err(format!(
                "domestic search authority {authority_name} schema must be String 1"
            ));
        }

        if value.get("name").and_then(Value::as_str) != Some("domestic-search-authority") {
            return Err(format!(
                "invalid domestic search authority descriptor name for {authority_name}"
            ));
        }

        let declared_name = value
            .get("authority")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "domestic search authority descriptor {} must declare authority String",
                    descriptor_path.display()
                )
            })?
            .trim();

        if declared_name != authority_name {
            return Err(
                format!(
                    "domestic search authority identity mismatch: registry={authority_name} descriptor={declared_name}"
                )
            );
        }

        let domestic_path = value
            .get("domestic_path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "domestic search authority {authority_name} must declare domestic_path String"
                )
            })?
            .trim();

        let domestic_path = PathBuf::from(domestic_path);

        if !domestic_path.is_absolute() {
            return Err(format!(
                "domestic search authority {authority_name} domestic_path must be absolute: {}",
                domestic_path.display()
            ));
        }

        Ok(DomesticSearchAuthorityDescriptor {
            name: authority_name.to_string(),
            descriptor_path: descriptor_path.to_path_buf(),
            domestic_path,
        })
    }
}

pub fn resolve_execution_search_authorities(
    world_root: &Path,
    registry: &DomesticSearchAuthorityRegistry,
    granted_authorities: &[String],
) -> Result<Vec<PathBuf>, String> {
    let mut granted = Vec::<PathBuf>::new();

    let mut seen = BTreeSet::<String>::new();

    for authority_name in granted_authorities {
        let authority_name = authority_name.trim();

        if authority_name.is_empty() {
            return Err("granted domestic search authority cannot be empty".to_string());
        }

        if !seen.insert(authority_name.to_string()) {
            continue;
        }

        let descriptor = registry.load_descriptor(authority_name)?;

        crate::domestic_world::resolve_world_reference(
            world_root,
            world_root,
            &descriptor.domestic_path,
        )
        .map_err(|error| {
            format!(
                "domestic search authority {} is invalid for world {}: {error}",
                descriptor.name,
                world_root.display()
            )
        })?;

        granted.push(descriptor.domestic_path);
    }

    Ok(granted)
}

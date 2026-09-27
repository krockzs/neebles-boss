use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedDomesticCategory {
    pub category: String,
    pub value: String,
    pub declared_targets: Vec<String>,
    pub resolved_targets: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedDomesticWorld {
    pub world: String,
    pub root: PathBuf,
    pub categories: BTreeMap<String, ResolvedDomesticCategory>,
}

fn load_json(path: &Path, label: &str) -> Result<Value, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("could not read {label} {}: {error}", path.display()))?;

    serde_json::from_str(&raw)
        .map_err(|error| format!("invalid {label} JSON {}: {error}", path.display()))
}

fn allowed_categories(contract_path: &Path) -> Result<BTreeSet<String>, String> {
    let value = load_json(contract_path, "domestic contract")?;

    if value.get("schema").and_then(Value::as_str) != Some("1") {
        return Err("domestic contract schema must be String 1".to_string());
    }

    if value.get("name").and_then(Value::as_str) != Some("domesticacion-elfica") {
        return Err("invalid domestic contract name".to_string());
    }

    let categories = value
        .get("categories")
        .and_then(Value::as_object)
        .ok_or_else(|| "domestic categories must be an object".to_string())?;

    if categories.is_empty() {
        return Err("domestic categories cannot be empty".to_string());
    }

    let mut result = BTreeSet::<String>::new();

    for (category, value_type) in categories {
        if category.trim().is_empty() {
            return Err("domestic category cannot be empty".to_string());
        }

        if value_type.as_str() != Some("String") {
            return Err(format!("domestic category {category} must receive String"));
        }

        result.insert(category.clone());
    }

    Ok(result)
}

fn validate_relative_target(target: &Path) -> Result<(), String> {
    if target.as_os_str().is_empty() {
        return Err("domestic target cannot be empty".to_string());
    }

    if target.is_absolute() {
        return Err(format!(
            "domestic target must be relative: {}",
            target.display()
        ));
    }

    for component in target.components() {
        if matches!(component, Component::ParentDir) {
            return Err(format!(
                "domestic target cannot escape root: {}",
                target.display()
            ));
        }
    }

    Ok(())
}

fn load_target_catalog(
    target_catalog_path: &Path,
    allowed: &BTreeSet<String>,
) -> Result<BTreeMap<(String, String), Vec<String>>, String> {
    let value = load_json(target_catalog_path, "domestic target catalog")?;

    if value.get("schema").and_then(Value::as_str) != Some("1") {
        return Err("domestic target catalog schema must be String 1".to_string());
    }

    if value.get("name").and_then(Value::as_str) != Some("domesticacion-elfica-targets") {
        return Err("invalid domestic target catalog name".to_string());
    }

    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| "domestic target catalog entries must be an array".to_string())?;

    let mut result = BTreeMap::<(String, String), Vec<String>>::new();

    for entry in entries {
        let object = entry
            .as_object()
            .ok_or_else(|| "domestic target entry must be an object".to_string())?;

        let category = object
            .get("category")
            .and_then(Value::as_str)
            .ok_or_else(|| "domestic target category must be String".to_string())?
            .trim()
            .to_string();

        let logical_value = object
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| "domestic target value must be String".to_string())?
            .trim()
            .to_string();

        if category.is_empty() || logical_value.is_empty() {
            return Err("domestic target category and value cannot be empty".to_string());
        }

        if !allowed.contains(&category) {
            return Err(format!(
                "domestic target catalog uses unknown category {category}"
            ));
        }

        let raw_targets = object
            .get("targets")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                format!("domestic target {category}:{logical_value} must contain targets array")
            })?;

        if raw_targets.is_empty() {
            return Err(format!(
                "domestic target {category}:{logical_value} cannot have empty targets"
            ));
        }

        let mut targets = Vec::<String>::new();

        for target in raw_targets {
            let target = target
                .as_str()
                .ok_or_else(|| {
                    format!("domestic target {category}:{logical_value} contains non String target")
                })?
                .trim();

            let path = PathBuf::from(target);

            validate_relative_target(&path)?;

            targets.push(target.to_string());
        }

        let key = (category.clone(), logical_value.clone());

        if result.insert(key, targets).is_some() {
            return Err(format!(
                "duplicate domestic target mapping {category}:{logical_value}"
            ));
        }
    }

    Ok(result)
}

pub fn resolve_world(
    contract_path: &Path,
    grimoire_path: &Path,
    target_catalog_path: &Path,
    root: &Path,
    world_name: &str,
) -> Result<ResolvedDomesticWorld, String> {
    if !root.is_absolute() {
        return Err("domestic root must be absolute".to_string());
    }

    if !root.is_dir() {
        return Err(format!(
            "domestic root is not a directory: {}",
            root.display()
        ));
    }

    if world_name.trim().is_empty() {
        return Err("domestic world name cannot be empty".to_string());
    }

    let allowed = allowed_categories(contract_path)?;

    let catalog = load_target_catalog(target_catalog_path, &allowed)?;

    let grimoire = load_json(grimoire_path, "domestic grimoire")?;

    if grimoire.get("schema").and_then(Value::as_str) != Some("1") {
        return Err("domestic grimoire schema must be String 1".to_string());
    }

    if grimoire.get("name").and_then(Value::as_str) != Some("domesticacion-elfica") {
        return Err("invalid domestic grimoire name".to_string());
    }

    let worlds = grimoire
        .get("worlds")
        .and_then(Value::as_object)
        .ok_or_else(|| "domestic grimoire worlds must be an object".to_string())?;

    let world = worlds
        .get(world_name)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("unknown domestic world: {world_name}"))?;

    if world.is_empty() {
        return Err(format!("domestic world {world_name} cannot be empty"));
    }

    let mut resolved = BTreeMap::<String, ResolvedDomesticCategory>::new();

    for (category, raw_logical_value) in world {
        if !allowed.contains(category) {
            return Err(format!(
                "domestic world {world_name} uses unknown category {category}"
            ));
        }

        let logical_value = raw_logical_value
            .as_str()
            .ok_or_else(|| {
                format!("domestic world {world_name} category {category} must receive String")
            })?
            .trim();

        if logical_value.is_empty() {
            return Err(format!(
                "domestic world {world_name} category {category} cannot receive empty String"
            ));
        }

        let key = (category.clone(), logical_value.to_string());

        let declared_targets =
            catalog
                .get(
                    &key
                )
                .ok_or_else(|| {
                    format!(
                        "domestic world {world_name} has no target mapping for {category}:{logical_value}"
                    )
                })?
                .clone();

        let mut resolved_targets = Vec::<PathBuf>::new();

        for declared_target in &declared_targets {
            let relative = PathBuf::from(declared_target);

            validate_relative_target(&relative)?;

            let physical = crate::domestic_world::resolve_world_reference(
                root,
                root,
                &relative,
            )
            .map_err(|error| {
                format!(
                    "domestic world {world_name} could not resolve target {declared_target}: {error}"
                )
            })?;

            resolved_targets.push(physical);
        }

        resolved.insert(
            category.clone(),
            ResolvedDomesticCategory {
                category: category.clone(),

                value: logical_value.to_string(),

                declared_targets,

                resolved_targets,
            },
        );
    }

    Ok(ResolvedDomesticWorld {
        world: world_name.to_string(),

        root: root.to_path_buf(),

        categories: resolved,
    })
}

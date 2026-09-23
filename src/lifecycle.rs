use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObjectContract {
    pub activate: String,
    pub deactivate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct LifecycleContract {
    #[serde(default)]
    pub hardcoded: BTreeMap<String, String>,

    #[serde(default)]
    pub objects: BTreeMap<String, ObjectContract>,

    #[serde(default)]
    pub install: Option<String>,

    #[serde(default)]
    pub update: Option<String>,

    #[serde(default)]
    pub uninstall: Option<String>,

    #[serde(default)]
    pub enable: Option<String>,

    #[serde(default)]
    pub disable: Option<String>,

    #[serde(default)]
    pub start: Option<String>,

    #[serde(default)]
    pub stop: Option<String>,
}

fn validate_recipe(field: &str, value: &Option<String>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };

    if value.trim().is_empty() {
        return Err(format!("lifecycle field {field} cannot be empty"));
    }

    Ok(())
}

pub fn validate(contract: &LifecycleContract) -> Result<(), String> {
    for key in contract.hardcoded.keys() {
        if key.trim().is_empty() {
            return Err("lifecycle hardcoded key cannot be empty".to_string());
        }
    }

    for (object_id, object) in &contract.objects {
        if object_id.trim().is_empty() {
            return Err("lifecycle object id cannot be empty".to_string());
        }

        if object.activate.trim().is_empty() {
            return Err(format!(
                "lifecycle object {object_id} activate recipe cannot be empty"
            ));
        }

        if object.deactivate.trim().is_empty() {
            return Err(format!(
                "lifecycle object {object_id} deactivate recipe cannot be empty"
            ));
        }
    }

    validate_recipe("install", &contract.install)?;
    validate_recipe("update", &contract.update)?;
    validate_recipe("uninstall", &contract.uninstall)?;
    validate_recipe("enable", &contract.enable)?;
    validate_recipe("disable", &contract.disable)?;
    validate_recipe("start", &contract.start)?;
    validate_recipe("stop", &contract.stop)?;

    Ok(())
}

pub fn load(path: &Path) -> Result<LifecycleContract, String> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "could not read lifecycle contract {}: {error}",
            path.display()
        )
    })?;

    let contract: LifecycleContract = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid lifecycle contract {}: {error}", path.display()))?;

    validate(&contract)?;

    Ok(contract)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Result<LifecycleContract, String> {
        let contract: LifecycleContract =
            serde_json::from_str(raw).map_err(|error| error.to_string())?;

        validate(&contract)?;

        Ok(contract)
    }

    #[test]
    fn lifecycle_accepts_arbitrary_string_hardcoded_map() {
        let contract = parse(
            r#"{
                "hardcoded": {
                    "alpha": "one",
                    "anything_we_want": "two",
                    "another.symbol": "three"
                }
            }"#,
        )
        .unwrap();

        assert_eq!(contract.hardcoded.len(), 3);
        assert_eq!(
            contract.hardcoded.get("anything_we_want"),
            Some(&"two".to_string())
        );
    }

    #[test]
    fn lifecycle_accepts_arbitrary_objects() {
        let contract = parse(
            r#"{
                "objects": {
                    "indicator": {
                        "activate": "activate indicator",
                        "deactivate": "deactivate indicator"
                    },
                    "worker": {
                        "activate": "activate worker",
                        "deactivate": "deactivate worker"
                    },
                    "whatever": {
                        "activate": "activate whatever",
                        "deactivate": "deactivate whatever"
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(contract.objects.len(), 3);
        assert!(contract.objects.contains_key("indicator"));
        assert!(contract.objects.contains_key("worker"));
        assert!(contract.objects.contains_key("whatever"));
    }

    #[test]
    fn lifecycle_rejects_empty_object_id() {
        let error = parse(
            r#"{
                "objects": {
                    "": {
                        "activate": "on",
                        "deactivate": "off"
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("object id cannot be empty"));
    }

    #[test]
    fn lifecycle_rejects_empty_object_activate_recipe() {
        let error = parse(
            r#"{
                "objects": {
                    "worker": {
                        "activate": "   ",
                        "deactivate": "off"
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("activate recipe cannot be empty"));
    }

    #[test]
    fn lifecycle_rejects_empty_object_deactivate_recipe() {
        let error = parse(
            r#"{
                "objects": {
                    "worker": {
                        "activate": "on",
                        "deactivate": "   "
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("deactivate recipe cannot be empty"));
    }

    #[test]
    fn lifecycle_rejects_unknown_object_field() {
        let error = parse(
            r#"{
                "objects": {
                    "worker": {
                        "activate": "on",
                        "deactivate": "off",
                        "technology": "tray"
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("unknown field"));
    }

    #[test]
    fn lifecycle_accepts_all_known_recipe_fields() {
        let contract = parse(
            r#"{
                "hardcoded": {},
                "install": "recipe install",
                "update": "recipe update",
                "uninstall": "recipe uninstall",
                "enable": "recipe enable",
                "disable": "recipe disable",
                "start": "recipe start",
                "stop": "recipe stop"
            }"#,
        )
        .unwrap();

        assert_eq!(contract.uninstall.as_deref(), Some("recipe uninstall"));
    }

    #[test]
    fn lifecycle_allows_absent_recipes() {
        let contract = parse(
            r#"{
                "hardcoded": {}
            }"#,
        )
        .unwrap();

        assert!(contract.install.is_none());
        assert!(contract.stop.is_none());
    }

    #[test]
    fn lifecycle_rejects_empty_recipe() {
        let error = parse(
            r#"{
                "start": "   "
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("lifecycle field start cannot be empty"));
    }

    #[test]
    fn lifecycle_rejects_non_string_hardcoded_value() {
        let error = parse(
            r#"{
                "hardcoded": {
                    "alpha": 7
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("invalid type"));
    }

    #[test]
    fn lifecycle_rejects_unknown_field() {
        let error = parse(
            r#"{
                "unistall": "typo"
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("unknown field"));
    }

    #[test]
    fn lifecycle_rejects_empty_hardcoded_key() {
        let error = parse(
            r#"{
                "hardcoded": {
                    "": "value"
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("hardcoded key cannot be empty"));
    }
}

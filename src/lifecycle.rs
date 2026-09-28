use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/*
 * N.E.E.B.L.E.S. lore: operation
 *
 * Technical meaning:
 * One independently addressable execution unit inside a battleplan.
 *
 * The macro structure is owned by Lifecycle.
 * The internal String values are supplied dynamically by the module.
 * Lifecycle must not hardcode concrete technologies, Rust dependencies,
 * objective vocabularies, munition fields, tactics or intelligence fields.
 */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    /*
     * N.E.E.B.L.E.S. lore: artillery
     *
     * Technical meaning:
     * Abstract execution capability selected through the module contract.
     */
    pub artillery: String,

    /*
     * N.E.E.B.L.E.S. lore: objective
     *
     * Technical meaning:
     * Technical operation requested from the selected artillery.
     */
    pub objective: String,

    /*
     * N.E.E.B.L.E.S. lore: munition
     *
     * Technical meaning:
     * Arbitrary technical String inputs supplied by the module.
     */
    #[serde(default)]
    pub munition: BTreeMap<String, String>,

    /*
     * N.E.E.B.L.E.S. lore: tactics
     *
     * Technical meaning:
     * Arbitrary technical execution information supplied by the module.
     *
     * Lifecycle owns only the map boundary at this layer.
     */
    #[serde(default)]
    pub tactics: BTreeMap<String, String>,

    /*
     * N.E.E.B.L.E.S. lore: intelligence
     *
     * Technical meaning:
     * Arbitrary technical output-selection information supplied by the module.
     */
    #[serde(default)]
    pub intelligence: BTreeMap<String, String>,
}

/*
 * N.E.E.B.L.E.S. lore: battleplan
 *
 * Technical meaning:
 * Declarative collection of named operations for one lifecycle transition.
 *
 * The BTreeMap provides deterministic storage only.
 * Its key order is not an execution-order contract.
 */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Battleplan {
    #[serde(default)]
    pub operations: BTreeMap<String, Operation>,
}

/*
 * Technical meaning:
 * Arbitrary lifecycle transitions exposed by one module-owned object.
 *
 * Lifecycle does not prescribe a fixed object anatomy.
 */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ObjectContract {
    /*
     * Canonical functional state is optional because not every
     * Lifecycle object has boolean active/inactive semantics.
     *
     * When present, initial_active is the explicit initial truth.
     * Lifecycle never infers it from transition names.
     */
    #[serde(default)]
    pub initial_active: Option<bool>,

    /*
     * Optional canonical state consequences for arbitrary object
     * transitions.
     *
     * Key   = object transition id
     * Value = resulting canonical active state after success
     */
    #[serde(default)]
    pub transition_active: BTreeMap<String, bool>,

    #[serde(default)]
    pub transitions: BTreeMap<String, Battleplan>,
}

/*
 * Technical meaning:
 * Opaque declaration attached to one required module.
 *
 * Lifecycle owns only the relation boundary.
 * The module owns every technical String key and value inside it.
 */
pub type RequireDeclaration = BTreeMap<String, String>;

/*
 * N.E.E.B.L.E.S. lore: Lifecycle
 *
 * Technical meaning:
 * Complete declarative lifecycle contract owned by one module.
 *
 * hardcoded remains neutral N.E.E.B.L.E.S. infrastructure.
 * objects and transitions remain dynamically named maps.
 */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct LifecycleContract {
    #[serde(default)]
    pub hardcoded: BTreeMap<String, String>,

    /*
     * N.E.E.B.L.E.S. lore: require
     *
     * Technical meaning:
     * Dynamically named inter-module requirements.
     *
     * The map key identifies the required module.
     * The nested String map remains opaque module-owned information.
     *
     * Lifecycle does not prescribe versions, technologies, installers,
     * activation flags or future requirement vocabulary here.
     */
    #[serde(default)]
    pub require: BTreeMap<String, RequireDeclaration>,

    #[serde(default)]
    pub objects: BTreeMap<String, ObjectContract>,

    #[serde(default)]
    pub transitions: BTreeMap<String, Battleplan>,
}

fn validate_non_empty_identifier(kind: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("lifecycle {kind} cannot be empty"));
    }

    Ok(())
}

fn validate_string_map(
    owner: &str,
    field: &str,
    values: &BTreeMap<String, String>,
) -> Result<(), String> {
    for key in values.keys() {
        if key.trim().is_empty() {
            return Err(format!("lifecycle {owner} {field} key cannot be empty"));
        }
    }

    Ok(())
}

fn validate_operation(
    owner: &str,
    operation_id: &str,
    operation: &Operation,
) -> Result<(), String> {
    validate_non_empty_identifier("operation id", operation_id)?;

    if operation.artillery.trim().is_empty() {
        return Err(format!(
            "lifecycle {owner} operation '{operation_id}' artillery cannot be empty"
        ));
    }

    if operation.objective.trim().is_empty() {
        return Err(format!(
            "lifecycle {owner} operation '{operation_id}' objective cannot be empty"
        ));
    }

    validate_string_map(owner, "munition", &operation.munition)?;
    validate_string_map(owner, "tactics", &operation.tactics)?;
    validate_string_map(owner, "intelligence", &operation.intelligence)?;

    Ok(())
}

fn validate_battleplan(owner: &str, battleplan: &Battleplan) -> Result<(), String> {
    for (operation_id, operation) in &battleplan.operations {
        validate_operation(owner, operation_id, operation)?;
    }

    Ok(())
}

pub fn validate(contract: &LifecycleContract) -> Result<(), String> {
    for key in contract.hardcoded.keys() {
        if key.trim().is_empty() {
            return Err("lifecycle hardcoded key cannot be empty".to_string());
        }
    }

    for (module_id, declaration) in &contract.require {
        validate_non_empty_identifier("require module id", module_id)?;

        let owner = format!("require '{module_id}'");

        validate_string_map(&owner, "data", declaration)?;
    }

    for (transition_id, battleplan) in &contract.transitions {
        validate_non_empty_identifier("transition id", transition_id)?;

        let owner = format!("transition '{transition_id}'");

        validate_battleplan(&owner, battleplan)?;
    }

    for (object_id, object) in &contract.objects {
        validate_non_empty_identifier("object id", object_id)?;

        if !object.transition_active.is_empty() && object.initial_active.is_none() {
            return Err(format!(
                "lifecycle object '{object_id}' declares transition_active but has no explicit initial_active"
            ));
        }

        for transition_id in object.transition_active.keys() {
            validate_non_empty_identifier("object state transition id", transition_id)?;

            if !object.transitions.contains_key(transition_id) {
                return Err(format!(
                    "lifecycle object '{object_id}' declares state for unknown transition '{transition_id}'"
                ));
            }
        }

        for (transition_id, battleplan) in &object.transitions {
            validate_non_empty_identifier("object transition id", transition_id)?;

            let owner = format!("object '{object_id}' transition '{transition_id}'");

            validate_battleplan(&owner, battleplan)?;
        }
    }

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
    fn lifecycle_accepts_arbitrary_transition_names() {
        let contract = parse(
            r#"{
                "transitions": {
                    "install": {
                        "operations": {}
                    },
                    "open": {
                        "operations": {}
                    },
                    "whatever_future_transition": {
                        "operations": {}
                    }
                }
            }"#,
        )
        .unwrap();

        assert!(contract.transitions.contains_key("install"));
        assert!(contract.transitions.contains_key("open"));
        assert!(contract
            .transitions
            .contains_key("whatever_future_transition"));
    }

    #[test]
    fn lifecycle_accepts_arbitrary_objects_and_object_transitions() {
        let contract = parse(
            r#"{
                "objects": {
                    "indicator": {
                        "transitions": {
                            "activate": {
                                "operations": {}
                            },
                            "deactivate": {
                                "operations": {}
                            },
                            "whatever": {
                                "operations": {}
                            }
                        }
                    },
                    "worker": {
                        "transitions": {}
                    }
                }
            }"#,
        )
        .unwrap();

        assert!(contract.objects.contains_key("indicator"));
        assert!(contract.objects.contains_key("worker"));

        let indicator = contract.objects.get("indicator").unwrap();

        assert!(indicator.transitions.contains_key("activate"));
        assert!(indicator.transitions.contains_key("deactivate"));
        assert!(indicator.transitions.contains_key("whatever"));
    }

    #[test]
    fn lifecycle_accepts_dynamic_operation_maps() {
        let contract = parse(
            r#"{
                "transitions": {
                    "install": {
                        "operations": {
                            "infiltrate": {
                                "artillery": "capability.one",
                                "objective": "technical.operation",
                                "munition": {
                                    "input_a": "value-a",
                                    "input_b": "value-b"
                                },
                                "tactics": {
                                    "technical_key": "technical-value"
                                },
                                "intelligence": {
                                    "technical_output": "technical-source"
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        let operation = contract
            .transitions
            .get("install")
            .unwrap()
            .operations
            .get("infiltrate")
            .unwrap();

        assert_eq!(operation.artillery, "capability.one");
        assert_eq!(operation.objective, "technical.operation");
        assert_eq!(operation.munition.len(), 2);
        assert_eq!(operation.tactics.len(), 1);
        assert_eq!(operation.intelligence.len(), 1);
    }

    #[test]
    fn lifecycle_does_not_prescribe_artillery_or_objective_vocabulary() {
        let contract = parse(
            r#"{
                "transitions": {
                    "something": {
                        "operations": {
                            "anything": {
                                "artillery": "future.capability.that.does.not.exist.today",
                                "objective": "future.technical.objective",
                                "munition": {},
                                "tactics": {},
                                "intelligence": {}
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        let operation = contract
            .transitions
            .get("something")
            .unwrap()
            .operations
            .get("anything")
            .unwrap();

        assert_eq!(
            operation.artillery,
            "future.capability.that.does.not.exist.today"
        );

        assert_eq!(operation.objective, "future.technical.objective");
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

    #[test]
    fn lifecycle_rejects_empty_object_id() {
        let error = parse(
            r#"{
                "objects": {
                    "": {
                        "transitions": {}
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("object id cannot be empty"));
    }

    #[test]
    fn lifecycle_rejects_empty_transition_id() {
        let error = parse(
            r#"{
                "transitions": {
                    "": {
                        "operations": {}
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("transition id cannot be empty"));
    }

    #[test]
    fn lifecycle_rejects_empty_operation_id() {
        let error = parse(
            r#"{
                "transitions": {
                    "install": {
                        "operations": {
                            "": {
                                "artillery": "anything",
                                "objective": "anything"
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("operation id cannot be empty"));
    }

    #[test]
    fn lifecycle_rejects_empty_artillery() {
        let error = parse(
            r#"{
                "transitions": {
                    "install": {
                        "operations": {
                            "infiltrate": {
                                "artillery": "   ",
                                "objective": "technical.operation"
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("artillery cannot be empty"));
    }

    #[test]
    fn lifecycle_rejects_empty_objective() {
        let error = parse(
            r#"{
                "transitions": {
                    "install": {
                        "operations": {
                            "infiltrate": {
                                "artillery": "capability",
                                "objective": "   "
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("objective cannot be empty"));
    }

    #[test]
    fn lifecycle_rejects_empty_dynamic_map_key() {
        let error = parse(
            r#"{
                "transitions": {
                    "install": {
                        "operations": {
                            "infiltrate": {
                                "artillery": "capability",
                                "objective": "technical.operation",
                                "munition": {
                                    "": "value"
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("munition key cannot be empty"));
    }

    #[test]
    fn lifecycle_rejects_non_string_dynamic_map_value() {
        let error = parse(
            r#"{
                "transitions": {
                    "install": {
                        "operations": {
                            "infiltrate": {
                                "artillery": "capability",
                                "objective": "technical.operation",
                                "tactics": {
                                    "anything": 7
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("invalid type"));
    }

    #[test]
    fn lifecycle_rejects_unknown_macro_field() {
        let error = parse(
            r#"{
                "transitions": {},
                "technology": "forbidden"
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("unknown field"));
    }

    #[test]
    fn lifecycle_rejects_unknown_operation_macro_field() {
        let error = parse(
            r#"{
                "transitions": {
                    "install": {
                        "operations": {
                            "infiltrate": {
                                "artillery": "capability",
                                "objective": "technical.operation",
                                "technology": "forbidden"
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("unknown field"));
    }

    #[test]
    fn lifecycle_accepts_dynamic_require_map() {
        let contract = parse(
            r#"{
                "require": {
                    "module-alpha": {},
                    "module-beta": {
                        "future.intent": "anything",
                        "opaque.data": "value"
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(contract.require.len(), 2);

        assert_eq!(
            contract
                .require
                .get("module-beta")
                .unwrap()
                .get("future.intent"),
            Some(&"anything".to_string())
        );
    }

    #[test]
    fn lifecycle_require_accepts_empty_declaration() {
        let contract = parse(
            r#"{
                "require": {
                    "module-alpha": {}
                }
            }"#,
        )
        .unwrap();

        assert!(contract.require.get("module-alpha").unwrap().is_empty());
    }

    #[test]
    fn lifecycle_require_rejects_empty_module_id() {
        let error = parse(
            r#"{
                "require": {
                    "": {}
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("require module id cannot be empty"));
    }

    #[test]
    fn lifecycle_require_rejects_empty_dynamic_key() {
        let error = parse(
            r#"{
                "require": {
                    "module-alpha": {
                        "": "value"
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("require 'module-alpha' data key cannot be empty"));
    }

    #[test]
    fn lifecycle_require_rejects_non_string_dynamic_value() {
        let error = parse(
            r#"{
                "require": {
                    "module-alpha": {
                        "future": 7
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(error.contains("invalid type"));
    }
}

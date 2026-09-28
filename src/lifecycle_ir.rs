use std::collections::BTreeMap;

use crate::lifecycle::{Battleplan, LifecycleContract, Operation};
use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_operation::{self, PreparedOperation};

/*
 * Lifecycle intermediate representation.
 *
 * Technical responsibility:
 * Preserve declarative Battleplan data as execution IR.
 *
 * Dynamic values are materialized only when an operation is selected
 * for execution against the current Battlefield.
 *
 * This layer does not execute operations.
 * It does not interpret tactics.
 * It does not prescribe artillery or objective vocabularies.
 * It does not define execution order.
 *
 * BTreeMap provides deterministic storage only.
 */
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BattleplanIr {
    operations: BTreeMap<String, Operation>,
}

impl BattleplanIr {
    pub fn get(&self, operation_id: &str) -> Option<&Operation> {
        self.operations.get(operation_id)
    }

    pub fn contains(&self, operation_id: &str) -> bool {
        self.operations.contains_key(operation_id)
    }

    pub fn len(&self) -> usize {
        self.operations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub fn operations(&self) -> &BTreeMap<String, Operation> {
        &self.operations
    }

    pub fn prepare_operation(
        &self,
        operation_id: &str,
        battlefield: &Battlefield,
    ) -> Result<PreparedOperation, String> {
        if operation_id.trim().is_empty() {
            return Err("lifecycle IR operation id cannot be empty".to_string());
        }

        let operation = self
            .operations
            .get(operation_id)
            .ok_or_else(|| format!("lifecycle IR operation '{operation_id}' does not exist"))?;

        lifecycle_operation::prepare(operation, battlefield).map_err(|error| {
            format!("could not prepare lifecycle operation '{operation_id}': {error}")
        })
    }
}

pub fn compile_battleplan(
    battleplan: &Battleplan,
    _battlefield: &Battlefield,
) -> Result<BattleplanIr, String> {
    Ok(BattleplanIr {
        operations: battleplan.operations.clone(),
    })
}

pub fn compile_transition(
    contract: &LifecycleContract,
    transition_id: &str,
    battlefield: &Battlefield,
) -> Result<BattleplanIr, String> {
    if transition_id.trim().is_empty() {
        return Err("lifecycle transition id cannot be empty".to_string());
    }

    let battleplan = contract
        .transitions
        .get(transition_id)
        .ok_or_else(|| format!("lifecycle transition '{transition_id}' does not exist"))?;

    compile_battleplan(battleplan, battlefield)
}

pub fn compile_object_transition(
    contract: &LifecycleContract,
    object_id: &str,
    transition_id: &str,
    battlefield: &Battlefield,
) -> Result<BattleplanIr, String> {
    if object_id.trim().is_empty() {
        return Err("lifecycle object id cannot be empty".to_string());
    }

    if transition_id.trim().is_empty() {
        return Err("lifecycle object transition id cannot be empty".to_string());
    }

    let object = contract
        .objects
        .get(object_id)
        .ok_or_else(|| format!("lifecycle object '{object_id}' does not exist"))?;

    let battleplan = object.transitions.get(transition_id).ok_or_else(|| {
        format!("lifecycle object '{object_id}' transition '{transition_id}' does not exist")
    })?;

    compile_battleplan(battleplan, battlefield)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle::{ObjectContract, Operation};

    fn operation(artillery: &str, objective: &str) -> Operation {
        Operation {
            artillery: artillery.to_string(),
            objective: objective.to_string(),
            munition: BTreeMap::new(),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        }
    }

    #[test]
    fn compiles_empty_battleplan() {
        let ir = compile_battleplan(&Battleplan::default(), &Battlefield::new()).unwrap();

        assert!(ir.is_empty());
    }

    #[test]
    fn preserves_operation_identifiers() {
        let battleplan = Battleplan {
            operations: BTreeMap::from([
                (
                    "alpha".to_string(),
                    operation("capability.alpha", "objective.alpha"),
                ),
                (
                    "future-operation".to_string(),
                    operation("capability.future", "objective.future"),
                ),
            ]),
        };

        let ir = compile_battleplan(&battleplan, &Battlefield::new()).unwrap();

        assert_eq!(ir.len(), 2);
        assert!(ir.contains("alpha"));
        assert!(ir.contains("future-operation"));
    }

    #[test]
    fn preparation_resolves_operation_values_against_current_battlefield() {
        let marker = char::from_u32(36).unwrap();

        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "alpha".to_string(),
                operation(
                    &format!("{marker}routing.artillery"),
                    &format!("{marker}routing.objective"),
                ),
            )]),
        };

        let ir = compile_battleplan(&battleplan, &Battlefield::new()).unwrap();

        let declared = ir.get("alpha").unwrap();

        assert_eq!(declared.artillery, format!("{marker}routing.artillery"));

        assert_eq!(declared.objective, format!("{marker}routing.objective"));

        let mut battlefield = Battlefield::new();

        battlefield.insert("routing", "artillery", "resolved-capability");

        battlefield.insert("routing", "objective", "resolved-objective");

        let prepared = ir.prepare_operation("alpha", &battlefield).unwrap();

        assert_eq!(prepared.artillery, "resolved-capability");

        assert_eq!(prepared.objective, "resolved-objective");
    }

    #[test]
    fn preparation_reports_operation_context_on_failure() {
        let marker = char::from_u32(36).unwrap();

        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "broken-operation".to_string(),
                operation(&format!("{marker}missing.value"), "objective"),
            )]),
        };

        let ir = compile_battleplan(&battleplan, &Battlefield::new()).unwrap();

        let error = ir
            .prepare_operation("broken-operation", &Battlefield::new())
            .unwrap_err();

        assert!(error.contains("broken-operation"));

        assert!(error.contains("could not be resolved"));
    }

    #[test]
    fn compiles_arbitrary_top_level_transition() {
        let contract = LifecycleContract {
            transitions: BTreeMap::from([(
                "whatever-transition".to_string(),
                Battleplan {
                    operations: BTreeMap::from([(
                        "whatever-operation".to_string(),
                        operation("future.capability", "future.objective"),
                    )]),
                },
            )]),
            ..LifecycleContract::default()
        };

        let ir = compile_transition(&contract, "whatever-transition", &Battlefield::new()).unwrap();

        assert!(ir.contains("whatever-operation"));
    }

    #[test]
    fn rejects_unknown_top_level_transition() {
        let error = compile_transition(
            &LifecycleContract::default(),
            "missing",
            &Battlefield::new(),
        )
        .unwrap_err();

        assert!(error.contains("does not exist"));
    }

    #[test]
    fn rejects_empty_top_level_transition_id() {
        let error = compile_transition(&LifecycleContract::default(), "   ", &Battlefield::new())
            .unwrap_err();

        assert!(error.contains("transition id cannot be empty"));
    }

    #[test]
    fn compiles_arbitrary_object_transition() {
        let contract = LifecycleContract {
            objects: BTreeMap::from([(
                "future-object".to_string(),
                ObjectContract {
                    transitions: BTreeMap::from([(
                        "future-transition".to_string(),
                        Battleplan {
                            operations: BTreeMap::from([(
                                "alpha".to_string(),
                                operation("capability", "objective"),
                            )]),
                        },
                    )]),
                },
            )]),
            ..LifecycleContract::default()
        };

        let ir = compile_object_transition(
            &contract,
            "future-object",
            "future-transition",
            &Battlefield::new(),
        )
        .unwrap();

        assert!(ir.contains("alpha"));
    }

    #[test]
    fn rejects_unknown_object() {
        let error = compile_object_transition(
            &LifecycleContract::default(),
            "missing-object",
            "transition",
            &Battlefield::new(),
        )
        .unwrap_err();

        assert!(error.contains("object 'missing-object' does not exist"));
    }

    #[test]
    fn rejects_unknown_object_transition() {
        let contract = LifecycleContract {
            objects: BTreeMap::from([("object".to_string(), ObjectContract::default())]),
            ..LifecycleContract::default()
        };

        let error = compile_object_transition(
            &contract,
            "object",
            "missing-transition",
            &Battlefield::new(),
        )
        .unwrap_err();

        assert!(error.contains("missing-transition"));
    }

    #[test]
    fn rejects_empty_object_id() {
        let error = compile_object_transition(
            &LifecycleContract::default(),
            "   ",
            "transition",
            &Battlefield::new(),
        )
        .unwrap_err();

        assert!(error.contains("object id cannot be empty"));
    }

    #[test]
    fn rejects_empty_object_transition_id() {
        let contract = LifecycleContract {
            objects: BTreeMap::from([("object".to_string(), ObjectContract::default())]),
            ..LifecycleContract::default()
        };

        let error =
            compile_object_transition(&contract, "object", "   ", &Battlefield::new()).unwrap_err();

        assert!(error.contains("object transition id cannot be empty"));
    }

    #[test]
    fn ir_does_not_assign_execution_order_semantics() {
        let battleplan = Battleplan {
            operations: BTreeMap::from([
                (
                    "z-last-lexically".to_string(),
                    operation("capability.z", "objective.z"),
                ),
                (
                    "a-first-lexically".to_string(),
                    operation("capability.a", "objective.a"),
                ),
            ]),
        };

        let ir = compile_battleplan(&battleplan, &Battlefield::new()).unwrap();

        assert_eq!(ir.len(), 2);
        assert!(ir.contains("z-last-lexically"));
        assert!(ir.contains("a-first-lexically"));
    }

    #[test]
    fn ir_preserves_arbitrary_operation_vocabulary() {
        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "module-decides-everything".to_string(),
                Operation {
                    artillery: "unknown.future.artillery".to_string(),
                    objective: "unknown.future.objective".to_string(),
                    munition: BTreeMap::from([(
                        "arbitrary.input".to_string(),
                        "opaque".to_string(),
                    )]),
                    tactics: BTreeMap::from([(
                        "arbitrary.execution.info".to_string(),
                        "opaque".to_string(),
                    )]),
                    intelligence: BTreeMap::from([(
                        "arbitrary.output.info".to_string(),
                        "opaque".to_string(),
                    )]),
                },
            )]),
        };

        let ir = compile_battleplan(&battleplan, &Battlefield::new()).unwrap();

        let prepared = ir.get("module-decides-everything").unwrap();

        assert_eq!(prepared.artillery, "unknown.future.artillery");

        assert_eq!(prepared.objective, "unknown.future.objective");
    }

    #[test]
    fn compilation_keeps_dynamic_values_declarative_until_execution() {
        let marker = char::from_u32(36).unwrap();

        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "late".to_string(),
                Operation {
                    artillery: format!("{marker}runtime.artillery"),

                    objective: "objective".to_string(),

                    munition: BTreeMap::from([(
                        "input".to_string(),
                        format!("{marker}result.previous"),
                    )]),

                    tactics: BTreeMap::new(),

                    intelligence: BTreeMap::new(),
                },
            )]),
        };

        let ir = compile_battleplan(&battleplan, &Battlefield::new()).unwrap();

        let operation = ir.get("late").unwrap();

        assert_eq!(operation.artillery, format!("{marker}runtime.artillery"));

        assert_eq!(
            operation.munition.get("input"),
            Some(&format!("{marker}result.previous"))
        );
    }

    #[test]
    fn operation_is_materialized_against_current_battlefield() {
        let marker = char::from_u32(36).unwrap();

        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "late".to_string(),
                Operation {
                    artillery: format!("{marker}runtime.artillery"),

                    objective: "objective".to_string(),

                    munition: BTreeMap::from([(
                        "input".to_string(),
                        format!("{marker}result.previous"),
                    )]),

                    tactics: BTreeMap::new(),

                    intelligence: BTreeMap::new(),
                },
            )]),
        };

        let ir = compile_battleplan(&battleplan, &Battlefield::new()).unwrap();

        let mut battlefield = Battlefield::new();

        battlefield.insert("runtime", "artillery", "dynamic.artillery");

        battlefield.insert("result", "previous", "dynamic-value");

        let prepared = ir.prepare_operation("late", &battlefield).unwrap();

        assert_eq!(prepared.artillery, "dynamic.artillery");

        assert_eq!(
            prepared.munition.get("input"),
            Some(&"dynamic-value".to_string())
        );
    }
}

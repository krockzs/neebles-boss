use std::collections::BTreeMap;

use crate::lifecycle::Operation;
use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_resolver;

/*
 * Lifecycle prepared operation.
 *
 * Technical responsibility:
 * Materialize one declarative Operation against the current Battlefield.
 *
 * This layer resolves execution inputs against the current Battlefield.
 *
 * artillery, objective, munition and tactics are materialized here.
 *
 * intelligence is deliberately preserved as declarative output-selection
 * information. It belongs to post-execution result propagation and must
 * not be resolved before the operation produces its payload.
 *
 * This layer does not interpret module semantics or execution technologies.
 *
 * Map keys remain opaque and unchanged.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedOperation {
    pub module_id: Option<String>,
    pub execution_id: Option<String>,
    pub artillery: String,
    pub objective: String,
    pub munition: BTreeMap<String, String>,
    pub tactics: BTreeMap<String, String>,
    pub intelligence: BTreeMap<String, String>,
}

fn resolve_map_values(
    values: &BTreeMap<String, String>,
    battlefield: &Battlefield,
) -> Result<BTreeMap<String, String>, String> {
    values
        .iter()
        .map(|(key, value)| {
            lifecycle_resolver::resolve(value, battlefield).map(|resolved| (key.clone(), resolved))
        })
        .collect()
}

pub fn prepare(
    operation: &Operation,
    battlefield: &Battlefield,
) -> Result<PreparedOperation, String> {
    Ok(PreparedOperation {
        module_id: battlefield
            .get("module", "id")
            .map(|value| value.to_string()),
        execution_id: battlefield
            .get("execution", "id")
            .map(|value| value.to_string()),
        artillery: lifecycle_resolver::resolve(&operation.artillery, battlefield)?,
        objective: lifecycle_resolver::resolve(&operation.objective, battlefield)?,
        munition: resolve_map_values(&operation.munition, battlefield)?,
        tactics: resolve_map_values(&operation.tactics, battlefield)?,
        intelligence: operation.intelligence.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation() -> Operation {
        Operation {
            artillery: "$routing.artillery".to_string(),
            objective: "$routing.objective".to_string(),
            munition: BTreeMap::from([
                ("source".to_string(), "$input.source".to_string()),
                (
                    "destination".to_string(),
                    "prefix/$generated.path".to_string(),
                ),
            ]),
            tactics: BTreeMap::from([(
                "opaque_technical_key".to_string(),
                "$runtime.value".to_string(),
            )]),
            intelligence: BTreeMap::from([(
                "opaque_output_key".to_string(),
                "$result.value".to_string(),
            )]),
        }
    }

    fn battlefield() -> Battlefield {
        let mut battlefield = Battlefield::new();

        battlefield.insert("routing", "artillery", "future.capability");

        battlefield.insert("routing", "objective", "future.objective");

        battlefield.insert("input", "source", "/source");

        battlefield.insert("generated", "path", "/generated");

        battlefield.insert("runtime", "value", "runtime-value");

        battlefield.insert("result", "value", "result-value");

        battlefield
    }

    #[test]
    fn prepares_artillery_and_objective() {
        let prepared = prepare(&operation(), &battlefield()).unwrap();

        assert_eq!(prepared.artillery, "future.capability");

        assert_eq!(prepared.objective, "future.objective");
    }

    #[test]
    fn prepares_munition_values() {
        let prepared = prepare(&operation(), &battlefield()).unwrap();

        assert_eq!(
            prepared.munition.get("source"),
            Some(&"/source".to_string())
        );

        assert_eq!(
            prepared.munition.get("destination"),
            Some(&"prefix//generated".to_string())
        );
    }

    #[test]
    fn prepares_tactics_without_interpreting_keys() {
        let prepared = prepare(&operation(), &battlefield()).unwrap();

        assert_eq!(
            prepared.tactics.get("opaque_technical_key"),
            Some(&"runtime-value".to_string())
        );
    }

    #[test]
    fn intelligence_remains_declarative_during_preparation() {
        let marker = char::from_u32(36).unwrap();

        let prepared = prepare(&operation(), &battlefield()).unwrap();

        assert_eq!(
            prepared.intelligence.get("opaque_output_key"),
            Some(&format!("{marker}result.value"))
        );
    }

    #[test]
    fn unresolved_future_intelligence_does_not_block_preparation() {
        let marker = char::from_u32(36).unwrap();

        let operation = Operation {
            artillery: "plain-artillery".to_string(),

            objective: "plain-objective".to_string(),

            munition: BTreeMap::new(),

            tactics: BTreeMap::new(),

            intelligence: BTreeMap::from([(
                "future.output".to_string(),
                format!("{marker}result.value"),
            )]),
        };

        let prepared = prepare(&operation, &Battlefield::new()).unwrap();

        assert_eq!(
            prepared.intelligence.get("future.output"),
            Some(&format!("{marker}result.value"))
        );
    }

    #[test]
    fn plain_values_remain_plain() {
        let operation = Operation {
            artillery: "plain-artillery".to_string(),
            objective: "plain-objective".to_string(),
            munition: BTreeMap::from([("anything".to_string(), "plain-value".to_string())]),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        };

        let prepared = prepare(&operation, &Battlefield::new()).unwrap();

        assert_eq!(prepared.artillery, "plain-artillery");

        assert_eq!(prepared.objective, "plain-objective");

        assert_eq!(
            prepared.munition.get("anything"),
            Some(&"plain-value".to_string())
        );
    }

    #[test]
    fn map_keys_are_never_resolved() {
        let operation = Operation {
            artillery: "alpha".to_string(),
            objective: "one".to_string(),
            munition: BTreeMap::from([(
                "$this.key.must.remain.opaque".to_string(),
                "value".to_string(),
            )]),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        };

        let prepared = prepare(&operation, &Battlefield::new()).unwrap();

        assert!(prepared
            .munition
            .contains_key("$this.key.must.remain.opaque"));
    }

    #[test]
    fn missing_reference_fails_preparation() {
        let operation = Operation {
            artillery: "$missing.artillery".to_string(),
            objective: "objective".to_string(),
            munition: BTreeMap::new(),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        };

        let error = prepare(&operation, &Battlefield::new()).unwrap_err();

        assert!(error.contains("could not be resolved"));
    }

    #[test]
    fn prepared_operation_preserves_arbitrary_vocabulary() {
        let operation = Operation {
            artillery: "module.future.capability".to_string(),
            objective: "module.future.objective".to_string(),
            munition: BTreeMap::from([(
                "whatever.input".to_string(),
                "whatever-value".to_string(),
            )]),
            tactics: BTreeMap::from([(
                "whatever.execution.info".to_string(),
                "opaque".to_string(),
            )]),
            intelligence: BTreeMap::from([(
                "whatever.output.info".to_string(),
                "opaque".to_string(),
            )]),
        };

        let prepared = prepare(&operation, &Battlefield::new()).unwrap();

        assert_eq!(prepared.artillery, "module.future.capability");

        assert_eq!(prepared.objective, "module.future.objective");
    }

    #[test]
    fn preparation_captures_governor_module_identity() {
        let mut battlefield = battlefield();

        battlefield.insert("module", "id", "module.alpha");

        let prepared = prepare(&operation(), &battlefield).unwrap();

        assert_eq!(prepared.module_id.as_deref(), Some("module.alpha"));
    }

    #[test]
    fn preparation_captures_governor_execution_identity() {
        let mut battlefield = battlefield();

        battlefield.insert(
            "execution",
            "id",
            "module.open.41",
        );

        let prepared =
            prepare(&operation(), &battlefield).unwrap();

        assert_eq!(
            prepared.execution_id.as_deref(),
            Some("module.open.41")
        );
    }
}

use std::collections::BTreeMap;

use crate::lifecycle_communication::LifecycleCommunication;
use crate::lifecycle_result::NormalizedResult;
use crate::lifecycle_state::LifecycleRuntimeState;

/*
 * N.E.E.B.L.E.S. Lifecycle Failure Context.
 *
 * Technical responsibility:
 *
 * execution context
 *      + transition identity
 *      + optional object identity
 *      + failed operation results
 *      + require context
 *      -> structured failure namespace
 *
 * Failure Context is data.
 *
 * It does not:
 * - log
 * - emit events
 * - emit telemetry
 * - notify UI
 * - know IPC
 * - know Governor
 * - interpret module intent
 *
 * A transition may contain multiple failed operations.
 * Therefore Failure Context never invents one global operation.id.
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureContext {
    values: BTreeMap<String, String>,
}

impl FailureContext {
    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn attach(&self, communication: &mut LifecycleCommunication) -> Result<(), String> {
        communication.replace_failure(&self.values)
    }

    fn base(
        transition_id: &str,
        object_id: Option<&str>,
        state: &LifecycleRuntimeState,
    ) -> Result<BTreeMap<String, String>, String> {
        if transition_id.trim().is_empty() {
            return Err("lifecycle failure transition id cannot be empty".to_string());
        }

        if let Some(object_id) = object_id {
            if object_id.trim().is_empty() {
                return Err("lifecycle failure object id cannot be empty".to_string());
            }
        }

        let mut values = BTreeMap::new();

        if let Some(execution_id) = state.get("execution.id") {
            values.insert("execution.id".to_string(), execution_id.to_string());
        }

        values.insert("transition.id".to_string(), transition_id.to_string());

        if let Some(object_id) = object_id {
            values.insert("object.id".to_string(), object_id.to_string());
        }

        for (key, value) in state.values() {
            if key.starts_with("require.") {
                values.insert(key.clone(), value.clone());
            }
        }

        Ok(values)
    }

    pub fn from_operation_results(
        transition_id: &str,
        object_id: Option<&str>,
        state: &LifecycleRuntimeState,
        results: &BTreeMap<String, NormalizedResult>,
    ) -> Result<Self, String> {
        let mut values = Self::base(transition_id, object_id, state)?;

        let mut failed_count = 0usize;

        for (operation_id, result) in results {
            if !result.is_failure() {
                continue;
            }

            if operation_id.trim().is_empty() {
                return Err("lifecycle failure operation id cannot be empty".to_string());
            }

            let error = result.error().ok_or_else(|| {
                format!("failed operation '{operation_id}' has no normalized error")
            })?;

            failed_count += 1;

            values.insert(
                format!("operation.{operation_id}.status"),
                "failed".to_string(),
            );

            values.insert(format!("operation.{operation_id}.error"), error.to_string());

            for (key, value) in result.payload() {
                values.insert(
                    format!("operation.{operation_id}.payload.{key}"),
                    value.clone(),
                );
            }
        }

        if failed_count == 0 {
            return Err(
                "lifecycle failure context requires at least one failed operation".to_string(),
            );
        }

        values.insert("scope".to_string(), "operation".to_string());

        values.insert("status".to_string(), "failed".to_string());

        values.insert("operations.failed".to_string(), failed_count.to_string());

        Ok(Self { values })
    }

    pub fn infrastructure(
        transition_id: &str,
        object_id: Option<&str>,
        state: &LifecycleRuntimeState,
        error: &str,
    ) -> Result<Self, String> {
        if error.trim().is_empty() {
            return Err("lifecycle infrastructure failure error cannot be empty".to_string());
        }

        let mut values = Self::base(transition_id, object_id, state)?;

        values.insert("scope".to_string(), "infrastructure".to_string());

        values.insert("status".to_string(), "failed".to_string());

        values.insert("error".to_string(), error.to_string());

        Ok(Self { values })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle_execution::ExecutionPayload;

    fn runtime() -> LifecycleRuntimeState {
        LifecycleRuntimeState::new("execution-failure-001", "deploy").unwrap()
    }

    #[test]
    fn operation_failure_preserves_execution_transition_and_error() {
        let state = runtime();

        let results = BTreeMap::from([(
            "prepare".to_string(),
            NormalizedResult::failed("synthetic failure").unwrap(),
        )]);

        let failure =
            FailureContext::from_operation_results("deploy", None, &state, &results).unwrap();

        assert_eq!(failure.get("execution.id"), Some("execution-failure-001"));

        assert_eq!(failure.get("transition.id"), Some("deploy"));

        assert_eq!(failure.get("scope"), Some("operation"));

        assert_eq!(
            failure.get("operation.prepare.error"),
            Some("synthetic failure")
        );

        assert_eq!(failure.get("operations.failed"), Some("1"));
    }

    #[test]
    fn multiple_operation_failures_do_not_fake_one_current_operation() {
        let state = runtime();

        let results = BTreeMap::from([
            (
                "alpha".to_string(),
                NormalizedResult::failed("alpha failure").unwrap(),
            ),
            (
                "beta".to_string(),
                NormalizedResult::failed("beta failure").unwrap(),
            ),
        ]);

        let failure =
            FailureContext::from_operation_results("deploy", None, &state, &results).unwrap();

        assert_eq!(failure.get("operations.failed"), Some("2"));

        assert_eq!(failure.get("operation.alpha.error"), Some("alpha failure"));

        assert_eq!(failure.get("operation.beta.error"), Some("beta failure"));

        assert_eq!(failure.get("operation.id"), None);
    }

    #[test]
    fn object_identity_is_explicit_not_inferred() {
        let state = runtime();

        let results = BTreeMap::from([(
            "alpha".to_string(),
            NormalizedResult::failed("failure").unwrap(),
        )]);

        let failure = FailureContext::from_operation_results(
            "activate",
            Some("worker.alpha"),
            &state,
            &results,
        )
        .unwrap();

        assert_eq!(failure.get("object.id"), Some("worker.alpha"));
    }

    #[test]
    fn require_context_is_preserved() {
        let mut state = runtime();

        state
            .merge_namespace(
                "require",
                &BTreeMap::from([
                    ("root".to_string(), "root-module".to_string()),
                    ("current".to_string(), "dependency.alpha".to_string()),
                    (
                        "path".to_string(),
                        "root-module -> dependency.alpha".to_string(),
                    ),
                ]),
            )
            .unwrap();

        let results = BTreeMap::from([(
            "alpha".to_string(),
            NormalizedResult::failed("failure").unwrap(),
        )]);

        let failure =
            FailureContext::from_operation_results("install", None, &state, &results).unwrap();

        assert_eq!(failure.get("require.root"), Some("root-module"));

        assert_eq!(failure.get("require.current"), Some("dependency.alpha"));

        assert_eq!(
            failure.get("require.path"),
            Some("root-module -> dependency.alpha")
        );
    }

    #[test]
    fn success_cannot_be_misrepresented_as_failure() {
        let state = runtime();

        let results = BTreeMap::from([(
            "alpha".to_string(),
            NormalizedResult::succeeded(ExecutionPayload::new()),
        )]);

        let failure = FailureContext::from_operation_results("deploy", None, &state, &results);

        assert!(failure.is_err());
    }

    #[test]
    fn infrastructure_failure_is_distinct() {
        let state = runtime();

        let failure =
            FailureContext::infrastructure("deploy", None, &state, "dependency graph invalid")
                .unwrap();

        assert_eq!(failure.get("scope"), Some("infrastructure"));

        assert_eq!(failure.get("error"), Some("dependency graph invalid"));

        assert_eq!(failure.get("operation.id"), None);
    }
}

use std::collections::BTreeMap;

/*
 * N.E.E.B.L.E.S. Lifecycle Runtime State Contract.
 *
 * Technical responsibility:
 *
 * Represent one live Lifecycle execution state without coupling the
 * contract to UI, IPC, logging, telemetry or a specific module anatomy.
 *
 * State is intentionally stored as dynamic String:String data.
 *
 * Conventional technical keys currently include:
 *
 * execution.id
 * transition.id
 * phase
 * operation.id
 * object.id
 *
 * Additional namespaces are represented by dotted keys:
 *
 * require.root
 * require.current
 * require.path
 * require.depth
 * future.anything
 *
 * Presence carries meaning.
 * Absence carries meaning.
 *
 * No ownership/status booleans are required.
 *
 * Phase vocabulary is extensible. The current Lifecycle roadmap uses
 * pending, resolving, running, succeeded, failed and cancelled, but
 * this contract does not prevent future phases.
 */

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LifecycleRuntimeState {
    values: BTreeMap<String, String>,
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("lifecycle runtime state key cannot be empty".to_string());
    }

    Ok(())
}

fn validate_value(key: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!(
            "lifecycle runtime state value for '{key}' cannot be empty"
        ));
    }

    Ok(())
}

impl LifecycleRuntimeState {
    pub fn new(
        execution_id: impl Into<String>,
        transition_id: impl Into<String>,
    ) -> Result<Self, String> {
        let execution_id = execution_id.into();

        let transition_id = transition_id.into();

        let mut state = Self::default();

        state.put("execution.id", execution_id)?;

        state.put("transition.id", transition_id)?;

        state.put("phase", "pending")?;

        Ok(state)
    }

    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) -> Result<(), String> {
        let key = key.into();

        let value = value.into();

        validate_key(&key)?;

        validate_value(&key, &value)?;

        self.values.insert(key, value);

        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.values.remove(key)
    }

    pub fn phase(&self) -> Option<&str> {
        self.get("phase")
    }

    pub fn enter_phase(&mut self, phase: impl Into<String>) -> Result<(), String> {
        self.put("phase", phase)
    }

    pub fn enter_operation(&mut self, operation_id: impl Into<String>) -> Result<(), String> {
        self.put("operation.id", operation_id)
    }

    pub fn leave_operation(&mut self) -> Option<String> {
        self.remove("operation.id")
    }

    pub fn enter_object(&mut self, object_id: impl Into<String>) -> Result<(), String> {
        self.put("object.id", object_id)
    }

    pub fn leave_object(&mut self) -> Option<String> {
        self.remove("object.id")
    }

    pub fn merge_namespace(
        &mut self,
        namespace: &str,
        values: &BTreeMap<String, String>,
    ) -> Result<(), String> {
        validate_key(namespace)?;

        for (key, value) in values {
            validate_key(key)?;

            let namespaced = format!("{namespace}.{key}");

            self.put(namespaced, value.clone())?;
        }

        Ok(())
    }

    pub fn clear_namespace(&mut self, namespace: &str) -> Result<usize, String> {
        validate_key(namespace)?;

        let prefix = format!("{namespace}.");

        let keys = self
            .values
            .keys()
            .filter(|key| key.starts_with(&prefix))
            .cloned()
            .collect::<Vec<_>>();

        let count = keys.len();

        for key in keys {
            self.values.remove(&key);
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::{BTreeMap, BTreeSet};

    use crate::lifecycle::{LifecycleContract, RequireDeclaration};

    fn require_plan() -> crate::lifecycle_require::RequirePlan {
        let mut root = LifecycleContract::default();

        root.require
            .insert("B".to_string(), RequireDeclaration::new());

        let modules = BTreeMap::from([
            ("A".to_string(), root),
            ("B".to_string(), LifecycleContract::default()),
        ]);

        crate::lifecycle_require::resolve("A", BTreeSet::new(), |module_id| {
            modules
                .get(module_id)
                .cloned()
                .ok_or_else(|| format!("missing synthetic module '{module_id}'"))
        })
        .unwrap()
    }

    #[test]
    fn new_state_contains_execution_transition_and_pending_phase() {
        let state = LifecycleRuntimeState::new("execution-001", "install").unwrap();

        assert_eq!(state.get("execution.id"), Some("execution-001"));

        assert_eq!(state.get("transition.id"), Some("install"));

        assert_eq!(state.phase(), Some("pending"));
    }

    #[test]
    fn conventional_lifecycle_phases_are_supported() {
        let mut state = LifecycleRuntimeState::new("execution-001", "anything").unwrap();

        for phase in [
            "pending",
            "resolving",
            "running",
            "succeeded",
            "failed",
            "cancelled",
        ] {
            state.enter_phase(phase).unwrap();

            assert_eq!(state.phase(), Some(phase));
        }
    }

    #[test]
    fn phase_vocabulary_remains_extensible() {
        let mut state = LifecycleRuntimeState::new("execution-001", "anything").unwrap();

        state.enter_phase("future.lifecycle.phase").unwrap();

        assert_eq!(state.phase(), Some("future.lifecycle.phase"));
    }

    #[test]
    fn operation_presence_is_dynamic_membership() {
        let mut state = LifecycleRuntimeState::new("execution-001", "anything").unwrap();

        assert!(!state.contains("operation.id"));

        state.enter_operation("operation-alpha").unwrap();

        assert_eq!(state.get("operation.id"), Some("operation-alpha"));

        assert_eq!(state.leave_operation(), Some("operation-alpha".to_string()));

        assert!(!state.contains("operation.id"));
    }

    #[test]
    fn object_presence_is_dynamic_membership() {
        let mut state = LifecycleRuntimeState::new("execution-001", "activate").unwrap();

        assert!(!state.contains("object.id"));

        state.enter_object("tray-alpha").unwrap();

        assert_eq!(state.get("object.id"), Some("tray-alpha"));

        state.leave_object();

        assert!(!state.contains("object.id"));
    }

    #[test]
    fn arbitrary_future_state_fields_are_preserved() {
        let mut state = LifecycleRuntimeState::new("execution-001", "anything").unwrap();

        state
            .put("future.namespace.anything", "opaque-value")
            .unwrap();

        assert_eq!(state.get("future.namespace.anything"), Some("opaque-value"));
    }

    #[test]
    fn require_context_enters_state_without_special_require_fields() {
        let plan = require_plan();

        let context =
            crate::lifecycle_require_human::RequireHumanContext::new(&plan, "B", "resolving")
                .unwrap();

        let mut state = LifecycleRuntimeState::new("execution-001", "install").unwrap();

        state.merge_namespace("require", context.values()).unwrap();

        assert_eq!(state.get("require.root"), Some("A"));

        assert_eq!(state.get("require.current"), Some("B"));

        assert_eq!(state.get("require.path"), Some("A -> B"));

        assert_eq!(state.get("require.depth"), Some("1"));

        assert_eq!(state.get("require.action"), Some("resolving"));
    }

    #[test]
    fn require_namespace_can_be_replaced_without_touching_execution_state() {
        let mut state = LifecycleRuntimeState::new("execution-001", "install").unwrap();

        state
            .merge_namespace(
                "require",
                &BTreeMap::from([
                    ("root".to_string(), "A".to_string()),
                    ("current".to_string(), "B".to_string()),
                ]),
            )
            .unwrap();

        let removed = state.clear_namespace("require").unwrap();

        assert_eq!(removed, 2);

        assert_eq!(state.get("execution.id"), Some("execution-001"));

        assert_eq!(state.get("transition.id"), Some("install"));

        assert!(!state.contains("require.root"));
    }

    #[test]
    fn empty_state_key_is_rejected() {
        let mut state = LifecycleRuntimeState::new("execution-001", "anything").unwrap();

        let error = state.put("   ", "value").unwrap_err();

        assert!(error.contains("key cannot be empty"));
    }

    #[test]
    fn empty_state_value_is_rejected() {
        let mut state = LifecycleRuntimeState::new("execution-001", "anything").unwrap();

        let error = state.put("future.key", "   ").unwrap_err();

        assert!(error.contains("cannot be empty"));
    }

    #[test]
    fn merge_namespace_is_generic() {
        let mut state = LifecycleRuntimeState::new("execution-001", "anything").unwrap();

        state
            .merge_namespace(
                "arbitrary",
                &BTreeMap::from([
                    ("one".to_string(), "1".to_string()),
                    ("two".to_string(), "2".to_string()),
                ]),
            )
            .unwrap();

        assert_eq!(state.get("arbitrary.one"), Some("1"));

        assert_eq!(state.get("arbitrary.two"), Some("2"));
    }
}

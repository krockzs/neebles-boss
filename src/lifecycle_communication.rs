use std::collections::BTreeMap;

use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_human::{self, HumanMessageTemplate, ResolvedHumanMessage};
use crate::lifecycle_state::LifecycleRuntimeState;

/*
 * N.E.E.B.L.E.S. Lifecycle Communication Contract.
 *
 * Lifecycle owns structured communication.
 * UI and transports only consume it later.
 *
 * Dynamic namespaces currently prepared:
 *
 * state.*
 * human.*
 * progress.*
 * result.*
 * failure.*
 *
 * Result semantics belong to step 19.
 * Failure semantics belong to step 22.
 *
 * This layer only provides the communication substrate.
 */

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LifecycleCommunication {
    values: BTreeMap<String, String>,
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("lifecycle communication key cannot be empty".to_string());
    }

    Ok(())
}

fn validate_value(key: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!(
            "lifecycle communication value for '{key}' cannot be empty"
        ));
    }

    Ok(())
}

fn apply_runtime_state(state: &LifecycleRuntimeState, battlefield: &mut Battlefield) {
    for (key, value) in state.values() {
        if let Some((namespace, path)) = key.split_once('.') {
            battlefield.insert(namespace, path, value.clone());
        } else {
            battlefield.insert("state", key, value.clone());
        }
    }
}

impl LifecycleCommunication {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_state(state: &LifecycleRuntimeState) -> Result<Self, String> {
        let mut communication = Self::new();

        communication.refresh_state(state)?;

        Ok(communication)
    }

    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
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

    pub fn replace_namespace(
        &mut self,
        namespace: &str,
        values: &BTreeMap<String, String>,
    ) -> Result<(), String> {
        self.clear_namespace(namespace)?;

        for (key, value) in values {
            self.put(format!("{namespace}.{key}"), value.clone())?;
        }

        Ok(())
    }

    pub fn refresh_state(&mut self, state: &LifecycleRuntimeState) -> Result<(), String> {
        self.clear_namespace("state")?;

        for (key, value) in state.values() {
            self.put(format!("state.{key}"), value.clone())?;
        }

        Ok(())
    }

    pub fn attach_human(&mut self, human: &ResolvedHumanMessage) -> Result<(), String> {
        self.put("human.message", human.message())?;

        if let Some(detail) = human.detail() {
            self.put("human.detail", detail)?;
        } else {
            self.remove("human.detail");
        }

        Ok(())
    }

    pub fn replace_progress(&mut self, values: &BTreeMap<String, String>) -> Result<(), String> {
        self.replace_namespace("progress", values)
    }

    fn replace_opaque_namespace(
        &mut self,
        namespace: &str,
        values: &BTreeMap<String, String>,
    ) -> Result<(), String> {
        validate_key(namespace)?;

        self.clear_namespace(namespace)?;

        for (key, value) in values {
            validate_key(key)?;

            self.values
                .insert(format!("{namespace}.{key}"), value.clone());
        }

        Ok(())
    }

    pub fn replace_result(&mut self, values: &BTreeMap<String, String>) -> Result<(), String> {
        self.replace_opaque_namespace("result", values)
    }

    pub fn replace_failure(&mut self, values: &BTreeMap<String, String>) -> Result<(), String> {
        self.replace_namespace("failure", values)
    }
}

pub fn resolve_human(
    template: &HumanMessageTemplate,
    state: &LifecycleRuntimeState,
    battlefield: &Battlefield,
) -> Result<ResolvedHumanMessage, String> {
    let mut scoped = battlefield.clone();

    apply_runtime_state(state, &mut scoped);

    lifecycle_human::resolve(template, &scoped)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::{BTreeMap, BTreeSet};

    use crate::lifecycle::{LifecycleContract, RequireDeclaration};

    fn reference(namespace: &str, path: &str) -> String {
        let marker = char::from_u32(36).unwrap();

        format!("{marker}{namespace}.{path}")
    }

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
    fn communication_snapshots_runtime_state() {
        let mut state = LifecycleRuntimeState::new("execution-001", "install").unwrap();

        state.enter_phase("running").unwrap();

        state.enter_operation("download").unwrap();

        let communication = LifecycleCommunication::from_state(&state).unwrap();

        assert_eq!(
            communication.get("state.execution.id"),
            Some("execution-001")
        );

        assert_eq!(communication.get("state.transition.id"), Some("install"));

        assert_eq!(communication.get("state.phase"), Some("running"));

        assert_eq!(communication.get("state.operation.id"), Some("download"));
    }

    #[test]
    fn communication_state_can_be_refreshed() {
        let mut state = LifecycleRuntimeState::new("execution-001", "install").unwrap();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        state.enter_phase("running").unwrap();

        state.enter_operation("extract").unwrap();

        communication.refresh_state(&state).unwrap();

        assert_eq!(communication.get("state.phase"), Some("running"));

        assert_eq!(communication.get("state.operation.id"), Some("extract"));
    }

    #[test]
    fn human_message_resolves_against_live_runtime_state() {
        let mut state = LifecycleRuntimeState::new("execution-001", "install").unwrap();

        state.enter_phase("running").unwrap();

        state.enter_operation("download").unwrap();

        let transition = reference("transition", "id");

        let operation = reference("operation", "id");

        let execution = reference("execution", "id");

        let phase = reference("state", "phase");

        let template = HumanMessageTemplate::new(format!("{transition}: {operation}"))
            .unwrap()
            .detail(format!("{execution} {phase}"))
            .unwrap();

        let resolved = resolve_human(&template, &state, &Battlefield::new()).unwrap();

        assert_eq!(resolved.message(), "install: download");

        assert_eq!(resolved.detail(), Some("execution-001 running"));
    }

    #[test]
    fn require_context_flows_state_to_human_to_communication() {
        let plan = require_plan();

        let require =
            crate::lifecycle_require_human::RequireHumanContext::new(&plan, "B", "resolving")
                .unwrap();

        let mut state = LifecycleRuntimeState::new("execution-001", "install").unwrap();

        state.merge_namespace("require", require.values()).unwrap();

        let current = reference("require", "current");

        let path = reference("require", "path");

        let template = HumanMessageTemplate::new(format!("Resolving {current}"))
            .unwrap()
            .detail(path)
            .unwrap();

        let resolved = resolve_human(&template, &state, &Battlefield::new()).unwrap();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        communication.attach_human(&resolved).unwrap();

        assert_eq!(communication.get("state.require.current"), Some("B"));

        assert_eq!(communication.get("human.message"), Some("Resolving B"));

        assert_eq!(communication.get("human.detail"), Some("A -> B"));
    }

    #[test]
    fn root_require_parent_is_absent_not_empty() {
        let modules = BTreeMap::from([("A".to_string(), LifecycleContract::default())]);

        let plan = crate::lifecycle_require::resolve("A", BTreeSet::new(), |module_id| {
            modules
                .get(module_id)
                .cloned()
                .ok_or_else(|| format!("missing synthetic module '{module_id}'"))
        })
        .unwrap();

        let require =
            crate::lifecycle_require_human::RequireHumanContext::new(&plan, "A", "complete")
                .unwrap();

        assert_eq!(require.get("required_by"), None);

        let mut state = LifecycleRuntimeState::new("execution-root", "install").unwrap();

        state.merge_namespace("require", require.values()).unwrap();

        assert!(!state.contains("require.required_by"));
    }

    #[test]
    fn progress_result_and_failure_are_dynamic_namespaces() {
        let mut communication = LifecycleCommunication::new();

        communication
            .replace_progress(&BTreeMap::from([
                ("current".to_string(), "3".to_string()),
                ("total".to_string(), "7".to_string()),
                ("unit".to_string(), "modules".to_string()),
            ]))
            .unwrap();

        communication
            .replace_result(&BTreeMap::from([(
                "future.output".to_string(),
                "opaque".to_string(),
            )]))
            .unwrap();

        communication
            .replace_failure(&BTreeMap::from([(
                "future.context".to_string(),
                "opaque".to_string(),
            )]))
            .unwrap();

        assert_eq!(communication.get("progress.current"), Some("3"));

        assert_eq!(communication.get("result.future.output"), Some("opaque"));

        assert_eq!(communication.get("failure.future.context"), Some("opaque"));
    }

    #[test]
    fn resolving_human_does_not_mutate_base_battlefield() {
        let state = LifecycleRuntimeState::new("execution-001", "anything").unwrap();

        let mut battlefield = Battlefield::new();

        battlefield.insert("runtime", "kept", "yes");

        let execution = reference("execution", "id");

        let template = HumanMessageTemplate::new(execution).unwrap();

        let resolved = resolve_human(&template, &state, &battlefield).unwrap();

        assert_eq!(resolved.message(), "execution-001");

        assert_eq!(battlefield.get("runtime", "kept",), Some("yes"));

        assert!(!battlefield.contains_namespace("execution"));
    }

    #[test]
    fn human_detail_uses_presence_not_empty_placeholder() {
        let state = LifecycleRuntimeState::new("execution-001", "anything").unwrap();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        let first_template = HumanMessageTemplate::new("one")
            .unwrap()
            .detail("detail")
            .unwrap();

        let first = resolve_human(&first_template, &state, &Battlefield::new()).unwrap();

        communication.attach_human(&first).unwrap();

        let second_template = HumanMessageTemplate::new("two").unwrap();

        let second = resolve_human(&second_template, &state, &Battlefield::new()).unwrap();

        communication.attach_human(&second).unwrap();

        assert_eq!(communication.get("human.message"), Some("two"));

        assert_eq!(communication.get("human.detail"), None);
    }
}

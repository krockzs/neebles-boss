use std::collections::BTreeMap;

use crate::lifecycle::LifecycleContract;

/*
 * Lifecycle generic object state store.
 *
 * Technical responsibility:
 * Track explicit boolean state for zero, one or many arbitrary
 * Lifecycle objects.
 *
 * This layer does not:
 * - infer an initial state
 * - execute object transitions
 * - interpret object identity
 * - know UI semantics
 * - persist state
 *
 * Object existence comes from LifecycleContract.
 * Initial state is always supplied explicitly by the caller.
 */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectState {
    active: bool,
}

impl ObjectState {
    pub fn new(active: bool) -> Self {
        Self { active }
    }

    pub fn active(&self) -> bool {
        self.active
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectStateStore {
    states: BTreeMap<String, ObjectState>,
}

impl ObjectStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        contract: &LifecycleContract,
        object_id: impl Into<String>,
        active: bool,
    ) -> Result<(), String> {
        let object_id = object_id.into();

        if object_id.trim().is_empty() {
            return Err("lifecycle object id cannot be empty".to_string());
        }

        if !contract.objects.contains_key(&object_id) {
            return Err(format!("lifecycle object '{object_id}' does not exist"));
        }

        if self.states.contains_key(&object_id) {
            return Err(format!(
                "lifecycle object '{object_id}' state is already registered"
            ));
        }

        self.states.insert(object_id, ObjectState::new(active));

        Ok(())
    }

    pub fn contains(&self, object_id: &str) -> bool {
        self.states.contains_key(object_id)
    }

    pub fn get(&self, object_id: &str) -> Option<ObjectState> {
        self.states.get(object_id).copied()
    }

    pub fn active(&self, object_id: &str) -> Result<bool, String> {
        self.states
            .get(object_id)
            .map(ObjectState::active)
            .ok_or_else(|| format!("lifecycle object '{object_id}' state is not registered"))
    }

    pub fn update(&mut self, object_id: &str, active: bool) -> Result<(), String> {
        let state = self
            .states
            .get_mut(object_id)
            .ok_or_else(|| format!("lifecycle object '{object_id}' state is not registered"))?;

        *state = ObjectState::new(active);

        Ok(())
    }

    pub fn unregister(&mut self, object_id: &str) -> Option<ObjectState> {
        self.states.remove(object_id)
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn states(&self) -> &BTreeMap<String, ObjectState> {
        &self.states
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle::ObjectContract;

    fn contract_with_objects(ids: &[&str]) -> LifecycleContract {
        let mut contract = LifecycleContract::default();

        for id in ids {
            contract
                .objects
                .insert(id.to_string(), ObjectContract::default());
        }

        contract
    }

    #[test]
    fn zero_objects_is_valid() {
        let store = ObjectStateStore::new();

        assert!(store.is_empty());

        assert_eq!(store.len(), 0);
    }

    #[test]
    fn one_object_can_be_registered() {
        let contract = contract_with_objects(&["alpha"]);

        let mut store = ObjectStateStore::new();

        store.register(&contract, "alpha", true).unwrap();

        assert_eq!(store.len(), 1);

        assert!(store.active("alpha").unwrap());
    }

    #[test]
    fn many_objects_can_be_registered() {
        let contract =
            contract_with_objects(&["alpha", "beta", "future-object", "anything.module.owns"]);

        let mut store = ObjectStateStore::new();

        store.register(&contract, "alpha", true).unwrap();

        store.register(&contract, "beta", false).unwrap();

        store.register(&contract, "future-object", true).unwrap();

        store
            .register(&contract, "anything.module.owns", false)
            .unwrap();

        assert_eq!(store.len(), 4);
    }

    #[test]
    fn arbitrary_object_identifiers_are_preserved() {
        let id = "future.module.object.whatever";

        let contract = contract_with_objects(&[id]);

        let mut store = ObjectStateStore::new();

        store.register(&contract, id, true).unwrap();

        assert!(store.contains(id));
    }

    #[test]
    fn unknown_contract_object_is_rejected() {
        let contract = LifecycleContract::default();

        let mut store = ObjectStateStore::new();

        let error = store.register(&contract, "missing", true).unwrap_err();

        assert!(error.contains("does not exist"));
    }

    #[test]
    fn empty_object_identifier_is_rejected() {
        let contract = LifecycleContract::default();

        let mut store = ObjectStateStore::new();

        let error = store.register(&contract, "   ", true).unwrap_err();

        assert!(error.contains("cannot be empty"));
    }

    #[test]
    fn duplicate_state_registration_is_rejected() {
        let contract = contract_with_objects(&["alpha"]);

        let mut store = ObjectStateStore::new();

        store.register(&contract, "alpha", true).unwrap();

        let error = store.register(&contract, "alpha", false).unwrap_err();

        assert!(error.contains("already registered"));
    }

    #[test]
    fn initial_state_is_explicit_and_not_inferred() {
        let contract = contract_with_objects(&["active-object", "inactive-object"]);

        let mut store = ObjectStateStore::new();

        store.register(&contract, "active-object", true).unwrap();

        store.register(&contract, "inactive-object", false).unwrap();

        assert!(store.active("active-object").unwrap());

        assert!(!store.active("inactive-object").unwrap());
    }

    #[test]
    fn object_state_can_change_without_affecting_siblings() {
        let contract = contract_with_objects(&["alpha", "beta"]);

        let mut store = ObjectStateStore::new();

        store.register(&contract, "alpha", false).unwrap();

        store.register(&contract, "beta", false).unwrap();

        store.update("alpha", true).unwrap();

        assert!(store.active("alpha").unwrap());

        assert!(!store.active("beta").unwrap());
    }

    #[test]
    fn updating_unknown_state_is_rejected() {
        let mut store = ObjectStateStore::new();

        let error = store.update("missing", true).unwrap_err();

        assert!(error.contains("not registered"));
    }

    #[test]
    fn object_state_can_be_unregistered() {
        let contract = contract_with_objects(&["alpha"]);

        let mut store = ObjectStateStore::new();

        store.register(&contract, "alpha", true).unwrap();

        let removed = store.unregister("alpha");

        assert_eq!(removed, Some(ObjectState::new(true)));

        assert!(store.is_empty());
    }

    #[test]
    fn contract_objects_are_not_automatically_registered() {
        let contract = contract_with_objects(&["alpha", "beta"]);

        let store = ObjectStateStore::new();

        assert_eq!(contract.objects.len(), 2);

        assert!(store.is_empty());
    }

    #[test]
    fn state_store_does_not_require_object_transition_vocabulary() {
        let id = "object-with-arbitrary-transitions";

        let contract = contract_with_objects(&[id]);

        let mut store = ObjectStateStore::new();

        store.register(&contract, id, true).unwrap();

        assert!(store.active(id).unwrap());
    }
}

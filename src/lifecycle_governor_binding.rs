use crate::lifecycle::LifecycleContract;

/*
 * N.E.E.B.L.E.S. Lifecycle Governor binding.
 *
 * Technical responsibility:
 *
 * Resolve one Governor-owned action into one module-owned Lifecycle
 * transition without imposing transition vocabulary on modules.
 *
 * Example:
 *
 *     hardcoded:
 *         governor.install -> prepare_my_world
 *
 * Governor owns the action name "install".
 * The module owns the transition name "prepare_my_world".
 *
 * Absence means that the module declares no Lifecycle transition for
 * that Governor action.
 *
 * This layer:
 *
 * - does not execute transitions
 * - does not interpret transition contents
 * - does not know artillery or objectives
 * - does not create FireControl routes
 * - does not create Capability handlers
 * - does not create authority grants
 */

const GOVERNOR_PREFIX: &str = "governor.";

fn validate_action(action: &str) -> Result<&str, String> {
    let action = action.trim();

    if action.is_empty() {
        return Err("lifecycle Governor action cannot be empty".to_string());
    }

    Ok(action)
}

pub fn resolve<'a>(
    contract: &'a LifecycleContract,
    action: &str,
) -> Result<Option<&'a str>, String> {
    let action = validate_action(action)?;

    let key = format!("{GOVERNOR_PREFIX}{action}");

    let Some(transition_id) = contract.hardcoded.get(&key) else {
        return Ok(None);
    };

    let transition_id = transition_id.trim();

    if transition_id.is_empty() {
        return Err(format!(
            "lifecycle Governor binding '{key}' cannot reference an empty transition"
        ));
    }

    if !contract.transitions.contains_key(transition_id) {
        return Err(format!(
            "lifecycle Governor binding '{key}' references undeclared transition '{transition_id}'"
        ));
    }

    Ok(Some(transition_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle::Battleplan;

    #[test]
    fn missing_binding_is_valid_absence() {
        let contract = LifecycleContract::default();

        assert_eq!(resolve(&contract, "install").unwrap(), None);
    }

    #[test]
    fn Governor_action_resolves_module_owned_transition() {
        let mut contract = LifecycleContract::default();

        contract.hardcoded.insert(
            "governor.install".to_string(),
            "module.decides.this.name".to_string(),
        );

        contract.transitions.insert(
            "module.decides.this.name".to_string(),
            Battleplan::default(),
        );

        assert_eq!(
            resolve(&contract, "install").unwrap(),
            Some("module.decides.this.name")
        );
    }

    #[test]
    fn Governor_action_does_not_require_same_transition_name() {
        let mut contract = LifecycleContract::default();

        contract
            .hardcoded
            .insert("governor.install".to_string(), "banana".to_string());

        contract
            .transitions
            .insert("banana".to_string(), Battleplan::default());

        assert_eq!(resolve(&contract, "install").unwrap(), Some("banana"));

        assert!(!contract.transitions.contains_key("install"));
    }

    #[test]
    fn unrelated_hardcoded_symbols_are_ignored() {
        let mut contract = LifecycleContract::default();

        contract
            .hardcoded
            .insert("anything".to_string(), "opaque".to_string());

        assert_eq!(resolve(&contract, "install").unwrap(), None);
    }

    #[test]
    fn dangling_binding_is_rejected() {
        let mut contract = LifecycleContract::default();

        contract.hardcoded.insert(
            "governor.install".to_string(),
            "missing.transition".to_string(),
        );

        let error = resolve(&contract, "install").unwrap_err();

        assert!(error.contains("undeclared transition"));
    }

    #[test]
    fn empty_binding_target_is_rejected() {
        let mut contract = LifecycleContract::default();

        contract
            .hardcoded
            .insert("governor.install".to_string(), "   ".to_string());

        let error = resolve(&contract, "install").unwrap_err();

        assert!(error.contains("empty transition"));
    }

    #[test]
    fn empty_Governor_action_is_rejected() {
        let error = resolve(&LifecycleContract::default(), "   ").unwrap_err();

        assert!(error.contains("action cannot be empty"));
    }

    #[test]
    fn future_Governor_actions_need_no_binding_engine_change() {
        let mut contract = LifecycleContract::default();

        contract.hardcoded.insert(
            "governor.future.action".to_string(),
            "some.future.transition".to_string(),
        );

        contract
            .transitions
            .insert("some.future.transition".to_string(), Battleplan::default());

        assert_eq!(
            resolve(&contract, "future.action",).unwrap(),
            Some("some.future.transition")
        );
    }
}

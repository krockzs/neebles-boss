use crate::surface_projection::{
    SurfaceRequirementState,
    SurfaceRequirements,
};

/*
 * Strict Boss Surface requirement resolver.
 *
 * This boundary answers functional preconditions declared by
 * module-owned SurfaceContent.
 *
 * It is intentionally fail-closed:
 * - an unresolved installation never satisfies a requirement
 * - a disabled module never satisfies active
 * - a configuration read failure never satisfies active
 *
 * Runtime/open authority is strict and fail-closed.
 * Do not use modules::module_running() here because that
 * helper is deliberately optimistic for non-destructive display.
 *
 * Callers evaluating Open must execute inside the persistent
 * Boss process that owns the authoritative RuntimeRegistry.
 */

fn module_active_with<Installed, Enabled>(
    module_id: &str,
    installed: Installed,
    enabled: Enabled,
) -> bool
where
    Installed: FnOnce(&str) -> Result<(), String>,
    Enabled: FnOnce(&str) -> Result<bool, String>,
{
    if installed(module_id).is_err() {
        return false;
    }

    matches!(enabled(module_id), Ok(true))
}

pub fn module_active(module_id: &str) -> bool {
    module_active_with(
        module_id,
        |name| crate::modules::installed_module_dir(name).map(|_| ()),
        crate::config::module_enabled,
    )
}

fn module_open_with<Active, RuntimeRegistered>(
    module_id: &str,
    active: Active,
    runtime_registered: RuntimeRegistered,
) -> bool
where
    Active: FnOnce(&str) -> bool,
    RuntimeRegistered: FnOnce(&str) -> Result<bool, String>,
{
    if !active(module_id) {
        return false;
    }

    matches!(runtime_registered(module_id), Ok(true))
}

pub fn module_open(module_id: &str) -> bool {
    module_open_with(
        module_id,
        module_active,
        |name| {
            crate::module_ipc::runtime_registry()
                .get(name)
                .map(|runtime| runtime.is_some())
        },
    )
}

fn requirement_state_satisfied(
    module_id: &str,
    state: SurfaceRequirementState,
) -> bool {
    match state {
        SurfaceRequirementState::Active => module_active(module_id),
        SurfaceRequirementState::Open => module_open(module_id),
    }
}

fn requirements_satisfied_with<Satisfied>(
    owner_module: &str,
    requirements: &SurfaceRequirements,
    mut satisfied: Satisfied,
) -> bool
where
    Satisfied: FnMut(&str, SurfaceRequirementState) -> bool,
{
    if let Some(state) = requirements.self_state() {
        if !satisfied(owner_module, state) {
            return false;
        }
    }

    for (module_id, state) in requirements.modules() {
        if !satisfied(module_id, *state) {
            return false;
        }
    }

    true
}

pub fn requirements_satisfied(
    owner_module: &str,
    requirements: &SurfaceRequirements,
) -> bool {
    requirements_satisfied_with(
        owner_module,
        requirements,
        requirement_state_satisfied,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn active_requires_installed_and_enabled() {
        assert!(module_active_with(
            "module.alpha",
            |_| Ok(()),
            |_| Ok(true),
        ));
    }

    #[test]
    fn active_rejects_disabled_installed_module() {
        assert!(!module_active_with(
            "module.alpha",
            |_| Ok(()),
            |_| Ok(false),
        ));
    }

    #[test]
    fn active_rejects_unresolved_installation() {
        let enabled_was_queried = Cell::new(false);

        let active = module_active_with(
            "module.alpha",
            |_| Err("module is not installed".to_string()),
            |_| {
                enabled_was_queried.set(true);
                Ok(true)
            },
        );

        assert!(!active);
        assert!(!enabled_was_queried.get());
    }

    #[test]
    fn active_fails_closed_when_enabled_state_cannot_be_read() {
        assert!(!module_active_with(
            "module.alpha",
            |_| Ok(()),
            |_| Err("Boss config unavailable".to_string()),
        ));
    }

    #[test]
    fn open_accepts_registered_runtime_when_active() {
        assert!(module_open_with(
            "module.alpha",
            |_| true,
            |_| Ok(true),
        ));
    }

    #[test]
    fn open_rejects_missing_runtime_when_active() {
        assert!(!module_open_with(
            "module.alpha",
            |_| true,
            |_| Ok(false),
        ));
    }

    #[test]
    fn open_fails_closed_when_runtime_registry_errors() {
        assert!(!module_open_with(
            "module.alpha",
            |_| true,
            |_| Err("runtime registry unavailable".to_string()),
        ));
    }

    #[test]
    fn open_requires_active_before_runtime_lookup() {
        let runtime_was_queried = Cell::new(false);

        let open = module_open_with(
            "module.alpha",
            |_| false,
            |_| {
                runtime_was_queried.set(true);
                Ok(true)
            },
        );

        assert!(!open);
        assert!(!runtime_was_queried.get());
    }

    fn requirements(value: serde_json::Value) -> SurfaceRequirements {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn empty_requirements_are_satisfied_without_queries() {
        let requirements = SurfaceRequirements::default();
        let queried = Cell::new(false);

        let satisfied = requirements_satisfied_with(
            "module.owner",
            &requirements,
            |_, _| {
                queried.set(true);
                false
            },
        );

        assert!(satisfied);
        assert!(!queried.get());
    }

    #[test]
    fn self_requirement_targets_owner_module() {
        let requirements = requirements(serde_json::json!({
            "self": "open"
        }));

        let satisfied = requirements_satisfied_with(
            "module.owner",
            &requirements,
            |module_id, state| {
                module_id == "module.owner"
                    && state == SurfaceRequirementState::Open
            },
        );

        assert!(satisfied);
    }

    #[test]
    fn self_failure_blocks_before_cross_module_queries() {
        let requirements = requirements(serde_json::json!({
            "self": "active",
            "modules": {
                "module.beta": "open"
            }
        }));

        let cross_module_queried = Cell::new(false);

        let satisfied = requirements_satisfied_with(
            "module.owner",
            &requirements,
            |module_id, _| {
                if module_id == "module.owner" {
                    return false;
                }

                cross_module_queried.set(true);
                true
            },
        );

        assert!(!satisfied);
        assert!(!cross_module_queried.get());
    }

    #[test]
    fn mixed_requirements_preserve_each_declared_state() {
        use std::cell::RefCell;

        let requirements = requirements(serde_json::json!({
            "self": "active",
            "modules": {
                "module.beta": "open",
                "module.gamma": "active"
            }
        }));

        let seen = RefCell::new(Vec::new());

        let satisfied = requirements_satisfied_with(
            "module.owner",
            &requirements,
            |module_id, state| {
                seen.borrow_mut().push((
                    module_id.to_string(),
                    state,
                ));
                true
            },
        );

        assert!(satisfied);

        assert_eq!(
            seen.into_inner(),
            vec![
                (
                    "module.owner".to_string(),
                    SurfaceRequirementState::Active,
                ),
                (
                    "module.beta".to_string(),
                    SurfaceRequirementState::Open,
                ),
                (
                    "module.gamma".to_string(),
                    SurfaceRequirementState::Active,
                ),
            ]
        );
    }

    #[test]
    fn one_unsatisfied_cross_module_requirement_blocks_all() {
        let requirements = requirements(serde_json::json!({
            "modules": {
                "module.beta": "active",
                "module.gamma": "open"
            }
        }));

        let satisfied = requirements_satisfied_with(
            "module.owner",
            &requirements,
            |module_id, state| {
                !(module_id == "module.gamma"
                    && state == SurfaceRequirementState::Open)
            },
        );

        assert!(!satisfied);
    }
}

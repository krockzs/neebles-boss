use std::sync::Arc;

use crate::lifecycle::LifecycleContract;
use crate::lifecycle_arsenal::LifecycleArsenal;
use crate::lifecycle_available_capabilities::AvailableCapabilityCatalog;
use crate::lifecycle_capabilities::CapabilityRegistry;
use crate::lifecycle_dag_executor::LifecycleDependencyGraph;
use crate::lifecycle_fire_control::FireControl;
use crate::lifecycle_governor_bridge::GovernorLifecycleExecution;
use crate::lifecycle_observer::LifecycleObserver;

/*
 * N.E.E.B.L.E.S. Governor Lifecycle runtime.
 *
 * Responsibility:
 *
 * Governor-owned action
 *     -> contractual binding
 *     -> module-owned transition
 *     -> existing Governor bridge
 *     -> existing Lifecycle engine
 *
 * This layer is infrastructure only.
 *
 * It does not know install, update, uninstall, enable or disable.
 * It does not decide module intent.
 * It does not fabricate transitions.
 * It does not fabricate FireControl routes.
 * It does not fabricate Capability handlers.
 * It does not fabricate authority grants.
 *
 * Product Governor operations consume this runtime later.
 */

pub struct GovernorLifecycleRuntime {
    fire_control: Option<Arc<FireControl>>,
    capabilities: Option<Arc<CapabilityRegistry>>,
    available: Option<Arc<AvailableCapabilityCatalog>>,
}

impl GovernorLifecycleRuntime {
    pub fn new(fire_control: Arc<FireControl>, capabilities: Arc<CapabilityRegistry>) -> Self {
        Self {
            fire_control: Some(fire_control),
            capabilities: Some(capabilities),
            available: None,
        }
    }

    pub fn from_available(available: Arc<AvailableCapabilityCatalog>) -> Self {
        Self {
            fire_control: None,
            capabilities: None,
            available: Some(available),
        }
    }

    pub fn from_arsenal(arsenal: LifecycleArsenal) -> Self {
        let (fire_control, capabilities) = arsenal.into_shared();

        Self::new(fire_control, capabilities)
    }

    pub fn execute_action_blocking(
        &self,
        module_id: &str,
        execution_id: &str,
        contract: &LifecycleContract,
        action: &str,
        dependencies: LifecycleDependencyGraph,
    ) -> Result<Option<GovernorLifecycleExecution>, String> {
        self.execute_action_blocking_observed(
            module_id,
            execution_id,
            contract,
            action,
            dependencies,
            LifecycleObserver::none(),
        )
    }

    pub fn execute_action_blocking_observed(
        &self,
        module_id: &str,
        execution_id: &str,
        contract: &LifecycleContract,
        action: &str,
        dependencies: LifecycleDependencyGraph,
        observer: LifecycleObserver,
    ) -> Result<Option<GovernorLifecycleExecution>, String> {
        let Some(transition_id) = crate::lifecycle_governor_binding::resolve(contract, action)?
        else {
            return Ok(None);
        };

        let execution = if let Some(available) = self.available.as_ref() {
            crate::lifecycle_governor_bridge::execute_transition_blocking_from_available_with_observer(
                module_id,
                execution_id,
                contract,
                transition_id,
                dependencies,
                Arc::clone(available),
                observer.clone(),
            )?
        } else {
            let fire_control = self
                .fire_control
                .as_ref()
                .ok_or_else(|| "Governor Lifecycle runtime lost FireControl".to_string())?;

            let capabilities = self
                .capabilities
                .as_ref()
                .ok_or_else(|| "Governor Lifecycle runtime lost CapabilityRegistry".to_string())?;

            crate::lifecycle_governor_bridge::execute_transition_blocking_with_observer(
                module_id,
                execution_id,
                contract,
                transition_id,
                dependencies,
                Arc::clone(fire_control),
                Arc::clone(capabilities),
                observer,
            )?
        };

        execution.require_success().map(Some)
    }

    pub fn execute_target_blocking(
        &self,
        module_id: &str,
        execution_id: &str,
        contract: &LifecycleContract,
        action: &str,
        object_id: Option<&str>,
        transition_id: Option<&str>,
        dependencies: LifecycleDependencyGraph,
    ) -> Result<Option<GovernorLifecycleExecution>, String> {
        self.execute_target_blocking_observed(
            module_id,
            execution_id,
            contract,
            action,
            object_id,
            transition_id,
            dependencies,
            LifecycleObserver::none(),
        )
    }

    pub fn execute_target_blocking_observed(
        &self,
        module_id: &str,
        execution_id: &str,
        contract: &LifecycleContract,
        action: &str,
        object_id: Option<&str>,
        transition_id: Option<&str>,
        dependencies: LifecycleDependencyGraph,
        observer: LifecycleObserver,
    ) -> Result<Option<GovernorLifecycleExecution>, String> {
        let object_id = object_id.map(str::trim).filter(|value| !value.is_empty());

        let transition_id = transition_id
            .map(str::trim)
            .filter(|value| !value.is_empty());

        match (object_id, transition_id) {
            (Some(object_id), Some(transition_id)) => {
                let execution = if let Some(available) = self.available.as_ref() {
                    crate::lifecycle_governor_bridge::
                            execute_object_transition_blocking_from_available_with_observer(
                                module_id,
                                execution_id,
                                contract,
                                object_id,
                                transition_id,
                                dependencies,
                                Arc::clone(
                                    available
                                ),
                                observer,
                            )?
                } else {
                    let fire_control = self
                        .fire_control
                        .as_ref()
                        .ok_or_else(|| "Governor Lifecycle runtime lost FireControl".to_string())?;

                    let capabilities = self.capabilities.as_ref().ok_or_else(|| {
                        "Governor Lifecycle runtime lost CapabilityRegistry".to_string()
                    })?;

                    crate::lifecycle_governor_bridge::
                            execute_object_transition_blocking_with_observer(
                                module_id,
                                execution_id,
                                contract,
                                object_id,
                                transition_id,
                                dependencies,
                                Arc::clone(
                                    fire_control
                                ),
                                Arc::clone(
                                    capabilities
                                ),
                                observer,
                            )?
                };

                Ok(Some(execution))
            }

            (Some(_), None) => {
                Err("Governor object target requires an explicit Lifecycle transition".to_string())
            }

            (None, Some(transition_id)) => {
                let execution = if let Some(available) = self.available.as_ref() {
                    crate::lifecycle_governor_bridge::
                            execute_transition_blocking_from_available_with_observer(
                                module_id,
                                execution_id,
                                contract,
                                transition_id,
                                dependencies,
                                Arc::clone(
                                    available
                                ),
                                observer,
                            )?
                } else {
                    let fire_control = self
                        .fire_control
                        .as_ref()
                        .ok_or_else(|| "Governor Lifecycle runtime lost FireControl".to_string())?;

                    let capabilities = self.capabilities.as_ref().ok_or_else(|| {
                        "Governor Lifecycle runtime lost CapabilityRegistry".to_string()
                    })?;

                    crate::lifecycle_governor_bridge::execute_transition_blocking_with_observer(
                        module_id,
                        execution_id,
                        contract,
                        transition_id,
                        dependencies,
                        Arc::clone(fire_control),
                        Arc::clone(capabilities),
                        observer,
                    )?
                };

                Ok(Some(execution))
            }

            (None, None) => self.execute_action_blocking_observed(
                module_id,
                execution_id,
                contract,
                action,
                dependencies,
                observer,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;
    use std::sync::Arc;

    use crate::lifecycle::{Battleplan, Operation};
    use crate::lifecycle_execution::ExecutionPayload;

    #[test]
    fn missing_binding_is_valid_runtime_absence() {
        let runtime = GovernorLifecycleRuntime::from_arsenal(LifecycleArsenal::new());

        let result = runtime
            .execute_action_blocking(
                "module.alpha",
                "execution.alpha",
                &LifecycleContract::default(),
                "future.action",
                BTreeMap::new(),
            )
            .unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn arbitrary_governor_action_resolves_arbitrary_module_transition() {
        let runtime = GovernorLifecycleRuntime::from_arsenal(LifecycleArsenal::new());

        let mut contract = LifecycleContract::default();

        contract
            .hardcoded
            .insert("governor.future.action".to_string(), "banana".to_string());

        contract
            .transitions
            .insert("banana".to_string(), Battleplan::default());

        let result = runtime
            .execute_action_blocking(
                "module.alpha",
                "execution.alpha",
                &contract,
                "future.action",
                BTreeMap::new(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(result.report().transition_id(), "banana");

        assert_eq!(result.report().terminal_phase(), "succeeded");
    }

    #[test]
    fn injected_arsenal_drives_real_transition() {
        let mut arsenal = LifecycleArsenal::new();

        arsenal
            .register(
                "synthetic.artillery",
                "synthetic.objective",
                "synthetic.implementation",
                Arc::new(|operation| {
                    Box::pin(async move {
                        Ok(ExecutionPayload::from([(
                            "seen".to_string(),
                            operation.objective,
                        )]))
                    })
                }),
            )
            .unwrap();

        let runtime = GovernorLifecycleRuntime::from_arsenal(arsenal);

        let mut contract = LifecycleContract::default();

        contract.hardcoded.insert(
            "governor.future.action".to_string(),
            "module.transition".to_string(),
        );

        contract.transitions.insert(
            "module.transition".to_string(),
            Battleplan {
                operations: BTreeMap::from([(
                    "operation".to_string(),
                    Operation {
                        artillery: "synthetic.artillery".to_string(),
                        objective: "synthetic.objective".to_string(),
                        munition: BTreeMap::new(),
                        tactics: BTreeMap::new(),
                        intelligence: BTreeMap::new(),
                    },
                )]),
            },
        );

        let result = runtime
            .execute_action_blocking(
                "module.alpha",
                "execution.alpha",
                &contract,
                "future.action",
                BTreeMap::new(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(result.report().terminal_phase(), "succeeded");
    }

    #[test]
    fn capability_failure_reaches_governor_runtime_boundary() {
        let mut arsenal = LifecycleArsenal::new();

        arsenal
            .register(
                "synthetic.artillery",
                "synthetic.failure",
                "synthetic.failure.impl",
                Arc::new(|_| Box::pin(async { Err("synthetic capability failure".to_string()) })),
            )
            .unwrap();

        let runtime = GovernorLifecycleRuntime::from_arsenal(arsenal);

        let mut contract = LifecycleContract::default();

        contract.hardcoded.insert(
            "governor.future.action".to_string(),
            "module.failure".to_string(),
        );

        contract.transitions.insert(
            "module.failure".to_string(),
            Battleplan {
                operations: BTreeMap::from([(
                    "operation".to_string(),
                    Operation {
                        artillery: "synthetic.artillery".to_string(),
                        objective: "synthetic.failure".to_string(),
                        munition: BTreeMap::new(),
                        tactics: BTreeMap::new(),
                        intelligence: BTreeMap::new(),
                    },
                )]),
            },
        );

        let result = runtime.execute_action_blocking(
            "module.alpha",
            "execution.alpha",
            &contract,
            "future.action",
            BTreeMap::new(),
        );

        let error = match result {
            Ok(_) => panic!("expected synthetic capability failure"),
            Err(error) => error,
        };

        assert!(error.contains("terminal phase"));
    }
}

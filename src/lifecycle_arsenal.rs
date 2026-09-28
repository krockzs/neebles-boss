use std::sync::Arc;

use crate::lifecycle_available_capabilities::AvailableCapabilityCatalog;
use crate::lifecycle_capabilities::{CapabilityHandler, CapabilityRegistry};
use crate::lifecycle_fire_control::{FireControl, FireControlTarget};

/*
 * N.E.E.B.L.E.S. Lifecycle productive arsenal.
 *
 * Technical responsibility:
 *
 * Own the productive pairing between:
 *
 *     FireControl
 *         artillery + objective
 *             -> implementation id
 *
 * and:
 *
 *     CapabilityRegistry
 *         implementation id
 *             -> Rust capability handler
 *
 * This layer does not interpret module semantics.
 * It does not prescribe Lifecycle transition vocabulary.
 * It does not fabricate AuthoritySupply grants.
 * It does not execute transitions.
 *
 * The Governor bridge consumes the finished pair.
 */

pub struct LifecycleArsenal {
    fire_control: FireControl,
    capabilities: CapabilityRegistry,
}

impl LifecycleArsenal {
    pub fn new() -> Self {
        Self {
            fire_control: FireControl::new(),
            capabilities: CapabilityRegistry::new(),
        }
    }

    pub fn register(
        &mut self,
        artillery: impl Into<String>,
        objective: impl Into<String>,
        implementation_id: impl Into<String>,
        handler: CapabilityHandler,
    ) -> Result<(), String> {
        let artillery = artillery.into();
        let objective = objective.into();
        let implementation_id = implementation_id.into();

        if artillery.trim().is_empty() {
            return Err("lifecycle arsenal artillery id cannot be empty".to_string());
        }

        if objective.trim().is_empty() {
            return Err("lifecycle arsenal objective id cannot be empty".to_string());
        }

        if implementation_id.trim().is_empty() {
            return Err("lifecycle arsenal implementation id cannot be empty".to_string());
        }

        self.capabilities
            .register(implementation_id.clone(), handler)?;

        if let Err(error) = self.fire_control.register(
            artillery,
            objective,
            FireControlTarget::new(implementation_id.clone())?,
        ) {
            self.capabilities.unregister(&implementation_id);

            return Err(error);
        }

        Ok(())
    }

    pub fn register_requested(
        &mut self,
        catalog: &AvailableCapabilityCatalog,
        artillery: &str,
        objective: &str,
    ) -> Result<(), String> {
        if self.fire_control.contains(artillery, objective) {
            return Ok(());
        }

        let available = catalog.resolve(artillery)?;

        let implementation_id = available.implementation_id().to_string();

        if !self.capabilities.contains(&implementation_id) {
            self.capabilities
                .register(implementation_id.clone(), available.handler())?;
        }

        if let Err(error) = self.fire_control.register(
            artillery,
            objective,
            FireControlTarget::new(implementation_id.clone())?,
        ) {
            if !self.fire_control.contains(artillery, objective) {
                return Err(error);
            }
        }

        Ok(())
    }

    pub fn into_shared(self) -> (Arc<FireControl>, Arc<CapabilityRegistry>) {
        (Arc::new(self.fire_control), Arc::new(self.capabilities))
    }
}

impl Default for LifecycleArsenal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;

    use crate::lifecycle::{Battleplan, LifecycleContract, Operation};
    use crate::lifecycle_execution::ExecutionPayload;
    use crate::lifecycle_governor_bridge::execute_transition_blocking;

    #[test]
    fn requested_routes_are_built_from_available_catalog() {
        let mut catalog = AvailableCapabilityCatalog::new();

        catalog
            .register(
                "future.tool",
                crate::lifecycle_available_capabilities::AvailableCapability::new(
                    "rust.future.adapter",
                    Arc::new(|operation| {
                        Box::pin(async move {
                            Ok(ExecutionPayload::from([(
                                "objective".to_string(),
                                operation.objective,
                            )]))
                        })
                    }),
                )
                .unwrap(),
            )
            .unwrap();

        let mut arsenal = LifecycleArsenal::new();

        arsenal
            .register_requested(&catalog, "future.tool", "first.action")
            .unwrap();

        arsenal
            .register_requested(&catalog, "future.tool", "second.action")
            .unwrap();

        let (fire_control, capabilities) = arsenal.into_shared();

        assert_eq!(
            fire_control
                .resolve("future.tool", "first.action",)
                .unwrap()
                .implementation(),
            "rust.future.adapter"
        );

        assert_eq!(
            fire_control
                .resolve("future.tool", "second.action",)
                .unwrap()
                .implementation(),
            "rust.future.adapter"
        );

        assert!(capabilities.contains("rust.future.adapter"));
    }

    #[test]
    fn unrequested_available_capability_is_not_registered() {
        let mut catalog = AvailableCapabilityCatalog::new();

        catalog
            .register(
                "available.but.unused",
                crate::lifecycle_available_capabilities::AvailableCapability::new(
                    "rust.unused.adapter",
                    Arc::new(|_| Box::pin(async { Ok(ExecutionPayload::new()) })),
                )
                .unwrap(),
            )
            .unwrap();

        let arsenal = LifecycleArsenal::new();

        let (fire_control, capabilities) = arsenal.into_shared();

        assert!(!fire_control.contains("available.but.unused", "anything",));

        assert!(!capabilities.contains("rust.unused.adapter"));
    }

    #[test]
    fn arsenal_registers_route_and_handler_as_one_productive_unit() {
        let mut arsenal = LifecycleArsenal::new();

        arsenal
            .register(
                "future.artillery",
                "future.objective",
                "future.implementation",
                Arc::new(|operation| {
                    Box::pin(async move {
                        Ok(ExecutionPayload::from([(
                            "objective".to_string(),
                            operation.objective,
                        )]))
                    })
                }),
            )
            .unwrap();

        let (fire_control, capabilities) = arsenal.into_shared();

        assert_eq!(
            fire_control
                .resolve("future.artillery", "future.objective",)
                .unwrap()
                .implementation(),
            "future.implementation"
        );

        assert!(capabilities.contains("future.implementation"));
    }

    #[test]
    fn arsenal_rolls_back_handler_when_route_registration_fails() {
        let mut arsenal = LifecycleArsenal::new();

        arsenal
            .register(
                "alpha",
                "same",
                "first.implementation",
                Arc::new(|_| Box::pin(async { Ok(ExecutionPayload::new()) })),
            )
            .unwrap();

        let error = arsenal
            .register(
                "alpha",
                "same",
                "second.implementation",
                Arc::new(|_| Box::pin(async { Ok(ExecutionPayload::new()) })),
            )
            .unwrap_err();

        assert!(error.contains("already registered"));

        assert!(!arsenal.capabilities.contains("second.implementation"));
    }

    #[test]
    fn arsenal_can_drive_existing_governor_bridge_without_new_engine() {
        let mut arsenal = LifecycleArsenal::new();

        arsenal
            .register(
                "synthetic",
                "execute",
                "synthetic.execute",
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

        let mut contract = LifecycleContract::default();

        contract.transitions.insert(
            "future.transition".to_string(),
            Battleplan {
                operations: BTreeMap::from([(
                    "operation".to_string(),
                    Operation {
                        artillery: "synthetic".to_string(),
                        objective: "execute".to_string(),
                        munition: BTreeMap::new(),
                        tactics: BTreeMap::new(),
                        intelligence: BTreeMap::new(),
                    },
                )]),
            },
        );

        let (fire_control, capabilities) = arsenal.into_shared();

        let execution = execute_transition_blocking(
            "future.module",
            "arsenal-test",
            &contract,
            "future.transition",
            BTreeMap::new(),
            fire_control,
            capabilities,
        )
        .unwrap();

        assert_eq!(execution.report().terminal_phase(), "succeeded");
    }
}

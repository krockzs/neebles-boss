use std::sync::Arc;

use crate::lifecycle::{self, LifecycleContract};
use crate::lifecycle_available_capabilities::AvailableCapabilityCatalog;
use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_capabilities::CapabilityRegistry;
use crate::lifecycle_communication::LifecycleCommunication;
use crate::lifecycle_dag_executor::LifecycleDependencyGraph;
use crate::lifecycle_event::LifecycleEventLog;
use crate::lifecycle_fire_control::FireControl;
use crate::lifecycle_object_transition_executor;
use crate::lifecycle_observer::LifecycleObserver;
use crate::lifecycle_state::LifecycleRuntimeState;
use crate::lifecycle_telemetry;
use crate::lifecycle_transition_executor::{self, TransitionExecutionReport};

/*
 * N.E.E.B.L.E.S. Lifecycle Governor Execution Bridge.
 *
 * Technical responsibility:
 *
 * synchronous Module Governor boundary
 *      -> explicit module and execution identity
 *      -> opaque hardcoded symbols in Battlefield
 *      -> already-built FireControl
 *      -> already-built CapabilityRegistry
 *      -> existing Transition Executor
 *      -> existing Event / Telemetry path
 *
 * This bridge does not fabricate Fire Control routes.
 * It does not fabricate Capability handlers.
 * It does not fabricate AuthoritySupply grants.
 *
 * Concrete capabilities remain responsible for requesting the explicit
 * authorities they need. The bridge only supplies the generic execution
 * context and drives the already-certified Lifecycle engine.
 *
 * Lifecycle transition vocabulary remains module-owned.
 */

pub struct GovernorLifecycleExecution {
    report: TransitionExecutionReport,
    state: LifecycleRuntimeState,
    communication: LifecycleCommunication,
    battlefield: Battlefield,
    events: LifecycleEventLog,
}

impl GovernorLifecycleExecution {
    pub fn report(&self) -> &TransitionExecutionReport {
        &self.report
    }

    pub fn state(&self) -> &LifecycleRuntimeState {
        &self.state
    }

    pub fn communication(&self) -> &LifecycleCommunication {
        &self.communication
    }

    pub fn battlefield(&self) -> &Battlefield {
        &self.battlefield
    }

    pub fn events(&self) -> &LifecycleEventLog {
        &self.events
    }

    pub fn require_success(self) -> Result<Self, String> {
        if self.report.terminal_phase() == "succeeded" {
            return Ok(self);
        }

        Err(format!(
            "module lifecycle transition '{}' ended in terminal phase '{}'",
            self.report.transition_id(),
            self.report.terminal_phase()
        ))
    }
}

fn validate_identity(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} cannot be empty"));
    }

    Ok(())
}

fn execution_battlefield(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
) -> Battlefield {
    let mut battlefield = Battlefield::new();

    battlefield.insert("module", "id", module_id);
    battlefield.insert("execution", "id", execution_id);

    for (key, value) in &contract.hardcoded {
        battlefield.insert("hardcoded", key.clone(), value.clone());
    }

    battlefield
}

fn record_event(
    events: &mut LifecycleEventLog,
    communication: &LifecycleCommunication,
    kind: impl Into<String>,
) -> Result<(), String> {
    events.record(kind, communication)?;

    if let Some(event) = events.latest() {
        let _ = lifecycle_telemetry::emit_failure(event);
    }

    Ok(())
}

pub async fn execute_transition_observed(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
) -> Result<GovernorLifecycleExecution, String> {
    execute_transition_with_observer(
        module_id,
        execution_id,
        contract,
        transition_id,
        dependencies,
        fire_control,
        capabilities,
        LifecycleObserver::none(),
    )
    .await
}

pub async fn execute_transition_with_observer(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
    observer: LifecycleObserver,
) -> Result<GovernorLifecycleExecution, String> {
    validate_identity("module lifecycle module id", module_id)?;
    validate_identity("module lifecycle execution id", execution_id)?;
    validate_identity("module lifecycle transition id", transition_id)?;

    lifecycle::validate(contract)?;

    let mut battlefield = execution_battlefield(module_id, execution_id, contract);

    let mut state = LifecycleRuntimeState::new(execution_id, transition_id)?;

    state.put("module.id", module_id)?;

    let mut communication = LifecycleCommunication::from_state(&state)?;

    let mut events = LifecycleEventLog::new();

    let report = match lifecycle_transition_executor::execute_observed(
        contract,
        transition_id,
        dependencies,
        &mut battlefield,
        &mut state,
        &mut communication,
        fire_control,
        capabilities,
        observer,
    )
    .await
    {
        Ok(report) => report,

        Err(error) => {
            record_event(&mut events, &communication, "transition.error")?;

            return Err(error);
        }
    };

    record_event(
        &mut events,
        &communication,
        format!("transition.{}", report.terminal_phase()),
    )?;

    Ok(GovernorLifecycleExecution {
        report,
        state,
        communication,
        battlefield,
        events,
    })
}

pub async fn execute_transition_observed_from_available(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    available: Arc<AvailableCapabilityCatalog>,
) -> Result<GovernorLifecycleExecution, String> {
    execute_transition_from_available_with_observer(
        module_id,
        execution_id,
        contract,
        transition_id,
        dependencies,
        available,
        LifecycleObserver::none(),
    )
    .await
}

pub async fn execute_transition_from_available_with_observer(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    available: Arc<AvailableCapabilityCatalog>,
    observer: LifecycleObserver,
) -> Result<GovernorLifecycleExecution, String> {
    validate_identity("module lifecycle module id", module_id)?;

    validate_identity("module lifecycle execution id", execution_id)?;

    validate_identity("module lifecycle transition id", transition_id)?;

    lifecycle::validate(contract)?;

    let mut battlefield = execution_battlefield(module_id, execution_id, contract);

    let mut state = LifecycleRuntimeState::new(execution_id, transition_id)?;

    state.put("module.id", module_id)?;

    let mut communication = LifecycleCommunication::from_state(&state)?;

    let mut events = LifecycleEventLog::new();

    let report = match lifecycle_transition_executor::execute_from_available_observed(
        contract,
        transition_id,
        dependencies,
        &mut battlefield,
        &mut state,
        &mut communication,
        available,
        observer,
    )
    .await
    {
        Ok(report) => report,

        Err(error) => {
            record_event(&mut events, &communication, "transition.error")?;

            return Err(error);
        }
    };

    record_event(
        &mut events,
        &communication,
        format!("transition.{}", report.terminal_phase(),),
    )?;

    Ok(GovernorLifecycleExecution {
        report,
        state,
        communication,
        battlefield,
        events,
    })
}

pub async fn execute_object_transition_from_available_with_observer(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    object_id: &str,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    available: Arc<AvailableCapabilityCatalog>,
    observer: LifecycleObserver,
) -> Result<GovernorLifecycleExecution, String> {
    validate_identity("module lifecycle module id", module_id)?;

    validate_identity("module lifecycle execution id", execution_id)?;

    validate_identity("module lifecycle object id", object_id)?;

    validate_identity("module lifecycle transition id", transition_id)?;

    lifecycle::validate(contract)?;

    let mut battlefield = execution_battlefield(module_id, execution_id, contract);

    let mut state = LifecycleRuntimeState::new(execution_id, transition_id)?;

    state.put("module.id", module_id)?;

    let mut communication = LifecycleCommunication::from_state(&state)?;

    let mut events = LifecycleEventLog::new();

    let object_execution =
        match lifecycle_object_transition_executor::execute_from_available_observed(
            contract,
            object_id,
            transition_id,
            dependencies,
            &mut battlefield,
            &mut state,
            &mut communication,
            available,
            observer,
        )
        .await
        {
            Ok(report) => report,

            Err(error) => {
                record_event(&mut events, &communication, "object-transition.error")?;

                return Err(error);
            }
        };

    let report = object_execution.transition().clone();

    record_event(
        &mut events,
        &communication,
        format!("object-transition.{}", report.terminal_phase(),),
    )?;

    Ok(GovernorLifecycleExecution {
        report,
        state,
        communication,
        battlefield,
        events,
    })
}

pub async fn execute_object_transition_with_observer(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    object_id: &str,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
    observer: LifecycleObserver,
) -> Result<GovernorLifecycleExecution, String> {
    validate_identity("module lifecycle module id", module_id)?;

    validate_identity("module lifecycle execution id", execution_id)?;

    validate_identity("module lifecycle object id", object_id)?;

    validate_identity("module lifecycle transition id", transition_id)?;

    lifecycle::validate(contract)?;

    let mut battlefield = execution_battlefield(module_id, execution_id, contract);

    let mut state = LifecycleRuntimeState::new(execution_id, transition_id)?;

    state.put("module.id", module_id)?;

    let mut communication = LifecycleCommunication::from_state(&state)?;

    let mut events = LifecycleEventLog::new();

    let object_execution = match lifecycle_object_transition_executor::execute_observed(
        contract,
        object_id,
        transition_id,
        dependencies,
        &mut battlefield,
        &mut state,
        &mut communication,
        fire_control,
        capabilities,
        observer,
    )
    .await
    {
        Ok(report) => report,

        Err(error) => {
            record_event(&mut events, &communication, "object-transition.error")?;

            return Err(error);
        }
    };

    let report = object_execution.transition().clone();

    record_event(
        &mut events,
        &communication,
        format!("object-transition.{}", report.terminal_phase(),),
    )?;

    Ok(GovernorLifecycleExecution {
        report,
        state,
        communication,
        battlefield,
        events,
    })
}

pub fn execute_object_transition_blocking_from_available_with_observer(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    object_id: &str,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    available: Arc<AvailableCapabilityCatalog>,
    observer: LifecycleObserver,
) -> Result<GovernorLifecycleExecution, String> {
    let execution =
        futures_lite::future::block_on(execute_object_transition_from_available_with_observer(
            module_id,
            execution_id,
            contract,
            object_id,
            transition_id,
            dependencies,
            available,
            observer,
        ))?;

    execution.require_success()
}

pub fn execute_object_transition_blocking_with_observer(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    object_id: &str,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
    observer: LifecycleObserver,
) -> Result<GovernorLifecycleExecution, String> {
    let execution = futures_lite::future::block_on(execute_object_transition_with_observer(
        module_id,
        execution_id,
        contract,
        object_id,
        transition_id,
        dependencies,
        fire_control,
        capabilities,
        observer,
    ))?;

    execution.require_success()
}

pub fn execute_transition_blocking_from_available_with_observer(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    available: Arc<AvailableCapabilityCatalog>,
    observer: LifecycleObserver,
) -> Result<GovernorLifecycleExecution, String> {
    let execution =
        futures_lite::future::block_on(execute_transition_from_available_with_observer(
            module_id,
            execution_id,
            contract,
            transition_id,
            dependencies,
            available,
            observer,
        ))?;

    execution.require_success()
}

pub fn execute_transition_blocking_from_available(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    available: Arc<AvailableCapabilityCatalog>,
) -> Result<GovernorLifecycleExecution, String> {
    let execution = futures_lite::future::block_on(execute_transition_observed_from_available(
        module_id,
        execution_id,
        contract,
        transition_id,
        dependencies,
        available,
    ))?;

    execution.require_success()
}

pub fn execute_transition_blocking_with_observer(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
    observer: LifecycleObserver,
) -> Result<GovernorLifecycleExecution, String> {
    let execution = futures_lite::future::block_on(execute_transition_with_observer(
        module_id,
        execution_id,
        contract,
        transition_id,
        dependencies,
        fire_control,
        capabilities,
        observer,
    ))?;

    execution.require_success()
}

pub fn execute_transition_blocking(
    module_id: &str,
    execution_id: &str,
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
) -> Result<GovernorLifecycleExecution, String> {
    let execution = futures_lite::future::block_on(execute_transition_observed(
        module_id,
        execution_id,
        contract,
        transition_id,
        dependencies,
        fire_control,
        capabilities,
    ))?;

    execution.require_success()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::{BTreeMap, BTreeSet};

    use crate::lifecycle::{Battleplan, Operation};
    use crate::lifecycle_execution::ExecutionPayload;
    use crate::lifecycle_fire_control::FireControlTarget;

    fn dependency_graph(operation_id: &str) -> LifecycleDependencyGraph {
        BTreeMap::from([(operation_id.to_string(), BTreeSet::new())])
    }

    fn successful_arsenal(objective: &str) -> (Arc<FireControl>, Arc<CapabilityRegistry>) {
        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "synthetic",
                objective,
                FireControlTarget::new("synthetic.success").unwrap(),
            )
            .unwrap();

        let mut capabilities = CapabilityRegistry::new();

        capabilities
            .register(
                "synthetic.success",
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

        (Arc::new(fire_control), Arc::new(capabilities))
    }

    fn failing_arsenal() -> (Arc<FireControl>, Arc<CapabilityRegistry>) {
        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "synthetic",
                "fail",
                FireControlTarget::new("synthetic.failure").unwrap(),
            )
            .unwrap();

        let mut capabilities = CapabilityRegistry::new();

        capabilities
            .register(
                "synthetic.failure",
                Arc::new(|_| {
                    Box::pin(async move { Err("synthetic governed failure".to_string()) })
                }),
            )
            .unwrap();

        (Arc::new(fire_control), Arc::new(capabilities))
    }

    #[test]
    fn hardcoded_symbols_feed_real_transition_resolution() {
        let marker = char::from_u32(36).unwrap();

        let mut contract = LifecycleContract::default();

        contract
            .hardcoded
            .insert("objective".to_string(), "resolved.objective".to_string());

        contract.transitions.insert(
            "future.transition".to_string(),
            Battleplan {
                operations: BTreeMap::from([(
                    "engage".to_string(),
                    Operation {
                        artillery: "synthetic".to_string(),
                        objective: format!("{marker}hardcoded.objective"),
                        munition: BTreeMap::new(),
                        tactics: BTreeMap::new(),
                        intelligence: BTreeMap::new(),
                    },
                )]),
            },
        );

        let (fire_control, capabilities) = successful_arsenal("resolved.objective");

        let execution = futures_lite::future::block_on(execute_transition_observed(
            "future.module",
            "execution-hardcoded",
            &contract,
            "future.transition",
            dependency_graph("engage"),
            fire_control,
            capabilities,
        ))
        .unwrap();

        assert_eq!(execution.report().terminal_phase(), "succeeded");

        assert_eq!(
            execution.battlefield().get("hardcoded", "objective"),
            Some("resolved.objective")
        );

        assert_eq!(execution.state().get("module.id"), Some("future.module"));

        assert_eq!(
            execution.communication().get("state.module.id"),
            Some("future.module")
        );
    }

    #[test]
    fn transition_vocabulary_remains_dynamic() {
        let mut contract = LifecycleContract::default();

        contract.transitions.insert(
            "future.operation.without.fixed.vocabulary".to_string(),
            Battleplan::default(),
        );

        let execution = execute_transition_blocking(
            "future.module",
            "execution-dynamic",
            &contract,
            "future.operation.without.fixed.vocabulary",
            BTreeMap::new(),
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        )
        .unwrap();

        assert_eq!(
            execution.report().transition_id(),
            "future.operation.without.fixed.vocabulary"
        );

        assert_eq!(execution.report().terminal_phase(), "succeeded");
    }

    #[test]
    fn operation_failure_is_observed_then_rejected_by_governor_boundary() {
        let mut contract = LifecycleContract::default();

        contract.transitions.insert(
            "future.failure".to_string(),
            Battleplan {
                operations: BTreeMap::from([(
                    "break".to_string(),
                    Operation {
                        artillery: "synthetic".to_string(),
                        objective: "fail".to_string(),
                        munition: BTreeMap::new(),
                        tactics: BTreeMap::new(),
                        intelligence: BTreeMap::new(),
                    },
                )]),
            },
        );

        let (fire_control, capabilities) = failing_arsenal();

        let execution = futures_lite::future::block_on(execute_transition_observed(
            "future.module",
            "execution-failure",
            &contract,
            "future.failure",
            dependency_graph("break"),
            fire_control,
            capabilities,
        ))
        .unwrap();

        assert_eq!(execution.report().terminal_phase(), "failed");

        assert_eq!(
            execution.events().latest().unwrap().kind(),
            "transition.failed"
        );

        let error = match execution.require_success() {
            Ok(_) => panic!("expected governed lifecycle failure"),
            Err(error) => error,
        };

        assert!(error.contains("terminal phase 'failed'"));
    }

    #[test]
    fn bridge_does_not_need_routes_for_empty_transition() {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert("empty.future.transition".to_string(), Battleplan::default());

        let execution = execute_transition_blocking(
            "future.module",
            "execution-empty",
            &contract,
            "empty.future.transition",
            BTreeMap::new(),
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        )
        .unwrap();

        assert_eq!(execution.report().terminal_phase(), "succeeded");
    }

    #[test]
    fn empty_module_identity_is_rejected() {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert("anything".to_string(), Battleplan::default());

        let error = match execute_transition_blocking(
            "   ",
            "execution-invalid-module",
            &contract,
            "anything",
            BTreeMap::new(),
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ) {
            Ok(_) => panic!("expected empty module identity rejection"),
            Err(error) => error,
        };

        assert!(error.contains("module id cannot be empty"));
    }
}

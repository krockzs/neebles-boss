use std::collections::BTreeMap;
use std::sync::Arc;

use crate::lifecycle::LifecycleContract;
use crate::lifecycle_available_capabilities::AvailableCapabilityCatalog;
use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_capabilities::CapabilityRegistry;
use crate::lifecycle_communication::LifecycleCommunication;
use crate::lifecycle_control::{CancellationToken, ExecutionControl};
use crate::lifecycle_dag_executor::{self, DagExecutionReport, LifecycleDependencyGraph};
use crate::lifecycle_failure::FailureContext;
use crate::lifecycle_fire_control::FireControl;
use crate::lifecycle_ir::{self, BattleplanIr};
use crate::lifecycle_result::NormalizedStatus;
use crate::lifecycle_state::LifecycleRuntimeState;

/*
 * N.E.E.B.L.E.S. Lifecycle Transition Executor.
 *
 * Technical responsibility:
 *
 * LifecycleContract
 *      + transition id
 *      + explicit dependency graph
 *      + Battlefield
 *      + FireControl
 *      + Capability Registry
 *      + optional execution controls
 *      -> compile Transition
 *      -> execute DAG
 *      -> preserve runtime intelligence chaining
 *      -> derive terminal transition state
 *      -> publish structured progress and result communication
 *
 * The DAG remains responsible for operation-level execution and
 * intelligence propagation because dependent operations may consume
 * intelligence while the transition is still running.
 *
 * This layer deliberately does not:
 * - infer dependencies from BTreeMap order
 * - interpret tactics
 * - prescribe artillery or objective vocabulary
 * - own Failure Context
 * - emit logs
 * - emit telemetry
 * - know Governor
 * - know UI
 *
 * operation.id remains absent at transition scope because a DAG can
 * execute more than one operation concurrently.
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionExecutionReport {
    transition_id: String,
    terminal_phase: String,
    dag: DagExecutionReport,
    failure: Option<FailureContext>,
}

impl TransitionExecutionReport {
    pub fn transition_id(&self) -> &str {
        &self.transition_id
    }

    pub fn terminal_phase(&self) -> &str {
        &self.terminal_phase
    }

    pub fn dag(&self) -> &DagExecutionReport {
        &self.dag
    }

    pub fn failure(&self) -> Option<&FailureContext> {
        self.failure.as_ref()
    }
}

fn normalized_status_text(status: NormalizedStatus) -> &'static str {
    match status {
        NormalizedStatus::Succeeded => "succeeded",

        NormalizedStatus::Failed => "failed",

        NormalizedStatus::Cancelled => "cancelled",
    }
}

fn terminal_phase(report: &DagExecutionReport) -> &'static str {
    let mut failure = false;

    let mut cancellation = false;

    for result in report.results().values() {
        match result.status() {
            NormalizedStatus::Succeeded => {}

            NormalizedStatus::Failed => {
                failure = true;
            }

            NormalizedStatus::Cancelled => {
                cancellation = true;
            }
        }
    }

    if failure {
        "failed"
    } else if cancellation {
        "cancelled"
    } else {
        "succeeded"
    }
}

fn progress_values(report: &DagExecutionReport) -> BTreeMap<String, String> {
    let mut succeeded = 0usize;

    let mut failed = 0usize;

    let mut cancelled = 0usize;

    for result in report.results().values() {
        match result.status() {
            NormalizedStatus::Succeeded => {
                succeeded += 1;
            }

            NormalizedStatus::Failed => {
                failed += 1;
            }

            NormalizedStatus::Cancelled => {
                cancelled += 1;
            }
        }
    }

    let total = report.results().len();

    BTreeMap::from([
        ("total".to_string(), total.to_string()),
        ("completed".to_string(), total.to_string()),
        ("succeeded".to_string(), succeeded.to_string()),
        ("failed".to_string(), failed.to_string()),
        ("cancelled".to_string(), cancelled.to_string()),
        ("unit".to_string(), "operations".to_string()),
    ])
}

fn result_values(
    transition_id: &str,
    phase: &str,
    report: &DagExecutionReport,
) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();

    values.insert("transition.id".to_string(), transition_id.to_string());

    values.insert("status".to_string(), phase.to_string());

    values.insert(
        "operations.total".to_string(),
        report.results().len().to_string(),
    );

    for (operation_id, result) in report.results() {
        values.insert(
            format!("operation.{operation_id}.status"),
            normalized_status_text(result.status()).to_string(),
        );

        for (key, value) in result.payload() {
            values.insert(
                format!("operation.{operation_id}.payload.{key}"),
                value.clone(),
            );
        }

        if let Some(error) = result.error() {
            values.insert(format!("operation.{operation_id}.error"), error.to_string());
        }
    }

    values
}

fn enter_running_state(
    transition_id: &str,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
    operation_total: usize,
) -> Result<(), String> {
    state.put("transition.id", transition_id)?;

    state.leave_operation();

    state.enter_phase("running")?;

    communication.refresh_state(state)?;

    communication.replace_failure(&BTreeMap::new())?;

    communication.replace_progress(&BTreeMap::from([
        ("total".to_string(), operation_total.to_string()),
        ("completed".to_string(), "0".to_string()),
        ("unit".to_string(), "operations".to_string()),
    ]))?;

    Ok(())
}

fn enter_infrastructure_failure(
    transition_id: &str,
    object_id: Option<&str>,
    error: &str,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
) -> Result<(), String> {
    state.leave_operation();

    state.enter_phase("failed")?;

    communication.refresh_state(state)?;

    let failure = FailureContext::infrastructure(transition_id, object_id, state, error)?;

    failure.attach(communication)
}

pub async fn execute(
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
) -> Result<TransitionExecutionReport, String> {
    execute_controlled(
        contract,
        transition_id,
        dependencies,
        battlefield,
        state,
        communication,
        fire_control,
        capabilities,
        BTreeMap::new(),
        CancellationToken::new(),
    )
    .await
}

pub async fn execute_from_available(
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
    available: Arc<AvailableCapabilityCatalog>,
) -> Result<TransitionExecutionReport, String> {
    execute_controlled_from_available(
        contract,
        transition_id,
        dependencies,
        battlefield,
        state,
        communication,
        available,
        BTreeMap::new(),
        CancellationToken::new(),
    )
    .await
}

pub async fn execute_controlled_from_available(
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
    available: Arc<AvailableCapabilityCatalog>,
    controls: BTreeMap<String, ExecutionControl>,
    cancellation: CancellationToken,
) -> Result<TransitionExecutionReport, String> {
    if transition_id.trim().is_empty() {
        return Err("lifecycle transition executor transition id cannot be empty".to_string());
    }

    let ir = lifecycle_ir::compile_transition(contract, transition_id, battlefield)?;

    execute_ir_from_available(
        ir,
        transition_id,
        None,
        dependencies,
        battlefield,
        state,
        communication,
        available,
        controls,
        cancellation,
    )
    .await
}

pub(crate) async fn execute_ir_from_available(
    ir: BattleplanIr,
    transition_id: &str,
    object_id: Option<&str>,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
    available: Arc<AvailableCapabilityCatalog>,
    controls: BTreeMap<String, ExecutionControl>,
    cancellation: CancellationToken,
) -> Result<TransitionExecutionReport, String> {
    if transition_id.trim().is_empty() {
        return Err("lifecycle transition executor transition id cannot be empty".to_string());
    }

    enter_running_state(transition_id, state, communication, ir.len())?;

    let report = match lifecycle_dag_executor::execute_controlled_from_available(
        Arc::new(ir),
        dependencies,
        battlefield,
        available,
        controls,
        cancellation,
    )
    .await
    {
        Ok(report) => report,

        Err(error) => {
            enter_infrastructure_failure(transition_id, object_id, &error, state, communication)?;

            return Err(format!(
                "lifecycle transition '{transition_id}' execution infrastructure failed: {error}"
            ));
        }
    };

    let phase = terminal_phase(&report);

    state.leave_operation();

    state.enter_phase(phase)?;

    communication.refresh_state(state)?;

    communication.replace_progress(&progress_values(&report))?;

    communication.replace_result(&result_values(transition_id, phase, &report))?;

    let failure = if phase == "failed" {
        let failure = FailureContext::from_operation_results(
            transition_id,
            object_id,
            state,
            report.results(),
        )?;

        failure.attach(communication)?;

        Some(failure)
    } else {
        None
    };

    Ok(TransitionExecutionReport {
        transition_id: transition_id.to_string(),

        terminal_phase: phase.to_string(),

        dag: report,

        failure,
    })
}

pub(crate) async fn execute_ir_controlled(
    ir: BattleplanIr,
    transition_id: &str,
    object_id: Option<&str>,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
    controls: BTreeMap<String, ExecutionControl>,
    cancellation: CancellationToken,
) -> Result<TransitionExecutionReport, String> {
    if transition_id.trim().is_empty() {
        return Err("lifecycle transition executor transition id cannot be empty".to_string());
    }

    enter_running_state(transition_id, state, communication, ir.len())?;

    let report = match lifecycle_dag_executor::execute_controlled(
        Arc::new(ir),
        dependencies,
        battlefield,
        fire_control,
        capabilities,
        controls,
        cancellation,
    )
    .await
    {
        Ok(report) => report,

        Err(error) => {
            enter_infrastructure_failure(transition_id, object_id, &error, state, communication)?;

            return Err(format!(
                "lifecycle transition '{transition_id}' execution infrastructure failed: {error}"
            ));
        }
    };

    let phase = terminal_phase(&report);

    state.leave_operation();

    state.enter_phase(phase)?;

    communication.refresh_state(state)?;

    communication.replace_progress(&progress_values(&report))?;

    communication.replace_result(&result_values(transition_id, phase, &report))?;

    let failure = if phase == "failed" {
        let failure = FailureContext::from_operation_results(
            transition_id,
            object_id,
            state,
            report.results(),
        )?;

        failure.attach(communication)?;

        Some(failure)
    } else {
        None
    };

    Ok(TransitionExecutionReport {
        transition_id: transition_id.to_string(),

        terminal_phase: phase.to_string(),

        dag: report,
        failure,
    })
}

pub async fn execute_controlled(
    contract: &LifecycleContract,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
    controls: BTreeMap<String, ExecutionControl>,
    cancellation: CancellationToken,
) -> Result<TransitionExecutionReport, String> {
    if transition_id.trim().is_empty() {
        return Err("lifecycle transition executor transition id cannot be empty".to_string());
    }

    /*
     * Compile before Runtime State mutation.
     *
     * The generic transition entry remains responsible only for
     * selecting the top-level Lifecycle transition.
     *
     * Actual execution authority lives in execute_ir_controlled so
     * object-scoped transitions can consume exactly the same engine.
     */
    let ir = lifecycle_ir::compile_transition(contract, transition_id, battlefield)?;

    execute_ir_controlled(
        ir,
        transition_id,
        None,
        dependencies,
        battlefield,
        state,
        communication,
        fire_control,
        capabilities,
        controls,
        cancellation,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::{BTreeMap, BTreeSet};

    use crate::lifecycle::{Battleplan, Operation};
    use crate::lifecycle_capabilities::CapabilityHandler;
    use crate::lifecycle_fire_control::FireControlTarget;

    fn operation(
        objective: &str,
        munition: BTreeMap<String, String>,
        intelligence: BTreeMap<String, String>,
    ) -> Operation {
        Operation {
            artillery: "synthetic".to_string(),

            objective: objective.to_string(),

            munition,

            tactics: BTreeMap::new(),

            intelligence,
        }
    }

    fn contract(transition_id: &str, operations: BTreeMap<String, Operation>) -> LifecycleContract {
        let mut contract = LifecycleContract::default();

        contract
            .transitions
            .insert(transition_id.to_string(), Battleplan { operations });

        contract
    }

    fn fire_control() -> Arc<FireControl> {
        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "synthetic",
                "first",
                FireControlTarget::new("implementation.first").unwrap(),
            )
            .unwrap();

        fire_control
            .register(
                "synthetic",
                "second",
                FireControlTarget::new("implementation.second").unwrap(),
            )
            .unwrap();

        fire_control
            .register(
                "synthetic",
                "fail",
                FireControlTarget::new("implementation.fail").unwrap(),
            )
            .unwrap();

        Arc::new(fire_control)
    }

    fn capabilities() -> Arc<CapabilityRegistry> {
        let mut registry = CapabilityRegistry::new();

        let first: CapabilityHandler = Arc::new(|_operation| {
            Box::pin(
                async move { Ok(BTreeMap::from([("value".to_string(), "alpha".to_string())])) },
            )
        });

        registry.register("implementation.first", first).unwrap();

        let second: CapabilityHandler = Arc::new(|operation| {
            Box::pin(async move {
                let seen = operation.munition.get("input").cloned().unwrap_or_default();

                Ok(BTreeMap::from([("seen".to_string(), seen)]))
            })
        });

        registry.register("implementation.second", second).unwrap();

        let fail: CapabilityHandler =
            Arc::new(|_operation| Box::pin(async move { Err("synthetic failure".to_string()) }));

        registry.register("implementation.fail", fail).unwrap();

        Arc::new(registry)
    }

    fn runtime(transition_id: &str) -> (LifecycleRuntimeState, LifecycleCommunication) {
        let state = LifecycleRuntimeState::new("execution-001", transition_id).unwrap();

        let communication = LifecycleCommunication::from_state(&state).unwrap();

        (state, communication)
    }

    #[test]
    fn dynamic_transition_id_is_not_hardcoded() {
        let transition = "future.custom.transition";

        let contract = contract(transition, BTreeMap::new());

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime(transition);

        let report = futures_lite::future::block_on(execute(
            &contract,
            transition,
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ))
        .unwrap();

        assert_eq!(report.transition_id(), transition);

        assert_eq!(report.terminal_phase(), "succeeded");

        assert_eq!(state.phase(), Some("succeeded"));
    }

    #[test]
    fn dependency_chain_consumes_runtime_intelligence() {
        let marker = char::from_u32(36).unwrap();

        let contract = contract(
            "deploy",
            BTreeMap::from([
                (
                    "first".to_string(),
                    operation(
                        "first",
                        BTreeMap::new(),
                        BTreeMap::from([("saved".to_string(), format!("{marker}result.value"))]),
                    ),
                ),
                (
                    "second".to_string(),
                    operation(
                        "second",
                        BTreeMap::from([(
                            "input".to_string(),
                            format!("{marker}intelligence.first.saved"),
                        )]),
                        BTreeMap::new(),
                    ),
                ),
            ]),
        );

        let dependencies = BTreeMap::from([
            ("first".to_string(), BTreeSet::new()),
            ("second".to_string(), BTreeSet::from(["first".to_string()])),
        ]);

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("deploy");

        let report = futures_lite::future::block_on(execute(
            &contract,
            "deploy",
            dependencies,
            &mut battlefield,
            &mut state,
            &mut communication,
            fire_control(),
            capabilities(),
        ))
        .unwrap();

        assert_eq!(report.terminal_phase(), "succeeded");

        assert_eq!(
            report.dag().result("second").unwrap().payload().get("seen"),
            Some(&"alpha".to_string())
        );

        assert_eq!(
            battlefield.get("intelligence", "first.saved",),
            Some("alpha")
        );

        assert_eq!(
            communication.get("result.operation.second.payload.seen"),
            Some("alpha")
        );
    }

    #[test]
    fn failed_operation_makes_transition_failed() {
        let contract = contract(
            "deploy",
            BTreeMap::from([(
                "broken".to_string(),
                operation("fail", BTreeMap::new(), BTreeMap::new()),
            )]),
        );

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("deploy");

        let report = futures_lite::future::block_on(execute(
            &contract,
            "deploy",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            fire_control(),
            capabilities(),
        ))
        .unwrap();

        assert_eq!(report.terminal_phase(), "failed");

        assert_eq!(state.phase(), Some("failed"));

        assert_eq!(
            communication.get("result.operation.broken.status"),
            Some("failed")
        );

        assert_eq!(
            communication.get("result.operation.broken.error"),
            Some("synthetic failure")
        );

        assert_eq!(communication.get("progress.failed"), Some("1"));
    }

    #[test]
    fn empty_transition_is_valid_successful_execution() {
        let contract = contract("noop", BTreeMap::new());

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("noop");

        let report = futures_lite::future::block_on(execute(
            &contract,
            "noop",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ))
        .unwrap();

        assert_eq!(report.dag().results().len(), 0);

        assert_eq!(report.terminal_phase(), "succeeded");

        assert_eq!(communication.get("progress.total"), Some("0"));
    }

    #[test]
    fn require_context_survives_transition_execution() {
        let contract = contract("deploy", BTreeMap::new());

        let mut battlefield = Battlefield::new();

        battlefield.insert("require", "current", "dependency");

        let (mut state, mut communication) = runtime("deploy");

        state
            .merge_namespace(
                "require",
                &BTreeMap::from([
                    ("root".to_string(), "root-module".to_string()),
                    ("current".to_string(), "dependency".to_string()),
                    ("path".to_string(), "root-module -> dependency".to_string()),
                ]),
            )
            .unwrap();

        communication.refresh_state(&state).unwrap();

        futures_lite::future::block_on(execute(
            &contract,
            "deploy",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ))
        .unwrap();

        assert_eq!(state.get("require.current"), Some("dependency"));

        assert_eq!(
            communication.get("state.require.current"),
            Some("dependency")
        );

        assert_eq!(battlefield.get("require", "current",), Some("dependency"));
    }

    #[test]
    fn unknown_transition_fails_before_runtime_mutation() {
        let contract = LifecycleContract::default();

        let mut battlefield = Battlefield::new();

        battlefield.insert("existing", "value", "kept");

        let (mut state, mut communication) = runtime("previous");

        let original_battlefield = battlefield.clone();

        let original_state = state.clone();

        let original_communication = communication.clone();

        let result = futures_lite::future::block_on(execute(
            &contract,
            "missing",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ));

        assert!(result.is_err());

        assert_eq!(battlefield, original_battlefield);

        assert_eq!(state, original_state);

        assert_eq!(communication, original_communication);
    }

    #[test]
    fn transition_scope_does_not_fake_single_current_operation() {
        let contract = contract(
            "deploy",
            BTreeMap::from([
                (
                    "first".to_string(),
                    operation("first", BTreeMap::new(), BTreeMap::new()),
                ),
                (
                    "second".to_string(),
                    operation("second", BTreeMap::new(), BTreeMap::new()),
                ),
            ]),
        );

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("deploy");

        futures_lite::future::block_on(execute(
            &contract,
            "deploy",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            fire_control(),
            capabilities(),
        ))
        .unwrap();

        assert!(!state.contains("operation.id"));

        assert_eq!(communication.get("state.operation.id"), None);
    }

    #[test]
    fn failed_transition_publishes_structured_failure_context() {
        let contract = contract(
            "deploy",
            BTreeMap::from([(
                "broken".to_string(),
                operation("fail", BTreeMap::new(), BTreeMap::new()),
            )]),
        );

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("deploy");

        let report = futures_lite::future::block_on(execute(
            &contract,
            "deploy",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            fire_control(),
            capabilities(),
        ))
        .unwrap();

        let failure = report.failure().unwrap();

        assert_eq!(failure.get("scope"), Some("operation"));

        assert_eq!(failure.get("transition.id"), Some("deploy"));

        assert_eq!(
            failure.get("operation.broken.error"),
            Some("synthetic failure")
        );

        assert_eq!(
            communication.get("failure.operation.broken.error"),
            Some("synthetic failure")
        );
    }

    #[test]
    fn successful_transition_clears_stale_failure_context() {
        let contract = contract("deploy", BTreeMap::new());

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("deploy");

        communication
            .replace_failure(&BTreeMap::from([(
                "error".to_string(),
                "stale".to_string(),
            )]))
            .unwrap();

        let report = futures_lite::future::block_on(execute(
            &contract,
            "deploy",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ))
        .unwrap();

        assert_eq!(report.failure(), None);

        assert_eq!(communication.get("failure.error"), None);
    }

    #[test]
    fn infrastructure_failure_publishes_distinct_failure_context() {
        let contract = contract(
            "deploy",
            BTreeMap::from([(
                "first".to_string(),
                operation("first", BTreeMap::new(), BTreeMap::new()),
            )]),
        );

        let dependencies = BTreeMap::from([("ghost".to_string(), BTreeSet::new())]);

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("deploy");

        let result = futures_lite::future::block_on(execute(
            &contract,
            "deploy",
            dependencies,
            &mut battlefield,
            &mut state,
            &mut communication,
            fire_control(),
            capabilities(),
        ));

        assert!(result.is_err());

        assert_eq!(state.phase(), Some("failed"));

        assert_eq!(communication.get("failure.scope"), Some("infrastructure"));

        assert_eq!(communication.get("failure.transition.id"), Some("deploy"));

        assert!(communication.get("failure.error").is_some());
    }

    #[test]
    fn cancelled_transition_is_not_failure_context() {
        let contract = contract(
            "deploy",
            BTreeMap::from([(
                "first".to_string(),
                operation("first", BTreeMap::new(), BTreeMap::new()),
            )]),
        );

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("deploy");

        let cancellation = CancellationToken::new();

        cancellation.cancel();

        let report = futures_lite::future::block_on(execute_controlled(
            &contract,
            "deploy",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            fire_control(),
            capabilities(),
            BTreeMap::new(),
            cancellation,
        ))
        .unwrap();

        assert_eq!(report.terminal_phase(), "cancelled");

        assert_eq!(report.failure(), None);

        assert_eq!(communication.get("failure.status"), None);
    }
}

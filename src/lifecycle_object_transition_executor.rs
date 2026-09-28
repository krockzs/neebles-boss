use std::collections::BTreeMap;
use std::sync::Arc;

use crate::lifecycle::LifecycleContract;
use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_capabilities::CapabilityRegistry;
use crate::lifecycle_communication::LifecycleCommunication;
use crate::lifecycle_control::{CancellationToken, ExecutionControl};
use crate::lifecycle_dag_executor::LifecycleDependencyGraph;
use crate::lifecycle_fire_control::FireControl;
use crate::lifecycle_ir;
use crate::lifecycle_state::LifecycleRuntimeState;
use crate::lifecycle_transition_executor::{self, TransitionExecutionReport};

/*
 * N.E.E.B.L.E.S. Lifecycle Object Transition Executor.
 *
 * Technical responsibility:
 *
 * LifecycleContract
 *      + arbitrary object id
 *      + arbitrary transition id
 *      -> compile_object_transition
 *      -> shared Transition Execution Core
 *      -> existing DAG
 *      -> existing runtime/result/intelligence machinery
 *
 * Object identity is execution context.
 * Object semantics remain module-owned and opaque to Lifecycle.
 *
 * This layer does not:
 * - prescribe object names
 * - prescribe object transition names
 * - infer active/inactive semantics
 * - interpret tactics
 * - create a second DAG executor
 * - create a second result pipeline
 * - know Governor
 * - know UI
 * - know IPC
 * - know Telemetry
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectTransitionExecutionReport {
    object_id: String,
    transition: TransitionExecutionReport,
}

impl ObjectTransitionExecutionReport {
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    pub fn transition(&self) -> &TransitionExecutionReport {
        &self.transition
    }
}

fn enter_object_scope(
    object_id: &str,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
) -> Result<(), String> {
    let mut staged_state = state.clone();

    let mut staged_communication = communication.clone();

    staged_state.enter_object(object_id)?;

    staged_communication.refresh_state(&staged_state)?;

    *state = staged_state;

    *communication = staged_communication;

    Ok(())
}

fn attach_object_result_identity(
    object_id: &str,
    communication: &mut LifecycleCommunication,
) -> Result<(), String> {
    let mut staged = communication.clone();

    staged.put("result.object.id", object_id)?;

    *communication = staged;

    Ok(())
}

pub async fn execute(
    contract: &LifecycleContract,
    object_id: &str,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
) -> Result<ObjectTransitionExecutionReport, String> {
    execute_controlled(
        contract,
        object_id,
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

pub async fn execute_controlled(
    contract: &LifecycleContract,
    object_id: &str,
    transition_id: &str,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
    controls: BTreeMap<String, ExecutionControl>,
    cancellation: CancellationToken,
) -> Result<ObjectTransitionExecutionReport, String> {
    if object_id.trim().is_empty() {
        return Err("lifecycle object transition executor object id cannot be empty".to_string());
    }

    if transition_id.trim().is_empty() {
        return Err(
            "lifecycle object transition executor transition id cannot be empty".to_string(),
        );
    }

    /*
     * Object and transition selection happens before any runtime
     * mutation. An unknown object or unknown object transition cannot
     * claim execution state.
     */
    let ir =
        lifecycle_ir::compile_object_transition(contract, object_id, transition_id, battlefield)?;

    enter_object_scope(object_id, state, communication)?;

    let transition = lifecycle_transition_executor::execute_ir_controlled(
        ir,
        transition_id,
        Some(object_id),
        dependencies,
        battlefield,
        state,
        communication,
        fire_control,
        capabilities,
        controls,
        cancellation,
    )
    .await?;

    attach_object_result_identity(object_id, communication)?;

    Ok(ObjectTransitionExecutionReport {
        object_id: object_id.to_string(),

        transition,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle::{Battleplan, ObjectContract, Operation};
    use crate::lifecycle_capabilities::CapabilityHandler;
    use crate::lifecycle_fire_control::FireControlTarget;

    fn operation(objective: &str, munition: BTreeMap<String, String>) -> Operation {
        Operation {
            artillery: "synthetic".to_string(),

            objective: objective.to_string(),

            munition,

            tactics: BTreeMap::new(),

            intelligence: BTreeMap::new(),
        }
    }

    fn contract(
        object_id: &str,
        transition_id: &str,
        operations: BTreeMap<String, Operation>,
    ) -> LifecycleContract {
        let mut contract = LifecycleContract::default();

        contract.objects.insert(
            object_id.to_string(),
            ObjectContract {
                transitions: BTreeMap::from([(
                    transition_id.to_string(),
                    Battleplan { operations },
                )]),
            },
        );

        contract
    }

    fn runtime(transition_id: &str) -> (LifecycleRuntimeState, LifecycleCommunication) {
        let state = LifecycleRuntimeState::new("execution-object-001", transition_id).unwrap();

        let communication = LifecycleCommunication::from_state(&state).unwrap();

        (state, communication)
    }

    fn fire_control() -> Arc<FireControl> {
        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "synthetic",
                "echo",
                FireControlTarget::new("implementation.echo").unwrap(),
            )
            .unwrap();

        Arc::new(fire_control)
    }

    fn capabilities() -> Arc<CapabilityRegistry> {
        let mut registry = CapabilityRegistry::new();

        let handler: CapabilityHandler = Arc::new(|operation| {
            Box::pin(async move {
                Ok(BTreeMap::from([(
                    "seen".to_string(),
                    operation.munition.get("value").cloned().unwrap_or_default(),
                )]))
            })
        });

        registry.register("implementation.echo", handler).unwrap();

        Arc::new(registry)
    }

    #[test]
    fn dynamic_object_and_transition_are_not_hardcoded() {
        let object_id = "future.object.kind";

        let transition_id = "future.object.transition";

        let contract = contract(object_id, transition_id, BTreeMap::new());

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime(transition_id);

        let report = futures_lite::future::block_on(execute(
            &contract,
            object_id,
            transition_id,
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ))
        .unwrap();

        assert_eq!(report.object_id(), object_id);

        assert_eq!(report.transition().transition_id(), transition_id);

        assert_eq!(report.transition().terminal_phase(), "succeeded");
    }

    #[test]
    fn object_identity_reaches_state_communication_and_result() {
        let object_id = "worker.alpha";

        let contract = contract(
            object_id,
            "activate",
            BTreeMap::from([(
                "echo".to_string(),
                operation(
                    "echo",
                    BTreeMap::from([("value".to_string(), "hello".to_string())]),
                ),
            )]),
        );

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("activate");

        let report = futures_lite::future::block_on(execute(
            &contract,
            object_id,
            "activate",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            fire_control(),
            capabilities(),
        ))
        .unwrap();

        assert_eq!(state.get("object.id"), Some(object_id));

        assert_eq!(communication.get("state.object.id"), Some(object_id));

        assert_eq!(communication.get("result.object.id"), Some(object_id));

        assert_eq!(
            report
                .transition()
                .dag()
                .result("echo")
                .unwrap()
                .payload()
                .get("seen"),
            Some(&"hello".to_string())
        );
    }

    #[test]
    fn top_level_transition_with_same_name_is_not_used() {
        let mut contract = contract("object.alpha", "deploy", BTreeMap::new());

        contract.transitions.insert(
            "deploy".to_string(),
            Battleplan {
                operations: BTreeMap::from([(
                    "trap".to_string(),
                    operation("route-that-does-not-exist", BTreeMap::new()),
                )]),
            },
        );

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("deploy");

        let report = futures_lite::future::block_on(execute(
            &contract,
            "object.alpha",
            "deploy",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ))
        .unwrap();

        assert_eq!(report.transition().terminal_phase(), "succeeded");

        assert_eq!(report.transition().dag().results().len(), 0);
    }

    #[test]
    fn unknown_object_fails_before_runtime_mutation() {
        let contract = LifecycleContract::default();

        let mut battlefield = Battlefield::new();

        battlefield.insert("existing", "value", "kept");

        let (mut state, mut communication) = runtime("previous");

        let original_battlefield = battlefield.clone();

        let original_state = state.clone();

        let original_communication = communication.clone();

        let result = futures_lite::future::block_on(execute(
            &contract,
            "missing-object",
            "anything",
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
    fn unknown_object_transition_fails_before_runtime_mutation() {
        let contract = contract("object.alpha", "existing", BTreeMap::new());

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("previous");

        let original_battlefield = battlefield.clone();

        let original_state = state.clone();

        let original_communication = communication.clone();

        let result = futures_lite::future::block_on(execute(
            &contract,
            "object.alpha",
            "missing-transition",
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
    fn failed_object_operation_keeps_object_context() {
        let contract = contract(
            "object.alpha",
            "activate",
            BTreeMap::from([(
                "broken".to_string(),
                operation("missing-route", BTreeMap::new()),
            )]),
        );

        let mut battlefield = Battlefield::new();

        let (mut state, mut communication) = runtime("activate");

        let report = futures_lite::future::block_on(execute(
            &contract,
            "object.alpha",
            "activate",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ))
        .unwrap();

        assert_eq!(report.transition().terminal_phase(), "failed");

        assert_eq!(state.phase(), Some("failed"));

        assert_eq!(state.get("object.id"), Some("object.alpha"));

        assert_eq!(communication.get("result.object.id"), Some("object.alpha"));

        assert_eq!(
            communication.get("failure.object.id"),
            communication.get("result.object.id")
        );

        assert_eq!(
            report
                .transition()
                .failure()
                .and_then(|failure| failure.get("object.id")),
            communication.get("result.object.id")
        );
    }

    #[test]
    fn require_context_survives_object_transition() {
        let contract = contract("object.alpha", "activate", BTreeMap::new());

        let mut battlefield = Battlefield::new();

        battlefield.insert("require", "current", "dependency.alpha");

        let (mut state, mut communication) = runtime("activate");

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

        communication.refresh_state(&state).unwrap();

        futures_lite::future::block_on(execute(
            &contract,
            "object.alpha",
            "activate",
            BTreeMap::new(),
            &mut battlefield,
            &mut state,
            &mut communication,
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ))
        .unwrap();

        assert_eq!(state.get("require.current"), Some("dependency.alpha"));

        assert_eq!(
            communication.get("state.require.current"),
            Some("dependency.alpha")
        );

        assert_eq!(
            battlefield.get("require", "current",),
            Some("dependency.alpha")
        );
    }
}

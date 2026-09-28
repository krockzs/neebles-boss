use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use crate::lifecycle_available_capabilities::AvailableCapabilityCatalog;
use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_capabilities::CapabilityRegistry;
use crate::lifecycle_control::{CancellationToken, ExecutionControl};
use crate::lifecycle_execution::ExecutionOutcome;
use crate::lifecycle_executor;
use crate::lifecycle_fire_control::FireControl;
use crate::lifecycle_intelligence;
use crate::lifecycle_ir::BattleplanIr;
use crate::lifecycle_orchestration::{
    self, OrchestrationHandler, OrchestrationPlan, OrchestrationReport, OrchestrationTask,
};
use crate::lifecycle_result::NormalizedResult;

/*
 * Lifecycle DAG executor.
 *
 * Technical responsibility:
 *
 * BattleplanIr
 *      + explicit dependency graph
 *      + current Battlefield
 *      + Fire Control
 *      + Capability Registry
 *      + Execution Control
 *      -> Orchestration DAG
 *      -> lazy operation preparation
 *      -> capability execution
 *      -> normalized result
 *      -> intelligence propagation
 *      -> updated Battlefield
 *
 * Dependency information remains explicit external data.
 * This layer does not infer execution order from BTreeMap storage
 * and does not assign special meanings to tactics keys.
 *
 * Ready operations are delegated to the existing Orchestration engine.
 * Independent ready operations may execute concurrently.
 *
 * Battlefield locking is limited to:
 * - lazy preparation
 * - post-execution intelligence propagation
 *
 * No Battlefield lock is held while a capability is awaiting execution.
 */

pub type LifecycleDependencyGraph = BTreeMap<String, BTreeSet<String>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagExecutionReport {
    orchestration: OrchestrationReport,
    results: BTreeMap<String, NormalizedResult>,
}

impl DagExecutionReport {
    pub fn result(&self, operation_id: &str) -> Option<&NormalizedResult> {
        self.results.get(operation_id)
    }

    pub fn succeeded(&self, operation_id: &str) -> bool {
        self.results
            .get(operation_id)
            .map(NormalizedResult::is_success)
            .unwrap_or(false)
    }

    pub fn failed(&self, operation_id: &str) -> bool {
        self.results
            .get(operation_id)
            .map(NormalizedResult::is_failure)
            .unwrap_or(false)
    }

    pub fn cancelled(&self, operation_id: &str) -> bool {
        self.results
            .get(operation_id)
            .map(NormalizedResult::is_cancelled)
            .unwrap_or(false)
    }

    pub fn orchestration(&self) -> &OrchestrationReport {
        &self.orchestration
    }

    pub fn results(&self) -> &BTreeMap<String, NormalizedResult> {
        &self.results
    }
}

fn lock_error(label: &str) -> String {
    format!("lifecycle DAG internal {label} lock was poisoned")
}

fn validate_external_maps(
    ir: &BattleplanIr,
    dependencies: &LifecycleDependencyGraph,
    controls: &BTreeMap<String, ExecutionControl>,
) -> Result<(), String> {
    for operation_id in dependencies.keys() {
        if !ir.contains(operation_id) {
            return Err(format!(
                "lifecycle dependency graph references unknown operation '{operation_id}'"
            ));
        }
    }

    for operation_id in controls.keys() {
        if !ir.contains(operation_id) {
            return Err(format!(
                "lifecycle execution control references unknown operation '{operation_id}'"
            ));
        }
    }

    Ok(())
}

fn complete_normalized_results(
    ir: &BattleplanIr,
    orchestration: &OrchestrationReport,
    mut results: BTreeMap<String, NormalizedResult>,
) -> Result<BTreeMap<String, NormalizedResult>, String> {
    for operation_id in ir.operations().keys() {
        if results.contains_key(operation_id) {
            continue;
        }

        let Some(outcome) = orchestration.get(operation_id) else {
            return Err(format!(
                "lifecycle DAG report lost operation '{operation_id}'"
            ));
        };

        let normalized = match outcome {
            ExecutionOutcome::Success(payload) => NormalizedResult::succeeded(payload.clone()),

            ExecutionOutcome::Failure(error) => NormalizedResult::failed(error.clone())?,
        };

        results.insert(operation_id.clone(), normalized);
    }

    Ok(results)
}

pub async fn execute(
    ir: Arc<BattleplanIr>,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
) -> Result<DagExecutionReport, String> {
    execute_controlled(
        ir,
        dependencies,
        battlefield,
        fire_control,
        capabilities,
        BTreeMap::new(),
        CancellationToken::new(),
    )
    .await
}

pub async fn execute_controlled(
    ir: Arc<BattleplanIr>,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
    controls: BTreeMap<String, ExecutionControl>,
    cancellation: CancellationToken,
) -> Result<DagExecutionReport, String> {
    execute_controlled_internal(
        ir,
        dependencies,
        battlefield,
        Some((fire_control, capabilities)),
        None,
        controls,
        cancellation,
    )
    .await
}

pub async fn execute_from_available(
    ir: Arc<BattleplanIr>,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    available: Arc<AvailableCapabilityCatalog>,
) -> Result<DagExecutionReport, String> {
    execute_controlled_from_available(
        ir,
        dependencies,
        battlefield,
        available,
        BTreeMap::new(),
        CancellationToken::new(),
    )
    .await
}

pub async fn execute_controlled_from_available(
    ir: Arc<BattleplanIr>,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    available: Arc<AvailableCapabilityCatalog>,
    controls: BTreeMap<String, ExecutionControl>,
    cancellation: CancellationToken,
) -> Result<DagExecutionReport, String> {
    execute_controlled_internal(
        ir,
        dependencies,
        battlefield,
        None,
        Some(available),
        controls,
        cancellation,
    )
    .await
}

async fn execute_controlled_internal(
    ir: Arc<BattleplanIr>,
    dependencies: LifecycleDependencyGraph,
    battlefield: &mut Battlefield,
    static_arsenal: Option<(Arc<FireControl>, Arc<CapabilityRegistry>)>,
    available: Option<Arc<AvailableCapabilityCatalog>>,
    controls: BTreeMap<String, ExecutionControl>,
    cancellation: CancellationToken,
) -> Result<DagExecutionReport, String> {
    validate_external_maps(&ir, &dependencies, &controls)?;

    let shared_battlefield = Arc::new(Mutex::new(battlefield.clone()));

    let shared_results = Arc::new(Mutex::new(BTreeMap::<String, NormalizedResult>::new()));

    let controls = Arc::new(controls);

    let mut plan = OrchestrationPlan::new();

    for operation_id in ir.operations().keys() {
        let operation_id = operation_id.clone();

        let operation_dependencies = dependencies.get(&operation_id).cloned().unwrap_or_default();

        let task_ir = Arc::clone(&ir);

        let task_battlefield = Arc::clone(&shared_battlefield);

        let task_static_arsenal = static_arsenal.as_ref().map(|(fire_control, capabilities)| {
            (Arc::clone(fire_control), Arc::clone(capabilities))
        });

        let task_available = available.as_ref().map(Arc::clone);

        let task_controls = Arc::clone(&controls);

        let task_cancellation = cancellation.clone();

        let task_results = Arc::clone(&shared_results);

        let handler_operation_id = operation_id.clone();

        let handler: OrchestrationHandler = Arc::new(move || {
            let operation_id = handler_operation_id.clone();

            let ir = Arc::clone(&task_ir);

            let battlefield = Arc::clone(&task_battlefield);

            let static_arsenal =
                task_static_arsenal
                    .as_ref()
                    .map(|(fire_control, capabilities)| {
                        (Arc::clone(fire_control), Arc::clone(capabilities))
                    });

            let available = task_available.as_ref().map(Arc::clone);

            let controls = Arc::clone(&task_controls);

            let cancellation = task_cancellation.clone();

            let results = Arc::clone(&task_results);

            Box::pin(async move {
                let operation = {
                    let battlefield = battlefield.lock().map_err(|_| lock_error("Battlefield"))?;

                    ir.prepare_operation(&operation_id, &battlefield)?
                };

                let intelligence = operation.intelligence.clone();

                let control = controls.get(&operation_id).cloned().unwrap_or_default();

                let result = match (static_arsenal, available) {
                    (Some((fire_control, capabilities)), None) => {
                        lifecycle_executor::execute_prepared_controlled(
                            operation,
                            fire_control,
                            capabilities,
                            control,
                            cancellation,
                        )
                        .await?
                    }

                    (None, Some(available)) => {
                        lifecycle_executor::execute_prepared_from_available_controlled(
                            operation,
                            available,
                            control,
                            cancellation,
                        )
                        .await?
                    }

                    _ => {
                        return Err(
                            "lifecycle DAG executor received invalid capability source".to_string()
                        );
                    }
                };

                {
                    let mut battlefield =
                        battlefield.lock().map_err(|_| lock_error("Battlefield"))?;

                    lifecycle_intelligence::propagate(
                        &operation_id,
                        &intelligence,
                        &result,
                        &mut battlefield,
                    )?;
                }

                {
                    let mut results = results
                        .lock()
                        .map_err(|_| lock_error("normalized result"))?;

                    results.insert(operation_id.clone(), result.clone());
                }

                if result.is_success() {
                    return Ok(result.payload().clone());
                }

                Err(result
                    .error()
                    .unwrap_or("lifecycle operation did not succeed")
                    .to_string())
            })
        });

        plan.register(OrchestrationTask::new(
            operation_id,
            operation_dependencies,
            handler,
        )?)?;
    }

    let orchestration_result = lifecycle_orchestration::execute(plan).await;

    let final_battlefield = shared_battlefield
        .lock()
        .map_err(|_| lock_error("Battlefield"))?
        .clone();

    *battlefield = final_battlefield;

    let orchestration = orchestration_result?;

    let results = shared_results
        .lock()
        .map_err(|_| lock_error("normalized result"))?
        .clone();

    let results = complete_normalized_results(&ir, &orchestration, results)?;

    Ok(DagExecutionReport {
        orchestration,
        results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::lifecycle::{Battleplan, Operation};

    use crate::lifecycle_fire_control::FireControlTarget;

    use crate::lifecycle_ir;

    fn operation(artillery: &str, objective: &str) -> Operation {
        Operation {
            artillery: artillery.to_string(),

            objective: objective.to_string(),

            munition: BTreeMap::new(),

            tactics: BTreeMap::new(),

            intelligence: BTreeMap::new(),
        }
    }

    fn compile(battleplan: &Battleplan) -> Arc<BattleplanIr> {
        Arc::new(lifecycle_ir::compile_battleplan(battleplan, &Battlefield::new()).unwrap())
    }

    #[test]
    fn dependency_chain_materializes_after_upstream_intelligence() {
        let marker = char::from_u32(36).unwrap();

        let mut first = operation("producer", "create");

        first
            .intelligence
            .insert("output".to_string(), format!("{marker}result.value"));

        let mut second = operation("consumer", "consume");

        second.munition.insert(
            "input".to_string(),
            format!("{marker}intelligence.first.output"),
        );

        let battleplan = Battleplan {
            operations: BTreeMap::from([
                ("first".to_string(), first),
                ("second".to_string(), second),
            ]),
        };

        let ir = compile(&battleplan);

        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "producer",
                "create",
                FireControlTarget::new("dag.producer").unwrap(),
            )
            .unwrap();

        fire_control
            .register(
                "consumer",
                "consume",
                FireControlTarget::new("dag.consumer").unwrap(),
            )
            .unwrap();

        let mut capabilities = CapabilityRegistry::new();

        capabilities
            .register(
                "dag.producer",
                Arc::new(|_| {
                    Box::pin(async {
                        Ok(BTreeMap::from([(
                            "value".to_string(),
                            "CHAINED".to_string(),
                        )]))
                    })
                }),
            )
            .unwrap();

        capabilities
            .register(
                "dag.consumer",
                Arc::new(|operation| {
                    Box::pin(async move {
                        let input = operation
                            .munition
                            .get("input")
                            .cloned()
                            .ok_or_else(|| "missing consumer input".to_string())?;

                        Ok(BTreeMap::from([("received".to_string(), input)]))
                    })
                }),
            )
            .unwrap();

        let dependencies = BTreeMap::from([(
            "second".to_string(),
            BTreeSet::from([("first".to_string())]),
        )]);

        let mut battlefield = Battlefield::new();

        let report = futures_lite::future::block_on(execute(
            ir,
            dependencies,
            &mut battlefield,
            Arc::new(fire_control),
            Arc::new(capabilities),
        ))
        .unwrap();

        assert!(report.succeeded("first"));

        assert!(report.succeeded("second"));

        assert_eq!(
            report.result("second").unwrap().payload().get("received"),
            Some(&"CHAINED".to_string())
        );

        assert_eq!(
            battlefield.get("intelligence", "first.output",),
            Some("CHAINED")
        );

        assert!(!battlefield.contains_namespace("result"));
    }

    #[test]
    fn join_consumes_intelligence_from_both_dependencies() {
        let marker = char::from_u32(36).unwrap();

        let mut left = operation("producer", "left");

        left.intelligence
            .insert("value".to_string(), format!("{marker}result.value"));

        let mut right = operation("producer", "right");

        right
            .intelligence
            .insert("value".to_string(), format!("{marker}result.value"));

        let mut join = operation("join", "consume");

        join.munition.insert(
            "left".to_string(),
            format!("{marker}intelligence.left.value"),
        );

        join.munition.insert(
            "right".to_string(),
            format!("{marker}intelligence.right.value"),
        );

        let battleplan = Battleplan {
            operations: BTreeMap::from([
                ("left".to_string(), left),
                ("right".to_string(), right),
                ("join".to_string(), join),
            ]),
        };

        let ir = compile(&battleplan);

        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "producer",
                "left",
                FireControlTarget::new("dag.left").unwrap(),
            )
            .unwrap();

        fire_control
            .register(
                "producer",
                "right",
                FireControlTarget::new("dag.right").unwrap(),
            )
            .unwrap();

        fire_control
            .register(
                "join",
                "consume",
                FireControlTarget::new("dag.join").unwrap(),
            )
            .unwrap();

        let mut capabilities = CapabilityRegistry::new();

        capabilities
            .register(
                "dag.left",
                Arc::new(|_| {
                    Box::pin(async {
                        Ok(BTreeMap::from([("value".to_string(), "LEFT".to_string())]))
                    })
                }),
            )
            .unwrap();

        capabilities
            .register(
                "dag.right",
                Arc::new(|_| {
                    Box::pin(async {
                        Ok(BTreeMap::from([("value".to_string(), "RIGHT".to_string())]))
                    })
                }),
            )
            .unwrap();

        capabilities
            .register(
                "dag.join",
                Arc::new(|operation| {
                    Box::pin(async move {
                        let left = operation
                            .munition
                            .get("left")
                            .cloned()
                            .ok_or_else(|| "left input missing".to_string())?;

                        let right = operation
                            .munition
                            .get("right")
                            .cloned()
                            .ok_or_else(|| "right input missing".to_string())?;

                        Ok(BTreeMap::from([
                            ("left".to_string(), left),
                            ("right".to_string(), right),
                        ]))
                    })
                }),
            )
            .unwrap();

        let dependencies = BTreeMap::from([(
            "join".to_string(),
            BTreeSet::from(["left".to_string(), "right".to_string()]),
        )]);

        let mut battlefield = Battlefield::new();

        let report = futures_lite::future::block_on(execute(
            ir,
            dependencies,
            &mut battlefield,
            Arc::new(fire_control),
            Arc::new(capabilities),
        ))
        .unwrap();

        assert!(report.succeeded("left"));

        assert!(report.succeeded("right"));

        assert!(report.succeeded("join"));

        let joined = report.result("join").unwrap().payload();

        assert_eq!(joined.get("left"), Some(&"LEFT".to_string()));

        assert_eq!(joined.get("right"), Some(&"RIGHT".to_string()));
    }

    #[test]
    fn failed_dependency_blocks_dependent_capability() {
        let calls = Arc::new(AtomicUsize::new(0));

        let battleplan = Battleplan {
            operations: BTreeMap::from([
                ("failure".to_string(), operation("producer", "fail")),
                ("dependent".to_string(), operation("consumer", "run")),
            ]),
        };

        let ir = compile(&battleplan);

        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "producer",
                "fail",
                FireControlTarget::new("dag.failure").unwrap(),
            )
            .unwrap();

        fire_control
            .register(
                "consumer",
                "run",
                FireControlTarget::new("dag.dependent").unwrap(),
            )
            .unwrap();

        let mut capabilities = CapabilityRegistry::new();

        capabilities
            .register(
                "dag.failure",
                Arc::new(|_| Box::pin(async { Err("synthetic failure".to_string()) })),
            )
            .unwrap();

        let dependent_calls = Arc::clone(&calls);

        capabilities
            .register(
                "dag.dependent",
                Arc::new(move |_| {
                    let calls = Arc::clone(&dependent_calls);

                    Box::pin(async move {
                        calls.fetch_add(1, Ordering::SeqCst);

                        Ok(BTreeMap::new())
                    })
                }),
            )
            .unwrap();

        let dependencies = BTreeMap::from([(
            "dependent".to_string(),
            BTreeSet::from([("failure".to_string())]),
        )]);

        let mut battlefield = Battlefield::new();

        let report = futures_lite::future::block_on(execute(
            ir,
            dependencies,
            &mut battlefield,
            Arc::new(fire_control),
            Arc::new(capabilities),
        ))
        .unwrap();

        assert!(report.failed("failure"));

        assert!(report.failed("dependent"));

        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn controlled_retry_can_feed_dependent_operation() {
        let marker = char::from_u32(36).unwrap();

        let attempts = Arc::new(AtomicUsize::new(0));

        let mut producer = operation("producer", "retry");

        producer
            .intelligence
            .insert("output".to_string(), format!("{marker}result.value"));

        let mut consumer = operation("consumer", "consume");

        consumer.munition.insert(
            "input".to_string(),
            format!("{marker}intelligence.producer.output"),
        );

        let battleplan = Battleplan {
            operations: BTreeMap::from([
                ("producer".to_string(), producer),
                ("consumer".to_string(), consumer),
            ]),
        };

        let ir = compile(&battleplan);

        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "producer",
                "retry",
                FireControlTarget::new("dag.retry").unwrap(),
            )
            .unwrap();

        fire_control
            .register(
                "consumer",
                "consume",
                FireControlTarget::new("dag.retry.consumer").unwrap(),
            )
            .unwrap();

        let mut capabilities = CapabilityRegistry::new();

        let producer_attempts = Arc::clone(&attempts);

        capabilities
            .register(
                "dag.retry",
                Arc::new(move |_| {
                    let attempts = Arc::clone(&producer_attempts);

                    Box::pin(async move {
                        let attempt = attempts.fetch_add(1, Ordering::SeqCst);

                        if attempt == 0 {
                            return Err("first attempt".to_string());
                        }

                        Ok(BTreeMap::from([(
                            "value".to_string(),
                            "RECOVERED".to_string(),
                        )]))
                    })
                }),
            )
            .unwrap();

        capabilities
            .register(
                "dag.retry.consumer",
                Arc::new(|operation| {
                    Box::pin(async move {
                        let input = operation
                            .munition
                            .get("input")
                            .cloned()
                            .ok_or_else(|| "retry consumer input missing".to_string())?;

                        Ok(BTreeMap::from([("received".to_string(), input)]))
                    })
                }),
            )
            .unwrap();

        let dependencies = BTreeMap::from([(
            "consumer".to_string(),
            BTreeSet::from([("producer".to_string())]),
        )]);

        let controls = BTreeMap::from([(
            "producer".to_string(),
            ExecutionControl::new().attempts(2).unwrap(),
        )]);

        let mut battlefield = Battlefield::new();

        let report = futures_lite::future::block_on(execute_controlled(
            ir,
            dependencies,
            &mut battlefield,
            Arc::new(fire_control),
            Arc::new(capabilities),
            controls,
            CancellationToken::new(),
        ))
        .unwrap();

        assert!(report.succeeded("producer"));

        assert!(report.succeeded("consumer"));

        assert_eq!(attempts.load(Ordering::SeqCst), 2);

        assert_eq!(
            report.result("consumer").unwrap().payload().get("received"),
            Some(&"RECOVERED".to_string())
        );
    }

    #[test]
    fn pre_cancelled_execution_preserves_cancelled_status() {
        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "operation".to_string(),
                operation("artillery", "objective"),
            )]),
        };

        let ir = compile(&battleplan);

        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "artillery",
                "objective",
                FireControlTarget::new("dag.cancelled").unwrap(),
            )
            .unwrap();

        let mut capabilities = CapabilityRegistry::new();

        capabilities
            .register(
                "dag.cancelled",
                Arc::new(|_| Box::pin(async { Ok(BTreeMap::new()) })),
            )
            .unwrap();

        let cancellation = CancellationToken::new();

        cancellation.cancel();

        let mut battlefield = Battlefield::new();

        let report = futures_lite::future::block_on(execute_controlled(
            ir,
            BTreeMap::new(),
            &mut battlefield,
            Arc::new(fire_control),
            Arc::new(capabilities),
            BTreeMap::new(),
            cancellation,
        ))
        .unwrap();

        assert!(report.cancelled("operation"));
    }

    #[test]
    fn unknown_dependency_graph_operation_is_rejected() {
        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "known".to_string(),
                operation("artillery", "objective"),
            )]),
        };

        let ir = compile(&battleplan);

        let error = futures_lite::future::block_on(execute(
            ir,
            BTreeMap::from([("unknown".to_string(), BTreeSet::new())]),
            &mut Battlefield::new(),
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ))
        .unwrap_err();

        assert!(error.contains("unknown operation"));
    }

    #[test]
    fn unknown_dependency_target_is_rejected_by_orchestration() {
        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "known".to_string(),
                operation("artillery", "objective"),
            )]),
        };

        let ir = compile(&battleplan);

        let error = futures_lite::future::block_on(execute(
            ir,
            BTreeMap::from([(
                "known".to_string(),
                BTreeSet::from([("missing".to_string())]),
            )]),
            &mut Battlefield::new(),
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
        ))
        .unwrap_err();

        assert!(error.contains("unknown task"));
    }

    #[test]
    fn unknown_execution_control_operation_is_rejected() {
        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "known".to_string(),
                operation("artillery", "objective"),
            )]),
        };

        let ir = compile(&battleplan);

        let error = futures_lite::future::block_on(execute_controlled(
            ir,
            BTreeMap::new(),
            &mut Battlefield::new(),
            Arc::new(FireControl::new()),
            Arc::new(CapabilityRegistry::new()),
            BTreeMap::from([("unknown".to_string(), ExecutionControl::new())]),
            CancellationToken::new(),
        ))
        .unwrap_err();

        assert!(error.contains("unknown operation"));
    }
}

use std::sync::Arc;

use crate::lifecycle_arsenal::LifecycleArsenal;
use crate::lifecycle_available_capabilities::AvailableCapabilityCatalog;
use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_capabilities::CapabilityRegistry;
use crate::lifecycle_control::{self, CancellationToken, ControlledHandler, ExecutionControl};
use crate::lifecycle_fire_control::FireControl;
use crate::lifecycle_intelligence;
use crate::lifecycle_ir::BattleplanIr;
use crate::lifecycle_operation::PreparedOperation;
use crate::lifecycle_result::NormalizedResult;

/*
 * Lifecycle generic operation executor.
 *
 * Technical responsibility:
 *
 * PreparedOperation
 *      -> Fire Control route resolution
 *      -> opaque implementation id
 *      -> Capability Registry invocation
 *      -> NormalizedResult
 *
 * This is the first execution layer that connects the previously
 * independent Lifecycle infrastructure pieces.
 *
 * This layer deliberately does not:
 * - interpret battleplan ordering
 * - interpret tactics
 * - interpret intelligence
 * - prescribe artillery names
 * - prescribe objective names
 * - understand module technologies
 * - persist outputs
 * - update object state
 * - choose human presentation
 *
 * Battleplan orchestration remains a separate concern because
 * BTreeMap ordering is not an execution contract.
 */

fn normalized_failure(error: impl Into<String>) -> Result<NormalizedResult, String> {
    let error = error.into();

    NormalizedResult::failed(error).map_err(|normalization_error| {
        format!("could not normalize lifecycle execution failure: {normalization_error}")
    })
}

pub async fn execute_prepared(
    operation: PreparedOperation,
    fire_control: &FireControl,
    capabilities: &CapabilityRegistry,
) -> Result<NormalizedResult, String> {
    let artillery = operation.artillery.clone();

    let objective = operation.objective.clone();

    let implementation = match fire_control.resolve(&artillery, &objective) {
        Ok(target) => target.implementation().to_string(),

        Err(error) => {
            return normalized_failure(format!("could not route lifecycle operation: {error}"));
        }
    };

    match capabilities.invoke(&implementation, operation).await {
        Ok(payload) => Ok(NormalizedResult::succeeded(payload)),

        Err(error) => {
            let error = if error.trim().is_empty() {
                format!("lifecycle implementation '{implementation}' failed without an error")
            } else {
                error
            };

            normalized_failure(error)
        }
    }
}

pub async fn execute_prepared_from_available(
    operation: PreparedOperation,
    available: &AvailableCapabilityCatalog,
) -> Result<NormalizedResult, String> {
    let mut arsenal = LifecycleArsenal::new();

    if let Err(error) =
        arsenal.register_requested(available, &operation.artillery, &operation.objective)
    {
        return normalized_failure(format!("could not arm lifecycle operation: {error}"));
    }

    let (fire_control, capabilities) = arsenal.into_shared();

    execute_prepared(operation, fire_control.as_ref(), capabilities.as_ref()).await
}

pub async fn execute_prepared_from_available_controlled(
    operation: PreparedOperation,
    available: Arc<AvailableCapabilityCatalog>,
    control: ExecutionControl,
    token: CancellationToken,
) -> Result<NormalizedResult, String> {
    let controlled_operation = operation.clone();

    let controlled_available = Arc::clone(&available);

    let handler: ControlledHandler = Arc::new(move || {
        let operation = controlled_operation.clone();

        let available = Arc::clone(&controlled_available);

        Box::pin(async move {
            let result = execute_prepared_from_available(operation, available.as_ref()).await?;

            if result.is_success() {
                return Ok(result.payload().clone());
            }

            Err(result
                .error()
                .unwrap_or("lifecycle operation failed without an error")
                .to_string())
        })
    });

    let result_token = token.clone();

    match lifecycle_control::execute_controlled(handler, control, token).await {
        Ok(payload) => Ok(NormalizedResult::succeeded(payload)),

        Err(error) => {
            if result_token.is_cancelled() {
                return NormalizedResult::cancelled("execution cancelled");
            }

            normalized_failure(error)
        }
    }
}

pub async fn execute_ir_operation(
    ir: &BattleplanIr,
    operation_id: &str,
    battlefield: &Battlefield,
    fire_control: &FireControl,
    capabilities: &CapabilityRegistry,
) -> Result<NormalizedResult, String> {
    let operation = ir.prepare_operation(operation_id, battlefield)?;

    execute_prepared(operation, fire_control, capabilities).await
}

pub async fn execute_prepared_controlled(
    operation: PreparedOperation,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
    control: ExecutionControl,
    token: CancellationToken,
) -> Result<NormalizedResult, String> {
    let controlled_operation = operation.clone();

    let controlled_fire_control = Arc::clone(&fire_control);

    let controlled_capabilities = Arc::clone(&capabilities);

    let handler: ControlledHandler = Arc::new(move || {
        let operation = controlled_operation.clone();

        let fire_control = Arc::clone(&controlled_fire_control);

        let capabilities = Arc::clone(&controlled_capabilities);

        Box::pin(async move {
            let result =
                execute_prepared(operation, fire_control.as_ref(), capabilities.as_ref()).await?;

            if result.is_success() {
                return Ok(result.payload().clone());
            }

            Err(result
                .error()
                .unwrap_or("lifecycle operation failed without an error")
                .to_string())
        })
    });

    let result_token = token.clone();

    match lifecycle_control::execute_controlled(handler, control, token).await {
        Ok(payload) => Ok(NormalizedResult::succeeded(payload)),

        Err(error) => {
            if result_token.is_cancelled() {
                return NormalizedResult::cancelled("execution cancelled");
            }

            normalized_failure(error)
        }
    }
}

pub async fn execute_ir_operation_controlled(
    ir: &BattleplanIr,
    operation_id: &str,
    battlefield: &Battlefield,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
    control: ExecutionControl,
    token: CancellationToken,
) -> Result<NormalizedResult, String> {
    let operation = ir.prepare_operation(operation_id, battlefield)?;

    execute_prepared_controlled(operation, fire_control, capabilities, control, token).await
}

pub async fn execute_ir_operation_live(
    ir: &BattleplanIr,
    operation_id: &str,
    battlefield: &mut Battlefield,
    fire_control: &FireControl,
    capabilities: &CapabilityRegistry,
) -> Result<NormalizedResult, String> {
    let operation = ir.prepare_operation(operation_id, battlefield)?;

    let intelligence = operation.intelligence.clone();

    let result = execute_prepared(operation, fire_control, capabilities).await?;

    lifecycle_intelligence::propagate(operation_id, &intelligence, &result, battlefield)?;

    Ok(result)
}

pub async fn execute_ir_operation_live_controlled(
    ir: &BattleplanIr,
    operation_id: &str,
    battlefield: &mut Battlefield,
    fire_control: Arc<FireControl>,
    capabilities: Arc<CapabilityRegistry>,
    control: ExecutionControl,
    token: CancellationToken,
) -> Result<NormalizedResult, String> {
    let operation = ir.prepare_operation(operation_id, battlefield)?;

    let intelligence = operation.intelligence.clone();

    let result =
        execute_prepared_controlled(operation, fire_control, capabilities, control, token).await?;

    lifecycle_intelligence::propagate(operation_id, &intelligence, &result, battlefield)?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;
    use std::sync::Arc;

    use crate::lifecycle::{Battleplan, Operation};
    use crate::lifecycle_execution::ExecutionPayload;
    use crate::lifecycle_fire_control::FireControlTarget;
    use crate::lifecycle_ir;

    fn prepared(artillery: &str, objective: &str) -> PreparedOperation {
        PreparedOperation {
            module_id: None,
            artillery: artillery.to_string(),

            objective: objective.to_string(),

            munition: BTreeMap::new(),

            tactics: BTreeMap::new(),

            intelligence: BTreeMap::new(),
        }
    }

    fn working_route(artillery: &str, objective: &str, implementation: &str) -> FireControl {
        let mut fire_control = FireControl::new();

        fire_control
            .register(
                artillery,
                objective,
                FireControlTarget::new(implementation).unwrap(),
            )
            .unwrap();

        fire_control
    }

    fn successful_registry(implementation: &str) -> CapabilityRegistry {
        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                implementation,
                Arc::new(|operation| {
                    Box::pin(async move {
                        let mut output = ExecutionPayload::new();

                        output.insert("received_artillery".to_string(), operation.artillery);

                        output.insert("received_objective".to_string(), operation.objective);

                        Ok(output)
                    })
                }),
            )
            .unwrap();

        registry
    }

    #[test]
    fn prepared_operation_takes_only_requested_available_capability() {
        use crate::lifecycle_available_capabilities::{
            AvailableCapability, AvailableCapabilityCatalog,
        };

        let mut available = AvailableCapabilityCatalog::new();

        available
            .register(
                "future.tool",
                AvailableCapability::new(
                    "rust.future.tool",
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

        available
            .register(
                "unused.tool",
                AvailableCapability::new(
                    "rust.unused.tool",
                    Arc::new(|_| Box::pin(async { Err("unused capability executed".to_string()) })),
                )
                .unwrap(),
            )
            .unwrap();

        let result = futures_lite::future::block_on(execute_prepared_from_available(
            prepared("future.tool", "module.decides.objective"),
            &available,
        ))
        .unwrap();

        assert!(result.is_success());

        assert_eq!(
            result.payload().get("objective"),
            Some(&"module.decides.objective".to_string(),)
        );
    }

    #[test]
    fn unavailable_requested_artillery_becomes_normalized_failure() {
        let available = AvailableCapabilityCatalog::new();

        let result = futures_lite::future::block_on(execute_prepared_from_available(
            prepared("missing.tool", "whatever.objective"),
            &available,
        ))
        .unwrap();

        assert!(result.is_failure());

        assert!(result
            .error()
            .unwrap()
            .contains("no available Rust capability",));
    }

    #[test]
    fn same_available_adapter_accepts_arbitrary_runtime_objective() {
        use crate::lifecycle_available_capabilities::{
            AvailableCapability, AvailableCapabilityCatalog,
        };

        let mut available = AvailableCapabilityCatalog::new();

        available
            .register(
                "dynamic.tool",
                AvailableCapability::new(
                    "rust.dynamic.tool",
                    Arc::new(|operation| {
                        Box::pin(async move {
                            Ok(ExecutionPayload::from([(
                                "received".to_string(),
                                operation.objective,
                            )]))
                        })
                    }),
                )
                .unwrap(),
            )
            .unwrap();

        for objective in ["anything.one", "future.action.two", "module.invented.this"] {
            let result = futures_lite::future::block_on(execute_prepared_from_available(
                prepared("dynamic.tool", objective),
                &available,
            ))
            .unwrap();

            assert!(result.is_success());

            assert_eq!(
                result.payload().get("received"),
                Some(&objective.to_string(),)
            );
        }
    }

    #[test]
    fn prepared_operation_executes_through_fire_control_and_registry() {
        let fire_control = working_route(
            "future.artillery",
            "future.objective",
            "future.implementation",
        );

        let registry = successful_registry("future.implementation");

        let result = futures_lite::future::block_on(execute_prepared(
            prepared("future.artillery", "future.objective"),
            &fire_control,
            &registry,
        ))
        .unwrap();

        assert!(result.is_success());

        assert_eq!(
            result.payload().get("received_artillery"),
            Some(&"future.artillery".to_string())
        );

        assert_eq!(
            result.payload().get("received_objective"),
            Some(&"future.objective".to_string())
        );
    }

    #[test]
    fn executor_uses_fire_control_target_not_artillery_as_implementation() {
        let fire_control = working_route(
            "module.artillery",
            "module.objective",
            "completely.different.implementation",
        );

        let registry = successful_registry("completely.different.implementation");

        let result = futures_lite::future::block_on(execute_prepared(
            prepared("module.artillery", "module.objective"),
            &fire_control,
            &registry,
        ))
        .unwrap();

        assert!(result.is_success());
    }

    #[test]
    fn missing_fire_control_route_becomes_normalized_failure() {
        let result = futures_lite::future::block_on(execute_prepared(
            prepared("missing.artillery", "missing.objective"),
            &FireControl::new(),
            &CapabilityRegistry::new(),
        ))
        .unwrap();

        assert!(result.is_failure());

        assert!(result
            .error()
            .unwrap()
            .contains("could not route lifecycle operation"));
    }

    #[test]
    fn missing_capability_implementation_becomes_normalized_failure() {
        let fire_control = working_route("artillery", "objective", "missing.implementation");

        let result = futures_lite::future::block_on(execute_prepared(
            prepared("artillery", "objective"),
            &fire_control,
            &CapabilityRegistry::new(),
        ))
        .unwrap();

        assert!(result.is_failure());

        assert!(result.error().unwrap().contains("is not registered"));
    }

    #[test]
    fn capability_failure_becomes_normalized_failure() {
        let fire_control = working_route("artillery", "objective", "failure.implementation");

        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "failure.implementation",
                Arc::new(|_| {
                    Box::pin(async { Err("synthetic implementation failure".to_string()) })
                }),
            )
            .unwrap();

        let result = futures_lite::future::block_on(execute_prepared(
            prepared("artillery", "objective"),
            &fire_control,
            &registry,
        ))
        .unwrap();

        assert!(result.is_failure());

        assert_eq!(result.error(), Some("synthetic implementation failure"));
    }

    #[test]
    fn empty_capability_error_is_replaced_by_nonempty_technical_failure() {
        let fire_control = working_route("artillery", "objective", "empty.failure");

        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "empty.failure",
                Arc::new(|_| Box::pin(async { Err(String::new()) })),
            )
            .unwrap();

        let result = futures_lite::future::block_on(execute_prepared(
            prepared("artillery", "objective"),
            &fire_control,
            &registry,
        ))
        .unwrap();

        assert!(result.is_failure());

        assert!(result.error().unwrap().contains("failed without an error"));
    }

    #[test]
    fn arbitrary_payload_is_preserved_by_normalized_result() {
        let fire_control = working_route("artillery", "objective", "payload.implementation");

        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "payload.implementation",
                Arc::new(|_| {
                    Box::pin(async {
                        Ok(BTreeMap::from([
                            ("future.output.one".to_string(), "alpha".to_string()),
                            ("anything.module.owns".to_string(), "beta".to_string()),
                        ]))
                    })
                }),
            )
            .unwrap();

        let result = futures_lite::future::block_on(execute_prepared(
            prepared("artillery", "objective"),
            &fire_control,
            &registry,
        ))
        .unwrap();

        assert_eq!(
            result.payload().get("future.output.one"),
            Some(&"alpha".to_string())
        );

        assert_eq!(
            result.payload().get("anything.module.owns"),
            Some(&"beta".to_string())
        );
    }

    #[test]
    fn complete_prepared_operation_reaches_capability() {
        let fire_control = working_route("artillery", "objective", "inspect.operation");

        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "inspect.operation",
                Arc::new(|operation| {
                    Box::pin(async move {
                        let mut output = ExecutionPayload::new();

                        output.insert(
                            "munition".to_string(),
                            operation.munition.get("input").cloned().unwrap(),
                        );

                        output.insert(
                            "tactics".to_string(),
                            operation.tactics.get("technical").cloned().unwrap(),
                        );

                        output.insert(
                            "intelligence".to_string(),
                            operation.intelligence.get("selection").cloned().unwrap(),
                        );

                        Ok(output)
                    })
                }),
            )
            .unwrap();

        let operation = PreparedOperation {
            module_id: None,
            artillery: "artillery".to_string(),

            objective: "objective".to_string(),

            munition: BTreeMap::from([("input".to_string(), "munition-value".to_string())]),

            tactics: BTreeMap::from([("technical".to_string(), "tactics-value".to_string())]),

            intelligence: BTreeMap::from([(
                "selection".to_string(),
                "intelligence-value".to_string(),
            )]),
        };

        let result =
            futures_lite::future::block_on(execute_prepared(operation, &fire_control, &registry))
                .unwrap();

        assert_eq!(
            result.payload().get("munition"),
            Some(&"munition-value".to_string())
        );

        assert_eq!(
            result.payload().get("tactics"),
            Some(&"tactics-value".to_string())
        );

        assert_eq!(
            result.payload().get("intelligence"),
            Some(&"intelligence-value".to_string())
        );
    }

    #[test]
    fn ir_operation_can_execute_without_assigning_battleplan_order() {
        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "chosen-operation".to_string(),
                Operation {
                    artillery: "artillery".to_string(),

                    objective: "objective".to_string(),

                    munition: BTreeMap::new(),

                    tactics: BTreeMap::new(),

                    intelligence: BTreeMap::new(),
                },
            )]),
        };

        let ir = lifecycle_ir::compile_battleplan(
            &battleplan,
            &crate::lifecycle_battlefield::Battlefield::new(),
        )
        .unwrap();

        let fire_control = working_route("artillery", "objective", "implementation");

        let registry = successful_registry("implementation");

        let result = futures_lite::future::block_on(execute_ir_operation(
            &ir,
            "chosen-operation",
            &crate::lifecycle_battlefield::Battlefield::new(),
            &fire_control,
            &registry,
        ))
        .unwrap();

        assert!(result.is_success());
    }

    #[test]
    fn unknown_ir_operation_is_rejected_before_execution() {
        let ir = lifecycle_ir::compile_battleplan(
            &Battleplan::default(),
            &crate::lifecycle_battlefield::Battlefield::new(),
        )
        .unwrap();

        let error = futures_lite::future::block_on(execute_ir_operation(
            &ir,
            "missing-operation",
            &crate::lifecycle_battlefield::Battlefield::new(),
            &FireControl::new(),
            &CapabilityRegistry::new(),
        ))
        .unwrap_err();

        assert!(error.contains("does not exist"));
    }

    #[test]
    fn empty_ir_operation_identifier_is_rejected() {
        let ir = lifecycle_ir::compile_battleplan(
            &Battleplan::default(),
            &crate::lifecycle_battlefield::Battlefield::new(),
        )
        .unwrap();

        let error = futures_lite::future::block_on(execute_ir_operation(
            &ir,
            "   ",
            &crate::lifecycle_battlefield::Battlefield::new(),
            &FireControl::new(),
            &CapabilityRegistry::new(),
        ))
        .unwrap_err();

        assert!(error.contains("cannot be empty"));
    }

    #[test]
    fn executor_does_not_require_specific_artillery_or_objective_vocabulary() {
        let artillery = "future.module.anything";

        let objective = "unknown.technical.action";

        let implementation = "opaque.rust.handler";

        let fire_control = working_route(artillery, objective, implementation);

        let registry = successful_registry(implementation);

        let result = futures_lite::future::block_on(execute_prepared(
            prepared(artillery, objective),
            &fire_control,
            &registry,
        ))
        .unwrap();

        assert!(result.is_success());
    }

    #[test]
    fn controlled_executor_runs_successful_capability() {
        let fire_control = Arc::new(working_route(
            "artillery",
            "objective",
            "controlled.success",
        ));

        let capabilities = Arc::new(successful_registry("controlled.success"));

        let result = futures_lite::future::block_on(execute_prepared_controlled(
            prepared("artillery", "objective"),
            fire_control,
            capabilities,
            ExecutionControl::new(),
            CancellationToken::new(),
        ))
        .unwrap();

        assert!(result.is_success());
    }

    #[test]
    fn controlled_executor_retries_real_capability_path() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let fire_control = Arc::new(working_route("artillery", "objective", "controlled.retry"));

        let attempts = Arc::new(AtomicUsize::new(0));

        let counter = Arc::clone(&attempts);

        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "controlled.retry",
                Arc::new(move |_| {
                    let counter = Arc::clone(&counter);

                    Box::pin(async move {
                        let current = counter.fetch_add(1, Ordering::SeqCst);

                        if current < 1 {
                            return Err("first attempt fails".to_string());
                        }

                        Ok(ExecutionPayload::new())
                    })
                }),
            )
            .unwrap();

        let control = ExecutionControl::new().attempts(2).unwrap();

        let result = futures_lite::future::block_on(execute_prepared_controlled(
            prepared("artillery", "objective"),
            fire_control,
            Arc::new(registry),
            control,
            CancellationToken::new(),
        ))
        .unwrap();

        assert!(result.is_success());

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn controlled_executor_stops_at_attempt_limit() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let fire_control = Arc::new(working_route(
            "artillery",
            "objective",
            "controlled.failure",
        ));

        let attempts = Arc::new(AtomicUsize::new(0));

        let counter = Arc::clone(&attempts);

        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "controlled.failure",
                Arc::new(move |_| {
                    let counter = Arc::clone(&counter);

                    Box::pin(async move {
                        counter.fetch_add(1, Ordering::SeqCst);

                        Err("persistent failure".to_string())
                    })
                }),
            )
            .unwrap();

        let control = ExecutionControl::new().attempts(2).unwrap();

        let result = futures_lite::future::block_on(execute_prepared_controlled(
            prepared("artillery", "objective"),
            fire_control,
            Arc::new(registry),
            control,
            CancellationToken::new(),
        ))
        .unwrap();

        assert!(result.is_failure());

        assert_eq!(attempts.load(Ordering::SeqCst), 2);

        assert_eq!(result.error(), Some("persistent failure"));
    }

    #[test]
    fn pre_cancelled_executor_returns_explicit_cancelled_status() {
        let fire_control = Arc::new(working_route("artillery", "objective", "controlled.cancel"));

        let capabilities = Arc::new(successful_registry("controlled.cancel"));

        let token = CancellationToken::new();

        token.cancel();

        let result = futures_lite::future::block_on(execute_prepared_controlled(
            prepared("artillery", "objective"),
            fire_control,
            capabilities,
            ExecutionControl::new(),
            token,
        ))
        .unwrap();

        assert!(result.is_cancelled());

        assert!(!result.is_failure());
    }

    #[test]
    fn cancellation_text_from_capability_is_not_inferred_as_cancelled() {
        let fire_control = Arc::new(working_route("artillery", "objective", "controlled.text"));

        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "controlled.text",
                Arc::new(|_| Box::pin(async { Err("execution cancelled".to_string()) })),
            )
            .unwrap();

        let result = futures_lite::future::block_on(execute_prepared_controlled(
            prepared("artillery", "objective"),
            fire_control,
            Arc::new(registry),
            ExecutionControl::new(),
            CancellationToken::new(),
        ))
        .unwrap();

        assert!(result.is_failure());

        assert!(!result.is_cancelled());

        assert_eq!(result.error(), Some("execution cancelled"));
    }

    #[test]
    fn controlled_ir_entrypoint_uses_same_executor_path() {
        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "controlled-operation".to_string(),
                Operation {
                    artillery: "artillery".to_string(),

                    objective: "objective".to_string(),

                    munition: BTreeMap::new(),

                    tactics: BTreeMap::new(),

                    intelligence: BTreeMap::new(),
                },
            )]),
        };

        let ir = lifecycle_ir::compile_battleplan(
            &battleplan,
            &crate::lifecycle_battlefield::Battlefield::new(),
        )
        .unwrap();

        let result = futures_lite::future::block_on(execute_ir_operation_controlled(
            &ir,
            "controlled-operation",
            &crate::lifecycle_battlefield::Battlefield::new(),
            Arc::new(working_route("artillery", "objective", "controlled.ir")),
            Arc::new(successful_registry("controlled.ir")),
            ExecutionControl::new(),
            CancellationToken::new(),
        ))
        .unwrap();

        assert!(result.is_success());
    }

    #[test]
    fn live_executor_persists_selected_intelligence_and_clears_result() {
        let marker = char::from_u32(36).unwrap();

        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "first".to_string(),
                Operation {
                    artillery: "artillery".to_string(),

                    objective: "objective".to_string(),

                    munition: BTreeMap::new(),

                    tactics: BTreeMap::new(),

                    intelligence: BTreeMap::from([(
                        "output".to_string(),
                        format!("{marker}result.value"),
                    )]),
                },
            )]),
        };

        let ir = lifecycle_ir::compile_battleplan(&battleplan, &Battlefield::new()).unwrap();

        let fire_control = working_route("artillery", "objective", "live.first");

        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "live.first",
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

        let mut battlefield = Battlefield::new();

        let result = futures_lite::future::block_on(execute_ir_operation_live(
            &ir,
            "first",
            &mut battlefield,
            &fire_control,
            &registry,
        ))
        .unwrap();

        assert!(result.is_success());

        assert_eq!(
            battlefield.get("intelligence", "first.output",),
            Some("CHAINED")
        );

        assert!(!battlefield.contains_namespace("result"));
    }

    #[test]
    fn second_operation_can_consume_first_operation_intelligence() {
        let marker = char::from_u32(36).unwrap();

        let battleplan = Battleplan {
            operations: BTreeMap::from([
                (
                    "first".to_string(),
                    Operation {
                        artillery: "producer".to_string(),

                        objective: "create".to_string(),

                        munition: BTreeMap::new(),

                        tactics: BTreeMap::new(),

                        intelligence: BTreeMap::from([(
                            "output".to_string(),
                            format!("{marker}result.value"),
                        )]),
                    },
                ),
                (
                    "second".to_string(),
                    Operation {
                        artillery: "consumer".to_string(),

                        objective: "consume".to_string(),

                        munition: BTreeMap::from([(
                            "input".to_string(),
                            format!("{marker}intelligence.first.output"),
                        )]),

                        tactics: BTreeMap::new(),

                        intelligence: BTreeMap::new(),
                    },
                ),
            ]),
        };

        let ir = lifecycle_ir::compile_battleplan(&battleplan, &Battlefield::new()).unwrap();

        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "producer",
                "create",
                FireControlTarget::new("chain.producer").unwrap(),
            )
            .unwrap();

        fire_control
            .register(
                "consumer",
                "consume",
                FireControlTarget::new("chain.consumer").unwrap(),
            )
            .unwrap();

        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "chain.producer",
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

        registry
            .register(
                "chain.consumer",
                Arc::new(|operation| {
                    Box::pin(async move {
                        let input = operation
                            .munition
                            .get("input")
                            .cloned()
                            .ok_or_else(|| "consumer input missing".to_string())?;

                        Ok(BTreeMap::from([("received".to_string(), input)]))
                    })
                }),
            )
            .unwrap();

        let mut battlefield = Battlefield::new();

        let first = futures_lite::future::block_on(execute_ir_operation_live(
            &ir,
            "first",
            &mut battlefield,
            &fire_control,
            &registry,
        ))
        .unwrap();

        assert!(first.is_success());

        let second = futures_lite::future::block_on(execute_ir_operation_live(
            &ir,
            "second",
            &mut battlefield,
            &fire_control,
            &registry,
        ))
        .unwrap();

        assert!(second.is_success());

        assert_eq!(
            second.payload().get("received"),
            Some(&"CHAINED".to_string())
        );

        assert_eq!(
            battlefield.get("intelligence", "first.output",),
            Some("CHAINED")
        );

        assert!(!battlefield.contains_namespace("result"));
    }

    #[test]
    fn failed_live_operation_does_not_publish_intelligence() {
        let marker = char::from_u32(36).unwrap();

        let battleplan = Battleplan {
            operations: BTreeMap::from([(
                "failure".to_string(),
                Operation {
                    artillery: "artillery".to_string(),

                    objective: "objective".to_string(),

                    munition: BTreeMap::new(),

                    tactics: BTreeMap::new(),

                    intelligence: BTreeMap::from([(
                        "output".to_string(),
                        format!("{marker}result.value"),
                    )]),
                },
            )]),
        };

        let ir = lifecycle_ir::compile_battleplan(&battleplan, &Battlefield::new()).unwrap();

        let fire_control = working_route("artillery", "objective", "live.failure");

        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "live.failure",
                Arc::new(|_| Box::pin(async { Err("synthetic failure".to_string()) })),
            )
            .unwrap();

        let mut battlefield = Battlefield::new();

        battlefield.insert("result", "stale", "old");

        let result = futures_lite::future::block_on(execute_ir_operation_live(
            &ir,
            "failure",
            &mut battlefield,
            &fire_control,
            &registry,
        ))
        .unwrap();

        assert!(result.is_failure());

        assert!(!battlefield.contains_namespace("result"));

        assert!(!battlefield.contains("intelligence", "failure.output",));
    }
}

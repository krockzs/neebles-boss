/*
 * N.E.E.B.L.E.S. TXN V5 -> Lifecycle Arsenal bridge.
 *
 * This is a PRIVATE, transaction-scoped Arsenal. A module cannot call its
 * destructive artillery from an ordinary Lifecycle transition: the handler
 * is deliberately absent from the global productive capability catalog.
 *
 * TXN V5 remains the coordinator of evidence, reversible staging, rollback
 * and post-commit retirement. Only the implementation of a certified
 * resource operation is dispatched through FireControl / CapabilityRegistry.
 */
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::lifecycle::{Battleplan, LifecycleContract, Operation};
use crate::lifecycle_arsenal::LifecycleArsenal;
use crate::lifecycle_capabilities::{CapabilityFuture, CapabilityHandler};
use crate::lifecycle_execution::ExecutionPayload;
use crate::lifecycle_governor_bridge;
use crate::lifecycle_operation::PreparedOperation;
use crate::module_uninstall::{self, ServiceStop};

const ARTILLERY: &str = "boss.uninstall.systemd-user";
const QUIESCE: &str = "service.quiesce";
const RETIRE: &str = "service.retire";
const REACTIVATE: &str = "service.reactivate";

/*
 * The bridge never uses the regular global catalog. Only the transaction
 * coordinator constructs this handler and grants it exactly one operation.
 * New resource adapters can reuse this function, but each must supply its
 * own bounded handler and transactional evidence/compensation strategy.
 */
fn execute_private_operation(
    module: &str,
    artillery: &str,
    objective: &str,
    munition: BTreeMap<String, String>,
    implementation_id: &str,
    handler: CapabilityHandler,
) -> Result<(), String> {
    if module.trim().is_empty() || artillery.trim().is_empty() || objective.trim().is_empty() {
        return Err("TXN V5 Arsenal requires nonempty module/artillery/objective".into());
    }

    let mut arsenal = LifecycleArsenal::new();
    arsenal.register(artillery, objective, implementation_id, handler)?;

    // Private, in-memory Lifecycle contract, *not* supplied by a module.
    let mut contract = LifecycleContract::default();
    contract.transitions.insert(
        "uninstall-step".into(),
        Battleplan {
            operations: BTreeMap::from([(
                "resource".into(),
                Operation {
                    artillery: artillery.into(),
                    objective: objective.into(),
                    munition,
                    tactics: BTreeMap::new(),
                    intelligence: BTreeMap::new(),
                },
            )]),
        },
    );
    // Use the existing Governor bridge, FireControl and Rust CapabilityRegistry.
    // Do not use require_success(), which would discard the handler's precise
    // error and leave TXN V5 without actionable rollback diagnostics.
    let (fire_control, capabilities) = arsenal.into_shared();
    let execution = lifecycle_governor_bridge::execute_transition_blocking(
        module,
        "txnv5.resource-step",
        &contract,
        "uninstall-step",
        BTreeMap::new(),
        fire_control,
        capabilities,
    )?;
    let report = execution.report();
    if report.terminal_phase() != "succeeded" {
        let detail = report
            .dag()
            .result("resource")
            .and_then(|result| result.error())
            .unwrap_or("resource operation failed without an error payload");
        return Err(format!(
            "TXN V5 Arsenal {artillery}/{objective} failed: {detail}"
        ));
    }

    Ok(())
}

struct ServiceRequest {
    module: String,
    unit: String,
    exec_start: String,
    required: bool,
    stop: ServiceStop,
    retire: bool,
    reactivate: bool,
}

fn parse_bool(field: &str, raw: &str) -> Result<bool, String> {
    match raw {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("TXN V5 invalid boolean in {field}")),
    }
}

fn parse_systemd_user_operation(operation: &PreparedOperation) -> Result<ServiceRequest, String> {
    if operation.artillery != ARTILLERY {
        return Err("TXN V5 service handler received foreign artillery".into());
    }
    let (retire, reactivate) = match operation.objective.as_str() {
        QUIESCE => (false, false),
        RETIRE => (true, false),
        REACTIVATE => (false, true),
        _ => return Err("TXN V5 service handler received unsupported objective".into()),
    };
    if !operation.tactics.is_empty() || !operation.intelligence.is_empty() {
        return Err("TXN V5 service handler forbids ungoverned tactics/intelligence".into());
    }
    const FIELDS: [&str; 5] = ["unit", "exec_start", "required", "disable", "timeout_ms"];
    if operation.munition.len() != FIELDS.len()
        || operation
            .munition
            .keys()
            .any(|key| !FIELDS.contains(&key.as_str()))
    {
        return Err("TXN V5 service handler received unexpected/missing munition".into());
    }
    let get = |name: &str| -> Result<&str, String> {
        operation
            .munition
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("TXN V5 missing munition.{name}"))
    };
    let module = operation
        .module_id
        .as_ref()
        .ok_or_else(|| "TXN V5 service handler requires Governor module identity".to_string())?
        .clone();
    if module.trim().is_empty() {
        return Err("TXN V5 module identity is empty".into());
    }
    let timeout_ms = get("timeout_ms")?
        .parse::<u64>()
        .map_err(|_| "TXN V5 invalid service timeout_ms".to_string())?;
    if !(1..=60_000).contains(&timeout_ms) {
        return Err("TXN V5 service timeout_ms outside 1..=60000".into());
    }
    let disable = parse_bool("disable", get("disable")?)?;
    if retire && !disable {
        return Err("TXN V5 cannot retire a service without disable=true".into());
    }
    Ok(ServiceRequest {
        module,
        unit: get("unit")?.into(),
        exec_start: get("exec_start")?.into(),
        required: parse_bool("required", get("required")?)?,
        stop: ServiceStop {
            disable,
            timeout_ms,
        },
        retire,
        reactivate,
    })
}

fn systemd_user_handler(operation: PreparedOperation) -> CapabilityFuture {
    Box::pin(async move {
        let request = parse_systemd_user_operation(&operation)?;
        // The existing TXN V5 adapter performs the actual authority checks:
        // exact module namespace, ExecStart, UID/GID, unit owner/mode, manager
        // fragment and hooks. Arsenal does not bypass or reimplement them.
        if request.reactivate {
            module_uninstall::reactivate_certified_service(
                &request.module,
                &request.unit,
                &request.exec_start,
                request.stop.timeout_ms,
            )?;
        } else {
            module_uninstall::stop_service(
                &request.module,
                &request.unit,
                &request.exec_start,
                request.required,
                &request.stop,
                request.retire,
            )?;
        }
        Ok(ExecutionPayload::from([
            ("unit".into(), request.unit),
            (
                "phase".into(),
                if request.reactivate {
                    REACTIVATE
                } else if request.retire {
                    RETIRE
                } else {
                    QUIESCE
                }
                .into(),
            ),
        ]))
    })
}

pub(crate) fn systemd_user_service(
    module: &str,
    unit: &str,
    exec_start: &str,
    required: bool,
    stop: &ServiceStop,
    retire: bool,
) -> Result<(), String> {
    let objective = if retire { RETIRE } else { QUIESCE };
    let munition = BTreeMap::from([
        ("unit".into(), unit.into()),
        ("exec_start".into(), exec_start.into()),
        ("required".into(), required.to_string()),
        ("disable".into(), stop.disable.to_string()),
        ("timeout_ms".into(), stop.timeout_ms.to_string()),
    ]);
    execute_private_operation(
        module,
        ARTILLERY,
        objective,
        munition,
        "rust.boss.uninstall.systemd-user",
        Arc::new(systemd_user_handler),
    )
}

// TXN V5 can only call this on a captured, previously active unit and after
// verifying the complete original physical installation was restored.
// The original recovery receipt and ordering stay in UninstallServiceRecovery.
pub(crate) fn systemd_user_restore(
    module: &str,
    unit: &str,
    exec_start: &str,
    timeout_ms: u64,
) -> Result<(), String> {
    let munition = BTreeMap::from([
        ("unit".into(), unit.into()),
        ("exec_start".into(), exec_start.into()),
        ("required".into(), "true".into()),
        ("disable".into(), "true".into()),
        ("timeout_ms".into(), timeout_ms.to_string()),
    ]);
    execute_private_operation(
        module,
        ARTILLERY,
        REACTIVATE,
        munition,
        "rust.boss.uninstall.systemd-user.reactivate",
        Arc::new(systemd_user_handler),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PreparedOperation {
        PreparedOperation {
            module_id: Some("test-module".into()),
            execution_id: Some("txnv5.synthetic".into()),
            artillery: ARTILLERY.into(),
            objective: QUIESCE.into(),
            munition: BTreeMap::from([
                ("unit".into(), "neebles-test-module-session-controller.service".into()),
                (
                    "exec_start".into(),
                    "/usr/bin/python3.13 /opt/neebles/modules/test-module/tray/session-controller.py"
                        .into(),
                ),
                ("required".into(), "false".into()),
                ("disable".into(), "true".into()),
                ("timeout_ms".into(), "5000".into()),
            ]),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        }
    }

    #[test]
    fn uninstall_systemd_service_munition_is_strictly_typed() {
        let request = parse_systemd_user_operation(&fixture()).unwrap();
        assert_eq!(request.module, "test-module");
        assert!(request.stop.disable);
        assert_eq!(request.stop.timeout_ms, 5000);
        assert!(!request.retire);
    }

    #[test]
    fn service_retirement_requires_disable() {
        let mut operation = fixture();
        operation.objective = RETIRE.into();
        operation.munition.insert("disable".into(), "false".into());
        assert!(parse_systemd_user_operation(&operation).is_err());
    }

    #[test]
    fn extra_munitions_and_unknown_objectives_fail_closed() {
        let mut operation = fixture();
        operation.munition.insert("shell".into(), "/bin/sh".into());
        assert!(parse_systemd_user_operation(&operation).is_err());
        operation = fixture();
        operation.objective = "service.exec".into();
        assert!(parse_systemd_user_operation(&operation).is_err());
    }

    #[test]
    fn missing_governor_identity_fails_closed() {
        let mut operation = fixture();
        operation.module_id = None;
        assert!(parse_systemd_user_operation(&operation).is_err());
    }

    #[test]
    fn unknown_service_timeout_and_bool_fail_closed() {
        let mut operation = fixture();
        operation
            .munition
            .insert("timeout_ms".into(), "60001".into());
        assert!(parse_systemd_user_operation(&operation).is_err());
        operation = fixture();
        operation.munition.insert("disable".into(), "yes".into());
        assert!(parse_systemd_user_operation(&operation).is_err());
    }

    #[test]
    fn private_arsenal_uses_governor_and_rust_handler() {
        let handler: CapabilityHandler = Arc::new(|operation| {
            Box::pin(async move {
                if operation.module_id.as_deref() != Some("test-module")
                    || operation.artillery != "test.private"
                    || operation.objective != "inspect"
                    || operation.munition.get("key").map(String::as_str) != Some("value")
                {
                    return Err("TXN V5 private Arsenal identity/munition mismatch".into());
                }
                Ok(ExecutionPayload::new())
            })
        });
        execute_private_operation(
            "test-module",
            "test.private",
            "inspect",
            BTreeMap::from([("key".into(), "value".into())]),
            "rust.boss.uninstall.private-test",
            handler,
        )
        .unwrap();
    }

    #[test]
    fn private_arsenal_propagates_handler_failure() {
        let handler: CapabilityHandler =
            Arc::new(|_| Box::pin(async { Err("synthetic handler failure".into()) }));
        assert!(execute_private_operation(
            "test-module",
            "test.private",
            "fail",
            BTreeMap::new(),
            "rust.boss.uninstall.private-test-failure",
            handler,
        )
        .is_err());
    }
}

use crate::contracts::{Lifecycle, StateMode};

use crate::module_ipc::protocol::{ModuleMessage, ModuleRuntimeState};

use crate::module_ipc::server::{pending_registry, runtime_registry};

use serde_json::Value;

use std::collections::BTreeMap;

use std::sync::atomic::{AtomicU64, Ordering};

use std::sync::mpsc::RecvTimeoutError;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_request_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();

    let counter = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);

    format!("req-{}-{}-{}", std::process::id(), timestamp, counter)
}

#[allow(clippy::too_many_arguments)]
pub fn invoke(
    module: &str,
    contract: &str,
    endpoint: &str,
    lifecycle: Lifecycle,
    state_mode: StateMode,
    args: Vec<String>,
    payload: Option<Value>,
    context: BTreeMap<String, Value>,
    timeout: Option<Duration>,
) -> Result<ModuleMessage, String> {
    let runtime = runtime_registry()
        .get(module)?
        .ok_or_else(|| format!("module '{}' has no registered runtime", module))?;

    if runtime.state != ModuleRuntimeState::Ready {
        return Err(format!(
            "module '{}' runtime session '{}' is not ready",
            module, runtime.session_id
        ));
    }

    let id = next_request_id();

    let receiver = pending_registry().register(&id, module, &runtime.session_id)?;

    /*
     * Close the race between taking the initial runtime
     * snapshot and registering this pending request.
     *
     * If the runtime disappeared after get() but before
     * register(), server cleanup may already have executed
     * fail_session(). Re-check the exact session now:
     *
     * - if it is still the same Ready session, sending is safe;
     * - if it disappeared or was replaced, cancel immediately.
     *
     * If the runtime dies after this check, server-side
     * fail_session() will see this already-registered request.
     */
    let current_runtime = runtime_registry().get(module)?;

    let session_still_ready = matches!(
        current_runtime,
        Some(ref current)
            if current.session_id == runtime.session_id
                && current.state == ModuleRuntimeState::Ready
    );

    if !session_still_ready {
        let _ = pending_registry().cancel(&id);

        return Err(format!(
            "module '{}' runtime session '{}' disconnected before request '{}' could be sent",
            module, runtime.session_id, id
        ));
    }

    let message = ModuleMessage::Invoke {
        id: id.clone(),

        module: module.to_string(),

        session_id: runtime.session_id.clone(),

        contract: contract.to_string(),

        endpoint: endpoint.to_string(),

        lifecycle,
        state_mode,

        args,
        payload,
        context,
    };

    if let Err(error) = runtime.writer.send(message) {
        let _ = pending_registry().cancel(&id);

        return Err(format!(
            "could not send request '{}' to module '{}': {error}",
            id, module
        ));
    }

    match timeout {
        Some(timeout) => match receiver.recv_timeout(timeout) {
            Ok(message) => Ok(message),

            Err(RecvTimeoutError::Timeout) => {
                let _ = pending_registry().cancel(&id);

                Err(format!("module '{}' request '{}' timed out", module, id))
            }

            Err(RecvTimeoutError::Disconnected) => {
                let _ = pending_registry().cancel(&id);

                Err(format!(
                    "module '{}' request '{}' response channel disconnected",
                    module, id
                ))
            }
        },

        None => match receiver.recv() {
            Ok(message) => Ok(message),

            Err(_) => {
                let _ = pending_registry().cancel(&id);

                Err(format!(
                    "module '{}' request '{}' response channel disconnected",
                    module, id
                ))
            }
        },
    }
}

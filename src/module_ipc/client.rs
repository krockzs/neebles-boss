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
    timeout: Duration,
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

    let receiver = pending_registry().register(&id)?;

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

    match receiver.recv_timeout(timeout) {
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
    }
}

use crate::module_ipc::client;
use crate::module_ipc::protocol::ModuleMessage;
use crate::module_ipc::server::runtime_registry;

use crate::{modules, privileges};

use serde_json::Value;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/*
 * Invoke one endpoint through its installed declarative contract.
 *
 * Boss understands only generic contract mechanics:
 *
 * module
 * contract type
 * logical endpoint name
 * lifecycle
 * state mode
 * privilege requirement
 *
 * Boss deliberately does NOT understand the semantic meaning of
 * contract names such as "commands", "connect", "llm", etc.
 */
#[allow(clippy::too_many_arguments)]
pub fn invoke_declared(
    module: &str,
    contract_type: &str,
    endpoint_name: &str,
    args: Vec<String>,
    payload: Option<Value>,
    context: BTreeMap<String, Value>,
    timeout: Option<Duration>,
) -> Result<ModuleMessage, String> {
    invoke_declared_with_retry(
        module,
        contract_type,
        endpoint_name,
        args,
        payload,
        context,
        timeout,
        crate::ipc_middleware::RetryPolicy::legacy(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn invoke_declared_with_retry(
    module: &str,
    contract_type: &str,
    endpoint_name: &str,
    args: Vec<String>,
    payload: Option<Value>,
    context: BTreeMap<String, Value>,
    timeout: Option<Duration>,
    retries: crate::ipc_middleware::RetryPolicy,
) -> Result<ModuleMessage, String> {
    if module.trim().is_empty() {
        return Err("module name cannot be empty".to_string());
    }

    if contract_type.trim().is_empty() {
        return Err("contract type cannot be empty".to_string());
    }

    if endpoint_name.trim().is_empty() {
        return Err("contract endpoint name cannot be empty".to_string());
    }

    /*
     * Resolve the logical endpoint from installed declarative
     * state. Installed contract JSON is Boss's authority.
     */
    let endpoint =
        modules::installed_module_contract_endpoint(module, contract_type, endpoint_name)?
            .ok_or_else(|| {
                format!(
                    "module '{}' does not declare endpoint '{}' in contract '{}'",
                    module, endpoint_name, contract_type
                )
            })?;

    /*
     * Privilege policy belongs to the declaration, not to the
     * runtime advertisement.
     */
    privileges::ensure_root(endpoint.requires_root)?;

    /*
     * Runtime advertisement is availability, not authority.
     *
     * Registration governance already guarantees runtimes cannot
     * advertise undeclared capabilities. Here we additionally
     * require that the currently connected runtime actually
     * advertised this declared endpoint.
     */
    let wait_started = Instant::now();
    let runtime = retries
        .wait_for_available(timeout, || runtime_registry().get(module))?
        .ok_or_else(|| {
            format!(
                "module '{}' declares '{}:{}' but has no registered runtime",
                module, contract_type, endpoint_name
            )
        })?;

    let advertised = runtime
        .endpoints
        .get(contract_type)
        .map(|endpoints| endpoints.iter().any(|value| value == &endpoint.endpoint))
        .unwrap_or(false);

    if !advertised {
        return Err(format!(
            "module '{}' runtime session '{}' does not advertise declared endpoint '{}' from contract '{}' (logical name '{}')",
            module,
            runtime.session_id,
            endpoint.endpoint,
            contract_type,
            endpoint_name
        ));
    }

    /*
     * Lifecycle and state semantics come exclusively from the
     * installed contract declaration.
     *
     * Callers cannot override them.
     */
    // Retry delays count against the total transport deadline. Once a provider
    // is registered, invoke exactly once. Never replay a delivered write.
    let remaining = match timeout {
        Some(limit) => Some(
            limit
                .checked_sub(wait_started.elapsed())
                .filter(|time| !time.is_zero())
                .ok_or_else(|| {
                    "IPC pre-dispatch deadline exceeded; request was not sent".to_string()
                })?,
        ),
        None => None,
    };

    client::invoke(
        module,
        contract_type,
        &endpoint.endpoint,
        endpoint.lifecycle,
        endpoint.state_mode,
        args,
        payload,
        context,
        remaining,
    )
}

use crate::module_ipc::client;
use crate::module_ipc::protocol::{ModuleMessage, ModuleRuntimeState};
use crate::module_ipc::server::runtime_registry;

use crate::{modules, privileges};

use serde_json::Value;

use std::collections::BTreeMap;
use std::time::Duration;

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
    let runtime = runtime_registry().get(module)?.ok_or_else(|| {
        format!(
            "module '{}' declares '{}:{}' but has no registered runtime",
            module, contract_type, endpoint_name
        )
    })?;

    if runtime.state != ModuleRuntimeState::Ready {
        return Err(format!(
            "module '{}' runtime session '{}' is not ready",
            module, runtime.session_id
        ));
    }

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
    client::invoke(
        module,
        contract_type,
        &endpoint.endpoint,
        endpoint.lifecycle,
        endpoint.state_mode,
        args,
        payload,
        context,
        timeout,
    )
}

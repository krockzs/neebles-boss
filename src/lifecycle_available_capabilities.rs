use std::collections::BTreeMap;
use std::os::fd::AsRawFd;
use std::sync::{Arc, OnceLock};

use crate::lifecycle_capabilities::{CapabilityFuture, CapabilityHandler};
use crate::lifecycle_execution::ExecutionPayload;
use crate::lifecycle_operation::PreparedOperation;
use crate::module_ipc::protocol::ModuleMessage;

#[derive(Clone)]
pub struct AvailableCapability {
    implementation_id: String,
    handler: CapabilityHandler,
}

impl AvailableCapability {
    pub fn new(
        implementation_id: impl Into<String>,
        handler: CapabilityHandler,
    ) -> Result<Self, String> {
        let implementation_id = implementation_id.into();

        if implementation_id.trim().is_empty() {
            return Err("available capability implementation id cannot be empty".to_string());
        }

        Ok(Self {
            implementation_id,
            handler,
        })
    }

    pub fn implementation_id(&self) -> &str {
        &self.implementation_id
    }

    pub fn handler(&self) -> CapabilityHandler {
        self.handler.clone()
    }
}

#[derive(Default)]
pub struct AvailableCapabilityCatalog {
    capabilities: BTreeMap<String, AvailableCapability>,
}

impl AvailableCapabilityCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        artillery: impl Into<String>,
        capability: AvailableCapability,
    ) -> Result<(), String> {
        let artillery = artillery.into();

        if artillery.trim().is_empty() {
            return Err("available capability artillery id cannot be empty".to_string());
        }

        if self.capabilities.contains_key(&artillery) {
            return Err(format!(
                "available capability artillery '{}' is already registered",
                artillery
            ));
        }

        self.capabilities.insert(artillery, capability);

        Ok(())
    }

    pub fn resolve(&self, artillery: &str) -> Result<&AvailableCapability, String> {
        if artillery.trim().is_empty() {
            return Err("available capability artillery id cannot be empty".to_string());
        }

        self.capabilities.get(artillery).ok_or_else(|| {
            format!(
                "Boss has no available Rust capability for artillery '{}'",
                artillery
            )
        })
    }

    pub fn contains(&self, artillery: &str) -> bool {
        self.capabilities.contains_key(artillery)
    }
}

const MODULE_IPC_ARTILLERY: &str = "boss.module_ipc";

const MODULE_IPC_IMPLEMENTATION: &str = "rust.boss.module_ipc";

fn module_ipc_operation_identity(
    operation: &PreparedOperation,
) -> Result<(String, String, String), String> {
    for key in operation.munition.keys() {
        if key != "endpoint" {
            return Err(format!("module IPC munition contains unknown key {}", key));
        }
    }

    if operation.munition.len() != 1 {
        return Err("module IPC invocation requires exactly munition.endpoint".to_string());
    }

    let module = operation.module_id.clone().ok_or_else(|| {
        "module IPC invocation requires Governor-owned module identity".to_string()
    })?;

    let contract = operation.objective.clone();

    let endpoint = operation
        .munition
        .get("endpoint")
        .ok_or_else(|| "module IPC munition.endpoint is missing".to_string())?
        .clone();

    for (label, value) in [
        ("module", module.as_str()),
        ("contract", contract.as_str()),
        ("endpoint", endpoint.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(format!("module IPC {} cannot be empty", label));
        }

        if value.trim() != value {
            return Err(format!(
                "module IPC {} cannot contain surrounding whitespace",
                label
            ));
        }
    }

    Ok((module, contract, endpoint))
}

async fn invoke_declared_without_blocking_executor(
    module: String,
    contract: String,
    endpoint: String,
) -> Result<ModuleMessage, String> {
    let (sender, receiver) = async_channel::bounded(1);

    std::thread::Builder::new()
        .name("neebles-lifecycle-module-ipc".to_string())
        .spawn(move || {
            let result = crate::module_ipc::invoke_declared(
                &module,
                &contract,
                &endpoint,
                Vec::new(),
                None,
                BTreeMap::new(),
                None,
            );

            let _ = sender.send_blocking(result);
        })
        .map_err(|error| format!("could not spawn module IPC Lifecycle worker: {}", error))?;

    receiver.recv().await.map_err(|error| {
        format!(
            "module IPC Lifecycle worker disappeared before delivering a result: {}",
            error
        )
    })?
}

fn module_ipc_response_payload(
    module: &str,
    contract: &str,
    endpoint: &str,
    message: ModuleMessage,
) -> Result<ExecutionPayload, String> {
    match message {
        ModuleMessage::Response {
            ok,
            code,
            result,
            error,
            ..
        } => {
            if !ok {
                let message = error
                    .map(|error| error.message)
                    .filter(|message| !message.trim().is_empty())
                    .unwrap_or_else(|| {
                        format!(
                            "module runtime returned failure code {}",
                            code
                        )
                    });

                return Err(format!(
                    "module IPC invocation failed: module={} contract={} endpoint={} code={} error={}",
                    module,
                    contract,
                    endpoint,
                    code,
                    message,
                ));
            }

            let mut payload = ExecutionPayload::from([
                ("module".to_string(), module.to_string()),
                ("contract".to_string(), contract.to_string()),
                ("endpoint".to_string(), endpoint.to_string()),
                ("code".to_string(), code.to_string()),
            ]);

            if let Some(result) = result {
                payload.insert(
                    "result".to_string(),
                    serde_json::to_string(&result).map_err(|error| {
                        format!(
                            "could not serialize module IPC result payload: {}",
                            error
                        )
                    })?,
                );
            }

            Ok(payload)
        }

        ModuleMessage::Error { error, .. } => Err(format!(
            "module IPC invocation failed: module={} contract={} endpoint={} kind={} error={}",
            module,
            contract,
            endpoint,
            error.kind,
            error.message,
        )),

        other => Err(format!(
            "module IPC invocation returned unexpected message: module={} contract={} endpoint={} message={:?}",
            module,
            contract,
            endpoint,
            other,
        )),
    }
}

async fn execute_module_ipc_operation(
    operation: PreparedOperation,
) -> Result<ExecutionPayload, String> {
    let (module, contract, endpoint) = module_ipc_operation_identity(&operation)?;

    let message = invoke_declared_without_blocking_executor(
        module.clone(),
        contract.clone(),
        endpoint.clone(),
    )
    .await?;

    module_ipc_response_payload(&module, &contract, &endpoint, message)
}

fn module_ipc_handler(operation: PreparedOperation) -> CapabilityFuture {
    Box::pin(execute_module_ipc_operation(operation))
}

const WORKSPACE_ARTILLERY: &str = "boss.workspace_execution";

const WORKSPACE_OBJECTIVE: &str = "construction.step";

const WORKSPACE_IMPLEMENTATION: &str = "rust.boss.workspace_execution";

fn workspace_operation_identity(operation: &PreparedOperation) -> Result<(String, String), String> {
    if operation.objective != WORKSPACE_OBJECTIVE {
        return Err(format!(
            "artillery '{}' does not support objective '{}'; expected '{}'",
            WORKSPACE_ARTILLERY, operation.objective, WORKSPACE_OBJECTIVE,
        ));
    }

    for key in operation.munition.keys() {
        if key != "step" {
            return Err(format!(
                "workspace execution munition contains unknown key '{}'",
                key
            ));
        }
    }

    if operation.munition.len() != 1 {
        return Err("workspace execution requires exactly munition.step".to_string());
    }

    let subject = operation
        .module_id
        .clone()
        .ok_or_else(|| "workspace execution requires Governor-owned module identity".to_string())?;

    neebles_backend::domestic_construction::validate_domestic_construction_subject(&subject)?;

    let step = operation
        .munition
        .get("step")
        .ok_or_else(|| "workspace execution munition.step is missing".to_string())?
        .clone();

    if step.trim().is_empty() {
        return Err("workspace execution munition.step cannot be empty".to_string());
    }

    if step.trim() != step {
        return Err(
            "workspace execution munition.step cannot contain surrounding whitespace".to_string(),
        );
    }

    Ok((subject, step))
}

/*
 * DomesticConstruction already owns the canonical conversion from
 * subject + step into the authenticated WorkspaceExecutionRequest.
 *
 * Lifecycle must not recreate that policy.
 *
 * The resulting std::process::Command is already fully composed by
 * the domestic boundary engine and has env_clear semantics.
 *
 * We reproduce only the finished process envelope using
 * async_process so Lifecycle does not turn one long domestic step
 * into a synchronous blocker for unrelated asynchronous work.
 */
fn async_domestic_command(command: std::process::Command) -> async_process::Command {
    let mut asynchronous = async_process::Command::new(command.get_program());

    asynchronous.env_clear();

    asynchronous.args(command.get_args());

    if let Some(directory) = command.get_current_dir() {
        asynchronous.current_dir(directory);
    }

    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            asynchronous.env(key, value);
        }
    }

    asynchronous
}

async fn execute_workspace_operation(
    operation: PreparedOperation,
) -> Result<ExecutionPayload, String> {
    let governor_execution_id = operation.execution_id.clone();

    let (subject, step) = workspace_operation_identity(&operation)?;

    let declaration =
        neebles_backend::domestic_construction::load_module_domestic_construction_declaration(
            &subject,
        )?;

    let registry =
        neebles_backend::domestic_authority_supply_process::process_supplied_authority_registry()?;

    let execution = declaration.step(&step)?.execution;

    let prepared = neebles_backend::domestic_construction::build_construction_step_command(
        &declaration,
        &step,
        registry,
    )?;

    let (command, runtime_lease) = prepared.into_parts();

    match execution {
        neebles_backend::domestic_construction::DomesticConstructionExecution::Foreground => {
            let status = async_domestic_command(command)
                .status()
                .await
                .map_err(|error| {
                    format!(
                        "domestic workspace execution failed to start: subject={} step={} error={}",
                        subject, step, error,
                    )
                })?;

            let exit_code = status.code().unwrap_or(125);

            drop(runtime_lease);

            if !status.success() {
                return Err(format!(
                    "domestic workspace execution failed: subject={} step={} exit_code={}",
                    subject, step, exit_code,
                ));
            }

            Ok(ExecutionPayload::from([
                ("subject".to_string(), subject),
                ("step".to_string(), step),
                ("execution".to_string(), "foreground".to_string()),
                ("exit_code".to_string(), exit_code.to_string()),
            ]))
        }

        neebles_backend::domestic_construction::DomesticConstructionExecution::Persistent => {
            let mut command = command;

            let owns_runtime_lease = runtime_lease.is_some();

            let runtime_execution_id = if owns_runtime_lease {
                Some(governor_execution_id.clone().ok_or_else(|| {
                    "persistent domestic runtime requires Governor-owned execution identity"
                        .to_string()
                })?)
            } else {
                None
            };

            let mut child = command.spawn().map_err(|error| {
                format!(
                    "persistent domestic workspace failed to start: subject={} step={} error={}",
                    subject, step, error,
                )
            })?;

            let pid = child.id();

            let opening_generation = if let Some(execution_id) = runtime_execution_id.as_deref() {
                match crate::module_ipc::runtime_registry().claim_opening_owner(
                    &subject,
                    execution_id,
                    pid,
                ) {
                    Ok(generation) => generation,

                    Err(error) => {
                        let _ = child.kill();
                        let _ = child.wait();

                        return Err(format!(
                                "persistent domestic runtime owner registration failed: subject={} step={} error={}",
                                subject,
                                step,
                                error
                            ));
                    }
                }
            } else {
                None
            };

            let opening_owner =
                opening_generation.map(|generation| (subject.clone(), generation, pid));

            std::thread::spawn(move || {
                let _runtime_lease = runtime_lease;
                let _ = child.wait();

                if let Some((module, generation, owner_pid)) = opening_owner {
                    match crate::module_ipc::runtime_registry()
                        .release_opening_owner(&module, generation, owner_pid)
                    {
                        Ok(true) => {
                            crate::modules::broadcast_surface_module_change(
                                &module,
                                "runtime_open_failed",
                            );
                        }

                        Ok(false) => {}

                        Err(error) => {
                            eprintln!(
                                "N.E.E.B.L.E.S.: runtime opening owner cleanup failed for {} generation {} pid {}: {}",
                                module,
                                generation,
                                owner_pid,
                                error
                            );
                        }
                    }
                }
            });

            Ok(ExecutionPayload::from([
                ("subject".to_string(), subject),
                ("step".to_string(), step),
                ("execution".to_string(), "persistent".to_string()),
                ("pid".to_string(), pid.to_string()),
            ]))
        }
    }
}

fn workspace_execution_handler(operation: PreparedOperation) -> CapabilityFuture {
    Box::pin(execute_workspace_operation(operation))
}

// Generic, Governor-scoped user-service installation. The module can name only
// one resource already declared in its authenticated uninstall contract.
const USER_SERVICE_ARTILLERY: &str = "boss.user_service";
const USER_SERVICE_IMPLEMENTATION: &str = "rust.boss.user_service.install";
fn user_service_install_handler(operation: PreparedOperation) -> CapabilityFuture {
    Box::pin(async move {
        if operation.objective != "install"
            || operation.munition.len() != 1
            || !operation.munition.contains_key("resource")
            || !operation.tactics.is_empty()
            || !operation.intelligence.is_empty()
        {
            return Err(
                "user service requires only objective=install and munition.resource".into(),
            );
        }
        let module = operation
            .module_id
            .ok_or("user service requires Governor-owned module")?;
        let resource = operation
            .munition
            .get("resource")
            .ok_or("missing resource")?
            .clone();
        crate::module_uninstall::install_declared_user_service(&module, &resource)?;
        Ok(ExecutionPayload::from([
            ("module".to_string(), module),
            ("resource".to_string(), resource),
            ("installed".to_string(), "true".to_string()),
        ]))
    })
}

// Platform operations require a root-authenticated OS provider.
const PLATFORM_OPERATION_ARTILLERY: &str = "boss.platform_operation";
const PLATFORM_OPERATION_IMPLEMENTATION: &str = "rust.boss.platform_operation";
// Privileged OS operations are handled by a separate root-owned provider.
// Boss remains unprivileged. The module never selects an executable or socket.
// Generic privileged-platform dispatch: Boss has no knowledge of module technologies.
const PLATFORM_AUTHORITY: &str = "neebles.platform.operation";
const PLATFORM_SOCKET: &str = "/run/neebles/platform-operations.sock";

fn platform_operation_validate(operation: &PreparedOperation) -> Result<(&str, &str), String> {
    if operation.objective != "execute" {
        return Err("platform operation requires objective=execute".into());
    }
    let module = operation
        .module_id
        .as_deref()
        .ok_or("platform operation requires Governor module identity")?;
    if module.is_empty()
        || module.len() > 128
        || !module
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("invalid Governor module identity".into());
    }
    if operation.munition.len() != 2
        || operation
            .munition
            .get("require_privilege")
            .map(String::as_str)
            != Some("true")
    {
        return Err(
            "platform operation requires exactly operation and require_privilege=true".into(),
        );
    }
    let action = operation
        .munition
        .get("operation")
        .ok_or("platform operation missing operation")?
        .as_str();
    if action.is_empty()
        || action.len() > 128
        || !action.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_' || b == b'-'
        })
    {
        return Err("invalid platform operation identifier".into());
    }
    Ok((module, action))
}

fn platform_operation_invoke(operation: PreparedOperation) -> Result<ExecutionPayload, String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;
    let (module, action) = platform_operation_validate(&operation)?;
    // Registered supply is OS-owned, not a field provided by the module.
    let registry =
        neebles_backend::domestic_authority_supply_process::process_supplied_authority_registry()?;
    let descriptor = registry.descriptor_path(PLATFORM_AUTHORITY)?;
    neebles_backend::domestic_platform_control::authenticate_platform_controlled_file(descriptor)?;
    let raw = std::fs::read(descriptor)
        .map_err(|e| format!("platform authority descriptor unreadable: {e}"))?;
    if raw.len() > 16384 {
        return Err("platform authority descriptor too large".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("invalid platform authority descriptor: {e}"))?;
    let obj = value
        .as_object()
        .ok_or("platform authority descriptor must be an object")?;
    if obj.len() != 4
        || obj.get("schema").and_then(|v| v.as_str()) != Some("1")
        || obj.get("authority").and_then(|v| v.as_str()) != Some(PLATFORM_AUTHORITY)
        || obj.get("module").and_then(|v| v.as_str()) != Some(module)
    {
        return Err("platform authority descriptor identity mismatch".into());
    }
    let actions = obj
        .get("operations")
        .and_then(|v| v.as_array())
        .ok_or("platform authority operations missing")?;
    if actions.is_empty()
        || actions.len() > 64
        || !actions.iter().all(|v| v.as_str().is_some())
        || !actions.iter().any(|v| v.as_str() == Some(action))
    {
        return Err("platform operation was not granted by OS descriptor".into());
    }
    let mut stream = UnixStream::connect(PLATFORM_SOCKET)
        .map_err(|e| format!("OS platform provider unavailable: {e}"))?;
    let timeout = Some(Duration::from_secs(10));
    stream
        .set_read_timeout(timeout)
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(timeout)
        .map_err(|e| e.to_string())?;
    let mut creds: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut creds as *mut libc::ucred).cast(),
            &mut len,
        )
    };
    if rc != 0 || len as usize != std::mem::size_of::<libc::ucred>() || creds.uid != 0 {
        return Err("OS platform provider is not authenticated as root".into());
    }
    let payload =
        serde_json::to_vec(&serde_json::json!({"schema":"1","module":module,"operation":action}))
            .map_err(|e| e.to_string())?;
    stream
        .write_all(&payload)
        .map_err(|e| format!("OS provider write failed: {e}"))?;
    stream.write_all(b"\n").map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    let mut terminated = false;
    while response.len() < 16384 {
        match stream
            .read(&mut byte)
            .map_err(|e| format!("OS provider read failed: {e}"))?
        {
            0 => return Err("OS provider closed before response".into()),
            _ if byte[0] == b'\n' => {
                terminated = true;
                break;
            }
            _ => response.push(byte[0]),
        }
    }
    if !terminated {
        return Err("OS provider response too large".into());
    }
    let reply: serde_json::Value = serde_json::from_slice(&response)
        .map_err(|e| format!("OS provider invalid response: {e}"))?;
    let r = reply
        .as_object()
        .ok_or("OS provider reply must be object")?;
    if r.get("schema").and_then(|v| v.as_str()) != Some("1")
        || r.get("ok").and_then(|v| v.as_bool()) != Some(true)
    {
        return Err(format!(
            "OS provider denied operation: {}",
            r.get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("no successful response")
        ));
    }
    Ok(ExecutionPayload::from([
        ("operation".into(), action.into()),
        ("module".into(), module.into()),
        (
            "provider_result".into(),
            r.get("result")
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".into()),
        ),
    ]))
}

fn platform_operation_handler(operation: PreparedOperation) -> CapabilityFuture {
    Box::pin(async move {
        let (sender, receiver) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("neebles-platform-operation".into())
            .spawn(move || {
                let _ = sender.send_blocking(platform_operation_invoke(operation));
            })
            .map_err(|e| format!("cannot spawn platform worker: {e}"))?;
        receiver
            .recv()
            .await
            .map_err(|e| format!("platform worker terminated: {e}"))?
    })
}

static PRODUCTIVE_CATALOG: OnceLock<Arc<AvailableCapabilityCatalog>> = OnceLock::new();

fn build_productive_catalog() -> Result<AvailableCapabilityCatalog, String> {
    /*
     * Productive Rust arsenal entrypoint.
     *
     * Lifecycle does not know module semantics and does not know
     * individual Rust crates.
     *
     * The stable generic execution boundary is Boss domestic
     * workspace execution. Modules provide only declarative
     * step identities through Lifecycle; module identity is injected
     * by Governor-owned execution context.
     *
     * Construction, AuthoritySupply, world resolution and
     * filesystem boundaries remain owned by their existing
     * canonical layers.
     */
    let mut catalog = AvailableCapabilityCatalog::new();

    catalog.register(
        WORKSPACE_ARTILLERY,
        AvailableCapability::new(
            WORKSPACE_IMPLEMENTATION,
            Arc::new(workspace_execution_handler),
        )?,
    )?;

    catalog.register(
        MODULE_IPC_ARTILLERY,
        AvailableCapability::new(MODULE_IPC_IMPLEMENTATION, Arc::new(module_ipc_handler))?,
    )?;

    catalog.register(
        USER_SERVICE_ARTILLERY,
        AvailableCapability::new(
            USER_SERVICE_IMPLEMENTATION,
            Arc::new(user_service_install_handler),
        )?,
    )?;

    catalog.register(
        PLATFORM_OPERATION_ARTILLERY,
        AvailableCapability::new(
            PLATFORM_OPERATION_IMPLEMENTATION,
            Arc::new(platform_operation_handler),
        )?,
    )?;

    Ok(catalog)
}

pub fn productive_catalog() -> Result<Arc<AvailableCapabilityCatalog>, String> {
    if let Some(catalog) = PRODUCTIVE_CATALOG.get() {
        return Ok(Arc::clone(catalog));
    }

    let catalog = Arc::new(build_productive_catalog()?);

    match PRODUCTIVE_CATALOG.set(Arc::clone(&catalog)) {
        Ok(()) => Ok(catalog),

        Err(_) => PRODUCTIVE_CATALOG.get().cloned().ok_or_else(|| {
            "productive Lifecycle capability catalog initialization failed".to_string()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    fn capability(implementation: &str) -> AvailableCapability {
        AvailableCapability::new(
            implementation,
            Arc::new(|_| Box::pin(async { Ok(ExecutionPayload::new()) })),
        )
        .unwrap()
    }

    #[test]
    fn catalog_starts_empty() {
        let catalog = AvailableCapabilityCatalog::new();

        assert!(!catalog.contains("anything"));
    }

    #[test]
    fn catalog_accepts_arbitrary_artillery() {
        let mut catalog = AvailableCapabilityCatalog::new();

        catalog
            .register("future.tool", capability("rust.future.tool"))
            .unwrap();

        assert!(catalog.contains("future.tool"));
    }

    #[test]
    fn catalog_keeps_implementation_private_to_boss() {
        let mut catalog = AvailableCapabilityCatalog::new();

        catalog
            .register("module.requests.this", capability("boss.private.adapter"))
            .unwrap();

        assert_eq!(
            catalog
                .resolve("module.requests.this")
                .unwrap()
                .implementation_id(),
            "boss.private.adapter"
        );
    }

    #[test]
    fn unavailable_artillery_fails_explicitly() {
        let catalog = AvailableCapabilityCatalog::new();

        let error = match catalog.resolve("missing.tool") {
            Ok(_) => {
                panic!("missing capability unexpectedly resolved");
            }

            Err(error) => error,
        };

        assert!(error.contains("no available Rust capability"));
    }

    #[test]
    fn platform_operation_is_registered_without_replacing_legacy_artillery() {
        let catalog = build_productive_catalog().unwrap();
        assert_eq!(
            catalog
                .resolve(PLATFORM_OPERATION_ARTILLERY)
                .unwrap()
                .implementation_id(),
            PLATFORM_OPERATION_IMPLEMENTATION
        );
        for legacy in [
            WORKSPACE_ARTILLERY,
            MODULE_IPC_ARTILLERY,
            USER_SERVICE_ARTILLERY,
        ] {
            assert!(catalog.resolve(legacy).is_ok());
        }
    }

    #[test]
    fn platform_operation_fails_without_authenticated_os_provider() {
        let operation = PreparedOperation {
            module_id: Some("neebles-proxy-server".into()),
            execution_id: None,
            artillery: PLATFORM_OPERATION_ARTILLERY.into(),
            objective: "execute".into(),
            munition: BTreeMap::from([
                ("operation".into(), "tailscale.exit_node.activate".into()),
                ("require_privilege".into(), "true".into()),
            ]),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        };
        let result = futures_lite::future::block_on(platform_operation_handler(operation));
        assert!(result.unwrap_err().contains("AuthoritySupply"));
    }

    #[test]
    fn productive_catalog_exposes_generic_module_ipc() {
        let catalog = build_productive_catalog().expect("productive catalog must build");

        let available = catalog
            .resolve(MODULE_IPC_ARTILLERY)
            .expect("module IPC artillery must be AVAILABLE");

        assert_eq!(available.implementation_id(), MODULE_IPC_IMPLEMENTATION,);
    }

    #[test]
    fn module_ipc_identity_uses_governor_module_and_objective_as_contract() {
        for contract in ["commands", "connect", "llm"] {
            let operation = PreparedOperation {
                module_id: Some("module.alpha".to_string()),
                execution_id: None,
                artillery: MODULE_IPC_ARTILLERY.to_string(),
                objective: contract.to_string(),
                munition: BTreeMap::from([("endpoint".to_string(), "logical.action".to_string())]),
                tactics: BTreeMap::new(),
                intelligence: BTreeMap::new(),
            };

            let (module, resolved_contract, endpoint) =
                module_ipc_operation_identity(&operation).unwrap();

            assert_eq!(module, "module.alpha");
            assert_eq!(resolved_contract, contract);
            assert_eq!(endpoint, "logical.action");
        }
    }

    #[test]
    fn module_ipc_identity_rejects_missing_identity_and_extra_munition() {
        let missing_identity = PreparedOperation {
            module_id: None,
            execution_id: None,
            artillery: MODULE_IPC_ARTILLERY.to_string(),
            objective: "commands".to_string(),
            munition: BTreeMap::from([("endpoint".to_string(), "notify".to_string())]),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        };

        let error = module_ipc_operation_identity(&missing_identity).unwrap_err();

        assert!(error.contains("Governor-owned module identity"));

        let extra = PreparedOperation {
            module_id: Some("module.alpha".to_string()),
            execution_id: None,
            artillery: MODULE_IPC_ARTILLERY.to_string(),
            objective: "commands".to_string(),
            munition: BTreeMap::from([
                ("endpoint".to_string(), "notify".to_string()),
                ("technology".to_string(), "forbidden".to_string()),
            ]),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        };

        let error = module_ipc_operation_identity(&extra).unwrap_err();

        assert!(error.contains("unknown key"));
    }

    #[test]
    fn module_ipc_response_mapping_preserves_success_and_failure() {
        let success = module_ipc_response_payload(
            "module.alpha",
            "commands",
            "notify",
            ModuleMessage::Response {
                id: "request-success".to_string(),
                module: "module.alpha".to_string(),
                session_id: "session-a".to_string(),
                contract: "commands".to_string(),
                endpoint: "test.notify".to_string(),
                ok: true,
                code: 0,
                result: Some(serde_json::json!({
                    "notified": true
                })),
                error: None,
            },
        )
        .unwrap();

        assert_eq!(success.get("module"), Some(&"module.alpha".to_string()),);

        assert_eq!(success.get("contract"), Some(&"commands".to_string()),);

        assert_eq!(success.get("endpoint"), Some(&"notify".to_string()),);

        assert_eq!(
            success.get("result"),
            Some(&"{\"notified\":true}".to_string()),
        );

        let failure = module_ipc_response_payload(
            "module.alpha",
            "commands",
            "notify",
            ModuleMessage::Response {
                id: "request-failure".to_string(),
                module: "module.alpha".to_string(),
                session_id: "session-a".to_string(),
                contract: "commands".to_string(),
                endpoint: "test.notify".to_string(),
                ok: false,
                code: 7,
                result: None,
                error: Some(crate::module_ipc::protocol::ModuleError {
                    kind: "runtime_failure".to_string(),
                    message: "synthetic runtime failure".to_string(),
                    details: None,
                }),
            },
        )
        .unwrap_err();

        assert!(failure.contains("synthetic runtime failure"));
        assert!(failure.contains("code=7"));
    }

    #[test]
    fn productive_catalog_exposes_generic_workspace_execution() {
        let catalog = build_productive_catalog().expect("productive catalog must build");

        let available = catalog
            .resolve(WORKSPACE_ARTILLERY)
            .expect("workspace artillery must be AVAILABLE");

        assert_eq!(available.implementation_id(), WORKSPACE_IMPLEMENTATION,);
    }

    #[test]
    fn workspace_execution_rejects_unknown_objective_before_authority_use() {
        let operation = PreparedOperation {
            module_id: Some("fixture".to_string()),
            execution_id: None,
            artillery: WORKSPACE_ARTILLERY.to_string(),

            objective: "future.unknown".to_string(),

            munition: BTreeMap::from([("step".to_string(), "run".to_string())]),

            tactics: BTreeMap::new(),

            intelligence: BTreeMap::new(),
        };

        let error = futures_lite::future::block_on(execute_workspace_operation(operation))
            .expect_err("unknown objective must fail before execution");

        assert!(error.contains("does not support objective"));
    }

    #[test]
    fn workspace_execution_requires_exact_step_munition() {
        let missing = PreparedOperation {
            module_id: Some("fixture".to_string()),
            execution_id: None,
            artillery: WORKSPACE_ARTILLERY.to_string(),

            objective: WORKSPACE_OBJECTIVE.to_string(),

            munition: BTreeMap::new(),

            tactics: BTreeMap::new(),

            intelligence: BTreeMap::new(),
        };

        let error = futures_lite::future::block_on(execute_workspace_operation(missing))
            .expect_err("missing step must fail before execution");

        assert!(error.contains("exactly munition.step"));

        let extra = PreparedOperation {
            module_id: Some("fixture".to_string()),
            execution_id: None,
            artillery: WORKSPACE_ARTILLERY.to_string(),

            objective: WORKSPACE_OBJECTIVE.to_string(),

            munition: BTreeMap::from([
                ("step".to_string(), "run".to_string()),
                ("technology".to_string(), "forbidden".to_string()),
            ]),

            tactics: BTreeMap::new(),

            intelligence: BTreeMap::new(),
        };

        let error = futures_lite::future::block_on(execute_workspace_operation(extra))
            .expect_err("unknown munition must fail before execution");

        assert!(error.contains("unknown key"));
    }

    #[test]
    fn workspace_execution_uses_governor_owned_module_identity() {
        let operation = PreparedOperation {
            module_id: Some("fixture".to_string()),
            execution_id: None,
            artillery: WORKSPACE_ARTILLERY.to_string(),
            objective: WORKSPACE_OBJECTIVE.to_string(),
            munition: BTreeMap::from([("step".to_string(), "run".to_string())]),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        };

        let (subject, step) = workspace_operation_identity(&operation).unwrap();

        assert_eq!(subject, "fixture");
        assert_eq!(step, "run");
    }

    #[test]
    fn workspace_execution_rejects_missing_governor_owned_module_identity() {
        let operation = PreparedOperation {
            module_id: None,
            execution_id: None,
            artillery: WORKSPACE_ARTILLERY.to_string(),
            objective: WORKSPACE_OBJECTIVE.to_string(),
            munition: BTreeMap::from([("step".to_string(), "run".to_string())]),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        };

        let error = workspace_operation_identity(&operation).unwrap_err();

        assert!(error.contains("Governor-owned module identity"));
    }
}

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
    timeout: Duration,
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::contracts::{Lifecycle, StateMode};
    use crate::module_ipc::protocol::{
        ModuleMessage, ModuleRuntimeState, MODULES_PROTOCOL_VERSION,
    };
    use crate::module_ipc::registry::ModuleRuntimeRecord;
    use crate::module_ipc::server::{pending_registry, runtime_registry};

    use serde_json::json;

    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn temporary_root() -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "neebles-router-test-{}-{}",
            std::process::id(),
            stamp
        ))
    }

    fn install_test_module(root: &std::path::Path, module: &str) {
        let module_dir = root.join("modules").join(module);
        let languages_dir = module_dir.join("languages");

        fs::create_dir_all(&languages_dir).expect("create module test directories");

        fs::write(
            module_dir.join("manifest.json"),
            format!(
                r#"{{
  "schema": 3,
  "name": "{module}",
  "version": "1.0.0",
  "entrypoint": "runtime.sh",
  "contracts": [
    {{
      "type": "commands",
      "file": "commands.json",
      "schema": 1,
      "enabled": true
    }}
  ]
}}"#
            ),
        )
        .expect("write manifest");

        fs::write(
            module_dir.join("commands.json"),
            r#"{
  "schema": 1,
  "contract": "commands",
  "endpoints": {
    "gradient": {
      "endpoint": "gradient.create",
      "lifecycle": "runtime",
      "state_mode": "clean",
      "requires_root": false
    }
  }
}"#,
        )
        .expect("write commands contract");

        fs::write(
            languages_dir.join("manifest.json"),
            r#"{
  "schema": 1,
  "default": "es",
  "languages": [
    {
      "code": "es"
    }
  ]
}"#,
        )
        .expect("write language manifest");

        fs::write(module_dir.join("runtime.sh"), "#!/bin/sh\nexit 0\n").expect("write runtime");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(module_dir.join("runtime.sh"))
                .expect("runtime metadata")
                .permissions();

            permissions.set_mode(0o755);

            fs::set_permissions(module_dir.join("runtime.sh"), permissions)
                .expect("runtime permissions");
        }
    }

    #[test]
    fn invoke_declared_resolves_logical_endpoint_and_rejects_unadvertised_runtime_capability() {
        let root = temporary_root();
        let module = "router-test-module";
        let session = "router-session-001";

        install_test_module(&root, module);

        std::env::set_var("NEEBLES_ROOT", &root);

        /*
         * Ensure global test registries start clean for this
         * module/session.
         */
        let _ = runtime_registry().unregister(module, session);

        /*
         * ----------------------------------------------------
         * CASE 1
         *
         * Contract logical name:
         *   gradient
         *
         * Installed endpoint:
         *   gradient.create
         *
         * Runtime advertises:
         *   gradient.create
         *
         * Router must send exactly the declared lifecycle/state.
         * ----------------------------------------------------
         */
        let (writer_tx, writer_rx) = mpsc::channel::<ModuleMessage>();

        let mut advertised = BTreeMap::<String, Vec<String>>::new();

        advertised.insert("commands".to_string(), vec!["gradient.create".to_string()]);

        runtime_registry()
            .register(ModuleRuntimeRecord {
                module: module.to_string(),
                session_id: session.to_string(),
                protocol: MODULES_PROTOCOL_VERSION,
                state: ModuleRuntimeState::Ready,
                endpoints: advertised,
                writer: writer_tx,
            })
            .expect("runtime registration");

        let worker_module = module.to_string();
        let worker_session = session.to_string();

        let worker = thread::spawn(move || {
            let invoke = writer_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("router should send Invoke");

            match invoke {
                ModuleMessage::Invoke {
                    id,
                    module,
                    session_id,
                    contract,
                    endpoint,
                    lifecycle,
                    state_mode,
                    args,
                    payload,
                    context,
                } => {
                    assert_eq!(module, worker_module);

                    assert_eq!(session_id, worker_session);

                    assert_eq!(contract, "commands");

                    /*
                     * The critical translation:
                     *
                     * logical "gradient"
                     * ->
                     * runtime "gradient.create"
                     */
                    assert_eq!(endpoint, "gradient.create");

                    assert_eq!(lifecycle, Lifecycle::Runtime);

                    assert_eq!(state_mode, StateMode::Clean);

                    assert_eq!(args, vec!["alpha".to_string()]);

                    assert_eq!(
                        payload,
                        Some(json!({
                            "strength": 7
                        }))
                    );

                    assert_eq!(
                        context.get("caller").and_then(|value| value.as_str()),
                        Some("router-test")
                    );

                    let response = ModuleMessage::Response {
                        id: id.clone(),
                        module: module.clone(),
                        session_id: session_id.clone(),
                        contract: contract.clone(),
                        endpoint: endpoint.clone(),
                        ok: true,
                        code: 0,
                        result: Some(json!({
                            "received_endpoint": endpoint
                        })),
                        error: None,
                    };

                    assert!(pending_registry()
                        .resolve(&id, response)
                        .expect("resolve pending request"));
                }

                message => {
                    panic!("expected Invoke, got {:?}", message);
                }
            }
        });

        let mut context = BTreeMap::<String, Value>::new();

        context.insert(
            "caller".to_string(),
            Value::String("router-test".to_string()),
        );

        let response = invoke_declared(
            module,
            "commands",
            "gradient",
            vec!["alpha".to_string()],
            Some(json!({
                "strength": 7
            })),
            context,
            Duration::from_secs(2),
        )
        .expect("declared invoke should succeed");

        match response {
            ModuleMessage::Response {
                ok,
                code,
                endpoint,
                result,
                ..
            } => {
                assert!(ok);
                assert_eq!(code, 0);

                assert_eq!(endpoint, "gradient.create");

                assert_eq!(
                    result,
                    Some(json!({
                        "received_endpoint":
                            "gradient.create"
                    }))
                );
            }

            message => {
                panic!("expected Response, got {:?}", message);
            }
        }

        worker.join().expect("runtime worker");

        assert!(runtime_registry()
            .unregister(module, session)
            .expect("unregister accepted runtime"));

        /*
         * ----------------------------------------------------
         * CASE 2
         *
         * Contract still declares gradient.create, but runtime
         * advertises only another endpoint.
         *
         * Router must reject before emitting Invoke.
         * ----------------------------------------------------
         */
        let (writer_tx, writer_rx) = mpsc::channel::<ModuleMessage>();

        let mut advertised = BTreeMap::<String, Vec<String>>::new();

        advertised.insert("commands".to_string(), vec!["hello".to_string()]);

        runtime_registry()
            .register(ModuleRuntimeRecord {
                module: module.to_string(),
                session_id: session.to_string(),
                protocol: MODULES_PROTOCOL_VERSION,
                state: ModuleRuntimeState::Ready,
                endpoints: advertised,
                writer: writer_tx,
            })
            .expect("second runtime registration");

        let error = invoke_declared(
            module,
            "commands",
            "gradient",
            Vec::new(),
            None,
            BTreeMap::new(),
            Duration::from_millis(200),
        )
        .expect_err("unadvertised endpoint must be rejected");

        assert!(
            error.contains("does not advertise declared endpoint 'gradient.create'"),
            "unexpected router error: {error}"
        );

        assert!(
            writer_rx.try_recv().is_err(),
            "router must not emit Invoke for an endpoint the runtime did not advertise"
        );

        assert!(runtime_registry()
            .unregister(module, session)
            .expect("unregister second runtime"));

        assert_eq!(pending_registry().len().expect("pending len"), 0);

        std::env::remove_var("NEEBLES_ROOT");

        fs::remove_dir_all(&root).expect("remove router test root");
    }
}

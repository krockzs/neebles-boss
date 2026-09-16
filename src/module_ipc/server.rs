use crate::config;
use crate::modules;
use crate::settings;

use crate::module_ipc::framing::{read_message, write_message};

use crate::module_ipc::pending::PendingRegistry;

use crate::module_ipc::protocol::{
    ModuleError, ModuleMessage, ModuleRuntimeState, MODULES_PROTOCOL_VERSION,
};

use crate::module_ipc::registry::{ModuleRuntimeRecord, RuntimeRegistry};

use std::collections::BTreeSet;
use std::env;
use std::fs;

use std::os::unix::net::{UnixListener, UnixStream};

use std::path::PathBuf;

use std::sync::{mpsc, OnceLock};

const DEFAULT_SOCKET_PATH: &str = "/run/neebles/modules.sock";

static RUNTIME_REGISTRY: OnceLock<RuntimeRegistry> = OnceLock::new();

static PENDING_REGISTRY: OnceLock<PendingRegistry> = OnceLock::new();

pub fn runtime_registry() -> &'static RuntimeRegistry {
    RUNTIME_REGISTRY.get_or_init(RuntimeRegistry::new)
}

pub fn pending_registry() -> &'static PendingRegistry {
    PENDING_REGISTRY.get_or_init(PendingRegistry::new)
}

pub fn socket_path() -> PathBuf {
    match env::var("NEEBLES_MODULES_SOCKET") {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value),

        _ => PathBuf::from(DEFAULT_SOCKET_PATH),
    }
}

fn bind_listener() -> Result<UnixListener, String> {
    let path = socket_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "could not create modules socket directory {}: {error}",
                parent.display()
            )
        })?;
    }

    if path.exists() {
        fs::remove_file(&path).map_err(|error| {
            format!(
                "could not remove stale modules socket {}: {error}",
                path.display()
            )
        })?;
    }

    let listener = UnixListener::bind(&path)
        .map_err(|error| format!("could not bind modules socket {}: {error}", path.display()))?;

    crate::ipc::secure_runtime_socket(&path)?;

    println!("N.E.E.B.L.E.S. module IPC listening on {}", path.display());

    Ok(listener)
}

fn accept_loop(listener: UnixListener) -> Result<(), String> {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(move || {
                    if let Err(error) = handle_client(stream) {
                        eprintln!("N.E.E.B.L.E.S.: module IPC client error: {error}");
                    }
                });
            }

            Err(error) => {
                eprintln!("N.E.E.B.L.E.S.: modules socket accept error: {error}");
            }
        }
    }

    Ok(())
}

pub fn serve() -> Result<(), String> {
    let listener = bind_listener()?;

    accept_loop(listener)
}

/*
 * Start modules.sock inside the SAME persistent Boss process.
 *
 * Binding is done synchronously so startup errors are returned
 * immediately instead of being hidden inside a background thread.
 */
pub fn start_background() -> Result<std::thread::JoinHandle<Result<(), String>>, String> {
    let listener = bind_listener()?;

    Ok(std::thread::spawn(move || accept_loop(listener)))
}

/*
 * Validate the identity and advertised capabilities of a
 * runtime before allowing it into RuntimeRegistry.
 *
 * Installed contract JSON is Boss's source of truth.
 * A runtime may advertise a subset of declared endpoints,
 * but it may never advertise an undeclared contract or endpoint.
 */
fn validate_registration(
    module: &str,
    session_id: &str,
    endpoints: &std::collections::BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    let module = module.trim();
    let session_id = session_id.trim();

    if module.is_empty() {
        return Err("runtime module name cannot be empty".to_string());
    }

    if session_id.is_empty() {
        return Err(format!(
            "module '{}' runtime session id cannot be empty",
            module
        ));
    }

    /*
     * This performs the canonical installed-module lookup
     * and module-id validation already owned by modules.rs.
     */
    let manifest = modules::installed_module_manifest(module).map_err(|error| {
        format!(
            "runtime registration rejected for module '{}': {}",
            module, error
        )
    })?;

    /*
     * The directory selected by Boss and the identity declared
     * by that installation must agree with the runtime identity.
     */
    if manifest.name != module {
        return Err(format!(
            "runtime registration identity mismatch: requested '{}' but installed manifest declares '{}'",
            module,
            manifest.name
        ));
    }

    if !config::module_enabled(module)? {
        return Err(format!(
            "module '{}' is disabled and cannot register a runtime",
            module
        ));
    }

    /*
     * Load all enabled dynamic contracts through the generic
     * contract loader. Boss still does not know contract types.
     */
    let declared = modules::installed_module_contracts(module).map_err(|error| {
        format!(
            "could not load contracts for runtime module '{}': {}",
            module, error
        )
    })?;

    for (contract_type, advertised_endpoints) in endpoints {
        let contract_type = contract_type.trim();

        if contract_type.is_empty() {
            return Err(format!(
                "module '{}' advertised an empty contract type",
                module
            ));
        }

        let contract = declared.contracts.get(contract_type).ok_or_else(|| {
            format!(
                "module '{}' advertised undeclared contract '{}'",
                module, contract_type
            )
        })?;

        /*
         * Runtime endpoint identity is the `endpoint` field,
         * not the logical key used inside contract JSON.
         *
         * Example:
         *
         * "gradient": {
         *     "endpoint": "gradient.create"
         * }
         *
         * Runtime announces "gradient.create".
         */
        let allowed = contract
            .endpoints
            .values()
            .map(|endpoint| endpoint.endpoint.as_str())
            .collect::<BTreeSet<_>>();

        let mut seen = BTreeSet::new();

        for endpoint in advertised_endpoints {
            let endpoint = endpoint.trim();

            if endpoint.is_empty() {
                return Err(format!(
                    "module '{}' contract '{}' advertised an empty endpoint",
                    module, contract_type
                ));
            }

            if !seen.insert(endpoint) {
                return Err(format!(
                    "module '{}' contract '{}' advertised endpoint '{}' more than once",
                    module, contract_type, endpoint
                ));
            }

            if !allowed.contains(endpoint) {
                return Err(format!(
                    "module '{}' contract '{}' advertised undeclared endpoint '{}'",
                    module, contract_type, endpoint
                ));
            }
        }
    }

    Ok(())
}

fn handle_client(mut stream: UnixStream) -> Result<(), String> {
    /*
     * Primer mensaje obligatorio:
     * Register.
     */
    let first = read_message(&mut stream)?
        .ok_or_else(|| "module runtime disconnected before register".to_string())?;

    let (protocol, module, session_id, endpoints) = match first {
        ModuleMessage::Register {
            protocol,
            module,
            session_id,
            endpoints,
        } => (protocol, module, session_id, endpoints),

        _ => {
            let response = ModuleMessage::Error {
                id: None,
                module: None,

                error: ModuleError {
                    kind: "register_required".to_string(),

                    message: "first module IPC message must be register".to_string(),

                    details: None,
                },
            };

            let _ = write_message(&mut stream, &response);

            return Err("first module IPC message was not register".to_string());
        }
    };

    if protocol != MODULES_PROTOCOL_VERSION {
        let response = ModuleMessage::Error {
            id: None,

            module: Some(module.clone()),

            error: ModuleError {
                kind: "unsupported_protocol".to_string(),

                message: format!(
                    "module protocol {} is unsupported; Boss supports {}",
                    protocol, MODULES_PROTOCOL_VERSION
                ),

                details: None,
            },
        };

        let _ = write_message(&mut stream, &response);

        return Err(format!("unsupported module protocol {protocol}"));
    }

    /*
     * Protocol compatibility alone is not enough.
     * Boss now verifies that this runtime belongs to a real,
     * enabled installation and that every advertised endpoint
     * exists in that module's dynamic contracts.
     */
    if let Err(error) = validate_registration(&module, &session_id, &endpoints) {
        let response = ModuleMessage::Error {
            id: None,

            module: if module.trim().is_empty() {
                None
            } else {
                Some(module.clone())
            },

            error: ModuleError {
                kind: "register_rejected".to_string(),

                message: error.clone(),

                details: None,
            },
        };

        let _ = write_message(&mut stream, &response);

        return Err(error);
    }

    /*
     * Una conexión Unix persistente necesita un lector
     * y un escritor independientes.
     *
     * try_clone() duplica el descriptor y ambos representan
     * la misma conexión.
     */
    let mut writer_stream = stream.try_clone().map_err(|error| {
        format!(
            "could not clone module IPC stream for '{}': {error}",
            module
        )
    })?;

    /*
     * Cola interna Boss -> runtime.
     */
    let (writer_sender, writer_receiver) = mpsc::channel::<ModuleMessage>();

    /*
     * Registrar el runtime ANTES de anunciar Registered.
     *
     * Si ya existe otro runtime del mismo módulo,
     * la nueva sesión se rechaza.
     */
    let record = ModuleRuntimeRecord {
        module: module.clone(),

        session_id: session_id.clone(),

        protocol,

        state: ModuleRuntimeState::Ready,

        endpoints,

        subscriptions: BTreeSet::new(),

        writer: writer_sender.clone(),
    };

    if let Err(error) = runtime_registry().register(record) {
        let response = ModuleMessage::Error {
            id: None,

            module: Some(module.clone()),

            error: ModuleError {
                kind: "register_rejected".to_string(),

                message: error.clone(),

                details: None,
            },
        };

        let _ = write_message(&mut stream, &response);

        return Err(error);
    }

    /*
     * Writer dedicado.
     *
     * Cualquier parte de Boss podrá mandar mensajes al runtime
     * usando writer_sender sin tocar directamente UnixStream.
     */
    let writer_module = module.clone();

    let writer_session = session_id.clone();

    let writer_handle = std::thread::spawn(move || {
        while let Ok(message) = writer_receiver.recv() {
            if let Err(error) = write_message(&mut writer_stream, &message) {
                eprintln!(
                    "N.E.E.B.L.E.S.: module IPC writer failed for '{}' session '{}': {}",
                    writer_module, writer_session, error
                );

                break;
            }
        }
    });

    /*
     * Confirmar registro usando ya el writer persistente.
     */
    if let Err(error) = writer_sender.send(ModuleMessage::Registered {
        protocol: MODULES_PROTOCOL_VERSION,

        module: module.clone(),

        session_id: session_id.clone(),
    }) {
        let _ = runtime_registry().unregister(&module, &session_id);

        return Err(format!(
            "could not acknowledge runtime registration for '{}': {error}",
            module
        ));
    }

    if let Err(error) = runtime_registry().broadcast_event(
        "module.lifecycle",
        "runtime_ready",
        serde_json::json!({
            "module": module,
            "session_id": session_id
        }),
    ) {
        eprintln!(
            "N.E.E.B.L.E.S.: runtime '{}' became ready but lifecycle event broadcast failed: {}",
            module, error
        );
    }

    /*
     * Reader persistente.
     */
    let result = client_loop(&mut stream, &writer_sender, &module, &session_id);

    /*
     * The reader ended: this exact runtime incarnation is no
     * longer available.
     *
     * Remove it from RuntimeRegistry first so no new request
     * can acquire this session.
     */
    let removed = match runtime_registry().unregister(&module, &session_id) {
        Ok(removed) => removed,

        Err(error) => {
            eprintln!(
                "N.E.E.B.L.E.S.: could not unregister runtime '{}' session '{}': {}",
                module, session_id, error
            );

            false
        }
    };

    if removed {
        if let Err(error) = runtime_registry().broadcast_event(
            "module.lifecycle",
            "runtime_dead",
            serde_json::json!({
                "module": module,
                "session_id": session_id
            }),
        ) {
            eprintln!(
                "N.E.E.B.L.E.S.: runtime '{}' ended but lifecycle event broadcast failed: {}",
                module, error
            );
        }
    }

    /*
     * Wake every request already in flight for this exact
     * session immediately.
     *
     * This must happen before joining the writer thread:
     * waiting invoke() callers may still hold cloned writers
     * through their runtime snapshot. Waking them releases
     * those clones without waiting for their normal timeout.
     */
    let disconnect_reason = match &result {
        Ok(()) => "runtime connection closed".to_string(),

        Err(error) => {
            format!("runtime connection failed: {}", error)
        }
    };

    match pending_registry().fail_session(&module, &session_id, &disconnect_reason) {
        Ok(failed) if failed > 0 => {
            eprintln!(
                "N.E.E.B.L.E.S.: failed {} pending request(s) for module '{}' session '{}'",
                failed, module, session_id
            );
        }

        Ok(_) => {}

        Err(error) => {
            eprintln!(
                "N.E.E.B.L.E.S.: could not fail pending requests for module '{}' session '{}': {}",
                module, session_id, error
            );
        }
    }

    /*
     * Registry ownership and all in-flight request ownership
     * for this session have now been released.
     */
    drop(writer_sender);

    let _ = writer_handle.join();

    result
}

fn client_loop(
    stream: &mut UnixStream,
    writer: &mpsc::Sender<ModuleMessage>,
    module: &str,
    session_id: &str,
) -> Result<(), String> {
    loop {
        let Some(message) = read_message(stream)? else {
            /*
             * EOF:
             * runtime murió o cerró limpiamente sin Unregister.
             * handle_client eliminará la sesión del registry.
             */
            return Ok(());
        };

        match message {
            ModuleMessage::Pong {
                module: pong_module,

                session_id: pong_session,
            } => {
                validate_session(module, session_id, &pong_module, &pong_session)?;
            }

            ModuleMessage::Ping {
                module: ping_module,

                session_id: ping_session,
            } => {
                validate_session(module, session_id, &ping_module, &ping_session)?;

                writer
                    .send(ModuleMessage::Pong {
                        module: module.to_string(),

                        session_id: session_id.to_string(),
                    })
                    .map_err(|error| {
                        format!("could not queue pong for module '{}': {error}", module)
                    })?;
            }

            ModuleMessage::Subscribe {
                module: subscribe_module,

                session_id: subscribe_session,

                topics,
            } => {
                validate_session(module, session_id, &subscribe_module, &subscribe_session)?;

                let topics = topics
                    .into_iter()
                    .map(|topic| topic.trim().to_string())
                    .filter(|topic| !topic.is_empty())
                    .collect::<BTreeSet<_>>();

                if topics.is_empty() {
                    writer
                        .send(ModuleMessage::Error {
                            id: None,

                            module: Some(module.to_string()),

                            error: ModuleError {
                                kind: "invalid_subscription".to_string(),

                                message: "module subscription requires at least one topic"
                                    .to_string(),

                                details: None,
                            },
                        })
                        .map_err(|error| {
                            format!(
                                "could not queue subscription error for module '{}': {error}",
                                module
                            )
                        })?;

                    continue;
                }

                let own_settings_topic = format!("settings.{}", module);

                if let Some(forbidden) = topics.iter().find(|topic| {
                    topic.starts_with("settings.") && topic.as_str() != own_settings_topic.as_str()
                }) {
                    writer
                        .send(ModuleMessage::Error {
                            id: None,

                            module: Some(module.to_string()),

                            error: ModuleError {
                                kind: "forbidden_subscription".to_string(),

                                message: format!(
                                    "module '{}' cannot subscribe to settings topic '{}'",
                                    module,
                                    forbidden
                                ),

                                details: None,
                            },
                        })
                        .map_err(|error| {
                            format!(
                                "could not queue forbidden subscription error for module '{}': {error}",
                                module
                            )
                        })?;

                    continue;
                }

                runtime_registry().set_subscriptions(module, session_id, topics.clone())?;

                writer
                    .send(ModuleMessage::Subscribed {
                        module: module.to_string(),

                        session_id: session_id.to_string(),

                        topics: topics.into_iter().collect(),
                    })
                    .map_err(|error| {
                        format!(
                            "could not acknowledge subscriptions for module '{}': {error}",
                            module
                        )
                    })?;
            }

            ModuleMessage::SettingsGet {
                id,
                module: settings_module,
                session_id: settings_session,
                path: setting_path,
            } => {
                validate_session(module, session_id, &settings_module, &settings_session)?;

                let result = (|| -> Result<String, String> {
                    let default = modules::installed_module_settings_default(module)?;

                    let settings_path =
                        settings::module_settings_path(&modules::neebles_root(), module);

                    let local = settings::load_or_create(&settings_path, &default)?;

                    settings::get_effective_path(&local, &default, &setting_path)
                })();

                match result {
                    Ok(value) => {
                        writer
                            .send(ModuleMessage::SettingsValue {
                                id,
                                module: module.to_string(),
                                session_id: session_id.to_string(),
                                path: setting_path,
                                value,
                            })
                            .map_err(|error| {
                                format!(
                                    "could not queue settings value for module '{}': {error}",
                                    module
                                )
                            })?;
                    }

                    Err(error) => {
                        writer
                            .send(ModuleMessage::Error {
                                id: Some(id),
                                module: Some(module.to_string()),
                                error: ModuleError {
                                    kind: "settings_read".to_string(),
                                    message: error,
                                    details: None,
                                },
                            })
                            .map_err(|send_error| {
                                format!(
                                    "could not queue settings read error for module '{}': {send_error}",
                                    module
                                )
                            })?;
                    }
                }
            }

            ModuleMessage::SettingsSet {
                id,
                module: settings_module,
                session_id: settings_session,
                path: setting_path,
                value,
            } => {
                validate_session(module, session_id, &settings_module, &settings_session)?;

                let result = (|| -> Result<(String, bool), String> {
                    let default = modules::installed_module_settings_default(module)?;

                    let settings_path =
                        settings::module_settings_path(&modules::neebles_root(), module);

                    let local = settings::load_or_create(&settings_path, &default)?;

                    let previous = settings::get_effective_path(&local, &default, &setting_path)?;

                    let value = settings::set_path(&settings_path, &default, &setting_path, value)?;

                    let changed = previous != value;

                    Ok((value, changed))
                })();

                match result {
                    Ok((value, changed)) => {
                        writer
                            .send(ModuleMessage::SettingsValue {
                                id,
                                module: module.to_string(),
                                session_id: session_id.to_string(),
                                path: setting_path.clone(),
                                value: value.clone(),
                            })
                            .map_err(|error| {
                                format!(
                                    "could not queue settings value for module '{}': {error}",
                                    module
                                )
                            })?;

                        if changed {
                            if let Err(error) = runtime_registry().broadcast_event(
                                &format!("settings.{}", module),
                                "changed",
                                serde_json::json!({
                                    "target": module,
                                    "path": setting_path,
                                    "value": value
                                }),
                            ) {
                                eprintln!(
                                    "N.E.E.B.L.E.S.: module settings persisted but event broadcast failed for '{}': {}",
                                    module,
                                    error
                                );
                            }
                        }
                    }

                    Err(error) => {
                        writer
                            .send(ModuleMessage::Error {
                                id: Some(id),
                                module: Some(module.to_string()),
                                error: ModuleError {
                                    kind: "settings_write".to_string(),
                                    message: error,
                                    details: None,
                                },
                            })
                            .map_err(|send_error| {
                                format!(
                                    "could not queue settings write error for module '{}': {send_error}",
                                    module
                                )
                            })?;
                    }
                }
            }

            ModuleMessage::Unregister {
                module: unregister_module,

                session_id: unregister_session,
                ..
            } => {
                validate_session(module, session_id, &unregister_module, &unregister_session)?;

                return Ok(());
            }

            ModuleMessage::ShutdownAck {
                module: ack_module,

                session_id: ack_session,
            } => {
                validate_session(module, session_id, &ack_module, &ack_session)?;

                return Ok(());
            }

            ModuleMessage::Response {
                id,
                module: response_module,

                session_id: response_session,

                contract,
                endpoint,
                ok,
                code,
                result,
                error,
            } => {
                validate_session(module, session_id, &response_module, &response_session)?;

                let request_id = id.clone();

                let response = ModuleMessage::Response {
                    id,
                    module: response_module,

                    session_id: response_session,

                    contract,
                    endpoint,
                    ok,
                    code,
                    result,
                    error,
                };

                let resolved = pending_registry().resolve(&request_id, response)?;

                if !resolved {
                    eprintln!(
                        "N.E.E.B.L.E.S.: module '{}' returned response for unknown request '{}'",
                        module, request_id
                    );
                }
            }

            ModuleMessage::Error {
                id,
                module: error_module,
                error,
            } => {
                if let Some(received_module) = error_module.as_deref() {
                    if received_module != module {
                        return Err(format!(
                            "module IPC identity mismatch: expected '{}' received '{}'",
                            module, received_module
                        ));
                    }
                }

                if let Some(request_id) = id.clone() {
                    let response = ModuleMessage::Error {
                        id,
                        module: error_module,
                        error,
                    };

                    let resolved = pending_registry().resolve(&request_id, response)?;

                    if !resolved {
                        eprintln!(
                            "N.E.E.B.L.E.S.: module '{}' returned error for unknown request '{}'",
                            module, request_id
                        );
                    }
                } else {
                    eprintln!(
                        "N.E.E.B.L.E.S.: module '{}' reported runtime error: {}",
                        module, error.message
                    );
                }
            }

            ModuleMessage::Register { .. }
            | ModuleMessage::Registered { .. }
            | ModuleMessage::Subscribed { .. }
            | ModuleMessage::SettingsValue { .. }
            | ModuleMessage::Event { .. }
            | ModuleMessage::Invoke { .. }
            | ModuleMessage::Shutdown { .. } => {
                writer
                    .send(
                        ModuleMessage::Error {
                            id: None,

                            module:
                                Some(
                                    module.to_string()
                                ),

                            error:
                                ModuleError {
                                    kind:
                                        "unexpected_message"
                                            .to_string(),

                                    message:
                                        "message is not valid from a registered runtime in the current protocol state"
                                            .to_string(),

                                    details:
                                        None,
                                },
                        }
                    )
                    .map_err(|error| {
                        format!(
                            "could not queue protocol error for module '{}': {error}",
                            module
                        )
                    })?;
            }
        }
    }
}

fn validate_session(
    expected_module: &str,
    expected_session: &str,
    received_module: &str,
    received_session: &str,
) -> Result<(), String> {
    if received_module != expected_module {
        return Err(format!(
            "module IPC identity mismatch: expected '{}' received '{}'",
            expected_module, received_module
        ));
    }

    if received_session != expected_session {
        return Err(format!(
            "module IPC session mismatch for '{}': expected '{}' received '{}'",
            expected_module, expected_session, received_session
        ));
    }

    Ok(())
}

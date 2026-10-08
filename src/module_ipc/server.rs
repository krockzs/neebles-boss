use crate::config;
use crate::external;
use crate::modules;
use crate::notifications::{
    self, NotificationImageData, NotificationOptions, NotificationOutcome, NotificationOwner,
    NotificationPresentation, NotificationReturnEvent, Severity,
};

use crate::module_ipc::framing::{read_message, write_message};

use crate::module_ipc::pending::PendingRegistry;

use crate::module_ipc::protocol::{
    ModuleError, ModuleMessage, ModuleNotificationOptions, ModuleNotificationPresentation,
    MODULES_PROTOCOL_VERSION,
};

use crate::module_ipc::registry::{ModuleRuntimeRecord, RuntimeRegistry};

use std::collections::BTreeSet;
use std::env;
use std::fs;

use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};

use std::path::PathBuf;

use std::sync::{mpsc, OnceLock};

const DEFAULT_SOCKET_PATH: &str = "/run/neebles/modules.sock";
fn emit_module_error_external(module: &str, error: &ModuleError) {
    let Some(envelope) = external::build_external_envelope(
        "error",
        "",
        || {
            let mut package = serde_json::Map::new();
            package.insert(
                "module".to_string(),
                serde_json::Value::String(module.to_string()),
            );
            package
        },
        || external::error_message(&error.kind, &error.message),
    ) else {
        return;
    };

    if let Err(send_error) = external::send(&envelope) {
        eprintln!(
            "N.E.E.B.L.E.S.: external module error delivery unavailable; continuing: {send_error}"
        );
    }
}

static RUNTIME_REGISTRY: OnceLock<RuntimeRegistry> = OnceLock::new();

static PENDING_REGISTRY: OnceLock<PendingRegistry> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotificationReturnRoute {
    Delivered,
    BossOwned,
    Unknown,
}

fn notification_return_message(
    owner_module: &str,
    owner_session: &str,
    event: &NotificationReturnEvent,
) -> ModuleMessage {
    match event {
        NotificationReturnEvent::Closed {
            notification_id,
            reason,
        } => ModuleMessage::NotificationClosed {
            module: owner_module.to_string(),
            session_id: owner_session.to_string(),
            notification_id: *notification_id,
            reason: *reason,
        },

        NotificationReturnEvent::ActionInvoked {
            notification_id,
            action_key,
        } => ModuleMessage::NotificationActionInvoked {
            module: owner_module.to_string(),
            session_id: owner_session.to_string(),
            notification_id: *notification_id,
            action_key: action_key.clone(),
        },

        NotificationReturnEvent::Replied {
            notification_id,
            text,
        } => ModuleMessage::NotificationReplied {
            module: owner_module.to_string(),
            session_id: owner_session.to_string(),
            notification_id: *notification_id,
            text: text.clone(),
        },

        NotificationReturnEvent::ActivationToken {
            notification_id,
            activation_token,
        } => ModuleMessage::NotificationActivationToken {
            module: owner_module.to_string(),
            session_id: owner_session.to_string(),
            notification_id: *notification_id,
            activation_token: activation_token.clone(),
        },
    }
}

fn route_notification_return_event_with<FOwner, FSend, FRelease>(
    event: NotificationReturnEvent,
    owner_for: FOwner,
    send_to_session: FSend,
    release_owner: FRelease,
) -> Result<NotificationReturnRoute, String>
where
    FOwner: FnOnce(u32) -> Result<Option<NotificationOwner>, String>,
    FSend: FnOnce(&str, &str, ModuleMessage) -> Result<(), String>,
    FRelease: FnOnce(u32) -> Result<Option<NotificationOwner>, String>,
{
    let notification_id = event.notification_id();

    let Some(owner) = owner_for(notification_id)? else {
        return Ok(NotificationReturnRoute::Unknown);
    };

    match owner {
        NotificationOwner::Boss => {
            if event.is_terminal() {
                let _ = release_owner(notification_id)?;
            }

            Ok(NotificationReturnRoute::BossOwned)
        }

        NotificationOwner::Module { module, session_id } => {
            let message = notification_return_message(&module, &session_id, &event);

            let delivery = send_to_session(&module, &session_id, message);

            if event.is_terminal() {
                let release = release_owner(notification_id);

                match (delivery, release) {
                    (Err(error), _) => Err(error),
                    (Ok(()), Err(error)) => Err(error),
                    (Ok(()), Ok(_)) => Ok(NotificationReturnRoute::Delivered),
                }
            } else {
                delivery?;

                Ok(NotificationReturnRoute::Delivered)
            }
        }
    }
}

pub(crate) fn route_notification_return_event(
    event: NotificationReturnEvent,
) -> Result<(), String> {
    match route_notification_return_event_with(
        event,
        notifications::notification_owner,
        |module, session_id, message| {
            runtime_registry().send_to_session(module, session_id, message)
        },
        notifications::release_notification_owner,
    )? {
        NotificationReturnRoute::Delivered
        | NotificationReturnRoute::BossOwned
        | NotificationReturnRoute::Unknown => Ok(()),
    }
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModulePeerCredentials {
    pid: u32,
    uid: u32,
}

fn authorize_module_peer_uid(
    peer_uid: libc::uid_t,
    desktop_uid: libc::uid_t,
) -> Result<(), String> {
    if peer_uid == 0 || peer_uid == desktop_uid {
        return Ok(());
    }

    Err(format!(
        "module IPC rejected unauthorized peer uid {peer_uid}; expected root or desktop uid {desktop_uid}"
    ))
}

fn module_peer_credentials(stream: &UnixStream) -> Result<ModulePeerCredentials, String> {
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;

    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut credentials as *mut libc::ucred as *mut libc::c_void,
            &mut length,
        )
    };

    if result != 0 {
        return Err(format!(
            "could not read module IPC peer credentials: {}",
            std::io::Error::last_os_error()
        ));
    }

    if credentials.pid <= 0 {
        return Err("module IPC peer reported an invalid pid".to_string());
    }

    let (desktop_uid, _) = crate::runtime_identity::desktop_identity()?;

    authorize_module_peer_uid(credentials.uid, desktop_uid)?;

    Ok(ModulePeerCredentials {
        pid: credentials.pid as u32,
        uid: credentials.uid,
    })
}

#[cfg(test)]
mod module_peer_uid_tests {
    use super::authorize_module_peer_uid;

    #[test]
    fn module_peer_policy_allows_root() {
        authorize_module_peer_uid(0, 1000).expect("root must be allowed to connect to module IPC");
    }

    #[test]
    fn module_peer_policy_allows_desktop_uid() {
        authorize_module_peer_uid(1000, 1000)
            .expect("desktop uid must be allowed to connect to module IPC");
    }

    #[test]
    fn module_peer_policy_rejects_unrelated_uid() {
        let error =
            authorize_module_peer_uid(2000, 1000).expect_err("unrelated uid must be rejected");

        assert!(error.contains("unauthorized peer uid 2000"));
    }
}

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
     * Kernel-authenticated identity of this Unix peer.
     *
     * module/session values carried by the protocol remain logical
     * transport identity only. They are not physical ownership proof.
     */
    let peer = module_peer_credentials(&stream)?;

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
    let (runtime_pid, runtime_start_time_ticks) =
        match validate_registration(&module, &session_id, &endpoints)
            .and_then(|_| modules::verify_module_runtime_process(&module, peer.pid))
        {
            Ok(identity) => identity,

            Err(error) => {
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
        };

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

        pid: runtime_pid,

        start_time_ticks: runtime_start_time_ticks,

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
        if matches!(
            runtime_registry().unregister(&module, &session_id),
            Ok(true)
        ) {
            crate::modules::broadcast_surface_module_change(&module, "runtime_register_failed");
        }

        return Err(format!(
            "could not acknowledge runtime registration for '{}': {error}",
            module
        ));
    }

    crate::modules::broadcast_surface_module_change(&module, "runtime_ready");
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

    match notifications::release_module_notification_session(&module, &session_id) {
        Ok(released) if !released.is_empty() => {
            eprintln!(
                "N.E.E.B.L.E.S.: released {} notification ownership record(s) for module '{}' session '{}'",
                released.len(),
                module,
                session_id
            );
        }

        Ok(_) => {}

        Err(error) => {
            eprintln!(
                "N.E.E.B.L.E.S.: could not release notification ownership for module '{}' session '{}': {}",
                module,
                session_id,
                error
            );
        }
    }

    if removed {
        crate::modules::broadcast_surface_module_change(&module, "runtime_dead");
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

fn notification_presentation_from_wire(
    presentation: ModuleNotificationPresentation,
) -> NotificationPresentation {
    let ModuleNotificationPresentation {
        application,
        icon,
        title,
        message,
        actions,
        reply,
        options,
    } = presentation;

    let ModuleNotificationOptions {
        replace_id,
        expire_timeout_ms,
        category,
        desktop_entry,
        resident,
        transient,
        sound_name,
        sound_file,
        suppress_sound,
        image_path,
        image_data,
        kde_urls,
        kde_origin_name,
        kde_display_appname,
    } = options;

    NotificationPresentation {
        application,
        icon,
        title,
        message,
        actions: actions
            .into_iter()
            .map(|action| crate::notifications::NotificationAction {
                key: action.key,
                label: action.label,
            })
            .collect(),
        reply: reply.map(|reply| crate::notifications::NotificationReply {
            label: reply.label,
            placeholder_text: reply.placeholder_text,
            submit_button_text: reply.submit_button_text,
            submit_button_icon_name: reply.submit_button_icon_name,
        }),
        options: NotificationOptions {
            replace_id,
            expire_timeout_ms,
            category,
            desktop_entry,
            resident,
            transient,
            sound_name,
            sound_file,
            suppress_sound,
            image_path,
            image_data: image_data.map(|image| NotificationImageData {
                width: image.width,
                height: image.height,
                rowstride: image.rowstride,
                has_alpha: image.has_alpha,
                bits_per_sample: image.bits_per_sample,
                channels: image.channels,
                data: image.data,
            }),
            kde_urls,
            kde_origin_name,
            kde_display_appname,
        },
    }
}

fn handle_module_default_notification<F>(
    writer: &mpsc::Sender<ModuleMessage>,
    module: &str,
    session_id: &str,
    id: String,
    notification_module: String,
    notification_session: String,
    severity: String,
    icon: String,
    title: String,
    message: String,
    expire_timeout_ms: Option<i32>,
    replace_id: Option<u32>,
    emit: F,
) -> Result<(), String>
where
    F: FnOnce(
        &str,
        Severity,
        &str,
        &str,
        &str,
        Option<i32>,
        Option<u32>,
    ) -> Result<NotificationOutcome, String>,
{
    validate_session(
        module,
        session_id,
        &notification_module,
        &notification_session,
    )?;

    let result = Severity::parse(&severity).and_then(|severity| {
        emit(
            module,
            severity,
            &icon,
            &title,
            &message,
            expire_timeout_ms,
            replace_id,
        )
    });

    match result {
        Ok(outcome) => {
            writer
                .send(ModuleMessage::NotificationAck {
                    id,
                    module: module.to_string(),
                    session_id: session_id.to_string(),
                    notification_id: outcome.notification_id(),
                })
                .map_err(|error| {
                    format!(
                        "could not queue default notification acknowledgement for module '{}': {error}",
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
                        kind: "notification".to_string(),
                        message: error,
                        details: None,
                    },
                })
                .map_err(|send_error| {
                    format!(
                        "could not queue default notification error for module '{}': {send_error}",
                        module
                    )
                })?;
        }
    }

    Ok(())
}

fn handle_module_notification<F>(
    writer: &mpsc::Sender<ModuleMessage>,
    module: &str,
    session_id: &str,
    id: String,
    notification_module: String,
    notification_session: String,
    severity: String,
    presentation: ModuleNotificationPresentation,
    emit: F,
) -> Result<(), String>
where
    F: FnOnce(&str, Severity, &NotificationPresentation) -> Result<NotificationOutcome, String>,
{
    validate_session(
        module,
        session_id,
        &notification_module,
        &notification_session,
    )?;

    let result = Severity::parse(&severity).and_then(|severity| {
        let presentation = notification_presentation_from_wire(presentation);

        emit(module, severity, &presentation)
    });

    match result {
        Ok(outcome) => {
            writer
                .send(ModuleMessage::NotificationAck {
                    id,
                    module: module.to_string(),
                    session_id: session_id.to_string(),
                    notification_id: outcome.notification_id(),
                })
                .map_err(|error| {
                    format!(
                        "could not queue notification acknowledgement for module '{}': {error}",
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
                        kind: "notification".to_string(),
                        message: error,
                        details: None,
                    },
                })
                .map_err(|send_error| {
                    format!(
                        "could not queue notification error for module '{}': {send_error}",
                        module
                    )
                })?;
        }
    }

    Ok(())
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

                let result = modules::module_setting_get(module, &setting_path);

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

                let result = modules::module_setting_set(module, &setting_path, value);

                match result {
                    Ok(write) => {
                        writer
                            .send(ModuleMessage::SettingsValue {
                                id,
                                module: module.to_string(),
                                session_id: session_id.to_string(),
                                path: setting_path,
                                value: write.value,
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

            ModuleMessage::DefaultNotification {
                id,
                module: notification_module,
                session_id: notification_session,
                severity,
                icon,
                title,
                message,
                expire_timeout_ms,
                replace_id,
            } => {
                handle_module_default_notification(
                    writer,
                    module,
                    session_id,
                    id,
                    notification_module,
                    notification_session,
                    severity,
                    icon,
                    title,
                    message,
                    expire_timeout_ms,
                    replace_id,
                    |module, severity, icon, title, message, expire_timeout_ms, replace_id| {
                        notifications::emit_default_for_module_owned(
                            module,
                            session_id,
                            severity,
                            icon,
                            title,
                            message,
                            expire_timeout_ms,
                            replace_id,
                        )
                    },
                )?;
            }

            ModuleMessage::Notification {
                id,
                module: notification_module,
                session_id: notification_session,
                severity,
                presentation,
            } => {
                handle_module_notification(
                    writer,
                    module,
                    session_id,
                    id,
                    notification_module,
                    notification_session,
                    severity,
                    presentation,
                    |module, severity, presentation| {
                        notifications::emit_for_module_with_identity_owned(
                            module,
                            session_id,
                            severity,
                            presentation,
                        )
                    },
                )?;
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

                    contract: contract.clone(),
                    endpoint: endpoint.clone(),
                    ok,
                    code,
                    result,
                    error,
                };

                let resolved = pending_registry().resolve_response(
                    &request_id,
                    module,
                    session_id,
                    &contract,
                    &endpoint,
                    response,
                )?;

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

                    let resolved = pending_registry().resolve_error(
                        &request_id,
                        module,
                        session_id,
                        response,
                    )?;

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

                    emit_module_error_external(module, &error);
                }
            }

            ModuleMessage::Register { .. }
            | ModuleMessage::Registered { .. }
            | ModuleMessage::Subscribed { .. }
            | ModuleMessage::SettingsValue { .. }
            | ModuleMessage::NotificationAck { .. }
            | ModuleMessage::NotificationClosed { .. }
            | ModuleMessage::NotificationActionInvoked { .. }
            | ModuleMessage::NotificationReplied { .. }
            | ModuleMessage::NotificationActivationToken { .. }
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

#[cfg(test)]
mod notification_return_router_tests {
    use super::*;

    use std::cell::Cell;

    fn module_owner() -> NotificationOwner {
        NotificationOwner::Module {
            module: "alpha".to_string(),
            session_id: "session-a".to_string(),
        }
    }

    #[test]
    fn synthetic_action_routes_only_to_exact_owner() {
        let sent = Cell::new(false);

        let route = route_notification_return_event_with(
            NotificationReturnEvent::ActionInvoked {
                notification_id: 41,
                action_key: "open".to_string(),
            },
            |_| Ok(Some(module_owner())),
            |module, session_id, message| {
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");

                match message {
                    ModuleMessage::NotificationActionInvoked {
                        module,
                        session_id,
                        notification_id,
                        action_key,
                    } => {
                        assert_eq!(module, "alpha");
                        assert_eq!(session_id, "session-a");
                        assert_eq!(notification_id, 41);
                        assert_eq!(action_key, "open");
                    }

                    other => panic!("unexpected notification return message: {other:?}"),
                }

                sent.set(true);

                Ok(())
            },
            |_| panic!("action must not release ownership"),
        )
        .unwrap();

        assert_eq!(route, NotificationReturnRoute::Delivered);
        assert!(sent.get());
    }

    #[test]
    fn synthetic_reply_preserves_payload_and_owner() {
        let route = route_notification_return_event_with(
            NotificationReturnEvent::Replied {
                notification_id: 42,
                text: "respuesta".to_string(),
            },
            |_| Ok(Some(module_owner())),
            |module, session_id, message| {
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");

                match message {
                    ModuleMessage::NotificationReplied {
                        notification_id,
                        text,
                        ..
                    } => {
                        assert_eq!(notification_id, 42);
                        assert_eq!(text, "respuesta");
                    }

                    other => panic!("unexpected notification return message: {other:?}"),
                }

                Ok(())
            },
            |_| panic!("reply must not release ownership"),
        )
        .unwrap();

        assert_eq!(route, NotificationReturnRoute::Delivered);
    }

    #[test]
    fn synthetic_activation_token_preserves_payload() {
        let route = route_notification_return_event_with(
            NotificationReturnEvent::ActivationToken {
                notification_id: 43,
                activation_token: "token-43".to_string(),
            },
            |_| Ok(Some(module_owner())),
            |_, _, message| {
                match message {
                    ModuleMessage::NotificationActivationToken {
                        notification_id,
                        activation_token,
                        ..
                    } => {
                        assert_eq!(notification_id, 43);
                        assert_eq!(activation_token, "token-43");
                    }

                    other => panic!("unexpected notification return message: {other:?}"),
                }

                Ok(())
            },
            |_| panic!("activation token must not release ownership"),
        )
        .unwrap();

        assert_eq!(route, NotificationReturnRoute::Delivered);
    }

    #[test]
    fn synthetic_closed_delivers_then_releases_owner() {
        let delivered = Cell::new(false);
        let released = Cell::new(false);

        let route = route_notification_return_event_with(
            NotificationReturnEvent::Closed {
                notification_id: 44,
                reason: 2,
            },
            |_| Ok(Some(module_owner())),
            |module, session_id, message| {
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");

                match message {
                    ModuleMessage::NotificationClosed {
                        notification_id,
                        reason,
                        ..
                    } => {
                        assert_eq!(notification_id, 44);
                        assert_eq!(reason, 2);
                    }

                    other => panic!("unexpected notification return message: {other:?}"),
                }

                delivered.set(true);

                Ok(())
            },
            |notification_id| {
                assert_eq!(notification_id, 44);
                released.set(true);

                Ok(Some(module_owner()))
            },
        )
        .unwrap();

        assert_eq!(route, NotificationReturnRoute::Delivered);
        assert!(delivered.get());
        assert!(released.get());
    }

    #[test]
    fn synthetic_closed_releases_even_when_delivery_fails() {
        let released = Cell::new(false);

        let error = route_notification_return_event_with(
            NotificationReturnEvent::Closed {
                notification_id: 45,
                reason: 3,
            },
            |_| Ok(Some(module_owner())),
            |_, _, _| Err("runtime disappeared".to_string()),
            |notification_id| {
                assert_eq!(notification_id, 45);
                released.set(true);

                Ok(Some(module_owner()))
            },
        )
        .expect_err("delivery failure must propagate");

        assert_eq!(error, "runtime disappeared");
        assert!(released.get());
    }

    #[test]
    fn synthetic_unknown_notification_is_not_broadcast() {
        let route = route_notification_return_event_with(
            NotificationReturnEvent::ActionInvoked {
                notification_id: 999,
                action_key: "open".to_string(),
            },
            |_| Ok(None),
            |_, _, _| panic!("unknown notification must never be delivered"),
            |_| panic!("unknown notification must never release ownership"),
        )
        .unwrap();

        assert_eq!(route, NotificationReturnRoute::Unknown);
    }

    #[test]
    fn synthetic_boss_owned_event_never_enters_modules_socket() {
        let route = route_notification_return_event_with(
            NotificationReturnEvent::ActionInvoked {
                notification_id: 50,
                action_key: "boss-action".to_string(),
            },
            |_| Ok(Some(NotificationOwner::Boss)),
            |_, _, _| panic!("Boss-owned notification must never enter modules.sock"),
            |_| panic!("non-terminal Boss event must not release ownership"),
        )
        .unwrap();

        assert_eq!(route, NotificationReturnRoute::BossOwned);
    }

    #[test]
    fn synthetic_boss_closed_releases_without_module_delivery() {
        let released = Cell::new(false);

        let route = route_notification_return_event_with(
            NotificationReturnEvent::Closed {
                notification_id: 51,
                reason: 1,
            },
            |_| Ok(Some(NotificationOwner::Boss)),
            |_, _, _| panic!("Boss-owned notification must never enter modules.sock"),
            |notification_id| {
                assert_eq!(notification_id, 51);
                released.set(true);

                Ok(Some(NotificationOwner::Boss))
            },
        )
        .unwrap();

        assert_eq!(route, NotificationReturnRoute::BossOwned);
        assert!(released.get());
    }
}

#[cfg(test)]
mod transport_semantics_tests {
    use super::*;
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;

    fn spawn_loop(
        module: &str,
        session_id: &str,
    ) -> (
        UnixStream,
        mpsc::Receiver<ModuleMessage>,
        std::thread::JoinHandle<Result<(), String>>,
    ) {
        let (boss_stream, runtime_stream) =
            UnixStream::pair().expect("UnixStream pair must be available");

        let (writer, receiver) = mpsc::channel();

        let module = module.to_string();
        let session_id = session_id.to_string();

        let handle = std::thread::spawn(move || {
            let mut boss_stream = boss_stream;

            client_loop(&mut boss_stream, &writer, &module, &session_id)
        });

        (runtime_stream, receiver, handle)
    }

    #[test]
    fn ping_returns_pong_for_exact_session() {
        let (mut runtime_stream, receiver, handle) = spawn_loop("alpha", "session-a");

        write_message(
            &mut runtime_stream,
            &ModuleMessage::Ping {
                module: "alpha".to_string(),
                session_id: "session-a".to_string(),
            },
        )
        .unwrap();

        match receiver.recv().unwrap() {
            ModuleMessage::Pong { module, session_id } => {
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");
            }

            other => {
                panic!("expected Pong, got {:?}", other);
            }
        }

        write_message(
            &mut runtime_stream,
            &ModuleMessage::Unregister {
                module: "alpha".to_string(),
                session_id: "session-a".to_string(),
                reason: Some("test complete".to_string()),
            },
        )
        .unwrap();

        assert!(handle.join().unwrap().is_ok());
    }

    #[test]
    fn ping_with_wrong_session_terminates_connection_semantics() {
        let (mut runtime_stream, _receiver, handle) = spawn_loop("alpha", "session-a");

        write_message(
            &mut runtime_stream,
            &ModuleMessage::Ping {
                module: "alpha".to_string(),
                session_id: "session-b".to_string(),
            },
        )
        .unwrap();

        let error = handle
            .join()
            .unwrap()
            .expect_err("wrong-session Ping must fail client_loop");

        assert!(error.contains("session mismatch"));
    }

    #[test]
    fn pong_with_wrong_module_is_rejected() {
        let (mut runtime_stream, _receiver, handle) = spawn_loop("alpha", "session-a");

        write_message(
            &mut runtime_stream,
            &ModuleMessage::Pong {
                module: "beta".to_string(),
                session_id: "session-a".to_string(),
            },
        )
        .unwrap();

        let error = handle
            .join()
            .unwrap()
            .expect_err("wrong-module Pong must fail client_loop");

        assert!(error.contains("identity mismatch"));
    }

    #[test]
    fn unregister_requires_exact_session_and_closes_cleanly() {
        let (mut runtime_stream, _receiver, handle) = spawn_loop("alpha", "session-a");

        write_message(
            &mut runtime_stream,
            &ModuleMessage::Unregister {
                module: "alpha".to_string(),
                session_id: "session-a".to_string(),
                reason: Some("normal exit".to_string()),
            },
        )
        .unwrap();

        assert!(handle.join().unwrap().is_ok());
    }

    #[test]
    fn stale_unregister_is_rejected() {
        let (mut runtime_stream, _receiver, handle) = spawn_loop("alpha", "session-current");

        write_message(
            &mut runtime_stream,
            &ModuleMessage::Unregister {
                module: "alpha".to_string(),
                session_id: "session-old".to_string(),
                reason: Some("stale runtime".to_string()),
            },
        )
        .unwrap();

        let error = handle
            .join()
            .unwrap()
            .expect_err("stale Unregister must fail client_loop");

        assert!(error.contains("session mismatch"));
    }

    #[test]
    fn shutdown_ack_requires_exact_session_and_closes_cleanly() {
        let (mut runtime_stream, _receiver, handle) = spawn_loop("alpha", "session-a");

        write_message(
            &mut runtime_stream,
            &ModuleMessage::ShutdownAck {
                module: "alpha".to_string(),
                session_id: "session-a".to_string(),
            },
        )
        .unwrap();

        assert!(handle.join().unwrap().is_ok());
    }

    #[test]
    fn stale_shutdown_ack_is_rejected() {
        let (mut runtime_stream, _receiver, handle) = spawn_loop("alpha", "session-current");

        write_message(
            &mut runtime_stream,
            &ModuleMessage::ShutdownAck {
                module: "alpha".to_string(),
                session_id: "session-old".to_string(),
            },
        )
        .unwrap();

        let error = handle
            .join()
            .unwrap()
            .expect_err("stale ShutdownAck must fail client_loop");

        assert!(error.contains("session mismatch"));
    }

    #[test]
    fn runtime_cannot_command_boss_shutdown() {
        let (mut runtime_stream, receiver, handle) = spawn_loop("alpha", "session-a");

        write_message(
            &mut runtime_stream,
            &ModuleMessage::Shutdown {
                module: "alpha".to_string(),
                session_id: "session-a".to_string(),
                reason: Some("runtime tried authority inversion".to_string()),
            },
        )
        .unwrap();

        match receiver.recv().unwrap() {
            ModuleMessage::Error { error, .. } => {
                assert_eq!(error.kind, "unexpected_message");
            }

            other => {
                panic!("expected protocol Error, got {:?}", other);
            }
        }

        write_message(
            &mut runtime_stream,
            &ModuleMessage::Unregister {
                module: "alpha".to_string(),
                session_id: "session-a".to_string(),
                reason: Some("test complete".to_string()),
            },
        )
        .unwrap();

        assert!(handle.join().unwrap().is_ok());
    }

    #[test]
    fn eof_closes_client_loop_cleanly() {
        let (runtime_stream, _receiver, handle) = spawn_loop("alpha", "session-a");

        drop(runtime_stream);

        assert!(handle.join().unwrap().is_ok());
    }
    #[test]
    fn default_notification_applies_three_second_timeout_without_override() {
        let (writer, receiver) = mpsc::channel();

        handle_module_default_notification(
            &writer,
            "alpha",
            "session-a",
            "default-timeout".to_string(),
            "alpha".to_string(),
            "session-a".to_string(),
            "info".to_string(),
            "assets/alpha.png".to_string(),
            "Default title".to_string(),
            "Default body".to_string(),
            None,
            None,
            |module, severity, icon, title, message, expire, replace_id| {
                assert_eq!(module, "alpha");
                assert!(matches!(severity, Severity::Info));
                assert_eq!(icon, "assets/alpha.png");
                assert_eq!(title, "Default title");
                assert_eq!(message, "Default body");
                assert_eq!(expire, None);
                assert_eq!(replace_id, None);

                Ok(NotificationOutcome::Presented(91))
            },
        )
        .expect("default notification must succeed");

        match receiver.recv().expect("default acknowledgement expected") {
            ModuleMessage::NotificationAck {
                id,
                module,
                session_id,
                notification_id,
            } => {
                assert_eq!(id, "default-timeout");
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");
                assert_eq!(notification_id, Some(91));
            }

            other => panic!("expected NotificationAck, got {other:?}"),
        }
    }

    #[test]
    fn default_notification_preserves_expire_and_replace_overrides() {
        let (writer, receiver) = mpsc::channel();

        handle_module_default_notification(
            &writer,
            "alpha",
            "session-a",
            "default-overrides".to_string(),
            "alpha".to_string(),
            "session-a".to_string(),
            "warning".to_string(),
            "assets/alpha.png".to_string(),
            "Progress".to_string(),
            "Half way".to_string(),
            Some(8500),
            Some(41),
            |module, severity, icon, title, message, expire, replace_id| {
                assert_eq!(module, "alpha");
                assert!(matches!(severity, Severity::Warning));
                assert_eq!(icon, "assets/alpha.png");
                assert_eq!(title, "Progress");
                assert_eq!(message, "Half way");
                assert_eq!(expire, Some(8500));
                assert_eq!(replace_id, Some(41));

                Ok(NotificationOutcome::Presented(77))
            },
        )
        .expect("default override notification must succeed");

        match receiver
            .recv()
            .expect("default override acknowledgement expected")
        {
            ModuleMessage::NotificationAck {
                notification_id, ..
            } => {
                assert_eq!(notification_id, Some(77));
            }

            other => panic!("expected NotificationAck, got {other:?}"),
        }
    }

    fn test_notification_presentation(
        application: &str,
        icon: &str,
        title: &str,
        message: &str,
        replace_id: Option<u32>,
    ) -> ModuleNotificationPresentation {
        ModuleNotificationPresentation {
            application: application.to_string(),
            icon: icon.to_string(),
            title: title.to_string(),
            message: message.to_string(),
            actions: Vec::new(),
            reply: None,
            options: ModuleNotificationOptions {
                replace_id,
                ..ModuleNotificationOptions::default()
            },
        }
    }

    #[test]
    fn notification_exact_session_returns_ack_and_preserves_identity() {
        let (writer, receiver) = mpsc::channel();

        handle_module_notification(
            &writer,
            "alpha",
            "session-a",
            "notification-1".to_string(),
            "alpha".to_string(),
            "session-a".to_string(),
            "warning".to_string(),
            test_notification_presentation(
                "Alpha Worker",
                "assets/alpha.png",
                "Alpha title",
                "Alpha body",
                None,
            ),
            |module, severity, presentation| {
                assert_eq!(module, "alpha");
                assert!(matches!(severity, Severity::Warning));
                assert_eq!(presentation.application, "Alpha Worker");
                assert_eq!(presentation.icon, "assets/alpha.png");
                assert_eq!(presentation.options.replace_id, None);
                assert_eq!(presentation.title, "Alpha title");
                assert_eq!(presentation.message, "Alpha body");

                Ok(NotificationOutcome::Presented(77))
            },
        )
        .expect("exact notification session must be accepted");

        match receiver
            .recv()
            .expect("notification acknowledgement expected")
        {
            ModuleMessage::NotificationAck {
                id,
                module,
                session_id,
                notification_id,
            } => {
                assert_eq!(id, "notification-1");
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");
                assert_eq!(notification_id, Some(77));
            }

            other => panic!("unexpected notification response: {other:?}"),
        }
    }

    #[test]
    fn notification_replace_id_reaches_presenter_and_ack_returns_new_id() {
        let (writer, receiver) = mpsc::channel();

        handle_module_notification(
            &writer,
            "alpha",
            "session-a",
            "replace-request".to_string(),
            "alpha".to_string(),
            "session-a".to_string(),
            "info".to_string(),
            test_notification_presentation(
                "Alpha",
                "assets/alpha.png",
                "Replacement",
                "Updated body",
                Some(41),
            ),
            |module, severity, presentation| {
                assert_eq!(module, "alpha");
                assert!(matches!(severity, Severity::Info));
                assert_eq!(presentation.application, "Alpha");
                assert_eq!(presentation.icon, "assets/alpha.png");
                assert_eq!(presentation.options.replace_id, Some(41));
                assert_eq!(presentation.title, "Replacement");
                assert_eq!(presentation.message, "Updated body");

                Ok(NotificationOutcome::Presented(77))
            },
        )
        .expect("replacement notification must succeed");

        match receiver
            .recv()
            .expect("replacement notification acknowledgement expected")
        {
            ModuleMessage::NotificationAck {
                id,
                module,
                session_id,
                notification_id,
            } => {
                assert_eq!(id, "replace-request");
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");
                assert_eq!(notification_id, Some(77));
            }

            other => panic!("expected NotificationAck, got {other:?}"),
        }
    }

    #[test]
    fn notification_rich_options_reach_presenter_as_one_domain_object() {
        let (writer, receiver) = mpsc::channel();

        let presentation = ModuleNotificationPresentation {
            application: "Alpha Rich".to_string(),
            icon: "assets/alpha.png".to_string(),
            title: "Rich title".to_string(),
            message: "Rich body".to_string(),
            actions: vec![
                crate::module_ipc::protocol::ModuleNotificationAction {
                    key: "open".to_string(),
                    label: "Open".to_string(),
                },
                crate::module_ipc::protocol::ModuleNotificationAction {
                    key: "details".to_string(),
                    label: "Details".to_string(),
                },
            ],
            reply: Some(crate::module_ipc::protocol::ModuleNotificationReply {
                label: "Reply".to_string(),
                placeholder_text: Some("Write a reply".to_string()),
                submit_button_text: Some("Send".to_string()),
                submit_button_icon_name: Some("mail-send".to_string()),
            }),
            options: ModuleNotificationOptions {
                replace_id: Some(41),
                expire_timeout_ms: Some(9000),
                category: Some("transfer".to_string()),
                desktop_entry: Some("alpha.desktop".to_string()),
                resident: true,
                transient: true,
                sound_name: Some("message-new-instant".to_string()),
                sound_file: Some("/tmp/alpha.ogg".to_string()),
                suppress_sound: true,
                image_path: Some("/tmp/alpha.png".to_string()),
                image_data: Some(crate::module_ipc::protocol::ModuleNotificationImageData {
                    width: 1,
                    height: 1,
                    rowstride: 4,
                    has_alpha: true,
                    bits_per_sample: 8,
                    channels: 4,
                    data: vec![10, 20, 30, 255],
                }),
                kde_urls: vec![
                    "https://example.invalid/one".to_string(),
                    "https://example.invalid/two".to_string(),
                ],
                kde_origin_name: Some("Alpha Origin".to_string()),
                kde_display_appname: Some("Alpha Display".to_string()),
            },
        };

        handle_module_notification(
            &writer,
            "alpha",
            "session-a",
            "rich-notification".to_string(),
            "alpha".to_string(),
            "session-a".to_string(),
            "success".to_string(),
            presentation,
            |module, severity, presentation| {
                assert_eq!(module, "alpha");
                assert!(matches!(severity, Severity::Success));

                assert_eq!(presentation.application, "Alpha Rich");
                assert_eq!(presentation.icon, "assets/alpha.png");
                assert_eq!(presentation.title, "Rich title");
                assert_eq!(presentation.message, "Rich body");

                assert_eq!(presentation.actions.len(), 2);
                assert_eq!(presentation.actions[0].key, "open");
                assert_eq!(presentation.actions[0].label, "Open");
                assert_eq!(presentation.actions[1].key, "details");
                assert_eq!(presentation.actions[1].label, "Details");

                let reply = presentation
                    .reply
                    .as_ref()
                    .expect("rich inline reply expected");

                assert_eq!(reply.label, "Reply");
                assert_eq!(reply.placeholder_text.as_deref(), Some("Write a reply"));
                assert_eq!(reply.submit_button_text.as_deref(), Some("Send"));
                assert_eq!(reply.submit_button_icon_name.as_deref(), Some("mail-send"));

                assert_eq!(presentation.options.replace_id, Some(41));
                assert_eq!(presentation.options.expire_timeout_ms, Some(9000));
                assert_eq!(presentation.options.category.as_deref(), Some("transfer"));
                assert_eq!(
                    presentation.options.desktop_entry.as_deref(),
                    Some("alpha.desktop")
                );
                assert!(presentation.options.resident);
                assert!(presentation.options.transient);
                assert_eq!(
                    presentation.options.sound_name.as_deref(),
                    Some("message-new-instant")
                );
                assert_eq!(
                    presentation.options.sound_file.as_deref(),
                    Some("/tmp/alpha.ogg")
                );
                assert!(presentation.options.suppress_sound);
                assert_eq!(
                    presentation.options.image_path.as_deref(),
                    Some("/tmp/alpha.png")
                );

                let image = presentation
                    .options
                    .image_data
                    .as_ref()
                    .expect("rich image data expected");

                assert_eq!(image.width, 1);
                assert_eq!(image.height, 1);
                assert_eq!(image.rowstride, 4);
                assert!(image.has_alpha);
                assert_eq!(image.bits_per_sample, 8);
                assert_eq!(image.channels, 4);
                assert_eq!(image.data, vec![10, 20, 30, 255]);

                assert_eq!(
                    presentation.options.kde_urls,
                    vec![
                        "https://example.invalid/one".to_string(),
                        "https://example.invalid/two".to_string(),
                    ]
                );

                assert_eq!(
                    presentation.options.kde_origin_name.as_deref(),
                    Some("Alpha Origin")
                );

                assert_eq!(
                    presentation.options.kde_display_appname.as_deref(),
                    Some("Alpha Display")
                );

                Ok(NotificationOutcome::Presented(88))
            },
        )
        .expect("rich notification must succeed");

        match receiver
            .recv()
            .expect("rich notification acknowledgement expected")
        {
            ModuleMessage::NotificationAck {
                id,
                module,
                session_id,
                notification_id,
            } => {
                assert_eq!(id, "rich-notification");
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");
                assert_eq!(notification_id, Some(88));
            }

            other => panic!("expected NotificationAck, got {other:?}"),
        }
    }

    #[test]
    fn notification_wrong_module_is_rejected_before_presenter() {
        let (writer, receiver) = mpsc::channel();

        let error = handle_module_notification(
            &writer,
            "alpha",
            "session-a",
            "notification-2".to_string(),
            "beta".to_string(),
            "session-a".to_string(),
            "info".to_string(),
            test_notification_presentation("Fake", "fake.png", "Title", "Body", None),
            |_, _, _| panic!("presenter must not run for falsified module identity"),
        )
        .expect_err("falsified module identity must fail");

        assert!(error.contains("identity mismatch"));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn notification_wrong_session_is_rejected_before_presenter() {
        let (writer, receiver) = mpsc::channel();

        let error = handle_module_notification(
            &writer,
            "alpha",
            "session-a",
            "notification-3".to_string(),
            "alpha".to_string(),
            "stale-session".to_string(),
            "info".to_string(),
            test_notification_presentation("Alpha", "alpha.png", "Title", "Body", None),
            |_, _, _| panic!("presenter must not run for stale session"),
        )
        .expect_err("stale session must fail");

        assert!(error.contains("session"));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn notification_presenter_failure_returns_correlated_error() {
        let (writer, receiver) = mpsc::channel();

        handle_module_notification(
            &writer,
            "alpha",
            "session-a",
            "notification-4".to_string(),
            "alpha".to_string(),
            "session-a".to_string(),
            "critical".to_string(),
            test_notification_presentation("Alpha", "alpha.png", "Title", "Body", None),
            |_, _, _| Err("synthetic presenter failure".to_string()),
        )
        .expect("presenter failure must be reported through protocol");

        match receiver.recv().expect("notification error expected") {
            ModuleMessage::Error { id, module, error } => {
                assert_eq!(id.as_deref(), Some("notification-4"));
                assert_eq!(module.as_deref(), Some("alpha"));
                assert_eq!(error.kind, "notification");
                assert_eq!(error.message, "synthetic presenter failure");
            }

            other => panic!("unexpected notification response: {other:?}"),
        }
    }

    #[test]
    fn notification_invalid_severity_returns_correlated_error() {
        let (writer, receiver) = mpsc::channel();

        handle_module_notification(
            &writer,
            "alpha",
            "session-a",
            "notification-5".to_string(),
            "alpha".to_string(),
            "session-a".to_string(),
            "purple".to_string(),
            test_notification_presentation("Alpha", "alpha.png", "Title", "Body", None),
            |_, _, _| panic!("presenter must not run for invalid severity"),
        )
        .expect("invalid severity must be reported through protocol");

        match receiver.recv().expect("notification error expected") {
            ModuleMessage::Error { id, module, error } => {
                assert_eq!(id.as_deref(), Some("notification-5"));
                assert_eq!(module.as_deref(), Some("alpha"));
                assert_eq!(error.kind, "notification");
                assert!(error.message.contains("unknown notification severity"));
            }

            other => panic!("unexpected notification response: {other:?}"),
        }
    }

    #[test]
    fn notifications_from_distinct_modules_keep_responses_isolated() {
        let (alpha_writer, alpha_receiver) = mpsc::channel();
        let (beta_writer, beta_receiver) = mpsc::channel();

        handle_module_notification(
            &alpha_writer,
            "alpha",
            "session-a",
            "alpha-notification".to_string(),
            "alpha".to_string(),
            "session-a".to_string(),
            "success".to_string(),
            test_notification_presentation("Alpha", "alpha.png", "Alpha title", "Alpha body", None),
            |module, _, presentation| {
                assert_eq!(module, "alpha");
                assert_eq!(presentation.application, "Alpha");
                Ok(NotificationOutcome::Presented(77))
            },
        )
        .expect("alpha notification must succeed");

        handle_module_notification(
            &beta_writer,
            "beta",
            "session-b",
            "beta-notification".to_string(),
            "beta".to_string(),
            "session-b".to_string(),
            "warning".to_string(),
            test_notification_presentation("Beta", "beta.png", "Beta title", "Beta body", None),
            |module, _, presentation| {
                assert_eq!(module, "beta");
                assert_eq!(presentation.application, "Beta");
                Ok(NotificationOutcome::Presented(77))
            },
        )
        .expect("beta notification must succeed");

        match alpha_receiver
            .recv()
            .expect("alpha acknowledgement expected")
        {
            ModuleMessage::NotificationAck {
                id,
                module,
                session_id,
                notification_id,
            } => {
                assert_eq!(id, "alpha-notification");
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");
                assert_eq!(notification_id, Some(77));
            }

            other => panic!("unexpected alpha response: {other:?}"),
        }

        match beta_receiver.recv().expect("beta acknowledgement expected") {
            ModuleMessage::NotificationAck {
                id,
                module,
                session_id,
                notification_id,
            } => {
                assert_eq!(id, "beta-notification");
                assert_eq!(module, "beta");
                assert_eq!(session_id, "session-b");
                assert_eq!(notification_id, Some(77));
            }

            other => panic!("unexpected beta response: {other:?}"),
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

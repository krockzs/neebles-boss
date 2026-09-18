use crate::config;
use crate::modules;
use crate::settings;
use crate::tray::manager;
use crate::tray::protocol::{TrayEvent, TrayMessage};

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

type SharedStream = Arc<Mutex<UnixStream>>;

#[derive(Debug, Clone, Copy)]
struct PeerCredentials {
    pid: u32,
    uid: u32,
}

fn peer_credentials(stream: &UnixStream) -> Result<PeerCredentials, String> {
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
            "could not read tray peer credentials: {}",
            std::io::Error::last_os_error()
        ));
    }

    if credentials.pid <= 0 {
        return Err("tray peer reported an invalid pid".to_string());
    }

    Ok(PeerCredentials {
        pid: credentials.pid as u32,
        uid: credentials.uid,
    })
}

fn assert_session_peer(peer: &PeerCredentials) -> Result<(), String> {
    let manager_uid = unsafe { libc::geteuid() };

    if peer.uid == manager_uid {
        return Ok(());
    }

    Err(format!(
        "tray connection uid {} does not match Tray Manager uid {}",
        peer.uid, manager_uid
    ))
}

fn assert_lifecycle_peer(peer: &PeerCredentials) -> Result<(), String> {
    let manager_uid = unsafe { libc::geteuid() };

    if peer.uid == manager_uid || peer.uid == 0 {
        return Ok(());
    }

    Err(format!(
        "tray lifecycle request uid {} is not authorized",
        peer.uid
    ))
}

static PROVIDERS: OnceLock<Mutex<BTreeMap<String, SharedStream>>> = OnceLock::new();

static SUBSCRIBERS: OnceLock<Mutex<Vec<SharedStream>>> = OnceLock::new();

fn providers() -> &'static Mutex<BTreeMap<String, SharedStream>> {
    PROVIDERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn subscribers() -> &'static Mutex<Vec<SharedStream>> {
    SUBSCRIBERS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn socket_path() -> PathBuf {
    crate::tray::protocol::socket_path()
}

fn manager_lock_path(socket_path: &Path) -> PathBuf {
    let mut value = socket_path.as_os_str().to_os_string();
    value.push(".lock");
    PathBuf::from(value)
}

fn acquire_manager_lock(socket_path: &Path) -> Result<File, String> {
    let path = manager_lock_path(socket_path);

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
        .map_err(|error| {
            format!(
                "could not open N.E.E.B.L.E.S. Tray Manager lock {}: {error}",
                path.display()
            )
        })?;

    let metadata = file.metadata().map_err(|error| {
        format!(
            "could not inspect N.E.E.B.L.E.S. Tray Manager lock {}: {error}",
            path.display()
        )
    })?;

    if !metadata.file_type().is_file() {
        return Err(format!(
            "N.E.E.B.L.E.S. Tray Manager lock {} is not a regular file",
            path.display()
        ));
    }

    let manager_uid = unsafe { libc::geteuid() };

    if metadata.uid() != manager_uid {
        return Err(format!(
            "N.E.E.B.L.E.S. Tray Manager lock {} belongs to uid {}, expected {}",
            path.display(),
            metadata.uid(),
            manager_uid
        ));
    }

    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| {
            format!(
                "could not secure N.E.E.B.L.E.S. Tray Manager lock {}: {error}",
                path.display()
            )
        })?;

    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };

    if result != 0 {
        let error = std::io::Error::last_os_error();

        if error.raw_os_error() == Some(libc::EWOULDBLOCK) {
            return Err(format!(
                "N.E.E.B.L.E.S. Tray Manager is already running for {}",
                socket_path.display()
            ));
        }

        return Err(format!(
            "could not lock N.E.E.B.L.E.S. Tray Manager lock {}: {error}",
            path.display()
        ));
    }

    Ok(file)
}

fn prepare_socket_path(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }

        Err(error) => {
            return Err(format!(
                "could not inspect N.E.E.B.L.E.S. tray socket {}: {error}",
                path.display()
            ));
        }
    };

    if !metadata.file_type().is_socket() {
        return Err(format!(
            "N.E.E.B.L.E.S. tray socket path {} exists but is not a Unix socket",
            path.display()
        ));
    }

    let manager_uid = unsafe { libc::geteuid() };

    if metadata.uid() != manager_uid {
        return Err(format!(
            "N.E.E.B.L.E.S. tray socket {} belongs to uid {}, expected {}",
            path.display(),
            metadata.uid(),
            manager_uid
        ));
    }

    match UnixStream::connect(path) {
        Ok(stream) => {
            drop(stream);

            Err(format!(
                "N.E.E.B.L.E.S. Tray Manager is already running on {}",
                path.display()
            ))
        }

        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
            fs::remove_file(path).map_err(|remove_error| {
                format!(
                    "could not remove stale N.E.E.B.L.E.S. tray socket {}: {remove_error}",
                    path.display()
                )
            })
        }

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),

        Err(error) => Err(format!(
            "could not determine whether N.E.E.B.L.E.S. tray socket {} is stale: {error}",
            path.display()
        )),
    }
}

pub fn serve() -> Result<(), String> {
    let path = socket_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "could not create N.E.E.B.L.E.S. tray socket directory {}: {error}",
                parent.display()
            )
        })?;
    }

    /*
     * Hold this lock for the entire Manager lifetime.
     *
     * Socket liveness probing alone is insufficient when two
     * Managers race against the same stale socket: both could
     * otherwise classify it as stale before either binds.
     */
    let _manager_lock = acquire_manager_lock(&path)?;

    prepare_socket_path(&path)?;

    let listener = UnixListener::bind(&path).map_err(|error| {
        format!(
            "could not bind N.E.E.B.L.E.S. tray socket {}: {error}",
            path.display()
        )
    })?;

    fs::set_permissions(&path, fs::Permissions::from_mode(0o660)).map_err(|error| {
        format!(
            "could not set permissions on N.E.E.B.L.E.S. tray socket {}: {error}",
            path.display()
        )
    })?;

    println!(
        "N.E.E.B.L.E.S. Tray Manager listening on {}",
        path.display()
    );

    if let Err(error) = modules::start_enabled_tray_providers() {
        /*
         * A broken module must not take the common
         * tray infrastructure down with it.
         *
         * The failure is explicit and visible.
         */
        eprintln!("N.E.E.B.L.E.S.: tray provider startup incomplete: {error}");
    }

    std::thread::spawn(run_watchdog);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(move || {
                    if let Err(error) = handle_client(stream) {
                        eprintln!("N.E.E.B.L.E.S.: tray socket client error: {error}");
                    }
                });
            }

            Err(error) => {
                eprintln!("N.E.E.B.L.E.S.: tray socket accept error: {error}");
            }
        }
    }

    Ok(())
}

fn write_message(writer: &SharedStream, message: &TrayMessage) -> Result<(), String> {
    let mut payload = serde_json::to_vec(message)
        .map_err(|error| format!("could not serialize tray message: {error}"))?;

    payload.push(b'\n');

    let mut stream = writer
        .lock()
        .map_err(|_| "tray stream lock poisoned".to_string())?;

    stream
        .write_all(&payload)
        .map_err(|error| format!("could not write tray message: {error}"))?;

    stream
        .flush()
        .map_err(|error| format!("could not flush tray message: {error}"))
}

fn ack(writer: &SharedStream, event: &str, tray_id: Option<String>) -> Result<(), String> {
    write_message(
        writer,
        &TrayMessage::Ack {
            event: event.to_string(),
            tray_id,
        },
    )
}

fn error_message(writer: &SharedStream, message: impl Into<String>) -> Result<(), String> {
    write_message(
        writer,
        &TrayMessage::Error {
            message: message.into(),
        },
    )
}

fn register_provider_stream(tray_id: &str, writer: SharedStream) -> Result<(), String> {
    providers()
        .lock()
        .map_err(|_| "tray provider registry lock poisoned".to_string())?
        .insert(tray_id.to_string(), writer);

    Ok(())
}

fn remove_provider_if_same(tray_id: &str, writer: &SharedStream) -> Result<bool, String> {
    let mut registry = providers()
        .lock()
        .map_err(|_| "tray provider registry lock poisoned".to_string())?;

    let should_remove = registry
        .get(tray_id)
        .map(|registered| Arc::ptr_eq(registered, writer))
        .unwrap_or(false);

    if should_remove {
        registry.remove(tray_id);
    }

    Ok(should_remove)
}

fn take_provider(tray_id: &str) -> Result<Option<SharedStream>, String> {
    providers()
        .lock()
        .map_err(|_| "tray provider registry lock poisoned".to_string())
        .map(|mut registry| registry.remove(tray_id))
}

fn expire_stale_providers() -> Result<(), String> {
    let manager = manager::global();

    let stale = {
        let mut guard = manager
            .lock()
            .map_err(|_| "tray manager lock poisoned".to_string())?;

        guard.remove_stale(crate::tray::protocol::TRAY_HEARTBEAT_TIMEOUT_MS)
    };

    let mut errors = Vec::new();

    for tray_id in stale {
        /*
         * Remove the provider stream first.
         *
         * The disconnected handler cannot accidentally
         * unregister a newer provider because
         * remove_provider_if_same() compares Arc identity.
         */
        if let Some(provider) = take_provider(&tray_id)? {
            if let Ok(stream) = provider.lock() {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }

        /*
         * A heartbeat timeout means the provider is no
         * longer trusted as healthy.
         *
         * Closing the socket is not enough: a hung process
         * may remain alive and its PID marker would prevent
         * reconciliation from starting a replacement.
         */
        if let Err(error) = modules::stop_tray_provider(&tray_id) {
            errors.push(format!(
                "could not stop stale tray provider '{}': {}",
                tray_id, error
            ));
        }

        broadcast_event(TrayEvent::Unregistered {
            tray_id: tray_id.clone(),
        });

        eprintln!(
            "N.E.E.B.L.E.S.: tray provider '{}' expired after heartbeat timeout",
            tray_id
        );

        /*
         * If the module is still enabled and still declares
         * tray capability, immediately reconcile that one
         * provider.
         *
         * Broken providers cannot poison the common Tray
         * Manager; a future watchdog cycle can retry again.
         */
        match config::module_enabled(&tray_id) {
            Ok(true) => {
                if let Err(error) = modules::start_tray_provider(&tray_id) {
                    errors.push(format!(
                        "could not restart tray provider '{}': {}",
                        tray_id, error
                    ));
                }
            }

            Ok(false) => {}

            Err(error) => {
                errors.push(format!(
                    "could not read enabled state for stale tray provider '{}': {}",
                    tray_id, error
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(" | "))
    }
}

fn run_watchdog() {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(
            crate::tray::protocol::TRAY_WATCHDOG_INTERVAL_MS,
        ));

        if let Err(error) = expire_stale_providers() {
            eprintln!("N.E.E.B.L.E.S.: tray watchdog error: {error}");
        }
    }
}

fn register_subscriber(writer: SharedStream) -> Result<(), String> {
    subscribers()
        .lock()
        .map_err(|_| "tray subscriber registry lock poisoned".to_string())?
        .push(writer);

    Ok(())
}

fn remove_subscriber_if_same(writer: &SharedStream) -> Result<(), String> {
    let mut registry = subscribers()
        .lock()
        .map_err(|_| "tray subscriber registry lock poisoned".to_string())?;

    registry.retain(|current| !Arc::ptr_eq(current, writer));

    Ok(())
}

fn broadcast_event(event: TrayEvent) {
    let listeners = match subscribers().lock() {
        Ok(registry) => registry.clone(),

        Err(_) => {
            eprintln!("N.E.E.B.L.E.S.: tray subscriber registry lock poisoned");
            return;
        }
    };

    let mut dead = Vec::new();

    for listener in listeners {
        let message = TrayMessage::Event {
            event: event.clone(),
        };

        if write_message(&listener, &message).is_err() {
            dead.push(listener);
        }
    }

    if dead.is_empty() {
        return;
    }

    if let Ok(mut registry) = subscribers().lock() {
        registry.retain(|current| !dead.iter().any(|item| Arc::ptr_eq(current, item)));
    }
}

fn broadcast_host_command(message: &TrayMessage) {
    let listeners = match subscribers().lock() {
        Ok(registry) => registry.clone(),

        Err(_) => {
            eprintln!("N.E.E.B.L.E.S.: tray subscriber registry lock poisoned");
            return;
        }
    };

    let mut dead = Vec::new();

    for listener in listeners {
        if write_message(&listener, message).is_err() {
            dead.push(listener);
        }
    }

    if dead.is_empty() {
        return;
    }

    if let Ok(mut registry) = subscribers().lock() {
        registry.retain(|current| !dead.iter().any(|item| Arc::ptr_eq(current, item)));
    }
}

fn forward_to_provider(tray_id: &str, message: &TrayMessage) -> Result<(), String> {
    let provider = {
        let registry = providers()
            .lock()
            .map_err(|_| "tray provider registry lock poisoned".to_string())?;

        registry.get(tray_id).cloned()
    };

    let provider = provider.ok_or_else(|| format!("tray provider '{tray_id}' is not connected"))?;

    write_message(&provider, message)
}

fn assert_provider_identity(registered_tray: &Option<String>, tray_id: &str) -> Result<(), String> {
    match registered_tray {
        Some(current) if current == tray_id => Ok(()),

        _ => Err(format!("connection does not own tray '{tray_id}'")),
    }
}

fn process_message(
    message: TrayMessage,
    writer: &SharedStream,
    peer: &PeerCredentials,
    registered_tray: &mut Option<String>,
    subscribed: &mut bool,
) -> Result<(), String> {
    match message {
        TrayMessage::Register {
            protocol,
            tray_id,
            owner_module,
            pid,
        } => {
            assert_session_peer(peer)?;

            if let Some(claimed_pid) = pid {
                if claimed_pid != peer.pid {
                    return Err(format!(
                        "tray provider claimed pid {}, but kernel reports pid {}",
                        claimed_pid, peer.pid
                    ));
                }
            }

            if *subscribed {
                return Err("tray subscriber connections cannot register providers".to_string());
            }

            modules::verify_tray_provider_process(&owner_module, peer.pid)?;

            let record = {
                let manager = manager::global();

                let mut manager = manager
                    .lock()
                    .map_err(|_| "tray manager lock poisoned".to_string())?;

                manager.register(protocol, tray_id.clone(), owner_module, Some(peer.pid))?
            };

            register_provider_stream(&tray_id, writer.clone())?;

            *registered_tray = Some(tray_id.clone());

            broadcast_event(TrayEvent::Registered { tray: record });

            ack(writer, "register", Some(tray_id))
        }

        TrayMessage::Heartbeat { tray_id } => {
            assert_session_peer(peer)?;
            assert_provider_identity(registered_tray, &tray_id)?;

            let manager = manager::global();

            manager
                .lock()
                .map_err(|_| "tray manager lock poisoned".to_string())?
                .heartbeat(&tray_id)?;

            ack(writer, "heartbeat", Some(tray_id))
        }

        TrayMessage::State {
            tray_id,
            opened,
            width,
            height,
            state,
        } => {
            assert_session_peer(peer)?;
            assert_provider_identity(registered_tray, &tray_id)?;

            let record = {
                let manager = manager::global();

                let mut guard = manager
                    .lock()
                    .map_err(|_| "tray manager lock poisoned".to_string())?;

                guard.update_state(&tray_id, opened, width, height, state)?
            };

            broadcast_event(TrayEvent::Updated { tray: record });

            ack(writer, "state", Some(tray_id))
        }

        TrayMessage::Unregister { tray_id } => {
            assert_session_peer(peer)?;
            assert_provider_identity(registered_tray, &tray_id)?;

            remove_provider_if_same(&tray_id, writer)?;

            let manager = manager::global();

            manager
                .lock()
                .map_err(|_| "tray manager lock poisoned".to_string())?
                .unregister(&tray_id)?;

            *registered_tray = None;

            broadcast_event(TrayEvent::Unregistered {
                tray_id: tray_id.clone(),
            });

            ack(writer, "unregister", Some(tray_id))
        }

        TrayMessage::SettingsGet { owner_module, path } => {
            assert_session_peer(peer)?;

            if *subscribed {
                return Err("tray subscriber connections cannot read module settings".to_string());
            }

            if let Some(registered) = registered_tray.as_deref() {
                if registered != owner_module {
                    return Err(format!(
                        "registered tray '{}' cannot read settings for module '{}'",
                        registered, owner_module
                    ));
                }
            }

            /*
             * Identity comes from the kernel PID, never from
             * owner_module alone.
             *
             * verify_tray_provider_process() proves that this
             * exact process is the provider declared by the
             * installed module.
             */
            modules::verify_tray_provider_process(&owner_module, peer.pid)?;

            let default = modules::installed_module_settings_default(&owner_module)?;

            let settings_path =
                settings::module_settings_path(&modules::neebles_root(), &owner_module);

            let local = settings::load_or_create(&settings_path, &default)?;

            let value = settings::get_effective_path(&local, &default, &path)?;

            write_message(
                writer,
                &TrayMessage::SettingsValue {
                    owner_module,
                    path,
                    value,
                },
            )
        }

        TrayMessage::SettingsSet {
            owner_module,
            path,
            value,
        } => {
            assert_session_peer(peer)?;

            if *subscribed {
                return Err("tray subscriber connections cannot write module settings".to_string());
            }

            if let Some(registered) = registered_tray.as_deref() {
                if registered != owner_module {
                    return Err(format!(
                        "registered tray '{}' cannot write settings for module '{}'",
                        registered, owner_module
                    ));
                }
            }

            /*
             * The caller cannot impersonate another module:
             * Boss verifies the real kernel PID against that
             * module's installed tray provider.
             */
            modules::verify_tray_provider_process(&owner_module, peer.pid)?;

            let default = modules::installed_module_settings_default(&owner_module)?;

            let settings_path =
                settings::module_settings_path(&modules::neebles_root(), &owner_module);

            let value = settings::set_path(&settings_path, &default, &path, value)?;

            write_message(
                writer,
                &TrayMessage::SettingsValue {
                    owner_module,
                    path,
                    value,
                },
            )
        }

        TrayMessage::SetVisibility { tray_id, visible } => {
            assert_session_peer(peer)?;

            if registered_tray.is_some() {
                return Err("tray providers cannot change tray visibility".to_string());
            }

            if *subscribed {
                return Err("tray subscriber connections cannot change tray visibility".to_string());
            }

            modules::resolved_tray_contract(&tray_id)?;

            config::set_module_visibility("tray", &tray_id, visible)?;

            let updated = {
                let manager = manager::global();

                let mut guard = manager
                    .lock()
                    .map_err(|_| "tray manager lock poisoned".to_string())?;

                guard.set_visibility(&tray_id, visible)
            };

            if let Some(record) = updated {
                broadcast_event(TrayEvent::Updated { tray: record });
            }

            ack(writer, "set_visibility", Some(tray_id))
        }

        TrayMessage::StopProvider { tray_id } => {
            assert_lifecycle_peer(peer)?;

            if registered_tray.is_some() {
                return Err("tray providers cannot stop providers".to_string());
            }

            if *subscribed {
                return Err("tray subscriber connections cannot stop providers".to_string());
            }

            modules::resolved_tray_contract(&tray_id)?;

            modules::stop_tray_provider(&tray_id)?;

            ack(writer, "stop_provider", Some(tray_id))
        }

        TrayMessage::Reconcile => {
            assert_lifecycle_peer(peer)?;

            if registered_tray.is_some() {
                return Err("tray providers cannot reconcile tray lifecycle".to_string());
            }

            if *subscribed {
                return Err(
                    "tray subscriber connections cannot reconcile tray lifecycle".to_string(),
                );
            }

            modules::start_enabled_tray_providers()?;

            ack(writer, "reconcile", None)
        }

        TrayMessage::Subscribe => {
            assert_session_peer(peer)?;

            if registered_tray.is_some() {
                return Err("tray provider connections cannot subscribe as hosts".to_string());
            }

            if *subscribed {
                return Err("tray connection is already subscribed".to_string());
            }

            register_subscriber(writer.clone())?;

            *subscribed = true;

            let manager = manager::global();

            let trays = manager
                .lock()
                .map_err(|_| "tray manager lock poisoned".to_string())?
                .list();

            write_message(writer, &TrayMessage::Snapshot { trays })
        }

        command @ TrayMessage::HostShow
        | command @ TrayMessage::HostHide
        | command @ TrayMessage::HostToggle => {
            assert_session_peer(peer)?;

            if registered_tray.is_some() {
                return Err("tray providers cannot control the root tray surface".to_string());
            }

            if *subscribed {
                return Err(
                    "tray subscriber connections cannot control the root tray surface".to_string(),
                );
            }

            broadcast_host_command(&command);

            let event = match command {
                TrayMessage::HostShow => "host_show",
                TrayMessage::HostHide => "host_hide",
                TrayMessage::HostToggle => "host_toggle",

                _ => unreachable!(),
            };

            ack(writer, event, None)
        }

        TrayMessage::List => {
            assert_session_peer(peer)?;

            let manager = manager::global();

            let trays = manager
                .lock()
                .map_err(|_| "tray manager lock poisoned".to_string())?
                .list();

            write_message(writer, &TrayMessage::Snapshot { trays })
        }

        TrayMessage::Get { tray_id } => {
            assert_session_peer(peer)?;

            let manager = manager::global();

            let tray = manager
                .lock()
                .map_err(|_| "tray manager lock poisoned".to_string())?
                .get(&tray_id);

            match tray {
                Some(tray) => write_message(writer, &TrayMessage::Record { tray }),

                None => error_message(writer, format!("tray '{tray_id}' is not registered")),
            }
        }

        command @ TrayMessage::Open { .. }
        | command @ TrayMessage::Close { .. }
        | command @ TrayMessage::Focus { .. }
        | command @ TrayMessage::Reload { .. }
        | command @ TrayMessage::Resize { .. } => {
            assert_session_peer(peer)?;

            let tray_id = match &command {
                TrayMessage::Open { tray_id }
                | TrayMessage::Close { tray_id }
                | TrayMessage::Focus { tray_id }
                | TrayMessage::Reload { tray_id }
                | TrayMessage::Resize { tray_id, .. } => tray_id.clone(),

                _ => unreachable!(),
            };

            forward_to_provider(&tray_id, &command)?;

            ack(writer, "forward", Some(tray_id))
        }

        TrayMessage::Ack { .. }
        | TrayMessage::Snapshot { .. }
        | TrayMessage::Event { .. }
        | TrayMessage::Record { .. }
        | TrayMessage::SettingsValue { .. }
        | TrayMessage::Error { .. } => {
            Err("response-only tray message received by server".to_string())
        }
    }
}

fn handle_client(stream: UnixStream) -> Result<(), String> {
    let reader_stream = stream
        .try_clone()
        .map_err(|error| format!("could not clone tray socket: {error}"))?;

    let peer = peer_credentials(&stream)?;

    let writer = Arc::new(Mutex::new(stream));

    let mut reader = BufReader::new(reader_stream);

    let mut registered_tray: Option<String> = None;
    let mut subscribed = false;

    loop {
        let mut line = String::new();

        let read = reader
            .read_line(&mut line)
            .map_err(|error| format!("could not read tray socket: {error}"))?;

        if read == 0 {
            break;
        }

        if line.trim().is_empty() {
            continue;
        }

        let message: TrayMessage = match serde_json::from_str(line.trim()) {
            Ok(message) => message,

            Err(error) => {
                error_message(&writer, format!("invalid tray message: {error}"))?;

                continue;
            }
        };

        if let Err(error) = process_message(
            message,
            &writer,
            &peer,
            &mut registered_tray,
            &mut subscribed,
        ) {
            error_message(&writer, error)?;
        }
    }

    if subscribed {
        if let Err(error) = remove_subscriber_if_same(&writer) {
            eprintln!("N.E.E.B.L.E.S.: could not remove tray subscriber: {error}");
        }
    }

    if let Some(tray_id) = registered_tray {
        if remove_provider_if_same(&tray_id, &writer)? {
            let manager = manager::global();

            let removed = match manager.lock() {
                Ok(mut guard) => guard.unregister(&tray_id).is_ok(),

                Err(_) => {
                    eprintln!(
                        "N.E.E.B.L.E.S.: tray manager lock poisoned while disconnecting '{}'",
                        tray_id
                    );

                    false
                }
            };

            if removed {
                broadcast_event(TrayEvent::Unregistered { tray_id });
            }
        }
    }

    Ok(())
}

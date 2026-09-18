use crate::config;
use crate::dispatcher;
use crate::request::{BossStreamMessage, ExecutionRequest, ExecutionResponse};
use std::env;
use std::ffi::CString;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::Shutdown;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, OnceLock,
};

const DEFAULT_SOCKET_PATH: &str = "/run/neebles/neebles.sock";

#[derive(Clone)]
struct BossSubscriber {
    id: u64,
    topics: Vec<String>,
    writer: Arc<Mutex<UnixStream>>,
}

static BOSS_SUBSCRIBERS: OnceLock<Mutex<Vec<BossSubscriber>>> = OnceLock::new();

static NEXT_SUBSCRIBER_ID: AtomicU64 = AtomicU64::new(1);

fn subscribers() -> &'static Mutex<Vec<BossSubscriber>> {
    BOSS_SUBSCRIBERS.get_or_init(|| Mutex::new(Vec::new()))
}

fn topic_matches(topics: &[String], topic: &str) -> bool {
    topics
        .iter()
        .any(|candidate| candidate == "*" || candidate == topic)
}

pub fn broadcast_event(
    topic: impl Into<String>,
    event: impl Into<String>,
    payload: serde_json::Value,
) {
    let topic = topic.into();

    let message = BossStreamMessage::Event {
        topic: topic.clone(),
        event: event.into(),
        payload,
    };

    let mut payload = match serde_json::to_vec(&message) {
        Ok(payload) => payload,
        Err(error) => {
            eprintln!("N.E.E.B.L.E.S.: could not serialize Boss event: {error}");
            return;
        }
    };

    payload.push(b'\n');

    let current = match subscribers().lock() {
        Ok(guard) => guard.clone(),
        Err(_) => {
            eprintln!("N.E.E.B.L.E.S.: Boss subscriber registry lock poisoned");
            return;
        }
    };

    let mut dead = Vec::new();

    for subscriber in current {
        if !topic_matches(&subscriber.topics, &topic) {
            continue;
        }

        let write_ok = subscriber
            .writer
            .lock()
            .map(|mut writer| writer.write_all(&payload).is_ok() && writer.flush().is_ok())
            .unwrap_or(false);

        if !write_ok {
            dead.push(subscriber.id);
        }
    }

    if dead.is_empty() {
        return;
    }

    if let Ok(mut guard) = subscribers().lock() {
        guard.retain(|subscriber| !dead.contains(&subscriber.id));
    }
}

pub(crate) fn secure_runtime_socket(path: &Path) -> Result<(), String> {
    let (uid, gid) = crate::runtime_identity::desktop_identity()?;

    let raw_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        format!(
            "socket path contains an invalid NUL byte: {}",
            path.display()
        )
    })?;

    let result = unsafe { libc::chown(raw_path.as_ptr(), uid, gid) };

    if result != 0 {
        return Err(format!(
            "could not assign N.E.E.B.L.E.S. socket {} to desktop user {uid}:{gid}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        format!(
            "could not secure N.E.E.B.L.E.S. socket {}: {error}",
            path.display()
        )
    })
}

pub fn socket_path() -> PathBuf {
    env::var("NEEBLES_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_SOCKET_PATH))
}

/*
 * Send one request to the persistent Boss process.
 *
 * neebles.sock keeps its existing one-request-per-connection
 * protocol:
 *
 * client writes JSON
 * client closes write side
 * Boss reads to EOF
 * Boss dispatches
 * Boss returns ExecutionResponse JSON
 */
pub fn request(request: &ExecutionRequest) -> Result<ExecutionResponse, String> {
    let path = socket_path();

    let mut stream = UnixStream::connect(&path).map_err(|error| {
        format!(
            "could not connect to N.E.E.B.L.E.S. Boss socket {}: {error}",
            path.display()
        )
    })?;

    let payload = serde_json::to_vec(request)
        .map_err(|error| format!("could not serialize ExecutionRequest: {error}"))?;

    stream
        .write_all(&payload)
        .map_err(|error| format!("could not write ExecutionRequest to Boss socket: {error}"))?;

    /*
     * Server uses read_to_string(), therefore EOF on the
     * request side is the framing boundary.
     */
    stream
        .shutdown(Shutdown::Write)
        .map_err(|error| format!("could not finish ExecutionRequest write side: {error}"))?;

    let mut raw = String::new();

    stream
        .read_to_string(&mut raw)
        .map_err(|error| format!("could not read ExecutionResponse from Boss socket: {error}"))?;

    serde_json::from_str(raw.trim())
        .map_err(|error| format!("invalid ExecutionResponse received from Boss socket: {error}"))
}

pub fn serve() -> Result<(), String> {
    /*
     * Boss local settings must exist and be valid before
     * any runtime surface becomes available.
     */
    let _ = config::load_or_initialize()?;

    /*
     * The Boss owns both IPC surfaces.
     *
     * neebles.sock:
     * external commands -> Boss
     *
     * modules.sock:
     * persistent module runtimes <-> Boss
     *
     * Both live inside this same process so runtime registries,
     * pending requests and module state are truly shared.
     */
    let _module_ipc = crate::module_ipc::start_background()
        .map_err(|error| format!("could not start module IPC: {error}"))?;

    /*
     * Boss UI presence is reported by a user-session lease through
     * neebles.sock. The system runtime must never depend on Session D-Bus
     * in order to expose its administrative IPC surface.
     */
    let path = socket_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "could not create N.E.E.B.L.E.S. socket directory {}: {error}",
                parent.display()
            )
        })?;
    }

    if path.exists() {
        fs::remove_file(&path).map_err(|error| {
            format!(
                "could not remove stale N.E.E.B.L.E.S. socket {}: {error}",
                path.display()
            )
        })?;
    }

    let listener = UnixListener::bind(&path).map_err(|error| {
        format!(
            "could not bind N.E.E.B.L.E.S. Unix socket {}: {error}",
            path.display()
        )
    })?;

    /*
     * neebles.sock is the administrative Boss dispatcher.
     *
     * Module processes must use their governed IPC surface,
     * never this generic administrative socket.
     */
    secure_runtime_socket(&path)?;

    println!("N.E.E.B.L.E.S. Boss listening on {}", path.display());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(move || {
                    if let Err(error) = handle_client(stream) {
                        eprintln!("N.E.E.B.L.E.S.: socket client error: {error}");
                    }
                });
            }
            Err(error) => {
                eprintln!("N.E.E.B.L.E.S.: Unix socket accept error: {error}");
            }
        }
    }

    Ok(())
}

fn handle_client(mut stream: UnixStream) -> Result<(), String> {
    /*
     * Backward-compatible framing:
     *
     * Existing clients:
     *   JSON -> shutdown(Write)
     *   read_line() completes at EOF.
     *
     * Stream subscribers:
     *   JSON + newline
     *   connection remains open.
     */
    let reader_stream = stream
        .try_clone()
        .map_err(|error| format!("could not clone N.E.E.B.L.E.S. Boss socket: {error}"))?;

    let mut reader = BufReader::new(reader_stream);

    let mut raw = String::new();

    let received = reader
        .read_line(&mut raw)
        .map_err(|error| format!("could not read ExecutionRequest from Unix socket: {error}"))?;

    if received == 0 {
        return Err("empty ExecutionRequest received through Unix socket".to_string());
    }

    let request: ExecutionRequest = serde_json::from_str(raw.trim()).map_err(|error| {
        format!("invalid ExecutionRequest received through Unix socket: {error}")
    })?;

    if request.target == "events" {
        match request.action.as_deref() {
            Some("subscribe") => {
                return handle_subscription(stream, reader, request);
            }

            Some("lease") => {
                return handle_surface_lease(stream, reader, request);
            }

            _ => {}
        }
    }

    let response = dispatcher::dispatch(request);

    let payload = serde_json::to_vec(&response).map_err(|error| {
        format!("could not serialize ExecutionResponse for Unix socket: {error}")
    })?;

    stream
        .write_all(&payload)
        .map_err(|error| format!("could not write ExecutionResponse to Unix socket: {error}"))?;

    Ok(())
}

fn handle_surface_lease(
    mut stream: UnixStream,
    mut reader: BufReader<UnixStream>,
    request: ExecutionRequest,
) -> Result<(), String> {
    if request.args.len() != 1 || request.args[0].trim() != "boss-ui" {
        return Err("events lease requires exactly one supported surface: boss-ui".to_string());
    }

    /*
     * caller is semantic metadata, not authentication.
     * neebles.sock itself is restricted to the desktop identity.
     */
    if request.context.caller.trim() != "boss-ui" {
        return Err("boss-ui lease requires caller boss-ui".to_string());
    }

    crate::surface_state::acquire_boss_ui_lease()?;

    let acknowledged = BossStreamMessage::Subscribed {
        topics: vec!["boss-ui".to_string()],
    };

    let mut payload = serde_json::to_vec(&acknowledged)
        .map_err(|error| format!("could not serialize Boss UI lease response: {error}"))?;

    payload.push(b'\n');

    if let Err(error) = stream.write_all(&payload) {
        let _ = crate::surface_state::release_boss_ui_lease();

        return Err(format!("could not acknowledge Boss UI lease: {error}"));
    }

    if let Err(error) = stream.flush() {
        let _ = crate::surface_state::release_boss_ui_lease();

        return Err(format!("could not flush Boss UI lease response: {error}"));
    }

    /*
     * The connection itself is the lease.
     *
     * Normal close, crash, SIGKILL or session loss all eventually produce
     * EOF/error here, so Boss cannot remain permanently stuck in Open.
     */
    loop {
        let mut line = String::new();

        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }

    crate::surface_state::release_boss_ui_lease()?;

    Ok(())
}

fn handle_subscription(
    mut stream: UnixStream,
    mut reader: BufReader<UnixStream>,
    request: ExecutionRequest,
) -> Result<(), String> {
    let mut topics = request.args;

    topics.retain(|topic| !topic.trim().is_empty());

    topics.sort();
    topics.dedup();

    if topics.is_empty() {
        return Err("events subscribe requires at least one topic".to_string());
    }

    let id = NEXT_SUBSCRIBER_ID.fetch_add(1, Ordering::Relaxed);

    let writer = Arc::new(Mutex::new(stream.try_clone().map_err(|error| {
        format!("could not clone Boss subscriber socket: {error}")
    })?));

    {
        let mut guard = subscribers()
            .lock()
            .map_err(|_| "Boss subscriber registry lock poisoned".to_string())?;

        guard.push(BossSubscriber {
            id,
            topics: topics.clone(),
            writer,
        });
    }

    let mut subscribed = serde_json::to_vec(&BossStreamMessage::Subscribed {
        topics: topics.clone(),
    })
    .map_err(|error| format!("could not serialize Boss subscription response: {error}"))?;

    subscribed.push(b'\n');

    if let Err(error) = stream.write_all(&subscribed) {
        if let Ok(mut guard) = subscribers().lock() {
            guard.retain(|subscriber| subscriber.id != id);
        }

        return Err(format!("could not acknowledge Boss subscription: {error}"));
    }

    stream
        .flush()
        .map_err(|error| format!("could not flush Boss subscription response: {error}"))?;

    if topic_matches(&topics, "boss-ui") {
        let snapshot = BossStreamMessage::Event {
            topic: "boss-ui".to_string(),
            event: "state_snapshot".to_string(),
            payload: serde_json::json!({
                "state":
                    crate::surface_state::boss_ui_state()
                        .as_str()
            }),
        };

        let mut snapshot_payload = serde_json::to_vec(&snapshot)
            .map_err(|error| format!("could not serialize Boss UI state snapshot: {error}"))?;

        snapshot_payload.push(b'\n');

        stream
            .write_all(&snapshot_payload)
            .map_err(|error| format!("could not write Boss UI state snapshot: {error}"))?;

        stream
            .flush()
            .map_err(|error| format!("could not flush Boss UI state snapshot: {error}"))?;
    }

    if topic_matches(&topics, "settings.boss") {
        let config = config::load_or_initialize()?;

        let snapshot = BossStreamMessage::Event {
            topic: "settings.boss".to_string(),
            event: "state_snapshot".to_string(),
            payload: serde_json::to_value(config)
                .map_err(|error| format!("could not serialize Boss settings snapshot: {error}"))?,
        };

        let mut snapshot_payload = serde_json::to_vec(&snapshot).map_err(|error| {
            format!("could not serialize Boss settings stream snapshot: {error}")
        })?;

        snapshot_payload.push(b'\n');

        stream
            .write_all(&snapshot_payload)
            .map_err(|error| format!("could not write Boss settings snapshot: {error}"))?;

        stream
            .flush()
            .map_err(|error| format!("could not flush Boss settings snapshot: {error}"))?;
    }

    /*
     * Keep this worker alive until the subscriber disconnects.
     *
     * Future protocol extensions may accept ping/control frames
     * here. For now, additional input is ignored.
     */
    loop {
        let mut line = String::new();

        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }

    if let Ok(mut guard) = subscribers().lock() {
        guard.retain(|subscriber| subscriber.id != id);
    }

    Ok(())
}

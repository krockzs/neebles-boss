/*
 * CAST30-B5: a desktop-UID presentation boundary.
 * Boss owns policy and module/session ownership. This process only uses D-Bus.
 */
use crate::notifications::{
    self, NotificationAction, NotificationOptions, NotificationReply, NotificationReturnEvent,
    Severity,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};

const MAX_FRAME: usize = 64 * 1024 * 1024;
const SIGNAL_BUFFER_LIMIT: usize = 128;
const SERVICE: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Presentation {
    pub severity: Severity,
    pub application: String,
    pub icon: String,
    pub options: NotificationOptions,
    pub title: String,
    pub message: String,
    pub actions: Vec<NotificationAction>,
    pub reply: Option<NotificationReply>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Wire {
    Present { presentation: Presentation },
    Presented { notification_id: u32 },
    Failure { error: String },
    Activate { notification_id: u32 },
    Return { event: NotificationReturnEvent },
}

fn write_wire(stream: &mut UnixStream, message: &Wire) -> Result<(), String> {
    let data =
        serde_json::to_vec(message).map_err(|e| format!("notification wire encode failed: {e}"))?;
    if data.is_empty() || data.len() > MAX_FRAME {
        return Err("notification wire frame exceeds limit".to_string());
    }
    let len = u32::try_from(data.len()).map_err(|e| e.to_string())?;
    stream
        .write_all(&len.to_be_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(&data).map_err(|e| e.to_string())
}

fn read_wire(stream: &mut UnixStream) -> Result<Option<Wire>, String> {
    let mut size = [0u8; 4];
    let first = stream.read(&mut size[..1]).map_err(|e| e.to_string())?;
    if first == 0 {
        return Ok(None);
    }
    stream
        .read_exact(&mut size[1..])
        .map_err(|e| e.to_string())?;
    let len = u32::from_be_bytes(size) as usize;
    if len == 0 || len > MAX_FRAME {
        return Err(format!("notification wire frame invalid size: {len}"));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).map_err(|e| e.to_string())?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| format!("notification wire decode failed: {e}"))
}

fn socket_path(uid: u32) -> PathBuf {
    PathBuf::from(format!("/run/user/{uid}/neebles/notifications.sock"))
}

fn peer_uid(stream: &UnixStream) -> Result<u32, String> {
    let mut creds: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut creds as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if result != 0 || len as usize != std::mem::size_of::<libc::ucred>() || creds.pid <= 0 {
        return Err("notification presenter cannot validate kernel peer credentials".to_string());
    }
    Ok(creds.uid)
}

fn require_boss_root(peer_uid: u32) -> Result<(), String> {
    if peer_uid != 0 {
        return Err(format!(
            "notification presenter rejected peer UID {peer_uid}; Boss root required"
        ));
    }
    Ok(())
}

struct ReturnEndpoint {
    writer: Arc<Mutex<UnixStream>>,
    active: bool,
    staged: Vec<NotificationReturnEvent>,
}

#[derive(Default)]
struct PresenterState {
    endpoints: HashMap<u32, ReturnEndpoint>,
    unmatched: VecDeque<NotificationReturnEvent>,
}

fn send_to_endpoint(
    endpoint: &ReturnEndpoint,
    event: NotificationReturnEvent,
) -> Result<(), String> {
    let mut writer = endpoint
        .writer
        .lock()
        .map_err(|_| "presenter writer lock poisoned")?;
    write_wire(&mut writer, &Wire::Return { event })
}

fn forward_signal(state: &Mutex<PresenterState>, event: NotificationReturnEvent) {
    let id = event.notification_id();
    let mut state = match state.lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    if let Some(endpoint) = state.endpoints.get_mut(&id) {
        if endpoint.active {
            if let Err(error) = send_to_endpoint(endpoint, event.clone()) {
                eprintln!("N.E.E.B.L.E.S. notification signal return unavailable: {error}");
            }
        } else if endpoint.staged.len() < SIGNAL_BUFFER_LIMIT {
            endpoint.staged.push(event.clone());
        }
        if event.is_terminal() && endpoint.active {
            if let Some(old) = state.endpoints.remove(&id) {
                if let Ok(stream) = old.writer.lock() {
                    let _ = stream.shutdown(Shutdown::Both);
                }
            }
        }
    } else {
        if state.unmatched.len() >= SIGNAL_BUFFER_LIMIT {
            state.unmatched.pop_front();
        }
        state.unmatched.push_back(event);
    }
}

fn listen_session_signals(
    state: Arc<Mutex<PresenterState>>,
    ready: mpsc::Sender<Result<(), String>>,
) {
    let result = (|| -> Result<(), String> {
        let conn =
            Connection::session().map_err(|e| format!("user Notifications D-Bus failed: {e}"))?;
        let proxy = Proxy::new(&conn, SERVICE, PATH, SERVICE)
            .map_err(|e| format!("user Notifications proxy failed: {e}"))?;
        let mut signals = proxy
            .receive_all_signals()
            .map_err(|e| format!("user Notifications signals failed: {e}"))?;
        let _ = ready.send(Ok(()));
        for message in &mut signals {
            match notifications::notification_return_event_from_message(&message) {
                Ok(Some(event)) => forward_signal(&state, event),
                Ok(None) => {}
                Err(error) => eprintln!("N.E.E.B.L.E.S. notification signal: {error}"),
            }
        }
        Err("user Notifications return signal stream ended".to_string())
    })();
    if let Err(error) = result {
        let _ = ready.send(Err(error.clone()));
        eprintln!("N.E.E.B.L.E.S. notification presenter: {error}");
        // The user service must restart rather than remain alive without return delivery.
        std::process::exit(1);
    }
}

fn accept_client(mut stream: UnixStream, state: Arc<Mutex<PresenterState>>) -> Result<(), String> {
    require_boss_root(peer_uid(&stream)?)?;
    let writer = Arc::new(Mutex::new(stream.try_clone().map_err(|e| e.to_string())?));
    loop {
        let Some(message) = read_wire(&mut stream)? else {
            break;
        };
        match message {
            Wire::Present { presentation } => {
                let result = notifications::physical_notify_for_presenter(&presentation);
                match result {
                    Ok(id) => {
                        let mut guard = state.lock().map_err(|_| "presenter registry poisoned")?;
                        let mut staged = Vec::new();
                        guard.unmatched.retain(|event| {
                            if event.notification_id() == id {
                                staged.push(event.clone());
                                false
                            } else {
                                true
                            }
                        });
                        if let Some(old) = guard.endpoints.insert(
                            id,
                            ReturnEndpoint {
                                writer: Arc::clone(&writer),
                                active: false,
                                staged,
                            },
                        ) {
                            if let Ok(previous) = old.writer.lock() {
                                let _ = previous.shutdown(Shutdown::Both);
                            }
                        }
                        drop(guard);
                        let mut guard = writer.lock().map_err(|_| "presenter writer poisoned")?;
                        write_wire(
                            &mut guard,
                            &Wire::Presented {
                                notification_id: id,
                            },
                        )?;
                    }
                    Err(error) => {
                        let mut guard = writer.lock().map_err(|_| "presenter writer poisoned")?;
                        write_wire(&mut guard, &Wire::Failure { error })?;
                    }
                }
            }
            Wire::Activate { notification_id } => {
                let mut guard = state.lock().map_err(|_| "presenter registry poisoned")?;
                if let Some(endpoint) = guard.endpoints.get_mut(&notification_id) {
                    if Arc::ptr_eq(&endpoint.writer, &writer) {
                        endpoint.active = true;
                        let staged = std::mem::take(&mut endpoint.staged);
                        let terminal = staged.iter().any(NotificationReturnEvent::is_terminal);
                        for event in staged {
                            send_to_endpoint(endpoint, event)?;
                        }
                        if terminal {
                            if let Some(old) = guard.endpoints.remove(&notification_id) {
                                if let Ok(s) = old.writer.lock() {
                                    let _ = s.shutdown(Shutdown::Both);
                                }
                            }
                        }
                    }
                }
            }
            _ => return Err("notification presenter rejected response frame from Boss".to_string()),
        }
    }
    let mut guard = state.lock().map_err(|_| "presenter registry poisoned")?;
    guard
        .endpoints
        .retain(|_, endpoint| !Arc::ptr_eq(&endpoint.writer, &writer));
    Ok(())
}

pub(crate) fn serve() -> Result<(), String> {
    let uid = unsafe { libc::geteuid() };
    if uid == 0 {
        return Err("notification presenter must run as desktop UID, never root".to_string());
    }
    let runtime_dir = PathBuf::from(format!("/run/user/{uid}"));
    let parent = runtime_dir.join("neebles");
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let path = socket_path(uid);
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if !metadata.file_type().is_socket() {
            return Err(format!(
                "refusing to replace non-socket presenter path {}",
                path.display()
            ));
        }
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    let state = Arc::new(Mutex::new(PresenterState::default()));
    let (ready_tx, ready_rx) = mpsc::channel();
    let state_for_signals = Arc::clone(&state);
    thread::Builder::new()
        .name("neebles-notification-dbus".to_string())
        .spawn(move || listen_session_signals(state_for_signals, ready_tx))
        .map_err(|e| e.to_string())?;
    ready_rx
        .recv_timeout(Duration::from_secs(20))
        .map_err(|e| format!("notification D-Bus readiness timeout: {e}"))??;
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let state = Arc::clone(&state);
                thread::spawn(move || {
                    if let Err(error) = accept_client(stream, state) {
                        eprintln!("N.E.E.B.L.E.S. presenter client: {error}");
                    }
                });
            }
            Err(error) => eprintln!("N.E.E.B.L.E.S. presenter accept: {error}"),
        }
    }
    Err("notification presenter accept loop terminated".to_string())
}

static ROOT_PENDING: OnceLock<Mutex<HashMap<u32, UnixStream>>> = OnceLock::new();

fn pending() -> &'static Mutex<HashMap<u32, UnixStream>> {
    ROOT_PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn present(presentation: Presentation) -> Result<u32, String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("notification presentation must originate in persistent Boss root".to_string());
    }
    let (uid, _) = crate::runtime_identity::desktop_identity()?;
    let path = socket_path(uid);
    let mut stream = UnixStream::connect(&path).map_err(|e| {
        format!(
            "notification user presenter unavailable at {}: {e}",
            path.display()
        )
    })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(20)))
        .map_err(|e| e.to_string())?;
    write_wire(&mut stream, &Wire::Present { presentation })?;
    let id = match read_wire(&mut stream)? {
        Some(Wire::Presented { notification_id }) if notification_id != 0 => notification_id,
        Some(Wire::Failure { error }) => return Err(error),
        _ => return Err("notification presenter returned an invalid acknowledgement".to_string()),
    };
    let previous = pending()
        .lock()
        .map_err(|_| "notification pending registry poisoned")?
        .insert(id, stream);
    if let Some(old) = previous {
        let _ = old.shutdown(Shutdown::Both);
    }
    Ok(id)
}

pub(crate) fn activate(id: u32) -> Result<(), String> {
    let stream = pending()
        .lock()
        .map_err(|_| "notification pending registry poisoned")?
        .remove(&id);
    let Some(mut stream) = stream else {
        return Err(format!(
            "notification {id} has no pending presenter return channel"
        ));
    };
    write_wire(
        &mut stream,
        &Wire::Activate {
            notification_id: id,
        },
    )?;
    stream.set_read_timeout(None).map_err(|e| e.to_string())?;
    thread::Builder::new()
        .name(format!("neebles-notification-{id}"))
        .spawn(move || loop {
            match read_wire(&mut stream) {
                Ok(Some(Wire::Return { event })) if event.notification_id() == id => {
                    let terminal = event.is_terminal();
                    if let Err(error) =
                        crate::module_ipc::server::route_notification_return_event(event)
                    {
                        eprintln!("N.E.E.B.L.E.S. notification owner return: {error}");
                    }
                    if terminal {
                        break;
                    }
                }
                Ok(None) => break,
                Ok(_) => {
                    eprintln!("N.E.E.B.L.E.S. presenter return identity mismatch");
                    break;
                }
                Err(error) => {
                    eprintln!("N.E.E.B.L.E.S. presenter return disconnected: {error}");
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn discard_unowned(id: u32) {
    if let Ok(mut guard) = pending().lock() {
        if let Some(stream) = guard.remove(&id) {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

#[cfg(test)]
mod certification_tests {
    use super::*;
    #[test]
    fn presenter_peer_is_kernel_root_only() {
        require_boss_root(0).unwrap();
        assert!(require_boss_root(1000).is_err());
    }
    #[test]
    fn wire_round_trip_preserves_notification_identity() {
        let (mut a, mut b) = UnixStream::pair().unwrap();
        write_wire(
            &mut a,
            &Wire::Return {
                event: NotificationReturnEvent::ActionInvoked {
                    notification_id: 92,
                    action_key: "open".to_string(),
                },
            },
        )
        .unwrap();
        match read_wire(&mut b).unwrap().unwrap() {
            Wire::Return {
                event:
                    NotificationReturnEvent::ActionInvoked {
                        notification_id,
                        action_key,
                    },
            } => {
                assert_eq!(notification_id, 92);
                assert_eq!(action_key, "open");
            }
            other => panic!("unexpected presenter wire {other:?}"),
        }
    }
    #[test]
    fn oversized_frame_is_rejected_before_allocation() {
        let (mut a, mut b) = UnixStream::pair().unwrap();
        a.write_all(&(MAX_FRAME as u32 + 1).to_be_bytes()).unwrap();
        assert!(read_wire(&mut b).unwrap_err().contains("invalid size"));
    }
}

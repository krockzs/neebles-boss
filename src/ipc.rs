use crate::config;
use crate::dispatcher;
use crate::request::{ExecutionRequest, ExecutionResponse};
use std::env;
use std::ffi::CString;
use std::fs;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

const DEFAULT_SOCKET_PATH: &str = "/run/neebles/neebles.sock";

fn runtime_desktop_identity() -> Result<(libc::uid_t, libc::gid_t), String> {
    let uid = env::var("NEEBLES_DESKTOP_UID");
    let gid = env::var("NEEBLES_DESKTOP_GID");

    match (uid, gid) {
        (Ok(uid), Ok(gid)) => {
            let uid = uid
                .parse::<u32>()
                .map_err(|error| format!("invalid NEEBLES_DESKTOP_UID: {error}"))?;

            let gid = gid
                .parse::<u32>()
                .map_err(|error| format!("invalid NEEBLES_DESKTOP_GID: {error}"))?;

            Ok((uid as libc::uid_t, gid as libc::gid_t))
        }

        (Err(env::VarError::NotPresent), Err(env::VarError::NotPresent)) => {
            Ok(unsafe { (libc::geteuid(), libc::getegid()) })
        }

        _ => Err(
            "NEEBLES_DESKTOP_UID and NEEBLES_DESKTOP_GID must be configured together".to_string(),
        ),
    }
}

pub(crate) fn secure_runtime_socket(path: &Path) -> Result<(), String> {
    let (uid, gid) = runtime_desktop_identity()?;

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
    let mut raw = String::new();

    stream
        .read_to_string(&mut raw)
        .map_err(|error| format!("could not read ExecutionRequest from Unix socket: {error}"))?;

    let request: ExecutionRequest = serde_json::from_str(raw.trim()).map_err(|error| {
        format!("invalid ExecutionRequest received through Unix socket: {error}")
    })?;

    let response = dispatcher::dispatch(request);

    let payload = serde_json::to_vec(&response).map_err(|error| {
        format!("could not serialize ExecutionResponse for Unix socket: {error}")
    })?;

    stream
        .write_all(&payload)
        .map_err(|error| format!("could not write ExecutionResponse to Unix socket: {error}"))?;

    Ok(())
}

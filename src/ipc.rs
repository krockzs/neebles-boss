use crate::dispatcher;
use crate::request::{ExecutionRequest, ExecutionResponse};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

const DEFAULT_SOCKET_PATH: &str = "/run/neebles/neebles.sock";

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

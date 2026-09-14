use crate::ipc;
use crate::request::{ExecutionRequest, ExecutionResponse};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

pub fn send(request: &ExecutionRequest) -> Result<ExecutionResponse, String> {
    let path = ipc::socket_path();

    let mut stream = UnixStream::connect(&path).map_err(|error| {
        format!(
            "could not connect to N.E.E.B.L.E.S. Boss socket {}: {error}",
            path.display()
        )
    })?;

    let payload = serde_json::to_vec(request).map_err(|error| {
        format!("could not serialize ExecutionRequest for N.E.E.B.L.E.S. socket: {error}")
    })?;

    stream.write_all(&payload).map_err(|error| {
        format!("could not write ExecutionRequest to N.E.E.B.L.E.S. socket: {error}")
    })?;

    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("could not finish N.E.E.B.L.E.S. socket request: {error}"))?;

    let mut response = Vec::new();

    stream.read_to_end(&mut response).map_err(|error| {
        format!("could not read ExecutionResponse from N.E.E.B.L.E.S. socket: {error}")
    })?;

    serde_json::from_slice(&response).map_err(|error| {
        format!("invalid ExecutionResponse received from N.E.E.B.L.E.S. socket: {error}")
    })
}

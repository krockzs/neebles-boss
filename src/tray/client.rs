use crate::tray::ipc::socket_path;
use crate::tray::protocol::TrayMessage;

use std::io::{BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

pub fn request(message: &TrayMessage) -> Result<TrayMessage, String> {
    let path = socket_path();

    let mut stream = UnixStream::connect(&path).map_err(|error| {
        format!(
            "could not connect to N.E.E.B.L.E.S. tray socket {}: {error}",
            path.display()
        )
    })?;

    let mut payload = serde_json::to_vec(message)
        .map_err(|error| format!("could not serialize tray request: {error}"))?;

    payload.push(b'\n');

    stream
        .write_all(&payload)
        .map_err(|error| format!("could not write tray request: {error}"))?;

    stream
        .shutdown(Shutdown::Write)
        .map_err(|error| format!("could not finish tray request: {error}"))?;

    let mut reader = BufReader::new(stream);

    let mut response = String::new();

    reader
        .read_line(&mut response)
        .map_err(|error| format!("could not read tray response: {error}"))?;

    if response.trim().is_empty() {
        return Err("N.E.E.B.L.E.S. tray socket returned an empty response".to_string());
    }

    serde_json::from_str(response.trim()).map_err(|error| format!("invalid tray response: {error}"))
}

use crate::module_ipc::protocol::ModuleMessage;

use std::io::{Read, Write};

const HEADER_SIZE: usize = 4;

/*
 * Límite de seguridad inicial.
 *
 * 16 MiB por mensaje.
 * Esto NO limita streams futuros, porque esos podrán tener
 * su propio mecanismo/protocolo.
 */
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

pub fn write_message<W: Write>(writer: &mut W, message: &ModuleMessage) -> Result<(), String> {
    let payload = serde_json::to_vec(message)
        .map_err(|error| format!("could not serialize module IPC message: {error}"))?;

    if payload.len() > MAX_FRAME_SIZE {
        return Err(format!(
            "module IPC frame too large: {} bytes; maximum is {}",
            payload.len(),
            MAX_FRAME_SIZE
        ));
    }

    let length = u32::try_from(payload.len())
        .map_err(|_| "module IPC frame length exceeds u32".to_string())?;

    writer
        .write_all(&length.to_be_bytes())
        .map_err(|error| format!("could not write module IPC frame header: {error}"))?;

    writer
        .write_all(&payload)
        .map_err(|error| format!("could not write module IPC frame payload: {error}"))?;

    writer
        .flush()
        .map_err(|error| format!("could not flush module IPC frame: {error}"))?;

    Ok(())
}

pub fn read_message<R: Read>(reader: &mut R) -> Result<Option<ModuleMessage>, String> {
    let mut header = [0u8; HEADER_SIZE];

    match reader.read_exact(&mut header) {
        Ok(()) => {}

        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Ok(None);
        }

        Err(error) => {
            return Err(format!("could not read module IPC frame header: {error}"));
        }
    }

    let length = u32::from_be_bytes(header) as usize;

    if length == 0 {
        return Err("module IPC frame cannot be empty".to_string());
    }

    if length > MAX_FRAME_SIZE {
        return Err(format!(
            "module IPC frame too large: {length} bytes; maximum is {}",
            MAX_FRAME_SIZE
        ));
    }

    let mut payload = vec![0u8; length];

    reader
        .read_exact(&mut payload)
        .map_err(|error| format!("could not read module IPC frame payload: {error}"))?;

    let message = serde_json::from_slice::<ModuleMessage>(&payload)
        .map_err(|error| format!("invalid module IPC JSON frame: {error}"))?;

    Ok(Some(message))
}

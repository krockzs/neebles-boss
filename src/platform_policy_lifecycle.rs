//! Governor-owned privileged platform policy lifecycle.
//!
//! The OS owns the provider, material approval and policy semantics. Boss owns
//! the installed module identity and when to reconcile it. No module names,
//! executable paths, host commands or root policy data are supplied by Boss.
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::time::Duration;

const SOCKET: &str = "/run/neebles/platform-operations.sock";

pub fn reconcile(module: &str, remove: bool) -> Result<(), String> {
    if module.is_empty()
        || module.len() > 128
        || !module
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    {
        return Err("invalid Governor module identity for platform policy".into());
    }
    let action = if remove {
        "platform.policy.revoke"
    } else {
        "platform.policy.sync"
    };
    let mut socket = UnixStream::connect(SOCKET)
        .map_err(|e| format!("OS platform policy service unavailable: {e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut peer: libc::ucred = unsafe { std::mem::zeroed() };
    let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let ok = unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut peer as *mut libc::ucred).cast(),
            &mut size,
        )
    };
    if ok != 0 || size as usize != std::mem::size_of::<libc::ucred>() || peer.uid != 0 {
        return Err("OS platform policy peer is not root-authenticated".into());
    }
    let request = serde_json::json!({"schema":"1", "module":module, "operation":action});
    socket
        .write_all(format!("{request}\n").as_bytes())
        .map_err(|e| format!("OS policy request write failed: {e}"))?;
    let mut reader = BufReader::new(socket);
    let mut response = String::new();
    if reader
        .read_line(&mut response)
        .map_err(|e| format!("OS policy reply failed: {e}"))?
        == 0
        || response.len() > 16_384
    {
        return Err("OS policy reply empty or oversized".into());
    }
    let reply: serde_json::Value =
        serde_json::from_str(&response).map_err(|e| format!("OS policy invalid reply: {e}"))?;
    if reply.get("schema").and_then(|v| v.as_str()) != Some("1")
        || reply.get("ok").and_then(|v| v.as_bool()) != Some(true)
    {
        return Err(format!(
            "OS platform policy denied module '{}': {}",
            module,
            reply
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
        ));
    }
    Ok(())
}

pub fn sync(module: &str) -> Result<(), String> {
    reconcile(module, false)
}

pub fn revoke(module: &str) -> Result<(), String> {
    reconcile(module, true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_module_names_or_helper_paths_encoded() {
        assert!(!super::SOCKET.contains("proxy"));
        assert!(!super::SOCKET.contains("tailscale"));
    }
}

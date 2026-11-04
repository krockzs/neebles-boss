//! Governor-owned privileged platform policy lifecycle.
//!
//! The OS provider owns approval and policy semantics. Governor supplies only
//! an authenticated module identity and a read-only digest preflight.
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

const SOCKET: &str = "/run/neebles/platform-operations.sock";

fn validate_module(module: &str) -> Result<(), String> {
    if module.is_empty()
        || module.len() > 128
        || !module
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    {
        return Err("invalid Governor module identity for platform policy".into());
    }
    Ok(())
}

fn sha256_regular_file(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| format!("candidate material unavailable {}: {e}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.len() > 16 * 1024 * 1024 {
        return Err(format!(
            "candidate material is not a bounded regular file: {}",
            path.display()
        ));
    }
    let bytes = fs::read(path)
        .map_err(|e| format!("candidate material unreadable {}: {e}", path.display()))?;
    let digest = Sha256::digest(&bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn candidate_fingerprint(
    module: &str,
    candidate_dir: &Path,
    has_platform_operations: bool,
) -> Result<serde_json::Value, String> {
    validate_module(module)?;
    if !has_platform_operations {
        return Ok(serde_json::Value::Null);
    }
    let manifest_sha256 = sha256_regular_file(&candidate_dir.join("manifest.json"))?;
    let contract_sha256 =
        sha256_regular_file(&candidate_dir.join("contracts/platform-operations.json"))?;
    let helper_sha256 = sha256_regular_file(
        &candidate_dir
            .join("runtime")
            .join(format!("{module}-platform")),
    )?;
    Ok(serde_json::json!({
        "schema": "1",
        "module": module,
        "manifest_sha256": manifest_sha256,
        "contract_sha256": contract_sha256,
        "helper_sha256": helper_sha256,
    }))
}

fn communicate(module: &str, request: serde_json::Value) -> Result<(), String> {
    validate_module(module)?;
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

/// A preflight never changes privileged policy. OS approval is independently
/// bound to the exact manifest, platform contract and helper ELF hashes.
pub fn preflight(
    module: &str,
    candidate_dir: &Path,
    has_platform_operations: bool,
) -> Result<(), String> {
    let candidate = candidate_fingerprint(module, candidate_dir, has_platform_operations)?;
    communicate(
        module,
        serde_json::json!({
            "schema": "1", "module": module,
            "operation": "platform.policy.preflight", "candidate": candidate,
        }),
    )
}

pub fn reconcile(module: &str, remove: bool) -> Result<(), String> {
    let operation = if remove {
        "platform.policy.revoke"
    } else {
        "platform.policy.sync"
    };
    communicate(
        module,
        serde_json::json!({
            "schema": "1", "module": module, "operation": operation,
        }),
    )
}

pub fn sync(module: &str) -> Result<(), String> {
    reconcile(module, false)
}

pub fn revoke(module: &str) -> Result<(), String> {
    reconcile(module, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_module_names_or_helper_paths_encoded() {
        assert!(!SOCKET.contains("proxy"));
        assert!(!SOCKET.contains("tailscale"));
    }

    #[test]
    fn preflight_without_platform_policy_requires_no_approval() {
        let candidate =
            candidate_fingerprint("example-module", Path::new("/absent"), false).unwrap();
        assert!(candidate.is_null());
    }

    #[test]
    fn preflight_hashes_exact_candidate_bytes() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("contracts")).unwrap();
        fs::create_dir_all(dir.path().join("runtime")).unwrap();
        fs::write(dir.path().join("manifest.json"), b"manifest").unwrap();
        fs::write(
            dir.path().join("contracts/platform-operations.json"),
            b"contract",
        )
        .unwrap();
        fs::write(
            dir.path().join("runtime/example-module-platform"),
            b"\x7fELFtest",
        )
        .unwrap();
        let result = candidate_fingerprint("example-module", dir.path(), true).unwrap();
        assert_eq!(result["schema"], "1");
        assert_eq!(result["module"], "example-module");
        assert_eq!(result["manifest_sha256"].as_str().unwrap().len(), 64);
        assert_eq!(result["contract_sha256"].as_str().unwrap().len(), 64);
    }
}

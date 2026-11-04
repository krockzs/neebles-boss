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
/// OS-owned review escrow. This is NOT an approval and grants no capability.
/// The copy contains only the exact manifest, policy contract and helper ELF.
const OS_REVIEW_ROOT: &str = "/var/lib/neebles/platform-operations/reviews";

fn review_source_file(
    path: &Path,
    cap: u64,
    executable: bool,
    strict: bool,
) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("OS review source unavailable {}: {e}", path.display()))?;
    if !meta.file_type().is_file()
        || meta.nlink() != 1
        || meta.len() > cap
        || (strict && (meta.uid() != 0 || meta.mode() & 0o022 != 0))
        || (executable && meta.mode() & 0o111 == 0)
    {
        return Err(format!(
            "OS review source is not a trusted bounded file: {}",
            path.display()
        ));
    }
    let bytes = fs::read(path).map_err(|e| format!("OS review source read failed: {e}"))?;
    if bytes.len() as u64 > cap || (executable && !bytes.starts_with(b"\x7fELF")) {
        return Err("OS review source size or ELF identity invalid".into());
    }
    Ok(bytes)
}

fn review_directory(path: &Path, strict: bool) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    let info = fs::symlink_metadata(path)
        .map_err(|e| format!("OS review directory unavailable {}: {e}", path.display()))?;
    if !info.file_type().is_dir() || (strict && (info.uid() != 0 || info.mode() & 0o022 != 0)) {
        return Err(format!(
            "OS review directory not trusted: {}",
            path.display()
        ));
    }
    Ok(())
}

fn review_root_ancestors(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || path.components().any(|p| {
            matches!(
                p,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err("OS review directory must be absolute and normalized".into());
    }
    for ancestor in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        if ancestor.exists() {
            review_directory(ancestor, true)?;
        }
    }
    Ok(())
}

fn preserve_review_candidate_at(
    module: &str,
    candidate_dir: &Path,
    review_root: &Path,
    strict: bool,
) -> Result<std::path::PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    validate_module(module)?;
    if strict && unsafe { libc::geteuid() } != 0 {
        return Err("only root Governor may prepare OS review material".into());
    }
    if strict {
        review_root_ancestors(candidate_dir)?;
        review_root_ancestors(
            review_root
                .parent()
                .ok_or("missing OS review root parent")?,
        )?;
    }
    review_directory(candidate_dir, strict)?;
    review_directory(&candidate_dir.join("contracts"), strict)?;
    review_directory(&candidate_dir.join("runtime"), strict)?;
    let parts = [
        ("manifest.json".to_string(), 16_384_u64, false),
        (
            "contracts/platform-operations.json".to_string(),
            16_384_u64,
            false,
        ),
        (format!("runtime/{module}-platform"), 32 * 1024 * 1024, true),
    ];
    // Read the complete authorized candidate BEFORE creating any OS review state.
    let input: Result<Vec<Vec<u8>>, String> = parts
        .iter()
        .map(|(p, max, elf)| review_source_file(&candidate_dir.join(p), *max, *elf, strict))
        .collect();
    let input = input?;
    let expected = candidate_fingerprint(module, candidate_dir, true)?;
    for ((_, _, _), (bytes, field)) in parts.iter().zip([
        (&input[0], "manifest_sha256"),
        (&input[1], "contract_sha256"),
        (&input[2], "helper_sha256"),
    ]) {
        let actual: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if expected[field].as_str() != Some(actual.as_str()) {
            return Err("candidate changed during OS review preparation".into());
        }
    }
    if !review_root.exists() {
        fs::create_dir(review_root).map_err(|e| format!("cannot prepare OS review root: {e}"))?;
        fs::set_permissions(review_root, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("cannot secure OS review root: {e}"))?;
    }
    review_directory(review_root, strict)?;
    if strict {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(review_root)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode();
        if perms & 0o077 != 0 {
            return Err("OS review root must be restricted to root (0700)".into());
        }
    }
    let final_path = review_root.join(module);
    let unique = format!(
        ".review-{}-{}-{}",
        module,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let temporary = review_root.join(&unique);
    fs::create_dir(&temporary).map_err(|e| format!("cannot reserve OS review snapshot: {e}"))?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("cannot secure OS review snapshot: {e}"))?;
    let staged = (|| -> Result<(), String> {
        for (sub, _, _) in &parts {
            let target = temporary.join(sub);
            fs::create_dir_all(target.parent().ok_or("invalid target")?)
                .map_err(|e| format!("OS review directory staging failed: {e}"))?;
        }
        for ((sub, _, elf), bytes) in parts.iter().zip(input.iter()) {
            let file = temporary.join(sub);
            fs::write(&file, bytes).map_err(|e| format!("OS review stage failed: {e}"))?;
            fs::set_permissions(
                &file,
                fs::Permissions::from_mode(if *elf { 0o700 } else { 0o600 }),
            )
            .map_err(|e| format!("OS review permission failed: {e}"))?;
            fs::File::open(&file)
                .and_then(|f| f.sync_all())
                .map_err(|e| format!("OS review file durability failed: {e}"))?;
        }
        for dir in [temporary.join("contracts"), temporary.join("runtime")] {
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("OS review directory permission failed: {e}"))?;
        }
        let check = candidate_fingerprint(module, &temporary, true)?;
        if check != expected {
            return Err("OS review copied bytes do not match source".into());
        }
        fs::File::open(&temporary)
            .and_then(|f| f.sync_all())
            .map_err(|e| format!("OS review directory durability failed: {e}"))?;
        Ok(())
    })();
    if let Err(e) = staged {
        let _ = fs::remove_dir_all(&temporary);
        return Err(e);
    }
    // The stable path contains exactly ONE candidate per module. Replacements
    // are atomic under a root-controlled directory and require a fresh review.
    let previous = review_root.join(format!("{unique}.previous"));
    let had_previous = final_path.exists() || final_path.is_symlink();
    if had_previous {
        if let Err(e) = review_directory(&final_path, strict) {
            let _ = fs::remove_dir_all(&temporary);
            return Err(e);
        }
        if let Err(e) = fs::rename(&final_path, &previous) {
            let _ = fs::remove_dir_all(&temporary);
            return Err(format!("cannot preserve previous OS review candidate: {e}"));
        }
    }
    if let Err(e) = fs::rename(&temporary, &final_path) {
        if had_previous {
            let _ = fs::rename(&previous, &final_path);
        }
        let _ = fs::remove_dir_all(&temporary);
        return Err(format!("cannot publish OS review snapshot: {e}"));
    }
    fs::File::open(review_root)
        .and_then(|f| f.sync_all())
        .map_err(|e| format!("OS review directory commit durability failed: {e}"))?;
    if had_previous {
        // The previous snapshot is not an approval and is never executable by
        // Lifecycle; no privileged policy or receipt is touched here.
        fs::remove_dir_all(&previous).map_err(|e| {
            format!("OS review snapshot replaced, stale snapshot cleanup failed: {e}")
        })?;
    }
    Ok(final_path)
}

/// Preserve a verified root-only review copy before the temporary candidate
/// is removed on an approval-required Install/Update failure.
pub fn preserve_candidate_for_os_review(
    module: &str,
    candidate_dir: &Path,
    preflight_error: &str,
) -> Result<std::path::PathBuf, String> {
    let expected = candidate_fingerprint(module, candidate_dir, true)?;
    let expected_prefix = format!(
        "OS_ADMIN_APPROVAL_REQUIRED module={module} manifest_sha256={} contract_sha256={} helper_sha256={}",
        expected["manifest_sha256"].as_str().unwrap_or(""),
        expected["contract_sha256"].as_str().unwrap_or(""),
        expected["helper_sha256"].as_str().unwrap_or(""),
    );
    if !preflight_error.starts_with(&expected_prefix) {
        return Err("OS review challenge is not bound to exact certified candidate".into());
    }
    preserve_review_candidate_at(module, candidate_dir, Path::new(OS_REVIEW_ROOT), true)
}

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

/// Interpret provider decisions; this function never authorizes or writes.
fn interpret_provider_reply(
    module: &str,
    request: &serde_json::Value,
    reply: &serde_json::Value,
) -> Result<(), String> {
    validate_module(module)?;
    if reply.get("schema").and_then(|v| v.as_str()) != Some("1") {
        return Err(format!(
            "OS platform policy denied module '{module}': invalid provider schema"
        ));
    }
    if reply.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        // A positive response cannot simultaneously demand approval.
        if reply.get("approval_required").is_some() {
            return Err("OS platform policy contradictory provider response".into());
        }
        return Ok(());
    }
    if reply.get("approval_required").and_then(|v| v.as_bool()) == Some(true) {
        let expected = &reply["fingerprint"];
        // A review request is valid only for the exact preflight candidate.
        // Reject other operations, unsent fingerprints and extra fields.
        if reply.get("ok").and_then(|v| v.as_bool()) == Some(false)
            && request.get("operation").and_then(|v| v.as_str())
                == Some("platform.policy.preflight")
            && request.get("candidate") == Some(expected)
            && expected.as_object().is_some_and(|map| map.len() == 5)
            && expected["schema"].as_str() == Some("1")
            && expected["module"].as_str() == Some(module)
            && ["manifest_sha256", "contract_sha256", "helper_sha256"]
                .iter()
                .all(|key| {
                    expected[key].as_str().is_some_and(|digest| {
                        digest.len() == 64
                            && digest
                                .bytes()
                                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                    })
                })
        {
            return Err(format!(
                "OS_ADMIN_APPROVAL_REQUIRED module={module} manifest_sha256={} contract_sha256={} helper_sha256={}; explicit OS administrator review of certified candidate is required before retrying Install/Update",
                expected["manifest_sha256"].as_str().unwrap_or(""),
                expected["contract_sha256"].as_str().unwrap_or(""),
                expected["helper_sha256"].as_str().unwrap_or(""),
            ));
        }
        return Err(format!(
            "OS platform policy denied module '{module}': invalid approval challenge"
        ));
    }
    Err(format!(
        "OS platform policy denied module '{}': {}",
        module,
        reply
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error")
    ))
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
    interpret_provider_reply(module, &request, &reply)
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
    fn exact_os_approval_required_challenge_is_not_success() {
        let denied = serde_json::json!({
            "schema": "1", "ok": false, "approval_required": true,
            "fingerprint": {
                "schema": "1", "module": "generic-module",
                "manifest_sha256": "a".repeat(64),
                "contract_sha256": "b".repeat(64),
                "helper_sha256": "c".repeat(64),
            },
        });
        let request = serde_json::json!({
            "operation": "platform.policy.preflight", "candidate": denied["fingerprint"]
        });
        let error = interpret_provider_reply("generic-module", &request, &denied).unwrap_err();
        assert!(error.contains("OS_ADMIN_APPROVAL_REQUIRED"));
        assert!(error.contains(&"a".repeat(64)));
    }

    #[test]
    fn malformed_or_contradictory_challenge_fails_closed() {
        let malformed = serde_json::json!({
            "schema":"1", "ok":false, "approval_required":true,
            "fingerprint":{"schema":"1", "module":"other"}
        });
        let request = serde_json::json!({
            "operation": "platform.policy.preflight", "candidate": malformed["fingerprint"]
        });
        assert!(
            interpret_provider_reply("generic-module", &request, &malformed)
                .unwrap_err()
                .contains("invalid approval challenge")
        );
        let contradictory = serde_json::json!({
            "schema":"1", "ok":true, "approval_required":true
        });
        assert!(interpret_provider_reply("generic-module", &request, &contradictory).is_err());
    }

    #[test]
    fn review_challenge_must_match_exact_sent_candidate_and_operation() {
        let fingerprint = serde_json::json!({
            "schema":"1", "module":"generic-module",
            "manifest_sha256":"a".repeat(64),
            "contract_sha256":"b".repeat(64),
            "helper_sha256":"c".repeat(64),
        });
        let reply = serde_json::json!({
            "schema":"1", "ok":false, "approval_required":true,
            "fingerprint":fingerprint,
        });
        let valid = serde_json::json!({
            "operation":"platform.policy.preflight", "candidate":fingerprint,
        });
        assert!(interpret_provider_reply("generic-module", &valid, &reply)
            .unwrap_err()
            .contains("OS_ADMIN_APPROVAL_REQUIRED"));
        let wrong = serde_json::json!({
            "operation":"platform.policy.preflight",
            "candidate": {
                "schema":"1", "module":"generic-module",
                "manifest_sha256":"f".repeat(64),
                "contract_sha256":"b".repeat(64),
                "helper_sha256":"c".repeat(64),
            }
        });
        assert!(interpret_provider_reply("generic-module", &wrong, &reply)
            .unwrap_err()
            .contains("invalid approval challenge"));
        let sync = serde_json::json!({
            "operation":"platform.policy.sync", "candidate":fingerprint,
        });
        assert!(interpret_provider_reply("generic-module", &sync, &reply)
            .unwrap_err()
            .contains("invalid approval challenge"));
        let mut extra = reply.clone();
        extra["fingerprint"]["unknown"] = serde_json::json!(true);
        let mut extra_request = valid.clone();
        extra_request["candidate"] = extra["fingerprint"].clone();
        assert!(
            interpret_provider_reply("generic-module", &extra_request, &extra)
                .unwrap_err()
                .contains("invalid approval challenge")
        );
    }

    #[test]
    fn ordinary_provider_success_and_denial_remain_distinct() {
        let request = serde_json::json!({"operation":"platform.policy.sync"});
        assert!(interpret_provider_reply(
            "generic-module",
            &request,
            &serde_json::json!({
                "schema":"1", "ok":true, "result":{"state":"ready"}
            })
        )
        .is_ok());
        assert!(interpret_provider_reply(
            "generic-module",
            &request,
            &serde_json::json!({
                "schema":"1", "ok":false, "error":"unreconciled transaction"
            })
        )
        .unwrap_err()
        .contains("unreconciled transaction"));
    }

    #[test]
    fn os_review_snapshot_contains_exact_three_materials_without_grant() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let candidate = root.path().join("candidate");
        fs::create_dir_all(candidate.join("contracts")).unwrap();
        fs::create_dir_all(candidate.join("runtime")).unwrap();
        fs::write(candidate.join("manifest.json"), b"candidate manifest").unwrap();
        fs::write(
            candidate.join("contracts/platform-operations.json"),
            b"candidate policy",
        )
        .unwrap();
        fs::write(
            candidate.join("runtime/generic-module-platform"),
            b"\x7fELFexample",
        )
        .unwrap();
        fs::set_permissions(
            candidate.join("runtime/generic-module-platform"),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let escrow = root.path().join("reviews");
        let review =
            preserve_review_candidate_at("generic-module", &candidate, &escrow, false).unwrap();
        assert_eq!(review, escrow.join("generic-module"));
        assert_eq!(
            candidate_fingerprint("generic-module", &review, true).unwrap(),
            candidate_fingerprint("generic-module", &candidate, true).unwrap()
        );
        assert!(!review.join("approvals").exists());
        assert!(!review.join("rootfs").exists());
        assert!(!review.join("lifecycle.json").exists());
        assert!(!escrow.join(".previous").exists());
    }

    #[test]
    fn os_review_snapshot_rejects_symlink_and_invalid_elf_without_replacing_previous() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let candidate = root.path().join("candidate");
        fs::create_dir_all(candidate.join("contracts")).unwrap();
        fs::create_dir_all(candidate.join("runtime")).unwrap();
        fs::write(candidate.join("manifest.json"), b"old manifest").unwrap();
        fs::write(
            candidate.join("contracts/platform-operations.json"),
            b"policy",
        )
        .unwrap();
        let helper = candidate.join("runtime/generic-module-platform");
        fs::write(&helper, b"\x7fELFexample").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let escrow = root.path().join("reviews");
        let review =
            preserve_review_candidate_at("generic-module", &candidate, &escrow, false).unwrap();
        let original = fs::read(review.join("manifest.json")).unwrap();
        fs::write(&helper, b"not-elf").unwrap();
        assert!(
            preserve_review_candidate_at("generic-module", &candidate, &escrow, false).is_err()
        );
        assert_eq!(fs::read(review.join("manifest.json")).unwrap(), original);
        fs::remove_file(&helper).unwrap();
        symlink(candidate.join("manifest.json"), &helper).unwrap();
        assert!(
            preserve_review_candidate_at("generic-module", &candidate, &escrow, false).is_err()
        );
        assert_eq!(fs::read(review.join("manifest.json")).unwrap(), original);
    }

    #[test]
    fn os_review_snapshot_replaces_previous_without_leaking_old_version() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let candidate = root.path().join("candidate");
        fs::create_dir_all(candidate.join("contracts")).unwrap();
        fs::create_dir_all(candidate.join("runtime")).unwrap();
        fs::write(candidate.join("manifest.json"), b"old").unwrap();
        fs::write(
            candidate.join("contracts/platform-operations.json"),
            b"contract",
        )
        .unwrap();
        let helper = candidate.join("runtime/generic-module-platform");
        fs::write(&helper, b"\x7fELFexample").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let escrow = root.path().join("reviews");
        preserve_review_candidate_at("generic-module", &candidate, &escrow, false).unwrap();
        fs::write(candidate.join("manifest.json"), b"new").unwrap();
        preserve_review_candidate_at("generic-module", &candidate, &escrow, false).unwrap();
        assert_eq!(
            fs::read(escrow.join("generic-module/manifest.json")).unwrap(),
            b"new"
        );
        assert_eq!(fs::read_dir(&escrow).unwrap().count(), 1);
    }
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

/*
 * N.E.E.B.L.E.S. Uninstall contract, schema 1.
 *
 * The module declares resources. Boss owns policy and execution. None of
 * these declarations grants a module permission to run arbitrary root code.
 * Lifecycle/ Governor remain the authority for the main application.
 */
use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_resolver;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

fn schema_one() -> u32 {
    1
}
fn governor() -> String {
    "governor".into()
}
fn inactive() -> String {
    "inactive".into()
}
fn terminated() -> String {
    "terminated".into()
}
fn abort() -> String {
    "abort".into()
}
fn default_timeout() -> u64 {
    5000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UninstallContract {
    #[serde(default = "schema_one")]
    pub schema: u32,
    #[serde(default)]
    pub hardcoded: BTreeMap<String, String>,
    #[serde(default = "governor")]
    pub main: String,
    #[serde(default)]
    pub extras: Vec<Extra>,
    #[serde(default)]
    pub cleanup: Vec<Cleanup>,
    #[serde(default)]
    pub failure: FailurePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Extra {
    Governed {
        id: String,
        #[serde(rename = "ref")]
        reference: String,
        #[serde(default)]
        required: bool,
        #[serde(default = "terminated")]
        verify: String,
    },
    Service {
        id: String,
        provider: String,
        name: String,
        // Complete exact systemd ExecStart line as declared by this module.
        // Boss interprets no programming language or runner technology.
        exec_start: String,
        #[serde(default)]
        required: bool,
        #[serde(default)]
        stop: ServiceStop,
        #[serde(default = "inactive")]
        verify: String,
        #[serde(default)]
        remove: bool,
    },
    // A desktop startup registration is an executable resource, NEVER user data.
    // Uninstall retires it irrespective of the decision to preserve Settings.
    Autostart {
        id: String,
        provider: String,
        name: String,
        exec_start: String,
        #[serde(default)]
        required: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceStop {
    #[serde(default)]
    pub disable: bool,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}
impl Default for ServiceStop {
    fn default() -> Self {
        Self {
            disable: false,
            timeout_ms: default_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Cleanup {
    Directory {
        id: String,
        scope: String,
        path: String,
        policy: String,
    },
    Settings {
        id: String,
        #[serde(rename = "ref")]
        reference: String,
        policy: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailurePolicy {
    #[serde(default = "abort")]
    pub unknown_process_owner: String,
    #[serde(default = "abort")]
    pub resource_still_running: String,
    #[serde(default = "abort")]
    pub cleanup_error: String,
}
impl Default for FailurePolicy {
    fn default() -> Self {
        Self {
            unknown_process_owner: abort(),
            resource_still_running: abort(),
            cleanup_error: abort(),
        }
    }
}

pub fn resolve(input: &str, contract: &UninstallContract, module: &str) -> Result<String, String> {
    let mut battlefield = Battlefield::new();
    for (key, value) in &contract.hardcoded {
        battlefield.insert("hardcoded", key, value);
    }
    battlefield.insert("module", "id", module);
    lifecycle_resolver::resolve(input, &battlefield)
}

fn relative_path(path: &str) -> Result<(), String> {
    let value = Path::new(path);
    if value.as_os_str().is_empty()
        || value.is_absolute()
        || !value
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "uninstall cleanup path must contain only relative normal components: {path}"
        ));
    }
    Ok(())
}

fn validate_service_name(module: &str, name: &str) -> Result<(), String> {
    let prefix = format!("neebles-{module}-");
    if !name.starts_with(&prefix)
        || !name.ends_with(".service")
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        || name.contains("..")
    {
        return Err(format!(
            "uninstall service name '{name}' is outside module '{module}' namespace"
        ));
    }
    Ok(())
}

// Identical ownership namespace to systemd-user, but different artifact type.
fn validate_autostart_name(module: &str, name: &str) -> Result<(), String> {
    let prefix = format!("neebles-{module}-");
    if !name.starts_with(&prefix)
        || !name.ends_with(".desktop")
        || name.len() <= prefix.len() + ".desktop".len()
        || name.contains("..")
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(format!(
            "uninstall autostart '{name}' is outside owned module namespace"
        ));
    }
    Ok(())
}

// No shell grammar or XDG field codes; each artifact must identify a literal
// command whose executable or direct script is inside this module.
fn validate_autostart_text(contents: &str, expected: &str) -> Result<(), String> {
    let mut in_desktop = false;
    let mut sections = 0usize;
    let mut entry_type = 0usize;
    let mut entry_exec = 0usize;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop = line == "[Desktop Entry]";
            if in_desktop {
                sections += 1;
            }
            continue;
        }
        if !in_desktop {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            if key == "Type" {
                entry_type += 1;
                if val != "Application" {
                    return Err("autostart Type must be Application".into());
                }
            }
            if key == "Exec" {
                entry_exec += 1;
                if val != expected {
                    return Err("autostart Exec differs from owned module declaration".into());
                }
            }
        }
    }
    if sections != 1 || entry_type != 1 || entry_exec != 1 {
        return Err("autostart requires exactly one Desktop Entry, Type and Exec".into());
    }
    Ok(())
}

fn validate_service_exec_start(expected: &str, module: &str) -> Result<(), String> {
    // Systemd's quoting/escaping grammar is deliberately not guessed here.
    // Schema 1 accepts an unambiguous argv spelling. More complex launchers
    // need a formally defined subsequent contract schema, not a shell parser.
    if expected.trim() != expected
        || expected.is_empty()
        || expected
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | '\'' | '"' | ';' | '%' | '$'))
    {
        return Err("uninstall ExecStart must be one literal, unescaped command line".into());
    }
    let args: Vec<_> = expected.split_ascii_whitespace().collect();
    if args.is_empty() || !args[0].starts_with('/') {
        return Err("uninstall ExecStart must begin with an absolute executable".into());
    }
    let root = format!("/opt/neebles/modules/{module}/");
    let owned_arg = if args[0].starts_with(&root) {
        args[0]
    } else if args.len() > 1 && args[1].starts_with(&root) {
        args[1]
    } else {
        return Err("uninstall ExecStart must directly launch a module-owned entrypoint".into());
    };
    relative_path(&owned_arg[root.len()..])?;
    Ok(())
}

fn validate_service_unit_text(contents: &str, expected: &str, module: &str) -> Result<(), String> {
    validate_service_exec_start(expected, module)?;
    let mut in_service = false;
    let mut commands = Vec::new();
    for line in contents.lines() {
        let entry = line.trim();
        if entry.starts_with('#') || entry.starts_with(';') || entry.is_empty() {
            continue;
        }
        if entry.starts_with('[') && entry.ends_with(']') {
            in_service = entry == "[Service]";
            continue;
        }
        if !in_service {
            continue;
        }
        if let Some((key, value)) = entry.split_once('=') {
            let key = key.trim();
            // These commands can execute on `systemctl stop`, even if
            // ExecStart itself belongs to this module.
            if matches!(
                key,
                "ExecStop" | "ExecStopPost" | "ExecStartPre" | "ExecStartPost" | "ExecCondition"
            ) {
                return Err(format!(
                    "uninstall service has untrusted execution hook {key}"
                ));
            }
            if key == "ExecStart" {
                commands.push(value);
            }
        }
    }
    if commands.as_slice() != [expected] {
        return Err("uninstall service ExecStart differs from exact module declaration".into());
    }
    Ok(())
}

pub fn validate(contract: &UninstallContract, module: &str, has_tray: bool) -> Result<(), String> {
    if contract.schema != 1 {
        return Err("unsupported uninstall contract schema (expected 1)".into());
    }
    if contract.main != "governor" {
        return Err("uninstall.main must equal 'governor'".into());
    }
    for (key, value) in &contract.hardcoded {
        if key.trim().is_empty() || value.trim().is_empty() {
            return Err("uninstall hardcoded keys/values cannot be empty".into());
        }
    }
    let failure = &contract.failure;
    if failure.unknown_process_owner != "abort"
        || failure.resource_still_running != "abort"
        || failure.cleanup_error != "abort"
    {
        return Err("uninstall schema 1 requires fail-closed 'abort' failure policy".into());
    }
    let mut ids = BTreeSet::new();
    for extra in &contract.extras {
        let id = match extra {
            Extra::Governed { id, .. }
            | Extra::Service { id, .. }
            | Extra::Autostart { id, .. } => id,
        };
        if id.trim().is_empty() || !ids.insert(id.as_str()) {
            return Err(format!("duplicate/empty uninstall resource id: {id}"));
        }
        match extra {
            Extra::Governed {
                reference,
                verify,
                required,
                ..
            } => {
                // The only currently registered governed optional extra in schema 4 is manifest.tray.
                // New adapters are added to Boss, never executed by guessing a process name.
                if resolve(reference, contract, module)? != "manifest.tray"
                    || (*required && !has_tray)
                {
                    return Err(format!(
                        "uninstall governed ref '{}' is not an available Boss resource",
                        reference
                    ));
                }
                if verify != "terminated" {
                    return Err(format!("unsupported governed verify '{verify}'"));
                }
            }
            Extra::Service {
                provider,
                name,
                exec_start,
                stop,
                verify,
                remove,
                ..
            } => {
                if provider != "systemd-user" || verify != "inactive" {
                    return Err(
                        "uninstall service requires provider=systemd-user and verify=inactive"
                            .into(),
                    );
                }
                if stop.timeout_ms == 0 || stop.timeout_ms > 60_000 {
                    return Err("uninstall service timeout_ms must be in 1..=60000".into());
                }
                validate_service_name(module, &resolve(name, contract, module)?)?;
                validate_service_exec_start(&resolve(exec_start, contract, module)?, module)?;
                // A module being UNINSTALLED must never retain an enabled
                // service, even when Settings are kept by the user.
                if !*remove || !stop.disable {
                    return Err(
                        "uninstall always requires service remove=true and stop.disable=true"
                            .into(),
                    );
                }
            }
            Extra::Autostart {
                provider,
                name,
                exec_start,
                ..
            } => {
                if provider != "xdg-user" {
                    return Err("uninstall autostart requires provider=xdg-user".into());
                }
                validate_autostart_name(module, &resolve(name, contract, module)?)?;
                validate_service_exec_start(&resolve(exec_start, contract, module)?, module)?;
            }
        }
    }
    for item in &contract.cleanup {
        let id = match item {
            Cleanup::Directory { id, .. } | Cleanup::Settings { id, .. } => id,
        };
        if id.trim().is_empty() || !ids.insert(id.as_str()) {
            return Err(format!("duplicate/empty uninstall resource id: {id}"));
        }
        match item {
            Cleanup::Directory {
                scope,
                path,
                policy,
                ..
            } => {
                if scope != "module-data" {
                    return Err("uninstall cleanup directory scope must be 'module-data'".into());
                }
                if policy != "always" && policy != "user-choice" {
                    return Err("uninstall cleanup policy must be always or user-choice".into());
                }
                relative_path(&resolve(path, contract, module)?)?;
            }
            Cleanup::Settings {
                reference, policy, ..
            } => {
                if reference != "manifest.settings" || policy != "user-choice" {
                    return Err("uninstall settings must reference manifest.settings with user-choice policy".into());
                }
            }
        }
    }
    Ok(())
}

// A login user's HOME is an authority boundary for both XDG autostart
// and systemd-user. Check the actual filesystem entry; metadata() would
// follow a symlink even though our startup-resource policy rejects it.
fn validate_desktop_home_path(path: &Path, uid: libc::uid_t) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("user home cannot be inspected: {error}"))?;
    if !metadata.file_type().is_dir() || metadata.uid() != uid || metadata.mode() & 0o002 != 0 {
        return Err("desktop user home must be an owned, non-world-writable real directory".into());
    }
    Ok(())
}

fn desktop_home(uid: libc::uid_t) -> Result<PathBuf, String> {
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result: *mut libc::passwd = std::ptr::null_mut();
    let mut buffer = vec![0i8; 16384];
    let rc = unsafe {
        libc::getpwuid_r(
            uid,
            &mut pwd,
            buffer.as_mut_ptr(),
            buffer.len(),
            &mut result,
        )
    };
    if rc != 0 || result.is_null() || pwd.pw_dir.is_null() {
        return Err(format!(
            "could not resolve certified desktop user home for uid {uid}"
        ));
    }
    let value = unsafe { std::ffi::CStr::from_ptr(pwd.pw_dir) }
        .to_string_lossy()
        .into_owned();
    let path = PathBuf::from(value);
    if !path.is_absolute() || path == Path::new("/") {
        return Err("invalid desktop user home".into());
    }
    validate_desktop_home_path(&path, uid)?;
    Ok(path)
}

fn systemctl_as_desktop(
    uid: libc::uid_t,
    gid: libc::gid_t,
    home: &Path,
    args: &[&str],
) -> Result<String, String> {
    let runtime_dir = format!("/run/user/{uid}");
    let output = Command::new("/usr/bin/systemctl")
        .args(["--user"])
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("XDG_RUNTIME_DIR", &runtime_dir)
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path={runtime_dir}/bus"),
        )
        .env("LC_ALL", "C")
        .uid(uid)
        .gid(gid)
        .output()
        .map_err(|error| format!("cannot execute desktop systemctl: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "desktop systemctl {:?} exited {}: {}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}

fn systemd_state(
    uid: libc::uid_t,
    gid: libc::gid_t,
    home: &Path,
    unit: &str,
) -> Result<(String, String), String> {
    let show = systemctl_as_desktop(
        uid,
        gid,
        home,
        &[
            "show",
            "--no-pager",
            "-p",
            "LoadState",
            "-p",
            "ActiveState",
            unit,
        ],
    )?;
    let mut load = None;
    let mut active = None;
    for line in show.lines() {
        if let Some(value) = line.strip_prefix("LoadState=") {
            load = Some(value.to_string());
        }
        if let Some(value) = line.strip_prefix("ActiveState=") {
            active = Some(value.to_string());
        }
    }
    Ok((
        load.ok_or("missing LoadState")?,
        active.ok_or("missing ActiveState")?,
    ))
}

fn validated_unit_path(
    home: &Path,
    uid: libc::uid_t,
    gid: libc::gid_t,
    unit: &str,
) -> Result<PathBuf, String> {
    let mut path = home.to_path_buf();
    let meta = fs::symlink_metadata(&path).map_err(|e| format!("uninstall home metadata: {e}"))?;
    if !meta.file_type().is_dir() || meta.uid() != uid {
        return Err("uninstall desktop home is not an owned real directory".into());
    }
    for part in [".config", "systemd", "user"] {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(meta)
                if meta.file_type().is_dir()
                    && meta.uid() == uid
                    && meta.gid() == gid
                    && meta.mode() & 0o002 == 0 => {}
            Ok(_) => {
                return Err(format!(
                "uninstall service parent is not an owned, non-world-writable real directory: {}",
                path.display()
            ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(format!(
                    "uninstall service parent inspection failed: {error}"
                ))
            }
        }
    }
    Ok(home.join(".config/systemd/user").join(unit))
}

fn validate_manager_state(show: &str, path: &Path, expected: &str) -> Result<(), String> {
    let mut fragment = None;
    let mut dropins = None;
    let mut execstart = None;
    let mut unexpected_hooks = false;
    for line in show.lines() {
        if let Some(value) = line.strip_prefix("FragmentPath=") {
            fragment = Some(value);
        }
        if let Some(value) = line.strip_prefix("DropInPaths=") {
            dropins = Some(value);
        }
        if let Some(value) = line.strip_prefix("ExecStart=") {
            execstart = Some(value);
        }
        for key in [
            "ExecStop=",
            "ExecStopPost=",
            "ExecStartPre=",
            "ExecStartPost=",
            "ExecCondition=",
        ] {
            if let Some(value) = line.strip_prefix(key) {
                if !value.is_empty() {
                    unexpected_hooks = true;
                }
            }
        }
    }
    if fragment != path.to_str() || dropins != Some("") || unexpected_hooks {
        return Err(
            "uninstall service manager has an untrusted fragment, override or executable hook"
                .into(),
        );
    }
    let argv = execstart
        .and_then(|value| value.split_once("argv[]="))
        .and_then(|(_, rest)| rest.split_once(" ; ignore_errors="))
        .map(|(argv, _)| argv);
    if argv != Some(expected) {
        return Err(
            "uninstall service manager argv does not match its exact module declaration".into(),
        );
    }
    Ok(())
}

fn validate_manager_fragment(
    uid: libc::uid_t,
    gid: libc::gid_t,
    home: &Path,
    unit: &str,
    path: &Path,
    expected: &str,
) -> Result<(), String> {
    let show = systemctl_as_desktop(
        uid,
        gid,
        home,
        &[
            "show",
            "--no-pager",
            "-p",
            "FragmentPath",
            "-p",
            "DropInPaths",
            "-p",
            "ExecStart",
            "-p",
            "ExecStop",
            "-p",
            "ExecStopPost",
            "-p",
            "ExecStartPre",
            "-p",
            "ExecStartPost",
            "-p",
            "ExecCondition",
            unit,
        ],
    )?;
    validate_manager_state(&show, path, expected)
}

pub fn stop_service(
    module: &str,
    unit: &str,
    exec_start: &str,
    required: bool,
    stop: &ServiceStop,
    remove: bool,
) -> Result<(), String> {
    validate_service_name(module, unit)?;
    let (uid, gid) = crate::runtime_identity::desktop_identity()?;
    if uid == 0 {
        return Err("cannot manage systemd-user services with desktop UID 0".into());
    }
    let home = desktop_home(uid)?;
    let path = validated_unit_path(&home, uid, gid, unit)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(meta) => Some(meta),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("cannot inspect uninstall service unit: {error}")),
    };
    let (load, active) = systemd_state(uid, gid, &home, unit)?;
    if metadata.is_none() {
        if active != "inactive" {
            return Err(format!("uninstall service '{unit}' still has an active manager state without an owned unit file"));
        }
        if load != "not-found" {
            // A previous attempt may have removed the unit but failed just
            // before daemon-reload. Refresh the desktop user's manager.
            systemctl_as_desktop(uid, gid, &home, &["daemon-reload"])?;
            let (load_after, active_after) = systemd_state(uid, gid, &home, unit)?;
            if load_after != "not-found" || active_after != "inactive" {
                return Err(format!(
                    "uninstall service '{unit}' remains loaded without an owned unit file"
                ));
            }
        }
        return if required {
            Err(format!("required uninstall service '{unit}' is absent"))
        } else {
            Ok(())
        };
    }
    let meta = metadata.expect("verified metadata presence");
    if !meta.file_type().is_file()
        || meta.uid() != uid
        || meta.gid() != gid
        || meta.mode() & 0o002 != 0
    {
        return Err(format!(
            "uninstall service '{unit}' file type/owner/mode is not certified"
        ));
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read uninstall service '{unit}': {error}"))?;
    validate_service_unit_text(&contents, exec_start, module)?;
    if load == "not-found" {
        return Err(format!(
            "uninstall service '{unit}' manager state is not loaded"
        ));
    }
    validate_manager_fragment(uid, gid, &home, unit, &path, exec_start)?;
    // Keep an enabled unit enabled while reversible work remains. The unit
    // will be disabled and retired only after the module has committed.
    if remove && stop.disable {
        systemctl_as_desktop(uid, gid, &home, &["disable", unit])?;
    }
    // --no-block lets Boss, not systemd's default job deadline, own the
    // bounded verification timeout from this contract.
    systemctl_as_desktop(uid, gid, &home, &["stop", "--no-block", unit])?;
    let deadline = Instant::now() + Duration::from_millis(stop.timeout_ms);
    loop {
        let (_, active) = systemd_state(uid, gid, &home, unit)?;
        if active == "inactive" || active == "failed" {
            break;
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "uninstall service '{unit}' still active after timeout"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if remove {
        // A module unit can only be removed from its desktop user's own unit folder.
        // symlink_metadata certifies the final entry; remove_file never follows it.
        fs::remove_file(&path)
            .map_err(|error| format!("cannot remove module unit '{unit}': {error}"))?;
        systemctl_as_desktop(uid, gid, &home, &["daemon-reload"])?;
    }
    Ok(())
}

// A receipt of exactly which declared services were active before quiescence.
// No service is started automatically by Drop: a recovery error must reach
// the caller and must never be disguised as a successful transaction rollback.
#[derive(Debug, Clone)]
struct ServicePriorState {
    unit: String,
    exec_start: String,
    timeout_ms: u64,
    was_active: bool,
}

#[derive(Debug, Default)]
pub struct UninstallServiceRecovery {
    entries: Vec<ServicePriorState>,
}

impl UninstallServiceRecovery {
    // Reverse-order compensation, limited strictly to previously active units.
    // Continue independent compensations even if one restart fails.
    fn restore_with<F>(&self, mut restore: F) -> Result<(), String>
    where
        F: FnMut(&ServicePriorState) -> Result<(), String>,
    {
        let mut failures = Vec::new();
        for entry in self.entries.iter().rev().filter(|entry| entry.was_active) {
            if let Err(error) = restore(entry) {
                failures.push(format!("{}: {error}", entry.unit));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "CRITICAL: service recovery incomplete: {}",
                failures.join("; ")
            ))
        }
    }

    pub fn reactivate(&self, module: &str) -> Result<(), String> {
        self.restore_with(|entry| {
            crate::module_uninstall_arsenal::systemd_user_restore(
                module,
                &entry.unit,
                &entry.exec_start,
                entry.timeout_ms,
            )
        })
    }
}

fn capture_prior_service_state(
    module: &str,
    unit: &str,
    exec_start: &str,
    timeout_ms: u64,
) -> Result<ServicePriorState, String> {
    validate_service_name(module, unit)?;
    validate_service_exec_start(exec_start, module)?;
    let (uid, gid) = crate::runtime_identity::desktop_identity()?;
    if uid == 0 {
        return Err("cannot inspect desktop service with UID 0".into());
    }
    let home = desktop_home(uid)?;
    let (_, state) = systemd_state(uid, gid, &home, unit)?;
    let was_active = match state.as_str() {
        "active" => true,
        "inactive" | "failed" => false,
        other => {
            return Err(format!(
                "uninstall service '{unit}' has transitional/unknown initial state '{other}'"
            ))
        }
    };
    Ok(ServicePriorState {
        unit: unit.into(),
        exec_start: exec_start.into(),
        timeout_ms,
        was_active,
    })
}

fn restart_originally_active_service(
    module: &str,
    entry: &ServicePriorState,
) -> Result<(), String> {
    // Revalidate the complete original authority, independently of the first
    // stop: never start a replacement service file or changed manager fragment.
    validate_service_name(module, &entry.unit)?;
    validate_service_exec_start(&entry.exec_start, module)?;
    let (uid, gid) = crate::runtime_identity::desktop_identity()?;
    if uid == 0 {
        return Err("cannot restore desktop service with UID 0".into());
    }
    let home = desktop_home(uid)?;
    let path = validated_unit_path(&home, uid, gid, &entry.unit)?;
    let meta = fs::symlink_metadata(&path)
        .map_err(|error| format!("cannot inspect recovery unit {}: {error}", entry.unit))?;
    if !meta.file_type().is_file()
        || meta.uid() != uid
        || meta.gid() != gid
        || meta.mode() & 0o002 != 0
    {
        return Err(format!(
            "recovery unit {} lost certified file owner/type/mode",
            entry.unit
        ));
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read recovery unit {}: {error}", entry.unit))?;
    validate_service_unit_text(&contents, &entry.exec_start, module)?;
    let (load, active) = systemd_state(uid, gid, &home, &entry.unit)?;
    if load != "loaded" {
        return Err(format!("recovery unit {} is not loaded", entry.unit));
    }
    validate_manager_fragment(uid, gid, &home, &entry.unit, &path, &entry.exec_start)?;
    if active == "active" {
        return Ok(());
    }
    if active != "inactive" && active != "failed" {
        return Err(format!(
            "recovery unit {} is in ambiguous state {active}",
            entry.unit
        ));
    }
    systemctl_as_desktop(uid, gid, &home, &["start", "--no-block", &entry.unit])?;
    let deadline = Instant::now() + Duration::from_millis(entry.timeout_ms);
    loop {
        let (_, state) = systemd_state(uid, gid, &home, &entry.unit)?;
        if state == "active" {
            return Ok(());
        }
        if state == "failed" {
            return Err(format!("recovery unit {} failed after start", entry.unit));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "recovery unit {} never reached active before deadline",
                entry.unit
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

// Only UninstallServiceRecovery may request this, after TXN V5 verifies
// the original installation is intact. The Rust adapter reauthenticates the
// exact unit and effective manager state before any systemctl start.
pub(crate) fn reactivate_certified_service(
    module: &str,
    unit: &str,
    exec_start: &str,
    timeout_ms: u64,
) -> Result<(), String> {
    restart_originally_active_service(
        module,
        &ServicePriorState {
            unit: unit.to_string(),
            exec_start: exec_start.to_string(),
            timeout_ms,
            was_active: true,
        },
    )
}

// Checked at pre-commit and again at retirement. No arbitrary startup
// directories, symlink following, foreign executables or foreign ownership.
fn inspect_owned_autostart(
    module: &str,
    name: &str,
    exec_start: &str,
    required: bool,
) -> Result<Option<PathBuf>, String> {
    validate_autostart_name(module, name)?;
    validate_service_exec_start(exec_start, module)?;
    let (uid, gid) = crate::runtime_identity::desktop_identity()?;
    if uid == 0 {
        return Err("uninstall cannot manage root XDG autostart".into());
    }
    let home = desktop_home(uid)?;
    let mut parent = home.clone();
    for part in [".config", "autostart"] {
        parent.push(part);
        match fs::symlink_metadata(&parent) {
            Ok(meta)
                if meta.file_type().is_dir()
                    && meta.uid() == uid
                    && meta.gid() == gid
                    && meta.mode() & 0o002 == 0 => {}
            Ok(_) => {
                return Err(format!(
                    "uninstall autostart parent not owned real directory: {}",
                    parent.display()
                ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if required {
                    return Err(format!("required autostart '{name}' is missing"));
                }
                return Ok(None);
            }
            Err(error) => return Err(format!("uninstall autostart parent inspection: {error}")),
        }
    }
    let path = parent.join(name);
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if required {
                return Err(format!("required autostart '{name}' is absent"));
            }
            return Ok(None);
        }
        Err(error) => return Err(format!("uninstall autostart inspection: {error}")),
    };
    if !meta.file_type().is_file()
        || meta.uid() != uid
        || meta.gid() != gid
        || meta.mode() & 0o002 != 0
    {
        return Err(format!(
            "uninstall autostart '{name}' owner/type/mode mismatch"
        ));
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("uninstall autostart read failed: {error}"))?;
    validate_autostart_text(&contents, exec_start)?;
    Ok(Some(path))
}

// Post-commit only: a successful uninstall may not retain a module-owned
// desktop autostart; the user Settings choice is deliberately not an input.
pub(crate) fn retire_owned_autostart(
    module: &str,
    name: &str,
    exec_start: &str,
    required: bool,
) -> Result<(), String> {
    if let Some(path) = inspect_owned_autostart(module, name, exec_start, required)? {
        fs::remove_file(&path).map_err(|e| format!("cannot retire owned autostart: {e}"))?;
        if fs::symlink_metadata(&path).is_ok() {
            return Err(format!(
                "uninstall autostart '{}' remains after retirement",
                name
            ));
        }
    }
    Ok(())
}

// A partially stopped service must never be restarted merely because its
// systemd unit is still present. The entire installed authority has to match
// the pre-quiescence evidence before any compensation can execute.
fn failed_extras_with_verified_recovery<V, R>(
    failure: &str,
    verify_original: V,
    reactivate: R,
) -> String
where
    V: FnOnce() -> Result<(), String>,
    R: FnOnce() -> Result<(), String>,
{
    match verify_original() {
        Err(integrity) => format!(
            "CRITICAL: uninstall extras failed: {failure}; original installation integrity not restored; services NOT restarted: {integrity}"
        ),
        Ok(()) => match reactivate() {
            Ok(()) => format!(
                "uninstall extras failed: {failure}; previously active services restored"
            ),
            Err(restore) => format!(
                "CRITICAL: uninstall extras failed: {failure}; {restore}"
            ),
        },
    }
}

pub fn run_extras(
    contract: &UninstallContract,
    module: &str,
    has_tray: bool,
    verify_original: impl Fn() -> Result<(), String>,
) -> Result<UninstallServiceRecovery, String> {
    // All declarations are validated before any process is stopped.
    validate(contract, module, has_tray)?;
    let mut recovery = UninstallServiceRecovery::default();
    for extra in &contract.extras {
        let result = match extra {
            Extra::Governed { .. } => {
                // The governed Tray is stopped by Boss's existing lifecycle.
                Ok(())
            }
            Extra::Autostart {
                name,
                exec_start,
                required,
                ..
            } => (|| -> Result<(), String> {
                // Pre-commit only validates; it never deletes on this path.
                // Keep resolution errors inside the result so prior service
                // quiescence is compensated rather than bypassed with `?`.
                inspect_owned_autostart(
                    module,
                    &resolve(name, contract, module)?,
                    &resolve(exec_start, contract, module)?,
                    *required,
                )?;
                Ok(())
            })(),
            Extra::Service {
                name,
                exec_start,
                required,
                stop,
                ..
            } => {
                (|| -> Result<(), String> {
                    let unit = resolve(name, contract, module)?;
                    let command = resolve(exec_start, contract, module)?;
                    let state =
                        capture_prior_service_state(module, &unit, &command, stop.timeout_ms)?;
                    // Register BEFORE issuing stop so a partial stop failure
                    // can also be compensated.
                    recovery.entries.push(state);
                    crate::module_uninstall_arsenal::systemd_user_service(
                        module, &unit, &command, *required, stop, false,
                    )
                })()
            }
        };
        if let Err(error) = result {
            return Err(failed_extras_with_verified_recovery(
                &error,
                || verify_original(),
                || recovery.reactivate(module),
            ));
        }
    }
    Ok(recovery)
}

// In post-commit retirement there is no safe rollback. Every independent
// resource must get its attempt even when its predecessor returns Err.
fn retire_independent_resources<T, F>(resources: &[T], mut retire: F) -> Result<(), String>
where
    F: FnMut(&T) -> Result<(), String>,
{
    let mut failures = Vec::new();
    for resource in resources {
        if let Err(error) = retire(resource) {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "CRITICAL: module startup resource retirement incomplete: {}",
            failures.join("; ")
        ))
    }
}

pub fn finalize_service_units(contract: &UninstallContract, module: &str) -> Result<(), String> {
    // Each closure call independently resolves declared strings and invokes
    // the private Governor/Arsenal adapter. A failed resolve() must NOT skip
    // the next systemd-user service or XDG autostart after the commit marker.
    retire_independent_resources(&contract.extras, |extra| -> Result<(), String> {
        let id = match extra {
            Extra::Governed { id, .. }
            | Extra::Service { id, .. }
            | Extra::Autostart { id, .. } => id,
        };
        let result = (|| -> Result<(), String> {
            match extra {
                Extra::Governed { .. } => Ok(()),
                Extra::Service {
                    name,
                    exec_start,
                    required,
                    stop,
                    ..
                } => crate::module_uninstall_arsenal::systemd_user_service(
                    module,
                    &resolve(name, contract, module)?,
                    &resolve(exec_start, contract, module)?,
                    *required,
                    stop,
                    true,
                ),
                Extra::Autostart {
                    name,
                    exec_start,
                    required,
                    ..
                } => crate::module_uninstall_arsenal::xdg_user_autostart(
                    module,
                    &resolve(name, contract, module)?,
                    &resolve(exec_start, contract, module)?,
                    *required,
                ),
            }
        })();
        result.map_err(|error| format!("resource '{id}': {error}"))
    })
}

pub fn cleanup_directories(
    contract: &UninstallContract,
    module: &str,
    remove_settings: bool,
    root: &Path,
) -> Result<Vec<PathBuf>, String> {
    let base = root.join("shared/data").join(module);
    // Reject symlinks in the entire declared authority root, not only in
    // the last path components. This remains safe when directories are absent.
    let mut parent = root.to_path_buf();
    for part in ["shared", "data", module] {
        parent.push(part);
        match fs::symlink_metadata(&parent) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "uninstall cleanup root symlink is forbidden: {}",
                    parent.display()
                ))
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "uninstall cleanup root is not a directory: {}",
                    parent.display()
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("could not inspect uninstall cleanup root: {error}")),
        }
    }
    let mut directories = Vec::new();
    for item in &contract.cleanup {
        if let Cleanup::Directory { path, policy, .. } = item {
            if policy == "user-choice" && !remove_settings {
                continue;
            }
            let relative = resolve(path, contract, module)?;
            relative_path(&relative)?;
            let mut current = base.clone();
            // Every existing parent component must be a real directory, never a symlink.
            for component in Path::new(&relative).components() {
                current.push(component.as_os_str());
                match fs::symlink_metadata(&current) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        return Err(format!(
                            "uninstall cleanup symlink is forbidden: {}",
                            current.display()
                        ))
                    }
                    Ok(metadata) if !metadata.is_dir() => {
                        return Err(format!(
                            "uninstall cleanup path is not a directory: {}",
                            current.display()
                        ))
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!("cannot inspect uninstall cleanup path: {error}"))
                    }
                }
            }
            directories.push(current);
        }
    }
    Ok(directories)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn example() -> UninstallContract {
        serde_json::from_str(r#"{
           "schema":1,"main":"governor", "hardcoded":{"unit":"neebles-test-module-session-controller.service", "cache":"cache"},
           "extras":[{"id":"tray","type":"governed","ref":"manifest.tray"},
                     {"id":"session","type":"service","provider":"systemd-user","name":"$hardcoded.unit", "exec_start":"/usr/bin/python3.13 /opt/neebles/modules/test-module/tray/session-controller.py", "stop":{"disable":true,"timeout_ms":5000},"verify":"inactive","remove":true}],
           "cleanup":[{"id":"cache","type":"directory","scope":"module-data","path":"$hardcoded.cache","policy":"always"}]
        }"#).unwrap()
    }
    #[test]
    fn extras_recovery_never_restarts_when_original_integrity_has_changed() {
        let restarted = std::cell::Cell::new(false);
        let failure = failed_extras_with_verified_recovery(
            "later declared resource failed",
            || Err("original manifest has changed".to_string()),
            || {
                restarted.set(true);
                Ok(())
            },
        );
        assert!(!restarted.get());
        assert!(failure.starts_with("CRITICAL:"));
        assert!(failure.contains("services NOT restarted"));
        assert!(failure.contains("original manifest has changed"));
    }

    #[test]
    fn extras_recovery_verifies_integrity_before_compensation() {
        let observed = std::cell::RefCell::new(Vec::new());
        let failure = failed_extras_with_verified_recovery(
            "autostart validation failed",
            || {
                observed.borrow_mut().push("verify-original");
                Ok(())
            },
            || {
                observed.borrow_mut().push("reactivate-service");
                Ok(())
            },
        );
        assert_eq!(
            *observed.borrow(),
            ["verify-original", "reactivate-service"]
        );
        assert!(failure.contains("autostart validation failed"));
        assert!(failure.contains("previously active services restored"));
        assert!(!failure.starts_with("CRITICAL:"));
    }

    #[test]
    fn extras_recovery_reports_compensation_failure_after_verified_integrity() {
        let failure = failed_extras_with_verified_recovery(
            "resource validation failed",
            || Ok(()),
            || Err("systemd refused restart".to_string()),
        );
        assert!(failure.starts_with("CRITICAL:"));
        assert!(failure.contains("systemd refused restart"));
    }

    #[test]
    fn validates_generic_manifest_and_hardcoded() {
        let contract = example();
        validate(&contract, "test-module", true).unwrap();
        assert_eq!(
            resolve("$hardcoded.cache", &contract, "test-module").unwrap(),
            "cache"
        );
        assert_eq!(
            resolve("$$hardcoded.cache", &contract, "test-module").unwrap(),
            "$hardcoded.cache"
        );
    }
    #[test]
    fn uninstall_services_must_always_be_removed_even_with_settings_preserved() {
        let mut contract = example();
        if let Extra::Service { remove, .. } = &mut contract.extras[1] {
            *remove = false;
        }
        assert!(validate(&contract, "test-module", true)
            .unwrap_err()
            .contains("remove=true"));
        let mut contract = example();
        if let Extra::Service { stop, .. } = &mut contract.extras[1] {
            stop.disable = false;
        }
        assert!(validate(&contract, "test-module", true)
            .unwrap_err()
            .contains("disable=true"));
        // The only user-choice branch is Cleanup::Settings / Cleanup::Directory.
        let contract = example();
        assert!(matches!(
            &contract.extras[1],
            Extra::Service { remove: true, .. }
        ));
        validate(&contract, "test-module", true).unwrap();
    }

    #[test]
    fn xdg_autostart_requires_module_owned_name_and_exec() {
        let mut contract = example();
        contract.extras.push(Extra::Autostart {
            id: "ui-startup".into(),
            provider: "xdg-user".into(),
            name: "neebles-test-module-ui.desktop".into(),
            exec_start: "/opt/neebles/modules/test-module/runtime.py".into(),
            required: false,
        });
        validate(&contract, "test-module", true).unwrap();
        let len = contract.extras.len();
        if let Extra::Autostart { name, .. } = &mut contract.extras[len - 1] {
            *name = "neebles-othermodule-ui.desktop".into();
        }
        assert!(validate(&contract, "test-module", true).is_err());
    }

    #[test]
    fn xdg_autostart_rejects_ambiguous_desktop_entries() {
        let exec = "/opt/neebles/modules/test-module/runtime.py";
        let valid = format!("[Desktop Entry]\nType=Application\nName=NEEBLES Test\nExec={exec}\n");
        validate_autostart_text(&valid, exec).unwrap();
        let duplicate = format!("{valid}Exec={exec}\n");
        assert!(validate_autostart_text(&duplicate, exec).is_err());
        let foreign = "[Desktop Entry]\nType=Application\nExec=/usr/bin/other-program\n";
        assert!(validate_autostart_text(foreign, exec).is_err());
    }

    #[test]
    fn recovery_only_restarts_previously_active_units_in_reverse_order() {
        let recovery = UninstallServiceRecovery {
            entries: vec![
                ServicePriorState {
                    unit: "first".into(),
                    exec_start: "a".into(),
                    timeout_ms: 100,
                    was_active: true,
                },
                ServicePriorState {
                    unit: "inactive".into(),
                    exec_start: "b".into(),
                    timeout_ms: 100,
                    was_active: false,
                },
                ServicePriorState {
                    unit: "last".into(),
                    exec_start: "c".into(),
                    timeout_ms: 100,
                    was_active: true,
                },
            ],
        };
        let mut calls = Vec::new();
        recovery
            .restore_with(|entry| {
                calls.push(entry.unit.clone());
                Ok(())
            })
            .unwrap();
        assert_eq!(calls, vec!["last".to_string(), "first".to_string()]);
    }

    #[test]
    fn recovery_failure_is_critical_and_does_not_skip_other_units() {
        let recovery = UninstallServiceRecovery {
            entries: vec![
                ServicePriorState {
                    unit: "one".into(),
                    exec_start: "a".into(),
                    timeout_ms: 100,
                    was_active: true,
                },
                ServicePriorState {
                    unit: "two".into(),
                    exec_start: "b".into(),
                    timeout_ms: 100,
                    was_active: true,
                },
            ],
        };
        let mut called = Vec::new();
        let error = recovery
            .restore_with(|entry| {
                called.push(entry.unit.clone());
                if entry.unit == "two" {
                    Err("injected start failure".into())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(called, vec!["two".to_string(), "one".to_string()]);
        assert!(error.starts_with("CRITICAL:"));
        assert!(error.contains("injected start failure"));
    }

    #[test]
    fn rejects_arbitrary_systemd_units() {
        let mut contract = example();
        contract.extras = vec![Extra::Service {
            id: "bad".into(),
            provider: "systemd-user".into(),
            name: "ssh.service".into(),
            exec_start: "/opt/neebles/modules/test-module/bin/app".into(),
            required: false,
            stop: ServiceStop::default(),
            verify: inactive(),
            remove: true,
        }];
        assert!(validate(&contract, "test-module", true).is_err());
    }
    #[test]
    fn rejects_unsafe_paths() {
        let mut contract = example();
        contract.cleanup = vec![Cleanup::Directory {
            id: "evil".into(),
            scope: "module-data".into(),
            path: "../../etc".into(),
            policy: "always".into(),
        }];
        assert!(validate(&contract, "test-module", true).is_err());
    }
    #[test]
    fn rejects_unmanaged_references() {
        let mut contract = example();
        contract.extras = vec![Extra::Governed {
            id: "extra".into(),
            reference: "python3".into(),
            required: false,
            verify: terminated(),
        }];
        assert!(validate(&contract, "test-module", true).is_err());
    }
    #[test]
    fn settings_policy_reuses_boss_choice() {
        let mut contract = example();
        contract.cleanup.push(Cleanup::Settings {
            id: "settings".into(),
            reference: "manifest.settings".into(),
            policy: "user-choice".into(),
        });
        validate(&contract, "test-module", true).unwrap();
    }
    #[test]
    fn optional_missing_tray_is_not_an_error() {
        validate(&example(), "test-module", false).unwrap();
    }
    #[test]
    fn rejects_fake_execstart_substring_and_stop_hooks() {
        let expected =
            "/usr/bin/python3.13 /opt/neebles/modules/test-module/tray/session-controller.py";
        let proper = format!(
            "[Unit]\nDescription=test\n[Service]\nExecStart={expected}\nRestart=on-failure\n"
        );
        validate_service_unit_text(&proper, expected, "test-module").unwrap();
        assert!(validate_service_unit_text(
            "[Service]\nExecStart=/bin/echo /tmp/evil --fake=/opt/neebles/modules/test-module/tray/session-controller.py\n",
            expected, "test-module").is_err());
        assert!(validate_service_unit_text(
            &format!("[Service]\nExecStart={expected}\nExecStop=/bin/true\n"),
            expected,
            "test-module"
        )
        .is_err());
        assert!(validate_service_exec_start(
            "/bin/sh -c /opt/neebles/modules/test-module/tray/session-controller.py",
            "test-module"
        )
        .is_err());
        assert!(validate_service_exec_start(
            "/usr/bin/python3.13 /opt/neebles/modules/test-module/../foreign.py",
            "test-module"
        )
        .is_err());
    }
    #[test]
    fn manager_must_match_loaded_exec_and_have_no_hooks() {
        let path =
            Path::new("/home/test/.config/systemd/user/neebles-test-module-controller.service");
        let expected =
            "/usr/bin/python3.13 /opt/neebles/modules/test-module/tray/session-controller.py";
        let manager = format!("FragmentPath={}\nDropInPaths=\nExecStart={{ path=/usr/bin/python3.13 ; argv[]={expected} ; ignore_errors=no ; start_time=[n/a] ; stop_time=[n/a] ; pid=0 ; code=(null) ; status=0/0 }}\nExecStop=\nExecStopPost=\nExecStartPre=\nExecStartPost=\nExecCondition=\n", path.display());
        validate_manager_state(&manager, path, expected).unwrap();
        assert!(validate_manager_state(
            &manager.replace("ExecStop=\n", "ExecStop=/bin/rm\n"),
            path,
            expected
        )
        .is_err());
        assert!(validate_manager_state(
            &manager.replace("DropInPaths=\n", "DropInPaths=/tmp/override.conf\n"),
            path,
            expected
        )
        .is_err());
        assert!(validate_manager_state(
            &manager.replace("argv[]=/usr/bin/python3.13", "argv[]=/bin/echo"),
            path,
            expected
        )
        .is_err());
    }
    #[test]
    fn committed_cleanup_failure_does_not_resurrect_partial_data() {
        let temp = tempfile::tempdir().unwrap();
        let original_first = temp.path().join("cache-first");
        let original_second = temp.path().join("cache-second");
        let staged_first = temp.path().join("staged-first");
        let staged_second = temp.path().join("staged-second");

        // The first detached directory can be removed successfully.
        fs::create_dir(&staged_first).unwrap();
        fs::write(staged_first.join("data.txt"), "committed").unwrap();
        // Simulate interference after staging: the second entry is now a file.
        // remove_dir_all on a regular file must fail without relying on UID,
        // chmod or a real NEEBLES runtime directory.
        fs::write(&staged_second, "BLOCKER").unwrap();

        let guard = StagedCleanup {
            moved: vec![
                (original_first.clone(), staged_first.clone()),
                (original_second.clone(), staged_second.clone()),
            ],
            finalized: false,
        };
        let error = guard.finalize().expect_err("second cleanup must fail");
        assert!(error.contains("cleanup is incomplete"), "{error}");
        assert!(!staged_first.exists(), "first cleanup must have committed");
        assert!(staged_second.is_file(), "failed entry stays in staging");
        assert!(
            !original_first.exists(),
            "partially deleted data must not reappear"
        );
        assert!(
            !original_second.exists(),
            "failed staged entry must not be restored"
        );
        assert_eq!(fs::read_to_string(staged_second).unwrap(), "BLOCKER");
    }

    #[test]
    fn committed_cleanup_continues_after_independent_errors() {
        let temp = tempfile::tempdir().unwrap();
        let original_first = temp.path().join("data-first");
        let original_middle = temp.path().join("data-middle");
        let original_last = temp.path().join("data-last");
        let staged_first = temp.path().join("staged-first");
        let staged_middle = temp.path().join("staged-middle");
        let staged_last = temp.path().join("staged-last");

        // Two deterministic failures with an independently removable directory
        // between them. This does not depend on chmod, root, or systemd.
        fs::write(&staged_first, "BLOCKER-FIRST").unwrap();
        fs::create_dir(&staged_middle).unwrap();
        fs::write(staged_middle.join("data.txt"), "committed").unwrap();
        fs::write(&staged_last, "BLOCKER-LAST").unwrap();

        let guard = StagedCleanup {
            moved: vec![
                (original_first.clone(), staged_first.clone()),
                (original_middle.clone(), staged_middle.clone()),
                (original_last.clone(), staged_last.clone()),
            ],
            finalized: false,
        };
        let error = guard.finalize().expect_err("both file blockers must fail");
        assert!(error.starts_with("CRITICAL:"), "{error}");
        assert!(
            error.contains(&staged_first.display().to_string()),
            "{error}"
        );
        assert!(
            error.contains(&staged_last.display().to_string()),
            "{error}"
        );
        assert!(
            !staged_middle.exists(),
            "independent directory must be removed"
        );
        assert!(staged_first.is_file(), "failed first entry remains staged");
        assert!(staged_last.is_file(), "failed last entry remains staged");
        for original in [original_first, original_middle, original_last] {
            assert!(
                !original.exists(),
                "post-commit data must never be restored"
            );
        }
    }

    #[test]
    fn committed_cleanup_unlinks_symlink_without_following_external_data() {
        let temp = tempfile::tempdir().unwrap();
        let original = temp.path().join("cache");
        let staged = temp.path().join("staged");
        let external = temp.path().join("external");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("keep.txt"), "unchanged").unwrap();
        std::os::unix::fs::symlink(&external, &staged).unwrap();
        let guard = StagedCleanup {
            moved: vec![(original.clone(), staged.clone())],
            finalized: false,
        };
        guard
            .finalize()
            .expect("symlink must be removed, not followed");
        assert!(!original.exists());
        assert!(fs::symlink_metadata(&staged).is_err());
        assert_eq!(
            fs::read_to_string(external.join("keep.txt")).unwrap(),
            "unchanged"
        );
    }
    #[test]
    fn staging_restores_data_on_abort() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let data = root.join("shared/data/test-module/cache");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("file.txt"), "USER_DATA").unwrap();
        let stage = root.join("shared/tmp");
        fs::create_dir_all(&stage).unwrap();
        let paths = cleanup_directories(&example(), "test-module", false, root).unwrap();
        {
            let _guard = stage_cleanup(&paths, &stage, "test-module").unwrap();
            assert!(!data.exists());
        }
        assert_eq!(
            fs::read_to_string(data.join("file.txt")).unwrap(),
            "USER_DATA"
        );
    }
    #[test]
    fn cleanup_rejects_symlink_components() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let data = root.join("shared/data/test-module");
        fs::create_dir_all(&data).unwrap();
        std::os::unix::fs::symlink("/tmp", data.join("cache")).unwrap();
        assert!(cleanup_directories(&example(), "test-module", false, root).is_err());
    }

    #[test]
    fn postcommit_retirement_attempts_all_resources_and_aggregates_failures() {
        let entries = ["first", "middle", "last"];
        let mut attempted = Vec::new();
        let failure = retire_independent_resources(&entries, |resource| {
            attempted.push(*resource);
            if *resource == "first" || *resource == "last" {
                Err(format!("injected {resource} failure"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(attempted, ["first", "middle", "last"]);
        assert!(failure.starts_with("CRITICAL:"));
        assert!(failure.contains("injected first failure"));
        assert!(failure.contains("injected last failure"));
    }

    #[test]
    fn postcommit_retirement_accepts_all_successful_resources() {
        let mut attempted = Vec::new();
        retire_independent_resources(&[1, 2, 3], |resource| {
            attempted.push(*resource);
            Ok(())
        })
        .unwrap();
        assert_eq!(attempted, [1, 2, 3]);
    }

    #[test]
    fn home_validator_rejects_symlink_and_world_writable_home() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        fs::create_dir(&home).unwrap();
        let uid = unsafe { libc::geteuid() };
        validate_desktop_home_path(&home, uid).unwrap();

        let linked = temp.path().join("home-link");
        symlink(&home, &linked).unwrap();
        assert!(validate_desktop_home_path(&linked, uid).is_err());

        fs::set_permissions(&home, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(validate_desktop_home_path(&home, uid).is_err());
    }
}

/* Stage module-data cleanup on the same filesystem as module staging.
 * If a later Boss uninstall step fails, Drop restores each existing path.
 * Missing optional paths are harmless and do not cause directory creation.
 */
pub struct StagedCleanup {
    moved: Vec<(PathBuf, PathBuf)>,
    finalized: bool,
}
impl Drop for StagedCleanup {
    fn drop(&mut self) {
        if !self.finalized {
            for (original, staged) in self.moved.iter().rev() {
                if staged.exists() && !original.exists() {
                    if let Err(error) = fs::rename(staged, original) {
                        eprintln!("N.E.E.B.L.E.S.: CRITICAL could not restore uninstall cleanup {} -> {}: {error}",
                                  staged.display(), original.display());
                    }
                }
            }
        }
    }
}
impl StagedCleanup {
    pub fn finalize(mut self) -> Result<(), String> {
        // COMMIT BOUNDARY: once a recursive delete begins it may have deleted
        // some files when an I/O error occurs. Never claim to restore such
        // partially erased directories to their active locations.
        self.finalized = true;
        let mut failures = Vec::new();
        for (_, staged) in &self.moved {
            if let Err(error) = fs::remove_dir_all(staged) {
                failures.push(format!("{}: {error}", staged.display()));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "CRITICAL: uninstall already committed; module-data staging cleanup is incomplete, manual recovery may be required: {}",
                failures.join("; ")
            ))
        }
    }
}

pub fn stage_cleanup(
    directories: &[PathBuf],
    staging_root: &Path,
    module: &str,
) -> Result<StagedCleanup, String> {
    for (i, first) in directories.iter().enumerate() {
        for second in &directories[i + 1..] {
            if first.starts_with(second) || second.starts_with(first) {
                return Err(format!(
                    "uninstall cleanup directories overlap: {} and {}",
                    first.display(),
                    second.display()
                ));
            }
        }
    }
    let mut staged = StagedCleanup {
        moved: Vec::new(),
        finalized: false,
    };
    for (index, path) in directories.iter().enumerate() {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "cannot inspect uninstall cleanup {}: {error}",
                    path.display()
                ))
            }
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(format!(
                "uninstall cleanup resource is not a real directory: {}",
                path.display()
            ));
        }
        let destination = staging_root.join(format!(
            "uninstall-data-{module}-{}-{index}",
            std::process::id()
        ));
        if destination.exists() {
            return Err(format!(
                "uninstall staging already exists: {}",
                destination.display()
            ));
        }
        fs::rename(path, &destination).map_err(|error| {
            format!(
                "could not stage uninstall cleanup {}: {error}",
                path.display()
            )
        })?;
        staged.moved.push((path.clone(), destination));
    }
    Ok(staged)
}

// F04B1: a *new* Update candidate can introduce a governed systemd-user
// service even when the currently installed version has none. The old Update
// transaction can roll back the module directory but cannot yet roll back a
// newly installed/enabled user unit. Reject that transition until the owned
// unit itself participates in publication, rollback and finalization.
//
// This is a declaration-only check over the already-validated staged
// ModuleManifest. No systemctl calls or filesystem side effects occur here.
pub(crate) fn candidate_declares_managed_user_service(
    module: &str,
    manifest: &crate::modules::ModuleManifest,
) -> Result<bool, String> {
    if manifest.schema != 4 || manifest.name != module {
        return Err(format!(
            "module '{module}' update candidate service identity does not match its manifest"
        ));
    }
    let Some(contract) = manifest.uninstall.as_ref() else {
        return Ok(false);
    };
    // Fail closed for malformed service ownership declarations rather than
    // interpreting an invalid uninstall contract as an empty declaration.
    validate(contract, module, manifest.tray.is_some())?;
    Ok(contract
        .extras
        .iter()
        .any(|extra| matches!(extra, Extra::Service { .. })))
}

#[cfg(test)]
mod f04b1_candidate_service_gate_tests {
    use super::candidate_declares_managed_user_service;
    use crate::modules::ModuleManifest;

    fn candidate(extra: Option<serde_json::Value>) -> ModuleManifest {
        let uninstall = extra.map(|item| {
            serde_json::json!({"schema":1, "extras":[item]})
        });
        serde_json::from_value(serde_json::json!({
            "schema": 4,
            "name": "neebles-secrets",
            "version": "1.0.0",
            "entrypoint": "bin/run",
            "uninstall": uninstall
        })).expect("test candidate must be a valid manifest shape")
    }

    #[test]
    fn ordinary_update_without_services_remains_eligible() {
        assert_eq!(candidate_declares_managed_user_service(
            "neebles-secrets", &candidate(None)
        ), Ok(false));
    }

    #[test]
    fn new_owned_service_is_blocked_before_update_publication() {
        let service = serde_json::json!({
            "type":"service", "id":"vault-daemon", "provider":"systemd-user",
            "name":"neebles-neebles-secrets-vault.service",
            "exec_start":"/opt/neebles/modules/neebles-secrets/bin/secrets-daemon",
            "required":true, "verify":"inactive", "remove":true,
            "stop":{"disable":true,"timeout_ms":5000}
        });
        assert_eq!(candidate_declares_managed_user_service(
            "neebles-secrets", &candidate(Some(service))
        ), Ok(true));
    }

    #[test]
    fn wrong_candidate_identity_fails_closed() {
        assert!(candidate_declares_managed_user_service(
            "neebles-connect", &candidate(None)
        ).is_err());
    }

    #[test]
    fn invalid_new_service_retirement_contract_fails_closed() {
        let service = serde_json::json!({
            "type":"service", "id":"vault-daemon", "provider":"systemd-user",
            "name":"neebles-neebles-secrets-vault.service",
            "exec_start":"/opt/neebles/modules/neebles-secrets/bin/secrets-daemon",
            "required":true, "verify":"inactive", "remove":false,
            "stop":{"disable":true,"timeout_ms":5000}
        });
        assert!(candidate_declares_managed_user_service(
            "neebles-secrets", &candidate(Some(service))
        ).is_err());
    }
}

// F04B2: read-only, exact-identity transition plan for governed user units.
// A separate follow-up will execute this plan transactionally. Until then
// update_with_observer rejects *every* plan that contains an owned unit.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ManagedServiceSpec {
    id: String,
    unit: String,
    exec_start: String,
    required: bool,
    timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum ManagedServiceTransition {
    Add,
    Retire,
    Keep,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedServiceStep {
    unit: String,
    transition: ManagedServiceTransition,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManagedServiceUpdatePlan {
    steps: Vec<ManagedServiceStep>,
}

impl ManagedServiceUpdatePlan {
    pub(crate) fn requires_governed_transaction(&self) -> bool {
        !self.steps.is_empty()
    }
}

fn declared_user_service_specs(
    module: &str,
    manifest: &crate::modules::ModuleManifest,
) -> Result<BTreeMap<String, ManagedServiceSpec>, String> {
    if manifest.schema != 4 || manifest.name != module {
        return Err(format!("service transition manifest identity mismatch for '{module}'"));
    }
    let Some(contract) = manifest.uninstall.as_ref() else {
        return Ok(BTreeMap::new());
    };
    validate(contract, module, manifest.tray.is_some())?;
    let mut result = BTreeMap::new();
    for extra in &contract.extras {
        let Extra::Service { id, provider, name, exec_start, required, stop, remove, verify } = extra else {
            continue;
        };
        if provider != "systemd-user" || !remove || !stop.disable || verify != "inactive" {
            return Err("untrusted service ownership during Update planning".into());
        }
        let unit = resolve(name, contract, module)?;
        let command = resolve(exec_start, contract, module)?;
        validate_service_name(module, &unit)?;
        validate_service_exec_start(&command, module)?;
        let spec = ManagedServiceSpec {
            id: id.clone(), unit: unit.clone(), exec_start: command,
            required: *required, timeout_ms: stop.timeout_ms,
        };
        if result.insert(unit.clone(), spec).is_some() {
            return Err(format!("duplicate governed unit name '{unit}'"));
        }
    }
    Ok(result)
}

fn plan_managed_service_changes(
    previous: &BTreeMap<String, ManagedServiceSpec>,
    proposed: &BTreeMap<String, ManagedServiceSpec>,
) -> Result<ManagedServiceUpdatePlan, String> {
    let names: BTreeSet<_> = previous.keys().chain(proposed.keys()).cloned().collect();
    let mut steps = Vec::with_capacity(names.len());
    for unit in names {
        let transition = match (previous.get(&unit), proposed.get(&unit)) {
            (Some(before), Some(after)) => {
                // A reused systemd unit cannot silently switch resource ownership.
                if before.id != after.id {
                    return Err(format!("service unit '{unit}' changed its declared resource identity"));
                }
                if before == after {
                    ManagedServiceTransition::Keep
                } else {
                    ManagedServiceTransition::Replace
                }
            }
            (Some(_), None) => ManagedServiceTransition::Retire,
            (None, Some(_)) => ManagedServiceTransition::Add,
            (None, None) => unreachable!("name comes from either map"),
        };
        steps.push(ManagedServiceStep { unit, transition });
    }
    Ok(ManagedServiceUpdatePlan { steps })
}

// Certify each *candidate* user unit from the pinned, staged module. This is
// a source inventory check, not a systemctl action or an authorization to run.
fn certify_staged_update_service_sources(
    candidate_root: &Path,
    module: &str,
    specs: &BTreeMap<String, ManagedServiceSpec>,
) -> Result<(), String> {
    if specs.is_empty() {
        return Ok(());
    }
    for folder in [candidate_root.join("integration"), candidate_root.join("integration/systemd-user")] {
        let meta = fs::symlink_metadata(&folder)
            .map_err(|e| format!("Update staged service directory {}: {e}", folder.display()))?;
        if !meta.file_type().is_dir() || meta.mode() & 0o022 != 0 {
            return Err(format!("Update staged service directory is not safe: {}", folder.display()));
        }
    }
    let unit_dir = candidate_root.join("integration/systemd-user");
    for spec in specs.values() {
        let path = unit_dir.join(&spec.unit);
        let meta = fs::symlink_metadata(&path)
            .map_err(|e| format!("Update staged unit '{}': {e}", spec.unit))?;
        if !meta.file_type().is_file() || meta.mode() & 0o022 != 0 || meta.len() > 8192 {
            return Err(format!("Update staged unit '{}' is not a safe regular file", spec.unit));
        }
        let text = fs::read_to_string(&path)
            .map_err(|e| format!("Update staged unit '{}' cannot be read: {e}", spec.unit))?;
        certify_install_unit(&text, module, &spec.exec_start)?;
    }
    Ok(())
}

pub(crate) fn plan_update_managed_user_services(
    module: &str,
    current: &crate::modules::ModuleManifest,
    candidate: &crate::modules::ModuleManifest,
    candidate_root: &Path,
) -> Result<ManagedServiceUpdatePlan, String> {
    let old = declared_user_service_specs(module, current)?;
    let new = declared_user_service_specs(module, candidate)?;
    certify_staged_update_service_sources(candidate_root, module, &new)?;
    plan_managed_service_changes(&old, &new)
}


// A registered runtime can itself be the systemd-user daemon that Update
// governs. Do not treat its live IPC session as an independent UI window.
// SO_PEERCRED and start ticks are already verified by Module IPC; here we
// additionally match its PID to an authenticated systemd MainPID.
pub(crate) fn is_managed_user_service_pid(module: &str, pid: u32) -> Result<bool, String> {
    if pid == 0 { return Ok(false); }
    let root = crate::modules::installed_module_dir(module)?;
    let manifest: crate::modules::ModuleManifest = serde_json::from_slice(
        &fs::read(root.join("manifest.json"))
            .map_err(|e| format!("read installed service manifest: {e}"))?,
    ).map_err(|e| format!("parse installed service manifest: {e}"))?;
    let specs = declared_user_service_specs(module, &manifest)?;
    if specs.is_empty() { return Ok(false); }
    let (uid, gid) = crate::runtime_identity::desktop_identity()?;
    if uid == 0 { return Err("cannot inspect a root desktop service".into()); }
    let home = desktop_home(uid)?;
    for spec in specs.values() {
        let observation = inspect_managed_update_unit_baseline(
            module, &spec.unit, Some(&spec.exec_start), spec.required,
        )?;
        if !matches!(observation, ManagedUpdateObservedUnit::Present { was_active: true, .. }) {
            continue;
        }
        let state = systemctl_as_desktop(uid, gid, &home,
            &["show", "--no-pager", "-p", "MainPID", &spec.unit])?;
        let main_pid = state.strip_prefix("MainPID=")
            .and_then(|v| v.trim().parse::<u32>().ok())
            .ok_or_else(|| format!("systemd MainPID is invalid for '{}'", spec.unit))?;
        if main_pid == pid { return Ok(true); }
    }
    Ok(false)
}

// Boss 2.0.3: productive Update bridge for module-owned systemd-user units.
// Reuses existing ownership validation, transition planning and TXN V5
// stop/start helpers. No module-specific branch and no new rollback engine.
// A user unit is independent of private module data and is never a vault.
pub(crate) struct ManagedServiceCutover {
    module: String,
    old: BTreeMap<String, ManagedServiceSpec>,
    new: BTreeMap<String, ManagedServiceSpec>,
    plan: ManagedServiceUpdatePlan,
    baseline: ManagedServiceUpdateBaseline,
    stopped: BTreeSet<String>,
}

pub(crate) fn prepare_managed_service_cutover(
    module: &str,
    current: &crate::modules::ModuleManifest,
    candidate: &crate::modules::ModuleManifest,
    candidate_root: &Path,
) -> Result<ManagedServiceCutover, String> {
    let old = declared_user_service_specs(module, current)?;
    let new = declared_user_service_specs(module, candidate)?;
    let mut plan = plan_update_managed_user_services(module, current, candidate, candidate_root)?;
    // Preserve the existing Update path for modules without user services:
    // it must not suddenly require a desktop manager or service home.
    if plan.steps.is_empty() {
        return Ok(ManagedServiceCutover {
            module: module.into(), old, new, plan,
            baseline: ManagedServiceUpdateBaseline::default(),
            stopped: BTreeSet::new(),
        });
    }
    let (uid, gid) = crate::runtime_identity::desktop_identity()?;
    if uid == 0 { return Err("Update cannot manage a root desktop user".into()); }
    let home = desktop_home(uid)?;
    // Authenticate the currently installed units before reading/comparing
    // their bytes with the staged candidate.
    let _verified_before_comparison = capture_managed_update_baseline(module, current, &plan)?;
    // A changed unit description/other certified field is Replace even when
    // its ExecStart and resource ID remain identical.
    for step in &mut plan.steps {
        if step.transition == ManagedServiceTransition::Keep {
            if matches!(_verified_before_comparison.previous.get(&step.unit),
                Some(ManagedUpdateObservedUnit::Absent)) { continue; }
            let path = validated_unit_path(&home, uid, gid, &step.unit)?;
            let current_bytes = fs::read(&path)
                .map_err(|e| format!("read current Update unit '{}': {e}", step.unit))?;
            let candidate_bytes = fs::read(candidate_root.join("integration/systemd-user").join(&step.unit))
                .map_err(|e| format!("read staged Update unit '{}': {e}", step.unit))?;
            if current_bytes != candidate_bytes {
                step.transition = ManagedServiceTransition::Replace;
            }
        }
    }
    let baseline = capture_managed_update_baseline(module, current, &plan)?;
    Ok(ManagedServiceCutover {
        module: module.into(), old, new, plan, baseline,
        stopped: BTreeSet::new(),
    })
}

impl ManagedServiceCutover {
    pub(crate) fn has_stopped_services(&self) -> bool { !self.stopped.is_empty() }

    // No service interruption until the candidate has been fully staged.
    // Unit files remain installed while old processes quiesce.
    pub(crate) fn quiesce(&mut self, current: &crate::modules::ModuleManifest) -> Result<(), String> {
        let observed = capture_managed_update_baseline(&self.module, current, &self.plan)?;
        if observed != self.baseline {
            return Err("managed service changed during Update preparation".into());
        }
        for step in &self.plan.steps {
            let Some(spec) = self.old.get(&step.unit) else { continue; };
            let state = self.baseline.previous.get(&step.unit)
                .ok_or_else(|| format!("missing service preflight '{}'", step.unit))?;
            let ManagedUpdateObservedUnit::Present { was_active: true, .. } = state else {
                continue;
            };
            let stop = ServiceStop { disable: false, timeout_ms: spec.timeout_ms };
            if let Err(error) = stop_service(&self.module, &step.unit, &spec.exec_start, spec.required, &stop, false) {
                // A partially stopped unit must be included in recovery.
                self.stopped.insert(step.unit.clone());
                let recovery = self.restore_original();
                return Err(match recovery {
                    Ok(()) => format!("cannot quiesce managed service '{}': {error}", step.unit),
                    Err(e) => format!("cannot quiesce managed service '{}': {error}; CRITICAL: original service restoration failed: {e}", step.unit),
                });
            }
            self.stopped.insert(step.unit.clone());
        }
        Ok(())
    }

    // Used ONLY when Boss's existing filesystem Update restored the previous
    // installation. Never start a previous process over a new module version.
    pub(crate) fn restore_original(&self) -> Result<(), String> {
        let mut errors = Vec::new();
        for unit in self.stopped.iter().rev() {
            if let Some(spec) = self.old.get(unit) {
                if let Err(e) = reactivate_certified_service(&self.module, unit, &spec.exec_start, spec.timeout_ms) {
                    errors.push(format!("{unit}: {e}"));
                }
            }
        }
        if errors.is_empty() { Ok(()) }
        else { Err(format!("CRITICAL: original services not restored: {}", errors.join("; "))) }
    }

    // This is called after the regular module/material/settings Update is
    // committed. A systemd failure is reported as a postcommit error; it is
    // never mislabeled as an undone module Update.
    pub(crate) fn complete(&self) -> Result<(), String> {
        for step in &self.plan.steps {
            let prior = self.baseline.previous.get(&step.unit);
            let was_active = matches!(prior,
                Some(ManagedUpdateObservedUnit::Present { was_active: true, .. }));
            let was_enabled = matches!(prior,
                Some(ManagedUpdateObservedUnit::Present { was_enabled: true, .. }));
            match step.transition {
                ManagedServiceTransition::Keep => {
                    if was_active {
                        let spec = self.old.get(&step.unit).ok_or("Keep lost old service")?;
                        reactivate_certified_service(&self.module, &step.unit, &spec.exec_start, spec.timeout_ms)?;
                    }
                }
                ManagedServiceTransition::Retire => {
                    if let Some(ManagedUpdateObservedUnit::Present { .. }) = prior {
                        let spec = self.old.get(&step.unit).ok_or("Retire lost old service")?;
                        let stop = ServiceStop { disable: true, timeout_ms: spec.timeout_ms };
                        stop_service(&self.module, &step.unit, &spec.exec_start, spec.required, &stop, true)?;
                    }
                }
                ManagedServiceTransition::Add | ManagedServiceTransition::Replace => {
                    let spec = self.new.get(&step.unit).ok_or("new service absent from declared plan")?;
                    if step.transition == ManagedServiceTransition::Add
                        || matches!(prior, Some(ManagedUpdateObservedUnit::Absent))
                    {
                        install_declared_user_service(&self.module, &spec.id)?;
                    } else {
                        replace_owned_update_service(&self.module, spec)?;
                        let (uid, gid) = crate::runtime_identity::desktop_identity()?;
                        let home = desktop_home(uid)?;
                        // Preserve the user's prior enablement and runtime
                        // state, not merely the candidate unit's existence.
                        let state = systemctl_as_desktop(uid, gid, &home,
                            &["show", "--no-pager", "-p", "UnitFileState", &step.unit])?;
                        let enabled = parse_unit_file_state(&state)? == "enabled";
                        if enabled != was_enabled {
                            if was_enabled {
                                systemctl_as_desktop(uid, gid, &home, &["enable", &step.unit])?;
                            } else {
                                systemctl_as_desktop(uid, gid, &home, &["disable", &step.unit])?;
                            }
                        }
                        if was_active {
                            reactivate_certified_service(&self.module, &step.unit, &spec.exec_start, spec.timeout_ms)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

// Atomically publish a certified candidate unit. This never follows an
// existing target symlink and never writes into an unrelated service path.
fn replace_owned_update_service(module: &str, spec: &ManagedServiceSpec) -> Result<(), String> {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let (uid, gid) = crate::runtime_identity::desktop_identity()?;
    if uid == 0 { return Err("cannot replace service for root desktop user".into()); }
    let home = desktop_home(uid)?;
    let destination = validated_unit_path(&home, uid, gid, &spec.unit)?;
    let old_meta = fs::symlink_metadata(&destination)
        .map_err(|e| format!("Update installed unit metadata '{}': {e}", spec.unit))?;
    if !old_meta.file_type().is_file() || old_meta.uid() != uid || old_meta.gid() != gid
        || old_meta.mode() & 0o022 != 0 || old_meta.len() > 8192
    {
        return Err(format!("Update refuses foreign unit '{}'", spec.unit));
    }
    let source = PathBuf::from(format!("/opt/neebles/modules/{module}/integration/systemd-user/{}", spec.unit));
    let content = fs::read_to_string(&source)
        .map_err(|e| format!("Update new unit '{}': {e}", spec.unit))?;
    certify_install_unit(&content, module, &spec.exec_start)?;
    let tmp = destination.with_file_name(format!(".{}.neebles-update-{}", spec.unit, std::process::id()));
    let result = (|| -> Result<(), String> {
        let mut f = fs::OpenOptions::new().write(true).create_new(true)
            .mode(0o600).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&tmp).map_err(|e| format!("create staged unit: {e}"))?;
        if unsafe { libc::fchown(f.as_raw_fd(), uid, gid) } != 0 {
            return Err(format!("chown staged unit: {}", std::io::Error::last_os_error()));
        }
        f.write_all(content.as_bytes()).and_then(|_| f.sync_all())
            .map_err(|e| format!("write staged unit: {e}"))?;
        // Refuse a replaced path between preflight and commit.
        let now = fs::symlink_metadata(&destination)
            .map_err(|e| format!("restat installed unit: {e}"))?;
        if now.ino() != old_meta.ino() || now.dev() != old_meta.dev()
            || now.uid() != old_meta.uid() || !now.file_type().is_file()
        {
            return Err("Update target unit changed during publication".into());
        }
        fs::rename(&tmp, &destination).map_err(|e| format!("publish new unit: {e}"))?;
        systemctl_as_desktop(uid, gid, &home, &["daemon-reload"])?;
        validate_manager_fragment(uid, gid, &home, &spec.unit, &destination, &spec.exec_start)?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&tmp); }
    result
}

#[cfg(test)]
mod f04b2_service_transition_plan_tests {
    use super::*;

    fn spec(id: &str, unit: &str, cmd: &str) -> ManagedServiceSpec {
        ManagedServiceSpec {
            id: id.into(), unit: unit.into(), exec_start: cmd.into(),
            required: true, timeout_ms: 5000,
        }
    }

    #[test]
    fn a_clean_update_without_units_has_no_managed_steps() {
        let plan = plan_managed_service_changes(&BTreeMap::new(), &BTreeMap::new()).unwrap();
        assert!(!plan.requires_governed_transaction());
    }

    #[test]
    fn the_plan_identifies_added_and_retired_units() {
        let old = BTreeMap::from([("neebles-demo-old.service".into(),
            spec("old", "neebles-demo-old.service", "/opt/neebles/modules/demo/old"))]);
        let new = BTreeMap::from([("neebles-demo-new.service".into(),
            spec("new", "neebles-demo-new.service", "/opt/neebles/modules/demo/new"))]);
        let plan = plan_managed_service_changes(&old, &new).unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].transition, ManagedServiceTransition::Add);
        assert_eq!(plan.steps[1].transition, ManagedServiceTransition::Retire);
    }

    #[test]
    fn the_plan_differentiates_kept_from_reconfigured_units() {
        let unit = "neebles-demo-daemon.service";
        let unchanged = spec("daemon", unit, "/opt/neebles/modules/demo/bin/v1");
        let mut changed = unchanged.clone();
        changed.exec_start = "/opt/neebles/modules/demo/bin/v2".into();
        let old = BTreeMap::from([(unit.into(), unchanged.clone())]);
        let stable = BTreeMap::from([(unit.into(), unchanged)]);
        let changed = BTreeMap::from([(unit.into(), changed)]);
        assert_eq!(plan_managed_service_changes(&old, &stable).unwrap().steps[0].transition,
                   ManagedServiceTransition::Keep);
        assert_eq!(plan_managed_service_changes(&old, &changed).unwrap().steps[0].transition,
                   ManagedServiceTransition::Replace);
    }

    #[test]
    fn reused_systemd_unit_with_new_resource_identity_is_rejected() {
        let unit = "neebles-demo-daemon.service";
        let old = BTreeMap::from([(unit.into(), spec("old", unit, "/opt/neebles/modules/demo/bin/v1"))]);
        let new = BTreeMap::from([(unit.into(), spec("new", unit, "/opt/neebles/modules/demo/bin/v2"))]);
        assert!(plan_managed_service_changes(&old, &new).is_err());
    }

    #[test]
    fn manifest_from_other_module_is_not_an_owned_plan() {
        let other: crate::modules::ModuleManifest = serde_json::from_value(serde_json::json!({
            "schema": 4, "name":"neebles-connect", "version":"1.0.0", "entrypoint":"bin/run"
        })).unwrap();
        assert!(declared_user_service_specs("neebles-secrets", &other).is_err());
    }

    #[test]
    fn staged_unit_validation_rejects_missing_source_file() {
        let spec = spec("vault", "neebles-neebles-secrets-vault.service",
                        "/opt/neebles/modules/neebles-secrets/bin/vault");
        let specs = BTreeMap::from([(spec.unit.clone(), spec)]);
        assert!(certify_staged_update_service_sources(
            Path::new("/nonexistent-neebles-staged-unit"), "neebles-secrets", &specs
        ).is_err());
    }
}

// F04B3: CERTIFIED, READ-ONLY BASELINE FOR FUTURE SERVICE UPDATE TXN.
// The transition planner cannot authorize a cutover on its own. Before the
// first stop it will need this immutable receipt of the old unit's state.
// This implementation does NOT stop, start, enable, disable or replace units.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ManagedUpdateObservedUnit {
    Absent,
    Present {
        was_active: bool,
        was_enabled: bool,
        unit_sha256: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ManagedServiceUpdateBaseline {
    previous: BTreeMap<String, ManagedUpdateObservedUnit>,
    reserved_new_units: BTreeSet<String>,
}

// Reject manager conditions which a future rollback cannot reproduce exactly.
// In particular an old service in a failed or transitional state MUST NOT be
// silently treated as an inactive service. Unknown UnitFileState is likewise
// not equivalent to disabled; even an inactive service can be enabled.
fn classify_managed_update_observation(
    has_owned_file: bool,
    required: bool,
    load: &str,
    active: &str,
    unit_file_state: Option<&str>,
    sha: Option<&str>,
) -> Result<ManagedUpdateObservedUnit, String> {
    if !has_owned_file {
        if load != "not-found" || active != "inactive"
            || unit_file_state.is_some() || sha.is_some()
        {
            return Err("Update service absent on disk but present in manager/receipt".into());
        }
        if required {
            return Err("required Update service unit is absent".into());
        }
        return Ok(ManagedUpdateObservedUnit::Absent);
    }
    if load != "loaded" {
        return Err(format!("Update unit has unsafe LoadState '{load}'"));
    }
    let was_active = match active {
        "active" => true,
        "inactive" => false,
        other => return Err(format!("Update cannot preserve initial ActiveState '{other}'")),
    };
    let was_enabled = match unit_file_state {
        Some("enabled") => true,
        Some("disabled") => false,
        other => return Err(format!("Update cannot preserve UnitFileState {other:?}")),
    };
    let digest = sha.ok_or("Update service receipt missing unit fingerprint")?;
    if digest.len() != 64 || !digest.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Update service receipt has invalid SHA-256".into());
    }
    Ok(ManagedUpdateObservedUnit::Present {
        was_active,
        was_enabled,
        unit_sha256: digest.to_string(),
    })
}

fn parse_unit_file_state(show: &str) -> Result<&str, String> {
    let mut lines = show.lines();
    let line = lines.next().ok_or("empty systemd UnitFileState response")?;
    if lines.next().is_some() {
        return Err("ambiguous systemd UnitFileState response".into());
    }
    line.strip_prefix("UnitFileState=")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "invalid systemd UnitFileState response".into())
}

// Use the same owner, unit-file, effective-manager and ExecStart validators
// as the private TXN V5 uninstall artillery. An Add must prove its target slot
// EMPTY, including manager cache, or it could replace a foreign user unit.
fn inspect_managed_update_unit_baseline(
    module: &str,
    unit: &str,
    exec_start: Option<&str>,
    required: bool,
) -> Result<ManagedUpdateObservedUnit, String> {
    use sha2::{Digest, Sha256};
    validate_service_name(module, unit)?;
    if let Some(command) = exec_start {
        validate_service_exec_start(command, module)?;
    }
    let (uid, gid) = crate::runtime_identity::desktop_identity()?;
    if uid == 0 {
        return Err("cannot inspect managed service for root desktop UID".into());
    }
    let home = desktop_home(uid)?;
    let path = validated_unit_path(&home, uid, gid, unit)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(meta) => Some(meta),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("managed Update unit '{unit}' metadata: {error}")),
    };
    let (load, active) = systemd_state(uid, gid, &home, unit)?;
    let Some(meta) = metadata else {
        return classify_managed_update_observation(
            false, required, &load, &active, None, None,
        );
    };
    // For a brand-new unit, ANY existing file is foreign, even if it happens
    // to share the declared name. No implicit takeover of an existing unit.
    let command = exec_start.ok_or_else(|| format!(
        "Update attempted to take over pre-existing user unit '{unit}'"
    ))?;
    if !meta.file_type().is_file() || meta.uid() != uid || meta.gid() != gid
        || meta.mode() & 0o022 != 0 || meta.len() > 8192
    {
        return Err(format!("managed Update unit '{unit}' is not an owned regular file"));
    }
    let bytes = fs::read(&path)
        .map_err(|error| format!("managed Update unit '{unit}' read: {error}"))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| format!("managed Update unit '{unit}' is not UTF-8"))?;
    validate_service_unit_text(text, command, module)?;
    certify_install_unit(text, module, command)?;
    validate_manager_fragment(uid, gid, &home, unit, &path, command)?;
    let unit_file_state = systemctl_as_desktop(
        uid, gid, &home,
        &["show", "--no-pager", "-p", "UnitFileState", unit],
    )?;
    let unit_file_state = parse_unit_file_state(&unit_file_state)?;
    // A changed manager fragment or unit during the probe invalidates the
    // receipt. This is still only a preflight; future cutover must recheck.
    let final_metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("managed Update unit '{unit}' changed: {error}"))?;
    if !final_metadata.file_type().is_file()
        || final_metadata.ino() != meta.ino() || final_metadata.dev() != meta.dev()
        || final_metadata.uid() != meta.uid() || final_metadata.gid() != meta.gid()
        || final_metadata.mode() != meta.mode() || final_metadata.len() != meta.len()
    {
        return Err(format!("managed Update unit '{unit}' changed during inspection"));
    }
    let digest = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    classify_managed_update_observation(
        true, required, &load, &active, Some(unit_file_state), Some(&digest),
    )
}

// Capture OLD state and reserve NEW target names before any mutation. Kept
// units must retain the same resource ID as guaranteed by the F04B2 planner.
// This receipt is intentionally ephemeral; no module can publish or edit it.
fn capture_managed_update_baseline(
    module: &str,
    current: &crate::modules::ModuleManifest,
    plan: &ManagedServiceUpdatePlan,
) -> Result<ManagedServiceUpdateBaseline, String> {
    let old = declared_user_service_specs(module, current)?;
    let mut baseline = ManagedServiceUpdateBaseline::default();
    for step in &plan.steps {
        if step.transition == ManagedServiceTransition::Add {
            if old.contains_key(&step.unit) {
                return Err(format!("invalid Add on already-owned unit '{}'", step.unit));
            }
            if inspect_managed_update_unit_baseline(module, &step.unit, None, false)?
                != ManagedUpdateObservedUnit::Absent
            {
                return Err(format!("new Update service slot '{}' is occupied", step.unit));
            }
            baseline.reserved_new_units.insert(step.unit.clone());
        } else {
            let spec = old.get(&step.unit).ok_or_else(|| format!(
                "Update transition lacks old unit declaration '{}'", step.unit
            ))?;
            let state = inspect_managed_update_unit_baseline(
                module, &spec.unit, Some(&spec.exec_start), spec.required,
            )?;
            baseline.previous.insert(step.unit.clone(), state);
        }
    }
    if baseline.previous.len() != old.len() {
        return Err("Update transition omitted an installed service".into());
    }
    Ok(baseline)
}

#[cfg(test)]
mod f04b3_managed_service_baseline_tests {
    use super::*;

    fn sha() -> &'static str {
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
    }

    #[test]
    fn active_enabled_service_is_recorded_exactly() {
        assert_eq!(classify_managed_update_observation(
            true, true, "loaded", "active", Some("enabled"), Some(sha()),
        ).unwrap(), ManagedUpdateObservedUnit::Present {
            was_active: true, was_enabled: true, unit_sha256: sha().into(),
        });
    }

    #[test]
    fn inactive_disabled_service_is_not_auto_started_on_rollback() {
        assert_eq!(classify_managed_update_observation(
            true, false, "loaded", "inactive", Some("disabled"), Some(sha()),
        ).unwrap(), ManagedUpdateObservedUnit::Present {
            was_active: false, was_enabled: false, unit_sha256: sha().into(),
        });
    }

    #[test]
    fn optional_absent_service_is_recorded_as_absent() {
        assert_eq!(classify_managed_update_observation(
            false, false, "not-found", "inactive", None, None,
        ).unwrap(), ManagedUpdateObservedUnit::Absent);
    }

    #[test]
    fn rejects_foreign_manager_and_missing_required_file() {
        assert!(classify_managed_update_observation(
            false, false, "loaded", "active", None, None,
        ).is_err());
        assert!(classify_managed_update_observation(
            false, true, "not-found", "inactive", None, None,
        ).is_err());
    }

    #[test]
    fn rejects_failed_and_transitional_initial_states() {
        for active in ["failed", "activating", "deactivating", "reloading", "unknown"] {
            assert!(classify_managed_update_observation(
                true, false, "loaded", active, Some("enabled"), Some(sha()),
            ).is_err());
        }
    }

    #[test]
    fn refuses_ambiguous_disabled_or_masked_unit_states() {
        for state in [None, Some("static"), Some("masked"), Some("enabled-runtime"), Some("")] {
            assert!(classify_managed_update_observation(
                true, true, "loaded", "active", state, Some(sha()),
            ).is_err());
        }
    }

    #[test]
    fn refuses_fake_or_missing_sha256_receipts() {
        for digest in [None, Some("0000"), Some("not-hex"), Some("")] {
            assert!(classify_managed_update_observation(
                true, true, "loaded", "active", Some("enabled"), digest,
            ).is_err());
        }
    }

    #[test]
    fn systemd_unit_state_must_be_exactly_one_field() {
        assert_eq!(parse_unit_file_state("UnitFileState=enabled"), Ok("enabled"));
        assert!(parse_unit_file_state("UnitFileState=enabled\nUnitFileState=disabled").is_err());
        assert!(parse_unit_file_state("UnitFileState=").is_err());
        assert!(parse_unit_file_state("ActiveState=enabled").is_err());
    }
}

// F04B4: IMMUTABLE RECOVERY RECEIPT FOR A FUTURE GOVERNED UPDATE TXN.
// This does not execute any operation or authorize service mutation. It
// records original bytes, owner-validated mode and exact startup states.
// A later txn executor MUST keep a separate mutation journal and certify
// each newly created unit before attempting to remove it on rollback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum ManagedUpdateRollbackTarget {
    // A candidate addition had NO unit in the old state. A future rollback
    // may remove only a unit proven to have been CREATED by that same txn.
    InitiallyVacant,
    // An optional old declaration had no unit; do not invent it on rollback.
    OriginallyAbsent,
    OriginalUnit {
        bytes: Vec<u8>,
        sha256: String,
        mode: u32,
        was_active: bool,
        was_enabled: bool,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedServiceRollbackReceipt {
    targets: BTreeMap<String, ManagedUpdateRollbackTarget>,
}

// Make an unambiguous recovery receipt from a certified baseline and the
// exact old unit data. Pure function: no systemctl, no file writes.
fn build_managed_service_rollback_receipt(
    specs: &BTreeMap<String, ManagedServiceSpec>,
    plan: &ManagedServiceUpdatePlan,
    baseline: &ManagedServiceUpdateBaseline,
    original_files: &BTreeMap<String, (Vec<u8>, u32)>,
) -> Result<ManagedServiceRollbackReceipt, String> {
    use sha2::{Digest, Sha256};
    let mut receipt = ManagedServiceRollbackReceipt::default();
    let mut seen = BTreeSet::new();
    let mut expected_new = BTreeSet::new();
    for step in &plan.steps {
        if !seen.insert(step.unit.clone()) {
            return Err(format!("duplicate Update rollback step '{}'", step.unit));
        }
        let target = match step.transition {
            ManagedServiceTransition::Add => {
                if specs.contains_key(&step.unit) || baseline.previous.contains_key(&step.unit)
                    || original_files.contains_key(&step.unit)
                    || !baseline.reserved_new_units.contains(&step.unit)
                {
                    return Err(format!("unsafe rollback receipt for new unit '{}'", step.unit));
                }
                expected_new.insert(step.unit.clone());
                ManagedUpdateRollbackTarget::InitiallyVacant
            }
            ManagedServiceTransition::Keep
            | ManagedServiceTransition::Replace
            | ManagedServiceTransition::Retire => {
                let spec = specs.get(&step.unit).ok_or_else(|| format!(
                    "rollback receipt misses old unit declaration '{}'", step.unit
                ))?;
                if spec.unit != step.unit {
                    return Err("rollback receipt old unit identifier mismatch".into());
                }
                match baseline.previous.get(&step.unit).ok_or_else(|| format!(
                    "rollback receipt lacks baseline for '{}'", step.unit
                ))? {
                    ManagedUpdateObservedUnit::Absent => {
                        if spec.required || original_files.contains_key(&step.unit) {
                            return Err(format!("unsafe absent old unit '{}'", step.unit));
                        }
                        ManagedUpdateRollbackTarget::OriginallyAbsent
                    }
                    ManagedUpdateObservedUnit::Present {
                        was_active, was_enabled, unit_sha256,
                    } => {
                        let (bytes, mode) = original_files.get(&step.unit).ok_or_else(|| format!(
                            "rollback receipt missing original bytes for '{}'", step.unit
                        ))?;
                        if bytes.is_empty() || bytes.len() > 8192 || (*mode & !0o777) != 0
                            || (*mode & 0o022) != 0 || (*mode & 0o400) == 0
                        {
                            return Err(format!("unsafe original unit bytes/mode '{}'", step.unit));
                        }
                        let digest = Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
                        if digest != *unit_sha256 {
                            return Err(format!("original unit '{}' changed since baseline", step.unit));
                        }
                        ManagedUpdateRollbackTarget::OriginalUnit {
                            bytes: bytes.clone(), sha256: digest, mode: *mode,
                            was_active: *was_active, was_enabled: *was_enabled,
                        }
                    }
                }
            }
        };
        receipt.targets.insert(step.unit.clone(), target);
    }
    let expected_old: BTreeSet<String> = specs.keys().cloned().collect();
    let old_seen: BTreeSet<String> = seen.difference(&expected_new).cloned().collect();
    if old_seen != expected_old
        || baseline.previous.keys().cloned().collect::<BTreeSet<_>>() != expected_old
        || original_files.keys().any(|unit| !expected_old.contains(unit))
        || expected_new != baseline.reserved_new_units
    {
        return Err("rollback receipt does not exactly cover the installed services".into());
    }
    Ok(receipt)
}

// The initial state and original bytes must be certified together. Calling
// F04B3 inspection both before and after the bounded file read rejects a
// changed manager state, file fingerprint, wrong owner and foreign unit.
// Future cutover MUST repeat this preflight at its actual mutation boundary.
fn capture_managed_update_rollback_receipt(
    module: &str,
    current: &crate::modules::ModuleManifest,
    plan: &ManagedServiceUpdatePlan,
) -> Result<ManagedServiceRollbackReceipt, String> {
    use sha2::{Digest, Sha256};
    let specs = declared_user_service_specs(module, current)?;
    let baseline = capture_managed_update_baseline(module, current, plan)?;
    let (uid, gid) = crate::runtime_identity::desktop_identity()?;
    if uid == 0 {
        return Err("cannot capture managed services for root desktop UID".into());
    }
    let home = desktop_home(uid)?;
    let mut original_files = BTreeMap::new();
    for (unit, state) in &baseline.previous {
        let ManagedUpdateObservedUnit::Present { unit_sha256, .. } = state else {
            continue;
        };
        let spec = specs.get(unit).ok_or("rollback capture lacks installed unit declaration")?;
        // Re-check with service owner/fragment/ExecStart validators, not just
        // the filename; reads must only be from the desktop owner's unit dir.
        let before = inspect_managed_update_unit_baseline(
            module, unit, Some(&spec.exec_start), spec.required,
        )?;
        if &before != state {
            return Err(format!("managed unit '{}' changed before recovery read", unit));
        }
        let path = validated_unit_path(&home, uid, gid, unit)?;
        let first = fs::symlink_metadata(&path)
            .map_err(|e| format!("rollback receipt metadata '{}': {e}", unit))?;
        if !first.file_type().is_file() || first.uid() != uid || first.gid() != gid
            || first.mode() & 0o022 != 0 || first.len() > 8192
        {
            return Err(format!("rollback receipt old unit '{}' is not owner-safe", unit));
        }
        let bytes = fs::read(&path)
            .map_err(|e| format!("rollback receipt read '{}': {e}", unit))?;
        let second = fs::symlink_metadata(&path)
            .map_err(|e| format!("rollback receipt second metadata '{}': {e}", unit))?;
        if !second.file_type().is_file() || first.ino() != second.ino()
            || first.dev() != second.dev() || first.uid() != second.uid()
            || first.gid() != second.gid() || first.mode() != second.mode()
            || first.len() != second.len() || bytes.len() > 8192
        {
            return Err(format!("rollback receipt old unit '{}' changed during read", unit));
        }
        let digest = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        if &digest != unit_sha256 {
            return Err(format!("rollback receipt unit '{}' digest changed", unit));
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| format!("rollback receipt unit '{}' is not UTF-8", unit))?;
        certify_install_unit(text, module, &spec.exec_start)?;
        let after = inspect_managed_update_unit_baseline(
            module, unit, Some(&spec.exec_start), spec.required,
        )?;
        if &after != state {
            return Err(format!("managed unit '{}' changed after recovery read", unit));
        }
        original_files.insert(unit.clone(), (bytes, first.mode() & 0o777));
    }
    // A new slot must stay vacant while the receipt is being collected.
    for unit in &baseline.reserved_new_units {
        if inspect_managed_update_unit_baseline(module, unit, None, false)?
            != ManagedUpdateObservedUnit::Absent
        {
            return Err(format!("new Update unit slot '{}' changed during snapshot", unit));
        }
    }
    build_managed_service_rollback_receipt(&specs, plan, &baseline, &original_files)
}

// F04B8: PURE TWO-PHASE JOURNAL FOR THE FUTURE OWNED USER-SERVICE UPDATE TXN.
// This is NOT an executor and grants NO permission to call systemd or change
// unit files. The future executor must durably record intent BEFORE attempting
// an effect, verify ownership/state AFTER it, and only then confirm the entry.
// An unfinished intent is AMBIGUOUS (the effect may have partially succeeded).
// It must NOT be mistaken for a no-op or compensated destructively on faith.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum ManagedUpdateForwardOp {
    Stop,
    Disable,
    WriteUnit,
    RemoveUnit,
    ManagerReload,
    Enable,
    Start,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedUpdateJournalEntry {
    unit: String,
    operation: ManagedUpdateForwardOp,
    verified: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedServiceMutationJournal {
    entries: Vec<ManagedUpdateJournalEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ManagedUpdateCompensationTarget {
    // Never remove an unverified creation, a foreign unit or an unknown slot.
    RemoveVerifiedCreation,
    // Restoration data comes ONLY from F04B4's original certified receipt.
    RestoreOriginal {
        bytes: Vec<u8>, sha256: String, mode: u32,
        was_active: bool, was_enabled: bool,
    },
}

fn check_managed_journal_contract(
    plan: &ManagedServiceUpdatePlan,
    receipt: &ManagedServiceRollbackReceipt,
) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for step in &plan.steps {
        if !names.insert(step.unit.clone()) {
            return Err("duplicate managed service transition in journal contract".into());
        }
        let target = receipt.targets.get(&step.unit)
            .ok_or("missing certified service receipt for planned unit")?;
        match (step.transition, target) {
            (ManagedServiceTransition::Add, ManagedUpdateRollbackTarget::InitiallyVacant)
            | (ManagedServiceTransition::Keep, ManagedUpdateRollbackTarget::OriginalUnit { .. })
            | (ManagedServiceTransition::Keep, ManagedUpdateRollbackTarget::OriginallyAbsent)
            | (ManagedServiceTransition::Replace, ManagedUpdateRollbackTarget::OriginalUnit { .. })
            | (ManagedServiceTransition::Replace, ManagedUpdateRollbackTarget::OriginallyAbsent)
            | (ManagedServiceTransition::Retire, ManagedUpdateRollbackTarget::OriginalUnit { .. })
            | (ManagedServiceTransition::Retire, ManagedUpdateRollbackTarget::OriginallyAbsent) => {}
            _ => return Err("managed service plan and immutable rollback receipt disagree".into()),
        }
    }
    if receipt.targets.keys().cloned().collect::<BTreeSet<_>>() != names {
        return Err("managed service recovery receipt has unplanned unit".into());
    }
    Ok(())
}

impl ManagedServiceMutationJournal {
    fn has_verified(&self, unit: &str, action: ManagedUpdateForwardOp) -> bool {
        self.entries.iter().any(|entry|
            entry.unit == unit && entry.operation == action && entry.verified)
    }

    // Only a certified, uniquely planned unit may acquire a mutation intent.
    // Never begin another operation while any preceding intent is unverified.
    fn begin(
        &mut self,
        plan: &ManagedServiceUpdatePlan,
        receipt: &ManagedServiceRollbackReceipt,
        unit: &str,
        action: ManagedUpdateForwardOp,
    ) -> Result<(), String> {
        check_managed_journal_contract(plan, receipt)?;
        if self.entries.iter().any(|entry| !entry.verified) {
            return Err("unresolved managed service mutation intent".into());
        }
        if self.entries.iter().any(|entry| entry.unit == unit && entry.operation == action) {
            return Err("managed service operation was already attempted".into());
        }
        let step = plan.steps.iter().find(|step| step.unit == unit)
            .ok_or("mutation targets a unit not owned by this Update plan")?;
        let target = receipt.targets.get(unit)
            .ok_or("mutation lacks matching immutable service receipt")?;
        use ManagedServiceTransition as T;
        use ManagedUpdateForwardOp as O;
        use ManagedUpdateRollbackTarget as R;
        let allowed = match (&step.transition, target, action) {
            (T::Add, R::InitiallyVacant, O::WriteUnit | O::ManagerReload | O::Enable | O::Start)
            | (T::Replace, R::OriginallyAbsent, O::WriteUnit | O::ManagerReload | O::Enable | O::Start)
            | (T::Replace, R::OriginalUnit { .. },
               O::Stop | O::Disable | O::WriteUnit | O::ManagerReload | O::Enable | O::Start)
            | (T::Retire, R::OriginalUnit { .. },
               O::Stop | O::Disable | O::RemoveUnit | O::ManagerReload) => true,
            _ => false,
        };
        if !allowed {
            return Err("operation forbidden by certified service transition".into());
        }
        let (was_active, was_enabled) = match target {
            R::OriginalUnit { was_active, was_enabled, .. } => (*was_active, *was_enabled),
            _ => (false, false),
        };
        match action {
            O::Stop if !was_active => return Err("old service was not active".into()),
            O::Disable if !was_enabled => return Err("old service was not enabled".into()),
            O::WriteUnit | O::RemoveUnit => {
                if was_active && !self.has_verified(unit, O::Stop) {
                    return Err("cannot mutate unit before certified Stop".into());
                }
                if was_enabled && !self.has_verified(unit, O::Disable) {
                    return Err("cannot mutate unit before certified Disable".into());
                }
            }
            O::ManagerReload => {
                if !self.has_verified(unit, O::WriteUnit)
                    && !self.has_verified(unit, O::RemoveUnit)
                {
                    return Err("cannot reload before a verified unit file mutation".into());
                }
            }
            O::Enable | O::Start => {
                if !self.has_verified(unit, O::ManagerReload) {
                    return Err("cannot activate before verified manager reload".into());
                }
            }
            _ => {}
        }
        self.entries.push(ManagedUpdateJournalEntry {
            unit: unit.to_owned(), operation: action, verified: false,
        });
        Ok(())
    }

    // A future executor must confirm only after the real operation and an
    // independent postcondition both succeed; this method never executes them.
    fn confirm(&mut self, unit: &str, action: ManagedUpdateForwardOp) -> Result<(), String> {
        let entry = self.entries.last_mut().ok_or("no pending service mutation")?;
        if entry.verified || entry.unit != unit || entry.operation != action {
            return Err("service mutation confirmation does not match pending intent".into());
        }
        entry.verified = true;
        Ok(())
    }

    // Pure compensation SCOPE, not a rollback executor or authorization.
    // An unverified intent may have partially mutated a unit: stop and
    // require explicit inspection rather than guessing what to undo.
    fn compensation_scope(
        &self,
        plan: &ManagedServiceUpdatePlan,
        receipt: &ManagedServiceRollbackReceipt,
    ) -> Result<BTreeMap<String, ManagedUpdateCompensationTarget>, String> {
        check_managed_journal_contract(plan, receipt)?;
        if self.entries.iter().any(|entry| !entry.verified) {
            return Err("pending service operation: rollback state is ambiguous".into());
        }
        let mut affected = BTreeSet::new();
        let mut seen = BTreeSet::new();
        for entry in &self.entries {
            let step = plan.steps.iter().find(|step| step.unit == entry.unit)
                .ok_or("unplanned unit in service mutation journal")?;
            if step.transition == ManagedServiceTransition::Keep
                || !seen.insert((entry.unit.clone(), entry.operation as u8))
            {
                return Err("invalid service journal entry or duplicate operation".into());
            }
            affected.insert(entry.unit.clone());
        }
        let mut result = BTreeMap::new();
        for unit in affected {
            let target = receipt.targets.get(&unit).ok_or("missing service recovery target")?;
            let compensation = match target {
                ManagedUpdateRollbackTarget::InitiallyVacant
                | ManagedUpdateRollbackTarget::OriginallyAbsent => {
                    if !self.has_verified(&unit, ManagedUpdateForwardOp::WriteUnit) {
                        return Err("cannot remove a unit without a verified creation".into());
                    }
                    ManagedUpdateCompensationTarget::RemoveVerifiedCreation
                }
                ManagedUpdateRollbackTarget::OriginalUnit {
                    bytes, sha256, mode, was_active, was_enabled,
                } => ManagedUpdateCompensationTarget::RestoreOriginal {
                    bytes: bytes.clone(), sha256: sha256.clone(), mode: *mode,
                    was_active: *was_active, was_enabled: *was_enabled,
                },
            };
            result.insert(unit, compensation);
        }
        Ok(result)
    }
}

// F04B9: DURABLE, FAIL-CLOSED SERVICE TXN CHECKPOINT (NO SYSTEMD EXECUTOR).
// This file is written ONLY into an existing owner-private directory, with a
// same-directory temporary file + sync_all + atomic rename + directory fsync.
// A process-wide flock guards this checkpoint against cooperating Boss writers.
// It is intentionally NOT connected to Update. The future executor must hold
// a separate full-lifetime transaction lock across systemd operations, bind
// the pinned installed/candidate revisions and resolve ambiguous intents only
// after a separate certified inspection. A digest detects damage/noncanonical
// bytes; filesystem ownership guards unprivileged tampering, not privileged
// attackers. Never treat this as a cryptographic signature.
const MAX_MANAGED_TXN_CHECKPOINT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedServiceUpdateCheckpoint {
    schema: u32,
    module: String,
    old_commit: String,
    candidate_commit: String,
    generation: u64,
    plan: ManagedServiceUpdatePlan,
    receipt: ManagedServiceRollbackReceipt,
    journal: ManagedServiceMutationJournal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedServiceCheckpointEnvelope {
    schema: u32,
    sha256: String,
    payload: ManagedServiceUpdateCheckpoint,
}

fn managed_txn_hex_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn managed_txn_pin_is_valid(pin: &str) -> bool {
    pin.len() == 40 && pin.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn validate_managed_txn_checkpoint(record: &ManagedServiceUpdateCheckpoint) -> Result<(), String> {
    if record.schema != 1
        || !neebles_backend::module_material::valid_module_id(&record.module)
        || !managed_txn_pin_is_valid(&record.old_commit)
        || !managed_txn_pin_is_valid(&record.candidate_commit)
        || record.plan.steps.is_empty() || record.plan.steps.len() > 64
        || record.journal.entries.len() > 512
    {
        return Err("invalid durable managed service transaction identity/limits".into());
    }
    check_managed_journal_contract(&record.plan, &record.receipt)?;
    for (unit, target) in &record.receipt.targets {
        validate_service_name(&record.module, unit)?;
        if let ManagedUpdateRollbackTarget::OriginalUnit {
            bytes, sha256, mode, ..
        } = target {
            if bytes.is_empty() || bytes.len() > 8192 || *mode & !0o777 != 0
                || *mode & 0o022 != 0 || *mode & 0o400 == 0
                || *sha256 != managed_txn_hex_digest(bytes)
            {
                return Err("invalid original unit recovery bytes or checksum".into());
            }
        }
    }
    // Replay through the strict F04B8 state machine: independently validates
    // each operation, sequence, confirmation and single pending tail.
    let mut replay = ManagedServiceMutationJournal::default();
    for entry in &record.journal.entries {
        replay.begin(&record.plan, &record.receipt, &entry.unit, entry.operation)?;
        if entry.verified {
            replay.confirm(&entry.unit, entry.operation)?;
        }
    }
    if replay != record.journal {
        return Err("persistent journal replay did not match original".into());
    }
    Ok(())
}

fn encode_managed_txn_checkpoint(record: &ManagedServiceUpdateCheckpoint) -> Result<Vec<u8>, String> {
    validate_managed_txn_checkpoint(record)?;
    let canonical = serde_json::to_vec(record).map_err(|e| format!("encode service checkpoint: {e}"))?;
    let envelope = ManagedServiceCheckpointEnvelope {
        schema: 1, sha256: managed_txn_hex_digest(&canonical), payload: record.clone(),
    };
    let bytes = serde_json::to_vec(&envelope)
        .map_err(|e| format!("encode service checkpoint envelope: {e}"))?;
    if bytes.len() > MAX_MANAGED_TXN_CHECKPOINT_BYTES {
        return Err("persistent service checkpoint exceeds size limit".into());
    }
    Ok(bytes)
}

fn decode_managed_txn_checkpoint(bytes: &[u8]) -> Result<ManagedServiceUpdateCheckpoint, String> {
    if bytes.is_empty() || bytes.len() > MAX_MANAGED_TXN_CHECKPOINT_BYTES {
        return Err("invalid persistent service checkpoint size".into());
    }
    let envelope: ManagedServiceCheckpointEnvelope = serde_json::from_slice(bytes)
        .map_err(|e| format!("decode service checkpoint: {e}"))?;
    if envelope.schema != 1 || serde_json::to_vec(&envelope).ok().as_deref() != Some(bytes) {
        return Err("noncanonical or unsupported service checkpoint".into());
    }
    let canonical = serde_json::to_vec(&envelope.payload)
        .map_err(|e| format!("encode checkpoint digest: {e}"))?;
    if envelope.sha256 != managed_txn_hex_digest(&canonical) {
        return Err("persistent service checkpoint digest mismatch".into());
    }
    validate_managed_txn_checkpoint(&envelope.payload)?;
    Ok(envelope.payload)
}

// This transition validator is a CAS boundary, not an automatic resume. The
// caller may append one pending intent or confirm an existing pending tail.
// Neither may silently remove or reorder previously recorded operations.
fn validate_managed_txn_successor(
    previous: &ManagedServiceUpdateCheckpoint,
    next: &ManagedServiceUpdateCheckpoint,
) -> Result<(), String> {
    validate_managed_txn_checkpoint(previous)?;
    validate_managed_txn_checkpoint(next)?;
    if next.generation != previous.generation.checked_add(1).ok_or("checkpoint generation overflow")?
        || next.schema != previous.schema || next.module != previous.module
        || next.old_commit != previous.old_commit
        || next.candidate_commit != previous.candidate_commit
        || next.plan != previous.plan || next.receipt != previous.receipt
    {
        return Err("persistent service checkpoint identity or generation drift".into());
    }
    let mut expected = previous.journal.clone();
    if next.journal.entries.len() == previous.journal.entries.len() + 1 {
        let appended = next.journal.entries.last().ok_or("missing appended intent")?;
        if appended.verified {
            return Err("new service mutation intent cannot be born confirmed".into());
        }
        expected.begin(&next.plan, &next.receipt, &appended.unit, appended.operation)?;
    } else if next.journal.entries.len() == previous.journal.entries.len() {
        let pending = previous.journal.entries.last().ok_or("no intent to confirm")?;
        if pending.verified {
            return Err("no pending service mutation intent".into());
        }
        expected.confirm(&pending.unit, pending.operation)?;
    } else {
        return Err("service checkpoint mutation history is not monotonic".into());
    }
    if expected != next.journal {
        return Err("service checkpoint rewrote historical operations".into());
    }
    Ok(())
}

fn managed_txn_private_directory(dir: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(dir).map_err(|e| format!("checkpoint directory: {e}"))?;
    let owner = unsafe { libc::geteuid() };
    if !meta.file_type().is_dir() || meta.uid() != owner
        || meta.mode() & 0o077 != 0 || meta.mode() & 0o700 != 0o700
    {
        return Err("service checkpoint directory must be owned private 0700 directory".into());
    }
    Ok(())
}

fn managed_txn_open_owned_file(path: &Path) -> Result<fs::File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let f = fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path).map_err(|e| format!("open service checkpoint: {e}"))?;
    let m = f.metadata().map_err(|e| format!("service checkpoint file stat: {e}"))?;
    if !m.file_type().is_file() || m.uid() != unsafe { libc::geteuid() }
        || m.nlink() != 1 || m.mode() & 0o777 != 0o600
        || m.len() == 0 || m.len() > MAX_MANAGED_TXN_CHECKPOINT_BYTES as u64
    {
        return Err("service checkpoint is foreign, linked or has unsafe mode/size".into());
    }
    Ok(f)
}

fn managed_txn_checkpoint_path(dir: &Path, module: &str) -> Result<PathBuf, String> {
    if !neebles_backend::module_material::valid_module_id(module) {
        return Err("unsafe durable service checkpoint module id".into());
    }
    Ok(dir.join(format!("{module}.managed-services-update.json")))
}

// Callers must NOT treat a successful write as permission to execute the
// recorded operation: this helper merely persists exactly one valid transition.
// The .lock file is permanent and must never be deleted opportunistically.
fn store_managed_txn_checkpoint(dir: &Path, next: &ManagedServiceUpdateCheckpoint) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    managed_txn_private_directory(dir)?;
    validate_managed_txn_checkpoint(next)?;
    let lock_path = dir.join(format!("{}.managed-services-update.lock", next.module));
    let lock = fs::OpenOptions::new().read(true).write(true).create(true).mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&lock_path).map_err(|e| format!("open checkpoint lock: {e}"))?;
    let lm = lock.metadata().map_err(|e| format!("checkpoint lock stat: {e}"))?;
    if !lm.file_type().is_file() || lm.uid() != unsafe { libc::geteuid() }
        || lm.nlink() != 1 || lm.mode() & 0o777 != 0o600
    {
        return Err("foreign or unsafe service checkpoint lock".into());
    }
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(format!("lock service checkpoint: {}", std::io::Error::last_os_error()));
    }
    // `lock` remains alive until after the rename+fsync or any error.
    let target = managed_txn_checkpoint_path(dir, &next.module)?;
    match managed_txn_open_owned_file(&target) {
        Ok(mut f) => {
            let mut old_bytes = Vec::new();
            f.take((MAX_MANAGED_TXN_CHECKPOINT_BYTES + 1) as u64)
                .read_to_end(&mut old_bytes)
                .map_err(|e| format!("read previous checkpoint: {e}"))?;
            let previous = decode_managed_txn_checkpoint(&old_bytes)?;
            validate_managed_txn_successor(&previous, next)?;
        }
        Err(e) => {
            // Only ENOENT permits the initial generation-zero write. A
            // dangling symlink, corrupt file or unreadable entry FAILS CLOSED.
            if fs::symlink_metadata(&target).is_ok() || next.generation != 0 {
                return Err(format!("prior checkpoint unsafe or missing: {e}"));
            }
            match fs::symlink_metadata(&target) {
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                _ => return Err("service checkpoint slot cannot be certified vacant".into()),
            }
        }
    }
    let bytes = encode_managed_txn_checkpoint(next)?;
    let tmp = dir.join(format!(".{}.{}.tmp", next.module, uuid::Uuid::new_v4()));
    let write_result = (|| -> Result<(), String> {
        let mut f = fs::OpenOptions::new().write(true).create_new(true).mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC).open(&tmp)
            .map_err(|e| format!("create service checkpoint temporary: {e}"))?;
        f.write_all(&bytes).and_then(|_| f.sync_all())
            .map_err(|e| format!("sync service checkpoint temporary: {e}"))?;
        fs::rename(&tmp, &target).map_err(|e| format!("publish service checkpoint: {e}"))?;
        fs::File::open(dir).and_then(|directory| directory.sync_all())
            .map_err(|e| format!("sync service checkpoint directory: {e}"))?;
        Ok(())
    })();
    if write_result.is_err() { let _ = fs::remove_file(&tmp); }
    // The caller must treat a post-rename fsync error as an indeterminate
    // durability result, never skip to systemd under the assumption of success.
    write_result
}

fn load_managed_txn_checkpoint(dir: &Path, module: &str) -> Result<ManagedServiceUpdateCheckpoint, String> {
    use std::io::Read;
    managed_txn_private_directory(dir)?;
    let path = managed_txn_checkpoint_path(dir, module)?;
    let f = managed_txn_open_owned_file(&path)?;
    let mut bytes = Vec::new();
    f.take((MAX_MANAGED_TXN_CHECKPOINT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read service checkpoint: {e}"))?;
    let checkpoint = decode_managed_txn_checkpoint(&bytes)?;
    if checkpoint.module != module {
        return Err("service checkpoint requested module identity mismatch".into());
    }
    Ok(checkpoint)
}

// F04B12: DURABLE TWO-PHASE STEP DRIVER (NOT WIRED TO UPDATE).
// The external-effect closure is deliberately NOT a systemctl implementation:
// a future caller must enforce the module's pinned service identity and use
// certified postconditions. This driver only guarantees that the declared
// intent is durably recorded before a caller-supplied effect can run.
// A failed effect or verification intentionally leaves an UNRESOLVED intent.
// Never guess that an effect did not happen and never auto-clear the journal.
fn managed_txn_open_lifetime_lock(dir: &Path, module: &str) -> Result<fs::File, String> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    managed_txn_private_directory(dir)?;
    managed_txn_checkpoint_path(dir, module)?; // validate id before constructing any path
    let path = dir.join(format!("{module}.managed-services-driver.lock"));
    let file = fs::OpenOptions::new().read(true).write(true).create(true)
        .mode(0o600).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path).map_err(|e| format!("open service driver lock: {e}"))?;
    let metadata = file.metadata().map_err(|e| format!("stat service driver lock: {e}"))?;
    if !metadata.file_type().is_file() || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.nlink() != 1 || metadata.mode() & 0o777 != 0o600
    {
        return Err("foreign or unsafe service driver lock".into());
    }
    // Do not block indefinitely when another controller is operating on the
    // same module. The lock covers begin, the effect, verification and ack.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("another service transaction already owns this module".into());
    }
    Ok(file)
}

#[allow(dead_code)]
fn managed_txn_perform_verified_step<Apply, Verify>(
    dir: &Path,
    module: &str,
    expected_old_commit: &str,
    expected_candidate_commit: &str,
    unit: &str,
    operation: ManagedUpdateForwardOp,
    apply: Apply,
    verify: Verify,
) -> Result<ManagedServiceUpdateCheckpoint, String>
where
    Apply: FnOnce() -> Result<(), String>,
    Verify: FnOnce() -> Result<(), String>,
{
    let _lifetime_lock = managed_txn_open_lifetime_lock(dir, module)?;
    let mut pending = load_managed_txn_checkpoint(dir, module)?;
    if pending.old_commit != expected_old_commit
        || pending.candidate_commit != expected_candidate_commit
    {
        return Err("service step rejected: pinned revisions no longer match".into());
    }
    pending.journal.begin(&pending.plan, &pending.receipt, unit, operation)?;
    pending.generation = pending.generation.checked_add(1)
        .ok_or("service checkpoint generation overflow")?;
    // If this fails, the caller-supplied effect MUST NOT execute.
    store_managed_txn_checkpoint(dir, &pending)?;
    // From this point onward, errors leave a durable pending intent for
    // certified manual reconciliation. Never automatically retry or undo.
    apply().map_err(|e| format!("service effect failed; intent remains unresolved: {e}"))?;
    verify().map_err(|e| format!("service verification failed; intent remains unresolved: {e}"))?;
    let current = load_managed_txn_checkpoint(dir, module)?;
    if current != pending {
        return Err("service checkpoint changed during effect; intent remains unresolved".into());
    }
    let mut acknowledged = current;
    acknowledged.journal.confirm(unit, operation)?;
    acknowledged.generation = acknowledged.generation.checked_add(1)
        .ok_or("service checkpoint generation overflow")?;
    store_managed_txn_checkpoint(dir, &acknowledged)?;
    Ok(acknowledged)
}

#[cfg(test)]
mod f04b12_durable_step_driver_tests {
    use super::*;
    use std::cell::Cell;
    use std::os::unix::fs::{symlink, PermissionsExt};

    fn fresh() -> (tempfile::TempDir, PathBuf, ManagedServiceUpdateCheckpoint) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("owner-private");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let module = "neebles-demo";
        let unit = "neebles-neebles-demo-agent.service";
        let checkpoint = ManagedServiceUpdateCheckpoint {
            schema: 1, module: module.into(), old_commit: "a".repeat(40),
            candidate_commit: "b".repeat(40), generation: 0,
            plan: ManagedServiceUpdatePlan { steps: vec![ManagedServiceStep {
                unit: unit.into(), transition: ManagedServiceTransition::Add,
            }] },
            receipt: ManagedServiceRollbackReceipt { targets: BTreeMap::from([(
                unit.into(), ManagedUpdateRollbackTarget::InitiallyVacant,
            )]) },
            journal: ManagedServiceMutationJournal::default(),
        };
        store_managed_txn_checkpoint(&dir, &checkpoint).unwrap();
        (tmp, dir, checkpoint)
    }

    fn execute<Apply, Verify>(
        dir: &Path, c: &ManagedServiceUpdateCheckpoint, apply: Apply, verify: Verify,
    ) -> Result<ManagedServiceUpdateCheckpoint, String>
    where Apply: FnOnce() -> Result<(), String>, Verify: FnOnce() -> Result<(), String> {
        managed_txn_perform_verified_step(
            dir, &c.module, &c.old_commit, &c.candidate_commit,
            "neebles-neebles-demo-agent.service", ManagedUpdateForwardOp::WriteUnit,
            apply, verify,
        )
    }

    #[test]
    fn intent_is_durable_before_effect_and_ack_requires_postcondition() {
        let (_tmp, dir, c) = fresh();
        let effect_dir = dir.clone();
        let effect_module = c.module.clone();
        let marker = dir.join("effect-marker");
        let marker_write = marker.clone();
        let marker_read = marker.clone();
        let last = execute(&dir, &c,
            move || {
                let current = load_managed_txn_checkpoint(&effect_dir, &effect_module)?;
                assert_eq!(current.generation, 1);
                assert!(!current.journal.entries[0].verified);
                fs::write(&marker_write, b"effect applied").map_err(|e| e.to_string())
            },
            move || {
                if fs::read(&marker_read).map_err(|e| e.to_string())?.as_slice() != b"effect applied" {
                    return Err("effect postcondition missing".into());
                }
                Ok(())
            },
        ).unwrap();
        assert_eq!(last.generation, 2);
        assert!(last.journal.entries[0].verified);
        assert_eq!(load_managed_txn_checkpoint(&dir, &c.module).unwrap(), last);
    }

    #[test]
    fn failed_effect_leaves_durable_pending_intent_and_blocks_retry() {
        let (_tmp, dir, c) = fresh();
        let called_verify = Cell::new(false);
        assert!(execute(&dir, &c, || Err("injected failure".into()), || {
            called_verify.set(true); Ok(())
        }).is_err());
        assert!(!called_verify.get());
        let pending = load_managed_txn_checkpoint(&dir, &c.module).unwrap();
        assert_eq!(pending.generation, 1);
        assert!(!pending.journal.entries[0].verified);
        let called_apply = Cell::new(false);
        assert!(execute(&dir, &c, || { called_apply.set(true); Ok(()) }, || Ok(())).is_err());
        assert!(!called_apply.get());
    }

    #[test]
    fn failed_postcondition_does_not_confirm_or_discard_intent() {
        let (_tmp, dir, c) = fresh();
        let called_apply = Cell::new(false);
        assert!(execute(&dir, &c, || { called_apply.set(true); Ok(()) },
            || Err("independently observed wrong state".into())).is_err());
        assert!(called_apply.get());
        let pending = load_managed_txn_checkpoint(&dir, &c.module).unwrap();
        assert_eq!(pending.generation, 1);
        assert!(!pending.journal.entries[0].verified);
        assert!(pending.journal.compensation_scope(&pending.plan, &pending.receipt).is_err());
    }

    #[test]
    fn wrong_revision_and_foreign_unit_never_invoke_effect() {
        let (_tmp, dir, c) = fresh();
        let touched = Cell::new(false);
        assert!(managed_txn_perform_verified_step(&dir, &c.module, &c.old_commit,
            &"c".repeat(40), "neebles-neebles-demo-agent.service",
            ManagedUpdateForwardOp::WriteUnit,
            || { touched.set(true); Ok(()) }, || Ok(())).is_err());
        assert!(managed_txn_perform_verified_step(&dir, &c.module, &c.old_commit,
            &c.candidate_commit, "foreign.service", ManagedUpdateForwardOp::WriteUnit,
            || { touched.set(true); Ok(()) }, || Ok(())).is_err());
        assert!(!touched.get());
        assert_eq!(load_managed_txn_checkpoint(&dir, &c.module).unwrap().generation, 0);
    }

    #[test]
    fn concurrent_driver_is_rejected_without_mutating_other_checkpoint() {
        let (_tmp, dir, c) = fresh();
        let touched = Cell::new(false);
        let result = execute(&dir, &c, || {
            let nested = execute(&dir, &c, || { touched.set(true); Ok(()) }, || Ok(()));
            assert!(nested.is_err());
            assert!(nested.unwrap_err().contains("already owns"));
            Ok(())
        }, || Ok(())).unwrap();
        assert!(!touched.get());
        assert_eq!(result.generation, 2);
    }

    #[test]
    fn poisoned_checkpoint_and_foreign_driver_lock_fail_before_effect() {
        let (_tmp, dir, c) = fresh();
        let path = managed_txn_checkpoint_path(&dir, &c.module).unwrap();
        fs::write(&path, b"malformed checkpoint").unwrap();
        let touched = Cell::new(false);
        assert!(execute(&dir, &c, || { touched.set(true); Ok(()) }, || Ok(())).is_err());
        assert!(!touched.get());
        // Use an independent fresh directory: the prior attempt created its
        // permanent driver lock, which must NOT be removed.
        let (tmp2, dir2, c2) = fresh();
        let other = tmp2.path().join("other");
        fs::create_dir(&other).unwrap();
        fs::set_permissions(&other, fs::Permissions::from_mode(0o700)).unwrap();
        let foreign = other.join("victim");
        fs::write(&foreign, b"unaltered").unwrap();
        let lock = dir2.join(format!("{}.managed-services-driver.lock", c2.module));
        symlink(&foreign, &lock).unwrap();
        assert!(execute(&dir2, &c2, || { touched.set(true); Ok(()) }, || Ok(())).is_err());
        assert_eq!(fs::read(&foreign).unwrap(), b"unaltered");
        assert!(!touched.get());
    }
}

#[cfg(test)]
mod f04b9_persistent_service_journal_tests {
    use super::*;
    use ManagedServiceTransition as T;
    use ManagedUpdateForwardOp as O;
    use ManagedUpdateRollbackTarget as R;
    use std::os::unix::fs::{symlink, PermissionsExt};

    fn checkpoint() -> ManagedServiceUpdateCheckpoint {
        let module = "neebles-demo";
        let unit = "neebles-neebles-demo-agent.service";
        ManagedServiceUpdateCheckpoint {
            schema: 1, module: module.into(), old_commit: "a".repeat(40),
            candidate_commit: "b".repeat(40), generation: 0,
            plan: ManagedServiceUpdatePlan { steps: vec![
                ManagedServiceStep { unit: unit.into(), transition: T::Add },
            ] },
            receipt: ManagedServiceRollbackReceipt { targets: BTreeMap::from([
                (unit.into(), R::InitiallyVacant),
            ]) },
            journal: ManagedServiceMutationJournal::default(),
        }
    }

    fn secure_dir() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("owned");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        (temp, dir)
    }

    #[test]
    fn checkpoint_roundtrip_and_exact_initial_save() {
        let (_tmp, dir) = secure_dir();
        let saved = checkpoint();
        store_managed_txn_checkpoint(&dir, &saved).unwrap();
        assert_eq!(load_managed_txn_checkpoint(&dir, &saved.module).unwrap(), saved);
        assert!(store_managed_txn_checkpoint(&dir, &saved).is_err());
    }

    #[test]
    fn crash_between_begin_and_confirmation_preserves_ambiguity() {
        let (_tmp, dir) = secure_dir();
        let original = checkpoint();
        store_managed_txn_checkpoint(&dir, &original).unwrap();
        let mut pending = original.clone();
        pending.journal.begin(&pending.plan, &pending.receipt,
            "neebles-neebles-demo-agent.service", O::WriteUnit).unwrap();
        pending.generation = 1;
        store_managed_txn_checkpoint(&dir, &pending).unwrap();
        let mut loaded = load_managed_txn_checkpoint(&dir, &pending.module).unwrap();
        assert!(loaded.journal.compensation_scope(&loaded.plan, &loaded.receipt).is_err());
        loaded.journal.confirm("neebles-neebles-demo-agent.service", O::WriteUnit).unwrap();
        loaded.generation = 2;
        store_managed_txn_checkpoint(&dir, &loaded).unwrap();
        assert!(load_managed_txn_checkpoint(&dir, &loaded.module).unwrap()
            .journal.compensation_scope(&loaded.plan, &loaded.receipt).is_ok());
    }

    #[test]
    fn generation_and_contract_cannot_roll_back_or_drift() {
        let old = checkpoint();
        let mut next = old.clone();
        next.generation = 1;
        assert!(validate_managed_txn_successor(&old, &next).is_err());
        next.journal.begin(&old.plan, &old.receipt,
            "neebles-neebles-demo-agent.service", O::WriteUnit).unwrap();
        assert!(validate_managed_txn_successor(&old, &next).is_ok());
        next.candidate_commit = "c".repeat(40);
        assert!(validate_managed_txn_successor(&old, &next).is_err());
    }

    #[test]
    fn mutated_or_noncanonical_checkpoint_is_rejected() {
        let mut raw = encode_managed_txn_checkpoint(&checkpoint()).unwrap();
        let middle = raw.len() / 2;
        raw[middle] ^= 1;
        assert!(decode_managed_txn_checkpoint(&raw).is_err());
        let mut raw = encode_managed_txn_checkpoint(&checkpoint()).unwrap();
        raw.push(b' ');
        assert!(decode_managed_txn_checkpoint(&raw).is_err());
    }

    #[test]
    fn foreign_symlink_and_untrusted_directory_do_not_get_written() {
        let (_tmp, dir) = secure_dir();
        let saved = checkpoint();
        let outside = dir.parent().unwrap().join("external.txt");
        fs::write(&outside, b"FOREIGN").unwrap();
        let target = managed_txn_checkpoint_path(&dir, &saved.module).unwrap();
        symlink(&outside, &target).unwrap();
        assert!(store_managed_txn_checkpoint(&dir, &saved).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"FOREIGN");
        fs::remove_file(&target).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(store_managed_txn_checkpoint(&dir, &saved).is_err());
    }

    #[test]
    fn original_bytes_with_inconsistent_digest_fail_closed() {
        let mut x = checkpoint();
        let u = "neebles-neebles-demo-agent.service";
        x.plan.steps[0].transition = T::Replace;
        x.receipt.targets.insert(u.into(), R::OriginalUnit {
            bytes: b"[Service]\nExecStart=/opt/neebles/modules/neebles-demo/bin/server\n".to_vec(),
            sha256: "f".repeat(64), mode: 0o600, was_active: false, was_enabled: false,
        });
        assert!(encode_managed_txn_checkpoint(&x).is_err());
    }
}

#[cfg(test)]
mod f04b8_mutation_journal_tests {
    use super::*;
    use ManagedServiceTransition as T;
    use ManagedUpdateForwardOp as O;
    use ManagedUpdateRollbackTarget as R;

    fn context(
        unit: &str, transition: T, target: R,
    ) -> (ManagedServiceUpdatePlan, ManagedServiceRollbackReceipt) {
        (ManagedServiceUpdatePlan {
            steps: vec![ManagedServiceStep { unit: unit.into(), transition }],
        }, ManagedServiceRollbackReceipt {
            targets: BTreeMap::from([(unit.into(), target)]),
        })
    }

    fn original(active: bool, enabled: bool) -> R {
        R::OriginalUnit { bytes: b"[Service]\\nExecStart=/test".to_vec(),
            sha256: "original-sha256".into(), mode: 0o600,
            was_active: active, was_enabled: enabled }
    }

    fn done(journal: &mut ManagedServiceMutationJournal, plan: &ManagedServiceUpdatePlan,
            receipt: &ManagedServiceRollbackReceipt, unit: &str, op: O) {
        journal.begin(plan, receipt, unit, op).unwrap();
        journal.confirm(unit, op).unwrap();
    }

    #[test]
    fn empty_journal_does_not_invent_work() {
        let (p, r) = context("neebles-demo.service", T::Add, R::InitiallyVacant);
        assert!(ManagedServiceMutationJournal::default().compensation_scope(&p, &r).unwrap().is_empty());
    }

    #[test]
    fn add_requires_verified_creation_before_any_cleanup() {
        let u = "neebles-demo.service";
        let (p, r) = context(u, T::Add, R::InitiallyVacant);
        let mut j = ManagedServiceMutationJournal::default();
        assert!(j.begin(&p, &r, u, O::ManagerReload).is_err());
        j.begin(&p, &r, u, O::WriteUnit).unwrap();
        assert!(j.compensation_scope(&p, &r).is_err());
        assert!(j.begin(&p, &r, u, O::ManagerReload).is_err());
        j.confirm(u, O::WriteUnit).unwrap();
        done(&mut j, &p, &r, u, O::ManagerReload);
        done(&mut j, &p, &r, u, O::Start);
        assert_eq!(j.compensation_scope(&p, &r).unwrap().get(u),
            Some(&ManagedUpdateCompensationTarget::RemoveVerifiedCreation));
    }

    #[test]
    fn replaced_running_enabled_unit_must_be_quiesced_and_is_restored_exactly() {
        let u = "neebles-demo.service";
        let (p, r) = context(u, T::Replace, original(true, true));
        let mut j = ManagedServiceMutationJournal::default();
        assert!(j.begin(&p, &r, u, O::WriteUnit).is_err());
        done(&mut j, &p, &r, u, O::Stop);
        assert!(j.begin(&p, &r, u, O::WriteUnit).is_err());
        done(&mut j, &p, &r, u, O::Disable);
        done(&mut j, &p, &r, u, O::WriteUnit);
        done(&mut j, &p, &r, u, O::ManagerReload);
        let scope = j.compensation_scope(&p, &r).unwrap();
        assert_eq!(scope.get(u), Some(&ManagedUpdateCompensationTarget::RestoreOriginal {
            bytes: b"[Service]\\nExecStart=/test".to_vec(),
            sha256: "original-sha256".into(), mode: 0o600,
            was_active: true, was_enabled: true,
        }));
    }

    #[test]
    fn inactive_disabled_retirement_restores_without_auto_start() {
        let u = "neebles-demo.service";
        let (p, r) = context(u, T::Retire, original(false, false));
        let mut j = ManagedServiceMutationJournal::default();
        done(&mut j, &p, &r, u, O::RemoveUnit);
        done(&mut j, &p, &r, u, O::ManagerReload);
        assert!(matches!(j.compensation_scope(&p, &r).unwrap().get(u),
            Some(ManagedUpdateCompensationTarget::RestoreOriginal {
                was_active: false, was_enabled: false, ..
            })));
    }

    #[test]
    fn keep_and_old_optional_absent_do_not_grant_mutation() {
        let u = "neebles-demo.service";
        for transition in [T::Keep, T::Retire] {
            let target = if transition == T::Keep { original(false, false) } else { R::OriginallyAbsent };
            let (p, r) = context(u, transition, target);
            let mut j = ManagedServiceMutationJournal::default();
            assert!(j.begin(&p, &r, u, O::WriteUnit).is_err());
            assert!(j.begin(&p, &r, u, O::RemoveUnit).is_err());
            assert!(j.compensation_scope(&p, &r).unwrap().is_empty());
        }
    }

    #[test]
    fn optional_absent_replace_can_only_remove_its_verified_creation() {
        let u = "neebles-demo.service";
        let (p, r) = context(u, T::Replace, R::OriginallyAbsent);
        let mut j = ManagedServiceMutationJournal::default();
        done(&mut j, &p, &r, u, O::WriteUnit);
        assert_eq!(j.compensation_scope(&p, &r).unwrap().get(u),
            Some(&ManagedUpdateCompensationTarget::RemoveVerifiedCreation));
    }

    #[test]
    fn foreign_unit_and_wrong_acknowledgment_fail_closed() {
        let u = "neebles-demo.service";
        let (p, r) = context(u, T::Add, R::InitiallyVacant);
        let mut j = ManagedServiceMutationJournal::default();
        assert!(j.begin(&p, &r, "foreign.service", O::WriteUnit).is_err());
        j.begin(&p, &r, u, O::WriteUnit).unwrap();
        assert!(j.confirm(u, O::Enable).is_err());
        assert!(j.confirm("foreign.service", O::WriteUnit).is_err());
        assert!(j.compensation_scope(&p, &r).is_err());
    }

    #[test]
    fn receipt_inconsistent_with_plan_is_never_an_authority() {
        let u = "neebles-demo.service";
        let (p, mut r) = context(u, T::Add, R::InitiallyVacant);
        let mut j = ManagedServiceMutationJournal::default();
        r.targets.insert("foreign.service".into(), R::InitiallyVacant);
        assert!(j.begin(&p, &r, u, O::WriteUnit).is_err());
        assert!(j.compensation_scope(&p, &r).is_err());
    }

    #[test]
    fn duplicate_operation_cannot_be_acknowledged_twice() {
        let u = "neebles-demo.service";
        let (p, r) = context(u, T::Add, R::InitiallyVacant);
        let mut j = ManagedServiceMutationJournal::default();
        done(&mut j, &p, &r, u, O::WriteUnit);
        assert!(j.begin(&p, &r, u, O::WriteUnit).is_err());
        assert!(j.confirm(u, O::WriteUnit).is_err());
    }
}

#[cfg(test)]
mod f04b4_rollback_receipt_tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn spec(unit: &str) -> ManagedServiceSpec {
        ManagedServiceSpec {
            id: "daemon".into(), unit: unit.into(),
            exec_start: "/opt/neebles/modules/example/bin/server".into(),
            required: true, timeout_ms: 5000,
        }
    }
    fn old_unit() -> String { "neebles-example-server.service".into() }
    fn new_unit() -> String { "neebles-example-new.service".into() }
    fn digest(bytes: &[u8]) -> String { Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>() }
    fn fixture() -> (
        BTreeMap<String, ManagedServiceSpec>, ManagedServiceUpdatePlan,
        ManagedServiceUpdateBaseline, BTreeMap<String, (Vec<u8>, u32)>,
    ) {
        let old = old_unit();
        let new = new_unit();
        let bytes = b"[Unit]\nDescription=Example\n".to_vec();
        let specs = BTreeMap::from([(old.clone(), spec(&old))]);
        let plan = ManagedServiceUpdatePlan { steps: vec![
            ManagedServiceStep { unit: old.clone(), transition: ManagedServiceTransition::Replace },
            ManagedServiceStep { unit: new.clone(), transition: ManagedServiceTransition::Add },
        ] };
        let baseline = ManagedServiceUpdateBaseline {
            previous: BTreeMap::from([(old.clone(), ManagedUpdateObservedUnit::Present {
                was_active: false, was_enabled: true, unit_sha256: digest(&bytes),
            })]),
            reserved_new_units: BTreeSet::from([new]),
        };
        let files = BTreeMap::from([(old, (bytes, 0o644))]);
        (specs, plan, baseline, files)
    }

    #[test]
    fn preserves_original_bytes_and_exact_disabled_running_state() {
        let (specs, plan, baseline, files) = fixture();
        let receipt = build_managed_service_rollback_receipt(&specs, &plan, &baseline, &files).unwrap();
        let old = old_unit();
        let target = receipt.targets.get(&old).unwrap();
        match target {
            ManagedUpdateRollbackTarget::OriginalUnit { bytes, mode, was_active, was_enabled, sha256 } => {
                assert_eq!(bytes, &files[&old].0);
                assert_eq!(*mode, 0o644);
                assert!(!*was_active);
                assert!(*was_enabled);
                assert_eq!(*sha256, digest(bytes));
            }
            other => panic!("expected original file, got {other:?}"),
        }
        assert_eq!(receipt.targets.get(&new_unit()), Some(&ManagedUpdateRollbackTarget::InitiallyVacant));
    }

    #[test]
    fn rejects_mutated_old_unit_content_and_missing_bytes() {
        let (specs, plan, baseline, mut files) = fixture();
        files.get_mut(&old_unit()).unwrap().0.push(b'!');
        assert!(build_managed_service_rollback_receipt(&specs, &plan, &baseline, &files).is_err());
        files.remove(&old_unit());
        assert!(build_managed_service_rollback_receipt(&specs, &plan, &baseline, &files).is_err());
    }

    #[test]
    fn rejects_duplicate_or_missing_transition_steps() {
        let (specs, mut plan, baseline, files) = fixture();
        plan.steps.push(plan.steps[0].clone());
        assert!(build_managed_service_rollback_receipt(&specs, &plan, &baseline, &files).is_err());
        plan.steps.remove(2);
        plan.steps.retain(|step| step.unit != new_unit());
        assert!(build_managed_service_rollback_receipt(&specs, &plan, &baseline, &files).is_err());
    }

    #[test]
    fn refuses_unreserved_new_unit_or_old_unit_reused_as_add() {
        let (specs, mut plan, mut baseline, files) = fixture();
        baseline.reserved_new_units.clear();
        assert!(build_managed_service_rollback_receipt(&specs, &plan, &baseline, &files).is_err());
        baseline.reserved_new_units.insert(new_unit());
        plan.steps[0].transition = ManagedServiceTransition::Add;
        assert!(build_managed_service_rollback_receipt(&specs, &plan, &baseline, &files).is_err());
    }

    #[test]
    fn optional_old_unit_absence_stays_absent() {
        let (mut specs, plan, mut baseline, mut files) = fixture();
        specs.get_mut(&old_unit()).unwrap().required = false;
        baseline.previous.insert(old_unit(), ManagedUpdateObservedUnit::Absent);
        files.clear();
        let receipt = build_managed_service_rollback_receipt(&specs, &plan, &baseline, &files).unwrap();
        assert_eq!(receipt.targets[&old_unit()], ManagedUpdateRollbackTarget::OriginallyAbsent);
    }

    #[test]
    fn rejects_insecure_mode_and_missing_baseline() {
        let (specs, plan, mut baseline, mut files) = fixture();
        files.get_mut(&old_unit()).unwrap().1 = 0o666;
        assert!(build_managed_service_rollback_receipt(&specs, &plan, &baseline, &files).is_err());
        files.get_mut(&old_unit()).unwrap().1 = 0o644;
        baseline.previous.clear();
        assert!(build_managed_service_rollback_receipt(&specs, &plan, &baseline, &files).is_err());
    }
}

// F04A: read-only, fail-closed detection of declared module-owned systemd-user
// services. A unit that exists (even inactive) can be started by its manager
// while an Update is in progress. Until F04B implements transactional service
// quiescence and compensation, such units MUST block an Update.
// No service is started, stopped, disabled, installed or removed by this probe.
fn classify_owned_service_presence(
    has_unit: bool,
    required: bool,
    load: &str,
    active: &str,
) -> Result<bool, String> {
    if !has_unit {
        if load != "not-found" || active != "inactive" {
            return Err("service manager has a resource without an authenticated unit file".into());
        }
        if required {
            return Err("required declared user service is absent".into());
        }
        return Ok(false);
    }
    if load != "loaded" {
        return Err(format!("declared service has unexpected LoadState '{load}'"));
    }
    if !matches!(active,
        "active" | "inactive" | "failed" | "activating" | "deactivating" | "reloading"
    ) {
        return Err(format!("declared service has unknown ActiveState '{active}'"));
    }
    // Even inactive and failed loaded units may be restarted by the user manager.
    Ok(true)
}

pub(crate) fn declared_user_services_present(module: &str) -> Result<bool, String> {
    let module_root = crate::modules::installed_module_dir(module)?;
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(module_root.join("manifest.json"))
            .map_err(|e| format!("cannot inspect installed service manifest: {e}"))?,
    )
    .map_err(|e| format!("invalid installed service manifest JSON: {e}"))?;
    if manifest.get("schema").and_then(|v| v.as_u64()) != Some(4)
        || manifest.get("name").and_then(|v| v.as_str()) != Some(module)
    {
        return Err("service inspection installed module identity mismatch".into());
    }
    let Some(uninstall) = manifest.get("uninstall") else {
        return Ok(false);
    };
    let contract: UninstallContract = serde_json::from_value(uninstall.clone())
        .map_err(|e| format!("invalid installed service ownership: {e}"))?;
    validate(&contract, module, manifest.get("tray").is_some())?;
    if !contract.extras.iter().any(|extra| matches!(extra, Extra::Service { .. })) {
        return Ok(false);
    }

    let (uid, gid) = crate::runtime_identity::desktop_identity()?;
    if uid == 0 {
        return Err("cannot inspect systemd-user service with desktop UID 0".into());
    }
    let home = desktop_home(uid)?;
    let mut present = false;
    for extra in &contract.extras {
        let Extra::Service {
            name, exec_start, required, ..
        } = extra else {
            continue;
        };
        let unit = resolve(name, &contract, module)?;
        let command = resolve(exec_start, &contract, module)?;
        validate_service_name(module, &unit)?;
        validate_service_exec_start(&command, module)?;
        let path = validated_unit_path(&home, uid, gid, &unit)?;
        let metadata = match fs::symlink_metadata(&path) {
            Ok(meta) => Some(meta),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(format!("cannot inspect declared user unit '{unit}': {error}")),
        };
        let (load, active) = systemd_state(uid, gid, &home, &unit)?;
        if let Some(meta) = metadata.as_ref() {
            if !meta.file_type().is_file() || meta.uid() != uid || meta.gid() != gid
                || meta.mode() & 0o022 != 0
            {
                return Err(format!("declared service '{unit}' has unsafe ownership or mode"));
            }
            let contents = fs::read_to_string(&path)
                .map_err(|e| format!("cannot read declared service '{unit}': {e}"))?;
            validate_service_unit_text(&contents, &command, module)?;
            validate_manager_fragment(uid, gid, &home, &unit, &path, &command)?;
        }
        if classify_owned_service_presence(metadata.is_some(), *required, &load, &active)? {
            present = true;
        }
    }
    Ok(present)
}

#[cfg(test)]
mod service_presence_gate_f04a_tests {
    use super::classify_owned_service_presence;

    #[test]
    fn an_inactive_but_installed_unit_is_still_a_resource() {
        assert_eq!(classify_owned_service_presence(true, false, "loaded", "inactive"), Ok(true));
        assert_eq!(classify_owned_service_presence(true, false, "loaded", "failed"), Ok(true));
        assert_eq!(classify_owned_service_presence(true, true, "loaded", "active"), Ok(true));
    }

    #[test]
    fn absent_optional_unit_is_not_a_resource() {
        assert_eq!(classify_owned_service_presence(false, false, "not-found", "inactive"), Ok(false));
    }

    #[test]
    fn fail_closed_on_foreign_loaded_or_missing_required_unit() {
        assert!(classify_owned_service_presence(false, false, "loaded", "active").is_err());
        assert!(classify_owned_service_presence(false, true, "not-found", "inactive").is_err());
        assert!(classify_owned_service_presence(true, false, "not-found", "inactive").is_err());
        assert!(classify_owned_service_presence(true, false, "loaded", "bogus").is_err());
    }
}

// Module-agnostic, fail-closed systemd-user *install* counterpart to TXN V5
// service retirement. Called only by authenticated Governor Lifecycle.
fn certify_install_unit(contents: &str, module: &str, exec: &str) -> Result<(), String> {
    if !contents.ends_with('\n') || contents.contains('\r') || contents.len() > 8192 {
        return Err("user service unit has unsupported encoding or size".into());
    }
    let manifest = format!("/opt/neebles/modules/{module}/manifest.json");
    let expected = [
        ("Unit", "After", "basic.target"),
        ("Unit", "ConditionPathExists", manifest.as_str()),
        ("Service", "Type", "simple"),
        ("Service", "ExecStart", exec),
        ("Service", "Restart", "on-failure"),
        ("Service", "RestartSec", "2"),
        ("Service", "UMask", "0077"),
        ("Service", "NoNewPrivileges", "true"),
        ("Install", "WantedBy", "default.target"),
    ];
    let mut section = "";
    let mut seen = BTreeSet::<(String, String)>::new();
    for line in contents.lines() {
        if line.is_empty() { continue; }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len()-1];
            if !matches!(section, "Unit" | "Service" | "Install") {
                return Err("user service contains unsupported section".into());
            }
            continue;
        }
        let (key, value) = line.split_once('=').ok_or("invalid user service line")?;
        if !seen.insert((section.into(), key.into())) {
            return Err("duplicate user service key".into());
        }
        if section == "Unit" && key == "Description"
            && !value.is_empty() && value.len() <= 160
        {
            continue;
        }
        if !expected.iter().any(|(s, k, v)| *s == section && *k == key && *v == value) {
            return Err(format!("user service has unsupported or altered key {section}/{key}"));
        }
    }
    for (s, k, _) in expected {
        if !seen.contains(&(s.into(), k.into())) {
            return Err(format!("user service missing required key {s}/{k}"));
        }
    }
    Ok(())
}

pub fn install_declared_user_service(module: &str, resource_id: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    neebles_backend::domestic_construction::validate_domestic_construction_subject(module)?;
    if resource_id.is_empty() || resource_id.len() > 128
        || !resource_id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("user service resource id is invalid".into());
    }
    let module_root = PathBuf::from(format!("/opt/neebles/modules/{module}"));
    let manifest_path = module_root.join("manifest.json");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(&manifest_path).map_err(|e| format!("service manifest unavailable: {e}"))?
    ).map_err(|e| format!("service manifest JSON: {e}"))?;
    if manifest.get("schema").and_then(|v|v.as_u64()) != Some(4)
        || manifest.get("name").and_then(|v|v.as_str()) != Some(module)
    {
        return Err("user service installed manifest identity mismatch".into());
    }
    let contract: UninstallContract = serde_json::from_value(
        manifest.get("uninstall").ok_or("user service has no uninstall contract")?.clone()
    ).map_err(|e| format!("invalid service ownership declaration: {e}"))?;
    validate(&contract, module, manifest.get("tray").is_some())?;
    let matches: Vec<_> = contract.extras.iter().filter(|extra| match extra {
        Extra::Service { id, .. } => id == resource_id,
        _ => false,
    }).collect();
    if matches.len() != 1 { return Err("user service resource is not uniquely declared".into()); }
    let (unit, exec_start) = match matches[0] {
        Extra::Service { provider, name, exec_start, stop, remove, .. }
            if provider == "systemd-user" && *remove && stop.disable => {
            (resolve(name, &contract, module)?, resolve(exec_start, &contract, module)?)
        }
        _ => return Err("user service resource is not uninstall-governed".into()),
    };
    validate_service_name(module, &unit)?;
    validate_service_exec_start(&exec_start, module)?;

    let folder = module_root.join("integration/systemd-user");
    for directory in [module_root.join("integration"), folder.clone()] {
        let m = fs::symlink_metadata(&directory)
            .map_err(|e|format!("user service source directory: {e}"))?;
        if !m.file_type().is_dir() || m.mode() & 0o002 != 0 {
            return Err("unsafe user service source directory".into());
        }
    }
    let source = folder.join(&unit);
    let source_meta = fs::symlink_metadata(&source)
        .map_err(|e| format!("user service source unit: {e}"))?;
    if !source_meta.file_type().is_file() || source_meta.mode() & 0o002 != 0 {
        return Err("untrusted user service source unit".into());
    }
    let content = fs::read_to_string(&source)
        .map_err(|e| format!("cannot read user service unit: {e}"))?;
    certify_install_unit(&content, module, &exec_start)?;

    let (uid, gid) = crate::runtime_identity::desktop_identity()?;
    if uid == 0 { return Err("refusing root as desktop service user".into()); }
    let home = desktop_home(uid)?;
    let mut dir = home.clone();
    for segment in [".config", "systemd", "user"] {
        dir.push(segment);
        match fs::symlink_metadata(&dir) {
            Ok(meta) => {
                if !meta.file_type().is_dir() || meta.uid() != uid
                    || meta.gid() != gid || meta.mode() & 0o002 != 0 {
                    return Err(format!("unsafe systemd user directory {}", dir.display()));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&dir).map_err(|e|format!("create systemd directory: {e}"))?;
                std::os::unix::fs::chown(&dir, Some(uid), Some(gid))
                    .map_err(|e|format!("set systemd directory owner: {e}"))?;
                fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                    .map_err(|e|format!("secure systemd directory: {e}"))?;
            }
            Err(e) => return Err(format!("cannot inspect systemd user directory: {e}")),
        }
    }
    let target = dir.join(&unit);
    let created = match fs::symlink_metadata(&target) {
        Ok(meta) => {
            if !meta.file_type().is_file() || meta.uid() != uid || meta.gid() != gid
                || meta.mode() & 0o077 != 0 || fs::read_to_string(&target).ok().as_deref() != Some(&content)
            {
                return Err("existing systemd unit is foreign or modified; refusing overwrite".into());
            }
            false
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut file = fs::OpenOptions::new().write(true).create_new(true)
                .mode(0o600).open(&target)
                .map_err(|e|format!("create user service unit exclusively: {e}"))?;
            let rc = unsafe { libc::fchown(file.as_raw_fd(), uid, gid) };
            if rc != 0 {
                let _ = fs::remove_file(&target);
                return Err(format!("set user service unit owner: {}", std::io::Error::last_os_error()));
            }
            if let Err(e) = file.write_all(content.as_bytes()).and_then(|_|file.sync_all()) {
                let _ = fs::remove_file(&target);
                return Err(format!("write user service unit: {e}"));
            }
            true
        }
        Err(e) => return Err(format!("inspect user service unit: {e}")),
    };
    let result = (|| {
        systemctl_as_desktop(uid, gid, &home, &["daemon-reload"])?;
        validate_manager_fragment(uid, gid, &home, &unit, &target, &exec_start)?;
        systemctl_as_desktop(uid, gid, &home, &["enable", "--now", &unit])?;
        systemctl_as_desktop(uid, gid, &home, &["is-active", "--quiet", &unit])?;
        let enabled = systemctl_as_desktop(uid, gid, &home, &["is-enabled", &unit])?;
        if enabled != "enabled" { return Err("user service unit did not become enabled".into()); }
        validate_manager_fragment(uid, gid, &home, &unit, &target, &exec_start)?;
        Ok::<(), String>(())
    })();
    if let Err(error) = result {
        if created {
            let stop = systemctl_as_desktop(uid, gid, &home, &["disable", "--now", &unit]);
            if let Err(cleanup) = stop {
                return Err(format!("user service install failed: {error}; CRITICAL: rollback failed: {cleanup}"));
            }
            fs::remove_file(&target).map_err(|e|format!("user service rollback removal failed: {e}"))?;
            systemctl_as_desktop(uid, gid, &home, &["daemon-reload"])?;
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod user_service_install_contract_tests {
    use super::*;
    #[test]
    fn allow_minimal_certified_unit() {
        let unit = "[Unit]\nDescription=Secrets daemon\nAfter=basic.target\nConditionPathExists=/opt/neebles/modules/example/manifest.json\n\n[Service]\nType=simple\nExecStart=/opt/neebles/modules/example/bin/service --serve\nRestart=on-failure\nRestartSec=2\nUMask=0077\nNoNewPrivileges=true\n\n[Install]\nWantedBy=default.target\n";
        assert!(certify_install_unit(unit, "example", "/opt/neebles/modules/example/bin/service --serve").is_ok());
        assert!(certify_install_unit(&unit.replace("WantedBy=default.target", "WantedBy=multi-user.target"), "example", "/opt/neebles/modules/example/bin/service --serve").is_err());
        assert!(certify_install_unit(&unit.replace("Type=simple", "ExecStartPre=/bin/true\nType=simple"), "example", "/opt/neebles/modules/example/bin/service --serve").is_err());
    }
}

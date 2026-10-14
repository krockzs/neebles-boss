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

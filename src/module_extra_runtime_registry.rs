//! Owner-identity registry for extra module components, NOT for canonical Open.
//!
//! A distinct authority layer will authenticate physical owners (SO_PEERCRED,
//! PID incarnation and session) before calling register(). The registry stores
//! opaque component IDs; it NEVER treats extras as Governor Open instances.
//! This unit does not spawn, stop, signal or restore processes.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, RwLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraOwnerIdentity {
    pub pid: u32,
    pub start_time_ticks: u64,
    pub session_id: String,
}

#[derive(Debug, Default)]
pub struct ExtraRuntimeRegistry {
    owners: RwLock<BTreeMap<(String, String), ExtraOwnerIdentity>>,
    // NEEBLES_B9_UPDATE_GATE: serialized launch reservations vs module update.
    gate: Mutex<ExtraUpdateGate>,
    // None for isolated unit-test instances; global Boss authority uses flock.
    kernel_lock_root: Option<PathBuf>,
}

#[derive(Debug, Default)]
struct ExtraUpdateGate {
    updating: std::collections::BTreeSet<String>,
    launching: BTreeMap<String, usize>,
}

/// One reservation for an extra process from BEFORE spawn until the registry
/// has authenticated its PID. Drop releases it on all early-error paths.
pub struct ExtraLaunchPermit<'a> {
    registry: &'a ExtraRuntimeRegistry,
    module: String,
    // Kept open for the entire launch-to-registration interval.
    _kernel_gate: Option<File>,
}
impl Drop for ExtraLaunchPermit<'_> {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.registry.gate.lock() {
            if let Some(value) = guard.launching.get_mut(&self.module) {
                *value -= 1;
                if *value == 0 {
                    guard.launching.remove(&self.module);
                }
            }
        }
        release_kernel_gate(&mut self._kernel_gate);
    }
}

/// Update transaction guard: freezes new extra spawns until the caller returns.
/// Zero or more canonical Open processes are unaffected by this separate gate.
pub struct ExtraUpdatePermit<'a> {
    registry: &'a ExtraRuntimeRegistry,
    module: String,
    // Kept open across the complete CLI filesystem transaction and rollback.
    _kernel_gate: Option<File>,
}
impl Drop for ExtraUpdatePermit<'_> {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.registry.gate.lock() {
            guard.updating.remove(&self.module);
        }
        release_kernel_gate(&mut self._kernel_gate);
    }
}

// B19_EXPLICIT_FLOCK_RELEASE: release the kernel reservation before the
// guard finishes Drop. Do not depend solely on the timing of File field drop.
// File ownership is still retained until close; failed unlock is not ignored
// silently and update remains fail-closed on the next acquisition.
fn release_kernel_gate(gate: &mut Option<File>) {
    if let Some(file) = gate.take() {
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
        if rc != 0 {
            eprintln!(
                "NEEBLES extra kernel gate unlock failed: {}",
                std::io::Error::last_os_error()
            );
        }
        drop(file);
    }
}

static GOVERNED_EXTRA_OWNERS: OnceLock<ExtraRuntimeRegistry> = OnceLock::new();

/// Lifetime authority: only Boss-owned, kernel-verified spawned extras are inserted.
pub fn governed_extra_owners() -> &'static ExtraRuntimeRegistry {
    GOVERNED_EXTRA_OWNERS.get_or_init(|| ExtraRuntimeRegistry {
        kernel_lock_root: Some(PathBuf::from("/run/lock/neebles-extra-update")),
        ..ExtraRuntimeRegistry::new()
    })
}

fn validate_id(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return Err(format!("invalid {label} identity"));
    }
    Ok(())
}

/// Linux /proc stat field 22, indexed from the first field after `(comm)`.
/// This binds a PID to one kernel process incarnation, not a process name.
pub(crate) fn kernel_process_start_ticks(pid: u32) -> Result<Option<u64>, String> {
    kernel_start_time_ticks(pid)
}

fn kernel_start_time_ticks(pid: u32) -> Result<Option<u64>, String> {
    if pid == 0 {
        return Err("extra process PID cannot be zero".to_owned());
    }
    let path = format!("/proc/{pid}/stat");
    let source = match std::fs::read_to_string(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("extra owner kernel probe failed: {error}")),
    };
    let end = source.rfind(')').ok_or("malformed kernel process stat")?;
    // stat starts with PID, comm. Tail begins at field 3 (state).
    // starttime is field 22 => tail index 19.
    let tail = source
        .get(end + 1..)
        .ok_or("malformed kernel process stat tail")?;
    let value = tail
        .split_whitespace()
        .nth(19)
        .ok_or("kernel process stat missing starttime")?;
    let ticks = value
        .parse::<u64>()
        .map_err(|error| format!("invalid kernel process starttime: {error}"))?;
    if ticks == 0 {
        return Err("kernel process starttime cannot be zero".to_owned());
    }
    Ok(Some(ticks))
}

impl ExtraRuntimeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Linux flock is attached to the open file description, not process memory.
    /// Crash/exit releases the lock automatically. This is cross-process and
    /// separate from the singular canonical Open registry.
    fn acquire_isolated_test_gate(
        &self,
        module: &str,
        root: &std::path::Path,
    ) -> Result<Option<File>, String> {
        let Some(ref root) = self.kernel_lock_root else {
            return Ok(None);
        };
        validate_id("module", module)?;
        // Fail closed on a pre-existing unsafe directory. Create new root with 0700.
        if !root.exists() {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(root)
                .map_err(|e| format!("cannot create extra gate directory: {e}"))?;
        }
        let meta = std::fs::symlink_metadata(root)
            .map_err(|e| format!("cannot inspect extra gate directory: {e}"))?;
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || meta.uid() != unsafe { libc::geteuid() }
            || meta.mode() & 0o077 != 0
        {
            return Err("insecure extra gate directory ownership or permissions".into());
        }
        let path = root.join(format!("{module}.lock"));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|e| format!("cannot open extra gate: {e}"))?;
        let meta = file
            .metadata()
            .map_err(|e| format!("cannot stat extra gate: {e}"))?;
        if !meta.is_file()
            || meta.nlink() != 1
            || meta.uid() != unsafe { libc::geteuid() }
            || meta.mode() & 0o077 != 0
        {
            return Err("insecure extra gate file ownership or permissions".into());
        }
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            return Err(format!(
                "module '{module}' has concurrent extra launch/update: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Some(file))
    }

    // B14_SHARED_DESKTOP_UID: privileged CLI and desktop Boss must lock
    // the SAME inode under the actual desktop runtime, never per-euid /run/lock.
    // B15_UID_IDENTITY_CONSISTENCY: never silently prefer one mismatched
    // identity source. Privileged CLI and user Boss must resolve one UID.
    fn resolve_desktop_gate_uid(
        euid: u32,
        declared: Option<&str>,
        runtime_dir: Option<&str>,
    ) -> Result<u32, String> {
        let parsed_declared = match declared.filter(|v| !v.is_empty()) {
            Some(value) => Some(
                value
                    .parse::<u32>()
                    .map_err(|_| "invalid declared desktop UID".to_string())?,
            ),
            None => None,
        };
        let parsed_runtime = match runtime_dir.filter(|v| !v.is_empty()) {
            Some(value) => {
                let path = std::path::Path::new(value);
                let Some(uid_text) = path.file_name().and_then(|v| v.to_str()) else {
                    return Err("invalid desktop runtime directory".into());
                };
                // Do not accept a lookalike path with the correct final UID.
                let uid = uid_text
                    .parse::<u32>()
                    .map_err(|_| "invalid desktop runtime UID".to_string())?;
                if path != std::path::Path::new(&format!("/run/user/{uid}")) {
                    return Err(
                        "desktop runtime directory is not the canonical user runtime".into(),
                    );
                }
                Some(uid)
            }
            None => None,
        };
        if let (Some(declared), Some(runtime)) = (parsed_declared, parsed_runtime) {
            if declared != runtime {
                return Err("desktop runtime identity disagrees with XDG_RUNTIME_DIR".into());
            }
        }
        let uid = parsed_declared.or(parsed_runtime).unwrap_or(euid);
        if uid == 0 {
            return Err("desktop extra gate refuses root runtime identity".into());
        }
        if euid != 0 && uid != euid {
            return Err("unprivileged process cannot lock another desktop user's extras".into());
        }
        Ok(uid)
    }

    fn desktop_gate_uid() -> Result<u32, String> {
        let euid = unsafe { libc::geteuid() };
        let declared = std::env::var("NEEBLES_RUNTIME_IDENTITY").ok();
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok();
        Self::resolve_desktop_gate_uid(euid, declared.as_deref(), runtime_dir.as_deref())
    }

    fn acquire_kernel_gate(&self, module: &str) -> Result<Option<File>, String> {
        let Some(configured) = self.kernel_lock_root.as_ref() else {
            return Ok(None);
        };
        validate_id("module", module)?;
        // Unit-test fixtures use their own temporary gates; production is /run/user.
        if configured != &PathBuf::from("/run/lock/neebles-extra-update") {
            return self.acquire_isolated_test_gate(module, configured);
        }
        let uid = Self::desktop_gate_uid()?;
        let runtime = PathBuf::from(format!("/run/user/{uid}"));
        let runtime_meta = std::fs::symlink_metadata(&runtime)
            .map_err(|e| format!("desktop runtime directory unavailable: {e}"))?;
        if !runtime_meta.is_dir()
            || runtime_meta.file_type().is_symlink()
            || runtime_meta.uid() != uid
            || runtime_meta.mode() & 0o077 != 0
        {
            return Err("insecure desktop runtime directory for extra gate".into());
        }
        let root = runtime.join("neebles-extra-update");
        if !root.exists() {
            match std::fs::DirBuilder::new().mode(0o700).create(&root) {
                Ok(()) => {
                    if unsafe { libc::geteuid() } == 0 {
                        let path = std::ffi::CString::new(root.as_os_str().as_bytes())
                            .map_err(|_| "invalid extra gate directory")?;
                        if unsafe { libc::chown(path.as_ptr(), uid, uid) } != 0 {
                            return Err(format!(
                                "extra gate chown failed: {}",
                                std::io::Error::last_os_error()
                            ));
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(format!("cannot create extra gate directory: {e}")),
            }
        }
        let meta = std::fs::symlink_metadata(&root)
            .map_err(|e| format!("cannot inspect extra gate directory: {e}"))?;
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || meta.uid() != uid
            || meta.mode() & 0o077 != 0
        {
            return Err("insecure extra gate directory owner/permissions".into());
        }
        let path = root.join(format!("{module}.lock"));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|e| format!("cannot open extra gate: {e}"))?;
        let mut meta = file
            .metadata()
            .map_err(|e| format!("cannot stat extra gate: {e}"))?;
        if meta.uid() == 0 && unsafe { libc::geteuid() } == 0 {
            if unsafe { libc::fchown(file.as_raw_fd(), uid, uid) } != 0 {
                return Err(format!(
                    "extra gate fchown failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            meta = file
                .metadata()
                .map_err(|e| format!("cannot re-stat extra gate: {e}"))?;
        }
        if !meta.is_file() || meta.nlink() != 1 || meta.uid() != uid || meta.mode() & 0o077 != 0 {
            return Err("insecure extra gate file owner/permissions".into());
        }
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(format!(
                "module '{module}' has concurrent extra launch/update: {}",
                std::io::Error::last_os_error()
            ));
        }
        // If the pathname changed between open and flock, refuse a stale inode.
        let linked = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("extra gate pathname disappeared: {e}"))?;
        if linked.file_type().is_symlink()
            || linked.ino() != meta.ino()
            || linked.dev() != meta.dev()
        {
            return Err("extra gate pathname changed while acquiring lock".into());
        }
        Ok(Some(file))
    }

    /// Reserve BEFORE spawning the child, preventing the spawn-to-register race.
    pub fn reserve_extra_launch(&self, module: &str) -> Result<ExtraLaunchPermit<'_>, String> {
        validate_id("module", module)?;
        let kernel_gate = self.acquire_kernel_gate(module)?;
        let mut state = self.gate.lock().map_err(|_| "extra update gate poisoned")?;
        if state.updating.contains(module) {
            return Err(format!(
                "module '{module}' is updating; extra launch denied"
            ));
        }
        *state.launching.entry(module.to_owned()).or_default() += 1;
        Ok(ExtraLaunchPermit {
            registry: self,
            module: module.to_owned(),
            _kernel_gate: kernel_gate,
        })
    }

    /// Acquires the update barrier only when no extra is launching. Combined
    /// with B8's owner check under this permit, no new extra can slip in.
    pub fn reserve_module_update(&self, module: &str) -> Result<ExtraUpdatePermit<'_>, String> {
        validate_id("module", module)?;
        let kernel_gate = self.acquire_kernel_gate(module)?;
        let mut state = self.gate.lock().map_err(|_| "extra update gate poisoned")?;
        if state.updating.contains(module) || state.launching.get(module).copied().unwrap_or(0) != 0
        {
            return Err(format!(
                "module '{module}' has an update or pending extra launch"
            ));
        }
        state.updating.insert(module.to_owned());
        Ok(ExtraUpdatePermit {
            registry: self,
            module: module.to_owned(),
            _kernel_gate: kernel_gate,
        })
    }

    /// The caller must first obtain PID from an authenticated transport
    /// (SO_PEERCRED or equally strong authority). This independently checks
    /// its exact kernel incarnation before insertion.
    pub fn register_kernel_verified(
        &self,
        module: &str,
        extra: &str,
        identity: ExtraOwnerIdentity,
    ) -> Result<(), String> {
        let actual = kernel_start_time_ticks(identity.pid)?;
        if actual != Some(identity.start_time_ticks) {
            return Err(format!(
                "extra '{extra}' for module '{module}' kernel process identity mismatch"
            ));
        }
        self.register_authenticated(module, extra, identity)
    }

    /// Fail closed if a registered process exited or its PID was reused.
    /// Never silently convert a stale owner into an inactive component.
    pub fn get_kernel_verified(
        &self,
        module: &str,
        extra: &str,
    ) -> Result<Option<ExtraOwnerIdentity>, String> {
        let owner = self.get(module, extra)?;
        if let Some(ref record) = owner {
            if kernel_start_time_ticks(record.pid)? != Some(record.start_time_ticks) {
                return Err(format!(
                    "extra '{extra}' for module '{module}' has stale kernel ownership"
                ));
            }
        }
        Ok(owner)
    }

    /// Call ONLY after the owning IPC boundary has verified the physical peer.
    /// Never replace an existing owner just because its PID or session differs.
    pub fn register_authenticated(
        &self,
        module: &str,
        extra: &str,
        identity: ExtraOwnerIdentity,
    ) -> Result<(), String> {
        validate_id("module", module)?;
        validate_id("extra", extra)?;
        if matches!(extra, "open" | "governor.open") {
            return Err("canonical Governor Open cannot be an extra".into());
        }
        if identity.pid == 0
            || identity.start_time_ticks == 0
            || identity.session_id.is_empty()
            || identity.session_id.len() > 256
        {
            return Err("invalid authenticated extra owner identity".into());
        }
        let mut owners = self
            .owners
            .write()
            .map_err(|_| "extra registry lock poisoned")?;
        let key = (module.to_owned(), extra.to_owned());
        if let Some(existing) = owners.get(&key) {
            if existing == &identity {
                return Ok(());
            }
            return Err(format!(
                "extra '{extra}' for module '{module}' already has an owner"
            ));
        }
        owners.insert(key, identity);
        Ok(())
    }

    /// B17_PIDFD_SAFE_STOP: signal ONLY a physically registered process
    /// incarnation, with its exact module/extra/session identity. The pidfd
    /// remains bound to the kernel task even if a numeric PID gets recycled.
    /// This is a signal primitive, NOT a completed stop transaction; callers
    /// must reap/wait and independently verify quiescence before publication.
    /// No production update or IPC action invokes this until restore exists.
    pub fn request_verified_extra_stop(
        &self,
        module: &str,
        extra: &str,
        expected: &ExtraOwnerIdentity,
    ) -> Result<(), String> {
        validate_id("module", module)?;
        validate_id("extra", extra)?;
        if matches!(extra, "open" | "governor.open") {
            return Err("canonical Open is not an extra stop target".into());
        }
        // Registry is the authority. Refuse stale session/incarnation instead
        // of interpreting a missing owner as a successful stop.
        if self.get(module, extra)?.as_ref() != Some(expected) {
            return Err("extra stop owner identity differs from registry".into());
        }
        if kernel_start_time_ticks(expected.pid)? != Some(expected.start_time_ticks) {
            return Err("extra stop kernel incarnation is stale".into());
        }
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, expected.pid as libc::pid_t, 0u32) };
        if fd < 0 {
            return Err(format!(
                "extra stop pidfd_open failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        use std::os::fd::{FromRawFd, OwnedFd};
        // SAFETY: pidfd_open returned a fresh owned descriptor.
        let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
        // Verify AGAIN after opening pidfd, to close the PID-reuse window
        // between the first /proc probe and pidfd_open.
        if self.get(module, extra)?.as_ref() != Some(expected)
            || kernel_start_time_ticks(expected.pid)? != Some(expected.start_time_ticks)
        {
            return Err("extra stop owner changed while acquiring pidfd".into());
        }
        let rc = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                libc::SIGTERM,
                std::ptr::null::<libc::siginfo_t>(),
                0u32,
            )
        };
        if rc != 0 {
            return Err(format!(
                "extra stop pidfd signal failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    /// B18_VERIFIED_EXTRA_EXIT_WAIT: SIGTERM alone is NOT proof of quiescence.
    /// The owning waiter must first reap its Child and unregister_exact();
    /// only then can update consider this extra physically absent.
    /// No production update or IPC action invokes this before recovery exists.
    pub fn wait_verified_extra_exit(
        &self,
        module: &str,
        extra: &str,
        expected: &ExtraOwnerIdentity,
        timeout: std::time::Duration,
    ) -> Result<(), String> {
        validate_id("module", module)?;
        validate_id("extra", extra)?;
        if matches!(extra, "open" | "governor.open") {
            return Err("canonical Open is not an extra wait target".into());
        }
        if expected.pid == 0 || expected.start_time_ticks == 0 || expected.session_id.is_empty() {
            return Err("invalid expected extra owner for exit wait".into());
        }
        if timeout.is_zero() {
            return Err("extra exit wait timeout must be positive".into());
        }
        // Never accept a fabricated/nonexistent owner as a completed stop.
        if self.get(module, extra)?.as_ref() != Some(expected) {
            return Err("extra exit wait requires exact registered owner".into());
        }
        let deadline = std::time::Instant::now()
            .checked_add(timeout)
            .ok_or("extra exit wait timeout overflow")?;
        loop {
            match self.get(module, extra)? {
                Some(current) if current != *expected => {
                    return Err("extra owner changed before quiescence was confirmed".into());
                }
                Some(_) => {} // Reaping/unregister has NOT happened yet.
                None => {
                    // A vanished registry entry is insufficient when its exact
                    // kernel incarnation is still present (e.g. bad cleanup).
                    if kernel_start_time_ticks(expected.pid)? == Some(expected.start_time_ticks) {
                        return Err(
                            "extra owner unregistered while kernel incarnation still exists".into(),
                        );
                    }
                    return Ok(());
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "extra '{extra}' for module '{module}' did not quiesce before timeout"
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// B20_STOP_WAIT_RACE_CLOSED: composite stop. The owner may reap and
    /// unregister between pidfd_send_signal and the first wait observation.
    /// Only an owner matched BEFORE signal is eligible; after signal an absent
    /// entry is accepted solely when the original kernel incarnation is gone.
    /// This is deliberately NOT exposed to IPC or update before restore exists.
    pub fn stop_and_confirm_exact_extra(
        &self,
        module: &str,
        extra: &str,
        expected: &ExtraOwnerIdentity,
        timeout: std::time::Duration,
    ) -> Result<(), String> {
        if timeout.is_zero() {
            return Err("extra stop confirmation timeout must be positive".into());
        }
        self.request_verified_extra_stop(module, extra, expected)?;
        let deadline = std::time::Instant::now()
            .checked_add(timeout)
            .ok_or("extra stop confirmation timeout overflow")?;
        loop {
            match self.get(module, extra)? {
                Some(current) if current != *expected => {
                    return Err("extra owner changed while waiting for stop".into());
                }
                Some(_) => {} // Reaper has not unregistered the exact child.
                None => {
                    if kernel_start_time_ticks(expected.pid)? == Some(expected.start_time_ticks) {
                        return Err(
                            "extra unregistered while original kernel process remains".into()
                        );
                    }
                    return Ok(());
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "extra '{extra}' for module '{module}' not reaped by deadline"
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Unregister only the exact authenticated owner incarnation.
    /// A delayed disconnect cannot clear an owner registered later.
    pub fn unregister_exact(
        &self,
        module: &str,
        extra: &str,
        identity: &ExtraOwnerIdentity,
    ) -> Result<bool, String> {
        let mut owners = self
            .owners
            .write()
            .map_err(|_| "extra registry lock poisoned")?;
        let key = (module.to_owned(), extra.to_owned());
        if owners.get(&key) != Some(identity) {
            return Ok(false);
        }
        owners.remove(&key);
        Ok(true)
    }

    pub fn get(&self, module: &str, extra: &str) -> Result<Option<ExtraOwnerIdentity>, String> {
        let owners = self
            .owners
            .read()
            .map_err(|_| "extra registry lock poisoned")?;
        Ok(owners.get(&(module.to_owned(), extra.to_owned())).cloned())
    }

    /// Read-only, module-scoped physical snapshot. It is NOT permission to
    /// terminate anything: consumer must revalidate kernel identity at action.
    /// A stopped/reused PID fails closed rather than being treated as inactive.
    pub fn verified_owner_snapshot(
        &self,
        module: &str,
    ) -> Result<BTreeMap<String, ExtraOwnerIdentity>, String> {
        validate_id("module", module)?;
        let gate = self.gate.lock().map_err(|_| "extra update gate poisoned")?;
        if gate.updating.contains(module) || gate.launching.get(module).copied().unwrap_or(0) != 0 {
            return Err(format!(
                "module '{module}' has a pending extra launch/update"
            ));
        }
        let owners = self.module_owners(module)?;
        for (extra, identity) in &owners {
            if self.get_kernel_verified(module, extra)?.as_ref() != Some(identity) {
                return Err(format!(
                    "module '{module}' extra '{extra}' owner changed during snapshot"
                ));
            }
        }
        drop(gate);
        Ok(owners)
    }

    /// IPC-read-only preflight from the persistent Boss authority.
    /// This is NOT an update reservation: a cross-process transaction lease
    /// is still required before automatic extra close/restore is enabled.
    pub fn persistent_update_preflight(&self, module: &str) -> Result<(), String> {
        validate_id("module", module)?;
        let gate = self.gate.lock().map_err(|_| "extra update gate poisoned")?;
        if gate.updating.contains(module) || gate.launching.get(module).copied().unwrap_or(0) > 0 {
            return Err(format!(
                "module '{module}' has a pending governed extra launch or update"
            ));
        }
        // Hold launch gate until we finish checking registered owners.
        self.require_no_owned_extras_for_update(module)
    }

    /// Fail closed until update transaction supports independently owned extras.
    /// A registered extra is not equivalent to the singular Open or Tray Manager.
    /// Do not allow it to survive a physical module-directory replacement.
    pub fn require_no_owned_extras_for_update(&self, module: &str) -> Result<(), String> {
        let owners = self.module_owners(module)?;
        for (extra, expected) in owners {
            // Reject stale or PID-reused owners too: never infer a safe stop.
            let actual = self.get_kernel_verified(module, &extra)?;
            if actual.as_ref() != Some(&expected) {
                return Err(format!(
                    "module '{module}' extra '{extra}' changed owner during update preflight"
                ));
            }
            return Err(format!(
                "module '{module}' has governed extra '{extra}'; update requires verified extra quiescence and restoration (not implemented yet)"
            ));
        }
        Ok(())
    }

    /// Deterministic module-scoped inventory; unrelated modules are excluded.
    pub fn module_owners(
        &self,
        module: &str,
    ) -> Result<BTreeMap<String, ExtraOwnerIdentity>, String> {
        let owners = self
            .owners
            .read()
            .map_err(|_| "extra registry lock poisoned")?;
        Ok(owners
            .iter()
            .filter_map(|((owner_module, extra), identity)| {
                (owner_module == module).then(|| (extra.clone(), identity.clone()))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn owner(pid: u32, session: &str) -> ExtraOwnerIdentity {
        ExtraOwnerIdentity {
            pid,
            start_time_ticks: pid as u64 * 100,
            session_id: session.into(),
        }
    }

    #[test]
    fn verified_extra_requires_matching_kernel_incarnation() {
        let registry = ExtraRuntimeRegistry::new();
        let pid = std::process::id();
        let ticks = kernel_start_time_ticks(pid).unwrap().unwrap();
        let identity = ExtraOwnerIdentity {
            pid,
            start_time_ticks: ticks,
            session_id: "verified-session".into(),
        };
        registry
            .register_kernel_verified("alpha", "sensor", identity.clone())
            .unwrap();
        assert_eq!(
            registry.get_kernel_verified("alpha", "sensor").unwrap(),
            Some(identity)
        );
    }

    #[test]
    fn b17_pidfd_stop_rejects_wrong_session_without_signalling() {
        let registry = ExtraRuntimeRegistry::new();
        let pid = std::process::id();
        let identity = ExtraOwnerIdentity {
            pid,
            start_time_ticks: kernel_start_time_ticks(pid).unwrap().unwrap(),
            session_id: "owner-session".into(),
        };
        registry
            .register_kernel_verified("alpha", "worker", identity.clone())
            .unwrap();
        let mut fake = identity.clone();
        fake.session_id = "other-session".into();
        assert!(registry
            .request_verified_extra_stop("alpha", "worker", &fake)
            .is_err());
        assert!(registry
            .request_verified_extra_stop("beta", "worker", &identity)
            .is_err());
        assert!(registry
            .request_verified_extra_stop("alpha", "governor.open", &identity)
            .is_err());
    }

    #[test]
    fn b17_pidfd_stop_terminates_only_owned_test_child() {
        use std::process::Command;
        let registry = ExtraRuntimeRegistry::new();
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        let ticks = kernel_start_time_ticks(pid).unwrap().unwrap();
        let identity = ExtraOwnerIdentity {
            pid,
            start_time_ticks: ticks,
            session_id: "test-child-session".into(),
        };
        registry
            .register_kernel_verified("alpha", "test-child", identity.clone())
            .unwrap();
        let stop = registry.request_verified_extra_stop("alpha", "test-child", &identity);
        if stop.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        stop.unwrap();
        let exit = child.wait().unwrap();
        assert!(!exit.success());
        assert!(registry
            .unregister_exact("alpha", "test-child", &identity)
            .unwrap());
        assert!(registry.get("alpha", "test-child").unwrap().is_none());
    }

    #[test]
    fn b18_exit_wait_refuses_unreaped_owner_and_wrong_open() {
        let registry = ExtraRuntimeRegistry::new();
        let pid = std::process::id();
        let identity = ExtraOwnerIdentity {
            pid,
            start_time_ticks: kernel_start_time_ticks(pid).unwrap().unwrap(),
            session_id: "wait-test".into(),
        };
        registry
            .register_kernel_verified("alpha", "worker", identity.clone())
            .unwrap();
        assert!(registry
            .wait_verified_extra_exit(
                "alpha",
                "worker",
                &identity,
                std::time::Duration::from_millis(20)
            )
            .is_err());
        assert!(registry
            .wait_verified_extra_exit(
                "alpha",
                "governor.open",
                &identity,
                std::time::Duration::from_millis(20)
            )
            .is_err());
        assert!(registry
            .unregister_exact("alpha", "worker", &identity)
            .unwrap());
        assert!(registry
            .wait_verified_extra_exit(
                "alpha",
                "worker",
                &identity,
                std::time::Duration::from_millis(20)
            )
            .is_err());
    }

    #[test]
    fn b18_exit_wait_accepts_only_reaped_exact_owner() {
        use std::process::Command;
        let registry = std::sync::Arc::new(ExtraRuntimeRegistry::new());
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let identity = ExtraOwnerIdentity {
            pid: child.id(),
            start_time_ticks: kernel_start_time_ticks(child.id()).unwrap().unwrap(),
            session_id: "wait-reaped".into(),
        };
        registry
            .register_kernel_verified("alpha", "worker", identity.clone())
            .unwrap();
        let waiter_registry = std::sync::Arc::clone(&registry);
        let waiter_identity = identity.clone();
        let reaper = std::thread::spawn(move || {
            let _ = child.kill();
            child.wait().unwrap();
            waiter_registry
                .unregister_exact("alpha", "worker", &waiter_identity)
                .unwrap();
        });
        let result = registry.wait_verified_extra_exit(
            "alpha",
            "worker",
            &identity,
            std::time::Duration::from_secs(3),
        );
        reaper.join().unwrap();
        result.unwrap();
    }

    #[test]
    fn b20_stop_accepts_fast_reaper_before_first_poll() {
        use std::process::Command;
        let registry = std::sync::Arc::new(ExtraRuntimeRegistry::new());
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let identity = ExtraOwnerIdentity {
            pid: child.id(),
            start_time_ticks: kernel_start_time_ticks(child.id()).unwrap().unwrap(),
            session_id: "b20-fast-reaper".into(),
        };
        registry
            .register_kernel_verified("alpha", "worker", identity.clone())
            .unwrap();
        let waiter_registry = std::sync::Arc::clone(&registry);
        let waiter_identity = identity.clone();
        let waiter = std::thread::spawn(move || {
            child.wait().unwrap();
            waiter_registry
                .unregister_exact("alpha", "worker", &waiter_identity)
                .unwrap();
        });
        let result = registry.stop_and_confirm_exact_extra(
            "alpha",
            "worker",
            &identity,
            std::time::Duration::from_secs(3),
        );
        waiter.join().unwrap();
        result.unwrap();
    }

    #[test]
    fn b20_stop_rejects_mismatched_owner_before_signal() {
        let registry = ExtraRuntimeRegistry::new();
        let pid = std::process::id();
        let identity = ExtraOwnerIdentity {
            pid,
            start_time_ticks: kernel_start_time_ticks(pid).unwrap().unwrap(),
            session_id: "b20-real".into(),
        };
        registry
            .register_kernel_verified("alpha", "worker", identity.clone())
            .unwrap();
        let mut fake = identity.clone();
        fake.session_id = "b20-fake".into();
        assert!(registry
            .stop_and_confirm_exact_extra(
                "alpha",
                "worker",
                &fake,
                std::time::Duration::from_millis(20)
            )
            .is_err());
        assert_eq!(registry.get("alpha", "worker").unwrap(), Some(identity));
    }

    #[test]
    fn fabricated_kernel_starttime_is_rejected() {
        let registry = ExtraRuntimeRegistry::new();
        let pid = std::process::id();
        let ticks = kernel_start_time_ticks(pid).unwrap().unwrap();
        let wrong = ExtraOwnerIdentity {
            pid,
            start_time_ticks: ticks + 1,
            session_id: "fake-session".into(),
        };
        assert!(registry
            .register_kernel_verified("alpha", "fake", wrong)
            .is_err());
        assert!(registry.get("alpha", "fake").unwrap().is_none());
    }

    #[test]
    fn b9_pending_extra_launch_blocks_update_until_registration_finishes() {
        let registry = ExtraRuntimeRegistry::new();
        let permit = registry.reserve_extra_launch("alpha").unwrap();
        assert!(registry.reserve_module_update("alpha").is_err());
        drop(permit);
        assert!(registry.reserve_module_update("alpha").is_ok());
    }

    #[test]
    fn b9_update_prevents_extra_spawning_but_not_other_modules() {
        let registry = ExtraRuntimeRegistry::new();
        let update = registry.reserve_module_update("alpha").unwrap();
        assert!(registry.reserve_extra_launch("alpha").is_err());
        assert!(registry.reserve_module_update("alpha").is_err());
        assert!(registry.reserve_extra_launch("beta").is_ok());
        drop(update);
        assert!(registry.reserve_extra_launch("alpha").is_ok());
    }

    #[test]
    fn b9_update_permit_drops_on_error_path() {
        fn simulate(registry: &ExtraRuntimeRegistry) -> Result<(), String> {
            let _guard = registry.reserve_module_update("module")?;
            Err("simulated publication failure".into())
        }
        let registry = ExtraRuntimeRegistry::new();
        assert!(simulate(&registry).is_err());
        assert!(registry.reserve_extra_launch("module").is_ok());
    }

    #[test]
    fn b8_guard_allows_modules_without_registered_extras() {
        let registry = ExtraRuntimeRegistry::new();
        assert!(registry.require_no_owned_extras_for_update("clean").is_ok());
        registry
            .register_authenticated("other", "cpu", owner(55, "other-session"))
            .unwrap();
        assert!(registry.require_no_owned_extras_for_update("clean").is_ok());
    }

    #[test]
    fn b8_guard_blocks_known_extra_before_update() {
        let registry = ExtraRuntimeRegistry::new();
        let pid = std::process::id();
        let ticks = kernel_process_start_ticks(pid).unwrap().unwrap();
        registry
            .register_kernel_verified(
                "monitor",
                "sensor",
                ExtraOwnerIdentity {
                    pid,
                    start_time_ticks: ticks,
                    session_id: "live-extra".into(),
                },
            )
            .unwrap();
        let err = registry
            .require_no_owned_extras_for_update("monitor")
            .unwrap_err();
        assert!(err.contains("sensor"));
        assert!(err.contains("not implemented yet"));
    }

    #[test]
    fn one_open_remains_outside_extra_registry() {
        let registry = ExtraRuntimeRegistry::new();
        assert!(registry
            .register_authenticated("alpha", "open", owner(10, "a"))
            .is_err());
        assert!(registry
            .register_authenticated("alpha", "governor.open", owner(10, "a"))
            .is_err());
        assert!(registry.module_owners("alpha").unwrap().is_empty());
    }

    #[test]
    fn extras_are_unbounded_by_type_and_scoped_by_module() {
        let registry = ExtraRuntimeRegistry::new();
        for i in 1..=24 {
            registry
                .register_authenticated("alpha", &format!("extra{i}"), owner(i, &format!("s{i}")))
                .unwrap();
        }
        registry
            .register_authenticated("beta", "extra1", owner(100, "other"))
            .unwrap();
        assert_eq!(registry.module_owners("alpha").unwrap().len(), 24);
        assert_eq!(registry.module_owners("beta").unwrap().len(), 1);
        assert_eq!(registry.get("alpha", "extra1").unwrap().unwrap().pid, 1);
    }

    #[test]
    fn b12_snapshot_is_module_scoped_and_kernel_verified() {
        let registry = ExtraRuntimeRegistry::new();
        let pid = std::process::id();
        let ticks = kernel_start_time_ticks(pid).unwrap().unwrap();
        let identity = ExtraOwnerIdentity {
            pid,
            start_time_ticks: ticks,
            session_id: "physical".into(),
        };
        registry
            .register_kernel_verified("alpha", "background", identity.clone())
            .unwrap();
        registry
            .register_kernel_verified("beta", "worker", identity.clone())
            .unwrap();
        let alpha = registry.verified_owner_snapshot("alpha").unwrap();
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha.get("background"), Some(&identity));
        assert!(alpha.get("worker").is_none());
        assert_eq!(registry.verified_owner_snapshot("gamma").unwrap().len(), 0);
    }

    #[test]
    fn b12_snapshot_rejects_pending_extra_launch() {
        let registry = ExtraRuntimeRegistry::new();
        let launch = registry.reserve_extra_launch("alpha").unwrap();
        assert!(registry.verified_owner_snapshot("alpha").is_err());
        assert!(registry.verified_owner_snapshot("beta").is_ok());
        drop(launch);
        assert!(registry.verified_owner_snapshot("alpha").is_ok());
    }

    #[test]
    fn b15_root_cli_and_user_boss_resolve_same_desktop_uid() {
        assert_eq!(
            ExtraRuntimeRegistry::resolve_desktop_gate_uid(0, Some("1000"), Some("/run/user/1000"))
                .unwrap(),
            1000
        );
        assert_eq!(
            ExtraRuntimeRegistry::resolve_desktop_gate_uid(1000, None, Some("/run/user/1000"))
                .unwrap(),
            1000
        );
    }

    #[test]
    fn b15_mismatched_identity_sources_fail_closed() {
        assert!(ExtraRuntimeRegistry::resolve_desktop_gate_uid(
            0,
            Some("1000"),
            Some("/run/user/1001")
        )
        .is_err());
        assert!(ExtraRuntimeRegistry::resolve_desktop_gate_uid(1000, Some("1001"), None).is_err());
        assert!(ExtraRuntimeRegistry::resolve_desktop_gate_uid(0, None, None).is_err());
    }

    #[test]
    fn b15_runtime_path_must_be_canonical() {
        assert!(ExtraRuntimeRegistry::resolve_desktop_gate_uid(
            1000,
            None,
            Some("/tmp/run/user/1000")
        )
        .is_err());
        assert!(ExtraRuntimeRegistry::resolve_desktop_gate_uid(
            1000,
            None,
            Some("/run/user/1000/../1000")
        )
        .is_err());
    }

    #[test]
    fn b11_kernel_gate_serializes_two_independent_registry_instances() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("locks");
        let first = ExtraRuntimeRegistry {
            kernel_lock_root: Some(root.clone()),
            ..ExtraRuntimeRegistry::new()
        };
        let second = ExtraRuntimeRegistry {
            kernel_lock_root: Some(root),
            ..ExtraRuntimeRegistry::new()
        };
        let update = first.reserve_module_update("same_module").unwrap();
        assert!(second.reserve_extra_launch("same_module").is_err());
        assert!(second.reserve_module_update("same_module").is_err());
        assert!(second.reserve_extra_launch("another_module").is_ok());
        drop(update);
        let launch = second.reserve_extra_launch("same_module").unwrap();
        assert!(first.reserve_module_update("same_module").is_err());
        drop(launch);
        assert!(first.reserve_module_update("same_module").is_ok());
    }

    #[test]
    fn b19_release_reacquires_same_kernel_inode_repeatedly() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("locks");
        let first = ExtraRuntimeRegistry {
            kernel_lock_root: Some(root.clone()),
            ..ExtraRuntimeRegistry::new()
        };
        let second = ExtraRuntimeRegistry {
            kernel_lock_root: Some(root),
            ..ExtraRuntimeRegistry::new()
        };
        for _ in 0..100 {
            {
                let update = first.reserve_module_update("same_module").unwrap();
                assert!(second.reserve_extra_launch("same_module").is_err());
                drop(update);
            }
            {
                let launch = second.reserve_extra_launch("same_module").unwrap();
                assert!(first.reserve_module_update("same_module").is_err());
                drop(launch);
            }
        }
        assert!(first.reserve_module_update("same_module").is_ok());
    }

    #[test]
    fn duplicate_owner_cannot_steal_registration() {
        let registry = ExtraRuntimeRegistry::new();
        registry
            .register_authenticated("alpha", "sensor", owner(10, "a"))
            .unwrap();
        registry
            .register_authenticated("alpha", "sensor", owner(10, "a"))
            .unwrap();
        assert!(registry
            .register_authenticated("alpha", "sensor", owner(11, "b"))
            .is_err());
        assert_eq!(
            registry.get("alpha", "sensor").unwrap(),
            Some(owner(10, "a"))
        );
    }

    #[test]
    fn stale_disconnect_cannot_clear_new_session() {
        let registry = ExtraRuntimeRegistry::new();
        let old = owner(10, "old");
        let new = owner(11, "new");
        registry
            .register_authenticated("alpha", "sensor", old.clone())
            .unwrap();
        assert!(registry.unregister_exact("alpha", "sensor", &old).unwrap());
        registry
            .register_authenticated("alpha", "sensor", new.clone())
            .unwrap();
        assert!(!registry.unregister_exact("alpha", "sensor", &old).unwrap());
        assert_eq!(registry.get("alpha", "sensor").unwrap(), Some(new));
    }

    #[test]
    fn invalid_id_and_zero_pid_fail_closed() {
        let registry = ExtraRuntimeRegistry::new();
        assert!(registry
            .register_authenticated("../bad", "x", owner(1, "a"))
            .is_err());
        assert!(registry
            .register_authenticated("alpha", "../bad", owner(1, "a"))
            .is_err());
        assert!(registry
            .register_authenticated("alpha", "ok", owner(0, "a"))
            .is_err());
        assert!(registry.module_owners("alpha").unwrap().is_empty());
    }
}

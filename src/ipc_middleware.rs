//! Shared execution policy for Boss administrative and authenticated module IPC.
//!
//! Deadlines bound the request's wait, not an already-running side effect.
//! A timed-out operation remains observable by its owner via its job ID.
//! No module-specific timeout rules or privileges are defined here.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionOptions {
    #[serde(rename = "async", default)]
    pub asynchronous: bool,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Retries ONLY before a request has been dispatched to the provider.
    /// Omitted execution metadata remains legacy (zero automatic retries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Administrative,
    Module,
}

#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub asynchronous: bool,
    pub timeout: Option<Duration>,
}

const DEFAULT_MAX_TIMEOUT_MS: u64 = 300_000;
// After a caller's finite wait expires, preserve the provider's pending response
// for a separately bounded interval. The caller's deadline is NOT cancellation.
const DEFAULT_LATE_RESPONSE_GRACE_MS: u64 = 30_000;
const DEFAULT_MAX_ACTIVE_JOBS: usize = 32;
const DEFAULT_PREDISPATCH_RETRIES: u8 = 3;
const ABSOLUTE_MAX_PREDISPATCH_RETRIES: u64 = 10;
const DEFAULT_RETRY_DELAY_MS: u64 = 100;
const MAX_RETRY_DELAY_MS: u64 = 1000;
const DEFAULT_MAX_RETAINED_JOBS: usize = 256;
const JOB_RETENTION: Duration = Duration::from_secs(600);

fn env_limit(name: &str, fallback: u64, ceiling: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| (1..=ceiling).contains(value))
        .unwrap_or(fallback)
}

impl Policy {
    /// Absent metadata MUST preserve the original synchronous/legacy behavior.
    pub fn resolve(options: Option<&ExecutionOptions>, _surface: Surface) -> Result<Self, String> {
        let Some(options) = options else {
            return Ok(Self {
                asynchronous: false,
                timeout: None,
            });
        };
        let limit = env_limit(
            "NEEBLES_IPC_MAX_TIMEOUT_MS",
            DEFAULT_MAX_TIMEOUT_MS,
            3_600_000,
        );
        let ms = options.timeout_ms.ok_or_else(|| {
            "execution.timeout_ms is required when execution metadata is supplied".to_string()
        })?;
        if ms == 0 || ms > limit {
            return Err(format!("execution.timeout_ms must be within 1..={limit}"));
        }
        Ok(Self {
            asynchronous: options.asynchronous,
            timeout: Some(Duration::from_millis(ms)),
        })
    }
}

impl Policy {
    /// The module transport has a separate, bounded opportunity to deliver a
    /// response after the requester's waiting deadline has expired. Otherwise
    /// client::invoke() cancels its pending request at exactly the same timeout
    /// and the middleware's `completed_late` job loses the provider response.
    /// Legacy (no execution metadata) remains unbounded, as before.
    pub fn module_provider_wait(&self) -> Option<Duration> {
        let grace = env_limit(
            "NEEBLES_IPC_LATE_RESPONSE_GRACE_MS",
            DEFAULT_LATE_RESPONSE_GRACE_MS,
            300_000,
        );
        self.timeout
            .map(|timeout| timeout.saturating_add(Duration::from_millis(grace)))
    }
}

/// These retries are strictly confined to the provider-not-yet-registered state.
/// Once dispatch is possible, exactly ONE Invoke is sent. A late/unknown outcome
/// MUST be reconciled using job_id, never re-issued blindly.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_retries: u8,
    pub initial_delay: Duration,
}

impl RetryPolicy {
    pub fn legacy() -> Self {
        Self {
            max_retries: 0,
            initial_delay: Duration::from_millis(DEFAULT_RETRY_DELAY_MS),
        }
    }

    pub fn for_module(options: Option<&ExecutionOptions>) -> Result<Self, String> {
        let Some(options) = options else {
            return Ok(Self::legacy());
        };
        let max_retries = options.max_retries.unwrap_or(DEFAULT_PREDISPATCH_RETRIES);
        let allowed = env_limit(
            "NEEBLES_IPC_MAX_RETRIES",
            u64::from(DEFAULT_PREDISPATCH_RETRIES),
            ABSOLUTE_MAX_PREDISPATCH_RETRIES,
        );
        if u64::from(max_retries) > allowed {
            return Err(format!(
                "execution.max_retries must be within 0..={allowed}"
            ));
        }
        let delay_ms = options.retry_delay_ms.unwrap_or_else(|| {
            env_limit(
                "NEEBLES_IPC_DEFAULT_RETRY_DELAY_MS",
                DEFAULT_RETRY_DELAY_MS,
                MAX_RETRY_DELAY_MS,
            )
        });
        if !(1..=MAX_RETRY_DELAY_MS).contains(&delay_ms) {
            return Err(format!(
                "execution.retry_delay_ms must be within 1..={MAX_RETRY_DELAY_MS}"
            ));
        }
        Ok(Self {
            max_retries,
            initial_delay: Duration::from_millis(delay_ms),
        })
    }

    /// Wait for runtime registration without sending a message. Three retries
    /// mean up to four availability checks. All waiting consumes the overall
    /// module transport deadline; no operation is submitted twice.
    pub fn wait_for_available<T, F>(
        self,
        deadline: Option<Duration>,
        mut lookup: F,
    ) -> Result<Option<T>, String>
    where
        F: FnMut() -> Result<Option<T>, String>,
    {
        let started = Instant::now();
        for attempt in 0..=self.max_retries {
            if let Some(found) = lookup()? {
                return Ok(Some(found));
            }
            if attempt == self.max_retries {
                break;
            }
            let scale = 1_u32 << u32::from(attempt);
            let delay = self.initial_delay.saturating_mul(scale);
            if let Some(limit) = deadline {
                if started.elapsed().saturating_add(delay) >= limit {
                    return Err(
                        "IPC pre-dispatch retry deadline exceeded; request was not sent"
                            .to_string(),
                    );
                }
            }
            std::thread::sleep(delay);
        }
        Ok(None)
    }
}

/// Boss-owned defaults apply only to established internal administrative calls.
/// Administrative policy belongs to Boss; module calls provide dynamic execution metadata.
/// Legacy commands with side effects retain their prior unbounded behavior until
/// a safe, per-operation execution/cancellation contract exists.
impl Policy {
    pub fn for_internal_request(
        request: &crate::request::ExecutionRequest,
        surface: Surface,
    ) -> Result<Self, String> {
        if surface == Surface::Module {
            return Self::resolve(request.execution.as_ref(), surface);
        }
        if request.execution.is_some() {
            return Err("administrative socket policy belongs to Boss; execution metadata belongs to modules.sock".to_string());
        }
        let budget_ms: Option<u64> = match (request.target.as_str(), request.action.as_deref()) {
            ("boss", Some("version")) => Some(20_000),
            ("boss", Some("update-status")) => Some(60_000),
            ("boss", Some("runtime-list" | "runtime-status" | "module-variables-snapshot")) => {
                Some(20_000)
            }
            ("settings" | "boss.settings", Some("show" | "get" | "has-state")) => Some(20_000),
            // Update/install/uninstall, state mutation and unknown future actions
            // are deliberately not cut off by an invented legacy timeout.
            _ => None,
        };
        Ok(Self {
            asynchronous: false,
            timeout: budget_ms.map(Duration::from_millis),
        })
    }
}

/// Execute within a privileged, short-lived CLI process. It must never create
/// detached workers or job IDs (those belong to the persistent Boss daemon).
/// A local budget is observational: on expiry we report overrun but return the
/// actual completed result, without pretending we safely cancelled an update.
pub fn run_inline<F>(policy: Policy, work: F) -> Result<Value, String>
where
    F: FnOnce() -> Value,
{
    if policy.asynchronous {
        return Err("asynchronous execution requires the persistent Boss IPC daemon".to_string());
    }
    let start = Instant::now();
    let value = work();
    if let Some(timeout) = policy.timeout {
        if start.elapsed() > timeout {
            eprintln!("N.E.E.B.L.E.S.: privileged local IPC budget exceeded; operation completed after {:?}", start.elapsed());
        }
    }
    Ok(value)
}

struct Job {
    owner: String,
    started: Instant,
    deadline: Option<Duration>,
    state: String,
    value: Option<Value>,
    finished: Option<Instant>,
}

#[derive(Default)]
struct JobRegistry {
    jobs: HashMap<String, Job>,
    running: usize,
}

static JOBS: OnceLock<Arc<Mutex<JobRegistry>>> = OnceLock::new();
fn jobs() -> &'static Arc<Mutex<JobRegistry>> {
    JOBS.get_or_init(|| Arc::new(Mutex::new(JobRegistry::default())))
}

pub enum Outcome {
    Complete(Value),
    Accepted(String),
    TimedOut(String),
}

fn trim_expired(jobs: &mut JobRegistry) {
    jobs.jobs
        .retain(|_, job| job.finished.is_none_or(|at| at.elapsed() < JOB_RETENTION));
}

/// The work is dispatched on a bounded number of threads. No persistent
/// background daemon or task survives a Boss restart.
/// A timed-out thread cannot be killed safely; completion is still recorded.
pub fn run<F>(owner: String, policy: Policy, work: F) -> Result<Outcome, String>
where
    F: FnOnce() -> Value + Send + 'static,
{
    // Keep legacy work synchronous when no deadline or async mode was requested.
    if !policy.asynchronous && policy.timeout.is_none() {
        return Ok(Outcome::Complete(work()));
    }
    let id = format!("job-{}", uuid::Uuid::new_v4());
    let mut registry = jobs()
        .lock()
        .map_err(|_| "IPC job registry lock poisoned")?;
    trim_expired(&mut registry);
    let maximum = env_limit(
        "NEEBLES_IPC_MAX_ACTIVE_JOBS",
        DEFAULT_MAX_ACTIVE_JOBS as u64,
        256,
    ) as usize;
    let retention_limit = env_limit(
        "NEEBLES_IPC_MAX_RETAINED_JOBS",
        DEFAULT_MAX_RETAINED_JOBS as u64,
        4096,
    ) as usize;
    let per_owner = env_limit("NEEBLES_IPC_MAX_OWNER_JOBS", 8, 64) as usize;
    if registry.running >= maximum
        || registry.jobs.len() >= retention_limit
        || registry
            .jobs
            .values()
            .filter(|job| job.owner == owner && job.finished.is_none())
            .count()
            >= per_owner
    {
        return Err("IPC execution capacity exhausted; retry later".to_string());
    }
    registry.running += 1;
    registry.jobs.insert(
        id.clone(),
        Job {
            owner,
            started: Instant::now(),
            deadline: policy.timeout,
            state: "running".to_string(),
            value: None,
            finished: None,
        },
    );
    drop(registry);
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker_id = id.clone();
    std::thread::spawn(move || {
        // Do not hold the registry lock while executing arbitrary module work.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
        let value = match outcome {
            Ok(value) => value,
            Err(_) => {
                json!({"ok":false,"code":1,"error":{"kind":"ipc_job_panic","message":"worker failed"}})
            }
        };
        let max_result =
            env_limit("NEEBLES_IPC_MAX_JOB_RESULT_BYTES", 1_048_576, 16_777_216) as usize;
        let stored_value = if serde_json::to_vec(&value)
            .map(|raw| raw.len() > max_result)
            .unwrap_or(true)
        {
            json!({"type":"error","kind":"ipc_result_too_large",
                "message":"job finished but result exceeds retained result limit"})
        } else {
            value.clone()
        };
        if let Ok(mut registry) = jobs().lock() {
            if let Some(job) = registry.jobs.get_mut(&worker_id) {
                job.state = if job
                    .deadline
                    .is_some_and(|deadline| job.started.elapsed() > deadline)
                {
                    "completed_late".to_string()
                } else {
                    "completed".to_string()
                };
                job.value = Some(stored_value);
                job.finished = Some(Instant::now());
            }
            registry.running = registry.running.saturating_sub(1);
        }
        let _ = sender.send(value);
    });
    if policy.asynchronous {
        return Ok(Outcome::Accepted(id));
    }
    match receiver.recv_timeout(
        policy
            .timeout
            .expect("bounded sync request requires timeout"),
    ) {
        Ok(value) => {
            // A successful finite call already returned its result directly.
            // Only async and timed-out calls need a retained job/result.
            if let Ok(mut registry) = jobs().lock() {
                registry.jobs.remove(&id);
            }
            Ok(Outcome::Complete(value))
        }
        Err(RecvTimeoutError::Timeout) => Ok(Outcome::TimedOut(id)),
        Err(RecvTimeoutError::Disconnected) => {
            if let Ok(mut registry) = jobs().lock() {
                registry.jobs.remove(&id);
            }
            Err("IPC execution worker disconnected".to_string())
        }
    }
}

/// Only the authenticated owner of the originating socket/session may read the job.
pub fn status(owner: &str, id: &str) -> Result<Value, String> {
    let mut registry = jobs()
        .lock()
        .map_err(|_| "IPC job registry lock poisoned")?;
    trim_expired(&mut registry);
    let job = registry
        .jobs
        .get_mut(id)
        .ok_or_else(|| "IPC job not found or expired".to_string())?;
    if job.owner != owner {
        return Err("IPC job access denied".to_string());
    }
    let deadline_exceeded = job.state == "running"
        && job
            .deadline
            .is_some_and(|deadline| job.started.elapsed() >= deadline);
    // One-shot collection prevents repeatedly exposing potentially sensitive results.
    let result = job.value.take();
    let consumed = job.finished.is_some() && result.is_none();
    Ok(json!({
        "job_id": id,
        "state": if deadline_exceeded { "deadline_exceeded_running" } else { &job.state },
        "elapsed_ms": job.started.elapsed().as_millis(),
        "result": result,
        "result_consumed": consumed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn internal_request(target: &str, action: &str) -> crate::request::ExecutionRequest {
        crate::request::ExecutionRequest {
            execution: None,
            target: target.to_string(),
            action: Some(action.to_string()),
            args: vec![],
            context: crate::request::ExecutionContext::default(),
        }
    }
    #[test]
    fn internal_admin_defaults_classify_safe_reads_only() {
        let read = internal_request("boss", "version");
        let policy = Policy::for_internal_request(&read, Surface::Administrative).unwrap();
        assert_eq!(policy.timeout, Some(Duration::from_secs(20)));
        assert!(!policy.asynchronous);
        let status = internal_request("boss", "update-status");
        assert_eq!(
            Policy::for_internal_request(&status, Surface::Administrative)
                .unwrap()
                .timeout,
            Some(Duration::from_secs(60))
        );
        let update = internal_request("boss", "update-execute");
        assert_eq!(
            Policy::for_internal_request(&update, Surface::Administrative)
                .unwrap()
                .timeout,
            None
        );
        let unknown = internal_request("new.module", "future-operation");
        assert_eq!(
            Policy::for_internal_request(&unknown, Surface::Administrative)
                .unwrap()
                .timeout,
            None
        );
    }
    #[test]
    fn provider_wait_remains_distinct_from_caller_deadline() {
        let finite = Policy {
            asynchronous: false,
            timeout: Some(Duration::from_millis(100)),
        };
        let provider_wait = finite.module_provider_wait().unwrap();
        assert!(provider_wait > Duration::from_millis(900));
        assert!(provider_wait > finite.timeout.unwrap());
        let asynchronous = Policy {
            asynchronous: true,
            timeout: Some(Duration::from_millis(2500)),
        };
        assert!(asynchronous.module_provider_wait() > asynchronous.timeout);
        let legacy = Policy {
            asynchronous: false,
            timeout: None,
        };
        assert_eq!(legacy.module_provider_wait(), None);
    }

    #[test]
    fn admin_execution_cannot_override_governor() {
        let mut req = internal_request("boss", "version");
        req.execution = Some(ExecutionOptions {
            asynchronous: false,
            timeout_ms: Some(75_000),
            max_retries: None,
            retry_delay_ms: None,
        });
        assert!(Policy::for_internal_request(&req, Surface::Administrative).is_err());
        req.execution = Some(ExecutionOptions {
            asynchronous: true,
            timeout_ms: Some(300_000),
            max_retries: None,
            retry_delay_ms: None,
        });
        assert!(Policy::for_internal_request(&req, Surface::Administrative).is_err());
        // Authenticated module IPC requests still honor caller-supplied execution options.
        let policy = Policy::for_internal_request(&req, Surface::Module).unwrap();
        assert!(policy.asynchronous);
        assert_eq!(policy.timeout, Some(Duration::from_secs(300)));
    }
    #[test]
    fn short_lived_process_must_not_start_background_job() {
        let p = Policy {
            asynchronous: true,
            timeout: Some(Duration::from_secs(30)),
        };
        assert!(run_inline(p, || json!({"ok": true})).is_err());
        let p = Policy {
            asynchronous: false,
            timeout: Some(Duration::from_secs(20)),
        };
        assert_eq!(run_inline(p, || json!({"ok": true})).unwrap()["ok"], true);
    }
    #[test]
    fn legacy_request_remains_sync_and_unbounded() {
        let p = Policy::resolve(None, Surface::Administrative).unwrap();
        assert!(!p.asynchronous);
        assert!(p.timeout.is_none());
    }
    #[test]
    fn options_parse_boolean_and_timeout() {
        let options: ExecutionOptions =
            serde_json::from_value(json!({"async":true,"timeout_ms":12000})).unwrap();
        let p = Policy::resolve(Some(&options), Surface::Module).unwrap();
        assert!(p.asynchronous);
        assert_eq!(p.timeout, Some(Duration::from_secs(12)));
    }
    #[test]
    fn predispatch_retry_defaults_and_explicit_limits() {
        let legacy = RetryPolicy::for_module(None).unwrap();
        assert_eq!(legacy.max_retries, 0);
        let opts: ExecutionOptions =
            serde_json::from_value(json!({"async":false,"timeout_ms":2000})).unwrap();
        assert_eq!(RetryPolicy::for_module(Some(&opts)).unwrap().max_retries, 3);
        let invalid: ExecutionOptions =
            serde_json::from_value(json!({"async":false,"timeout_ms":2000,"max_retries":11}))
                .unwrap();
        assert!(RetryPolicy::for_module(Some(&invalid)).is_err());
    }

    #[test]
    fn predispatch_retries_do_not_duplicate_dispatch() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay: Duration::from_millis(1),
        };
        let mut checks = 0;
        let found = policy
            .wait_for_available(Some(Duration::from_secs(2)), || {
                checks += 1;
                Ok(if checks == 4 { Some(42) } else { None })
            })
            .unwrap();
        assert_eq!(found, Some(42));
        assert_eq!(checks, 4);
        let mut never = 0;
        let missing: Option<u32> = policy
            .wait_for_available(None, || {
                never += 1;
                Ok(None)
            })
            .unwrap();
        assert_eq!(missing, None);
        assert_eq!(never, 4);
    }

    #[test]
    fn predispatch_retries_respect_total_deadline() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
        };
        let mut calls = 0;
        let result = policy.wait_for_available::<u8, _>(Some(Duration::from_millis(10)), || {
            calls += 1;
            Ok(None)
        });
        assert!(result.is_err());
        assert_eq!(calls, 1);
    }

    #[test]
    fn invalid_timeout_rejected() {
        let options = ExecutionOptions {
            asynchronous: false,
            timeout_ms: Some(0),
            max_retries: None,
            retry_delay_ms: None,
        };
        assert!(Policy::resolve(Some(&options), Surface::Module).is_err());
    }
    #[test]
    fn async_job_restricts_status_to_owner() {
        let policy = Policy {
            asynchronous: true,
            timeout: Some(Duration::from_secs(2)),
        };
        let id = match run("module:a:session-1".into(), policy, || json!({"ok":true})).unwrap() {
            Outcome::Accepted(id) => id,
            _ => panic!("expected job ID"),
        };
        assert!(status("module:b:session-1", &id).is_err());
        assert!(status("module:a:session-1", &id).is_ok());
    }
    #[test]
    fn finite_completed_request_does_not_retain_job() {
        let owner = format!("finite-cleanup-{}", uuid::Uuid::new_v4());
        let policy = Policy {
            asynchronous: false,
            timeout: Some(Duration::from_secs(2)),
        };
        for _ in 0..16 {
            let outcome = run(owner.clone(), policy, || json!({"ok":true})).unwrap();
            assert!(matches!(outcome, Outcome::Complete(_)));
        }
        let registry = jobs().lock().unwrap();
        assert_eq!(
            registry
                .jobs
                .values()
                .filter(|job| job.owner == owner)
                .count(),
            0
        );
    }
    #[test]
    fn timeout_does_not_claim_to_cancel_work() {
        let policy = Policy {
            asynchronous: false,
            timeout: Some(Duration::from_millis(1)),
        };
        let outcome = run("admin".into(), policy, || {
            std::thread::sleep(Duration::from_millis(15));
            json!({"ok":true})
        })
        .unwrap();
        let id = match outcome {
            Outcome::TimedOut(id) => id,
            _ => panic!("expected timeout"),
        };
        let state = status("admin", &id).unwrap();
        assert_eq!(state["state"], "deadline_exceeded_running");
    }
}

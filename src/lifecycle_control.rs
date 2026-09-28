use std::future::Future;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use async_io::Timer;

use crate::lifecycle_execution::ExecutionPayload;

/*
 * Lifecycle generic execution control.
 *
 * Technical responsibility:
 * Provide reusable internal machinery for cancellation, deadlines
 * and repeated execution attempts.
 *
 * This layer is not a module-facing tactics vocabulary.
 * It does not inspect Lifecycle tactics, artillery, objective,
 * munition, intelligence or module semantics.
 */

pub type ControlledFuture =
    Pin<Box<dyn Future<Output = Result<ExecutionPayload, String>> + Send + 'static>>;

pub type ControlledHandler = Arc<dyn Fn() -> ControlledFuture + Send + Sync + 'static>;

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionControl {
    pub max_attempts: usize,
    pub deadline: Option<Duration>,
}

impl Default for ExecutionControl {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            deadline: None,
        }
    }
}

impl ExecutionControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attempts(mut self, max_attempts: usize) -> Result<Self, String> {
        if max_attempts == 0 {
            return Err("execution attempts cannot be zero".to_string());
        }

        self.max_attempts = max_attempts;

        Ok(self)
    }

    pub fn deadline(mut self, duration: Duration) -> Result<Self, String> {
        if duration.is_zero() {
            return Err("execution deadline cannot be zero".to_string());
        }

        self.deadline = Some(duration);

        Ok(self)
    }
}

async fn execute_once(
    handler: ControlledHandler,
    token: CancellationToken,
    deadline: Option<Duration>,
) -> Result<ExecutionPayload, String> {
    if token.is_cancelled() {
        return Err("execution cancelled".to_string());
    }

    let execution = handler();

    match deadline {
        None => {
            let result = execution.await;

            if token.is_cancelled() {
                return Err("execution cancelled".to_string());
            }

            result
        }

        Some(duration) => {
            futures_lite::pin!(execution);

            let timer = Timer::after(duration);

            futures_lite::pin!(timer);

            match futures_lite::future::race(async { (true, execution.await) }, async {
                timer.await;

                (false, Err("execution deadline exceeded".to_string()))
            })
            .await
            {
                (true, result) => {
                    if token.is_cancelled() {
                        Err("execution cancelled".to_string())
                    } else {
                        result
                    }
                }

                (false, result) => result,
            }
        }
    }
}

pub async fn execute_controlled(
    handler: ControlledHandler,
    control: ExecutionControl,
    token: CancellationToken,
) -> Result<ExecutionPayload, String> {
    if control.max_attempts == 0 {
        return Err("execution attempts cannot be zero".to_string());
    }

    let mut last_error = None;

    for _ in 0..control.max_attempts {
        if token.is_cancelled() {
            return Err("execution cancelled".to_string());
        }

        match execute_once(Arc::clone(&handler), token.clone(), control.deadline).await {
            Ok(output) => {
                return Ok(output);
            }

            Err(error) => {
                if token.is_cancelled() {
                    return Err("execution cancelled".to_string());
                }

                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "execution failed without an error".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};

    fn success(value: &str) -> ControlledHandler {
        let value = value.to_string();

        Arc::new(move || {
            let value = value.clone();

            Box::pin(async move {
                let mut output = ExecutionPayload::new();

                output.insert("value".to_string(), value);

                Ok(output)
            })
        })
    }

    #[test]
    fn cancellation_token_starts_active() {
        let token = CancellationToken::new();

        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancellation_token_can_be_cancelled() {
        let token = CancellationToken::new();

        token.cancel();

        assert!(token.is_cancelled());
    }

    #[test]
    fn cancellation_token_clones_share_state() {
        let first = CancellationToken::new();

        let second = first.clone();

        second.cancel();

        assert!(first.is_cancelled());
    }

    #[test]
    fn control_defaults_to_one_attempt_without_deadline() {
        let control = ExecutionControl::new();

        assert_eq!(control.max_attempts, 1);

        assert_eq!(control.deadline, None);
    }

    #[test]
    fn zero_attempts_are_rejected() {
        let result = ExecutionControl::new().attempts(0);

        assert!(result.is_err());
    }

    #[test]
    fn zero_deadline_is_rejected() {
        let result = ExecutionControl::new().deadline(Duration::ZERO);

        assert!(result.is_err());
    }

    #[test]
    fn successful_execution_returns_payload() {
        let output = futures_lite::future::block_on(execute_controlled(
            success("ok"),
            ExecutionControl::new(),
            CancellationToken::new(),
        ))
        .unwrap();

        assert_eq!(output.get("value"), Some(&"ok".to_string()));
    }

    #[test]
    fn already_cancelled_execution_never_runs() {
        let executions = Arc::new(AtomicUsize::new(0));

        let executions_for_handler = Arc::clone(&executions);

        let handler: ControlledHandler = Arc::new(move || {
            let executions = Arc::clone(&executions_for_handler);

            Box::pin(async move {
                executions.fetch_add(1, Ordering::SeqCst);

                Ok(ExecutionPayload::new())
            })
        });

        let token = CancellationToken::new();

        token.cancel();

        let error = futures_lite::future::block_on(execute_controlled(
            handler,
            ExecutionControl::new(),
            token,
        ))
        .unwrap_err();

        assert_eq!(error, "execution cancelled");

        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn failed_execution_can_be_repeated_until_success() {
        let attempts = Arc::new(AtomicUsize::new(0));

        let attempts_for_handler = Arc::clone(&attempts);

        let handler: ControlledHandler = Arc::new(move || {
            let attempts = Arc::clone(&attempts_for_handler);

            Box::pin(async move {
                let current = attempts.fetch_add(1, Ordering::SeqCst);

                if current < 2 {
                    return Err("synthetic failure".to_string());
                }

                let mut output = ExecutionPayload::new();

                output.insert("status".to_string(), "recovered".to_string());

                Ok(output)
            })
        });

        let control = ExecutionControl::new().attempts(3).unwrap();

        let output = futures_lite::future::block_on(execute_controlled(
            handler,
            control,
            CancellationToken::new(),
        ))
        .unwrap();

        assert_eq!(attempts.load(Ordering::SeqCst), 3);

        assert_eq!(output.get("status"), Some(&"recovered".to_string()));
    }

    #[test]
    fn failed_execution_stops_after_attempt_limit() {
        let attempts = Arc::new(AtomicUsize::new(0));

        let attempts_for_handler = Arc::clone(&attempts);

        let handler: ControlledHandler = Arc::new(move || {
            let attempts = Arc::clone(&attempts_for_handler);

            Box::pin(async move {
                attempts.fetch_add(1, Ordering::SeqCst);

                Err("still failing".to_string())
            })
        });

        let control = ExecutionControl::new().attempts(2).unwrap();

        let error = futures_lite::future::block_on(execute_controlled(
            handler,
            control,
            CancellationToken::new(),
        ))
        .unwrap_err();

        assert_eq!(attempts.load(Ordering::SeqCst), 2);

        assert_eq!(error, "still failing");
    }

    #[test]
    fn deadline_interrupts_waiting_future() {
        let handler: ControlledHandler = Arc::new(|| {
            Box::pin(async {
                Timer::after(Duration::from_millis(100)).await;

                Ok(ExecutionPayload::new())
            })
        });

        let control = ExecutionControl::new()
            .deadline(Duration::from_millis(5))
            .unwrap();

        let error = futures_lite::future::block_on(execute_controlled(
            handler,
            control,
            CancellationToken::new(),
        ))
        .unwrap_err();

        assert_eq!(error, "execution deadline exceeded");
    }

    #[test]
    fn no_lifecycle_tactics_map_is_required() {
        let control = ExecutionControl::new()
            .attempts(2)
            .unwrap()
            .deadline(Duration::from_secs(1))
            .unwrap();

        assert_eq!(control.max_attempts, 2);

        assert_eq!(control.deadline, Some(Duration::from_secs(1)));
    }
}

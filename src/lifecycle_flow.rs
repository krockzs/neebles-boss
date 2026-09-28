use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_io::Timer;

use crate::lifecycle_control::CancellationToken;
use crate::lifecycle_execution::ExecutionPayload;

/*
 * Lifecycle generic flow combinators.
 *
 * Technical responsibility:
 * Provide reusable asynchronous composition primitives.
 *
 * This layer does not inspect Lifecycle tactics and does not define
 * module-facing vocabulary. It only exposes generic Rust machinery
 * that a later interpreter may compose from declarative information.
 */

pub type FlowFuture =
    Pin<Box<dyn Future<Output = Result<ExecutionPayload, String>> + Send + 'static>>;

pub type FlowHandler = Arc<dyn Fn() -> FlowFuture + Send + Sync + 'static>;

pub type Condition = Arc<dyn Fn() -> bool + Send + Sync + 'static>;

pub type ErrorHandler = Arc<dyn Fn(String) -> FlowFuture + Send + Sync + 'static>;

async fn cancellation_wait(token: CancellationToken) {
    loop {
        if token.is_cancelled() {
            return;
        }

        Timer::after(Duration::from_millis(1)).await;
    }
}

pub async fn wait(duration: Duration, token: CancellationToken) -> Result<(), String> {
    if duration.is_zero() {
        return Err("wait duration cannot be zero".to_string());
    }

    if token.is_cancelled() {
        return Err("execution cancelled".to_string());
    }

    let timer = Timer::after(duration);

    futures_lite::pin!(timer);

    let cancelled = cancellation_wait(token.clone());

    futures_lite::pin!(cancelled);

    let completed_by_timer = futures_lite::future::race(
        async {
            timer.await;
            true
        },
        async {
            cancelled.await;
            false
        },
    )
    .await;

    if completed_by_timer {
        if token.is_cancelled() {
            Err("execution cancelled".to_string())
        } else {
            Ok(())
        }
    } else {
        Err("execution cancelled".to_string())
    }
}

pub async fn execute_if(
    condition: Condition,
    handler: FlowHandler,
) -> Result<Option<ExecutionPayload>, String> {
    if !condition() {
        return Ok(None);
    }

    handler().await.map(Some)
}

pub async fn fallback(
    primary: FlowHandler,
    secondary: FlowHandler,
) -> Result<ExecutionPayload, String> {
    match primary().await {
        Ok(output) => Ok(output),
        Err(_) => secondary().await,
    }
}

pub async fn on_error(
    primary: FlowHandler,
    error_handler: ErrorHandler,
) -> Result<ExecutionPayload, String> {
    match primary().await {
        Ok(output) => Ok(output),
        Err(error) => error_handler(error).await,
    }
}

pub async fn finally(
    primary: FlowHandler,
    finalizer: FlowHandler,
) -> Result<ExecutionPayload, String> {
    let primary_result = primary().await;

    let finalizer_result = finalizer().await;

    match (primary_result, finalizer_result) {
        (Ok(output), Ok(_)) => Ok(output),

        (Ok(_), Err(finalizer_error)) => Err(finalizer_error),

        (Err(primary_error), Ok(_)) => Err(primary_error),

        (Err(primary_error), Err(finalizer_error)) => Err(format!(
            "primary failure: {primary_error}; finalizer failure: {finalizer_error}"
        )),
    }
}

pub async fn race(first: FlowHandler, second: FlowHandler) -> Result<ExecutionPayload, String> {
    let first_future = first();

    let second_future = second();

    futures_lite::pin!(first_future);

    futures_lite::pin!(second_future);

    futures_lite::future::race(first_future, second_future).await
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn success(key: &str, value: &str) -> FlowHandler {
        let key = key.to_string();

        let value = value.to_string();

        Arc::new(move || {
            let key = key.clone();

            let value = value.clone();

            Box::pin(async move {
                let mut output = ExecutionPayload::new();

                output.insert(key, value);

                Ok(output)
            })
        })
    }

    fn failure(message: &str) -> FlowHandler {
        let message = message.to_string();

        Arc::new(move || {
            let message = message.clone();

            Box::pin(async move { Err(message) })
        })
    }

    #[test]
    fn wait_rejects_zero_duration() {
        let result = futures_lite::future::block_on(wait(Duration::ZERO, CancellationToken::new()));

        assert!(result.is_err());
    }

    #[test]
    fn wait_completes_normally() {
        futures_lite::future::block_on(wait(Duration::from_millis(2), CancellationToken::new()))
            .unwrap();
    }

    #[test]
    fn wait_observes_preexisting_cancellation() {
        let token = CancellationToken::new();

        token.cancel();

        let error =
            futures_lite::future::block_on(wait(Duration::from_secs(1), token)).unwrap_err();

        assert_eq!(error, "execution cancelled");
    }

    #[test]
    fn false_condition_skips_handler() {
        let executed = Arc::new(AtomicBool::new(false));

        let executed_handler = Arc::clone(&executed);

        let handler: FlowHandler = Arc::new(move || {
            let executed = Arc::clone(&executed_handler);

            Box::pin(async move {
                executed.store(true, Ordering::SeqCst);

                Ok(ExecutionPayload::new())
            })
        });

        let result =
            futures_lite::future::block_on(execute_if(Arc::new(|| false), handler)).unwrap();

        assert_eq!(result, None);

        assert!(!executed.load(Ordering::SeqCst));
    }

    #[test]
    fn true_condition_executes_handler() {
        let result = futures_lite::future::block_on(execute_if(
            Arc::new(|| true),
            success("status", "executed"),
        ))
        .unwrap()
        .unwrap();

        assert_eq!(result.get("status"), Some(&"executed".to_string()));
    }

    #[test]
    fn fallback_keeps_primary_success() {
        let secondary_runs = Arc::new(AtomicUsize::new(0));

        let secondary_counter = Arc::clone(&secondary_runs);

        let secondary: FlowHandler = Arc::new(move || {
            let counter = Arc::clone(&secondary_counter);

            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);

                Ok(ExecutionPayload::new())
            })
        });

        let output =
            futures_lite::future::block_on(fallback(success("source", "primary"), secondary))
                .unwrap();

        assert_eq!(output.get("source"), Some(&"primary".to_string()));

        assert_eq!(secondary_runs.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn fallback_executes_secondary_after_failure() {
        let output = futures_lite::future::block_on(fallback(
            failure("primary failed"),
            success("source", "secondary"),
        ))
        .unwrap();

        assert_eq!(output.get("source"), Some(&"secondary".to_string()));
    }

    #[test]
    fn on_error_receives_original_error() {
        let handler: ErrorHandler = Arc::new(|error| {
            Box::pin(async move {
                let mut output = ExecutionPayload::new();

                output.insert("error".to_string(), error);

                Ok(output)
            })
        });

        let output =
            futures_lite::future::block_on(on_error(failure("original failure"), handler)).unwrap();

        assert_eq!(output.get("error"), Some(&"original failure".to_string()));
    }

    #[test]
    fn finally_runs_after_success() {
        let finalizer_ran = Arc::new(AtomicBool::new(false));

        let flag = Arc::clone(&finalizer_ran);

        let finalizer: FlowHandler = Arc::new(move || {
            let flag = Arc::clone(&flag);

            Box::pin(async move {
                flag.store(true, Ordering::SeqCst);

                Ok(ExecutionPayload::new())
            })
        });

        let output =
            futures_lite::future::block_on(finally(success("status", "ok"), finalizer)).unwrap();

        assert_eq!(output.get("status"), Some(&"ok".to_string()));

        assert!(finalizer_ran.load(Ordering::SeqCst));
    }

    #[test]
    fn finally_runs_after_failure() {
        let finalizer_ran = Arc::new(AtomicBool::new(false));

        let flag = Arc::clone(&finalizer_ran);

        let finalizer: FlowHandler = Arc::new(move || {
            let flag = Arc::clone(&flag);

            Box::pin(async move {
                flag.store(true, Ordering::SeqCst);

                Ok(ExecutionPayload::new())
            })
        });

        let error = futures_lite::future::block_on(finally(failure("primary failed"), finalizer))
            .unwrap_err();

        assert_eq!(error, "primary failed");

        assert!(finalizer_ran.load(Ordering::SeqCst));
    }

    #[test]
    fn finally_reports_both_failures() {
        let error = futures_lite::future::block_on(finally(
            failure("primary failed"),
            failure("finalizer failed"),
        ))
        .unwrap_err();

        assert!(error.contains("primary failed"));

        assert!(error.contains("finalizer failed"));
    }

    #[test]
    fn race_returns_first_completion() {
        let slow: FlowHandler = Arc::new(|| {
            Box::pin(async {
                Timer::after(Duration::from_millis(25)).await;

                let mut output = ExecutionPayload::new();

                output.insert("winner".to_string(), "slow".to_string());

                Ok(output)
            })
        });

        let fast: FlowHandler = Arc::new(|| {
            Box::pin(async {
                Timer::after(Duration::from_millis(1)).await;

                let mut output = ExecutionPayload::new();

                output.insert("winner".to_string(), "fast".to_string());

                Ok(output)
            })
        });

        let output = futures_lite::future::block_on(race(slow, fast)).unwrap();

        assert_eq!(output.get("winner"), Some(&"fast".to_string()));
    }

    #[test]
    fn race_preserves_first_failure_semantics() {
        let fast_failure: FlowHandler =
            Arc::new(|| Box::pin(async { Err("fast failure".to_string()) }));

        let slow_success: FlowHandler = Arc::new(|| {
            Box::pin(async {
                Timer::after(Duration::from_millis(20)).await;

                Ok(ExecutionPayload::new())
            })
        });

        let error = futures_lite::future::block_on(race(fast_failure, slow_success)).unwrap_err();

        assert_eq!(error, "fast failure");
    }

    #[test]
    fn flow_combinators_do_not_require_lifecycle_tactics() {
        let condition: Condition = Arc::new(|| true);

        assert!(condition());
    }
}

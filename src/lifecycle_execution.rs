use futures_util::future::join_all;

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

/*
 * Lifecycle multi-execution core.
 *
 * Technical responsibility:
 * Execute multiple independent asynchronous jobs and collect their
 * outcomes without knowing module semantics, artillery semantics,
 * objective semantics or tactics vocabulary.
 *
 * This is infrastructure only.
 *
 * Ordering, dependencies, retry policies, timeouts, races, fallback
 * and other orchestration decisions are deliberately not interpreted
 * here. A later layer may compose execution batches according to
 * dynamically supplied technical information.
 */

pub type ExecutionPayload = BTreeMap<String, String>;

type ExecutionFuture =
    Pin<Box<dyn Future<Output = Result<ExecutionPayload, String>> + Send + 'static>>;

pub struct ExecutionJob {
    id: String,
    future: ExecutionFuture,
}

impl ExecutionJob {
    pub fn new<F>(id: impl Into<String>, future: F) -> Result<Self, String>
    where
        F: Future<Output = Result<ExecutionPayload, String>> + Send + 'static,
    {
        let id = id.into();

        if id.trim().is_empty() {
            return Err("execution job id cannot be empty".to_string());
        }

        Ok(Self {
            id,
            future: Box::pin(future),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionOutcome {
    Success(ExecutionPayload),
    Failure(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionReport {
    outcomes: BTreeMap<String, ExecutionOutcome>,
}

impl ExecutionReport {
    pub fn get(&self, id: &str) -> Option<&ExecutionOutcome> {
        self.outcomes.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.outcomes.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }

    pub fn succeeded(&self, id: &str) -> bool {
        matches!(self.outcomes.get(id), Some(ExecutionOutcome::Success(_)))
    }

    pub fn failed(&self, id: &str) -> bool {
        matches!(self.outcomes.get(id), Some(ExecutionOutcome::Failure(_)))
    }
}

fn validate_unique_jobs(jobs: &[ExecutionJob]) -> Result<(), String> {
    let mut seen = BTreeMap::<String, ()>::new();

    for job in jobs {
        if seen.insert(job.id.clone(), ()).is_some() {
            return Err(format!("duplicate execution job id '{}'", job.id));
        }
    }

    Ok(())
}

/*
 * Execute one technical batch concurrently.
 *
 * The batch itself has no Lifecycle semantic meaning.
 * It is only a generic Rust multi-execution primitive.
 *
 * One failed job does not erase successful sibling outcomes.
 */
pub async fn execute_batch(jobs: Vec<ExecutionJob>) -> Result<ExecutionReport, String> {
    validate_unique_jobs(&jobs)?;

    let futures = jobs.into_iter().map(|job| async move {
        let id = job.id;

        let outcome = match job.future.await {
            Ok(payload) => ExecutionOutcome::Success(payload),
            Err(error) => ExecutionOutcome::Failure(error),
        };

        (id, outcome)
    });

    let completed = join_all(futures).await;

    let outcomes = completed.into_iter().collect();

    Ok(ExecutionReport { outcomes })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(key: &str, value: &str) -> ExecutionPayload {
        let mut payload = ExecutionPayload::new();

        payload.insert(key.to_string(), value.to_string());

        payload
    }

    #[test]
    fn execution_job_accepts_arbitrary_identifier() {
        let job = ExecutionJob::new("future.operation.whatever", async {
            Ok(ExecutionPayload::new())
        })
        .unwrap();

        assert_eq!(job.id(), "future.operation.whatever");
    }

    #[test]
    fn execution_job_rejects_empty_identifier() {
        let result = ExecutionJob::new("   ", async { Ok(ExecutionPayload::new()) });

        assert!(result.is_err());
    }

    #[test]
    fn empty_batch_is_valid() {
        let report = futures_lite::future::block_on(execute_batch(Vec::new())).unwrap();

        assert!(report.is_empty());
    }

    #[test]
    fn batch_executes_multiple_jobs() {
        let jobs = vec![
            ExecutionJob::new("alpha", async { Ok(payload("value", "one")) }).unwrap(),
            ExecutionJob::new("beta", async { Ok(payload("value", "two")) }).unwrap(),
            ExecutionJob::new("gamma", async { Ok(payload("value", "three")) }).unwrap(),
        ];

        let report = futures_lite::future::block_on(execute_batch(jobs)).unwrap();

        assert_eq!(report.len(), 3);
        assert!(report.succeeded("alpha"));
        assert!(report.succeeded("beta"));
        assert!(report.succeeded("gamma"));
    }

    #[test]
    fn batch_preserves_arbitrary_output_maps() {
        let jobs = vec![ExecutionJob::new("alpha", async {
            let mut output = ExecutionPayload::new();

            output.insert("anything".to_string(), "one".to_string());

            output.insert("future.output.path".to_string(), "two".to_string());

            Ok(output)
        })
        .unwrap()];

        let report = futures_lite::future::block_on(execute_batch(jobs)).unwrap();

        let Some(ExecutionOutcome::Success(output)) = report.get("alpha") else {
            panic!("alpha must succeed");
        };

        assert_eq!(output.get("anything"), Some(&"one".to_string()));

        assert_eq!(output.get("future.output.path"), Some(&"two".to_string()));
    }

    #[test]
    fn one_failure_does_not_destroy_sibling_success() {
        let jobs = vec![
            ExecutionJob::new("success", async { Ok(payload("value", "alive")) }).unwrap(),
            ExecutionJob::new("failure", async { Err("synthetic failure".to_string()) }).unwrap(),
        ];

        let report = futures_lite::future::block_on(execute_batch(jobs)).unwrap();

        assert!(report.succeeded("success"));
        assert!(report.failed("failure"));

        assert_eq!(
            report.get("failure"),
            Some(&ExecutionOutcome::Failure("synthetic failure".to_string()))
        );
    }

    #[test]
    fn duplicate_job_ids_are_rejected_before_execution() {
        let jobs = vec![
            ExecutionJob::new("same", async { Ok(ExecutionPayload::new()) }).unwrap(),
            ExecutionJob::new("same", async { Ok(ExecutionPayload::new()) }).unwrap(),
        ];

        let error = futures_lite::future::block_on(execute_batch(jobs)).unwrap_err();

        assert!(error.contains("duplicate execution job id"));
    }

    #[test]
    fn batch_does_not_prescribe_job_vocabulary() {
        let jobs = vec![ExecutionJob::new("module.decides.this.name", async {
            Ok(payload("module.decides.this.output", "opaque-value"))
        })
        .unwrap()];

        let report = futures_lite::future::block_on(execute_batch(jobs)).unwrap();

        assert!(report.succeeded("module.decides.this.name"));
    }

    #[test]
    fn asynchronous_jobs_can_yield_independently() {
        let jobs = vec![
            ExecutionJob::new("first", async {
                futures_lite::future::yield_now().await;

                Ok(payload("done", "first"))
            })
            .unwrap(),
            ExecutionJob::new("second", async {
                futures_lite::future::yield_now().await;
                futures_lite::future::yield_now().await;

                Ok(payload("done", "second"))
            })
            .unwrap(),
        ];

        let report = futures_lite::future::block_on(execute_batch(jobs)).unwrap();

        assert!(report.succeeded("first"));
        assert!(report.succeeded("second"));
    }
}

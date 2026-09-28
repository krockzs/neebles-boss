use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::lifecycle_execution::{self, ExecutionJob, ExecutionOutcome, ExecutionPayload};

/*
 * Lifecycle generic orchestration engine.
 *
 * Technical responsibility:
 * Execute arbitrary asynchronous tasks according to dependency
 * relationships.
 *
 * This layer does not expose a fixed vocabulary such as sequence,
 * parallel, join or any module-facing tactics language.
 *
 * Those execution shapes emerge naturally from the dependency graph.
 *
 * No module semantics, artillery semantics, objective semantics or
 * concrete capability names live here.
 */

pub type OrchestrationFuture =
    Pin<Box<dyn Future<Output = Result<ExecutionPayload, String>> + Send + 'static>>;

pub type OrchestrationHandler = Arc<dyn Fn() -> OrchestrationFuture + Send + Sync + 'static>;

pub struct OrchestrationTask {
    id: String,
    dependencies: BTreeSet<String>,
    handler: OrchestrationHandler,
}

impl OrchestrationTask {
    pub fn new(
        id: impl Into<String>,
        dependencies: impl IntoIterator<Item = String>,
        handler: OrchestrationHandler,
    ) -> Result<Self, String> {
        let id = id.into();

        if id.trim().is_empty() {
            return Err("orchestration task id cannot be empty".to_string());
        }

        let dependencies: BTreeSet<String> = dependencies.into_iter().collect();

        if dependencies.contains(&id) {
            return Err(format!("orchestration task '{id}' cannot depend on itself"));
        }

        if dependencies
            .iter()
            .any(|dependency| dependency.trim().is_empty())
        {
            return Err("orchestration dependency id cannot be empty".to_string());
        }

        Ok(Self {
            id,
            dependencies,
            handler,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn dependencies(&self) -> &BTreeSet<String> {
        &self.dependencies
    }
}

#[derive(Default)]
pub struct OrchestrationPlan {
    tasks: BTreeMap<String, OrchestrationTask>,
}

impl OrchestrationPlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, task: OrchestrationTask) -> Result<(), String> {
        if self.tasks.contains_key(task.id()) {
            return Err(format!(
                "orchestration task '{}' is already registered",
                task.id()
            ));
        }

        self.tasks.insert(task.id.clone(), task);

        Ok(())
    }

    pub fn contains(&self, task_id: &str) -> bool {
        self.tasks.contains_key(task_id)
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    fn validate_dependencies(&self) -> Result<(), String> {
        for task in self.tasks.values() {
            for dependency in task.dependencies() {
                if !self.tasks.contains_key(dependency) {
                    return Err(format!(
                        "orchestration task '{}' depends on unknown task '{}'",
                        task.id(),
                        dependency
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_acyclic(&self) -> Result<(), String> {
        let mut completed = BTreeSet::<String>::new();

        loop {
            let ready: Vec<String> = self
                .tasks
                .iter()
                .filter(|(id, task)| {
                    !completed.contains(*id)
                        && task
                            .dependencies()
                            .iter()
                            .all(|dependency| completed.contains(dependency))
                })
                .map(|(id, _)| id.clone())
                .collect();

            if ready.is_empty() {
                break;
            }

            for id in ready {
                completed.insert(id);
            }
        }

        if completed.len() != self.tasks.len() {
            return Err("orchestration dependency graph contains a cycle".to_string());
        }

        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_dependencies()?;
        self.validate_acyclic()?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OrchestrationReport {
    outcomes: BTreeMap<String, ExecutionOutcome>,
}

impl OrchestrationReport {
    pub fn get(&self, task_id: &str) -> Option<&ExecutionOutcome> {
        self.outcomes.get(task_id)
    }

    pub fn succeeded(&self, task_id: &str) -> bool {
        matches!(
            self.outcomes.get(task_id),
            Some(ExecutionOutcome::Success(_))
        )
    }

    pub fn failed(&self, task_id: &str) -> bool {
        matches!(
            self.outcomes.get(task_id),
            Some(ExecutionOutcome::Failure(_))
        )
    }

    pub fn len(&self) -> usize {
        self.outcomes.len()
    }
}

pub async fn execute(plan: OrchestrationPlan) -> Result<OrchestrationReport, String> {
    plan.validate()?;

    let mut pending: BTreeSet<String> = plan.tasks.keys().cloned().collect();

    let mut outcomes = BTreeMap::<String, ExecutionOutcome>::new();

    while !pending.is_empty() {
        let blocked: Vec<String> = pending
            .iter()
            .filter(|id| {
                let task = &plan.tasks[*id];

                task.dependencies().iter().any(|dependency| {
                    matches!(outcomes.get(dependency), Some(ExecutionOutcome::Failure(_)))
                })
            })
            .cloned()
            .collect();

        for id in blocked {
            pending.remove(&id);

            outcomes.insert(
                id,
                ExecutionOutcome::Failure("dependency failed".to_string()),
            );
        }

        if pending.is_empty() {
            break;
        }

        let ready: Vec<String> = pending
            .iter()
            .filter(|id| {
                let task = &plan.tasks[*id];

                task.dependencies().iter().all(|dependency| {
                    matches!(outcomes.get(dependency), Some(ExecutionOutcome::Success(_)))
                })
            })
            .cloned()
            .collect();

        if ready.is_empty() {
            return Err("orchestration reached an impossible execution state".to_string());
        }

        let mut jobs = Vec::<ExecutionJob>::new();

        for id in &ready {
            let task = &plan.tasks[id];
            let future = (task.handler)();

            jobs.push(ExecutionJob::new(id.clone(), future)?);
        }

        let report = lifecycle_execution::execute_batch(jobs).await?;

        for id in ready {
            let outcome = report
                .get(&id)
                .cloned()
                .ok_or_else(|| format!("execution report lost orchestration task '{id}'"))?;

            pending.remove(&id);
            outcomes.insert(id, outcome);
        }
    }

    Ok(OrchestrationReport { outcomes })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    fn success(value: &str) -> OrchestrationHandler {
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

    fn failure(message: &str) -> OrchestrationHandler {
        let message = message.to_string();

        Arc::new(move || {
            let message = message.clone();

            Box::pin(async move { Err(message) })
        })
    }

    #[test]
    fn empty_plan_is_valid() {
        let plan = OrchestrationPlan::new();

        assert!(plan.validate().is_ok());

        let report = futures_lite::future::block_on(execute(plan)).unwrap();

        assert_eq!(report.len(), 0);
    }

    #[test]
    fn plan_accepts_arbitrary_task_ids() {
        let mut plan = OrchestrationPlan::new();

        plan.register(
            OrchestrationTask::new(
                "future.module.operation",
                Vec::<String>::new(),
                success("ok"),
            )
            .unwrap(),
        )
        .unwrap();

        assert!(plan.contains("future.module.operation"));
    }

    #[test]
    fn duplicate_task_is_rejected() {
        let mut plan = OrchestrationPlan::new();

        plan.register(
            OrchestrationTask::new("alpha", Vec::<String>::new(), success("one")).unwrap(),
        )
        .unwrap();

        let error = plan
            .register(
                OrchestrationTask::new("alpha", Vec::<String>::new(), success("two")).unwrap(),
            )
            .unwrap_err();

        assert!(error.contains("already registered"));
    }

    #[test]
    fn unknown_dependency_is_rejected() {
        let mut plan = OrchestrationPlan::new();

        plan.register(
            OrchestrationTask::new("alpha", vec!["missing".to_string()], success("one")).unwrap(),
        )
        .unwrap();

        let error = plan.validate().unwrap_err();

        assert!(error.contains("unknown task"));
    }

    #[test]
    fn self_dependency_is_rejected() {
        let result = OrchestrationTask::new("alpha", vec!["alpha".to_string()], success("one"));

        match result {
            Ok(_) => panic!("self dependency should have been rejected"),
            Err(error) => {
                assert!(error.contains("cannot depend on itself"));
            }
        }
    }

    #[test]
    fn cyclic_dependency_is_rejected() {
        let mut plan = OrchestrationPlan::new();

        plan.register(
            OrchestrationTask::new("alpha", vec!["beta".to_string()], success("one")).unwrap(),
        )
        .unwrap();

        plan.register(
            OrchestrationTask::new("beta", vec!["alpha".to_string()], success("two")).unwrap(),
        )
        .unwrap();

        let error = plan.validate().unwrap_err();

        assert!(error.contains("contains a cycle"));
    }

    #[test]
    fn independent_tasks_execute_successfully() {
        let mut plan = OrchestrationPlan::new();

        plan.register(
            OrchestrationTask::new("alpha", Vec::<String>::new(), success("one")).unwrap(),
        )
        .unwrap();

        plan.register(
            OrchestrationTask::new("beta", Vec::<String>::new(), success("two")).unwrap(),
        )
        .unwrap();

        let report = futures_lite::future::block_on(execute(plan)).unwrap();

        assert!(report.succeeded("alpha"));

        assert!(report.succeeded("beta"));
    }

    #[test]
    fn dependency_chain_executes_in_dependency_order() {
        let log = Arc::new(Mutex::new(Vec::<String>::new()));

        let first_log = Arc::clone(&log);

        let second_log = Arc::clone(&log);

        let first: OrchestrationHandler = Arc::new(move || {
            let log = Arc::clone(&first_log);

            Box::pin(async move {
                log.lock().unwrap().push("first".to_string());

                Ok(ExecutionPayload::new())
            })
        });

        let second: OrchestrationHandler = Arc::new(move || {
            let log = Arc::clone(&second_log);

            Box::pin(async move {
                log.lock().unwrap().push("second".to_string());

                Ok(ExecutionPayload::new())
            })
        });

        let mut plan = OrchestrationPlan::new();

        plan.register(OrchestrationTask::new("first", Vec::<String>::new(), first).unwrap())
            .unwrap();

        plan.register(OrchestrationTask::new("second", vec!["first".to_string()], second).unwrap())
            .unwrap();

        let report = futures_lite::future::block_on(execute(plan)).unwrap();

        assert!(report.succeeded("first"));

        assert!(report.succeeded("second"));

        assert_eq!(
            *log.lock().unwrap(),
            vec!["first".to_string(), "second".to_string(),]
        );
    }

    #[test]
    fn join_shape_runs_after_all_dependencies() {
        let mut plan = OrchestrationPlan::new();

        plan.register(
            OrchestrationTask::new("alpha", Vec::<String>::new(), success("alpha")).unwrap(),
        )
        .unwrap();

        plan.register(
            OrchestrationTask::new("beta", Vec::<String>::new(), success("beta")).unwrap(),
        )
        .unwrap();

        plan.register(
            OrchestrationTask::new(
                "join",
                vec!["alpha".to_string(), "beta".to_string()],
                success("joined"),
            )
            .unwrap(),
        )
        .unwrap();

        let report = futures_lite::future::block_on(execute(plan)).unwrap();

        assert!(report.succeeded("alpha"));

        assert!(report.succeeded("beta"));

        assert!(report.succeeded("join"));
    }

    #[test]
    fn failed_dependency_blocks_dependent_task() {
        let executed = Arc::new(Mutex::new(false));

        let dependent_executed = Arc::clone(&executed);

        let dependent: OrchestrationHandler = Arc::new(move || {
            let executed = Arc::clone(&dependent_executed);

            Box::pin(async move {
                *executed.lock().unwrap() = true;

                Ok(ExecutionPayload::new())
            })
        });

        let mut plan = OrchestrationPlan::new();

        plan.register(
            OrchestrationTask::new(
                "failure",
                Vec::<String>::new(),
                failure("synthetic failure"),
            )
            .unwrap(),
        )
        .unwrap();

        plan.register(
            OrchestrationTask::new("dependent", vec!["failure".to_string()], dependent).unwrap(),
        )
        .unwrap();

        let report = futures_lite::future::block_on(execute(plan)).unwrap();

        assert!(report.failed("failure"));

        assert!(report.failed("dependent"));

        assert!(!*executed.lock().unwrap());
    }

    #[test]
    fn failure_does_not_block_unrelated_task() {
        let mut plan = OrchestrationPlan::new();

        plan.register(
            OrchestrationTask::new(
                "failure",
                Vec::<String>::new(),
                failure("synthetic failure"),
            )
            .unwrap(),
        )
        .unwrap();

        plan.register(
            OrchestrationTask::new("unrelated", Vec::<String>::new(), success("alive")).unwrap(),
        )
        .unwrap();

        let report = futures_lite::future::block_on(execute(plan)).unwrap();

        assert!(report.failed("failure"));

        assert!(report.succeeded("unrelated"));
    }

    #[test]
    fn dependency_vocabulary_is_not_prescribed() {
        let mut plan = OrchestrationPlan::new();

        plan.register(
            OrchestrationTask::new("whatever.before", Vec::<String>::new(), success("one"))
                .unwrap(),
        )
        .unwrap();

        plan.register(
            OrchestrationTask::new(
                "whatever.after",
                vec!["whatever.before".to_string()],
                success("two"),
            )
            .unwrap(),
        )
        .unwrap();

        let report = futures_lite::future::block_on(execute(plan)).unwrap();

        assert!(report.succeeded("whatever.after"));
    }
}

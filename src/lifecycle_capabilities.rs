use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::lifecycle_execution::ExecutionPayload;
use crate::lifecycle_operation::PreparedOperation;

/*
 * Lifecycle Rust-native capability registry.
 *
 * Technical responsibility:
 * Associate an opaque implementation id with an asynchronous Rust
 * implementation capable of receiving one fully prepared Operation.
 *
 * This layer does not prescribe:
 * - artillery names
 * - objective names
 * - module semantics
 * - concrete Rust crates
 * - munition vocabulary
 * - tactics vocabulary
 * - intelligence vocabulary
 *
 * The implementation receives the complete PreparedOperation and is
 * responsible for understanding only the technical contract it owns.
 */

pub type CapabilityFuture =
    Pin<Box<dyn Future<Output = Result<ExecutionPayload, String>> + Send + 'static>>;

pub type CapabilityHandler =
    Arc<dyn Fn(PreparedOperation) -> CapabilityFuture + Send + Sync + 'static>;

#[derive(Default)]
pub struct CapabilityRegistry {
    handlers: BTreeMap<String, CapabilityHandler>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        implementation_id: impl Into<String>,
        handler: CapabilityHandler,
    ) -> Result<(), String> {
        let implementation_id = implementation_id.into();

        if implementation_id.trim().is_empty() {
            return Err("capability implementation id cannot be empty".to_string());
        }

        if self.handlers.contains_key(&implementation_id) {
            return Err(format!(
                "capability implementation '{implementation_id}' is already registered"
            ));
        }

        self.handlers.insert(implementation_id, handler);

        Ok(())
    }

    pub fn contains(&self, implementation_id: &str) -> bool {
        self.handlers.contains_key(implementation_id)
    }

    pub fn unregister(&mut self, implementation_id: &str) -> bool {
        self.handlers.remove(implementation_id).is_some()
    }

    pub async fn invoke(
        &self,
        implementation_id: &str,
        operation: PreparedOperation,
    ) -> Result<ExecutionPayload, String> {
        if implementation_id.trim().is_empty() {
            return Err("capability implementation id cannot be empty".to_string());
        }

        let handler = self.handlers.get(implementation_id).ok_or_else(|| {
            format!("capability implementation '{implementation_id}' is not registered")
        })?;

        handler(operation).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(artillery: &str, objective: &str) -> PreparedOperation {
        PreparedOperation {
            module_id: None,
            artillery: artillery.to_string(),
            objective: objective.to_string(),
            munition: BTreeMap::new(),
            tactics: BTreeMap::new(),
            intelligence: BTreeMap::new(),
        }
    }

    #[test]
    fn registry_starts_empty() {
        let registry = CapabilityRegistry::new();

        assert!(!registry.contains("anything"));
    }

    #[test]
    fn registry_accepts_arbitrary_implementation_ids() {
        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "future.implementation.whatever",
                Arc::new(|_| Box::pin(async { Ok(ExecutionPayload::new()) })),
            )
            .unwrap();

        assert!(registry.contains("future.implementation.whatever"));
    }

    #[test]
    fn registry_rejects_empty_implementation_id() {
        let mut registry = CapabilityRegistry::new();

        let error = registry
            .register(
                "   ",
                Arc::new(|_| Box::pin(async { Ok(ExecutionPayload::new()) })),
            )
            .unwrap_err();

        assert!(error.contains("cannot be empty"));
    }

    #[test]
    fn registry_rejects_duplicate_implementation() {
        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "alpha",
                Arc::new(|_| Box::pin(async { Ok(ExecutionPayload::new()) })),
            )
            .unwrap();

        let error = registry
            .register(
                "alpha",
                Arc::new(|_| Box::pin(async { Ok(ExecutionPayload::new()) })),
            )
            .unwrap_err();

        assert!(error.contains("already registered"));
    }

    #[test]
    fn registry_can_unregister_implementation() {
        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "alpha",
                Arc::new(|_| Box::pin(async { Ok(ExecutionPayload::new()) })),
            )
            .unwrap();

        assert!(registry.unregister("alpha"));

        assert!(!registry.contains("alpha"));
    }

    #[test]
    fn invocation_receives_complete_prepared_operation() {
        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "synthetic",
                Arc::new(|operation| {
                    Box::pin(async move {
                        let mut output = ExecutionPayload::new();

                        output.insert("artillery".to_string(), operation.artillery);

                        output.insert("objective".to_string(), operation.objective);

                        output.insert(
                            "munition".to_string(),
                            operation
                                .munition
                                .get("source")
                                .cloned()
                                .unwrap_or_default(),
                        );

                        output.insert(
                            "tactics".to_string(),
                            operation.tactics.get("opaque").cloned().unwrap_or_default(),
                        );

                        Ok(output)
                    })
                }),
            )
            .unwrap();

        let prepared = PreparedOperation {
            module_id: None,
            artillery: "arbitrary.artillery".to_string(),
            objective: "arbitrary.objective".to_string(),
            munition: BTreeMap::from([("source".to_string(), "munition-value".to_string())]),
            tactics: BTreeMap::from([("opaque".to_string(), "tactics-value".to_string())]),
            intelligence: BTreeMap::from([(
                "anything".to_string(),
                "intelligence-value".to_string(),
            )]),
        };

        let output =
            futures_lite::future::block_on(registry.invoke("synthetic", prepared)).unwrap();

        assert_eq!(
            output.get("artillery"),
            Some(&"arbitrary.artillery".to_string())
        );

        assert_eq!(
            output.get("objective"),
            Some(&"arbitrary.objective".to_string())
        );

        assert_eq!(output.get("munition"), Some(&"munition-value".to_string()));

        assert_eq!(output.get("tactics"), Some(&"tactics-value".to_string()));
    }

    #[test]
    fn registry_does_not_interpret_objective() {
        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "implementation",
                Arc::new(|operation| {
                    Box::pin(async move {
                        let mut output = ExecutionPayload::new();

                        output.insert("received".to_string(), operation.objective);

                        Ok(output)
                    })
                }),
            )
            .unwrap();

        let output = futures_lite::future::block_on(registry.invoke(
            "implementation",
            operation("whatever", "future.module.owns.this.objective"),
        ))
        .unwrap();

        assert_eq!(
            output.get("received"),
            Some(&"future.module.owns.this.objective".to_string())
        );
    }

    #[test]
    fn registry_propagates_handler_failure() {
        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "failure",
                Arc::new(|_| Box::pin(async { Err("synthetic capability failure".to_string()) })),
            )
            .unwrap();

        let error =
            futures_lite::future::block_on(registry.invoke("failure", operation("alpha", "one")))
                .unwrap_err();

        assert_eq!(error, "synthetic capability failure");
    }

    #[test]
    fn registry_rejects_unknown_implementation() {
        let registry = CapabilityRegistry::new();

        let error =
            futures_lite::future::block_on(registry.invoke("missing", operation("alpha", "one")))
                .unwrap_err();

        assert!(error.contains("is not registered"));
    }

    #[test]
    fn one_implementation_can_receive_arbitrary_objectives() {
        let mut registry = CapabilityRegistry::new();

        registry
            .register(
                "implementation",
                Arc::new(|operation| {
                    Box::pin(async move {
                        let mut output = ExecutionPayload::new();

                        output.insert("objective".to_string(), operation.objective);

                        Ok(output)
                    })
                }),
            )
            .unwrap();

        let first = futures_lite::future::block_on(
            registry.invoke("implementation", operation("alpha", "objective.one")),
        )
        .unwrap();

        let second = futures_lite::future::block_on(
            registry.invoke("implementation", operation("alpha", "objective.two")),
        )
        .unwrap();

        assert_eq!(first.get("objective"), Some(&"objective.one".to_string()));

        assert_eq!(second.get("objective"), Some(&"objective.two".to_string()));
    }
}

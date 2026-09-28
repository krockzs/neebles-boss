use std::collections::BTreeMap;

use crate::lifecycle_communication::LifecycleCommunication;

/*
 * N.E.E.B.L.E.S. Lifecycle Event / Log Contract.
 *
 * Technical responsibility:
 *
 * LifecycleCommunication snapshot
 *      + dynamic event kind
 *      + deterministic sequence
 *      -> immutable LifecycleEvent
 *      -> ordered LifecycleEventLog
 *
 * Event vocabulary is intentionally open.
 *
 * The event payload is an opaque String map copied from the current
 * LifecycleCommunication world. Therefore existing and future namespaces
 * survive without the Event layer having to understand their semantics.
 *
 * This layer does not:
 * - write files
 * - select a persistence backend
 * - emit telemetry
 * - know UI
 * - know IPC
 * - know Governor
 * - interpret module intent
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEvent {
    sequence: u64,
    kind: String,
    values: BTreeMap<String, String>,
}

impl LifecycleEvent {
    fn new(
        sequence: u64,
        kind: impl Into<String>,
        values: BTreeMap<String, String>,
    ) -> Result<Self, String> {
        if sequence == 0 {
            return Err("lifecycle event sequence cannot be zero".to_string());
        }

        let kind = kind.into();

        if kind.trim().is_empty() {
            return Err("lifecycle event kind cannot be empty".to_string());
        }

        for (key, value) in &values {
            if key.trim().is_empty() {
                return Err("lifecycle event value key cannot be empty".to_string());
            }

            if value.is_empty() {
                return Err(format!("lifecycle event value '{key}' cannot be empty"));
            }
        }

        Ok(Self {
            sequence,
            kind,
            values,
        })
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEventLog {
    next_sequence: u64,
    events: Vec<LifecycleEvent>,
}

impl LifecycleEventLog {
    pub fn new() -> Self {
        Self {
            next_sequence: 1,
            events: Vec::new(),
        }
    }

    pub fn record(
        &mut self,
        kind: impl Into<String>,
        communication: &LifecycleCommunication,
    ) -> Result<u64, String> {
        let sequence = self.next_sequence;

        let event = LifecycleEvent::new(sequence, kind, communication.values().clone())?;

        let next = sequence
            .checked_add(1)
            .ok_or_else(|| "lifecycle event sequence overflow".to_string())?;

        self.events.push(event);

        self.next_sequence = next;

        Ok(sequence)
    }

    pub fn events(&self) -> &[LifecycleEvent] {
        &self.events
    }

    pub fn latest(&self) -> Option<&LifecycleEvent> {
        self.events.last()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for LifecycleEventLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle_communication::LifecycleCommunication;
    use crate::lifecycle_state::LifecycleRuntimeState;

    fn communication() -> LifecycleCommunication {
        let state = LifecycleRuntimeState::new("execution-event-001", "deploy").unwrap();

        LifecycleCommunication::from_state(&state).unwrap()
    }

    #[test]
    fn dynamic_event_kind_is_not_hardcoded() {
        let communication = communication();

        let mut log = LifecycleEventLog::new();

        log.record("module.custom.future.event", &communication)
            .unwrap();

        assert_eq!(log.latest().unwrap().kind(), "module.custom.future.event");
    }

    #[test]
    fn event_snapshot_is_immutable_after_communication_changes() {
        let mut communication = communication();

        communication.put("phase", "running").unwrap();

        let mut log = LifecycleEventLog::new();

        log.record("snapshot.before", &communication).unwrap();

        communication.put("phase", "succeeded").unwrap();

        let event = log.latest().unwrap();

        assert_eq!(event.get("phase"), Some("running"));
    }

    #[test]
    fn require_tree_context_survives_event_snapshot() {
        let mut state = LifecycleRuntimeState::new("execution-event-002", "install").unwrap();

        state
            .merge_namespace(
                "require",
                &BTreeMap::from([
                    ("root".to_string(), "root.module".to_string()),
                    ("current".to_string(), "dependency.beta".to_string()),
                    ("required_by".to_string(), "dependency.alpha".to_string()),
                    (
                        "path".to_string(),
                        "root.module -> dependency.alpha -> dependency.beta".to_string(),
                    ),
                    ("depth".to_string(), "2".to_string()),
                ]),
            )
            .unwrap();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        let event_require_values = BTreeMap::from([
            ("root".to_string(), "root.module".to_string()),
            ("current".to_string(), "dependency.beta".to_string()),
            ("required_by".to_string(), "dependency.alpha".to_string()),
            (
                "path".to_string(),
                "root.module -> dependency.alpha -> dependency.beta".to_string(),
            ),
            ("depth".to_string(), "2".to_string()),
        ]);

        communication
            .replace_namespace("require", &event_require_values)
            .unwrap();

        assert_eq!(communication.get("require.root"), Some("root.module"));

        let mut log = LifecycleEventLog::new();

        log.record("require.progress", &communication).unwrap();

        let event = log.latest().unwrap();

        assert_eq!(event.get("require.root"), Some("root.module"));

        assert_eq!(event.get("require.current"), Some("dependency.beta"));

        assert_eq!(event.get("require.required_by"), Some("dependency.alpha"));

        assert_eq!(
            event.get("require.path"),
            Some("root.module -> dependency.alpha -> dependency.beta")
        );

        assert_eq!(event.get("require.depth"), Some("2"));
    }

    #[test]
    fn failure_context_survives_event_snapshot() {
        let mut communication = communication();

        communication
            .replace_failure(&BTreeMap::from([
                ("scope".to_string(), "operation".to_string()),
                ("status".to_string(), "failed".to_string()),
                (
                    "operation.compile.error".to_string(),
                    "synthetic failure".to_string(),
                ),
            ]))
            .unwrap();

        let mut log = LifecycleEventLog::new();

        log.record("transition.failed", &communication).unwrap();

        let event = log.latest().unwrap();

        assert_eq!(event.get("failure.scope"), Some("operation"));

        assert_eq!(
            event.get("failure.operation.compile.error"),
            Some("synthetic failure")
        );
    }

    #[test]
    fn all_current_communication_namespaces_survive() {
        let mut communication = communication();

        communication.put("human.message", "Working").unwrap();

        communication
            .replace_progress(&BTreeMap::from([("current".to_string(), "3".to_string())]))
            .unwrap();

        communication
            .replace_result(&BTreeMap::from([(
                "status".to_string(),
                "succeeded".to_string(),
            )]))
            .unwrap();

        communication
            .replace_failure(&BTreeMap::from([(
                "status".to_string(),
                "failed".to_string(),
            )]))
            .unwrap();

        let mut log = LifecycleEventLog::new();

        log.record("communication.snapshot", &communication)
            .unwrap();

        let event = log.latest().unwrap();

        assert_eq!(event.get("human.message"), Some("Working"));

        assert_eq!(event.get("progress.current"), Some("3"));

        assert_eq!(event.get("result.status"), Some("succeeded"));

        assert_eq!(event.get("failure.status"), Some("failed"));
    }

    #[test]
    fn arbitrary_future_fields_survive_without_event_schema_changes() {
        let mut communication = communication();

        communication
            .put("future.namespace.alpha", "opaque-value")
            .unwrap();

        let mut log = LifecycleEventLog::new();

        log.record("future.kind", &communication).unwrap();

        assert_eq!(
            log.latest().unwrap().get("future.namespace.alpha"),
            Some("opaque-value")
        );
    }

    #[test]
    fn event_log_preserves_order_and_monotonic_sequence() {
        let communication = communication();

        let mut log = LifecycleEventLog::new();

        let first = log.record("alpha", &communication).unwrap();

        let second = log.record("beta", &communication).unwrap();

        let third = log.record("gamma", &communication).unwrap();

        assert_eq!(first, 1);

        assert_eq!(second, 2);

        assert_eq!(third, 3);

        assert_eq!(log.events()[0].kind(), "alpha");

        assert_eq!(log.events()[1].kind(), "beta");

        assert_eq!(log.events()[2].kind(), "gamma");
    }

    #[test]
    fn rejected_event_does_not_consume_sequence() {
        let communication = communication();

        let mut log = LifecycleEventLog::new();

        assert!(log.record("   ", &communication,).is_err());

        assert!(log.is_empty());

        let sequence = log.record("valid", &communication).unwrap();

        assert_eq!(sequence, 1);
    }
}

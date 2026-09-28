use serde_json::{Map, Value};

use crate::external::{self, ExternalEnvelope};
use crate::lifecycle_event::LifecycleEvent;

/*
 * N.E.E.B.L.E.S. Lifecycle Telemetry Failure Hook.
 *
 * Lifecycle owns the structured failure event.
 *
 * External owns:
 * - the global telemetry permission
 * - privacy-safe disabled behavior
 * - envelope transport
 * - external.sock
 * - future remote delivery policy
 *
 * The Lifecycle hook never reads telemetry configuration and never decides
 * that telemetry is enabled.
 *
 * Only events carrying the failure namespace are eligible here.
 *
 * Payload construction stays lazy behind External's master telemetry gate.
 * Therefore disabled telemetry does not require Lifecycle to build a
 * diagnostic package merely to discard it afterward.
 *
 * Delivery is best-effort. Failure to report telemetry must never mutate
 * Lifecycle execution state or turn a local failure into another control
 * failure.
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleTelemetryOutcome {
    IgnoredNonFailure,
    Disabled,
    Delivered,
    DeliveryUnavailable(String),
}

fn is_failure_event(event: &LifecycleEvent) -> bool {
    event.values().keys().any(|key| key.starts_with("failure."))
}

fn failure_values(event: &LifecycleEvent) -> Map<String, Value> {
    let mut values = Map::new();

    for (key, value) in event.values() {
        let Some(path) = key.strip_prefix("failure.") else {
            continue;
        };

        if path.is_empty() {
            continue;
        }

        values.insert(path.to_string(), Value::String(value.clone()));
    }

    values
}

fn failure_package(event: &LifecycleEvent) -> Map<String, Value> {
    let mut package = Map::new();

    package.insert("source".to_string(), Value::String("lifecycle".to_string()));

    package.insert("event_sequence".to_string(), Value::from(event.sequence()));

    package.insert(
        "event_kind".to_string(),
        Value::String(event.kind().to_string()),
    );

    package.insert("failure".to_string(), Value::Object(failure_values(event)));

    package
}

fn technical_failure_message(event: &LifecycleEvent) -> String {
    if let Some(message) = event.get("failure.error") {
        return message.to_string();
    }

    for (key, value) in event.values() {
        if key.starts_with("failure.") && key.ends_with(".error") {
            return value.clone();
        }
    }

    if let Some(message) = event.get("failure.message") {
        return message.to_string();
    }

    "Lifecycle reported structured failure".to_string()
}

fn failure_message(event: &LifecycleEvent) -> Map<String, Value> {
    external::error_message("lifecycle_failure", technical_failure_message(event))
}

fn build_failure_envelope(event: &LifecycleEvent) -> Option<ExternalEnvelope> {
    if !is_failure_event(event) {
        return None;
    }

    external::build_external_envelope(
        "error",
        "",
        || failure_package(event),
        || failure_message(event),
    )
}

pub fn emit_failure(event: &LifecycleEvent) -> LifecycleTelemetryOutcome {
    if !is_failure_event(event) {
        return LifecycleTelemetryOutcome::IgnoredNonFailure;
    }

    let Some(envelope) = build_failure_envelope(event) else {
        return LifecycleTelemetryOutcome::Disabled;
    };

    match external::send(&envelope) {
        Ok(()) => LifecycleTelemetryOutcome::Delivered,

        Err(error) => LifecycleTelemetryOutcome::DeliveryUnavailable(error),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    use crate::lifecycle_communication::LifecycleCommunication;
    use crate::lifecycle_event::LifecycleEventLog;
    use crate::lifecycle_state::LifecycleRuntimeState;

    fn event_without_failure() -> LifecycleEvent {
        let state = LifecycleRuntimeState::new("execution-telemetry-clean", "open").unwrap();

        let communication = LifecycleCommunication::from_state(&state).unwrap();

        let mut log = LifecycleEventLog::new();

        log.record("transition.succeeded", &communication).unwrap();

        log.latest().unwrap().clone()
    }

    fn event_with_failure(failure: BTreeMap<String, String>) -> LifecycleEvent {
        let state = LifecycleRuntimeState::new("execution-telemetry-failure", "deploy").unwrap();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        communication.replace_failure(&failure).unwrap();

        let mut log = LifecycleEventLog::new();

        log.record("module.future.failure", &communication).unwrap();

        log.latest().unwrap().clone()
    }

    #[test]
    fn non_failure_event_is_not_telemetry_eligible() {
        let event = event_without_failure();

        assert!(!is_failure_event(&event));

        assert_eq!(
            emit_failure(&event),
            LifecycleTelemetryOutcome::IgnoredNonFailure
        );
    }

    #[test]
    fn structured_failure_context_becomes_dynamic_package() {
        let event = event_with_failure(BTreeMap::from([
            ("scope".to_string(), "operation".to_string()),
            ("status".to_string(), "failed".to_string()),
            (
                "execution.id".to_string(),
                "execution-telemetry-failure".to_string(),
            ),
            ("transition.id".to_string(), "deploy".to_string()),
            ("object.id".to_string(), "database".to_string()),
            ("require.root".to_string(), "root.module".to_string()),
            ("require.current".to_string(), "dependency.beta".to_string()),
            (
                "require.path".to_string(),
                "root.module -> dependency.beta".to_string(),
            ),
            (
                "operation.compile.error".to_string(),
                "compiler unavailable".to_string(),
            ),
        ]));

        assert!(is_failure_event(&event));

        let package = failure_package(&event);

        assert_eq!(
            package.get("source").and_then(Value::as_str),
            Some("lifecycle")
        );

        assert_eq!(
            package.get("event_kind").and_then(Value::as_str),
            Some("module.future.failure")
        );

        let failure = package.get("failure").and_then(Value::as_object).unwrap();

        assert_eq!(
            failure.get("scope").and_then(Value::as_str),
            Some("operation")
        );

        assert_eq!(
            failure.get("object.id").and_then(Value::as_str),
            Some("database")
        );

        assert_eq!(
            failure.get("require.current").and_then(Value::as_str),
            Some("dependency.beta")
        );

        assert_eq!(
            failure.get("require.path").and_then(Value::as_str),
            Some("root.module -> dependency.beta")
        );

        assert_eq!(
            failure
                .get("operation.compile.error")
                .and_then(Value::as_str),
            Some("compiler unavailable")
        );
    }

    #[test]
    fn arbitrary_future_failure_fields_survive_without_schema_change() {
        let event = event_with_failure(BTreeMap::from([
            ("status".to_string(), "failed".to_string()),
            (
                "future.vendor.alpha".to_string(),
                "opaque-diagnostic".to_string(),
            ),
        ]));

        let package = failure_package(&event);

        let failure = package.get("failure").and_then(Value::as_object).unwrap();

        assert_eq!(
            failure.get("future.vendor.alpha").and_then(Value::as_str),
            Some("opaque-diagnostic")
        );
    }

    #[test]
    fn telemetry_package_does_not_copy_non_failure_namespaces() {
        let state = LifecycleRuntimeState::new("execution-telemetry-filter", "deploy").unwrap();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        communication
            .put("human.message", "user-visible text")
            .unwrap();

        communication
            .put("future.private.opaque", "not-failure-context")
            .unwrap();

        communication
            .replace_failure(&BTreeMap::from([
                ("status".to_string(), "failed".to_string()),
                ("error".to_string(), "synthetic error".to_string()),
            ]))
            .unwrap();

        let mut log = LifecycleEventLog::new();

        log.record("failure.filtered", &communication).unwrap();

        let package = failure_package(log.latest().unwrap());

        let failure = package.get("failure").and_then(Value::as_object).unwrap();

        assert!(!failure.contains_key("human.message"));

        assert!(!failure.contains_key("future.private.opaque"));

        assert_eq!(
            failure.get("error").and_then(Value::as_str),
            Some("synthetic error")
        );
    }

    #[test]
    fn technical_error_is_used_as_diagnostic_message() {
        let event = event_with_failure(BTreeMap::from([
            ("status".to_string(), "failed".to_string()),
            (
                "operation.compile.error".to_string(),
                "compiler unavailable".to_string(),
            ),
        ]));

        let message = failure_message(&event);

        assert_eq!(
            message.get("code").and_then(Value::as_str),
            Some("lifecycle_failure")
        );

        assert_eq!(
            message.get("message").and_then(Value::as_str),
            Some("compiler unavailable")
        );
    }

    #[test]
    fn generic_failure_without_error_gets_safe_fallback_message() {
        let event = event_with_failure(BTreeMap::from([(
            "status".to_string(),
            "failed".to_string(),
        )]));

        assert_eq!(
            technical_failure_message(&event),
            "Lifecycle reported structured failure"
        );
    }

    #[test]
    fn event_identity_is_preserved_in_diagnostic_package() {
        let event = event_with_failure(BTreeMap::from([
            ("status".to_string(), "failed".to_string()),
            ("error".to_string(), "failure".to_string()),
        ]));

        let package = failure_package(&event);

        assert_eq!(
            package.get("event_sequence").and_then(Value::as_u64),
            Some(event.sequence())
        );

        assert_eq!(
            package.get("event_kind").and_then(Value::as_str),
            Some(event.kind())
        );
    }
}

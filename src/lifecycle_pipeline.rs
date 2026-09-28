use std::collections::BTreeMap;

use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_communication::LifecycleCommunication;
use crate::lifecycle_intelligence;
use crate::lifecycle_result::{NormalizedResult, NormalizedStatus};
use crate::lifecycle_state::LifecycleRuntimeState;

/*
 * N.E.E.B.L.E.S. Lifecycle Result / Intelligence / Battlefield Pipeline.
 *
 * Final flow for one completed operation:
 *
 * NormalizedResult
 *     -> ephemeral result namespace in Battlefield
 *     -> declarative intelligence selection
 *     -> persistent intelligence namespace in Battlefield
 *     -> terminal Runtime State phase
 *     -> structured Communication result snapshot
 *
 * Battlefield result is ephemeral execution material.
 * Battlefield intelligence is persistent selected material.
 * Communication result is a structured observation of the terminal result.
 *
 * The existing lifecycle_intelligence layer remains the authority
 * for result selection and intelligence persistence.
 *
 * This coordinator does not execute operations, choose transition order,
 * route FireControl, invoke capabilities, own Failure Context, emit logs,
 * emit telemetry, know Governor or render UI.
 *
 * Every mutation is staged first.
 * Failure before commit leaves Battlefield, Runtime State and
 * Communication unchanged.
 */

fn status_text(status: NormalizedStatus) -> &'static str {
    match status {
        NormalizedStatus::Succeeded => "succeeded",

        NormalizedStatus::Failed => "failed",

        NormalizedStatus::Cancelled => "cancelled",
    }
}

fn communication_result(operation_id: &str, result: &NormalizedResult) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();

    values.insert("operation.id".to_string(), operation_id.to_string());

    values.insert(
        "status".to_string(),
        status_text(result.status()).to_string(),
    );

    for (key, value) in result.payload() {
        values.insert(format!("payload.{key}"), value.clone());
    }

    if let Some(error) = result.error() {
        values.insert("error".to_string(), error.to_string());
    }

    values
}

pub fn commit_operation_result(
    operation_id: &str,
    intelligence: &BTreeMap<String, String>,
    result: &NormalizedResult,
    battlefield: &mut Battlefield,
    state: &mut LifecycleRuntimeState,
    communication: &mut LifecycleCommunication,
) -> Result<(), String> {
    if operation_id.trim().is_empty() {
        return Err("lifecycle pipeline operation id cannot be empty".to_string());
    }

    let mut staged_battlefield = battlefield.clone();

    let mut staged_state = state.clone();

    let mut staged_communication = communication.clone();

    lifecycle_intelligence::propagate(operation_id, intelligence, result, &mut staged_battlefield)?;

    staged_state.enter_phase(status_text(result.status()))?;

    staged_communication.refresh_state(&staged_state)?;

    staged_communication.replace_result(&communication_result(operation_id, result))?;

    *battlefield = staged_battlefield;

    *state = staged_state;

    *communication = staged_communication;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(namespace: &str, path: &str) -> String {
        let marker = char::from_u32(36).unwrap();

        format!("{marker}{namespace}.{path}")
    }

    fn running_state() -> LifecycleRuntimeState {
        let mut state = LifecycleRuntimeState::new("execution-001", "anything").unwrap();

        state.enter_phase("running").unwrap();

        state
    }

    #[test]
    fn success_persists_intelligence_and_discards_ephemeral_result() {
        let result = NormalizedResult::succeeded(BTreeMap::from([
            ("hash".to_string(), "abc123".to_string()),
            ("path".to_string(), "/tmp/archive".to_string()),
        ]));

        let intelligence = BTreeMap::from([
            ("digest".to_string(), reference("result", "hash")),
            ("artifact".to_string(), reference("result", "path")),
        ]);

        let mut battlefield = Battlefield::new();

        let mut state = running_state();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        commit_operation_result(
            "download",
            &intelligence,
            &result,
            &mut battlefield,
            &mut state,
            &mut communication,
        )
        .unwrap();

        assert_eq!(
            battlefield.get("intelligence", "download.digest",),
            Some("abc123")
        );

        assert_eq!(
            battlefield.get("intelligence", "download.artifact",),
            Some("/tmp/archive")
        );

        assert!(!battlefield.contains_namespace("result"));

        assert_eq!(state.phase(), Some("succeeded"));

        assert_eq!(communication.get("state.phase"), Some("succeeded"));

        assert_eq!(communication.get("result.operation.id"), Some("download"));

        assert_eq!(communication.get("result.status"), Some("succeeded"));

        assert_eq!(communication.get("result.payload.hash"), Some("abc123"));
    }

    #[test]
    fn failed_result_becomes_terminal_without_intelligence() {
        let result = NormalizedResult::failed("synthetic failure").unwrap();

        let mut battlefield = Battlefield::new();

        battlefield.insert("result", "stale", "old");

        let mut state = running_state();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        commit_operation_result(
            "operation",
            &BTreeMap::new(),
            &result,
            &mut battlefield,
            &mut state,
            &mut communication,
        )
        .unwrap();

        assert!(!battlefield.contains_namespace("result"));

        assert!(!battlefield.contains_namespace("intelligence"));

        assert_eq!(state.phase(), Some("failed"));

        assert_eq!(communication.get("result.status"), Some("failed"));

        assert_eq!(communication.get("result.error"), Some("synthetic failure"));
    }

    #[test]
    fn cancellation_remains_distinct_terminal_state() {
        let result = NormalizedResult::cancelled("synthetic cancellation").unwrap();

        let mut battlefield = Battlefield::new();

        let mut state = running_state();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        commit_operation_result(
            "operation",
            &BTreeMap::new(),
            &result,
            &mut battlefield,
            &mut state,
            &mut communication,
        )
        .unwrap();

        assert_eq!(state.phase(), Some("cancelled"));

        assert_eq!(communication.get("result.status"), Some("cancelled"));

        assert_eq!(
            communication.get("result.error"),
            Some("synthetic cancellation")
        );
    }

    #[test]
    fn opaque_result_value_may_be_empty() {
        let result = NormalizedResult::succeeded(BTreeMap::from([(
            "possibly.empty".to_string(),
            String::new(),
        )]));

        let mut battlefield = Battlefield::new();

        let mut state = running_state();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        commit_operation_result(
            "operation",
            &BTreeMap::new(),
            &result,
            &mut battlefield,
            &mut state,
            &mut communication,
        )
        .unwrap();

        assert_eq!(communication.get("result.payload.possibly.empty"), Some(""));
    }

    #[test]
    fn ordinary_communication_still_rejects_empty_value() {
        let mut communication = LifecycleCommunication::new();

        assert!(communication.put("human.message", "").is_err());
    }

    #[test]
    fn selector_failure_is_atomic_across_all_surfaces() {
        let result = NormalizedResult::succeeded(BTreeMap::from([(
            "present".to_string(),
            "value".to_string(),
        )]));

        let intelligence =
            BTreeMap::from([("missing".to_string(), reference("result", "not.present"))]);

        let mut battlefield = Battlefield::new();

        battlefield.insert("existing", "value", "kept");

        let mut state = running_state();

        state.put("future.context", "kept").unwrap();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        communication.put("human.message", "kept").unwrap();

        let original_battlefield = battlefield.clone();

        let original_state = state.clone();

        let original_communication = communication.clone();

        let result = commit_operation_result(
            "operation",
            &intelligence,
            &result,
            &mut battlefield,
            &mut state,
            &mut communication,
        );

        assert!(result.is_err());

        assert_eq!(battlefield, original_battlefield);

        assert_eq!(state, original_state);

        assert_eq!(communication, original_communication);
    }

    #[test]
    fn intelligence_from_previous_operations_survives() {
        let first =
            NormalizedResult::succeeded(BTreeMap::from([("value".to_string(), "one".to_string())]));

        let second =
            NormalizedResult::succeeded(BTreeMap::from([("value".to_string(), "two".to_string())]));

        let intelligence = BTreeMap::from([("saved".to_string(), reference("result", "value"))]);

        let mut battlefield = Battlefield::new();

        let mut state = running_state();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        commit_operation_result(
            "first",
            &intelligence,
            &first,
            &mut battlefield,
            &mut state,
            &mut communication,
        )
        .unwrap();

        state.enter_phase("running").unwrap();

        communication.refresh_state(&state).unwrap();

        commit_operation_result(
            "second",
            &intelligence,
            &second,
            &mut battlefield,
            &mut state,
            &mut communication,
        )
        .unwrap();

        assert_eq!(battlefield.get("intelligence", "first.saved",), Some("one"));

        assert_eq!(
            battlefield.get("intelligence", "second.saved",),
            Some("two")
        );
    }

    #[test]
    fn require_context_survives_pipeline_commit() {
        let result = NormalizedResult::succeeded(BTreeMap::new());

        let mut battlefield = Battlefield::new();

        battlefield.insert("require", "current", "dependency");

        let mut state = running_state();

        state
            .merge_namespace(
                "require",
                &BTreeMap::from([
                    ("root".to_string(), "root-module".to_string()),
                    ("current".to_string(), "dependency".to_string()),
                    ("path".to_string(), "root-module -> dependency".to_string()),
                ]),
            )
            .unwrap();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        commit_operation_result(
            "operation",
            &BTreeMap::new(),
            &result,
            &mut battlefield,
            &mut state,
            &mut communication,
        )
        .unwrap();

        assert_eq!(battlefield.get("require", "current",), Some("dependency"));

        assert_eq!(state.get("require.current"), Some("dependency"));

        assert_eq!(
            communication.get("state.require.current"),
            Some("dependency")
        );
    }

    #[test]
    fn communication_result_is_replaced_not_accumulated() {
        let first =
            NormalizedResult::succeeded(BTreeMap::from([("first".to_string(), "one".to_string())]));

        let second = NormalizedResult::succeeded(BTreeMap::from([(
            "second".to_string(),
            "two".to_string(),
        )]));

        let mut battlefield = Battlefield::new();

        let mut state = running_state();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        commit_operation_result(
            "first",
            &BTreeMap::new(),
            &first,
            &mut battlefield,
            &mut state,
            &mut communication,
        )
        .unwrap();

        state.enter_phase("running").unwrap();

        communication.refresh_state(&state).unwrap();

        commit_operation_result(
            "second",
            &BTreeMap::new(),
            &second,
            &mut battlefield,
            &mut state,
            &mut communication,
        )
        .unwrap();

        assert_eq!(communication.get("result.payload.first"), None);

        assert_eq!(communication.get("result.payload.second"), Some("two"));

        assert_eq!(communication.get("result.operation.id"), Some("second"));
    }

    #[test]
    fn empty_operation_id_fails_without_mutation() {
        let result = NormalizedResult::succeeded(BTreeMap::new());

        let mut battlefield = Battlefield::new();

        let mut state = running_state();

        let mut communication = LifecycleCommunication::from_state(&state).unwrap();

        let original_battlefield = battlefield.clone();

        let original_state = state.clone();

        let original_communication = communication.clone();

        let error = commit_operation_result(
            "   ",
            &BTreeMap::new(),
            &result,
            &mut battlefield,
            &mut state,
            &mut communication,
        )
        .unwrap_err();

        assert!(error.contains("operation id cannot be empty"));

        assert_eq!(battlefield, original_battlefield);

        assert_eq!(state, original_state);

        assert_eq!(communication, original_communication);
    }
}

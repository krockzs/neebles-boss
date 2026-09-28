use std::collections::BTreeMap;

use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_resolver;
use crate::lifecycle_result::NormalizedResult;

/*
 * Lifecycle result and intelligence propagation.
 *
 * Technical responsibility:
 *
 * Successful normalized payload
 *      -> temporary result namespace
 *      -> declarative intelligence selection
 *      -> persistent intelligence namespace
 *
 * result is ephemeral execution state.
 *
 * intelligence is persisted inside the current Battlefield using:
 *
 *     operation_id + "." + declared intelligence key
 *
 * This produces paths such as:
 *
 *     first.output
 *
 * under the intelligence namespace.
 *
 * The selector value is resolved only after the capability has produced
 * its real payload.
 *
 * Selection is staged before persistence so a failed selector cannot leave
 * partially written intelligence.
 *
 * This layer does not:
 * - understand capability semantics
 * - prescribe payload field names
 * - prescribe intelligence keys
 * - assign execution order
 * - interpret module technologies
 * - expose presentation behavior
 */

pub fn propagate(
    operation_id: &str,
    intelligence: &BTreeMap<String, String>,
    result: &NormalizedResult,
    battlefield: &mut Battlefield,
) -> Result<(), String> {
    if operation_id.trim().is_empty() {
        return Err("lifecycle intelligence operation id cannot be empty".to_string());
    }

    for key in intelligence.keys() {
        if key.trim().is_empty() {
            return Err(format!(
                "lifecycle intelligence key for operation '{operation_id}' cannot be empty"
            ));
        }
    }

    battlefield.clear_namespace("result");

    if !result.is_success() {
        return Ok(());
    }

    for (key, value) in result.payload() {
        battlefield.insert("result", key.clone(), value.clone());
    }

    let selected = intelligence
        .iter()
        .map(|(key, selector)| {
            lifecycle_resolver::resolve(selector, battlefield)
                .map(|value| (key.clone(), value))
                .map_err(|error| {
                    format!(
                        "could not select lifecycle intelligence '{operation_id}.{key}': {error}"
                    )
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>();

    battlefield.clear_namespace("result");

    let selected = selected?;

    for (key, value) in selected {
        battlefield.insert("intelligence", format!("{operation_id}.{key}"), value);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle_execution::ExecutionPayload;

    #[test]
    fn successful_payload_can_be_selected_into_persistent_intelligence() {
        let marker = char::from_u32(36).unwrap();

        let result = NormalizedResult::succeeded(BTreeMap::from([
            ("hash".to_string(), "abc123".to_string()),
            ("path".to_string(), "/tmp/archive".to_string()),
        ]));

        let intelligence = BTreeMap::from([
            ("digest".to_string(), format!("{marker}result.hash")),
            ("artifact".to_string(), format!("{marker}result.path")),
        ]);

        let mut battlefield = Battlefield::new();

        propagate("download", &intelligence, &result, &mut battlefield).unwrap();

        assert_eq!(
            battlefield.get("intelligence", "download.digest",),
            Some("abc123")
        );

        assert_eq!(
            battlefield.get("intelligence", "download.artifact",),
            Some("/tmp/archive")
        );
    }

    #[test]
    fn result_namespace_is_ephemeral_after_successful_selection() {
        let marker = char::from_u32(36).unwrap();

        let result = NormalizedResult::succeeded(BTreeMap::from([(
            "value".to_string(),
            "temporary".to_string(),
        )]));

        let intelligence = BTreeMap::from([("saved".to_string(), format!("{marker}result.value"))]);

        let mut battlefield = Battlefield::new();

        propagate("first", &intelligence, &result, &mut battlefield).unwrap();

        assert!(!battlefield.contains_namespace("result"));

        assert_eq!(
            battlefield.get("intelligence", "first.saved",),
            Some("temporary")
        );
    }

    #[test]
    fn result_payload_is_not_persisted_when_no_intelligence_is_requested() {
        let result = NormalizedResult::succeeded(BTreeMap::from([(
            "secret".to_string(),
            "ephemeral".to_string(),
        )]));

        let mut battlefield = Battlefield::new();

        propagate("first", &BTreeMap::new(), &result, &mut battlefield).unwrap();

        assert!(!battlefield.contains_namespace("result"));

        assert!(!battlefield.contains_namespace("intelligence"));
    }

    #[test]
    fn failed_result_clears_stale_result_namespace() {
        let mut battlefield = Battlefield::new();

        battlefield.insert("result", "stale", "old-value");

        let result = NormalizedResult::failed("synthetic failure").unwrap();

        propagate("first", &BTreeMap::new(), &result, &mut battlefield).unwrap();

        assert!(!battlefield.contains_namespace("result"));
    }

    #[test]
    fn selector_failure_does_not_leave_result_namespace() {
        let marker = char::from_u32(36).unwrap();

        let result = NormalizedResult::succeeded(ExecutionPayload::new());

        let intelligence = BTreeMap::from([(
            "missing".to_string(),
            format!("{marker}result.does_not_exist"),
        )]);

        let mut battlefield = Battlefield::new();

        let error = propagate("first", &intelligence, &result, &mut battlefield).unwrap_err();

        assert!(error.contains("first.missing"));

        assert!(!battlefield.contains_namespace("result"));

        assert!(!battlefield.contains("intelligence", "first.missing",));
    }

    #[test]
    fn selector_failure_is_transactional_for_intelligence() {
        let marker = char::from_u32(36).unwrap();

        let result = NormalizedResult::succeeded(BTreeMap::from([(
            "good".to_string(),
            "value".to_string(),
        )]));

        let intelligence = BTreeMap::from([
            ("good".to_string(), format!("{marker}result.good")),
            ("bad".to_string(), format!("{marker}result.missing")),
        ]);

        let mut battlefield = Battlefield::new();

        assert!(propagate("first", &intelligence, &result, &mut battlefield,).is_err());

        assert!(!battlefield.contains("intelligence", "first.good",));

        assert!(!battlefield.contains("intelligence", "first.bad",));
    }

    #[test]
    fn arbitrary_dotted_intelligence_key_remains_opaque() {
        let marker = char::from_u32(36).unwrap();

        let result = NormalizedResult::succeeded(BTreeMap::from([(
            "value".to_string(),
            "opaque".to_string(),
        )]));

        let intelligence = BTreeMap::from([(
            "future.output.path".to_string(),
            format!("{marker}result.value"),
        )]);

        let mut battlefield = Battlefield::new();

        propagate("operation", &intelligence, &result, &mut battlefield).unwrap();

        assert_eq!(
            battlefield.get("intelligence", "operation.future.output.path",),
            Some("opaque")
        );
    }

    #[test]
    fn plain_intelligence_value_remains_valid_opaque_data() {
        let result = NormalizedResult::succeeded(ExecutionPayload::new());

        let intelligence = BTreeMap::from([("metadata".to_string(), "literal-value".to_string())]);

        let mut battlefield = Battlefield::new();

        propagate("operation", &intelligence, &result, &mut battlefield).unwrap();

        assert_eq!(
            battlefield.get("intelligence", "operation.metadata",),
            Some("literal-value")
        );
    }

    #[test]
    fn empty_operation_id_is_rejected() {
        let result = NormalizedResult::succeeded(ExecutionPayload::new());

        let error =
            propagate("   ", &BTreeMap::new(), &result, &mut Battlefield::new()).unwrap_err();

        assert!(error.contains("operation id cannot be empty"));
    }

    #[test]
    fn empty_intelligence_key_is_rejected() {
        let result = NormalizedResult::succeeded(ExecutionPayload::new());

        let error = propagate(
            "operation",
            &BTreeMap::from([("   ".to_string(), "value".to_string())]),
            &result,
            &mut Battlefield::new(),
        )
        .unwrap_err();

        assert!(error.contains("intelligence key"));
    }
}

use crate::lifecycle_execution::{ExecutionOutcome, ExecutionPayload};

/*
 * Lifecycle normalized terminal result.
 *
 * Technical responsibility:
 * Provide one stable internal representation for a completed
 * Lifecycle execution unit.
 *
 * This layer does not know:
 * - modules
 * - artillery
 * - objectives
 * - munition
 * - tactics
 * - intelligence persistence
 * - process semantics
 * - human presentation
 *
 * Cancellation is explicit. It is never inferred by parsing an
 * arbitrary error String.
 */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedResult {
    status: NormalizedStatus,
    payload: ExecutionPayload,
    error: Option<String>,
}

impl NormalizedResult {
    pub fn succeeded(payload: ExecutionPayload) -> Self {
        Self {
            status: NormalizedStatus::Succeeded,
            payload,
            error: None,
        }
    }

    pub fn failed(error: impl Into<String>) -> Result<Self, String> {
        let error = error.into();

        if error.trim().is_empty() {
            return Err("normalized failure error cannot be empty".to_string());
        }

        Ok(Self {
            status: NormalizedStatus::Failed,
            payload: ExecutionPayload::new(),
            error: Some(error),
        })
    }

    pub fn cancelled(reason: impl Into<String>) -> Result<Self, String> {
        let reason = reason.into();

        if reason.trim().is_empty() {
            return Err("normalized cancellation reason cannot be empty".to_string());
        }

        Ok(Self {
            status: NormalizedStatus::Cancelled,
            payload: ExecutionPayload::new(),
            error: Some(reason),
        })
    }

    pub fn status(&self) -> NormalizedStatus {
        self.status
    }

    pub fn payload(&self) -> &ExecutionPayload {
        &self.payload
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn is_success(&self) -> bool {
        self.status == NormalizedStatus::Succeeded
    }

    pub fn is_failure(&self) -> bool {
        self.status == NormalizedStatus::Failed
    }

    pub fn is_cancelled(&self) -> bool {
        self.status == NormalizedStatus::Cancelled
    }
}

impl From<ExecutionOutcome> for NormalizedResult {
    fn from(outcome: ExecutionOutcome) -> Self {
        match outcome {
            ExecutionOutcome::Success(payload) => Self::succeeded(payload),

            ExecutionOutcome::Failure(error) => Self {
                status: NormalizedStatus::Failed,
                payload: ExecutionPayload::new(),
                error: Some(error),
            },
        }
    }
}

pub fn normalize_result(
    result: Result<ExecutionPayload, String>,
) -> Result<NormalizedResult, String> {
    match result {
        Ok(payload) => Ok(NormalizedResult::succeeded(payload)),

        Err(error) => NormalizedResult::failed(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;

    #[test]
    fn success_preserves_arbitrary_payload() {
        let payload = BTreeMap::from([
            ("future.output".to_string(), "opaque-value".to_string()),
            ("another".to_string(), "anything".to_string()),
        ]);

        let result = NormalizedResult::succeeded(payload.clone());

        assert!(result.is_success());

        assert_eq!(result.status(), NormalizedStatus::Succeeded);

        assert_eq!(result.payload(), &payload);

        assert_eq!(result.error(), None);
    }

    #[test]
    fn failure_has_explicit_terminal_status() {
        let result = NormalizedResult::failed("synthetic failure").unwrap();

        assert!(result.is_failure());

        assert_eq!(result.status(), NormalizedStatus::Failed);

        assert!(result.payload().is_empty());

        assert_eq!(result.error(), Some("synthetic failure"));
    }

    #[test]
    fn cancellation_has_explicit_terminal_status() {
        let result = NormalizedResult::cancelled("synthetic cancellation").unwrap();

        assert!(result.is_cancelled());

        assert_eq!(result.status(), NormalizedStatus::Cancelled);

        assert_eq!(result.error(), Some("synthetic cancellation"));
    }

    #[test]
    fn empty_failure_error_is_rejected() {
        let result = NormalizedResult::failed("   ");

        assert!(result.is_err());
    }

    #[test]
    fn empty_cancellation_reason_is_rejected() {
        let result = NormalizedResult::cancelled("   ");

        assert!(result.is_err());
    }

    #[test]
    fn execution_success_converts_to_normalized_success() {
        let payload = BTreeMap::from([("answer".to_string(), "42".to_string())]);

        let result: NormalizedResult = ExecutionOutcome::Success(payload.clone()).into();

        assert!(result.is_success());

        assert_eq!(result.payload(), &payload);
    }

    #[test]
    fn execution_failure_converts_to_normalized_failure() {
        let result: NormalizedResult =
            ExecutionOutcome::Failure("execution failed".to_string()).into();

        assert!(result.is_failure());

        assert_eq!(result.error(), Some("execution failed"));
    }

    #[test]
    fn generic_ok_result_can_be_normalized() {
        let source: Result<ExecutionPayload, String> = Ok(BTreeMap::from([(
            "value".to_string(),
            "opaque".to_string(),
        )]));

        let result = normalize_result(source).unwrap();

        assert!(result.is_success());

        assert_eq!(result.payload().get("value"), Some(&"opaque".to_string()));
    }

    #[test]
    fn generic_error_result_can_be_normalized() {
        let source: Result<ExecutionPayload, String> = Err("arbitrary failure".to_string());

        let result = normalize_result(source).unwrap();

        assert!(result.is_failure());

        assert_eq!(result.error(), Some("arbitrary failure"));
    }

    #[test]
    fn cancellation_is_not_inferred_from_error_text() {
        let source: Result<ExecutionPayload, String> = Err("execution cancelled".to_string());

        let result = normalize_result(source).unwrap();

        assert!(result.is_failure());

        assert!(!result.is_cancelled());
    }

    #[test]
    fn result_layer_does_not_prescribe_payload_vocabulary() {
        let payload = BTreeMap::from([(
            "whatever.future.module.key".to_string(),
            "opaque".to_string(),
        )]);

        let result = NormalizedResult::succeeded(payload);

        assert_eq!(
            result.payload().get("whatever.future.module.key"),
            Some(&"opaque".to_string())
        );
    }
}

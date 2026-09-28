use crate::module_ipc::protocol::{ModuleError, ModuleMessage};

use std::collections::HashMap;

use std::sync::{
    mpsc::{self, Receiver, SyncSender},
    Arc, Mutex,
};

#[derive(Debug)]
struct PendingRequest {
    module: String,
    session_id: String,
    contract: String,
    endpoint: String,
    sender: SyncSender<ModuleMessage>,
}

#[derive(Debug, Clone, Default)]
pub struct PendingRegistry {
    inner: Arc<Mutex<HashMap<String, PendingRequest>>>,
}

impl PendingRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /*
     * Register one request against the exact runtime
     * incarnation that is expected to answer it.
     *
     * Session ownership matters: a later runtime for the
     * same module must never inherit or cancel requests
     * belonging to an older session.
     */
    pub fn register(
        &self,
        id: &str,
        module: &str,
        session_id: &str,
        contract: &str,
        endpoint: &str,
    ) -> Result<Receiver<ModuleMessage>, String> {
        if id.trim().is_empty() {
            return Err("pending request id cannot be empty".to_string());
        }

        if module.trim().is_empty() {
            return Err("pending request module cannot be empty".to_string());
        }

        if session_id.trim().is_empty() {
            return Err("pending request session id cannot be empty".to_string());
        }

        if contract.trim().is_empty() {
            return Err("pending request contract cannot be empty".to_string());
        }

        if endpoint.trim().is_empty() {
            return Err("pending request endpoint cannot be empty".to_string());
        }

        let (sender, receiver) = mpsc::sync_channel(1);

        let mut pending = self
            .inner
            .lock()
            .map_err(|_| "pending request registry lock poisoned".to_string())?;

        if pending.contains_key(id) {
            return Err(format!("pending request '{}' already exists", id));
        }

        pending.insert(
            id.to_string(),
            PendingRequest {
                module: module.to_string(),
                session_id: session_id.to_string(),
                contract: contract.to_string(),
                endpoint: endpoint.to_string(),
                sender,
            },
        );

        Ok(receiver)
    }

    pub fn resolve_response(
        &self,
        id: &str,
        module: &str,
        session_id: &str,
        contract: &str,
        endpoint: &str,
        message: ModuleMessage,
    ) -> Result<bool, String> {
        let request = {
            let mut pending = self
                .inner
                .lock()
                .map_err(|_| "pending request registry lock poisoned".to_string())?;

            let Some(request) = pending.get(id) else {
                return Ok(false);
            };

            if request.module != module {
                return Err(format!(
                    "pending request '{}' module mismatch: expected '{}' received '{}'",
                    id, request.module, module
                ));
            }

            if request.session_id != session_id {
                return Err(format!(
                    "pending request '{}' session mismatch for module '{}': expected '{}' received '{}'",
                    id, module, request.session_id, session_id
                ));
            }

            if request.contract != contract {
                return Err(format!(
                    "pending request '{}' contract mismatch: expected '{}' received '{}'",
                    id, request.contract, contract
                ));
            }

            if request.endpoint != endpoint {
                return Err(format!(
                    "pending request '{}' endpoint mismatch: expected '{}' received '{}'",
                    id, request.endpoint, endpoint
                ));
            }

            pending
                .remove(id)
                .expect("validated pending request must still exist")
        };

        request.sender.send(message).map_err(|_| {
            format!(
                "request '{}' receiver disappeared before response delivery",
                id
            )
        })?;

        Ok(true)
    }

    pub fn resolve_error(
        &self,
        id: &str,
        module: &str,
        session_id: &str,
        message: ModuleMessage,
    ) -> Result<bool, String> {
        let request = {
            let mut pending = self
                .inner
                .lock()
                .map_err(|_| "pending request registry lock poisoned".to_string())?;

            let Some(request) = pending.get(id) else {
                return Ok(false);
            };

            if request.module != module {
                return Err(format!(
                    "pending request '{}' module mismatch: expected '{}' received '{}'",
                    id, request.module, module
                ));
            }

            if request.session_id != session_id {
                return Err(format!(
                    "pending request '{}' session mismatch for module '{}': expected '{}' received '{}'",
                    id, module, request.session_id, session_id
                ));
            }

            pending
                .remove(id)
                .expect("validated pending request must still exist")
        };

        request.sender.send(message).map_err(|_| {
            format!(
                "request '{}' receiver disappeared before error delivery",
                id
            )
        })?;

        Ok(true)
    }

    pub fn cancel(&self, id: &str) -> Result<bool, String> {
        let mut pending = self
            .inner
            .lock()
            .map_err(|_| "pending request registry lock poisoned".to_string())?;

        Ok(pending.remove(id).is_some())
    }

    /*
     * Fail every in-flight request owned by one exact
     * runtime session.
     *
     * Requests are removed from the registry before any
     * delivery is attempted, so a late response from a dead
     * session cannot resolve the same request afterwards.
     */
    pub fn fail_session(
        &self,
        module: &str,
        session_id: &str,
        reason: &str,
    ) -> Result<usize, String> {
        let failed = {
            let mut pending = self
                .inner
                .lock()
                .map_err(|_| "pending request registry lock poisoned".to_string())?;

            let ids = pending
                .iter()
                .filter_map(|(id, request)| {
                    if request.module == module && request.session_id == session_id {
                        Some(id.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            let mut failed = Vec::with_capacity(ids.len());

            for id in ids {
                if let Some(request) = pending.remove(&id) {
                    failed.push((id, request));
                }
            }

            failed
        };

        let count = failed.len();

        for (id, request) in failed {
            let message = ModuleMessage::Error {
                id: Some(id),
                module: Some(module.to_string()),

                error: ModuleError {
                    kind: "runtime_disconnected".to_string(),

                    message: format!(
                        "module '{}' runtime session '{}' disconnected before completing the request: {}",
                        module,
                        session_id,
                        reason
                    ),

                    details: None,
                },
            };

            /*
             * The caller may already have disappeared due to
             * its own timeout. That is not a registry failure:
             * the request has already been removed safely.
             */
            let _ = request.sender.send(message);
        }

        Ok(count)
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;

    fn payload(id: &str, module: &str) -> ModuleMessage {
        ModuleMessage::Error {
            id: Some(id.to_string()),
            module: Some(module.to_string()),
            error: ModuleError {
                kind: "test".to_string(),
                message: "test".to_string(),
                details: None,
            },
        }
    }

    #[test]
    fn wrong_endpoint_does_not_consume_pending() {
        let registry = PendingRegistry::new();

        let receiver = registry
            .register("req-a", "alpha", "session-a", "commands", "version")
            .unwrap();

        assert!(registry
            .resolve_response(
                "req-a",
                "alpha",
                "session-a",
                "commands",
                "wrong",
                payload("req-a", "alpha"),
            )
            .is_err());

        assert!(receiver.try_recv().is_err());

        assert!(registry
            .resolve_response(
                "req-a",
                "alpha",
                "session-a",
                "commands",
                "version",
                payload("req-a", "alpha"),
            )
            .unwrap());

        assert!(receiver.recv().is_ok());
    }

    #[test]
    fn wrong_contract_does_not_consume_pending() {
        let registry = PendingRegistry::new();

        let receiver = registry
            .register("req-b", "alpha", "session-a", "commands", "version")
            .unwrap();

        assert!(registry
            .resolve_response(
                "req-b",
                "alpha",
                "session-a",
                "wrong",
                "version",
                payload("req-b", "alpha"),
            )
            .is_err());

        assert!(receiver.try_recv().is_err());

        assert!(registry
            .resolve_response(
                "req-b",
                "alpha",
                "session-a",
                "commands",
                "version",
                payload("req-b", "alpha"),
            )
            .unwrap());

        assert!(receiver.recv().is_ok());
    }

    #[test]
    fn wrong_session_does_not_consume_pending() {
        let registry = PendingRegistry::new();

        let receiver = registry
            .register("req-c", "alpha", "session-a", "commands", "version")
            .unwrap();

        assert!(registry
            .resolve_response(
                "req-c",
                "alpha",
                "session-b",
                "commands",
                "version",
                payload("req-c", "alpha"),
            )
            .is_err());

        assert!(receiver.try_recv().is_err());

        assert!(registry
            .resolve_response(
                "req-c",
                "alpha",
                "session-a",
                "commands",
                "version",
                payload("req-c", "alpha"),
            )
            .unwrap());

        assert!(receiver.recv().is_ok());
    }

    #[test]
    fn error_requires_exact_runtime_session() {
        let registry = PendingRegistry::new();

        let receiver = registry
            .register("req-d", "alpha", "session-a", "commands", "version")
            .unwrap();

        assert!(registry
            .resolve_error("req-d", "alpha", "session-b", payload("req-d", "alpha"),)
            .is_err());

        assert!(receiver.try_recv().is_err());

        assert!(registry
            .resolve_error("req-d", "alpha", "session-a", payload("req-d", "alpha"),)
            .unwrap());

        assert!(receiver.recv().is_ok());
    }

    #[test]
    fn unknown_request_returns_false() {
        let registry = PendingRegistry::new();

        assert!(!registry
            .resolve_response(
                "missing",
                "alpha",
                "session-a",
                "commands",
                "version",
                payload("missing", "alpha"),
            )
            .unwrap());
    }

    #[test]
    fn late_response_after_cancel_is_unknown() {
        let registry = PendingRegistry::new();

        let receiver = registry
            .register("req-timeout", "alpha", "session-a", "commands", "version")
            .unwrap();

        assert!(registry.cancel("req-timeout").unwrap());

        assert!(!registry
            .resolve_response(
                "req-timeout",
                "alpha",
                "session-a",
                "commands",
                "version",
                payload("req-timeout", "alpha"),
            )
            .unwrap());

        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn duplicate_response_is_unknown_after_first_resolution() {
        let registry = PendingRegistry::new();

        let receiver = registry
            .register("req-duplicate", "alpha", "session-a", "commands", "version")
            .unwrap();

        assert!(registry
            .resolve_response(
                "req-duplicate",
                "alpha",
                "session-a",
                "commands",
                "version",
                payload("req-duplicate", "alpha"),
            )
            .unwrap());

        assert!(receiver.recv().is_ok());

        assert!(!registry
            .resolve_response(
                "req-duplicate",
                "alpha",
                "session-a",
                "commands",
                "version",
                payload("req-duplicate", "alpha"),
            )
            .unwrap());
    }

    #[test]
    fn fail_session_only_removes_exact_runtime_session() {
        let registry = PendingRegistry::new();

        let old_receiver = registry
            .register("req-old", "alpha", "session-old", "commands", "version")
            .unwrap();

        let new_receiver = registry
            .register("req-new", "alpha", "session-new", "commands", "version")
            .unwrap();

        let other_receiver = registry
            .register("req-other", "beta", "session-old", "commands", "version")
            .unwrap();

        let failed = registry
            .fail_session("alpha", "session-old", "connection closed")
            .unwrap();

        assert_eq!(failed, 1);

        let old_message = old_receiver.recv().unwrap();

        match old_message {
            ModuleMessage::Error { error, .. } => {
                assert_eq!(error.kind, "runtime_disconnected");
            }

            other => {
                panic!("expected runtime_disconnected error, got {:?}", other);
            }
        }

        assert!(new_receiver.try_recv().is_err());
        assert!(other_receiver.try_recv().is_err());

        assert!(registry
            .resolve_response(
                "req-new",
                "alpha",
                "session-new",
                "commands",
                "version",
                payload("req-new", "alpha"),
            )
            .unwrap());

        assert!(registry
            .resolve_response(
                "req-other",
                "beta",
                "session-old",
                "commands",
                "version",
                payload("req-other", "beta"),
            )
            .unwrap());

        assert!(new_receiver.recv().is_ok());
        assert!(other_receiver.recv().is_ok());
    }

    #[test]
    fn cancel_only_removes_exact_request() {
        let registry = PendingRegistry::new();

        let cancelled_receiver = registry
            .register("req-cancelled", "alpha", "session-a", "commands", "one")
            .unwrap();

        let surviving_receiver = registry
            .register("req-surviving", "alpha", "session-a", "commands", "two")
            .unwrap();

        assert!(registry.cancel("req-cancelled").unwrap());

        assert!(!registry
            .resolve_response(
                "req-cancelled",
                "alpha",
                "session-a",
                "commands",
                "one",
                payload("req-cancelled", "alpha"),
            )
            .unwrap());

        assert!(registry
            .resolve_response(
                "req-surviving",
                "alpha",
                "session-a",
                "commands",
                "two",
                payload("req-surviving", "alpha"),
            )
            .unwrap());

        assert!(cancelled_receiver.try_recv().is_err());
        assert!(surviving_receiver.recv().is_ok());
    }
}

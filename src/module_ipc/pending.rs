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
                sender,
            },
        );

        Ok(receiver)
    }

    pub fn resolve(&self, id: &str, message: ModuleMessage) -> Result<bool, String> {
        let request = {
            let mut pending = self
                .inner
                .lock()
                .map_err(|_| "pending request registry lock poisoned".to_string())?;

            pending.remove(id)
        };

        let Some(request) = request else {
            return Ok(false);
        };

        request.sender.send(message).map_err(|_| {
            format!(
                "request '{}' receiver disappeared before response delivery",
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

    pub fn len(&self) -> Result<usize, String> {
        let pending = self
            .inner
            .lock()
            .map_err(|_| "pending request registry lock poisoned".to_string())?;

        Ok(pending.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fail_session_wakes_only_matching_runtime_session() {
        let registry = PendingRegistry::new();

        let old_receiver = registry
            .register("req-old", "demo", "session-old")
            .expect("old request should register");

        let new_receiver = registry
            .register("req-new", "demo", "session-new")
            .expect("new request should register");

        let other_receiver = registry
            .register("req-other", "other-module", "session-old")
            .expect("other module request should register");

        let failed = registry
            .fail_session("demo", "session-old", "test disconnect")
            .expect("session failure should succeed");

        assert_eq!(failed, 1);
        assert_eq!(registry.len().expect("len should work"), 2);

        match old_receiver
            .recv()
            .expect("old session should receive failure")
        {
            ModuleMessage::Error { id, module, error } => {
                assert_eq!(id.as_deref(), Some("req-old"));
                assert_eq!(module.as_deref(), Some("demo"));
                assert_eq!(error.kind, "runtime_disconnected");
                assert!(error.message.contains("session-old"));
            }

            message => {
                panic!("expected runtime_disconnected error, got {:?}", message);
            }
        }

        /*
         * The replacement session and another module must
         * remain pending and resolvable normally.
         */
        assert!(registry
            .resolve(
                "req-new",
                ModuleMessage::Pong {
                    module: "demo".to_string(),
                    session_id: "session-new".to_string(),
                },
            )
            .expect("new session resolve should work"));

        assert!(registry
            .resolve(
                "req-other",
                ModuleMessage::Pong {
                    module: "other-module".to_string(),
                    session_id: "session-old".to_string(),
                },
            )
            .expect("other module resolve should work"));

        assert!(matches!(
            new_receiver.recv().expect("new response"),
            ModuleMessage::Pong { .. }
        ));

        assert!(matches!(
            other_receiver.recv().expect("other response"),
            ModuleMessage::Pong { .. }
        ));

        assert_eq!(registry.len().expect("final len should work"), 0);
    }

    #[test]
    fn fail_session_removes_all_matching_requests() {
        let registry = PendingRegistry::new();

        let first = registry
            .register("req-1", "demo", "session-1")
            .expect("first request");

        let second = registry
            .register("req-2", "demo", "session-1")
            .expect("second request");

        let failed = registry
            .fail_session("demo", "session-1", "runtime EOF")
            .expect("session failure");

        assert_eq!(failed, 2);
        assert_eq!(registry.len().expect("len"), 0);

        for receiver in [first, second] {
            match receiver.recv().expect("disconnect error") {
                ModuleMessage::Error { error, .. } => {
                    assert_eq!(error.kind, "runtime_disconnected");
                }

                message => {
                    panic!("expected error, got {:?}", message);
                }
            }
        }
    }
}

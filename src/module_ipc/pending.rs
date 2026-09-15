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
}

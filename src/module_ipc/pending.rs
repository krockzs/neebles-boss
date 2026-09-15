use crate::module_ipc::protocol::ModuleMessage;

use std::collections::HashMap;

use std::sync::{
    mpsc::{self, Receiver, SyncSender},
    Arc, Mutex,
};

#[derive(Debug, Clone, Default)]
pub struct PendingRegistry {
    inner: Arc<Mutex<HashMap<String, SyncSender<ModuleMessage>>>>,
}

impl PendingRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, id: &str) -> Result<Receiver<ModuleMessage>, String> {
        if id.trim().is_empty() {
            return Err("pending request id cannot be empty".to_string());
        }

        let (sender, receiver) = mpsc::sync_channel(1);

        let mut pending = self
            .inner
            .lock()
            .map_err(|_| "pending request registry lock poisoned".to_string())?;

        if pending.contains_key(id) {
            return Err(format!("pending request '{}' already exists", id));
        }

        pending.insert(id.to_string(), sender);

        Ok(receiver)
    }

    pub fn resolve(&self, id: &str, message: ModuleMessage) -> Result<bool, String> {
        let sender = {
            let mut pending = self
                .inner
                .lock()
                .map_err(|_| "pending request registry lock poisoned".to_string())?;

            pending.remove(id)
        };

        let Some(sender) = sender else {
            return Ok(false);
        };

        sender.send(message).map_err(|_| {
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

    pub fn len(&self) -> Result<usize, String> {
        let pending = self
            .inner
            .lock()
            .map_err(|_| "pending request registry lock poisoned".to_string())?;

        Ok(pending.len())
    }
}

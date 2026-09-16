use crate::module_ipc::protocol::{ModuleMessage, ModuleRuntimeState, MODULES_PROTOCOL_VERSION};

use std::collections::{BTreeMap, BTreeSet, HashMap};

use std::sync::{mpsc::Sender, Arc, RwLock};

#[derive(Debug, Clone)]
pub struct ModuleRuntimeRecord {
    pub module: String,
    pub session_id: String,
    pub protocol: u32,
    pub state: ModuleRuntimeState,

    /*
     * contract -> runtime endpoints realmente disponibles.
     */
    pub endpoints: BTreeMap<String, Vec<String>>,

    /*
     * Topics que este runtime pidió recibir desde Boss.
     *
     * "*" significa todos los eventos disponibles.
     */
    pub subscriptions: BTreeSet<String>,

    /*
     * Canal interno Boss -> writer dedicado del runtime.
     *
     * Boss nunca necesita conocer el lenguaje del módulo
     * ni manipular directamente su UnixStream.
     */
    pub writer: Sender<ModuleMessage>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeRegistry {
    inner: Arc<RwLock<HashMap<String, ModuleRuntimeRecord>>>,
}

impl RuntimeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, record: ModuleRuntimeRecord) -> Result<(), String> {
        if record.module.trim().is_empty() {
            return Err("runtime module name cannot be empty".to_string());
        }

        if record.session_id.trim().is_empty() {
            return Err("runtime session_id cannot be empty".to_string());
        }

        if record.protocol != MODULES_PROTOCOL_VERSION {
            return Err(format!(
                "unsupported module runtime protocol {}; Boss supports {}",
                record.protocol, MODULES_PROTOCOL_VERSION
            ));
        }

        let mut registry = self
            .inner
            .write()
            .map_err(|_| "runtime registry write lock poisoned".to_string())?;

        if let Some(existing) = registry.get(&record.module) {
            return Err(format!(
                "module '{}' already has registered session '{}'; new session '{}' was rejected",
                record.module, existing.session_id, record.session_id
            ));
        }

        registry.insert(record.module.clone(), record);

        Ok(())
    }

    pub fn unregister(&self, module: &str, session_id: &str) -> Result<bool, String> {
        let mut registry = self
            .inner
            .write()
            .map_err(|_| "runtime registry write lock poisoned".to_string())?;

        let Some(existing) = registry.get(module) else {
            return Ok(false);
        };

        if existing.session_id != session_id {
            return Ok(false);
        }

        registry.remove(module);

        Ok(true)
    }

    pub fn set_subscriptions(
        &self,
        module: &str,
        session_id: &str,
        topics: BTreeSet<String>,
    ) -> Result<(), String> {
        let mut registry = self
            .inner
            .write()
            .map_err(|_| "runtime registry write lock poisoned".to_string())?;

        let record = registry
            .get_mut(module)
            .ok_or_else(|| format!("module '{}' has no registered runtime", module))?;

        if record.session_id != session_id {
            return Err(format!(
                "module '{}' runtime session mismatch while updating subscriptions",
                module
            ));
        }

        record.subscriptions = topics;

        Ok(())
    }

    pub fn broadcast_event(
        &self,
        topic: &str,
        event: &str,
        payload: serde_json::Value,
    ) -> Result<usize, String> {
        let topic = topic.trim();

        if topic.is_empty() {
            return Err("module event topic cannot be empty".to_string());
        }

        let event = event.trim();

        if event.is_empty() {
            return Err("module event name cannot be empty".to_string());
        }

        let records = self.list()?;

        let mut delivered = 0usize;

        for record in records {
            /*
             * Persistent settings are private to their owning
             * module runtime.
             *
             * A wildcard means every topic this runtime is
             * authorized to receive, never every Boss topic.
             */
            if let Some(settings_owner) = topic.strip_prefix("settings.") {
                if settings_owner != record.module {
                    continue;
                }
            }

            if !record.subscriptions.contains("*") && !record.subscriptions.contains(topic) {
                continue;
            }

            let message = ModuleMessage::Event {
                module: record.module.clone(),
                session_id: record.session_id.clone(),
                topic: topic.to_string(),
                event: event.to_string(),
                payload: payload.clone(),
            };

            match record.writer.send(message) {
                Ok(()) => {
                    delivered += 1;
                }

                Err(error) => {
                    eprintln!(
                        "N.E.E.B.L.E.S.: could not deliver event '{}:{}' to module '{}' session '{}': {}",
                        topic,
                        event,
                        record.module,
                        record.session_id,
                        error
                    );
                }
            }
        }

        Ok(delivered)
    }

    pub fn get(&self, module: &str) -> Result<Option<ModuleRuntimeRecord>, String> {
        let registry = self
            .inner
            .read()
            .map_err(|_| "runtime registry read lock poisoned".to_string())?;

        Ok(registry.get(module).cloned())
    }

    pub fn list(&self) -> Result<Vec<ModuleRuntimeRecord>, String> {
        let registry = self
            .inner
            .read()
            .map_err(|_| "runtime registry read lock poisoned".to_string())?;

        let mut records = registry.values().cloned().collect::<Vec<_>>();

        records.sort_by(|left, right| left.module.cmp(&right.module));

        Ok(records)
    }
}

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ModuleRuntimeSnapshot {
    pub module: String,
    pub session_id: String,
    pub protocol: u32,
    pub state: ModuleRuntimeState,
    pub endpoints: BTreeMap<String, Vec<String>>,
    pub subscriptions: BTreeSet<String>,
}

impl From<ModuleRuntimeRecord> for ModuleRuntimeSnapshot {
    fn from(record: ModuleRuntimeRecord) -> Self {
        Self {
            module: record.module,

            session_id: record.session_id,

            protocol: record.protocol,

            state: record.state,

            endpoints: record.endpoints,

            subscriptions: record.subscriptions,
        }
    }
}

impl RuntimeRegistry {
    pub fn snapshot(&self) -> Result<Vec<ModuleRuntimeSnapshot>, String> {
        Ok(self
            .list()?
            .into_iter()
            .map(ModuleRuntimeSnapshot::from)
            .collect())
    }

    pub fn snapshot_module(&self, module: &str) -> Result<Option<ModuleRuntimeSnapshot>, String> {
        Ok(self.get(module)?.map(ModuleRuntimeSnapshot::from))
    }
}

#[cfg(test)]
mod certification_tests {
    use super::*;
    use std::sync::mpsc;

    fn record(
        module: &str,
        session_id: &str,
        subscriptions: &[&str],
    ) -> (ModuleRuntimeRecord, mpsc::Receiver<ModuleMessage>) {
        let (tx, rx) = mpsc::channel();

        (
            ModuleRuntimeRecord {
                module: module.to_string(),
                session_id: session_id.to_string(),
                protocol: MODULES_PROTOCOL_VERSION,
                state: ModuleRuntimeState::Ready,
                endpoints: BTreeMap::new(),
                subscriptions: subscriptions
                    .iter()
                    .map(|value| value.to_string())
                    .collect(),
                writer: tx,
            },
            rx,
        )
    }

    #[test]
    fn certification_registry_rejects_duplicate_runtime_and_wrong_session() {
        let registry = RuntimeRegistry::new();

        let (first, _rx_first) = record("alpha", "session-a", &[]);
        registry.register(first).unwrap();

        let (duplicate, _rx_duplicate) = record("alpha", "session-b", &[]);
        assert!(registry.register(duplicate).is_err());

        assert!(!registry.unregister("alpha", "wrong-session").unwrap());
        assert!(registry.get("alpha").unwrap().is_some());

        assert!(registry.unregister("alpha", "session-a").unwrap());
        assert!(registry.get("alpha").unwrap().is_none());
    }

    #[test]
    fn certification_registry_settings_are_owner_isolated() {
        let registry = RuntimeRegistry::new();

        let (alpha, alpha_rx) = record("alpha", "session-a", &["*"]);
        let (beta, beta_rx) = record("beta", "session-b", &["*"]);

        registry.register(alpha).unwrap();
        registry.register(beta).unwrap();

        let delivered = registry
            .broadcast_event(
                "settings.alpha",
                "changed",
                serde_json::json!({"path": "ui.enabled"}),
            )
            .unwrap();

        assert_eq!(delivered, 1);

        match alpha_rx
            .try_recv()
            .expect("owner must receive settings event")
        {
            ModuleMessage::Event {
                module,
                topic,
                event,
                ..
            } => {
                assert_eq!(module, "alpha");
                assert_eq!(topic, "settings.alpha");
                assert_eq!(event, "changed");
            }
            other => panic!("unexpected message: {other:?}"),
        }

        assert!(beta_rx.try_recv().is_err());
    }

    #[test]
    fn certification_registry_subscriptions_filter_normal_topics() {
        let registry = RuntimeRegistry::new();

        let (alpha, alpha_rx) = record("alpha", "session-a", &["module.lifecycle"]);

        registry.register(alpha).unwrap();

        assert_eq!(
            registry
                .broadcast_event(
                    "module.lifecycle",
                    "installed",
                    serde_json::json!({"module": "demo"}),
                )
                .unwrap(),
            1
        );

        assert!(alpha_rx.try_recv().is_ok());

        assert_eq!(
            registry
                .broadcast_event("other.topic", "event", serde_json::json!({}),)
                .unwrap(),
            0
        );
    }

    #[test]
    fn certification_registry_subscription_update_requires_same_session() {
        let registry = RuntimeRegistry::new();

        let (alpha, _rx) = record("alpha", "session-a", &[]);
        registry.register(alpha).unwrap();

        let mut topics = BTreeSet::new();
        topics.insert("module.lifecycle".to_string());

        assert!(registry
            .set_subscriptions("alpha", "wrong-session", topics.clone())
            .is_err());

        registry
            .set_subscriptions("alpha", "session-a", topics)
            .unwrap();

        assert!(registry
            .get("alpha")
            .unwrap()
            .unwrap()
            .subscriptions
            .contains("module.lifecycle"));
    }
}

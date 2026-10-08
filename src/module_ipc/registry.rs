use crate::module_ipc::protocol::{ModuleMessage, MODULES_PROTOCOL_VERSION};

use std::collections::{BTreeMap, BTreeSet, HashMap};

use std::sync::{mpsc::Sender, Arc, RwLock};

#[derive(Debug, Clone)]
pub struct ModuleRuntimeRecord {
    pub module: String,
    pub session_id: String,
    pub protocol: u32,

    /*
     * Physical identity authenticated by the kernel through
     * SO_PEERCRED and captured at registration time.
     *
     * start_time_ticks prevents PID reuse from transferring
     * ownership to an unrelated later process.
     */
    pub pid: u32,
    pub start_time_ticks: u64,

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleRuntimeState {
    Closed,
    Opening,
    Open,
}

impl ModuleRuntimeState {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Closed => "closed",
            Self::Opening => "opening",
            Self::Open => "open",
        }
    }
}

#[derive(Debug)]
struct RuntimeOpeningRecord {
    generation: u64,
    execution_id: String,
    owners: BTreeSet<u32>,
    sealed: bool,
}

#[derive(Debug, Default)]
struct RuntimeRegistryInner {
    runtimes: HashMap<String, ModuleRuntimeRecord>,
    openings: HashMap<String, RuntimeOpeningRecord>,
    next_opening_generation: u64,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeRegistry {
    inner: Arc<RwLock<RuntimeRegistryInner>>,
}

impl RuntimeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(
        &self,
        module: &str,
    ) -> Result<ModuleRuntimeState, String> {
        let module = module.trim();

        if module.is_empty() {
            return Err(
                "runtime state module cannot be empty"
                    .to_string()
            );
        }

        let registry = self
            .inner
            .read()
            .map_err(|_| {
                "runtime registry read lock poisoned"
                    .to_string()
            })?;

        if registry.runtimes.contains_key(module) {
            return Ok(ModuleRuntimeState::Open);
        }

        if registry.openings.contains_key(module) {
            return Ok(ModuleRuntimeState::Opening);
        }

        Ok(ModuleRuntimeState::Closed)
    }

    pub fn begin_opening(
        &self,
        module: &str,
    ) -> Result<(u64, String), String> {
        let module = module.trim();

        if module.is_empty() {
            return Err(
                "runtime opening module cannot be empty"
                    .to_string()
            );
        }

        let mut registry = self
            .inner
            .write()
            .map_err(|_| {
                "runtime registry write lock poisoned"
                    .to_string()
            })?;

        if registry.runtimes.contains_key(module) {
            return Err(format!(
                "module {} already has an open runtime",
                module
            ));
        }

        if registry.openings.contains_key(module) {
            return Err(format!(
                "module {} is already opening",
                module
            ));
        }

        let generation = registry
            .next_opening_generation
            .checked_add(1)
            .ok_or_else(|| {
                "runtime opening generation overflow"
                    .to_string()
            })?;

        registry.next_opening_generation =
            generation;

        let execution_id =
            format!("module.open.{generation}");

        registry.openings.insert(
            module.to_string(),
            RuntimeOpeningRecord {
                generation,
                execution_id: execution_id.clone(),
                owners: BTreeSet::new(),
                sealed: false,
            },
        );

        Ok((generation, execution_id))
    }

    pub fn claim_opening_owner(
        &self,
        module: &str,
        execution_id: &str,
        pid: u32,
    ) -> Result<Option<u64>, String> {
        let module = module.trim();

        if module.is_empty() {
            return Err(
                "runtime opening owner module cannot be empty"
                    .to_string()
            );
        }

        if execution_id.trim().is_empty() {
            return Err(
                "runtime opening owner execution id cannot be empty"
                    .to_string()
            );
        }
        if pid == 0 {
            return Err(
                "runtime opening owner pid cannot be zero"
                    .to_string()
            );
        }

        let mut registry = self
            .inner
            .write()
            .map_err(|_| {
                "runtime registry write lock poisoned"
                    .to_string()
            })?;

        if registry.runtimes.contains_key(module) {
            return Ok(None);
        }

        let Some(opening) =
            registry.openings.get_mut(module)
        else {
            return Ok(None);
        };

        if opening.execution_id != execution_id {
            return Err(format!(
                "module {} opening generation {} belongs to Governor execution {} and cannot be claimed by {}",
                module,
                opening.generation,
                opening.execution_id,
                execution_id
            ));
        }
        if opening.sealed {
            return Ok(None);
        }

        opening.owners.insert(pid);

        Ok(Some(opening.generation))
    }

    pub fn seal_opening(
        &self,
        module: &str,
        generation: u64,
    ) -> Result<bool, String> {
        let module = module.trim();

        if module.is_empty() {
            return Err(
                "runtime opening module cannot be empty"
                    .to_string()
            );
        }

        let mut registry = self
            .inner
            .write()
            .map_err(|_| {
                "runtime registry write lock poisoned"
                    .to_string()
            })?;

        if registry.runtimes.contains_key(module) {
            return Ok(true);
        }

        let keep_opening = {
            let Some(opening) =
                registry.openings.get_mut(module)
            else {
                return Ok(false);
            };

            if opening.generation != generation {
                return Err(format!(
                    "module {} opening generation mismatch: expected={} actual={}",
                    module,
                    generation,
                    opening.generation
                ));
            }

            opening.sealed = true;

            !opening.owners.is_empty()
        };

        if !keep_opening {
            registry.openings.remove(module);
        }

        Ok(keep_opening)
    }

    pub fn release_opening_owner(
        &self,
        module: &str,
        generation: u64,
        pid: u32,
    ) -> Result<bool, String> {
        let module = module.trim();

        if module.is_empty() {
            return Err(
                "runtime opening owner module cannot be empty"
                    .to_string()
            );
        }

        let mut registry = self
            .inner
            .write()
            .map_err(|_| {
                "runtime registry write lock poisoned"
                    .to_string()
            })?;

        if registry.runtimes.contains_key(module) {
            return Ok(false);
        }

        let should_close = {
            let Some(opening) =
                registry.openings.get_mut(module)
            else {
                return Ok(false);
            };

            if opening.generation != generation {
                return Ok(false);
            }

            if !opening.owners.remove(&pid) {
                return Ok(false);
            }

            opening.sealed
                && opening.owners.is_empty()
        };

        if should_close {
            registry.openings.remove(module);
            return Ok(true);
        }

        Ok(false)
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

        if let Some(existing) = registry.runtimes.get(&record.module) {
            return Err(format!(
                "module '{}' already has registered session '{}'; new session '{}' was rejected",
                record.module, existing.session_id, record.session_id
            ));
        }

        let module = record.module.clone();

        registry.runtimes.insert(
            module.clone(),
            record,
        );

        registry.openings.remove(&module);

        Ok(())
    }

    pub fn unregister(&self, module: &str, session_id: &str) -> Result<bool, String> {
        let mut registry = self
            .inner
            .write()
            .map_err(|_| "runtime registry write lock poisoned".to_string())?;

        let Some(existing) = registry.runtimes.get(module) else {
            return Ok(false);
        };

        if existing.session_id != session_id {
            return Ok(false);
        }

        registry.runtimes.remove(module);
        registry.openings.remove(module);

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
            .runtimes
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

    pub fn send_to_session(
        &self,
        module: &str,
        session_id: &str,
        message: ModuleMessage,
    ) -> Result<(), String> {
        let runtime = self
            .get(module)?
            .ok_or_else(|| format!("module '{}' has no registered runtime", module))?;

        if runtime.session_id != session_id {
            return Err(format!(
                "module '{}' runtime session mismatch: notification owner session '{}' but current runtime session is '{}'",
                module,
                session_id,
                runtime.session_id
            ));
        }

        runtime.writer.send(message).map_err(|error| {
            format!(
                "could not deliver targeted message to module '{}' session '{}': {}",
                module, session_id, error
            )
        })
    }

    pub fn get(&self, module: &str) -> Result<Option<ModuleRuntimeRecord>, String> {
        let registry = self
            .inner
            .read()
            .map_err(|_| "runtime registry read lock poisoned".to_string())?;

        Ok(registry.runtimes.get(module).cloned())
    }

    pub fn list(&self) -> Result<Vec<ModuleRuntimeRecord>, String> {
        let registry = self
            .inner
            .read()
            .map_err(|_| "runtime registry read lock poisoned".to_string())?;

        let mut records = registry
            .runtimes
            .values()
            .cloned()
            .collect::<Vec<_>>();

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
    pub endpoints: BTreeMap<String, Vec<String>>,
    pub subscriptions: BTreeSet<String>,
}

impl From<ModuleRuntimeRecord> for ModuleRuntimeSnapshot {
    fn from(record: ModuleRuntimeRecord) -> Self {
        Self {
            module: record.module,

            session_id: record.session_id,

            protocol: record.protocol,

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

    #[test]
    fn canonical_opening_is_single_flight() {
        let registry = RuntimeRegistry::new();

        assert_eq!(
            registry.state("alpha").unwrap(),
            ModuleRuntimeState::Closed
        );

        let (generation, _execution_id) =
            registry.begin_opening("alpha").unwrap();

        assert_eq!(
            registry.state("alpha").unwrap(),
            ModuleRuntimeState::Opening
        );

        let error = registry
            .begin_opening("alpha")
            .expect_err(
                "second opening must fail"
            );

        assert!(error.contains("already opening"));

        assert!(
            !registry
                .seal_opening(
                    "alpha",
                    generation,
                )
                .unwrap()
        );

        assert_eq!(
            registry.state("alpha").unwrap(),
            ModuleRuntimeState::Closed
        );
    }

    #[test]
    fn runtime_registration_consumes_opening_state() {
        let registry = RuntimeRegistry::new();

        let (generation, _execution_id) =
            registry.begin_opening("alpha").unwrap();

        let (runtime, _receiver) =
            record("alpha", "session-a", &[]);

        registry.register(runtime).unwrap();

        assert_eq!(
            registry.state("alpha").unwrap(),
            ModuleRuntimeState::Open
        );

        assert!(
            registry
                .seal_opening(
                    "alpha",
                    generation,
                )
                .unwrap()
        );
    }

    #[test]
    fn opening_owners_are_generation_bound_and_multi_process_safe() {
        let registry = RuntimeRegistry::new();

        let (first, first_execution_id) =
            registry.begin_opening("alpha").unwrap();

        assert_eq!(
            registry
                .claim_opening_owner("alpha", &first_execution_id, 1001)
                .unwrap(),
            Some(first)
        );

        assert_eq!(
            registry
                .claim_opening_owner("alpha", &first_execution_id, 1002)
                .unwrap(),
            Some(first)
        );

        assert!(
            registry
                .seal_opening("alpha", first)
                .unwrap()
        );

        assert!(
            !registry
                .release_opening_owner(
                    "alpha",
                    first,
                    1001,
                )
                .unwrap()
        );

        assert_eq!(
            registry.state("alpha").unwrap(),
            ModuleRuntimeState::Opening
        );

        assert!(
            registry
                .release_opening_owner(
                    "alpha",
                    first,
                    1002,
                )
                .unwrap()
        );

        assert_eq!(
            registry.state("alpha").unwrap(),
            ModuleRuntimeState::Closed
        );

        let (second, _second_execution_id) =
            registry.begin_opening("alpha").unwrap();

        assert_ne!(first, second);

        assert!(
            !registry
                .release_opening_owner(
                    "alpha",
                    first,
                    1002,
                )
                .unwrap()
        );

        assert_eq!(
            registry.state("alpha").unwrap(),
            ModuleRuntimeState::Opening
        );

        assert!(
            !registry
                .seal_opening("alpha", second)
                .unwrap()
        );
    }
    #[test]
    fn foreign_governor_execution_cannot_claim_opening_owner() {
        let registry = RuntimeRegistry::new();

        let (generation, execution_id) =
            registry.begin_opening("alpha").unwrap();

        let error = registry
            .claim_opening_owner(
                "alpha",
                "module.notify-demo",
                1001,
            )
            .expect_err(
                "foreign Governor execution must fail"
            );

        assert!(error.contains("Governor execution"));

        assert_eq!(
            registry.state("alpha").unwrap(),
            ModuleRuntimeState::Opening
        );

        assert_eq!(
            registry
                .claim_opening_owner(
                    "alpha",
                    &execution_id,
                    1001,
                )
                .unwrap(),
            Some(generation)
        );

        assert!(
            registry
                .seal_opening("alpha", generation)
                .unwrap()
        );

        assert!(
            registry
                .release_opening_owner(
                    "alpha",
                    generation,
                    1001,
                )
                .unwrap()
        );

        assert_eq!(
            registry.state("alpha").unwrap(),
            ModuleRuntimeState::Closed
        );
    }
    #[test]
    fn targeted_delivery_reaches_only_exact_runtime_session() {
        let registry = RuntimeRegistry::new();

        let (runtime, receiver) = record("alpha", "session-a", &[]);

        registry.register(runtime).unwrap();

        registry
            .send_to_session(
                "alpha",
                "session-a",
                ModuleMessage::NotificationActionInvoked {
                    module: "alpha".to_string(),
                    session_id: "session-a".to_string(),
                    notification_id: 77,
                    action_key: "open".to_string(),
                },
            )
            .unwrap();

        match receiver.recv().unwrap() {
            ModuleMessage::NotificationActionInvoked {
                module,
                session_id,
                notification_id,
                action_key,
            } => {
                assert_eq!(module, "alpha");
                assert_eq!(session_id, "session-a");
                assert_eq!(notification_id, 77);
                assert_eq!(action_key, "open");
            }

            other => panic!("unexpected targeted delivery: {other:?}"),
        }
    }

    #[test]
    fn targeted_delivery_rejects_stale_runtime_session() {
        let registry = RuntimeRegistry::new();

        let (runtime, _receiver) = record("alpha", "session-current", &[]);

        registry.register(runtime).unwrap();

        let error = registry
            .send_to_session(
                "alpha",
                "session-old",
                ModuleMessage::NotificationClosed {
                    module: "alpha".to_string(),
                    session_id: "session-old".to_string(),
                    notification_id: 88,
                    reason: 2,
                },
            )
            .expect_err("stale session must never receive notification events");

        assert!(error.contains("session mismatch"));
    }

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
                pid: 4242,
                start_time_ticks: 1,
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

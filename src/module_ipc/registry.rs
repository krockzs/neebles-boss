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

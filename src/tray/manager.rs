use crate::config;
use crate::modules;
use crate::tray::protocol::{TrayRecord, TRAY_PROTOCOL_VERSION};

use serde_json::Value;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct TrayManager {
    trays: BTreeMap<String, TrayRecord>,
}

static MANAGER: OnceLock<Arc<Mutex<TrayManager>>> = OnceLock::new();

pub fn global() -> Arc<Mutex<TrayManager>> {
    MANAGER
        .get_or_init(|| Arc::new(Mutex::new(TrayManager::default())))
        .clone()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub fn valid_tray_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

impl TrayManager {
    pub fn register(
        &mut self,
        protocol: u32,
        tray_id: String,
        owner_module: String,
        pid: Option<u32>,
    ) -> Result<TrayRecord, String> {
        if protocol != TRAY_PROTOCOL_VERSION {
            return Err(format!(
                "unsupported N.E.E.B.L.E.S. tray protocol {protocol}; expected {TRAY_PROTOCOL_VERSION}"
            ));
        }

        if !valid_tray_id(&tray_id) {
            return Err(format!("invalid N.E.E.B.L.E.S. tray id: {tray_id}"));
        }

        if owner_module.trim().is_empty() {
            return Err("tray owner_module cannot be empty".to_string());
        }

        if tray_id != owner_module {
            return Err(format!(
                "tray id '{tray_id}' does not match owner module '{owner_module}'"
            ));
        }

        if !config::module_enabled(&owner_module)? {
            return Err(format!(
                "module '{}' is disabled and cannot register a tray",
                owner_module
            ));
        }

        let contract = modules::resolved_tray_contract(&owner_module)?;

        if contract.protocol != protocol {
            return Err(format!(
                "tray protocol mismatch for module '{}': manifest declares {}, provider requested {}",
                owner_module,
                contract.protocol,
                protocol
            ));
        }

        if let Some(existing) = self.trays.get(&tray_id) {
            if existing.owner_module != owner_module {
                return Err(format!(
                    "tray '{tray_id}' is already owned by module '{}'",
                    existing.owner_module
                ));
            }
        }

        let visible = config::module_visible("tray", &owner_module)?;

        let record = TrayRecord {
            tray_id: tray_id.clone(),
            owner_module,
            module_version: contract.module_version,
            icon: contract.icon,
            provider: contract.provider,
            protocol,
            pid,
            visible,
            opened: false,
            width: None,
            height: None,
            state: None,
            last_heartbeat_ms: now_ms(),
        };

        self.trays.insert(tray_id, record.clone());

        Ok(record)
    }

    pub fn unregister(&mut self, tray_id: &str) -> Result<(), String> {
        if self.trays.remove(tray_id).is_none() {
            return Err(format!("tray '{tray_id}' is not registered"));
        }

        Ok(())
    }

    pub fn heartbeat(&mut self, tray_id: &str) -> Result<(), String> {
        let tray = self
            .trays
            .get_mut(tray_id)
            .ok_or_else(|| format!("tray '{tray_id}' is not registered"))?;

        tray.last_heartbeat_ms = now_ms();

        Ok(())
    }

    pub fn update_state(
        &mut self,
        tray_id: &str,
        opened: Option<bool>,
        width: Option<u32>,
        height: Option<u32>,
        state: Option<Value>,
    ) -> Result<TrayRecord, String> {
        let tray = self
            .trays
            .get_mut(tray_id)
            .ok_or_else(|| format!("tray '{tray_id}' is not registered"))?;

        if let Some(value) = opened {
            tray.opened = value;
        }

        if let Some(value) = width {
            tray.width = Some(value);
        }

        if let Some(value) = height {
            tray.height = Some(value);
        }

        if let Some(value) = state {
            tray.state = Some(value);
        }

        tray.last_heartbeat_ms = now_ms();

        Ok(tray.clone())
    }

    pub fn set_visibility(&mut self, tray_id: &str, visible: bool) -> Option<TrayRecord> {
        let tray = self.trays.get_mut(tray_id)?;

        tray.visible = visible;

        Some(tray.clone())
    }

    pub fn remove_stale(&mut self, timeout_ms: u64) -> Vec<String> {
        let now = now_ms();

        let stale = self
            .trays
            .iter()
            .filter_map(|(tray_id, tray)| {
                let age = now.saturating_sub(tray.last_heartbeat_ms);

                if age > timeout_ms {
                    Some(tray_id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for tray_id in &stale {
            self.trays.remove(tray_id);
        }

        stale
    }

    pub fn list(&self) -> Vec<TrayRecord> {
        self.trays.values().cloned().collect()
    }

    pub fn get(&self, tray_id: &str) -> Option<TrayRecord> {
        self.trays.get(tray_id).cloned()
    }
}

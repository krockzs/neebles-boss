use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const CONTRACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Oneshot,
    Runtime,
    Tracked,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::Oneshot
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StateMode {
    Preserve,
    Clean,
    New,
}

impl Default for StateMode {
    fn default() -> Self {
        Self::Preserve
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractReference {
    #[serde(rename = "type")]
    pub contract_type: String,

    pub file: String,

    #[serde(default = "default_contract_schema")]
    pub schema: u32,

    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_contract_schema() -> u32 {
    CONTRACT_SCHEMA_VERSION
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractEndpoint {
    pub endpoint: String,

    #[serde(default)]
    pub lifecycle: Lifecycle,

    #[serde(default)]
    pub state_mode: StateMode,

    #[serde(default)]
    pub requires_root: bool,

    #[serde(default)]
    pub launcher: bool,

    /*
     * Espacio extensible.
     *
     * Un contrato puede agregar metadata sin obligar a Boss
     * a recompilarse por cada propiedad nueva que no tenga
     * semántica propia dentro del motor.
     */
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDefinition {
    #[serde(default = "default_contract_schema")]
    pub schema: u32,

    pub contract: String,

    #[serde(default)]
    pub endpoints: BTreeMap<String, ContractEndpoint>,

    /*
     * Metadata general del contrato.
     *
     * Boss la conserva pero no necesita comprenderla.
     */
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default)]
pub struct ModuleContracts {
    pub contracts: BTreeMap<String, ContractDefinition>,
}

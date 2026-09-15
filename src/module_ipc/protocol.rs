use crate::contracts::{Lifecycle, StateMode};

use serde::{Deserialize, Serialize};

use serde_json::Value;

use std::collections::BTreeMap;

pub const MODULES_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModuleRuntimeState {
    Starting,
    Ready,
    Stopping,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleError {
    pub kind: String,
    pub message: String,

    #[serde(default)]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModuleMessage {
    Register {
        protocol: u32,
        module: String,
        session_id: String,

        /*
         * Endpoints que el runtime declara haber cargado
         * realmente.
         *
         * Ej:
         * commands -> ["gradient.create", "version"]
         */
        #[serde(default)]
        endpoints: BTreeMap<String, Vec<String>>,
    },

    Registered {
        protocol: u32,
        module: String,
        session_id: String,
    },

    Invoke {
        id: String,
        module: String,
        session_id: String,

        contract: String,
        endpoint: String,

        lifecycle: Lifecycle,
        state_mode: StateMode,

        #[serde(default)]
        args: Vec<String>,

        #[serde(default)]
        payload: Option<Value>,

        #[serde(default)]
        context: BTreeMap<String, Value>,
    },

    Response {
        id: String,
        module: String,
        session_id: String,

        contract: String,
        endpoint: String,

        ok: bool,
        code: i32,

        #[serde(default)]
        result: Option<Value>,

        #[serde(default)]
        error: Option<ModuleError>,
    },

    Shutdown {
        module: String,
        session_id: String,

        #[serde(default)]
        reason: Option<String>,
    },

    ShutdownAck {
        module: String,
        session_id: String,
    },

    Unregister {
        module: String,
        session_id: String,

        #[serde(default)]
        reason: Option<String>,
    },

    Ping {
        module: String,
        session_id: String,
    },

    Pong {
        module: String,
        session_id: String,
    },

    Error {
        #[serde(default)]
        id: Option<String>,

        #[serde(default)]
        module: Option<String>,

        error: ModuleError,
    },
}

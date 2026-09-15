use serde::{Deserialize, Serialize};
use serde_json::Value;

use std::env;
use std::path::PathBuf;

pub const TRAY_PROTOCOL_VERSION: u32 = 1;

pub const TRAY_HEARTBEAT_TIMEOUT_MS: u64 = 15_000;
pub const TRAY_WATCHDOG_INTERVAL_MS: u64 = 2_000;

pub fn socket_path() -> PathBuf {
    if let Ok(value) = env::var("NEEBLES_TRAY_SOCKET") {
        let value = value.trim();

        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }

    let uid = unsafe { libc::geteuid() };

    PathBuf::from(format!("/run/user/{uid}/neebles/tray.sock"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayRecord {
    pub tray_id: String,
    pub owner_module: String,

    #[serde(default)]
    pub module_version: String,

    pub icon: String,
    pub provider: String,
    pub protocol: u32,

    #[serde(default)]
    pub pid: Option<u32>,

    #[serde(default)]
    pub visible: bool,

    #[serde(default)]
    pub opened: bool,

    #[serde(default)]
    pub width: Option<u32>,

    #[serde(default)]
    pub height: Option<u32>,

    #[serde(default)]
    pub state: Option<Value>,

    pub last_heartbeat_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TrayEvent {
    Registered { tray: TrayRecord },

    Updated { tray: TrayRecord },

    Unregistered { tray_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TrayMessage {
    Register {
        protocol: u32,
        tray_id: String,
        owner_module: String,

        #[serde(default)]
        pid: Option<u32>,
    },

    Unregister {
        tray_id: String,
    },

    Heartbeat {
        tray_id: String,
    },

    State {
        tray_id: String,

        #[serde(default)]
        opened: Option<bool>,

        #[serde(default)]
        width: Option<u32>,

        #[serde(default)]
        height: Option<u32>,

        #[serde(default)]
        state: Option<Value>,
    },

    Open {
        tray_id: String,
    },

    Close {
        tray_id: String,
    },

    Focus {
        tray_id: String,
    },

    Resize {
        tray_id: String,
        width: u32,
        height: u32,
    },

    Reload {
        tray_id: String,
    },

    SetVisibility {
        tray_id: String,
        visible: bool,
    },

    SettingsGet {
        owner_module: String,
        path: String,
    },

    SettingsSet {
        owner_module: String,
        path: String,
        value: Value,
    },

    StopProvider {
        tray_id: String,
    },

    Reconcile,

    /*
     * Persistent subscription used by the N.E.E.B.L.E.S.
     * Tray Host.
     *
     * The first response is always Snapshot.
     * Afterwards the same connection receives Event messages.
     */
    Subscribe,

    List,

    Get {
        tray_id: String,
    },

    Ack {
        event: String,

        #[serde(default)]
        tray_id: Option<String>,
    },

    Snapshot {
        trays: Vec<TrayRecord>,
    },

    Event {
        event: TrayEvent,
    },

    Record {
        tray: TrayRecord,
    },

    SettingsValue {
        owner_module: String,
        path: String,
        value: Value,
    },

    Error {
        message: String,
    },
}

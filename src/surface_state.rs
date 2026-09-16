use crate::ipc;

use serde_json::json;

use std::sync::{
    OnceLock,
    RwLock,
};

use zbus::{
    blocking::{
        fdo::DBusProxy,
        Connection,
    },
    names::BusName,
};

const BOSS_UI_SERVICE: &str =
    "org.neebles.Boss";

const BOSS_UI_TOPIC: &str =
    "boss-ui";

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
pub enum BossUiState {
    Closed,
    Opening,
    Open,
}

impl BossUiState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Opening => "opening",
            Self::Open => "open",
        }
    }
}

static BOSS_UI_STATE:
    OnceLock<RwLock<BossUiState>> =
    OnceLock::new();

fn state_lock()
    -> &'static RwLock<BossUiState>
{
    BOSS_UI_STATE.get_or_init(|| {
        RwLock::new(
            BossUiState::Closed
        )
    })
}

pub fn boss_ui_state() -> BossUiState {
    state_lock()
        .read()
        .map(|guard| *guard)
        .unwrap_or(BossUiState::Closed)
}

pub fn set_boss_ui_state(
    state: BossUiState,
) {
    let changed = match state_lock().write() {
        Ok(mut guard) => {
            if *guard == state {
                false
            } else {
                *guard = state;
                true
            }
        }

        Err(_) => {
            eprintln!(
                "N.E.E.B.L.E.S.: Boss UI state lock poisoned"
            );

            return;
        }
    };

    if !changed {
        return;
    }

    ipc::broadcast_event(
        BOSS_UI_TOPIC,
        "state_changed",
        json!({
            "state": state.as_str()
        }),
    );
}

pub fn start_background()
    -> Result<std::thread::JoinHandle<()>, String>
{
    let connection =
        Connection::session()
            .map_err(|error| {
                format!(
                    "could not connect Boss surface watcher to session D-Bus: {error}"
                )
            })?;

    let proxy =
        DBusProxy::new(&connection)
            .map_err(|error| {
                format!(
                    "could not create D-Bus proxy for Boss surface watcher: {error}"
                )
            })?;

    let boss_name =
        BusName::try_from(
            BOSS_UI_SERVICE
        )
        .map_err(|error| {
            format!(
                "invalid Boss UI D-Bus service name: {error}"
            )
        })?;

    let open =
        proxy
            .name_has_owner(
                boss_name.clone()
            )
            .map_err(|error| {
                format!(
                    "could not query Boss UI D-Bus ownership: {error}"
                )
            })?;

    set_boss_ui_state(
        if open {
            BossUiState::Open
        } else {
            BossUiState::Closed
        },
    );

    let signals =
        proxy
            .receive_name_owner_changed_with_args(
                &[
                    (
                        0,
                        BOSS_UI_SERVICE,
                    )
                ]
            )
            .map_err(|error| {
                format!(
                    "could not subscribe to Boss UI D-Bus ownership changes: {error}"
                )
            })?;

    Ok(std::thread::spawn(move || {
        /*
         * Keep the D-Bus connection and proxy alive
         * inside this worker for the complete lifetime
         * of the watcher.
         */
        let _connection = connection;
        let _proxy = proxy;

        for signal in signals {
            let args = match signal.args() {
                Ok(args) => args,

                Err(error) => {
                    eprintln!(
                        "N.E.E.B.L.E.S.: invalid Boss UI ownership event: {error}"
                    );

                    continue;
                }
            };

            let open =
                args
                    .new_owner()
                    .as_ref()
                    .is_some();

            set_boss_ui_state(
                if open {
                    BossUiState::Open
                } else {
                    BossUiState::Closed
                },
            );
        }
    }))
}

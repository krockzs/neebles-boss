use crate::ipc;

use serde_json::json;
use std::sync::{Mutex, OnceLock, RwLock};
use std::thread;
use std::time::Duration;

const BOSS_UI_TOPIC: &str = "boss-ui";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

static BOSS_UI_STATE: OnceLock<RwLock<BossUiState>> = OnceLock::new();
static BOSS_UI_LEASES: OnceLock<Mutex<usize>> = OnceLock::new();
static BOSS_UI_OPENING_GENERATION: OnceLock<Mutex<u64>> = OnceLock::new();

const BOSS_UI_OPENING_TIMEOUT: Duration = Duration::from_secs(10);

fn opening_generation_lock() -> &'static Mutex<u64> {
    BOSS_UI_OPENING_GENERATION.get_or_init(|| Mutex::new(0))
}

fn lease_lock() -> &'static Mutex<usize> {
    BOSS_UI_LEASES.get_or_init(|| Mutex::new(0))
}

fn state_lock() -> &'static RwLock<BossUiState> {
    BOSS_UI_STATE.get_or_init(|| RwLock::new(BossUiState::Closed))
}

pub fn boss_ui_state() -> BossUiState {
    state_lock()
        .read()
        .map(|guard| *guard)
        .unwrap_or(BossUiState::Closed)
}

pub fn set_boss_ui_state(state: BossUiState) {
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
            eprintln!("N.E.E.B.L.E.S.: Boss UI state lock poisoned");
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

pub fn begin_boss_ui_opening() -> Result<u64, String> {
    let generation = {
        let mut guard = opening_generation_lock()
            .lock()
            .map_err(|_| "Boss UI opening generation lock poisoned".to_string())?;

        *guard = guard.wrapping_add(1);

        if *guard == 0 {
            *guard = 1;
        }

        *guard
    };

    set_boss_ui_state(BossUiState::Opening);

    Ok(generation)
}

pub fn cancel_boss_ui_opening(generation: u64) {
    let is_current = opening_generation_lock()
        .lock()
        .map(|guard| *guard == generation)
        .unwrap_or(false);

    if is_current && boss_ui_state() == BossUiState::Opening {
        set_boss_ui_state(BossUiState::Closed);
    }
}

pub fn watch_boss_ui_opening(generation: u64) {
    thread::spawn(move || {
        thread::sleep(BOSS_UI_OPENING_TIMEOUT);
        cancel_boss_ui_opening(generation);
    });
}

pub fn acquire_boss_ui_lease() -> Result<(), String> {
    let should_open = {
        let mut guard = lease_lock()
            .lock()
            .map_err(|_| "Boss UI lease lock poisoned".to_string())?;

        *guard = guard
            .checked_add(1)
            .ok_or_else(|| "Boss UI lease count overflow".to_string())?;

        *guard == 1
    };

    if should_open {
        set_boss_ui_state(BossUiState::Open);
    }

    Ok(())
}

pub fn release_boss_ui_lease() -> Result<(), String> {
    let should_close = {
        let mut guard = lease_lock()
            .lock()
            .map_err(|_| "Boss UI lease lock poisoned".to_string())?;

        if *guard == 0 {
            return Err("Boss UI lease count underflow".to_string());
        }

        *guard -= 1;
        *guard == 0
    };

    if should_close {
        set_boss_ui_state(BossUiState::Closed);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset_surface_state() {
        if let Ok(mut guard) = lease_lock().lock() {
            *guard = 0;
        }

        if let Ok(mut guard) = state_lock().write() {
            *guard = BossUiState::Closed;
        }

        if let Ok(mut guard) = opening_generation_lock().lock() {
            *guard = 0;
        }
    }

    #[test]
    fn boss_ui_stays_open_until_last_lease_is_released() {
        let _test_guard = TEST_LOCK.lock().unwrap();
        reset_surface_state();

        acquire_boss_ui_lease().unwrap();
        acquire_boss_ui_lease().unwrap();

        assert_eq!(boss_ui_state(), BossUiState::Open);

        release_boss_ui_lease().unwrap();

        assert_eq!(boss_ui_state(), BossUiState::Open);

        release_boss_ui_lease().unwrap();

        assert_eq!(boss_ui_state(), BossUiState::Closed);
    }

    #[test]
    fn stale_opening_generation_cannot_close_newer_opening() {
        let _test_guard = TEST_LOCK.lock().unwrap();
        reset_surface_state();

        let first = begin_boss_ui_opening().unwrap();
        let second = begin_boss_ui_opening().unwrap();

        assert_ne!(first, second);
        assert_eq!(boss_ui_state(), BossUiState::Opening);

        cancel_boss_ui_opening(first);

        assert_eq!(boss_ui_state(), BossUiState::Opening);

        cancel_boss_ui_opening(second);

        assert_eq!(boss_ui_state(), BossUiState::Closed);
    }

    #[test]
    fn opening_timeout_cannot_close_acquired_lease() {
        let _test_guard = TEST_LOCK.lock().unwrap();
        reset_surface_state();

        let generation = begin_boss_ui_opening().unwrap();

        acquire_boss_ui_lease().unwrap();

        assert_eq!(boss_ui_state(), BossUiState::Open);

        cancel_boss_ui_opening(generation);

        assert_eq!(boss_ui_state(), BossUiState::Open);

        release_boss_ui_lease().unwrap();

        assert_eq!(boss_ui_state(), BossUiState::Closed);
    }

    #[test]
    fn boss_ui_lease_release_rejects_underflow() {
        let _test_guard = TEST_LOCK.lock().unwrap();
        reset_surface_state();

        assert!(release_boss_ui_lease().is_err());
        assert_eq!(boss_ui_state(), BossUiState::Closed);
    }
}

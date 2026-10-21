/*
 * NEEBLES Boss 2.0.2 — Live opaque variables owned by authenticated modules.
 *
 * Separate from RuntimeRegistry: a service may outlive the UI's Open session.
 * Values are observations, NEVER Registry authority. Registry declares expected
 * values; modules publish their actual values; Boss compares literal strings.
 *
 * SECURITY: callers must authenticate module, PID, process start time and
 * service/session ownership BEFORE invoking publish(). The data types below
 * do not perform operating-system peer authentication. No production IPC
 * handler is connected in F03A; that is a later, separately audited change.
 *
 * Publication scope is a single live connection. It is atomically replaced
 * on a new snapshot and revoked on that connection's verified teardown.
 */

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublisherIdentity {
    pub module: String,
    pub session_id: String,
    pub pid: u32,
    pub start_time_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Publication {
    owner: PublisherIdentity,
    variables: BTreeMap<String, String>,
}

type PublisherSet = BTreeMap<String, Publication>;
type PublishedState = BTreeMap<String, PublisherSet>;

#[derive(Debug, Default)]
pub struct LiveModuleVariables {
    inner: Mutex<PublishedState>,
}

static LIVE_MODULE_VARIABLES: OnceLock<LiveModuleVariables> = OnceLock::new();

pub fn live_module_variables() -> &'static LiveModuleVariables {
    LIVE_MODULE_VARIABLES.get_or_init(LiveModuleVariables::default)
}

fn validate_identity(identity: &PublisherIdentity) -> Result<(), String> {
    if !neebles_backend::module_material::valid_module_id(&identity.module)
        || identity.session_id.is_empty()
        || identity.session_id.len() > 256
        || identity.session_id.chars().any(char::is_control)
        || identity.pid == 0
        || identity.start_time_ticks == 0
    {
        return Err("invalid module variable publisher identity".to_string());
    }
    Ok(())
}

fn validate_variables(variables: &BTreeMap<String, String>) -> Result<(), String> {
    if variables.len() > 128 {
        return Err("module variable publication exceeds 128 entries".to_string());
    }
    for (key, value) in variables {
        if key.is_empty()
            || key.len() > 512
            || key.chars().any(char::is_control)
            || value.is_empty()
            || value.len() > 2048
            || value.chars().any(char::is_control)
        {
            return Err("invalid opaque module variable key or value".to_string());
        }
    }
    Ok(())
}

fn merge_publisher_set(publishers: &PublisherSet) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for publication in publishers.values() {
        for (key, value) in &publication.variables {
            // Two services of a module may publish distinct keys, but a key
            // must never have two competing owners, even for equal values.
            if result.insert(key.clone(), value.clone()).is_some() {
                return Err("duplicate module variable published by separate owners".to_string());
            }
        }
    }
    Ok(result)
}

impl LiveModuleVariables {
    pub fn new() -> Self {
        Self::default()
    }

    /*
     * Called ONLY after Boss verified this exact connection's OS identity.
     * Returns true only when the visible effective variables changed.
     * An empty snapshot clears the previously published values for this owner.
     */
    pub fn publish(
        &self,
        owner: PublisherIdentity,
        values: BTreeMap<String, String>,
    ) -> Result<bool, String> {
        validate_identity(&owner)?;
        validate_variables(&values)?;
        let mut state = self.inner.lock()
            .map_err(|_| "module variable registry lock poisoned".to_string())?;
        let current = state.get(&owner.module).cloned().unwrap_or_default();
        let before = merge_publisher_set(&current)?;
        let mut candidate = current;
        if let Some(previous) = candidate.get(&owner.session_id) {
            if previous.owner != owner {
                return Err("module variable publication session belongs to a different process incarnation".to_string());
            }
        }
        if values.is_empty() {
            candidate.remove(&owner.session_id);
        } else {
            if !candidate.contains_key(&owner.session_id) && candidate.len() >= 32 {
                return Err("too many simultaneous module variable publishers".to_string());
            }
            candidate.insert(owner.session_id.clone(), Publication { owner: owner.clone(), variables: values });
        }
        let after = merge_publisher_set(&candidate)?;
        if candidate.is_empty() {
            state.remove(&owner.module);
        } else {
            state.insert(owner.module, candidate);
        }
        Ok(before != after)
    }

    /*
     * Only the exact authenticated owner can revoke its contribution. A late
     * disconnect for an older PID incarnation must not revoke a newer one.
     */
    pub fn revoke(&self, owner: &PublisherIdentity) -> Result<bool, String> {
        validate_identity(owner)?;
        let mut state = self.inner.lock()
            .map_err(|_| "module variable registry lock poisoned".to_string())?;
        let Some(current) = state.get(&owner.module).cloned() else {
            return Ok(false);
        };
        let Some(publication) = current.get(&owner.session_id) else {
            return Ok(false);
        };
        if publication.owner != *owner {
            return Ok(false);
        }
        let before = merge_publisher_set(&current)?;
        let mut candidate = current;
        candidate.remove(&owner.session_id);
        let after = merge_publisher_set(&candidate)?;
        if candidate.is_empty() {
            state.remove(&owner.module);
        } else {
            state.insert(owner.module.clone(), candidate);
        }
        Ok(before != after)
    }

    /* Atomic snapshot used by the generic Registry preflight, not persisted. */
    pub fn snapshot(&self) -> Result<BTreeMap<String, BTreeMap<String, String>>, String> {
        let state = self.inner.lock()
            .map_err(|_| "module variable registry lock poisoned".to_string())?;
        let mut all = BTreeMap::new();
        for (module, publishers) in state.iter() {
            all.insert(module.clone(), merge_publisher_set(publishers)?);
        }
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(module: &str, session_id: &str, pid: u32) -> PublisherIdentity {
        PublisherIdentity {
            module: module.to_string(),
            session_id: session_id.to_string(),
            pid,
            start_time_ticks: 9001,
        }
    }

    fn vars(items: &[(&str, &str)]) -> BTreeMap<String, String> {
        items.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn two_opaque_variables_require_no_semantic_interpretation() {
        let state = LiveModuleVariables::new();
        let who = owner("neebles-secrets", "daemon", 101);
        assert!(state.publish(who.clone(), vars(&[("master-key", "enabled"), ("vault", "unlocked")])).unwrap());
        assert_eq!(state.snapshot().unwrap()["neebles-secrets"], vars(&[("master-key", "enabled"), ("vault", "unlocked")]));
        assert!(!state.publish(who.clone(), vars(&[("master-key", "enabled"), ("vault", "unlocked")])).unwrap());
        assert!(state.publish(who, vars(&[("master-key", "enabled"), ("vault", "locked")])).unwrap());
        assert_eq!(state.snapshot().unwrap()["neebles-secrets"]["vault"], "locked");
    }

    #[test]
    fn modules_do_not_share_publication_territory() {
        let state = LiveModuleVariables::new();
        state.publish(owner("neebles-secrets", "daemon", 101), vars(&[("status", "one")])).unwrap();
        state.publish(owner("neebles-db", "daemon", 201), vars(&[("status", "two")])).unwrap();
        assert_eq!(state.snapshot().unwrap().len(), 2);
        assert_eq!(state.snapshot().unwrap()["neebles-db"]["status"], "two");
    }

    #[test]
    fn two_publishers_with_disjoint_keys_can_coexist() {
        let state = LiveModuleVariables::new();
        state.publish(owner("neebles-proxy", "service-a", 100), vars(&[("a", "1")])).unwrap();
        state.publish(owner("neebles-proxy", "service-b", 101), vars(&[("b", "2")])).unwrap();
        assert_eq!(state.snapshot().unwrap()["neebles-proxy"], vars(&[("a", "1"), ("b", "2")]));
        assert!(state.revoke(&owner("neebles-proxy", "service-b", 101)).unwrap());
        assert_eq!(state.snapshot().unwrap()["neebles-proxy"], vars(&[("a", "1")]));
    }

    #[test]
    fn conflicting_owners_are_rejected_without_state_mutation() {
        let state = LiveModuleVariables::new();
        state.publish(owner("neebles-proxy", "service-a", 100), vars(&[("a", "1")])).unwrap();
        assert!(state.publish(owner("neebles-proxy", "service-b", 101), vars(&[("a", "1")])).is_err());
        assert_eq!(state.snapshot().unwrap()["neebles-proxy"], vars(&[("a", "1")]));
    }

    #[test]
    fn stale_process_cannot_replace_or_revoke_live_publisher() {
        let state = LiveModuleVariables::new();
        let old = owner("neebles-secrets", "daemon", 100);
        let new = owner("neebles-secrets", "daemon", 101);
        state.publish(old.clone(), vars(&[("x", "old")])).unwrap();
        assert!(state.publish(new.clone(), vars(&[("x", "new")])).is_err());
        assert!(!state.revoke(&new).unwrap());
        assert_eq!(state.snapshot().unwrap()["neebles-secrets"]["x"], "old");
        assert!(state.revoke(&old).unwrap());
        assert!(state.publish(new, vars(&[("x", "new")])).unwrap());
    }

    #[test]
    fn empty_snapshot_and_verified_disconnect_invalidate_variables() {
        let state = LiveModuleVariables::new();
        let who = owner("neebles-secrets", "daemon", 101);
        state.publish(who.clone(), vars(&[("x", "y")])).unwrap();
        assert!(state.publish(who.clone(), BTreeMap::new()).unwrap());
        assert!(state.snapshot().unwrap().is_empty());
        assert!(!state.revoke(&who).unwrap());
    }

    #[test]
    fn invalid_identity_and_oversized_values_fail_closed() {
        let state = LiveModuleVariables::new();
        assert!(state.publish(owner("../../other", "one", 100), vars(&[("x", "1")])).is_err());
        assert!(state.publish(owner("neebles-secrets", "", 100), vars(&[("x", "1")])).is_err());
        assert!(state.publish(owner("neebles-secrets", "one", 0), vars(&[("x", "1")])).is_err());
        assert!(state.publish(owner("neebles-secrets", "one", 100), vars(&[("x", "\n")])).is_err());
        assert!(state.publish(owner("neebles-secrets", "one", 100), vars(&[("x", &"x".repeat(2049))])).is_err());
        assert!(state.snapshot().unwrap().is_empty());
    }
}

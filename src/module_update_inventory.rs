//! Update snapshot planning, without any knowledge of module implementation technology.
//!
//! The canonical Governor Open is SINGLE per module and is never represented
//! by an extra. Additional components are an opaque, dynamically sized set.
//! This is a planning/value type only: runtime ownership and lifecycle execution
//! remain with their existing authoritative subsystems.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraSnapshot {
    pub id: String,
    pub active: bool,
    pub resume_transition: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeExtra {
    pub id: String,
    pub transition: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleUpdateSnapshot {
    module: String,
    canonical_open_was_active: bool,
    extras: BTreeMap<String, ExtraSnapshot>,
}

/// B22_EXTRA_TRANSACTION_JOURNAL: value-only proof of which physical extras
/// have been stopped and which resumed. This is NOT an executor: callers must
/// authenticate process exit / new runtime ownership BEFORE recording success.
/// The journal deliberately cannot claim a completed update until all original
/// active extras have been independently confirmed restored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraUpdatePhase {
    Stopping,
    Quiesced,
    Published,
    Restoring,
    Complete,
    RecoveryRequired,
}

#[derive(Debug)]
pub struct ExtraUpdateJournal {
    module: String,
    required: BTreeSet<String>,
    stopped: BTreeSet<String>,
    restored: BTreeSet<String>,
    phase: ExtraUpdatePhase,
}

impl ExtraUpdateJournal {
    pub fn new(snapshot: &ModuleUpdateSnapshot) -> Self {
        Self {
            module: snapshot.module().to_owned(),
            required: snapshot
                .active_extra_ids()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            stopped: BTreeSet::new(),
            restored: BTreeSet::new(),
            phase: ExtraUpdatePhase::Stopping,
        }
    }

    pub fn phase(&self) -> ExtraUpdatePhase {
        self.phase
    }
    pub fn module(&self) -> &str {
        &self.module
    }

    /// Must only follow pidfd stop + physical reaping + exact owner unregister.
    pub fn record_confirmed_stop(&mut self, id: &str) -> Result<(), String> {
        if self.phase != ExtraUpdatePhase::Stopping
            || !self.required.contains(id)
            || !self.stopped.insert(id.to_owned())
        {
            return Err(format!(
                "cannot record unverified/duplicate stop of '{}'",
                id
            ));
        }
        Ok(())
    }

    /// All originally active extras have been physically confirmed gone.
    pub fn confirm_quiescent(&mut self) -> Result<(), String> {
        if self.phase != ExtraUpdatePhase::Stopping || self.stopped != self.required {
            return Err("cannot publish while an original extra has not stopped".into());
        }
        self.phase = ExtraUpdatePhase::Quiesced;
        Ok(())
    }

    /// Must only be called after the existing atomic module publication succeeds.
    pub fn record_published(&mut self) -> Result<(), String> {
        if self.phase != ExtraUpdatePhase::Quiesced {
            return Err("publication requires confirmed extra quiescence".into());
        }
        self.phase = ExtraUpdatePhase::Published;
        Ok(())
    }

    /// Record only after a fresh, correctly scoped owner is kernel verified.
    pub fn record_confirmed_restore(&mut self, id: &str) -> Result<(), String> {
        if !matches!(
            self.phase,
            ExtraUpdatePhase::Published | ExtraUpdatePhase::Restoring
        ) || !self.required.contains(id)
            || !self.stopped.contains(id)
            || !self.restored.insert(id.to_owned())
        {
            return Err(format!(
                "cannot record unverified/duplicate resume of '{}'",
                id
            ));
        }
        self.phase = ExtraUpdatePhase::Restoring;
        Ok(())
    }

    pub fn confirm_complete(&mut self) -> Result<(), String> {
        if !matches!(
            self.phase,
            ExtraUpdatePhase::Published | ExtraUpdatePhase::Restoring
        ) || self.restored != self.required
        {
            return Err("update still has unrestored active extras".into());
        }
        self.phase = ExtraUpdatePhase::Complete;
        Ok(())
    }

    /// Missing owners are compensation targets, never evidence of success.
    pub fn pending_restores(&self) -> Vec<&str> {
        self.stopped
            .difference(&self.restored)
            .map(String::as_str)
            .collect()
    }

    /// Enter a non-success terminal state when publication or recovery fails.
    /// The actual rollback and physical verification remain external duties.
    pub fn require_recovery(&mut self) {
        if self.phase != ExtraUpdatePhase::Complete {
            self.phase = ExtraUpdatePhase::RecoveryRequired;
        }
    }
}

/// B23_PREPUBLICATION_COMPENSATION: drive the *prepublication* stop batch
/// through caller-provided authoritative operations. `stop_exact` must include
/// pidfd + wait/reap + exact owner unregister verification. `restore_previous`
/// must execute the OLD contract via Lifecycle and verify a fresh owned runtime.
/// Neither callback is implemented here; productive update wiring is disabled.
/// If an operation reports an ambiguous failure, it is preserved as unresolved
/// instead of claiming rollback succeeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraStopFailure {
    pub failed_extra: String,
    pub cause: String,
    /// Reverse-order compensation failures, including the uncertain stop itself.
    pub unresolved: Vec<String>,
    pub recovered: Vec<String>,
}

pub fn stop_extras_before_publication<Stop, Restore>(
    snapshot: &ModuleUpdateSnapshot,
    journal: &mut ExtraUpdateJournal,
    mut stop_exact: Stop,
    mut restore_previous: Restore,
) -> Result<(), ExtraStopFailure>
where
    Stop: FnMut(&str) -> Result<(), String>,
    Restore: FnMut(&ResumeExtra) -> Result<(), String>,
{
    if journal.module() != snapshot.module()
        || journal.phase() != ExtraUpdatePhase::Stopping
        || !journal.stopped.is_empty()
        || !journal.restored.is_empty()
        || journal.required
            != snapshot
                .active_extra_ids()
                .into_iter()
                .map(str::to_owned)
                .collect()
    {
        return Err(ExtraStopFailure {
            failed_extra: String::new(),
            cause: "stop journal does not match untouched snapshot".into(),
            unresolved: Vec::new(),
            recovered: Vec::new(),
        });
    }

    let intents = snapshot.resume_extras();
    let mut confirmed = Vec::<ResumeExtra>::new();
    for intent in &intents {
        let result =
            stop_exact(&intent.id).and_then(|()| journal.record_confirmed_stop(&intent.id));
        if let Err(cause) = result {
            // A failed stop MAY already have affected the target. Do not
            // restart it blindly: caller must resolve its true physical state.
            let mut unresolved = vec![intent.id.clone()];
            let mut recovered = Vec::new();
            for previous in confirmed.iter().rev() {
                match restore_previous(previous) {
                    Ok(()) => recovered.push(previous.id.clone()),
                    Err(_) => unresolved.push(previous.id.clone()),
                }
            }
            journal.require_recovery();
            return Err(ExtraStopFailure {
                failed_extra: intent.id.clone(),
                cause,
                unresolved,
                recovered,
            });
        }
        confirmed.push(intent.clone());
    }
    if let Err(cause) = journal.confirm_quiescent() {
        journal.require_recovery();
        return Err(ExtraStopFailure {
            failed_extra: String::new(),
            cause,
            unresolved: confirmed.into_iter().map(|item| item.id).collect(),
            recovered: Vec::new(),
        });
    }
    Ok(())
}

/// B24_POSTPUBLICATION_RESUME_GATE: resume only the originally active extras
/// after the caller confirms that the new version was atomically published.
/// `restore_new` MUST execute the new Lifecycle contract and return a freshly
/// kernel-authenticated owner, not merely a successful spawn command.
/// A failure is a rollback requirement, never a completed hot-update. The
/// actual module rollback and physical cleanup remain with update authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraPostpublicationFailure {
    pub failed_extra: String,
    pub cause: String,
    pub confirmed_restored: Vec<String>,
    pub pending_restore: Vec<String>,
}

pub fn restore_extras_after_publication<Restore>(
    snapshot: &ModuleUpdateSnapshot,
    journal: &mut ExtraUpdateJournal,
    mut restore_new: Restore,
) -> Result<(), ExtraPostpublicationFailure>
where
    Restore: FnMut(&ResumeExtra) -> Result<(), String>,
{
    if snapshot.module() != journal.module()
        || journal.phase() != ExtraUpdatePhase::Published
        || journal.required
            != snapshot
                .active_extra_ids()
                .into_iter()
                .map(str::to_owned)
                .collect()
        || journal.stopped != journal.required
        || !journal.restored.is_empty()
    {
        return Err(ExtraPostpublicationFailure {
            failed_extra: String::new(),
            cause: "publication journal does not match confirmed quiescent snapshot".into(),
            confirmed_restored: Vec::new(),
            pending_restore: journal
                .pending_restores()
                .into_iter()
                .map(str::to_owned)
                .collect(),
        });
    }
    let mut confirmed = Vec::<String>::new();
    for intent in snapshot.resume_extras() {
        if let Err(cause) =
            restore_new(&intent).and_then(|()| journal.record_confirmed_restore(&intent.id))
        {
            let pending = journal
                .pending_restores()
                .into_iter()
                .map(str::to_owned)
                .collect();
            journal.require_recovery();
            return Err(ExtraPostpublicationFailure {
                failed_extra: intent.id,
                cause,
                confirmed_restored: confirmed,
                pending_restore: pending,
            });
        }
        confirmed.push(intent.id);
    }
    if let Err(cause) = journal.confirm_complete() {
        let pending = journal
            .pending_restores()
            .into_iter()
            .map(str::to_owned)
            .collect();
        journal.require_recovery();
        return Err(ExtraPostpublicationFailure {
            failed_extra: String::new(),
            cause,
            confirmed_restored: confirmed,
            pending_restore: pending,
        });
    }
    Ok(())
}

/// B25_POSTPUBLICATION_ROLLBACK_COORDINATOR: callback-driven rollback policy.
/// Every callback MUST use the existing authoritative process/Lifecycle/filesystem
/// mechanisms; success means an independently VERIFIED completion.
/// Never restore old extras until ALL new owners have been removed and the
/// previous version has been physically restored. An ambiguous stop must halt
/// the sequence, not launch a duplicate extra or roll back underneath a live one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtraRollbackStage {
    Validation,
    StopNew,
    VerifyNewQuiescent,
    RestorePreviousFiles,
    RestorePreviousExtras,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraRollbackFailure {
    pub stage: ExtraRollbackStage,
    pub unresolved: Vec<String>,
    pub detail: String,
}

/// Returns Ok only when old files and all originally active extras were
/// independently verified recovered. It deliberately keeps RecoveryRequired:
/// a rolled-back update must never be reported as a successful new version.
pub fn rollback_extras_after_failed_publication<
    StopNew,
    VerifyNewQuiescent,
    RestoreFiles,
    RestoreOld,
>(
    snapshot: &ModuleUpdateSnapshot,
    journal: &mut ExtraUpdateJournal,
    mut stop_new: StopNew,
    mut verify_new_quiescent: VerifyNewQuiescent,
    mut restore_previous_files: RestoreFiles,
    mut restore_old: RestoreOld,
) -> Result<(), ExtraRollbackFailure>
where
    StopNew: FnMut(&str) -> Result<(), String>,
    VerifyNewQuiescent: FnMut() -> Result<(), String>,
    RestoreFiles: FnMut() -> Result<(), String>,
    RestoreOld: FnMut(&ResumeExtra) -> Result<(), String>,
{
    let expected: BTreeSet<String> = snapshot
        .active_extra_ids()
        .into_iter()
        .map(str::to_owned)
        .collect();
    if journal.module != snapshot.module
        || journal.required != expected
        || journal.stopped != expected
        || journal.phase != ExtraUpdatePhase::RecoveryRequired
    {
        return Err(ExtraRollbackFailure {
            stage: ExtraRollbackStage::Validation,
            unresolved: Vec::new(),
            detail: "rollback requires the exact postpublication recovery journal".into(),
        });
    }
    // The set of confirmed new owners is authoritative; stop in reverse order.
    // If any stop fails, do NOT switch files under possibly running code.
    let mut owners: Vec<String> = journal.restored.iter().cloned().collect();
    owners.reverse();
    let mut stop_errors = Vec::new();
    for id in owners {
        if let Err(error) = stop_new(&id) {
            stop_errors.push(format!("{id}: {error}"));
        }
    }
    if !stop_errors.is_empty() {
        return Err(ExtraRollbackFailure {
            stage: ExtraRollbackStage::StopNew,
            unresolved: stop_errors,
            detail: "new extra cleanup is not fully verified; previous files untouched".into(),
        });
    }
    // B34: recorded successful resumes are NOT a complete inventory. A failed
    // Lifecycle transition may spawn a new process before reporting failure.
    // Query the authoritative physical registry (fail closed on uncertainty)
    // before replacing files, even when journal.restored is empty.
    if let Err(error) = verify_new_quiescent() {
        return Err(ExtraRollbackFailure {
            stage: ExtraRollbackStage::VerifyNewQuiescent,
            unresolved: expected.iter().cloned().collect(),
            detail: format!(
                "new-version physical owner quiescence not verified; previous files untouched: {error}"
            ),
        });
    }
    if let Err(error) = restore_previous_files() {
        return Err(ExtraRollbackFailure {
            stage: ExtraRollbackStage::RestorePreviousFiles,
            unresolved: expected.iter().cloned().collect(),
            detail: error,
        });
    }
    let mut failures = Vec::new();
    for extra in snapshot.resume_extras() {
        if let Err(error) = restore_old(&extra) {
            failures.push(format!("{}: {}", extra.id, error));
        }
    }
    if !failures.is_empty() {
        return Err(ExtraRollbackFailure {
            stage: ExtraRollbackStage::RestorePreviousExtras,
            unresolved: failures,
            detail: "old version restored but some old extra owners remain unverified".into(),
        });
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

impl ModuleUpdateSnapshot {
    pub fn capture(
        module: impl Into<String>,
        canonical_open_was_active: bool,
        extras: impl IntoIterator<Item = ExtraSnapshot>,
    ) -> Result<Self, String> {
        let module = module.into();
        if !valid_identifier(&module) {
            return Err("invalid module identity in update snapshot".to_string());
        }
        let mut indexed = BTreeMap::new();
        for extra in extras {
            if !valid_identifier(&extra.id) || !valid_identifier(&extra.resume_transition) {
                return Err("invalid extra identity or transition in update snapshot".to_string());
            }
            // Governor Open is a singular, separate architectural concept.
            if extra.id == "open" || extra.id == "governor.open" {
                return Err("canonical Open cannot be declared as an extra".to_string());
            }
            if indexed.insert(extra.id.clone(), extra).is_some() {
                return Err("duplicate extra identity in update snapshot".to_string());
            }
        }
        Ok(Self {
            module,
            canonical_open_was_active,
            extras: indexed,
        })
    }

    pub fn module(&self) -> &str {
        &self.module
    }
    pub fn restore_canonical_open(&self) -> bool {
        self.canonical_open_was_active
    }
    pub fn extra_count(&self) -> usize {
        self.extras.len()
    }

    pub fn active_extra_ids(&self) -> BTreeSet<&str> {
        self.extras
            .iter()
            .filter_map(|(id, entry)| entry.active.then_some(id.as_str()))
            .collect()
    }

    pub fn resume_extras(&self) -> Vec<ResumeExtra> {
        self.extras
            .values()
            .filter(|entry| entry.active)
            .map(|entry| ResumeExtra {
                id: entry.id.clone(),
                transition: entry.resume_transition.clone(),
            })
            .collect()
    }

    /// Capture the singular Open from its runtime authority. A concurrent Opening
    /// is not a stable snapshot and must never be treated as Closed.
    pub fn canonical_open_from_runtime_state(
        state: crate::module_ipc::registry::ModuleRuntimeState,
    ) -> Result<bool, String> {
        use crate::module_ipc::registry::ModuleRuntimeState;
        match state {
            ModuleRuntimeState::Closed => Ok(false),
            ModuleRuntimeState::Open => Ok(true),
            ModuleRuntimeState::Opening => {
                Err("cannot snapshot module while canonical Open is transitioning".to_owned())
            }
        }
    }

    /// Obtain the actual canonical Open from the existing singular registry.
    /// No extra ever enters this registry through this adapter.
    pub fn probe_canonical_open(module: &str) -> Result<bool, String> {
        let state = crate::module_ipc::runtime_registry().state(module)?;
        Self::canonical_open_from_runtime_state(state)
    }

    /// Logical object state is NOT evidence of a running extra process.
    /// The caller must supply an authenticated runtime owner probe for each
    /// object, otherwise fail closed instead of inventing live ownership.
    pub fn capture_with_verified_extras<F>(
        module: &str,
        canonical_open_active: bool,
        lifecycle: &crate::lifecycle::LifecycleContract,
        logical_states: &BTreeMap<String, bool>,
        resume_transitions: &BTreeMap<String, String>,
        mut owner_probe: F,
    ) -> Result<Self, String>
    where
        F: FnMut(&str) -> Result<bool, String>,
    {
        let mut extras = Vec::new();
        for (id, resume) in resume_transitions {
            if id == "open" || id == "governor.open" {
                return Err("canonical Open cannot be declared as an extra".to_owned());
            }
            let object = lifecycle
                .objects
                .get(id)
                .ok_or_else(|| format!("undeclared Lifecycle extra '{}'", id))?;
            if !object.transitions.contains_key(resume) {
                return Err(format!(
                    "Lifecycle extra '{}' has no '{}' transition",
                    id, resume
                ));
            }
            let logical = logical_states
                .get(id)
                .copied()
                .ok_or_else(|| format!("extra '{}' has no recorded canonical logical state", id))?;
            // A live process may exist even if its logical flag is false.
            // Treat the mismatch as a blocker, not as permission to discard it.
            let live = owner_probe(id)?;
            if live != logical {
                return Err(format!(
                    "extra '{}' logical state and authenticated owner disagree",
                    id
                ));
            }
            extras.push(ExtraSnapshot {
                id: id.clone(),
                active: live,
                resume_transition: resume.clone(),
            });
        }
        Self::capture(module, canonical_open_active, extras)
    }

    /// Read only persisted logical object states; no state reconciliation or mutation.
    /// Missing records are not interpreted as evidence of stopped processes.
    pub fn logical_states_from_config(
        module: &str,
        states: &BTreeMap<String, BTreeMap<String, bool>>,
    ) -> BTreeMap<String, bool> {
        states.get(module).cloned().unwrap_or_default()
    }

    /// Read the actual singular Open registry and Boss persisted object states.
    /// Each extra still requires a separately authenticated owner probe, supplied
    /// by its future runtime authority. The caller cannot use configuration alone.
    /// This entrypoint does not start, stop or alter any component.
    pub fn capture_from_boss_authorities<F>(
        module: &str,
        lifecycle: &crate::lifecycle::LifecycleContract,
        resume_transitions: &BTreeMap<String, String>,
        owner_probe: F,
    ) -> Result<Self, String>
    where
        F: FnMut(&str) -> Result<bool, String>,
    {
        let canonical_open = Self::probe_canonical_open(module)?;
        let config = crate::config::load_or_initialize()?;
        let logical_states = Self::logical_states_from_config(module, &config.modules);
        Self::capture_with_verified_extras(
            module,
            canonical_open,
            lifecycle,
            &logical_states,
            resume_transitions,
            owner_probe,
        )
    }

    /// B21_PHYSICAL_LOGICAL_RESUME_PLAN: reconcile authenticated physical extra
    /// identifiers with persisted logical states before preparing any destructive
    /// update. Owner identities MUST have been checked by the persistent Boss;
    /// this method does not accept PIDs, issue signals, or create a second Open.
    /// Every physical extra must have a declared Lifecycle resume transition.
    pub fn capture_from_physical_extra_ids(
        module: &str,
        canonical_open_active: bool,
        lifecycle: &crate::lifecycle::LifecycleContract,
        logical_states: &BTreeMap<String, bool>,
        resume_transitions: &BTreeMap<String, String>,
        physical_extra_ids: impl IntoIterator<Item = String>,
    ) -> Result<Self, String> {
        let mut physical = BTreeSet::new();
        for id in physical_extra_ids {
            if !valid_identifier(&id)
                || matches!(id.as_str(), "open" | "governor.open")
                || !physical.insert(id)
            {
                return Err("invalid or duplicated authenticated physical extra".into());
            }
        }
        // Unknown physical owners must never be omitted from a stop/restore plan.
        for id in &physical {
            if !resume_transitions.contains_key(id) {
                return Err(format!("physical extra '{}' has no resume transition", id));
            }
        }
        let snapshot = Self::capture_with_verified_extras(
            module,
            canonical_open_active,
            lifecycle,
            logical_states,
            resume_transitions,
            |id| Ok(physical.contains(id)),
        )?;
        // Keep plan validation here: a logical state may contain unrelated
        // objects, but every declared extra must be represented and verifiable.
        if snapshot.active_extra_ids().len() != physical.len() {
            return Err("physical extra inventory and resume plan disagree".into());
        }
        Ok(snapshot)
    }

    /// Candidate preflight: a live singular Open must remain restorable.
    /// This checks only the canonical Governor binding, never an extra object.
    pub fn verify_candidate_canonical_open(
        canonical_open_was_active: bool,
        candidate: &crate::lifecycle::LifecycleContract,
    ) -> Result<(), String> {
        if !canonical_open_was_active {
            return Ok(());
        }
        if crate::lifecycle_governor_binding::resolve(candidate, "open")?.is_none() {
            return Err(
                "updated module cannot restore the singular canonical Governor Open".to_owned(),
            );
        }
        Ok(())
    }

    /// Fail closed if a new Lifecycle contract cannot restore an active extra.
    /// No executable paths or system actions are accepted by this planner.
    /// Validate active extras against actual module-owned Lifecycle objects.
    /// This does not infer any extra Open instance or execute a transition.
    pub fn verify_lifecycle_contract(
        &self,
        lifecycle: &crate::lifecycle::LifecycleContract,
    ) -> Result<(), String> {
        for intent in self.resume_extras() {
            let object = lifecycle.objects.get(&intent.id).ok_or_else(|| {
                format!("updated module no longer declares extra '{}'", intent.id)
            })?;
            if !object.transitions.contains_key(&intent.transition) {
                return Err(format!(
                    "updated module cannot restore extra '{}' through '{}'",
                    intent.id, intent.transition,
                ));
            }
        }
        Ok(())
    }

    pub fn verify_new_extras(
        &self,
        new_contract: &BTreeMap<String, BTreeSet<String>>,
    ) -> Result<(), String> {
        for intent in self.resume_extras() {
            if !new_contract
                .get(&intent.id)
                .is_some_and(|transitions| transitions.contains(&intent.transition))
            {
                return Err(format!(
                    "updated module cannot restore extra '{}' through '{}'",
                    intent.id, intent.transition,
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extra(id: &str, active: bool) -> ExtraSnapshot {
        ExtraSnapshot {
            id: id.to_owned(),
            active,
            resume_transition: "resume".to_owned(),
        }
    }

    #[test]
    fn b25_rollback_stops_new_first_then_restores_old_after_files() {
        let snap = ModuleUpdateSnapshot::capture(
            "mod",
            true,
            [extra("a", true), extra("b", true), extra("inactive", false)],
        )
        .unwrap();
        let mut journal = ExtraUpdateJournal::new(&snap);
        for id in ["a", "b"] {
            journal.record_confirmed_stop(id).unwrap();
        }
        journal.confirm_quiescent().unwrap();
        journal.record_published().unwrap();
        let failure = restore_extras_after_publication(&snap, &mut journal, |item| {
            if item.id == "b" {
                Err("new failed".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(failure.confirmed_restored, vec!["a"]);
        let events = std::cell::RefCell::new(Vec::new());
        rollback_extras_after_failed_publication(
            &snap,
            &mut journal,
            |id| {
                events.borrow_mut().push(format!("stop-new:{id}"));
                Ok(())
            },
            || {
                events.borrow_mut().push("verify-new-quiescent".into());
                Ok(())
            },
            || {
                events.borrow_mut().push("restore-files".into());
                Ok(())
            },
            |item| {
                events.borrow_mut().push(format!("restore-old:{}", item.id));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            *events.borrow(),
            vec![
                "stop-new:a",
                "verify-new-quiescent",
                "restore-files",
                "restore-old:a",
                "restore-old:b"
            ]
        );
        assert_eq!(journal.phase(), ExtraUpdatePhase::RecoveryRequired);
        assert!(journal.confirm_complete().is_err());
        assert!(snap.restore_canonical_open());
    }

    #[test]
    fn b25_ambiguous_new_stop_blocks_files_and_old_restore() {
        let snap =
            ModuleUpdateSnapshot::capture("mod", false, [extra("a", true), extra("b", true)])
                .unwrap();
        let mut journal = ExtraUpdateJournal::new(&snap);
        for id in ["a", "b"] {
            journal.record_confirmed_stop(id).unwrap();
        }
        journal.confirm_quiescent().unwrap();
        journal.record_published().unwrap();
        let _ = restore_extras_after_publication(&snap, &mut journal, |item| {
            if item.id == "b" {
                Err("new failed".into())
            } else {
                Ok(())
            }
        });
        let err = rollback_extras_after_failed_publication(
            &snap,
            &mut journal,
            |_| Err("owner still alive".into()),
            || panic!("must not probe after failed verified stop"),
            || panic!("cannot swap files while new extra may be live"),
            |_| panic!("cannot restore old while new extra may be live"),
        )
        .unwrap_err();
        assert_eq!(err.stage, ExtraRollbackStage::StopNew);
        assert_eq!(err.unresolved.len(), 1);
    }

    #[test]
    fn b25_old_restore_reports_partial_failure_without_claiming_success() {
        let snap =
            ModuleUpdateSnapshot::capture("mod", false, [extra("a", true), extra("b", true)])
                .unwrap();
        let mut journal = ExtraUpdateJournal::new(&snap);
        for id in ["a", "b"] {
            journal.record_confirmed_stop(id).unwrap();
        }
        journal.confirm_quiescent().unwrap();
        journal.record_published().unwrap();
        journal.require_recovery();
        let err = rollback_extras_after_failed_publication(
            &snap,
            &mut journal,
            |_| panic!("there were no verified new owners"),
            || Ok(()), // physically verified empty inventory
            || Ok(()),
            |item| {
                if item.id == "b" {
                    Err("old lifecycle failed".into())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(err.stage, ExtraRollbackStage::RestorePreviousExtras);
        assert_eq!(err.unresolved, vec!["b: old lifecycle failed"]);
        assert_eq!(journal.phase(), ExtraUpdatePhase::RecoveryRequired);
    }

    #[test]
    fn b34_unjournaled_new_owner_blocks_previous_files() {
        let snap = ModuleUpdateSnapshot::capture("mod", false, [extra("a", true)]).unwrap();
        let mut journal = ExtraUpdateJournal::new(&snap);
        journal.record_confirmed_stop("a").unwrap();
        journal.confirm_quiescent().unwrap();
        journal.record_published().unwrap();
        // A new owner may be spawned before Lifecycle reports failure;
        // journal.restored is still empty and is NOT proof of quiescence.
        journal.require_recovery();
        let err = rollback_extras_after_failed_publication(
            &snap,
            &mut journal,
            |_| panic!("no journaled new owners"),
            || Err("unrecorded new process still alive".into()),
            || panic!("old files must remain untouched"),
            |_| panic!("old extras must remain untouched"),
        )
        .unwrap_err();
        assert_eq!(err.stage, ExtraRollbackStage::VerifyNewQuiescent);
        assert_eq!(err.unresolved, vec!["a"]);
        assert_eq!(journal.phase(), ExtraUpdatePhase::RecoveryRequired);
    }

    #[test]
    fn b34_quiescence_probe_runs_even_without_journaled_new_owners() {
        let snap = ModuleUpdateSnapshot::capture("mod", false, [extra("a", true)]).unwrap();
        let mut journal = ExtraUpdateJournal::new(&snap);
        journal.record_confirmed_stop("a").unwrap();
        journal.confirm_quiescent().unwrap();
        journal.record_published().unwrap();
        journal.require_recovery();
        let events = std::cell::RefCell::new(Vec::new());
        rollback_extras_after_failed_publication(
            &snap,
            &mut journal,
            |_| panic!("no journaled new owners"),
            || {
                events.borrow_mut().push("verify");
                Ok(())
            },
            || {
                events.borrow_mut().push("files");
                Ok(())
            },
            |_| {
                events.borrow_mut().push("old");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(*events.borrow(), vec!["verify", "files", "old"]);
        assert_eq!(journal.phase(), ExtraUpdatePhase::RecoveryRequired);
    }

    #[test]
    fn b24_restores_only_originally_active_extras_after_publication() {
        let snapshot = ModuleUpdateSnapshot::capture(
            "alpha",
            true,
            [extra("a", true), extra("b", false), extra("c", true)],
        )
        .unwrap();
        let mut journal = ExtraUpdateJournal::new(&snapshot);
        journal.record_confirmed_stop("a").unwrap();
        journal.record_confirmed_stop("c").unwrap();
        journal.confirm_quiescent().unwrap();
        journal.record_published().unwrap();
        let mut resumed = Vec::new();
        restore_extras_after_publication(&snapshot, &mut journal, |intent| {
            resumed.push((intent.id.clone(), intent.transition.clone()));
            Ok(())
        })
        .unwrap();
        assert_eq!(resumed.len(), 2);
        assert_eq!(
            resumed.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(),
            vec!["a", "c"]
        );
        assert_eq!(journal.phase(), ExtraUpdatePhase::Complete);
        assert!(snapshot.restore_canonical_open());
    }

    #[test]
    fn b24_partial_restore_requires_rollback_and_keeps_exact_pending() {
        let snapshot = ModuleUpdateSnapshot::capture(
            "alpha",
            false,
            [extra("a", true), extra("b", true), extra("c", true)],
        )
        .unwrap();
        let mut journal = ExtraUpdateJournal::new(&snapshot);
        for id in ["a", "b", "c"] {
            journal.record_confirmed_stop(id).unwrap();
        }
        journal.confirm_quiescent().unwrap();
        journal.record_published().unwrap();
        let failure = restore_extras_after_publication(&snapshot, &mut journal, |intent| {
            if intent.id == "b" {
                Err("new Lifecycle failed".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(failure.failed_extra, "b");
        assert_eq!(failure.confirmed_restored, vec!["a"]);
        assert_eq!(failure.pending_restore, vec!["b", "c"]);
        assert_eq!(journal.phase(), ExtraUpdatePhase::RecoveryRequired);
        assert!(journal.confirm_complete().is_err());
    }

    #[test]
    fn b24_refuses_resume_before_publication_without_invoking_lifecycle() {
        let snapshot = ModuleUpdateSnapshot::capture("alpha", false, [extra("a", true)]).unwrap();
        let mut journal = ExtraUpdateJournal::new(&snapshot);
        journal.record_confirmed_stop("a").unwrap();
        journal.confirm_quiescent().unwrap();
        let err = restore_extras_after_publication(&snapshot, &mut journal, |_| {
            panic!("cannot resume before physical publication")
        })
        .unwrap_err();
        assert!(err.cause.contains("publication journal"));
        assert_eq!(journal.phase(), ExtraUpdatePhase::Quiesced);
    }

    #[test]
    fn b23_stops_all_active_extras_before_publication() {
        let snapshot = ModuleUpdateSnapshot::capture(
            "mod",
            true,
            [extra("a", true), extra("b", false), extra("c", true)],
        )
        .unwrap();
        let mut journal = ExtraUpdateJournal::new(&snapshot);
        let mut stopped = Vec::new();
        stop_extras_before_publication(
            &snapshot,
            &mut journal,
            |id| {
                stopped.push(id.to_owned());
                Ok(())
            },
            |_| panic!("no compensation on success"),
        )
        .unwrap();
        assert_eq!(stopped, vec!["a", "c"]);
        assert_eq!(journal.phase(), ExtraUpdatePhase::Quiesced);
        assert_eq!(journal.pending_restores(), vec!["a", "c"]);
        assert!(snapshot.restore_canonical_open());
    }

    #[test]
    fn b23_failure_compensates_confirmed_stops_in_reverse_order() {
        let snapshot = ModuleUpdateSnapshot::capture(
            "mod",
            false,
            [extra("a", true), extra("b", true), extra("c", true)],
        )
        .unwrap();
        let mut journal = ExtraUpdateJournal::new(&snapshot);
        let mut recovered = Vec::new();
        let failure = stop_extras_before_publication(
            &snapshot,
            &mut journal,
            |id| {
                if id == "c" {
                    Err("signal uncertain".into())
                } else {
                    Ok(())
                }
            },
            |item| {
                recovered.push(item.id.clone());
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(failure.failed_extra, "c");
        assert_eq!(failure.unresolved, vec!["c"]);
        assert_eq!(failure.recovered, vec!["b", "a"]);
        assert_eq!(recovered, vec!["b", "a"]);
        assert_eq!(journal.phase(), ExtraUpdatePhase::RecoveryRequired);
        assert!(journal.record_published().is_err());
    }

    #[test]
    fn b23_compensation_failure_remains_explicitly_unresolved() {
        let snapshot = ModuleUpdateSnapshot::capture(
            "mod",
            false,
            [extra("a", true), extra("b", true), extra("c", true)],
        )
        .unwrap();
        let mut journal = ExtraUpdateJournal::new(&snapshot);
        let failure = stop_extras_before_publication(
            &snapshot,
            &mut journal,
            |id| {
                if id == "c" {
                    Err("unable to confirm exit".into())
                } else {
                    Ok(())
                }
            },
            |item| {
                if item.id == "b" {
                    Err("resume failed".into())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(failure.unresolved, vec!["c", "b"]);
        assert_eq!(failure.recovered, vec!["a"]);
        assert_eq!(journal.phase(), ExtraUpdatePhase::RecoveryRequired);
    }

    #[test]
    fn b23_rejects_mismatched_journal_without_calling_operations() {
        let snapshot = ModuleUpdateSnapshot::capture("mod", false, [extra("a", true)]).unwrap();
        let other = ModuleUpdateSnapshot::capture("other", false, [extra("a", true)]).unwrap();
        let mut journal = ExtraUpdateJournal::new(&other);
        let err = stop_extras_before_publication(
            &snapshot,
            &mut journal,
            |_| panic!("never stop"),
            |_| panic!("never restore"),
        )
        .unwrap_err();
        assert_eq!(err.cause, "stop journal does not match untouched snapshot");
        assert_eq!(journal.phase(), ExtraUpdatePhase::Stopping);
    }

    #[test]
    fn b22_requires_every_confirmed_stop_before_publication() {
        let snapshot = ModuleUpdateSnapshot::capture(
            "mod",
            true,
            [extra("a", true), extra("b", true), extra("c", false)],
        )
        .unwrap();
        let mut journal = ExtraUpdateJournal::new(&snapshot);
        assert_eq!(journal.module(), "mod");
        assert!(journal.confirm_quiescent().is_err());
        assert!(journal.record_published().is_err());
        journal.record_confirmed_stop("a").unwrap();
        assert!(journal.record_confirmed_stop("a").is_err());
        assert!(journal.record_confirmed_stop("c").is_err());
        assert!(journal.confirm_quiescent().is_err());
        journal.record_confirmed_stop("b").unwrap();
        journal.confirm_quiescent().unwrap();
        journal.record_published().unwrap();
        assert_eq!(journal.pending_restores(), vec!["a", "b"]);
        assert!(journal.confirm_complete().is_err());
    }

    #[test]
    fn b22_only_confirms_complete_after_every_new_owner_verified() {
        let snapshot =
            ModuleUpdateSnapshot::capture("mod", false, [extra("a", true), extra("b", true)])
                .unwrap();
        let mut journal = ExtraUpdateJournal::new(&snapshot);
        assert!(journal.record_confirmed_restore("a").is_err());
        journal.record_confirmed_stop("a").unwrap();
        journal.record_confirmed_stop("b").unwrap();
        journal.confirm_quiescent().unwrap();
        journal.record_published().unwrap();
        journal.record_confirmed_restore("b").unwrap();
        assert_eq!(journal.pending_restores(), vec!["a"]);
        assert!(journal.confirm_complete().is_err());
        assert!(journal.record_confirmed_restore("b").is_err());
        journal.record_confirmed_restore("a").unwrap();
        journal.confirm_complete().unwrap();
        assert_eq!(journal.phase(), ExtraUpdatePhase::Complete);
        assert!(journal.record_confirmed_restore("a").is_err());
    }

    #[test]
    fn b22_failure_never_becomes_success_and_zero_extras_is_valid() {
        let snapshot = ModuleUpdateSnapshot::capture("mod", false, [extra("a", true)]).unwrap();
        let mut journal = ExtraUpdateJournal::new(&snapshot);
        journal.record_confirmed_stop("a").unwrap();
        journal.require_recovery();
        assert_eq!(journal.phase(), ExtraUpdatePhase::RecoveryRequired);
        assert_eq!(journal.pending_restores(), vec!["a"]);
        assert!(journal.confirm_quiescent().is_err());
        assert!(journal.confirm_complete().is_err());
        let empty =
            ModuleUpdateSnapshot::capture("mod", false, Vec::<ExtraSnapshot>::new()).unwrap();
        let mut zero = ExtraUpdateJournal::new(&empty);
        zero.confirm_quiescent().unwrap();
        zero.record_published().unwrap();
        zero.confirm_complete().unwrap();
        assert_eq!(zero.phase(), ExtraUpdatePhase::Complete);
    }

    #[test]
    fn b21_rejects_undeclared_physical_owner_even_when_logical_false() {
        let lifecycle = crate::lifecycle::LifecycleContract::default();
        let mut states = BTreeMap::new();
        states.insert("worker".into(), false);
        let transitions = BTreeMap::new();
        assert!(ModuleUpdateSnapshot::capture_from_physical_extra_ids(
            "alpha",
            false,
            &lifecycle,
            &states,
            &transitions,
            vec!["worker".to_string()]
        )
        .is_err());
    }

    #[test]
    fn b21_empty_physical_and_resume_plan_preserves_single_open() {
        let lifecycle = crate::lifecycle::LifecycleContract::default();
        let snapshot = ModuleUpdateSnapshot::capture_from_physical_extra_ids(
            "alpha",
            true,
            &lifecycle,
            &BTreeMap::new(),
            &BTreeMap::new(),
            Vec::<String>::new(),
        )
        .unwrap();
        assert!(snapshot.restore_canonical_open());
        assert!(snapshot.resume_extras().is_empty());
    }

    #[test]
    fn b21_duplicate_or_canonical_physical_owner_fails_closed() {
        let lifecycle = crate::lifecycle::LifecycleContract::default();
        for entries in [vec!["cpu", "cpu"], vec!["governor.open"]] {
            assert!(ModuleUpdateSnapshot::capture_from_physical_extra_ids(
                "alpha",
                false,
                &lifecycle,
                &BTreeMap::new(),
                &BTreeMap::new(),
                entries.into_iter().map(str::to_string)
            )
            .is_err());
        }
    }

    #[test]
    fn canonical_open_is_single_and_not_an_extra() {
        let snapshot = ModuleUpdateSnapshot::capture(
            "monitor",
            true,
            [extra("sensor.cpu", true), extra("sensor.gpu", true)],
        )
        .unwrap();
        assert_eq!(snapshot.module(), "monitor");
        assert!(snapshot.restore_canonical_open());
        assert_eq!(snapshot.extra_count(), 2);
        assert!(ModuleUpdateSnapshot::capture("monitor", false, [extra("open", true)]).is_err());
        assert!(
            ModuleUpdateSnapshot::capture("monitor", false, [extra("governor.open", true)])
                .is_err()
        );
    }

    #[test]
    fn arbitrary_extras_only_restore_active_members() {
        let snapshot = ModuleUpdateSnapshot::capture(
            "monitor",
            false,
            [
                extra("cpu", true),
                extra("gpu", false),
                extra("fan", true),
                extra("clock", false),
                extra("ram", true),
                extra("disk", true),
            ],
        )
        .unwrap();
        assert!(!snapshot.restore_canonical_open());
        assert_eq!(snapshot.extra_count(), 6);
        assert_eq!(
            snapshot
                .resume_extras()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["cpu", "disk", "fan", "ram"]
        );
    }

    #[test]
    fn rejects_duplicate_and_unsafe_extras() {
        assert!(ModuleUpdateSnapshot::capture(
            "monitor",
            false,
            [extra("cpu", true), extra("cpu", false)]
        )
        .is_err());
        assert!(
            ModuleUpdateSnapshot::capture("monitor", false, [extra("../escape", true)]).is_err()
        );
        assert!(ModuleUpdateSnapshot::capture("../monitor", false, [extra("cpu", true)]).is_err());
    }

    #[test]
    fn new_contract_must_allow_resume_for_each_active_extra() {
        let snapshot = ModuleUpdateSnapshot::capture(
            "monitor",
            true,
            [extra("cpu", true), extra("gpu", false)],
        )
        .unwrap();
        assert!(snapshot.verify_new_extras(&BTreeMap::new()).is_err());
        let supported = BTreeMap::from([("cpu".to_owned(), BTreeSet::from(["resume".to_owned()]))]);
        assert!(snapshot.verify_new_extras(&supported).is_ok());
        assert_eq!(snapshot.active_extra_ids(), BTreeSet::from(["cpu"]));
    }

    #[test]
    fn real_lifecycle_contract_rejects_missing_or_removed_active_extra() {
        use crate::lifecycle::{Battleplan, LifecycleContract, ObjectContract};
        let snapshot = ModuleUpdateSnapshot::capture(
            "monitor",
            true,
            [extra("cpu", true), extra("gpu", false)],
        )
        .unwrap();
        let mut lifecycle = LifecycleContract::default();
        assert!(snapshot.verify_lifecycle_contract(&lifecycle).is_err());
        let mut cpu = ObjectContract::default();
        cpu.transitions
            .insert("resume".to_owned(), Battleplan::default());
        lifecycle.objects.insert("cpu".to_owned(), cpu);
        assert!(snapshot.verify_lifecycle_contract(&lifecycle).is_ok());
        lifecycle
            .objects
            .get_mut("cpu")
            .unwrap()
            .transitions
            .clear();
        assert!(snapshot.verify_lifecycle_contract(&lifecycle).is_err());
    }

    #[test]
    fn real_lifecycle_contract_accepts_zero_extras() {
        let snapshot = ModuleUpdateSnapshot::capture("simple", true, []).unwrap();
        assert!(snapshot
            .verify_lifecycle_contract(&crate::lifecycle::LifecycleContract::default())
            .is_ok());
    }

    #[test]
    fn logical_states_read_only_target_module() {
        let states = BTreeMap::from([
            (
                "alpha".to_string(),
                BTreeMap::from([("sensor".to_string(), true)]),
            ),
            (
                "beta".to_string(),
                BTreeMap::from([("sensor".to_string(), false)]),
            ),
        ]);
        assert_eq!(
            ModuleUpdateSnapshot::logical_states_from_config("alpha", &states).get("sensor"),
            Some(&true)
        );
        assert_eq!(
            ModuleUpdateSnapshot::logical_states_from_config("beta", &states).get("sensor"),
            Some(&false)
        );
        assert!(ModuleUpdateSnapshot::logical_states_from_config("gamma", &states).is_empty());
        assert_eq!(states.len(), 2);
    }

    #[test]
    fn opening_race_refuses_to_capture_open_as_closed() {
        use crate::module_ipc::registry::ModuleRuntimeState;
        assert_eq!(
            ModuleUpdateSnapshot::canonical_open_from_runtime_state(ModuleRuntimeState::Closed)
                .unwrap(),
            false
        );
        assert_eq!(
            ModuleUpdateSnapshot::canonical_open_from_runtime_state(ModuleRuntimeState::Open)
                .unwrap(),
            true
        );
        assert!(ModuleUpdateSnapshot::canonical_open_from_runtime_state(
            ModuleRuntimeState::Opening
        )
        .is_err());
    }

    #[test]
    fn verified_extra_owner_must_match_logical_state() {
        use crate::lifecycle::{Battleplan, LifecycleContract, ObjectContract};
        let mut lifecycle = LifecycleContract::default();
        let mut extra = ObjectContract::default();
        extra
            .transitions
            .insert("resume".to_owned(), Battleplan::default());
        lifecycle.objects.insert("sensor.cpu".to_owned(), extra);
        let states = BTreeMap::from([("sensor.cpu".to_owned(), true)]);
        let resumes = BTreeMap::from([("sensor.cpu".to_owned(), "resume".to_owned())]);
        assert!(ModuleUpdateSnapshot::capture_with_verified_extras(
            "monitor",
            true,
            &lifecycle,
            &states,
            &resumes,
            |_| Ok(false)
        )
        .is_err());
        let snapshot = ModuleUpdateSnapshot::capture_with_verified_extras(
            "monitor",
            true,
            &lifecycle,
            &states,
            &resumes,
            |_| Ok(true),
        )
        .unwrap();
        assert_eq!(snapshot.active_extra_ids(), BTreeSet::from(["sensor.cpu"]));
        assert!(snapshot.restore_canonical_open());
    }

    #[test]
    fn live_canonical_open_requires_valid_new_governor_open_binding() {
        use crate::lifecycle::{Battleplan, LifecycleContract};
        let mut candidate = LifecycleContract::default();
        assert!(ModuleUpdateSnapshot::verify_candidate_canonical_open(true, &candidate).is_err());
        assert!(ModuleUpdateSnapshot::verify_candidate_canonical_open(false, &candidate).is_ok());
        candidate
            .transitions
            .insert("launch-ui".into(), Battleplan::default());
        candidate
            .hardcoded
            .insert("governor.open".into(), "launch-ui".into());
        assert!(ModuleUpdateSnapshot::verify_candidate_canonical_open(true, &candidate).is_ok());
        candidate
            .hardcoded
            .insert("governor.open".into(), "missing".into());
        assert!(ModuleUpdateSnapshot::verify_candidate_canonical_open(true, &candidate).is_err());
    }

    #[test]
    fn zero_extras_does_not_change_open_semantics() {
        let snapshot = ModuleUpdateSnapshot::capture("simple", true, []).unwrap();
        assert_eq!(snapshot.extra_count(), 0);
        assert!(snapshot.restore_canonical_open());
        assert!(snapshot.verify_new_extras(&BTreeMap::new()).is_ok());
    }
}

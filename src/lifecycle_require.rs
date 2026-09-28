use std::collections::{BTreeMap, BTreeSet};

use crate::lifecycle::{self, LifecycleContract, RequireDeclaration};

/*
 * Lifecycle recursive require resolver.
 *
 * Technical responsibility:
 *
 * root module
 *      -> inspect Lifecycle require declarations
 *      -> recursively inspect every required module
 *      -> detect cycles
 *      -> deduplicate shared branches
 *      -> retain baseline membership
 *      -> produce dependency-first resolution order
 *
 * This layer performs no installation and no filesystem mutation.
 *
 * Existing modules are NOT treated as a reason to stop graph discovery.
 * Baseline membership means only that the Governor must not reacquire
 * that module later.
 *
 * A baseline module may itself declare requirements which still need
 * to be inspected and satisfied.
 *
 * State is represented by membership and graph structure rather than
 * installation/acquisition boolean flags.
 */

pub type RequireGraph = BTreeMap<String, BTreeMap<String, RequireDeclaration>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirePlan {
    root: String,
    baseline: BTreeSet<String>,
    resolved: BTreeSet<String>,
    graph: RequireGraph,
    order: Vec<String>,
}

impl RequirePlan {
    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn baseline(&self) -> &BTreeSet<String> {
        &self.baseline
    }

    pub fn resolved(&self) -> &BTreeSet<String> {
        &self.resolved
    }

    pub fn graph(&self) -> &RequireGraph {
        &self.graph
    }

    pub fn resolution_order(&self) -> &[String] {
        &self.order
    }

    pub fn declaration(&self, parent: &str, required: &str) -> Option<&RequireDeclaration> {
        self.graph
            .get(parent)
            .and_then(|requires| requires.get(required))
    }
}

fn validate_module_id(module_id: &str) -> Result<(), String> {
    if module_id.trim().is_empty() {
        return Err("lifecycle require module id cannot be empty".to_string());
    }

    Ok(())
}

fn cycle_error(module_id: &str, stack: &[String]) -> String {
    let start = stack
        .iter()
        .position(|entry| entry == module_id)
        .unwrap_or(0);

    let mut cycle = stack[start..].to_vec();

    cycle.push(module_id.to_string());

    format!("lifecycle require cycle detected: {}", cycle.join(" -> "))
}

fn visit<F>(
    module_id: &str,
    source: &mut F,
    visiting: &mut BTreeSet<String>,
    resolved: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
    graph: &mut RequireGraph,
    order: &mut Vec<String>,
) -> Result<(), String>
where
    F: FnMut(&str) -> Result<LifecycleContract, String>,
{
    validate_module_id(module_id)?;

    if resolved.contains(module_id) {
        return Ok(());
    }

    if visiting.contains(module_id) {
        return Err(cycle_error(module_id, stack));
    }

    visiting.insert(module_id.to_string());

    stack.push(module_id.to_string());

    let contract = source(module_id).map_err(|error| {
        format!(
            "could not inspect lifecycle require module '{}' while resolving '{}': {}",
            module_id,
            stack.join(" -> "),
            error
        )
    })?;

    lifecycle::validate(&contract).map_err(|error| {
        format!(
            "invalid lifecycle contract for require module '{}': {}",
            module_id, error
        )
    })?;

    let requires = contract.require.clone();

    graph.insert(module_id.to_string(), requires.clone());

    for required_module in requires.keys() {
        visit(
            required_module,
            source,
            visiting,
            resolved,
            stack,
            graph,
            order,
        )?;
    }

    let popped = stack.pop();

    if popped.as_deref() != Some(module_id) {
        return Err(format!(
            "lifecycle require resolver stack corruption while leaving '{module_id}'"
        ));
    }

    visiting.remove(module_id);

    resolved.insert(module_id.to_string());

    order.push(module_id.to_string());

    Ok(())
}

pub fn resolve<F>(
    root: &str,
    baseline: BTreeSet<String>,
    mut source: F,
) -> Result<RequirePlan, String>
where
    F: FnMut(&str) -> Result<LifecycleContract, String>,
{
    validate_module_id(root)?;

    let mut visiting = BTreeSet::new();

    let mut resolved = BTreeSet::new();

    let mut stack = Vec::new();

    let mut graph = RequireGraph::new();

    let mut order = Vec::new();

    visit(
        root,
        &mut source,
        &mut visiting,
        &mut resolved,
        &mut stack,
        &mut graph,
        &mut order,
    )?;

    if !visiting.is_empty() {
        return Err(
            "lifecycle require resolver finished with non-empty visiting state".to_string(),
        );
    }

    if !stack.is_empty() {
        return Err(
            "lifecycle require resolver finished with non-empty traversal stack".to_string(),
        );
    }

    Ok(RequirePlan {
        root: root.to_string(),

        baseline,

        resolved,

        graph,

        order,
    })
}

/*
 * Recursive require transaction.
 *
 * Technical responsibility:
 *
 * RequirePlan
 *      + baseline membership
 *      + acquired membership
 *      + reversible mutation journal
 *      -> safe transaction state
 *
 * No boolean ownership flags are stored.
 *
 * Existing state is represented by baseline membership.
 * Transaction-created state is represented by acquired membership.
 * Rollback authority is represented by journal entries.
 *
 * Journal operation names and data remain opaque strings.
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequireJournalEntry {
    operation: String,
    target: String,
    data: BTreeMap<String, String>,
}

impl RequireJournalEntry {
    pub fn new(
        operation: impl Into<String>,
        target: impl Into<String>,
        data: BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let operation = operation.into();

        let target = target.into();

        if operation.trim().is_empty() {
            return Err("lifecycle require journal operation cannot be empty".to_string());
        }

        validate_module_id(&target)?;

        for key in data.keys() {
            if key.trim().is_empty() {
                return Err("lifecycle require journal data key cannot be empty".to_string());
            }
        }

        Ok(Self {
            operation,
            target,
            data,
        })
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn data(&self) -> &BTreeMap<String, String> {
        &self.data
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequireTransaction {
    plan: RequirePlan,
    acquired: BTreeSet<String>,
    journal: Vec<RequireJournalEntry>,
}

impl RequireTransaction {
    pub fn new(plan: RequirePlan) -> Self {
        Self {
            plan,
            acquired: BTreeSet::new(),
            journal: Vec::new(),
        }
    }

    pub fn plan(&self) -> &RequirePlan {
        &self.plan
    }

    pub fn baseline(&self) -> &BTreeSet<String> {
        self.plan.baseline()
    }

    pub fn acquired(&self) -> &BTreeSet<String> {
        &self.acquired
    }

    pub fn journal(&self) -> &[RequireJournalEntry] {
        &self.journal
    }

    pub fn pending_acquisitions(&self) -> Vec<&str> {
        self.plan
            .resolution_order()
            .iter()
            .filter(|module_id| {
                !self.baseline().contains(module_id.as_str())
                    && !self.acquired.contains(module_id.as_str())
            })
            .map(String::as_str)
            .collect()
    }

    pub fn record_acquisition(
        &mut self,
        module_id: &str,
        operation: impl Into<String>,
        data: BTreeMap<String, String>,
    ) -> Result<(), String> {
        validate_module_id(module_id)?;

        if !self.plan.resolved().contains(module_id) {
            return Err(format!(
                "lifecycle require cannot acquire unresolved module '{module_id}'"
            ));
        }

        if self.baseline().contains(module_id) {
            return Err(format!(
                "lifecycle require baseline module '{module_id}' cannot be recorded as acquired"
            ));
        }

        if self.acquired.contains(module_id) {
            return Err(format!(
                "lifecycle require module '{module_id}' was already acquired by this transaction"
            ));
        }

        let entry = RequireJournalEntry::new(operation, module_id, data)?;

        self.acquired.insert(module_id.to_string());

        self.journal.push(entry);

        Ok(())
    }

    pub fn rollback<F>(&mut self, mut compensate: F) -> Result<(), String>
    where
        F: FnMut(&RequireJournalEntry) -> Result<(), String>,
    {
        let mut restored = BTreeSet::new();

        let mut failures = Vec::new();

        for entry in self.journal.iter().rev() {
            match compensate(entry) {
                Ok(()) => {
                    restored.insert(entry.target().to_string());
                }

                Err(error) => {
                    failures.push(format!(
                        "{} [{}]: {}",
                        entry.target(),
                        entry.operation(),
                        error
                    ));
                }
            }
        }

        for module_id in restored {
            self.acquired.remove(&module_id);
        }

        self.journal
            .retain(|entry| self.acquired.contains(entry.target()));

        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "lifecycle require rollback incomplete: {}",
                failures.join(" | ")
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(required: &[&str]) -> LifecycleContract {
        let mut contract = LifecycleContract::default();

        for module_id in required {
            contract
                .require
                .insert(module_id.to_string(), BTreeMap::new());
        }

        contract
    }

    fn source(
        modules: &BTreeMap<String, LifecycleContract>,
        module_id: &str,
    ) -> Result<LifecycleContract, String> {
        modules
            .get(module_id)
            .cloned()
            .ok_or_else(|| format!("synthetic module '{module_id}' missing"))
    }

    #[test]
    fn empty_require_tree_resolves_root_only() {
        let modules = BTreeMap::from([("A".to_string(), contract(&[]))]);

        let plan = resolve("A", BTreeSet::new(), |module_id| {
            source(&modules, module_id)
        })
        .unwrap();

        assert_eq!(plan.resolution_order(), &["A".to_string()]);

        assert_eq!(plan.resolved().len(), 1);
    }

    #[test]
    fn recursive_chain_is_dependency_first() {
        let modules = BTreeMap::from([
            ("A".to_string(), contract(&["B"])),
            ("B".to_string(), contract(&["C"])),
            ("C".to_string(), contract(&[])),
        ]);

        let plan = resolve("A", BTreeSet::new(), |module_id| {
            source(&modules, module_id)
        })
        .unwrap();

        assert_eq!(
            plan.resolution_order(),
            &["C".to_string(), "B".to_string(), "A".to_string(),]
        );
    }

    #[test]
    fn shared_requirement_is_inspected_once() {
        let modules = BTreeMap::from([
            ("A".to_string(), contract(&["B", "C"])),
            ("B".to_string(), contract(&["D"])),
            ("C".to_string(), contract(&["D"])),
            ("D".to_string(), contract(&[])),
        ]);

        let mut calls = BTreeMap::<String, usize>::new();

        let plan = resolve("A", BTreeSet::new(), |module_id| {
            let count = calls.entry(module_id.to_string()).or_insert(0);

            *count += 1;

            source(&modules, module_id)
        })
        .unwrap();

        assert_eq!(calls.get("D"), Some(&1));

        assert_eq!(plan.resolved().len(), 4);
    }

    #[test]
    fn cycle_reports_complete_require_path() {
        let modules = BTreeMap::from([
            ("A".to_string(), contract(&["B"])),
            ("B".to_string(), contract(&["C"])),
            ("C".to_string(), contract(&["A"])),
        ]);

        let error = resolve("A", BTreeSet::new(), |module_id| {
            source(&modules, module_id)
        })
        .unwrap_err();

        assert!(error.contains("A -> B -> C -> A"));
    }

    #[test]
    fn baseline_membership_does_not_hide_nested_requirements() {
        let modules = BTreeMap::from([
            ("A".to_string(), contract(&["B"])),
            ("B".to_string(), contract(&["C"])),
            ("C".to_string(), contract(&[])),
        ]);

        let baseline = BTreeSet::from([("B".to_string())]);

        let plan = resolve("A", baseline, |module_id| source(&modules, module_id)).unwrap();

        assert!(plan.baseline().contains("B"));

        assert!(plan.resolved().contains("C"));

        assert_eq!(
            plan.resolution_order(),
            &["C".to_string(), "B".to_string(), "A".to_string(),]
        );
    }

    #[test]
    fn declaration_data_remains_opaque() {
        let mut root = contract(&["B"]);

        root.require
            .get_mut("B")
            .unwrap()
            .insert("future.intent".to_string(), "opaque-value".to_string());

        let modules = BTreeMap::from([("A".to_string(), root), ("B".to_string(), contract(&[]))]);

        let plan = resolve("A", BTreeSet::new(), |module_id| {
            source(&modules, module_id)
        })
        .unwrap();

        assert_eq!(
            plan.declaration("A", "B",).unwrap().get("future.intent"),
            Some(&"opaque-value".to_string())
        );
    }

    #[test]
    fn invalid_nested_contract_is_rejected() {
        let mut invalid = LifecycleContract::default();

        invalid.require.insert(
            "B".to_string(),
            BTreeMap::from([("   ".to_string(), "value".to_string())]),
        );

        let modules = BTreeMap::from([("A".to_string(), invalid)]);

        let error = resolve("A", BTreeSet::new(), |module_id| {
            source(&modules, module_id)
        })
        .unwrap_err();

        assert!(error.contains("data key cannot be empty"));
    }

    #[test]
    fn empty_root_is_rejected() {
        let error =
            resolve("   ", BTreeSet::new(), |_| Ok(LifecycleContract::default())).unwrap_err();

        assert!(error.contains("module id cannot be empty"));
    }

    #[test]
    fn transaction_pending_acquisitions_excludes_baseline() {
        let modules = BTreeMap::from([
            ("A".to_string(), contract(&["B"])),
            ("B".to_string(), contract(&["C"])),
            ("C".to_string(), contract(&[])),
        ]);

        let plan = resolve("A", BTreeSet::from(["B".to_string()]), |module_id| {
            source(&modules, module_id)
        })
        .unwrap();

        let transaction = RequireTransaction::new(plan);

        assert_eq!(transaction.pending_acquisitions(), vec!["C", "A",]);
    }

    #[test]
    fn transaction_records_acquired_by_membership() {
        let modules = BTreeMap::from([
            ("A".to_string(), contract(&["B"])),
            ("B".to_string(), contract(&[])),
        ]);

        let plan = resolve("A", BTreeSet::new(), |module_id| {
            source(&modules, module_id)
        })
        .unwrap();

        let mut transaction = RequireTransaction::new(plan);

        transaction
            .record_acquisition(
                "B",
                "future.acquire.operation",
                BTreeMap::from([("opaque.key".to_string(), "opaque.value".to_string())]),
            )
            .unwrap();

        assert!(transaction.acquired().contains("B"));

        assert_eq!(transaction.journal().len(), 1);

        assert_eq!(
            transaction.journal()[0].operation(),
            "future.acquire.operation"
        );

        assert_eq!(
            transaction.journal()[0].data().get("opaque.key"),
            Some(&"opaque.value".to_string())
        );
    }

    #[test]
    fn baseline_module_cannot_be_recorded_as_acquired() {
        let modules = BTreeMap::from([
            ("A".to_string(), contract(&["B"])),
            ("B".to_string(), contract(&[])),
        ]);

        let plan = resolve("A", BTreeSet::from(["B".to_string()]), |module_id| {
            source(&modules, module_id)
        })
        .unwrap();

        let mut transaction = RequireTransaction::new(plan);

        let error = transaction
            .record_acquisition("B", "anything", BTreeMap::new())
            .unwrap_err();

        assert!(error.contains("baseline module"));
    }

    #[test]
    fn duplicate_acquisition_is_rejected() {
        let modules = BTreeMap::from([("A".to_string(), contract(&[]))]);

        let plan = resolve("A", BTreeSet::new(), |module_id| {
            source(&modules, module_id)
        })
        .unwrap();

        let mut transaction = RequireTransaction::new(plan);

        transaction
            .record_acquisition("A", "acquire.one", BTreeMap::new())
            .unwrap();

        let error = transaction
            .record_acquisition("A", "acquire.two", BTreeMap::new())
            .unwrap_err();

        assert!(error.contains("already acquired"));
    }

    #[test]
    fn unresolved_module_cannot_enter_transaction_journal() {
        let modules = BTreeMap::from([("A".to_string(), contract(&[]))]);

        let plan = resolve("A", BTreeSet::new(), |module_id| {
            source(&modules, module_id)
        })
        .unwrap();

        let mut transaction = RequireTransaction::new(plan);

        let error = transaction
            .record_acquisition("Z", "anything", BTreeMap::new())
            .unwrap_err();

        assert!(error.contains("unresolved module"));
    }

    #[test]
    fn rollback_runs_journal_in_reverse_order() {
        let modules = BTreeMap::from([
            ("A".to_string(), contract(&["B"])),
            ("B".to_string(), contract(&["C"])),
            ("C".to_string(), contract(&[])),
        ]);

        let plan = resolve("A", BTreeSet::new(), |module_id| {
            source(&modules, module_id)
        })
        .unwrap();

        let mut transaction = RequireTransaction::new(plan);

        for module_id in ["C", "B", "A"] {
            transaction
                .record_acquisition(module_id, "future.acquire", BTreeMap::new())
                .unwrap();
        }

        let mut rollback_order = Vec::new();

        transaction
            .rollback(|entry| {
                rollback_order.push(entry.target().to_string());

                Ok(())
            })
            .unwrap();

        assert_eq!(rollback_order, vec!["A", "B", "C",]);

        assert!(transaction.acquired().is_empty());

        assert!(transaction.journal().is_empty());
    }

    #[test]
    fn rollback_failure_does_not_stop_other_compensations() {
        let modules = BTreeMap::from([
            ("A".to_string(), contract(&["B"])),
            ("B".to_string(), contract(&[])),
        ]);

        let plan = resolve("A", BTreeSet::new(), |module_id| {
            source(&modules, module_id)
        })
        .unwrap();

        let mut transaction = RequireTransaction::new(plan);

        for module_id in ["B", "A"] {
            transaction
                .record_acquisition(module_id, "future.acquire", BTreeMap::new())
                .unwrap();
        }

        let mut attempted = Vec::new();

        let error = transaction
            .rollback(|entry| {
                attempted.push(entry.target().to_string());

                if entry.target() == "A" {
                    return Err("synthetic rollback failure".to_string());
                }

                Ok(())
            })
            .unwrap_err();

        assert_eq!(attempted, vec!["A", "B",]);

        assert!(error.contains("synthetic rollback failure"));

        assert!(transaction.acquired().contains("A"));

        assert!(!transaction.acquired().contains("B"));

        assert_eq!(transaction.journal().len(), 1);

        assert_eq!(transaction.journal()[0].target(), "A");
    }

    #[test]
    fn journal_rejects_empty_dynamic_operation() {
        let error = RequireJournalEntry::new("   ", "A", BTreeMap::new()).unwrap_err();

        assert!(error.contains("operation cannot be empty"));
    }

    #[test]
    fn journal_rejects_empty_dynamic_data_key() {
        let error = RequireJournalEntry::new(
            "anything",
            "A",
            BTreeMap::from([("   ".to_string(), "value".to_string())]),
        )
        .unwrap_err();

        assert!(error.contains("data key cannot be empty"));
    }
}

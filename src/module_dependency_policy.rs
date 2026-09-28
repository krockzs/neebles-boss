use std::collections::{BTreeMap, BTreeSet};

use crate::lifecycle::LifecycleContract;

/*
 * Installed module dependency policy.
 *
 * Source of truth:
 *
 *     installed module Lifecycle.require
 *
 * This layer derives both directions:
 *
 *     requires
 *     required_by
 *
 * No reverse dependency state is persisted.
 * No UI state is owned here.
 * No module technology is interpreted here.
 */

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstalledDependencyGraph {
    requires: BTreeMap<String, BTreeSet<String>>,

    required_by: BTreeMap<String, BTreeSet<String>>,
}

impl InstalledDependencyGraph {
    pub fn from_contracts(contracts: &BTreeMap<String, LifecycleContract>) -> Result<Self, String> {
        let installed = contracts.keys().cloned().collect::<BTreeSet<_>>();

        let mut requires = BTreeMap::<String, BTreeSet<String>>::new();

        let mut required_by = BTreeMap::<String, BTreeSet<String>>::new();

        for module_id in &installed {
            requires.insert(module_id.clone(), BTreeSet::new());

            required_by.insert(module_id.clone(), BTreeSet::new());
        }

        for (module_id, contract) in contracts {
            for required in contract.require.keys() {
                if !installed.contains(required) {
                    return Err(format!(
                        "installed module '{}' requires module '{}', but '{}' is not installed",
                        module_id, required, required,
                    ));
                }

                requires
                    .get_mut(module_id)
                    .expect(
                        "installed dependency graph requires map must contain every installed module"
                    )
                    .insert(
                        required.clone()
                    );

                required_by
                    .get_mut(required)
                    .expect(
                        "installed dependency graph required_by map must contain every installed module"
                    )
                    .insert(
                        module_id.clone()
                    );
            }
        }

        Ok(Self {
            requires,
            required_by,
        })
    }

    pub fn contains(&self, module_id: &str) -> bool {
        self.requires.contains_key(module_id)
    }

    pub fn requires(&self, module_id: &str) -> Result<&BTreeSet<String>, String> {
        self.requires.get(module_id).ok_or_else(|| {
            format!(
                "module '{}' is not present in installed dependency graph",
                module_id
            )
        })
    }

    pub fn required_by(&self, module_id: &str) -> Result<&BTreeSet<String>, String> {
        self.required_by.get(module_id).ok_or_else(|| {
            format!(
                "module '{}' is not present in installed dependency graph",
                module_id
            )
        })
    }

    pub fn uninstall_blockers(&self, module_id: &str) -> Result<Vec<String>, String> {
        Ok(self.required_by(module_id)?.iter().cloned().collect())
    }

    pub fn can_uninstall(&self, module_id: &str) -> Result<(), String> {
        let blockers = self.uninstall_blockers(module_id)?;

        if blockers.is_empty() {
            return Ok(());
        }

        Err(format!(
            "module '{}' cannot be uninstalled because it is required by installed modules: {}",
            module_id,
            blockers.join(", "),
        ))
    }

    /*
     * Disable direction:
     *
     *     required module
     *          ↑
     *     dependent
     *          ↑
     *     dependent
     *
     * Dependents are returned before the requested
     * module so callers can stop consumers before
     * stopping what they require.
     */
    pub fn disable_order(&self, module_id: &str) -> Result<Vec<String>, String> {
        if !self.contains(module_id) {
            return Err(format!(
                "module '{}' is not present in installed dependency graph",
                module_id
            ));
        }

        let mut visited = BTreeSet::new();

        let mut order = Vec::new();

        self.visit_disable(module_id, &mut visited, &mut order)?;

        Ok(order)
    }

    fn visit_disable(
        &self,
        module_id: &str,
        visited: &mut BTreeSet<String>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
        if !visited.insert(module_id.to_string()) {
            return Ok(());
        }

        for dependent in self.required_by(module_id)? {
            self.visit_disable(dependent, visited, order)?;
        }

        order.push(module_id.to_string());

        Ok(())
    }

    /*
     * Enable direction:
     *
     *     requested module
     *          ↓
     *      requirement
     *          ↓
     *      requirement
     *
     * Requirements are returned first.
     */
    pub fn enable_order(&self, module_id: &str) -> Result<Vec<String>, String> {
        if !self.contains(module_id) {
            return Err(format!(
                "module '{}' is not present in installed dependency graph",
                module_id
            ));
        }

        let mut visiting = BTreeSet::new();

        let mut visited = BTreeSet::new();

        let mut order = Vec::new();

        self.visit_enable(module_id, &mut visiting, &mut visited, &mut order)?;

        Ok(order)
    }

    fn visit_enable(
        &self,
        module_id: &str,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
        if visited.contains(module_id) {
            return Ok(());
        }

        if !visiting.insert(module_id.to_string()) {
            return Err(format!(
                "installed dependency graph contains a require cycle involving '{}'",
                module_id
            ));
        }

        for required in self.requires(module_id)? {
            self.visit_enable(required, visiting, visited, order)?;
        }

        visiting.remove(module_id);

        visited.insert(module_id.to_string());

        order.push(module_id.to_string());

        Ok(())
    }
}

/*
 * Execute one already-resolved functional state plan.
 *
 * The graph decides order.
 * The caller owns the actual module mutation.
 *
 * Only modules that really changed are compensated.
 * Compensation always runs in reverse order and continues
 * even if one compensation itself fails.
 */
pub fn execute_state_plan_with_compensation<Apply, Compensate>(
    plan: &[String],
    mut apply: Apply,
    mut compensate: Compensate,
) -> Result<Vec<String>, String>
where
    Apply: FnMut(&str) -> Result<bool, String>,

    Compensate: FnMut(&str) -> Result<(), String>,
{
    let mut changed = Vec::<String>::new();

    for module_id in plan {
        match apply(module_id) {
            Ok(true) => {
                changed.push(module_id.clone());
            }

            Ok(false) => {}

            Err(error) => {
                let mut compensation_errors = Vec::<String>::new();

                for changed_module in changed.iter().rev() {
                    if let Err(compensation_error) = compensate(changed_module) {
                        compensation_errors
                            .push(format!("{}: {}", changed_module, compensation_error,));
                    }
                }

                if compensation_errors.is_empty() {
                    return Err(
                        format!(
                            "module state transaction failed at '{}': {}; completed changes were compensated",
                            module_id,
                            error,
                        )
                    );
                }

                return Err(
                    format!(
                        "CRITICAL: module state transaction failed at '{}': {}; compensation also failed: {}",
                        module_id,
                        error,
                        compensation_errors.join(" | "),
                    )
                );
            }
        }
    }

    Ok(changed)
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

    fn graph(declarations: &[(&str, &[&str])]) -> InstalledDependencyGraph {
        let contracts = declarations
            .iter()
            .map(|(module_id, required)| (module_id.to_string(), contract(required)))
            .collect::<BTreeMap<_, _>>();

        InstalledDependencyGraph::from_contracts(&contracts).unwrap()
    }

    #[test]
    fn reverse_relation_is_derived_from_requires() {
        let graph = graph(&[("A", &["B"]), ("B", &[])]);

        assert_eq!(
            graph
                .required_by("B")
                .unwrap()
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["A".to_string()]
        );
    }

    #[test]
    fn shared_requirement_has_many_dependents() {
        let graph = graph(&[("A", &["B"]), ("C", &["B"]), ("B", &[])]);

        assert_eq!(
            graph.uninstall_blockers("B").unwrap(),
            vec!["A".to_string(), "C".to_string(),]
        );
    }

    #[test]
    fn uninstall_is_allowed_when_nobody_requires_module() {
        let graph = graph(&[("A", &["B"]), ("B", &[])]);

        graph.can_uninstall("A").unwrap();
    }

    #[test]
    fn uninstall_is_blocked_when_module_is_required() {
        let graph = graph(&[("A", &["B"]), ("B", &[])]);

        let error = graph.can_uninstall("B").unwrap_err();

        assert!(error.contains("required by installed modules"));

        assert!(error.contains("A"));
    }

    #[test]
    fn disable_order_is_recursive_dependents_first() {
        let graph = graph(&[("C", &["A"]), ("A", &["B"]), ("B", &[])]);

        assert_eq!(
            graph.disable_order("B").unwrap(),
            vec!["C".to_string(), "A".to_string(), "B".to_string(),]
        );
    }

    #[test]
    fn disable_deduplicates_shared_dependents() {
        let graph = graph(&[
            ("TOP", &["A", "C"]),
            ("A", &["B"]),
            ("C", &["B"]),
            ("B", &[]),
        ]);

        let order = graph.disable_order("B").unwrap();

        let unique = order.iter().cloned().collect::<BTreeSet<_>>();

        assert_eq!(order.len(), unique.len());

        assert_eq!(order.last().map(String::as_str), Some("B"));
    }

    #[test]
    fn enable_order_is_dependency_first() {
        let graph = graph(&[("A", &["B"]), ("B", &["C"]), ("C", &[])]);

        assert_eq!(
            graph.enable_order("A").unwrap(),
            vec!["C".to_string(), "B".to_string(), "A".to_string(),]
        );
    }

    #[test]
    fn missing_required_installed_module_is_rejected() {
        let contracts = BTreeMap::from([("A".to_string(), contract(&["B"]))]);

        let error = InstalledDependencyGraph::from_contracts(&contracts).unwrap_err();

        assert!(error.contains("is not installed"));
    }

    #[test]
    fn state_plan_executes_in_declared_order() {
        let plan = vec!["C".to_string(), "A".to_string(), "B".to_string()];

        let mut applied = Vec::<String>::new();

        let changed = execute_state_plan_with_compensation(
            &plan,
            |module_id| {
                applied.push(module_id.to_string());

                Ok(true)
            },
            |_| Ok(()),
        )
        .unwrap();

        assert_eq!(applied, plan);

        assert_eq!(changed, plan);
    }

    #[test]
    fn state_plan_does_not_compensate_unchanged_modules() {
        let plan = vec!["dependency".to_string(), "root".to_string()];

        let mut compensated = Vec::<String>::new();

        let error = execute_state_plan_with_compensation(
            &plan,
            |module_id| {
                if module_id == "dependency" {
                    return Ok(false);
                }

                Err("boom".to_string())
            },
            |module_id| {
                compensated.push(module_id.to_string());

                Ok(())
            },
        )
        .unwrap_err();

        assert!(error.contains("module state transaction failed"));

        assert!(compensated.is_empty());
    }

    #[test]
    fn state_plan_compensates_changed_modules_in_reverse_order() {
        let plan = vec![
            "dependency-a".to_string(),
            "dependency-b".to_string(),
            "root".to_string(),
        ];

        let mut compensated = Vec::<String>::new();

        let error = execute_state_plan_with_compensation(
            &plan,
            |module_id| {
                if module_id == "root" {
                    return Err("root failed".to_string());
                }

                Ok(true)
            },
            |module_id| {
                compensated.push(module_id.to_string());

                Ok(())
            },
        )
        .unwrap_err();

        assert!(error.contains("completed changes were compensated"));

        assert_eq!(
            compensated,
            vec!["dependency-b".to_string(), "dependency-a".to_string(),]
        );
    }

    #[test]
    fn state_plan_continues_compensation_after_compensation_failure() {
        let plan = vec!["one".to_string(), "two".to_string(), "three".to_string()];

        let mut compensated = Vec::<String>::new();

        let error = execute_state_plan_with_compensation(
            &plan,
            |module_id| {
                if module_id == "three" {
                    return Err("apply failed".to_string());
                }

                Ok(true)
            },
            |module_id| {
                compensated.push(module_id.to_string());

                if module_id == "two" {
                    return Err("rollback failed".to_string());
                }

                Ok(())
            },
        )
        .unwrap_err();

        assert!(error.contains("CRITICAL"));

        assert_eq!(compensated, vec!["two".to_string(), "one".to_string(),]);
    }

    #[test]
    fn arbitrary_module_names_are_preserved() {
        let graph = graph(&[
            ("whatever.future.module", &["dependency.strange.name"]),
            ("dependency.strange.name", &[]),
        ]);

        assert_eq!(
            graph.uninstall_blockers("dependency.strange.name").unwrap(),
            vec!["whatever.future.module".to_string()]
        );
    }
}

use std::collections::{BTreeMap, BTreeSet};

use crate::lifecycle_battlefield::Battlefield;
use crate::lifecycle_human::{self, HumanMessageTemplate, ResolvedHumanMessage};
use crate::lifecycle_require::RequirePlan;

/*
 * Human context for recursive Lifecycle require resolution.
 *
 * This layer does not present UI and does not emit transport events.
 *
 * It supplies structured dynamic information to the existing
 * Lifecycle Human Communication resolver.
 *
 * Context is represented as String:String data.
 * No installation-state booleans are introduced.
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequireHumanContext {
    values: BTreeMap<String, String>,
}

fn collect_paths(
    graph: &crate::lifecycle_require::RequireGraph,
    current: &str,
    target: &str,
    path: &mut Vec<String>,
    paths: &mut Vec<Vec<String>>,
) {
    path.push(current.to_string());

    if current == target {
        paths.push(path.clone());

        path.pop();

        return;
    }

    if let Some(children) = graph.get(current) {
        for child in children.keys() {
            if path.contains(child) {
                continue;
            }

            collect_paths(graph, child, target, path, paths);
        }
    }

    path.pop();
}

fn parents_for(plan: &RequirePlan, module_id: &str) -> BTreeSet<String> {
    plan.graph()
        .iter()
        .filter_map(|(parent, children)| {
            if children.contains_key(module_id) {
                Some(parent.clone())
            } else {
                None
            }
        })
        .collect()
}

fn paths_for(plan: &RequirePlan, module_id: &str) -> Vec<Vec<String>> {
    let mut paths = Vec::new();

    let mut current = Vec::new();

    collect_paths(
        plan.graph(),
        plan.root(),
        module_id,
        &mut current,
        &mut paths,
    );

    paths
}

impl RequireHumanContext {
    pub fn new(plan: &RequirePlan, module_id: &str, action: &str) -> Result<Self, String> {
        if module_id.trim().is_empty() {
            return Err("require human current module cannot be empty".to_string());
        }

        if action.trim().is_empty() {
            return Err("require human action cannot be empty".to_string());
        }

        if !plan.resolved().contains(module_id) {
            return Err(format!(
                "require human context references unresolved module '{module_id}'"
            ));
        }

        let parents = parents_for(plan, module_id);

        let paths = paths_for(plan, module_id);

        if paths.is_empty() {
            return Err(format!(
                "require human context could not derive path from '{}' to '{}'",
                plan.root(),
                module_id
            ));
        }

        let depth = paths
            .iter()
            .map(|path| path.len().saturating_sub(1))
            .min()
            .unwrap_or_default();

        let path_texts = paths
            .iter()
            .map(|path| path.join(" -> "))
            .collect::<Vec<_>>();

        let required_by = parents.iter().cloned().collect::<Vec<_>>().join(", ");

        let mut values = BTreeMap::new();

        values.insert("root".to_string(), plan.root().to_string());

        values.insert("current".to_string(), module_id.to_string());

        values.insert("action".to_string(), action.to_string());

        if !required_by.is_empty() {
            values.insert("required_by".to_string(), required_by);
        }

        values.insert("path".to_string(), path_texts[0].clone());

        values.insert("paths".to_string(), path_texts.join(" | "));

        values.insert("depth".to_string(), depth.to_string());

        Ok(Self { values })
    }

    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn apply(&self, battlefield: &mut Battlefield) {
        battlefield.clear_namespace("require");

        for (key, value) in &self.values {
            battlefield.insert("require", key.clone(), value.clone());
        }
    }

    pub fn resolve(
        &self,
        template: &HumanMessageTemplate,
        battlefield: &Battlefield,
    ) -> Result<ResolvedHumanMessage, String> {
        let mut scoped = battlefield.clone();

        self.apply(&mut scoped);

        lifecycle_human::resolve(template, &scoped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle::{LifecycleContract, RequireDeclaration};

    fn contract(required: &[&str]) -> LifecycleContract {
        let mut contract = LifecycleContract::default();

        for module_id in required {
            contract
                .require
                .insert(module_id.to_string(), RequireDeclaration::new());
        }

        contract
    }

    fn plan(modules: BTreeMap<String, LifecycleContract>) -> RequirePlan {
        crate::lifecycle_require::resolve("A", BTreeSet::new(), |module_id| {
            modules
                .get(module_id)
                .cloned()
                .ok_or_else(|| format!("missing synthetic module '{module_id}'"))
        })
        .unwrap()
    }

    fn reference(namespace: &str, path: &str) -> String {
        let marker = char::from_u32(36).unwrap();

        format!("{marker}{namespace}.{path}")
    }

    #[test]
    fn chain_context_exposes_complete_require_path() {
        let plan = plan(BTreeMap::from([
            ("A".to_string(), contract(&["B"])),
            ("B".to_string(), contract(&["C"])),
            ("C".to_string(), contract(&[])),
        ]));

        let context = RequireHumanContext::new(&plan, "C", "publishing").unwrap();

        assert_eq!(context.get("root"), Some("A"));

        assert_eq!(context.get("current"), Some("C"));

        assert_eq!(context.get("required_by"), Some("B"));

        assert_eq!(context.get("path"), Some("A -> B -> C"));

        assert_eq!(context.get("depth"), Some("2"));

        assert_eq!(context.get("action"), Some("publishing"));
    }

    #[test]
    fn shared_requirement_exposes_all_parents_and_paths() {
        let plan = plan(BTreeMap::from([
            ("A".to_string(), contract(&["B", "C"])),
            ("B".to_string(), contract(&["D"])),
            ("C".to_string(), contract(&["D"])),
            ("D".to_string(), contract(&[])),
        ]));

        let context = RequireHumanContext::new(&plan, "D", "resolving").unwrap();

        assert_eq!(context.get("required_by"), Some("B, C"));

        assert_eq!(context.get("paths"), Some("A -> B -> D | A -> C -> D"));

        assert_eq!(context.get("depth"), Some("2"));
    }

    #[test]
    fn root_context_has_zero_depth() {
        let plan = plan(BTreeMap::from([("A".to_string(), contract(&[]))]));

        let context = RequireHumanContext::new(&plan, "A", "complete").unwrap();

        assert_eq!(context.get("path"), Some("A"));

        assert_eq!(context.get("depth"), Some("0"));

        assert_eq!(context.get("required_by"), None);
    }

    #[test]
    fn context_resolves_through_existing_human_speaker() {
        let plan = plan(BTreeMap::from([
            ("A".to_string(), contract(&["B"])),
            ("B".to_string(), contract(&[])),
        ]));

        let context = RequireHumanContext::new(&plan, "B", "publishing").unwrap();

        let current = reference("require", "current");

        let path = reference("require", "path");

        let action = reference("require", "action");

        let template = HumanMessageTemplate::new(format!("Processing {current}"))
            .unwrap()
            .detail(format!("{action}: {path}"))
            .unwrap();

        let resolved = context.resolve(&template, &Battlefield::new()).unwrap();

        assert_eq!(resolved.message(), "Processing B");

        assert_eq!(resolved.detail(), Some("publishing: A -> B"));
    }

    #[test]
    fn require_context_does_not_destroy_other_battlefield_namespaces() {
        let plan = plan(BTreeMap::from([("A".to_string(), contract(&[]))]));

        let context = RequireHumanContext::new(&plan, "A", "complete").unwrap();

        let mut battlefield = Battlefield::new();

        battlefield.insert("runtime", "example", "kept");

        let runtime = reference("runtime", "example");

        let current = reference("require", "current");

        let template = HumanMessageTemplate::new(format!("{runtime} {current}")).unwrap();

        let resolved = context.resolve(&template, &battlefield).unwrap();

        assert_eq!(resolved.message(), "kept A");

        assert_eq!(battlefield.get("runtime", "example",), Some("kept"));

        assert!(!battlefield.contains_namespace("require"));
    }

    #[test]
    fn arbitrary_action_vocabulary_is_preserved() {
        let plan = plan(BTreeMap::from([("A".to_string(), contract(&[]))]));

        let context =
            RequireHumanContext::new(&plan, "A", "future.operation.without.hardcoded.vocabulary")
                .unwrap();

        assert_eq!(
            context.get("action"),
            Some("future.operation.without.hardcoded.vocabulary")
        );
    }

    #[test]
    fn unresolved_module_is_rejected() {
        let plan = plan(BTreeMap::from([("A".to_string(), contract(&[]))]));

        let error = RequireHumanContext::new(&plan, "Z", "anything").unwrap_err();

        assert!(error.contains("unresolved module"));
    }

    #[test]
    fn empty_action_is_rejected() {
        let plan = plan(BTreeMap::from([("A".to_string(), contract(&[]))]));

        let error = RequireHumanContext::new(&plan, "A", "   ").unwrap_err();

        assert!(error.contains("action cannot be empty"));
    }
}

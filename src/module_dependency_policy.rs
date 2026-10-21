use std::collections::{BTreeMap, BTreeSet};

use crate::lifecycle::LifecycleContract;

/*
 * Installed module dependency policy.
 *
 * Canonical source: each installed consumer's authenticated depends_install
 * snapshot (to be wired at its transactional install/update boundary).
 *
 * The legacy Lifecycle.require adapter below remains temporarily in use by
 * existing callers until F02C wires the installed dependency snapshot. It is
 * NOT the canonical future authority.
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

/*
 * Boss-owned installed requirement snapshot: a COPY of the immutable Registry
 * selection captured in the candidate staging directory at Install/Update.
 * The Registry remains the declarative source; this is installed-version truth.
 * This metadata is NEVER updated when a provider publishes variables.
 *
 * The snapshot travels with the module tree through the existing filesystem
 * rename/rollback transaction. Reading a missing snapshot from an old module
 * is a separate migration matter; never synthesize one from current Registry.
 */
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub const INSTALLED_REQUIREMENTS_FILENAME: &str = ".neebles-boss-installed-requirements.json";

/*
 * Boss-owned snapshots contain public Registry selections, not vault secrets.
 * Only Boss/root may WRITE them. Desktop users must READ them for the catalog,
 * the installed dependency graph, and fail-closed local action preflights.
 */
const SNAPSHOT_READ_ONLY_MODE: u32 = 0o644;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstalledRequirementsSnapshot {
    pub schema: u32,
    pub module: String,
    pub version: String,
    pub source_commit: String,
    pub short_name: String,
    pub depends_install: BTreeMap<String, BTreeMap<String, String>>,
}

impl InstalledRequirementsSnapshot {
    pub fn from_registry(
        module: &str,
        entry: &crate::modules::RegistryModule,
    ) -> Result<Self, String> {
        let snapshot = Self {
            schema: 1,
            module: module.to_string(),
            version: entry.version.clone(),
            source_commit: entry.commit.clone(),
            short_name: entry.short_name.clone(),
            depends_install: entry
                .depends_install
                .iter()
                .map(|(id, requirement)| (id.clone(), requirement.variables.clone()))
                .collect(),
        };
        snapshot.validate(module, &entry.version)?;
        Ok(snapshot)
    }

    pub fn validate(&self, module: &str, installed_version: &str) -> Result<(), String> {
        if self.schema != 1 || self.module != module || self.version != installed_version {
            return Err(format!(
                "installed requirements owner/schema/version mismatch for '{module}'"
            ));
        }
        if !neebles_backend::module_material::valid_module_id(module)
            || self.version.is_empty()
            || self.source_commit.len() != 40
            || !self.source_commit.bytes().all(|byte| byte.is_ascii_hexdigit())
            || self.short_name.is_empty()
            || self.short_name.len() > 256
            || self.short_name.chars().any(|ch| ch.is_control())
            || self.depends_install.len() > 128
        {
            return Err(format!("invalid installed requirements metadata for '{module}'"));
        }
        for (provider, variables) in &self.depends_install {
            if !neebles_backend::module_material::valid_module_id(provider)
                || provider == module
                || variables.len() > 128
                || variables.iter().any(|(key, value)| {
                    key.is_empty() || key.len() > 512 || key.chars().any(|ch| ch.is_control())
                        || value.is_empty() || value.len() > 2048
                        || value.chars().any(|ch| ch.is_control())
                })
            {
                return Err(format!(
                    "invalid installed requirements from '{module}' to '{provider}'"
                ));
            }
        }
        Ok(())
    }

    pub fn provider_ids(&self) -> BTreeSet<String> {
        self.depends_install.keys().cloned().collect()
    }
}

pub fn stage_installed_requirements(
    candidate_dir: &Path,
    module: &str,
    entry: &crate::modules::RegistryModule,
) -> Result<(), String> {
    let snapshot = InstalledRequirementsSnapshot::from_registry(module, entry)?;
    let dir_meta = fs::symlink_metadata(candidate_dir)
        .map_err(|error| format!("invalid module staging directory: {error}"))?;
    if !dir_meta.file_type().is_dir() {
        return Err("module staging must be a real directory".to_string());
    }
    let payload = serde_json::to_vec_pretty(&snapshot)
        .map_err(|error| format!("could not encode installed requirements: {error}"))?;
    if payload.len() > 65536 {
        return Err("installed requirements exceed 64KiB".to_string());
    }
    let path = candidate_dir.join(INSTALLED_REQUIREMENTS_FILENAME);
    // create_new refuses to overwrite a source-controlled or existing file.
    // A module must never be able to supply this Boss-owned metadata itself.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|error| format!("cannot create Boss-owned requirements file: {error}"))?;
    file.write_all(&payload)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("cannot persist staged requirements: {error}"))?;

    // Create privately (0600), then publish only after fully writing the
    // authenticated Boss-owned snapshot. Explicitly set the final mode:
    // OpenOptions::mode alone is filtered by a restrictive process umask.
    file.set_permissions(fs::Permissions::from_mode(SNAPSHOT_READ_ONLY_MODE))
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("cannot publish readable requirements: {error}"))?;
    drop(file);
    let verified = read_installed_requirements(candidate_dir, module, &entry.version)?;
    if verified != snapshot {
        return Err("staged requirements changed during verification".to_string());
    }
    Ok(())
}

pub fn read_installed_requirements(
    module_dir: &Path,
    module: &str,
    version: &str,
) -> Result<InstalledRequirementsSnapshot, String> {
    let path = module_dir.join(INSTALLED_REQUIREMENTS_FILENAME);
    let meta = fs::symlink_metadata(&path)
        .map_err(|error| format!("installed requirements unavailable for '{module}': {error}"))?;
    if !meta.file_type().is_file() || meta.len() > 65536 {
        return Err(format!("invalid installed requirements file for '{module}'"));
    }
    let directory = fs::symlink_metadata(module_dir)
        .map_err(|error| format!("cannot inspect installed module directory: {error}"))?;
    if !directory.file_type().is_dir()
        || directory.uid() != meta.uid()
        || meta.mode() & 0o022 != 0
        || meta.mode() & 0o111 != 0
    {
        return Err(format!("unsafe owner or mode for installed requirements of '{module}'"));
    }
    let raw = fs::read(&path)
        .map_err(|error| format!("cannot read installed requirements for '{module}': {error}"))?;
    if raw.len() > 65536 {
        return Err("installed requirements exceed 64KiB".to_string());
    }
    let snapshot: InstalledRequirementsSnapshot = serde_json::from_slice(&raw)
        .map_err(|error| format!("malformed installed requirements for '{module}': {error}"))?;
    snapshot.validate(module, version)?;
    Ok(snapshot)
}


/*
 * Explicit 2.0.4 -> 2.0.5 access migration. NEVER invent legacy snapshots.
 * Validate ALL owner/identity/version/mode facts before changing any mode.
 * No KDBX, module settings, registry or payload contents are modified.
 */
pub fn migrate_readonly_access(
    installed: &BTreeMap<String, (std::path::PathBuf, String)>,
) -> Result<usize, String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("requirements access migration requires authenticated root".to_string());
    }
    let root = Path::new("/opt/neebles/modules");
    let mut candidates = Vec::new();
    for (module, (directory, version)) in installed {
        if directory.parent() != Some(root) || directory.file_name() != Some(std::ffi::OsStr::new(module.as_str())) {
            return Err(format!("unexpected installed module path for '{module}'"));
        }
        let parent = fs::symlink_metadata(directory)
            .map_err(|error| format!("module '{module}' directory metadata: {error}"))?;
        if !parent.file_type().is_dir() || parent.uid() != 0 {
            return Err(format!("module '{module}' is not in a root-owned real directory"));
        }
        let path = directory.join(INSTALLED_REQUIREMENTS_FILENAME);
        let meta = fs::symlink_metadata(&path)
            .map_err(|error| format!("module '{module}' snapshot metadata: {error}"))?;
        let mode = meta.mode() & 0o777;
        if !meta.file_type().is_file()
            || meta.uid() != 0
            || (mode != 0o600 && mode != SNAPSHOT_READ_ONLY_MODE)
        {
            return Err(format!("module '{module}' has an unexpected snapshot owner or mode"));
        }
        let _authenticated = read_installed_requirements(directory, module, version)?;
        if mode == 0o600 {
            candidates.push(path);
        }
    }
    for path in &candidates {
        // root-owned parents are not writable by an unprivileged desktop user.
        fs::set_permissions(path, fs::Permissions::from_mode(SNAPSHOT_READ_ONLY_MODE))
            .map_err(|error| format!("cannot migrate {}: {error}", path.display()))?;
    }
    Ok(candidates.len())
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstalledDependencyGraph {
    requires: BTreeMap<String, BTreeSet<String>>,

    required_by: BTreeMap<String, BTreeSet<String>>,
}

impl InstalledDependencyGraph {
    /*
     * Generic graph input: one immutable dependency-ID set for every installed
     * consumer, obtained from its authenticated installed snapshot.
     *
     * The map includes installed modules with no dependencies (empty set).
     * Live variable status NEVER changes this graph.
     */
    pub fn from_installed_requirements(
        requires: BTreeMap<String, BTreeSet<String>>,
    ) -> Result<Self, String> {
        let installed = requires.keys().cloned().collect::<BTreeSet<_>>();
        let mut required_by = installed
            .iter()
            .map(|module_id| (module_id.clone(), BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();

        for (consumer, providers) in &requires {
            for provider in providers {
                if !installed.contains(provider) {
                    return Err(format!(
                        "installed module '{}' requires module '{}', but '{}' is not installed",
                        consumer, provider, provider
                    ));
                }
                if consumer == provider {
                    return Err(format!("installed module '{}' depends on itself", consumer));
                }
                required_by
                    .get_mut(provider)
                    .expect("all provider IDs were validated")
                    .insert(consumer.clone());
            }
        }

        let graph = Self { requires, required_by };
        // A circular installed graph must fail closed instead of deadlocking
        // a caller that later needs a dependency traversal.
        for module_id in &installed {
            graph.enable_order(module_id)?;
        }
        Ok(graph)
    }

    /*
     * Production installed graph reader. Only the installed module tree is
     * consulted, never the remote Registry or live variables. The snapshot
     * belongs to the same installed version as the module manifest.
     *
     * Old installations do not have this file. They fail closed here;
     * a separate audited migration must establish trustworthy history,
     * never silently synthesize dependencies from today's remote Registry.
     */
    pub fn from_installed_snapshot_paths(
        modules: &BTreeMap<String, (std::path::PathBuf, String)>,
    ) -> Result<Self, String> {
        let mut requirements = BTreeMap::new();
        for (module, (directory, version)) in modules {
            let dir_meta = fs::symlink_metadata(directory).map_err(|error| {
                format!("cannot inspect installed module '{}' directory: {}", module, error)
            })?;
            if !dir_meta.file_type().is_dir() {
                return Err(format!(
                    "installed module '{}' directory is not a real directory", module
                ));
            }
            let snapshot = read_installed_requirements(directory, module, version).map_err(|error| {
                format!(
                    "module '{}' has no valid installed dependency snapshot: {}; migration is required before changing installed dependencies",
                    module, error
                )
            })?;
            if requirements.insert(module.clone(), snapshot.provider_ids()).is_some() {
                return Err(format!("duplicate installed module identity '{}'", module));
            }
        }
        Self::from_installed_requirements(requirements)
    }

    /*
     * Legacy construction path, retained only until callers are migrated.
     * It must NOT be used to authorize new Registry-based dependencies.
     */
    pub fn from_contracts(contracts: &BTreeMap<String, LifecycleContract>) -> Result<Self, String> {
        let requires = contracts
            .iter()
            .map(|(module_id, lifecycle)| {
                (
                    module_id.clone(),
                    lifecycle.require.keys().cloned().collect::<BTreeSet<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        Self::from_installed_requirements(requires)
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

    /* A provider required by ANY installed consumer cannot be disabled. */
    pub fn can_disable(&self, module_id: &str) -> Result<(), String> {
        let blockers = self.uninstall_blockers(module_id)?;
        if blockers.is_empty() {
            return Ok(());
        }
        Err(format!(
            "module '{}' cannot be disabled because it is required by installed modules: {}",
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
 * Pure Registry install-requirement evaluator.
 *
 * "expected" comes ONLY from declared Registry depends_install.
 * "observed" comes ONLY from authenticated live module publications.
 * "installed" comes ONLY from the installed module inventory.
 *
 * Keys and values are opaque strings. This function never interprets
 * master keys, vaults, databases, services, booleans or semantic names.
 * No UI text or authorization state is persisted in Registry.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallRequirementFailure {
    ProviderNotInstalled { provider: String },
    VariableNotPublished { provider: String, variable: String },
    VariableDoesNotMatch { provider: String, variable: String },
}

pub fn unmet_install_requirements(
    expected: &BTreeMap<String, BTreeMap<String, String>>,
    installed: &BTreeSet<String>,
    observed: &BTreeMap<String, BTreeMap<String, String>>,
) -> Vec<InstallRequirementFailure> {
    let mut failures = Vec::new();
    for (provider, required_variables) in expected {
        if !installed.contains(provider) {
            failures.push(InstallRequirementFailure::ProviderNotInstalled {
                provider: provider.clone(),
            });
            continue;
        }
        for (variable, expected_value) in required_variables {
            match observed.get(provider).and_then(|published| published.get(variable)) {
                None => failures.push(InstallRequirementFailure::VariableNotPublished {
                    provider: provider.clone(),
                    variable: variable.clone(),
                }),
                Some(actual_value) if actual_value != expected_value => {
                    failures.push(InstallRequirementFailure::VariableDoesNotMatch {
                        provider: provider.clone(),
                        variable: variable.clone(),
                    });
                }
                Some(_) => {}
            }
        }
    }
    failures
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

    fn f02f_fixture(
        root: &std::path::Path,
        module: &str,
        version: &str,
        dependencies: &[&str],
    ) -> (std::path::PathBuf, String) {
        let directory = root.join(module);
        std::fs::create_dir(&directory).unwrap();
        let entry = crate::modules::RegistryModule {
            repo: format!("https://github.com/example/{module}.git"),
            commit: "a".repeat(40),
            version: version.into(),
            folder: None,
            short_name: module.into(),
            depends_install: dependencies.iter().map(|provider| (
                (*provider).to_string(),
                crate::modules::RegistryInstallDependency {
                    variables: BTreeMap::from([
                        ("first".into(), "one".into()),
                        ("second".into(), "two".into()),
                    ]),
                }
            )).collect(),
        };
        stage_installed_requirements(&directory, module, &entry).unwrap();
        (directory, version.to_string())
    }

    #[test]
    fn f02f_installed_snapshots_protect_multiple_consumers_without_remote_registry() {
        let root = tempfile::tempdir().unwrap();
        let modules = BTreeMap::from([
            ("neebles-secrets".to_string(),
                f02f_fixture(root.path(), "neebles-secrets", "1.0.0", &[])),
            ("neebles-connect".to_string(),
                f02f_fixture(root.path(), "neebles-connect", "1.0.0", &["neebles-secrets"])),
            ("neebles-db".to_string(),
                f02f_fixture(root.path(), "neebles-db", "2.0.0", &["neebles-secrets"])),
        ]);
        let graph = InstalledDependencyGraph::from_installed_snapshot_paths(&modules).unwrap();
        assert_eq!(graph.uninstall_blockers("neebles-secrets").unwrap(), vec![
            "neebles-connect".to_string(), "neebles-db".to_string(),
        ]);
        assert!(graph.can_disable("neebles-secrets").is_err());
        assert!(graph.can_uninstall("neebles-secrets").is_err());
        graph.can_disable("neebles-connect").unwrap();
    }

    #[test]
    fn f02f_missing_or_invalid_snapshot_fails_closed() {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("legacy-module");
        std::fs::create_dir(&legacy).unwrap();
        let modules = BTreeMap::from([
            ("legacy-module".to_string(), (legacy.clone(), "1.0.0".to_string())),
        ]);
        let error = InstalledDependencyGraph::from_installed_snapshot_paths(&modules).unwrap_err();
        assert!(error.contains("migration is required"));
        // A malformed snapshot is also a hard failure rather than empty deps.
        std::fs::write(legacy.join(INSTALLED_REQUIREMENTS_FILENAME), b"{}").unwrap();
        assert!(InstalledDependencyGraph::from_installed_snapshot_paths(&modules).is_err());
    }

    #[test]
    fn f02f_missing_provider_in_snapshot_is_not_silently_ignored() {
        let root = tempfile::tempdir().unwrap();
        let modules = BTreeMap::from([
            ("neebles-connect".to_string(),
                f02f_fixture(root.path(), "neebles-connect", "1.0.0", &["neebles-secrets"])),
        ]);
        let error = InstalledDependencyGraph::from_installed_snapshot_paths(&modules).unwrap_err();
        assert!(error.contains("is not installed"));
    }

    #[test]
    fn f02f_snapshot_version_must_equal_installed_manifest_version() {
        let root = tempfile::tempdir().unwrap();
        let (directory, _) = f02f_fixture(root.path(), "neebles-connect", "1.0.0", &[]);
        let modules = BTreeMap::from([
            ("neebles-connect".to_string(), (directory, "1.0.1".to_string())),
        ]);
        assert!(InstalledDependencyGraph::from_installed_snapshot_paths(&modules).is_err());
    }

    #[test]
    fn f02e_snapshot_preserves_registry_variables_and_short_name() {
        let entry = crate::modules::RegistryModule {
            repo: "https://github.com/example/connect.git".into(),
            commit: "a".repeat(40),
            version: "1.2.3".into(),
            folder: None,
            short_name: "Connect".into(),
            depends_install: BTreeMap::from([(
                "neebles-secrets".into(),
                crate::modules::RegistryInstallDependency {
                    variables: BTreeMap::from([
                        ("master-key".into(), "enabled".into()),
                        ("vault".into(), "unlocked".into()),
                    ]),
                },
            )]),
        };
        let dir = tempfile::tempdir().unwrap();
        stage_installed_requirements(dir.path(), "neebles-connect", &entry).unwrap();
        let snap = read_installed_requirements(dir.path(), "neebles-connect", "1.2.3").unwrap();
        assert_eq!(snap.short_name, "Connect");
        assert_eq!(snap.provider_ids(), BTreeSet::from(["neebles-secrets".into()]));
        assert_eq!(snap.depends_install["neebles-secrets"]["vault"], "unlocked");
        assert!(read_installed_requirements(dir.path(), "foreign", "1.2.3").is_err());
        assert!(read_installed_requirements(dir.path(), "neebles-connect", "2.0.0").is_err());
        assert!(stage_installed_requirements(dir.path(), "neebles-connect", &entry).is_err());
    }

    #[test]
    fn boss205_snapshot_is_desktop_readable_but_never_desktop_writable() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let entry = crate::modules::RegistryModule {
            repo: "https://github.com/example/secrets.git".into(),
            commit: "a".repeat(40),
            version: "1.0.1".into(),
            folder: None,
            short_name: "Secrets".into(),
            depends_install: BTreeMap::new(),
        };
        let directory = tempfile::tempdir().unwrap();
        stage_installed_requirements(directory.path(), "neebles-secrets", &entry).unwrap();
        let path = directory.path().join(INSTALLED_REQUIREMENTS_FILENAME);
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        assert_eq!(metadata.mode() & 0o777, SNAPSHOT_READ_ONLY_MODE);
        assert_eq!(metadata.uid(), std::fs::metadata(directory.path()).unwrap().uid());
        let read = read_installed_requirements(directory.path(), "neebles-secrets", "1.0.1").unwrap();
        assert!(read.depends_install.is_empty());
    }

    #[test]
    fn boss205_rejects_group_or_world_writable_snapshot() {
        use std::os::unix::fs::PermissionsExt;
        let entry = crate::modules::RegistryModule {
            repo: "https://github.com/example/secrets.git".into(),
            commit: "a".repeat(40),
            version: "1.0.1".into(),
            folder: None,
            short_name: "Secrets".into(),
            depends_install: BTreeMap::new(),
        };
        let directory = tempfile::tempdir().unwrap();
        stage_installed_requirements(directory.path(), "neebles-secrets", &entry).unwrap();
        let path = directory.path().join(INSTALLED_REQUIREMENTS_FILENAME);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();
        assert!(read_installed_requirements(directory.path(), "neebles-secrets", "1.0.1").is_err());
    }

    #[test]
    fn f02e_snapshot_rejects_tampering_and_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(INSTALLED_REQUIREMENTS_FILENAME);
        std::fs::write(&path, b"{\"schema\":1}").unwrap();
        assert!(read_installed_requirements(dir.path(), "example", "1.0.0").is_err());
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", &path).unwrap();
        assert!(read_installed_requirements(dir.path(), "example", "1.0.0").is_err());
    }

    #[test]
    fn schema3_installed_graph_protects_provider_even_when_consumer_is_inactive() {
        // Active/Inactive is deliberately not an input to the installed graph.
        let requires = BTreeMap::from([
            ("neebles-connect".to_string(), BTreeSet::from(["neebles-secrets".to_string()])),
            ("neebles-db".to_string(), BTreeSet::from(["neebles-secrets".to_string()])),
            ("neebles-secrets".to_string(), BTreeSet::new()),
        ]);
        let graph = InstalledDependencyGraph::from_installed_requirements(requires).unwrap();
        assert_eq!(graph.uninstall_blockers("neebles-secrets").unwrap(), vec![
            "neebles-connect".to_string(), "neebles-db".to_string()
        ]);
        assert!(graph.can_uninstall("neebles-secrets").is_err());
        assert!(graph.can_disable("neebles-secrets").is_err());
        graph.can_disable("neebles-connect").unwrap();
    }

    #[test]
    fn installed_graph_rejects_missing_provider_and_cycles() {
        let missing = BTreeMap::from([("connect".to_string(), BTreeSet::from(["secrets".to_string()]))]);
        assert!(InstalledDependencyGraph::from_installed_requirements(missing).is_err());
        let circular = BTreeMap::from([
            ("a".to_string(), BTreeSet::from(["b".to_string()])),
            ("b".to_string(), BTreeSet::from(["a".to_string()])),
        ]);
        assert!(InstalledDependencyGraph::from_installed_requirements(circular).is_err());
    }

    #[test]
    fn install_policy_checks_all_opaque_variables_with_exact_string_equality() {
        let expected = BTreeMap::from([("neebles-secrets".to_string(), BTreeMap::from([
            ("master-key".to_string(), "enabled".to_string()),
            ("some-other-variable".to_string(), "READY".to_string()),
        ]))]);
        let installed = BTreeSet::from(["neebles-secrets".to_string()]);
        let observed = BTreeMap::from([("neebles-secrets".to_string(), BTreeMap::from([
            ("master-key".to_string(), "enabled".to_string()),
            ("some-other-variable".to_string(), "READY".to_string()),
            ("unrelated".to_string(), "value".to_string()),
        ]))]);
        assert!(unmet_install_requirements(&expected, &installed, &observed).is_empty());
        let observed_wrong = BTreeMap::from([("neebles-secrets".to_string(), BTreeMap::from([
            ("master-key".to_string(), "Enabled".to_string()),
            ("some-other-variable".to_string(), "READY".to_string()),
        ]))]);
        assert_eq!(unmet_install_requirements(&expected, &installed, &observed_wrong), vec![
            InstallRequirementFailure::VariableDoesNotMatch {
                provider: "neebles-secrets".into(), variable: "master-key".into()
            }
        ]);
    }

    #[test]
    fn install_policy_fails_closed_for_absent_provider_or_publication() {
        let expected = BTreeMap::from([("secrets".to_string(), BTreeMap::from([
            ("a".to_string(), "1".to_string()), ("b".to_string(), "2".to_string())
        ]))]);
        let no_installed = BTreeSet::new();
        let missing = unmet_install_requirements(&expected, &no_installed, &BTreeMap::new());
        assert_eq!(missing, vec![InstallRequirementFailure::ProviderNotInstalled { provider: "secrets".into() }]);
        let installed = BTreeSet::from(["secrets".to_string()]);
        let unknown = unmet_install_requirements(&expected, &installed, &BTreeMap::new());
        assert_eq!(unknown.len(), 2);
        assert!(unknown.iter().all(|error| matches!(error, InstallRequirementFailure::VariableNotPublished { .. })));
    }

    #[test]
    fn install_policy_allows_presence_only_dependency_when_installed() {
        let expected = BTreeMap::from([("secrets".to_string(), BTreeMap::new())]);
        let installed = BTreeSet::from(["secrets".to_string()]);
        assert!(unmet_install_requirements(&expected, &installed, &BTreeMap::new()).is_empty());
        assert!(!unmet_install_requirements(&expected, &BTreeSet::new(), &BTreeMap::new()).is_empty());
    }

}

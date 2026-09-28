use crate::config;
use crate::contracts::{
    load_module_contracts, ContractDefinition, ContractEndpoint, ContractReference, ModuleContracts,
};
use crate::languages;
use crate::privileges;
use crate::settings;
use neebles_backend::domestic_environment::parse_environment_lines;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MODULE_SCHEMA_VERSION: u32 = 3;
pub const MODULE_LANGUAGE_SCHEMA_VERSION: u32 = 1;
pub const MODULE_NOTIFICATIONS_PROTOCOL_VERSION: u32 = 1;
pub const REGISTRY_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommandLifecycle {
    #[default]
    Oneshot,
    Tracked,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandContract {
    #[serde(default)]
    pub requires_root: bool,

    #[serde(default)]
    pub lifecycle: CommandLifecycle,

    #[serde(default)]
    pub launcher: bool,

    #[serde(default)]
    pub allowed_session_inputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayContract {
    #[serde(default = "default_tray_protocol")]
    pub protocol: u32,

    pub icon: String,

    pub provider: String,
}

fn default_tray_protocol() -> u32 {
    crate::tray::protocol::TRAY_PROTOCOL_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationContract {
    #[serde(default = "default_notifications_protocol")]
    pub protocol: u32,
}

fn default_notifications_protocol() -> u32 {
    MODULE_NOTIFICATIONS_PROTOCOL_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    #[serde(default = "default_schema")]
    pub schema: u32,
    pub name: String,
    #[serde(default)]
    pub version: String,
    pub entrypoint: String,

    /*
     * Optional module lifecycle contract.
     *
     * This is a reference to a module-owned declarative lifecycle
     * file. Boss knows the lifecycle transition names but does not
     * know the technology described by their String recipes.
     */
    #[serde(default)]
    pub lifecycle: Option<String>,

    /*
     * Dynamic module contracts.
     *
     * A module may extend the N.E.E.B.L.E.S. ecosystem by
     * declaring contract files without requiring Boss to be
     * recompiled for each module, command or contract type.
     *
     * Legacy Schema 3 manifests without this field remain
     * valid and resolve to an empty contract list.
     */
    #[serde(default)]
    pub contracts: Vec<ContractReference>,

    /*
     * Legacy embedded command contracts.
     *
     * Kept for Schema 3 compatibility while the dynamic
     * contract architecture is introduced.
     */
    #[serde(default)]
    pub commands: BTreeMap<String, CommandContract>,

    #[serde(default)]
    pub tray: Option<TrayContract>,

    #[serde(default)]
    pub notifications: Option<NotificationContract>,

    /*
     * Optional persistent local-settings default.
     *
     * The path is relative to the module directory.
     * If omitted, Boss treats the module default as {}.
     *
     * The module owns the structure and meaning of this JSON.
     * Boss owns persistence and reconciliation.
     */
    #[serde(default)]
    pub settings: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleLanguageEntry {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleLanguageManifest {
    pub schema: u32,

    pub default: String,

    pub languages: Vec<ModuleLanguageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryModule {
    pub repo: String,

    pub commit: String,

    pub version: String,

    #[serde(default)]
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default = "default_schema")]
    pub schema: u32,
    #[serde(default)]
    pub modules: BTreeMap<String, RegistryModule>,
}

fn default_schema() -> u32 {
    1
}

fn valid_git_commit(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn validate_registry(registry: &Registry) -> Result<(), String> {
    if registry.schema != REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported N.E.E.B.L.E.S. registry schema {}; expected {}",
            registry.schema, REGISTRY_SCHEMA_VERSION
        ));
    }

    let mut effective_folders = BTreeMap::new();

    for (name, module) in &registry.modules {
        if !valid_module_id(name) {
            return Err(format!("invalid module id in registry: {name}"));
        }

        if module.repo.trim().is_empty() {
            return Err(format!("module '{}' registry repo cannot be empty", name));
        }

        if !valid_git_commit(module.commit.trim()) {
            return Err(format!(
                "module '{}' registry commit '{}' is not a full 40-character Git commit SHA",
                name, module.commit
            ));
        }

        let version = module.version.trim();

        if version.is_empty() {
            return Err(format!(
                "module '{}' registry version cannot be empty",
                name
            ));
        }

        Version::parse(version).map_err(|error| {
            format!(
                "module '{}' registry version '{}' is not valid SemVer: {error}",
                name, version
            )
        })?;

        let effective_folder = module.folder.as_deref().unwrap_or(name).trim();

        if !valid_module_id(effective_folder) {
            return Err(format!(
                "invalid effective module folder '{}' in registry for '{}'",
                effective_folder, name
            ));
        }

        if let Some(previous) =
            effective_folders.insert(effective_folder.to_string(), name.to_string())
        {
            return Err(format!(
                "registry folder collision: modules '{}' and '{}' both resolve to folder '{}'",
                previous, name, effective_folder
            ));
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct ResolvedTrayContract {
    pub protocol: u32,
    pub module_version: String,
    pub icon: String,
    pub provider: String,
}

fn resolve_module_contract_path(
    module_dir: &Path,
    value: &str,
    field: &str,
) -> Result<PathBuf, String> {
    let value = value.trim();

    if value.is_empty() {
        return Err(format!("module tray {field} cannot be empty"));
    }

    let relative = Path::new(value);

    if relative.is_absolute() {
        return Err(format!(
            "module tray {field} must be relative to the module directory"
        ));
    }

    for component in relative.components() {
        use std::path::Component;

        match component {
            Component::Normal(_) | Component::CurDir => {}

            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "module tray {field} contains an invalid path: {value}"
                ));
            }
        }
    }

    let candidate = module_dir.join(relative);

    let canonical = candidate.canonicalize().map_err(|error| {
        format!(
            "could not resolve module tray {field} {}: {error}",
            candidate.display()
        )
    })?;

    let canonical_module = module_dir.canonicalize().map_err(|error| {
        format!(
            "could not resolve module directory {}: {error}",
            module_dir.display()
        )
    })?;

    if !canonical.starts_with(&canonical_module) {
        return Err(format!("module tray {field} escapes module directory"));
    }

    if !canonical.is_file() {
        return Err(format!(
            "module tray {field} is not a file: {}",
            canonical.display()
        ));
    }

    Ok(canonical)
}

pub fn installed_module_manifest(name: &str) -> Result<ModuleManifest, String> {
    if !valid_module_id(name) {
        return Err(format!("invalid module id: {name}"));
    }

    let module_dir = find_module_dir(name)?;

    read_manifest(&module_dir.join("manifest.json"))
}

/*
 * Resolve Lifecycle from any already validated module directory.
 *
 * The same path is used by installed modules and staged candidates.
 * A module without a Lifecycle declaration simply contributes an
 * empty Lifecycle contract and therefore has no require relations.
 */
fn lifecycle_contract_from_module(
    module_dir: &Path,
    manifest: &ModuleManifest,
) -> Result<crate::lifecycle::LifecycleContract, String> {
    let Some(reference) = manifest.lifecycle.as_deref() else {
        return Ok(crate::lifecycle::LifecycleContract::default());
    };

    let lifecycle_path = resolve_module_file(module_dir, reference, "lifecycle")?;

    crate::lifecycle::load(&lifecycle_path)
}

pub fn installed_module_lifecycle_contract(
    name: &str,
) -> Result<crate::lifecycle::LifecycleContract, String> {
    if !valid_module_id(name) {
        return Err(format!("invalid module id: {name}"));
    }

    let module_dir = find_module_dir(name)?;

    let manifest = read_manifest(&module_dir.join("manifest.json"))?;

    lifecycle_contract_from_module(&module_dir, &manifest)
}

/*
 * Load every dynamic contract declared by an installed module.
 *
 * Boss does not know or care whether those contracts are named
 * "commands", "connect", "service", "llm", "whatever", etc.
 *
 * The manifest is the discovery index and each contract file is
 * loaded through the generic contracts subsystem.
 */
pub fn installed_module_contracts(name: &str) -> Result<ModuleContracts, String> {
    if !valid_module_id(name) {
        return Err(format!("invalid module id: {name}"));
    }

    let module_dir = find_module_dir(name)?;

    let manifest = read_manifest(&module_dir.join("manifest.json"))?;

    load_module_contracts(&manifest.name, &module_dir, &manifest.contracts)
}

/*
 * Resolve one arbitrary contract by its declared type.
 *
 * Contract type remains a String deliberately.
 * Adding a new contract type must not require recompiling Boss.
 */
pub fn installed_module_contract(
    name: &str,
    contract_type: &str,
) -> Result<Option<ContractDefinition>, String> {
    let contracts = installed_module_contracts(name)?;

    Ok(contracts.contracts.get(contract_type).cloned())
}

/*
 * Resolve one logical endpoint from one arbitrary contract.
 *
 * Example:
 *
 * module   = test-module
 * contract = commands
 * name     = gradient
 *
 * Result may declare:
 *
 * endpoint   = gradient.create
 * lifecycle  = runtime
 * state_mode = clean
 */
pub fn installed_module_contract_endpoint(
    name: &str,
    contract_type: &str,
    endpoint_name: &str,
) -> Result<Option<ContractEndpoint>, String> {
    let Some(contract) = installed_module_contract(name, contract_type)? else {
        return Ok(None);
    };

    Ok(contract.endpoints.get(endpoint_name).cloned())
}

fn resolve_tray_contract_from(
    module_dir: &Path,
    manifest: &ModuleManifest,
) -> Result<ResolvedTrayContract, String> {
    let tray = manifest.tray.as_ref().ok_or_else(|| {
        format!(
            "module '{}' does not declare a tray capability",
            manifest.name
        )
    })?;

    if tray.protocol != crate::tray::protocol::TRAY_PROTOCOL_VERSION {
        return Err(format!(
            "module '{}' declares unsupported tray protocol {}; expected {}",
            manifest.name,
            tray.protocol,
            crate::tray::protocol::TRAY_PROTOCOL_VERSION
        ));
    }

    let icon = resolve_module_contract_path(module_dir, &tray.icon, "icon")?;

    let provider = resolve_module_contract_path(module_dir, &tray.provider, "provider")?;

    let metadata = fs::metadata(&provider).map_err(|error| {
        format!(
            "could not inspect module '{}' tray provider {}: {error}",
            manifest.name,
            provider.display()
        )
    })?;

    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!(
            "module '{}' tray provider is not executable: {}",
            manifest.name,
            provider.display()
        ));
    }

    Ok(ResolvedTrayContract {
        protocol: tray.protocol,
        module_version: manifest.version.clone(),
        icon: icon.display().to_string(),
        provider: provider.display().to_string(),
    })
}

pub fn resolved_tray_contract(name: &str) -> Result<ResolvedTrayContract, String> {
    let module_dir = find_module_dir(name)?;

    let manifest = installed_module_manifest(name)?;

    resolve_tray_contract_from(&module_dir, &manifest)
}

const MODULE_ICON_EXTENSIONS: &[&str] = &["svg", "png", "webp", "jpg", "jpeg"];

static TRANSACTION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn transaction_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();

    let counter = TRANSACTION_COUNTER.fetch_add(1, Ordering::Relaxed);

    format!("{}-{}-{}", std::process::id(), timestamp, counter)
}

struct ModuleInstallStaging {
    path: PathBuf,
    committed: bool,
}

impl ModuleInstallStaging {
    fn prepare(path: PathBuf) -> Result<Self, String> {
        if path.exists() {
            return Err(format!(
                "module staging path already exists and will not be removed automatically: {}",
                path.display()
            ));
        }

        Ok(Self {
            path,
            committed: false,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn commit(mut self, destination: &Path) -> Result<(), String> {
        fs::rename(&self.path, destination).map_err(|error| {
            format!(
                "could not move module staging directory {} into {}: {error}",
                self.path.display(),
                destination.display()
            )
        })?;

        self.committed = true;

        Ok(())
    }
}

impl Drop for ModuleInstallStaging {
    fn drop(&mut self) {
        if self.committed || !self.path.exists() {
            return;
        }

        if let Err(error) = fs::remove_dir_all(&self.path) {
            eprintln!(
                "N.E.E.B.L.E.S.: could not clean module staging directory {}: {error}",
                self.path.display()
            );
        }
    }
}

/*
 * Candidate staged for recursive Lifecycle require resolution.
 *
 * Presence inside RequireCandidateInventory means the candidate was
 * acquired for inspection during this transaction and has not been
 * published into modules_root().
 *
 * No installation-state booleans are required.
 */
struct RequireCandidate {
    module_id: String,
    folder: String,
    staging: ModuleInstallStaging,
    manifest: ModuleManifest,
    lifecycle: crate::lifecycle::LifecycleContract,
}

impl RequireCandidate {
    fn module_id(&self) -> &str {
        &self.module_id
    }

    fn folder(&self) -> &str {
        &self.folder
    }

    fn staging_path(&self) -> &Path {
        self.staging.path()
    }

    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    fn lifecycle(&self) -> &crate::lifecycle::LifecycleContract {
        &self.lifecycle
    }
}

/*
 * Transaction-local candidate inventory.
 *
 * baseline:
 *     modules that existed before require resolution
 *
 * staged:
 *     missing modules materialized from immutable Registry commits
 *     and retained without publication
 */
struct RequireCandidateInventory {
    baseline: BTreeSet<String>,
    staged: BTreeMap<String, RequireCandidate>,
}

impl RequireCandidateInventory {
    fn new() -> Result<Self, String> {
        let baseline = installed_names()?.into_iter().collect();

        Ok(Self {
            baseline,
            staged: BTreeMap::new(),
        })
    }

    fn baseline(&self) -> &BTreeSet<String> {
        &self.baseline
    }

    fn staged(&self) -> &BTreeMap<String, RequireCandidate> {
        &self.staged
    }

    fn staged_ids(&self) -> BTreeSet<String> {
        self.staged.keys().cloned().collect()
    }

    fn take_staged(&mut self, module_id: &str) -> Option<RequireCandidate> {
        self.staged.remove(module_id)
    }

    fn lifecycle_source(
        &mut self,
        module_id: &str,
        registry: &Registry,
    ) -> Result<crate::lifecycle::LifecycleContract, String> {
        if !valid_module_id(module_id) {
            return Err(format!("invalid require module id: {module_id}"));
        }

        /*
         * Existing module:
         *
         * reuse it as baseline.
         * Do not stage it and do not reinstall it.
         *
         * Its own require declaration is still inspected recursively.
         */
        if self.baseline.contains(module_id) {
            return installed_module_lifecycle_contract(module_id);
        }

        /*
         * Shared recursive requirement:
         *
         * one transaction keeps exactly one staged candidate.
         */
        if let Some(candidate) = self.staged.get(module_id) {
            return Ok(candidate.lifecycle().clone());
        }

        let candidate = stage_require_candidate(module_id, registry)?;

        let lifecycle = candidate.lifecycle().clone();

        self.staged.insert(module_id.to_string(), candidate);

        Ok(lifecycle)
    }
}

fn stage_require_candidate(name: &str, registry: &Registry) -> Result<RequireCandidate, String> {
    if !valid_module_id(name) {
        return Err(format!("invalid module id in require candidate: {name}"));
    }

    let entry = registry.modules.get(name).ok_or_else(|| {
        format!("required module '{name}' does not exist in the N.E.E.B.L.E.S. registry")
    })?;

    let temp_root = neebles_root().join("shared/tmp");

    fs::create_dir_all(&temp_root)
        .map_err(|error| format!("could not create {}: {error}", temp_root.display()))?;

    let temp = temp_root.join(format!("require-{name}-{}", transaction_id()));

    let staging = ModuleInstallStaging::prepare(temp)?;

    checkout_registry_commit(name, entry, staging.path(), "boss.modules.install_staging")?;

    let manifest = read_manifest(&staging.path().join("manifest.json"))?;

    if manifest.name != name {
        return Err(format!(
            "required module manifest name '{}' does not match registry id '{name}'",
            manifest.name
        ));
    }

    if manifest.version != entry.version {
        return Err(format!(
            "required module '{}' manifest version '{}' does not match registry version '{}'",
            name, manifest.version, entry.version
        ));
    }

    let folder = entry.folder.as_deref().unwrap_or(name).to_string();

    if !valid_module_id(&folder) {
        return Err(format!("invalid module folder in registry: {folder}"));
    }

    let lifecycle = lifecycle_contract_from_module(staging.path(), &manifest)?;

    Ok(RequireCandidate {
        module_id: name.to_string(),

        folder,

        staging,

        manifest,

        lifecycle,
    })
}

fn resolve_require_candidates(
    root: &str,
    registry: &Registry,
) -> Result<
    (
        crate::lifecycle_require::RequirePlan,
        RequireCandidateInventory,
    ),
    String,
> {
    let mut inventory = RequireCandidateInventory::new()?;

    let baseline = inventory.baseline().clone();

    let plan = crate::lifecycle_require::resolve(root, baseline, |module_id| {
        inventory.lifecycle_source(module_id, registry)
    })?;

    Ok((plan, inventory))
}

/*
 * Validate the complete publication surface before mutating
 * the installed module tree.
 *
 * Folder collisions and pre-existing destinations are rejected
 * before the first staged candidate is committed.
 */
fn validate_require_publication_destinations(
    inventory: &RequireCandidateInventory,
) -> Result<(), String> {
    let mut folders = BTreeMap::<String, String>::new();

    for (module_id, candidate) in inventory.staged() {
        let folder = candidate.folder().to_string();

        if let Some(previous) = folders.insert(folder.clone(), module_id.clone()) {
            return Err(format!(
                "require publication folder collision: modules '{}' and '{}' both resolve to '{}'",
                previous, module_id, folder
            ));
        }

        let destination = modules_root().join(&folder);

        if destination.exists() {
            return Err(format!(
                "require publication destination already exists for module '{}': {}",
                module_id,
                destination.display()
            ));
        }
    }

    Ok(())
}

/*
 * Publish one already staged and validated candidate.
 *
 * This is the same installation boundary the normal Governor
 * owns: module tree, settings contract, deactivate registry and
 * lifecycle installation event.
 *
 * The caller owns multi-module rollback.
 */
fn publish_require_candidate(
    candidate: RequireCandidate,
) -> Result<BTreeMap<String, String>, String> {
    let RequireCandidate {
        module_id,
        folder,
        staging,
        manifest,
        lifecycle: _,
    } = candidate;

    if find_module_dir(&module_id).is_ok() {
        return Err(format!(
            "module '{}' became installed while require transaction was in progress",
            module_id
        ));
    }

    if manifest.tray.is_some() {
        resolve_tray_contract_from(staging.path(), &manifest)?;
    }

    let settings_default = module_settings_default_from(staging.path(), &manifest)?;

    let destination = modules_root().join(&folder);

    fs::create_dir_all(modules_root())
        .map_err(|error| format!("could not create {}: {error}", modules_root().display()))?;

    if destination.exists() {
        return Err(format!(
            "module destination already exists: {}",
            destination.display()
        ));
    }

    staging.commit(&destination)?;

    let settings_path = settings::module_settings_path(&neebles_root(), &module_id);

    if let Err(error) = settings::update_from_default(&settings_path, &settings_default) {
        let rollback_error = fs::remove_dir_all(&destination).err();

        return match rollback_error {
            None => Err(
                format!(
                    "could not initialize settings for required module '{}': {}; module publication was rolled back",
                    module_id,
                    error
                )
            ),

            Some(rollback_error) => Err(
                format!(
                    "CRITICAL: could not initialize settings for required module '{}': {}; publication rollback also failed: {}",
                    module_id,
                    error,
                    rollback_error
                )
            ),
        };
    }

    if let Err(error) = register_deactivate_entry(&module_id, &manifest) {
        let rollback_error = fs::remove_dir_all(&destination).err();

        return match rollback_error {
            None => Err(
                format!(
                    "could not register deactivate resources for required module '{}': {}; module publication was rolled back and local settings were preserved",
                    module_id,
                    error
                )
            ),

            Some(rollback_error) => Err(
                format!(
                    "CRITICAL: could not register deactivate resources for required module '{}': {}; publication rollback also failed: {}",
                    module_id,
                    error,
                    rollback_error
                )
            ),
        };
    }

    if let Err(error) = crate::module_ipc::runtime_registry().broadcast_event(
        "module.lifecycle",
        "installed",
        serde_json::json!({
            "module":
                module_id,

            "version":
                manifest.version
        }),
    ) {
        eprintln!(
            "N.E.E.B.L.E.S.: required module '{}' installed but lifecycle event broadcast failed: {}",
            module_id,
            error
        );
    }

    Ok(BTreeMap::from([
        ("folder".to_string(), folder),
        ("version".to_string(), manifest.version),
    ]))
}

fn module_governor_lifecycle_runtime(
) -> Result<crate::lifecycle_governor_runtime::GovernorLifecycleRuntime, String> {
    let available = crate::lifecycle_available_capabilities::productive_catalog()?;

    Ok(crate::lifecycle_governor_runtime::GovernorLifecycleRuntime::from_available(available))
}

fn execute_module_governor_lifecycle(
    runtime: &crate::lifecycle_governor_runtime::GovernorLifecycleRuntime,
    module_id: &str,
    action: &str,
    contract: &crate::lifecycle::LifecycleContract,
) -> Result<(), String> {
    runtime
        .execute_action_blocking(
            module_id,
            &format!("module.{action}"),
            contract,
            action,
            std::collections::BTreeMap::new(),
        )
        .map(|_| ())
}

fn require_install_failure(
    cause: String,
    transaction: &mut crate::lifecycle_require::RequireTransaction,
) -> String {
    match transaction.rollback(|entry| uninstall_without_lifecycle(entry.target(), true)) {
        Ok(()) => {
            format!("{cause}; modules acquired by this require transaction were rolled back")
        }

        Err(rollback_error) => {
            format!("CRITICAL: {cause}; require rollback was incomplete: {rollback_error}")
        }
    }
}

fn install_require_tree(
    root: &str,
    registry: &Registry,
    lifecycle_runtime: &crate::lifecycle_governor_runtime::GovernorLifecycleRuntime,
) -> Result<(), String> {
    let (plan, mut inventory) = resolve_require_candidates(root, registry)?;

    validate_require_publication_destinations(&inventory)?;

    let order = plan.resolution_order().to_vec();

    let mut transaction = crate::lifecycle_require::RequireTransaction::new(plan);

    for module_id in order {
        if transaction.baseline().contains(&module_id) {
            continue;
        }

        let Some(candidate) = inventory.take_staged(&module_id) else {
            let error = format!("require transaction lost staged candidate '{}'", module_id);

            return Err(require_install_failure(error, &mut transaction));
        };

        let lifecycle = candidate.lifecycle().clone();

        let publication = publish_require_candidate(candidate);

        let data = match publication {
            Ok(data) => data,

            Err(error) => {
                return Err(require_install_failure(
                    format!(
                        "could not publish required module '{}': {}",
                        module_id, error
                    ),
                    &mut transaction,
                ));
            }
        };

        if let Err(error) = transaction.record_acquisition(&module_id, "module.install", data) {
            let current_cleanup = uninstall_without_lifecycle(&module_id, true);

            let previous_cleanup =
                transaction.rollback(|entry| uninstall_without_lifecycle(entry.target(), true));

            let current_text = match current_cleanup {
                Ok(()) => "current module was compensated".to_string(),

                Err(cleanup_error) => {
                    format!("current module compensation failed: {cleanup_error}")
                }
            };

            let previous_text = match previous_cleanup {
                Ok(()) => "previous acquisitions were compensated".to_string(),

                Err(cleanup_error) => {
                    format!("previous acquisition compensation failed: {cleanup_error}")
                }
            };

            return Err(
                format!(
                    "CRITICAL: module '{}' was published but could not enter require journal: {}; {}; {}",
                    module_id,
                    error,
                    current_text,
                    previous_text
                )
            );
        }

        if let Err(error) =
            execute_module_governor_lifecycle(lifecycle_runtime, &module_id, "install", &lifecycle)
        {
            return Err(require_install_failure(
                format!(
                    "install Lifecycle failed for module '{}': {}",
                    module_id, error
                ),
                &mut transaction,
            ));
        }
    }

    if !inventory.staged().is_empty() {
        let remaining = inventory
            .staged_ids()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");

        return Err(require_install_failure(
            format!("require transaction finished with unpublished staged candidates: {remaining}"),
            &mut transaction,
        ));
    }

    Ok(())
}

fn checkout_registry_commit(
    name: &str,
    entry: &RegistryModule,
    destination: &Path,
    writable_authority: &str,
) -> Result<(), String> {
    let expected = entry.commit.trim().to_ascii_lowercase();

    if !valid_git_commit(&expected) {
        return Err(format!(
            "module '{}' has invalid registry commit '{}'",
            name, entry.commit
        ));
    }

    fs::create_dir(destination).map_err(|error| {
        format!(
            "could not materialize module staging directory {} for '{}': {error}",
            destination.display(),
            name
        )
    })?;

    let registry =
        neebles_backend::domestic_authority_supply_process::process_supplied_authority_registry()?;

    let grants = neebles_backend::domestic_authority_supply::build_authority_grant_set(
        registry,
        [
            "platform.filesystem_boundary",
            "system.dns_resolver_config",
            writable_authority,
        ],
    )?;

    let platform_descriptor = grants.descriptor_path(registry, "platform.filesystem_boundary")?;

    let dns_descriptor = grants.descriptor_path(registry, "system.dns_resolver_config")?;

    let writable_descriptor = grants.descriptor_path(registry, writable_authority)?;

    let platform =
        neebles_backend::domestic_platform_authority::load_platform_authority_descriptor(
            &platform_descriptor,
            "platform.filesystem_boundary",
        )?;

    let dns =
        neebles_backend::domestic_external_data_authority::load_external_data_authority_descriptor(
            &dns_descriptor,
            "system.dns_resolver_config",
        )?;

    let writable =
        neebles_backend::domestic_writable_data_authority::load_writable_data_authority_descriptor(
            &writable_descriptor,
            writable_authority,
        )?;

    let writable_grant =
        neebles_backend::domestic_writable_data_authority::grant_writable_data_subpath(
            &writable,
            destination,
            destination,
        )?;

    let runtime_manifest =
        neebles_backend::domestic_runtime_authority::current_boss_runtime_manifest()?;

    let plan =
        neebles_backend::domestic_boundary_execution::compose_materialized_boundary_execution_plan_with_writable_data(
            &runtime_manifest,
            "boss.git",
            &platform,
            &[dns],
            &[writable_grant],
            true,
            true,
            true,
            Some(Path::new("/tmp")),
        )?;

    let git_command = |arguments: &[std::ffi::OsString]| {
        neebles_backend::domestic_boundary_execution::build_pure_materialized_boundary_execution_command(
                &plan,
                arguments,
            )
    };

    let init_arguments = [
        std::ffi::OsString::from("init"),
        destination.as_os_str().to_os_string(),
    ];

    let status = git_command(&init_arguments)?.status().map_err(|error| {
        format!(
            "could not initialize staged repository for module '{}': {error}",
            name
        )
    })?;

    if !status.success() {
        return Err(format!(
            "git init failed while staging module '{}' with status {}",
            name, status
        ));
    }

    let remote_arguments = [
        std::ffi::OsString::from("-C"),
        destination.as_os_str().to_os_string(),
        std::ffi::OsString::from("remote"),
        std::ffi::OsString::from("add"),
        std::ffi::OsString::from("origin"),
        std::ffi::OsString::from(&entry.repo),
    ];

    let status = git_command(&remote_arguments)?
        .status()
        .map_err(|error| format!("could not configure module '{}' repository: {error}", name))?;

    if !status.success() {
        return Err(format!(
            "git remote add failed while staging module '{}' with status {}",
            name, status
        ));
    }

    let fetch_arguments = [
        std::ffi::OsString::from("-C"),
        destination.as_os_str().to_os_string(),
        std::ffi::OsString::from("fetch"),
        std::ffi::OsString::from("--depth"),
        std::ffi::OsString::from("1"),
        std::ffi::OsString::from("origin"),
        std::ffi::OsString::from(&expected),
    ];

    let status = git_command(&fetch_arguments)?.status().map_err(|error| {
        format!(
            "could not fetch pinned commit for module '{}': {error}",
            name
        )
    })?;

    if !status.success() {
        return Err(format!(
            "git fetch failed for pinned module '{}' commit {} with status {}",
            name, expected, status
        ));
    }

    let checkout_arguments = [
        std::ffi::OsString::from("-C"),
        destination.as_os_str().to_os_string(),
        std::ffi::OsString::from("checkout"),
        std::ffi::OsString::from("--detach"),
        std::ffi::OsString::from("FETCH_HEAD"),
    ];

    let status = git_command(&checkout_arguments)?
        .status()
        .map_err(|error| {
            format!(
                "could not checkout pinned module '{}' commit: {error}",
                name
            )
        })?;

    if !status.success() {
        return Err(format!(
            "git checkout failed for pinned module '{}' commit {} with status {}",
            name, expected, status
        ));
    }

    let verify_arguments = [
        std::ffi::OsString::from("-C"),
        destination.as_os_str().to_os_string(),
        std::ffi::OsString::from("rev-parse"),
        std::ffi::OsString::from("HEAD"),
    ];

    let output = git_command(&verify_arguments)?.output().map_err(|error| {
        format!(
            "could not verify checked out commit for module '{}': {error}",
            name
        )
    })?;

    if !output.status.success() {
        return Err(format!(
            "git rev-parse failed while verifying module '{}'",
            name
        ));
    }

    let actual = String::from_utf8(output.stdout)
        .map_err(|error| {
            format!(
                "invalid git rev-parse output for module '{}': {error}",
                name
            )
        })?
        .trim()
        .to_ascii_lowercase();

    if actual != expected {
        return Err(format!(
            "module '{}' integrity failure: registry requires commit {}, but staged repository is {}",
            name,
            expected,
            actual
        ));
    }

    Ok(())
}

fn local_module_icon(module_dir: &Path) -> String {
    for extension in MODULE_ICON_EXTENSIONS {
        let candidate = module_dir.join(format!("icon.{extension}"));

        if candidate.is_file() {
            if let Ok(path) = candidate.canonicalize() {
                return format!("file://{}", path.display());
            }
        }
    }

    String::new()
}

fn build_network_boundary_command(
    world_name: &str,
    arguments: &[std::ffi::OsString],
) -> Result<std::process::Command, String> {
    let registry =
        neebles_backend::domestic_authority_supply_process::process_supplied_authority_registry()?;

    let grants = neebles_backend::domestic_authority_supply::build_authority_grant_set(
        registry,
        ["platform.filesystem_boundary", "system.dns_resolver_config"],
    )?;

    let platform_descriptor = grants.descriptor_path(registry, "platform.filesystem_boundary")?;

    let dns_descriptor = grants.descriptor_path(registry, "system.dns_resolver_config")?;

    let platform =
        neebles_backend::domestic_platform_authority::load_platform_authority_descriptor(
            &platform_descriptor,
            "platform.filesystem_boundary",
        )?;

    let dns =
        neebles_backend::domestic_external_data_authority::load_external_data_authority_descriptor(
            &dns_descriptor,
            "system.dns_resolver_config",
        )?;

    let runtime_manifest =
        neebles_backend::domestic_runtime_authority::current_boss_runtime_manifest()?;

    let plan =
        neebles_backend::domestic_boundary_execution::compose_materialized_boundary_execution_plan(
            &runtime_manifest,
            world_name,
            &platform,
            &[dns],
            true,
            true,
            true,
            Some(std::path::Path::new("/tmp")),
        )?;

    neebles_backend::domestic_boundary_execution::build_pure_materialized_boundary_execution_command(
        &plan, arguments,
    )
}

fn github_raw_base(repo: &str, revision: &str) -> Option<String> {
    let repo = repo
        .strip_prefix("https://github.com/")?
        .trim_end_matches(".git")
        .trim_end_matches('/');

    Some(format!(
        "https://raw.githubusercontent.com/{repo}/{revision}"
    ))
}

fn remote_module_icon(repo: &str, commit: &str) -> String {
    let Some(base) = github_raw_base(repo, commit) else {
        return String::new();
    };

    for extension in MODULE_ICON_EXTENSIONS {
        let url = format!("{base}/icon.{extension}");

        let arguments = [
            std::ffi::OsString::from("-fsIL"),
            std::ffi::OsString::from("--max-time"),
            std::ffi::OsString::from("5"),
            std::ffi::OsString::from(url.as_str()),
        ];

        let mut command = match build_network_boundary_command("boss.curl", &arguments) {
            Ok(command) => command,
            Err(_) => return String::new(),
        };

        let status = command.stdout(Stdio::null()).stderr(Stdio::null()).status();

        if matches!(status, Ok(value) if value.success()) {
            return url;
        }
    }

    String::new()
}

pub fn neebles_root() -> PathBuf {
    env::var("NEEBLES_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt/neebles"))
}

pub fn modules_root() -> PathBuf {
    neebles_root().join("modules")
}

const DEACTIVATE_RESOURCE_RUNTIME: &str = "runtime";
const DEACTIVATE_RESOURCE_TRAY: &str = "tray";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DeactivateEntry {
    resources: Vec<String>,
}

static DEACTIVATE_REGISTRY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn deactivate_registry_path() -> PathBuf {
    neebles_root().join("shared/deactivate.json")
}

fn deactivate_registry_lock() -> &'static Mutex<()> {
    DEACTIVATE_REGISTRY_LOCK.get_or_init(|| Mutex::new(()))
}

fn validate_deactivate_registry(
    registry: &BTreeMap<String, DeactivateEntry>,
) -> Result<(), String> {
    for (module, entry) in registry {
        if !valid_module_id(module) {
            return Err(format!(
                "deactivate registry contains invalid module id: {module}"
            ));
        }

        let mut seen = HashSet::new();

        for resource in &entry.resources {
            if resource != DEACTIVATE_RESOURCE_RUNTIME && resource != DEACTIVATE_RESOURCE_TRAY {
                return Err(format!(
                    "module '{module}' declares unknown deactivate resource '{resource}'"
                ));
            }

            if !seen.insert(resource.as_str()) {
                return Err(format!(
                    "module '{module}' declares duplicate deactivate resource '{resource}'"
                ));
            }
        }
    }

    Ok(())
}

fn load_deactivate_registry_unlocked() -> Result<BTreeMap<String, DeactivateEntry>, String> {
    let path = deactivate_registry_path();

    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => Some(metadata),

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,

        Err(error) => {
            return Err(format!(
                "could not inspect deactivate registry {}: {error}",
                path.display()
            ));
        }
    };

    let Some(metadata) = metadata else {
        return Ok(BTreeMap::new());
    };

    if metadata.file_type().is_symlink() {
        return Err(format!(
            "refusing symlinked deactivate registry: {}",
            path.display()
        ));
    }

    if !metadata.is_file() {
        return Err(format!(
            "deactivate registry is not a regular file: {}",
            path.display()
        ));
    }

    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "could not read deactivate registry {}: {error}",
            path.display()
        )
    })?;

    let registry: BTreeMap<String, DeactivateEntry> =
        serde_json::from_str(&raw).map_err(|error| {
            format!(
                "invalid deactivate registry JSON {}: {error}",
                path.display()
            )
        })?;

    validate_deactivate_registry(&registry)?;

    Ok(registry)
}

fn write_deactivate_registry_unlocked(
    registry: &BTreeMap<String, DeactivateEntry>,
) -> Result<(), String> {
    validate_deactivate_registry(registry)?;

    let path = deactivate_registry_path();

    let parent = path.parent().ok_or_else(|| {
        format!(
            "deactivate registry has no parent directory: {}",
            path.display()
        )
    })?;

    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "could not create deactivate registry directory {}: {error}",
            parent.display()
        )
    })?;

    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "refusing symlinked deactivate registry: {}",
                path.display()
            ));
        }

        if !metadata.is_file() {
            return Err(format!(
                "deactivate registry is not a regular file: {}",
                path.display()
            ));
        }
    }

    let temporary = parent.join(format!(
        ".deactivate.json.tmp.{}.{}",
        std::process::id(),
        transaction_id()
    ));

    let mut payload = serde_json::to_vec_pretty(registry)
        .map_err(|error| format!("could not serialize deactivate registry: {error}"))?;

    payload.push(b'\n');

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temporary)
        .map_err(|error| {
            format!(
                "could not create deactivate registry temporary {}: {error}",
                temporary.display()
            )
        })?;

    if let Err(error) = file.write_all(&payload) {
        let _ = fs::remove_file(&temporary);

        return Err(format!(
            "could not write deactivate registry temporary {}: {error}",
            temporary.display()
        ));
    }

    if let Err(error) = file.sync_all() {
        let _ = fs::remove_file(&temporary);

        return Err(format!(
            "could not sync deactivate registry temporary {}: {error}",
            temporary.display()
        ));
    }

    drop(file);

    fs::rename(&temporary, &path).map_err(|error| {
        let _ = fs::remove_file(&temporary);

        format!(
            "could not publish deactivate registry {}: {error}",
            path.display()
        )
    })?;

    let directory = fs::File::open(parent).map_err(|error| {
        format!(
            "could not open deactivate registry directory {}: {error}",
            parent.display()
        )
    })?;

    directory.sync_all().map_err(|error| {
        format!(
            "deactivate registry was published but directory {} could not be synchronized: {error}",
            parent.display()
        )
    })?;

    Ok(())
}

fn deactivate_resources_for_manifest(manifest: &ModuleManifest) -> Vec<String> {
    let mut resources = vec![DEACTIVATE_RESOURCE_RUNTIME.to_string()];

    if manifest.tray.is_some() {
        resources.push(DEACTIVATE_RESOURCE_TRAY.to_string());
    }

    resources
}

fn register_deactivate_entry(name: &str, manifest: &ModuleManifest) -> Result<(), String> {
    let expected = DeactivateEntry {
        resources: deactivate_resources_for_manifest(manifest),
    };

    let _guard = deactivate_registry_lock()
        .lock()
        .map_err(|_| "deactivate registry lock poisoned".to_string())?;

    let mut registry = load_deactivate_registry_unlocked()?;

    if registry.get(name) == Some(&expected) {
        return Ok(());
    }

    registry.insert(name.to_string(), expected);

    write_deactivate_registry_unlocked(&registry)
}

fn remove_deactivate_entry(name: &str) -> Result<(), String> {
    let _guard = deactivate_registry_lock()
        .lock()
        .map_err(|_| "deactivate registry lock poisoned".to_string())?;

    let mut registry = load_deactivate_registry_unlocked()?;

    if registry.remove(name).is_none() {
        return Ok(());
    }

    write_deactivate_registry_unlocked(&registry)
}

fn ensure_deactivate_entry(name: &str) -> Result<DeactivateEntry, String> {
    let manifest = installed_module_manifest(name)?;

    let expected = DeactivateEntry {
        resources: deactivate_resources_for_manifest(&manifest),
    };

    let _guard = deactivate_registry_lock()
        .lock()
        .map_err(|_| "deactivate registry lock poisoned".to_string())?;

    let mut registry = load_deactivate_registry_unlocked()?;

    if registry.get(name) != Some(&expected) {
        registry.insert(name.to_string(), expected.clone());

        write_deactivate_registry_unlocked(&registry)?;
    }

    Ok(expected)
}

fn deactivate_resource_active(name: &str, resource: &str) -> Result<bool, String> {
    match resource {
        DEACTIVATE_RESOURCE_RUNTIME => {
            if probe_module_pid(name)?.is_some() {
                return Ok(true);
            }

            Ok(crate::module_ipc::runtime_registry().get(name)?.is_some())
        }

        DEACTIVATE_RESOURCE_TRAY => Ok(probe_tray_provider_pid(name)?.is_some()),

        other => Err(format!(
            "module '{name}' declares unsupported deactivate resource '{other}'"
        )),
    }
}

fn module_has_active_resources(name: &str) -> Result<bool, String> {
    let entry = ensure_deactivate_entry(name)?;

    for resource in &entry.resources {
        if deactivate_resource_active(name, resource)? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn request_runtime_shutdown(name: &str, reason: &str) -> Result<(), String> {
    let Some(record) = crate::module_ipc::runtime_registry().get(name)? else {
        return Ok(());
    };

    if let Err(error) = record
        .writer
        .send(crate::module_ipc::protocol::ModuleMessage::Shutdown {
            module: name.to_string(),
            session_id: record.session_id.clone(),
            reason: Some(reason.to_string()),
        })
    {
        eprintln!(
            "N.E.E.B.L.E.S.: cooperative shutdown delivery failed for module '{}' session '{}': {}; governed process shutdown will continue",
            name,
            record.session_id,
            error
        );
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);

    while std::time::Instant::now() < deadline {
        let registered = crate::module_ipc::runtime_registry().get(name)?.is_some();

        let tracked = probe_module_pid(name)?.is_some();

        if !registered && !tracked {
            return Ok(());
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}

fn deactivate_module(name: &str, reason: &str) -> Result<(), String> {
    let entry = ensure_deactivate_entry(name)?;

    for resource in &entry.resources {
        match resource.as_str() {
            DEACTIVATE_RESOURCE_RUNTIME => {
                request_runtime_shutdown(name, reason)?;

                stop_module(name)?;
            }

            DEACTIVATE_RESOURCE_TRAY => {
                stop_tray_provider(name)?;
            }

            other => {
                return Err(format!(
                    "module '{name}' declares unsupported deactivate resource '{other}'"
                ));
            }
        }
    }

    for resource in &entry.resources {
        if deactivate_resource_active(name, resource)? {
            return Err(format!(
                "module '{name}' resource '{resource}' is still active after deactivation"
            ));
        }
    }

    Ok(())
}

fn runtime_identity() -> String {
    /*
     * Privileged re-execution must keep operating on the
     * runtime state of the original desktop user.
     */
    if let Ok(value) = env::var("NEEBLES_RUNTIME_IDENTITY") {
        let value = value.trim();

        if !value.is_empty() && value.chars().all(|character| character.is_ascii_digit()) {
            return value.to_string();
        }
    }

    /*
     * Normal desktop session:
     * XDG_RUNTIME_DIR is normally /run/user/<uid>.
     */
    if let Ok(value) = env::var("XDG_RUNTIME_DIR") {
        if let Some(name) = Path::new(&value)
            .file_name()
            .and_then(|value| value.to_str())
        {
            if !name.is_empty() && name.chars().all(|character| character.is_ascii_digit()) {
                return name.to_string();
            }
        }
    }

    /*
     * Final deterministic fallback.
     * Never use USER: sudo changes USER to root while the
     * runtime ownership we care about is UID-based.
     */
    unsafe { libc::geteuid() }.to_string()
}

fn runtime_modules_root() -> PathBuf {
    env::temp_dir()
        .join(format!("neebles-runtime-{}", runtime_identity()))
        .join("modules")
}

fn runtime_marker_path(name: &str) -> PathBuf {
    runtime_modules_root().join(format!("{name}.pid"))
}

fn clear_runtime_marker(name: &str) {
    let _ = fs::remove_file(runtime_marker_path(name));
}

fn clear_runtime_marker_if_pid(name: &str, pid: u32) {
    let path = runtime_marker_path(name);

    let matches = fs::read_to_string(&path)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        == Some(pid);

    if matches {
        let _ = fs::remove_file(path);
    }
}

fn write_runtime_marker(name: &str, pid: u32) -> Result<(), String> {
    let root = runtime_modules_root();

    fs::create_dir_all(&root).map_err(|error| {
        format!(
            "could not create module runtime directory {}: {error}",
            root.display()
        )
    })?;

    let path = runtime_marker_path(name);

    fs::write(&path, format!("{pid}\n")).map_err(|error| {
        format!(
            "could not write module runtime marker {}: {error}",
            path.display()
        )
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProcessIdentity {
    pid: u32,
    start_time_ticks: u64,
}

fn process_start_time_ticks(pid: u32) -> Result<Option<u64>, String> {
    let path = PathBuf::from(format!("/proc/{pid}/stat"));

    let raw = match fs::read_to_string(&path) {
        Ok(value) => value,

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }

        Err(error) => {
            return Err(format!(
                "could not inspect process identity for pid {} at {}: {error}",
                pid,
                path.display()
            ));
        }
    };

    let end_comm = raw.rfind(')').ok_or_else(|| {
        format!(
            "could not parse process identity for pid {}: malformed /proc stat",
            pid
        )
    })?;

    let tail = raw
        .get(end_comm + 1..)
        .ok_or_else(|| {
            format!(
                "could not parse process identity for pid {}: malformed /proc stat",
                pid
            )
        })?
        .trim();

    let fields: Vec<&str> = tail.split_whitespace().collect();

    let start_time = fields.get(19).ok_or_else(|| {
        format!(
            "could not parse process identity for pid {}: missing starttime",
            pid
        )
    })?;

    let start_time_ticks = start_time.parse::<u64>().map_err(|error| {
        format!(
            "could not parse process identity for pid {} starttime '{}': {error}",
            pid, start_time
        )
    })?;

    Ok(Some(start_time_ticks))
}

fn capture_process_identity(pid: u32) -> Result<Option<ProcessIdentity>, String> {
    Ok(
        process_start_time_ticks(pid)?.map(|start_time_ticks| ProcessIdentity {
            pid,
            start_time_ticks,
        }),
    )
}

fn process_identity_is_alive(identity: ProcessIdentity) -> Result<bool, String> {
    Ok(matches!(
        process_start_time_ticks(identity.pid)?,
        Some(start_time_ticks) if start_time_ticks == identity.start_time_ticks
    ))
}

fn probe_module_pid(name: &str) -> Result<Option<u32>, String> {
    let path = runtime_marker_path(name);

    let raw = match fs::read_to_string(&path) {
        Ok(value) => value,

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }

        Err(error) => {
            return Err(format!(
                "could not read module runtime marker {}: {error}",
                path.display()
            ));
        }
    };

    let pid = raw.trim().parse::<u32>().map_err(|error| {
        format!(
            "module '{}' has an invalid runtime marker {}: {error}",
            name,
            path.display()
        )
    })?;

    let process_path = PathBuf::from(format!("/proc/{pid}"));

    if !process_path.exists() {
        clear_runtime_marker_if_pid(name, pid);

        return Ok(None);
    }

    /*
     * From here onward a live PID exists.
     *
     * Failure to prove ownership must NEVER be converted
     * into "not running". That would allow destructive
     * operations to proceed while an unknown process may
     * still be using the installed module.
     */

    let module_dir = find_module_dir(name).map_err(|error| {
        format!(
            "module '{}' runtime process {} exists, but its installation cannot be resolved: {}",
            name, pid, error
        )
    })?;

    let manifest = read_manifest(&module_dir.join("manifest.json")).map_err(|error| {
        format!(
            "module '{}' runtime process {} exists, but its manifest cannot be verified: {}",
            name, pid, error
        )
    })?;

    let expected = resolve_entrypoint(&module_dir, &manifest.entrypoint).map_err(|error| {
        format!(
            "module '{}' runtime process {} exists, but its entrypoint cannot be verified: {}",
            name, pid, error
        )
    })?;

    /*
     * Native executable.
     */
    let proc_exe = PathBuf::from(format!("/proc/{pid}/exe"));

    if let Ok(actual_exe) = fs::read_link(&proc_exe) {
        if let Ok(actual_exe) = actual_exe.canonicalize() {
            if actual_exe == expected {
                return Ok(Some(pid));
            }
        }
    }

    /*
     * Interpreted entrypoint:
     * Bash/Python/Node/etc.
     */
    let cmdline =
        fs::read(
            format!("/proc/{pid}/cmdline")
        )
        .map_err(|error| {
            format!(
                "module '{}' runtime process {} exists, but its command line cannot be inspected: {error}",
                name,
                pid
            )
        })?;

    let expected_bytes = expected.as_os_str().as_bytes();

    let matches_entrypoint = cmdline
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .any(|argument| argument == expected_bytes);

    if matches_entrypoint {
        return Ok(Some(pid));
    }

    /*
     * PID exists, marker exists, but ownership cannot be
     * proven. Preserve the marker and fail closed.
     */
    Err(format!(
        "module '{}' runtime marker points to live process {}, but Boss cannot verify that the process still belongs to the declared entrypoint",
        name,
        pid
    ))
}

pub fn module_running(name: &str) -> bool {
    match probe_module_pid(name) {
        Ok(Some(_)) => true,
        Ok(None) => false,

        /*
         * Unknown runtime state is treated as running in
         * non-destructive views. This prevents the UI from
         * presenting an unsafe false "stopped" state.
         */
        Err(_) => true,
    }
}

fn stop_module(name: &str) -> Result<(), String> {
    let Some(pid) = probe_module_pid(name)? else {
        return Ok(());
    };

    let Some(identity) = capture_process_identity(pid)? else {
        clear_runtime_marker_if_pid(name, pid);
        return Ok(());
    };

    if probe_module_pid(name)? != Some(pid) || !process_identity_is_alive(identity)? {
        return Err(format!(
            "module '{name}' process {pid} changed identity while lifecycle ownership was being verified"
        ));
    }

    let signal_result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };

    if signal_result != 0 {
        return Err(format!(
            "could not send SIGTERM to module '{name}' process {pid}: {}",
            std::io::Error::last_os_error()
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

    while process_identity_is_alive(identity)? && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if !process_identity_is_alive(identity)? {
        clear_runtime_marker_if_pid(name, pid);
        return Ok(());
    }

    let signal_result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };

    if signal_result != 0 {
        return Err(format!(
            "could not send SIGKILL to module '{name}' process {pid}: {}",
            std::io::Error::last_os_error()
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);

    while process_identity_is_alive(identity)? && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if process_identity_is_alive(identity)? {
        return Err(format!(
            "module '{name}' process {pid} did not terminate after SIGKILL"
        ));
    }

    clear_runtime_marker_if_pid(name, pid);

    Ok(())
}

fn tray_runtime_marker_path(name: &str) -> PathBuf {
    runtime_modules_root().join(format!("{name}.tray.pid"))
}

fn clear_tray_runtime_marker_if_pid(name: &str, pid: u32) {
    let path = tray_runtime_marker_path(name);

    let matches = fs::read_to_string(&path)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        == Some(pid);

    if matches {
        let _ = fs::remove_file(path);
    }
}

fn write_tray_runtime_marker(name: &str, pid: u32) -> Result<(), String> {
    let root = runtime_modules_root();

    fs::create_dir_all(&root).map_err(|error| {
        format!(
            "could not create module runtime directory {}: {error}",
            root.display()
        )
    })?;

    let path = tray_runtime_marker_path(name);

    fs::write(&path, format!("{pid}\n")).map_err(|error| {
        format!(
            "could not write tray provider runtime marker {}: {error}",
            path.display()
        )
    })
}

fn probe_tray_provider_pid(name: &str) -> Result<Option<u32>, String> {
    let path = tray_runtime_marker_path(name);

    let raw = match fs::read_to_string(&path) {
        Ok(value) => value,

        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }

        Err(error) => {
            return Err(format!(
                "could not read tray provider runtime marker {}: {error}",
                path.display()
            ));
        }
    };

    let pid = raw.trim().parse::<u32>().map_err(|error| {
        format!(
            "module '{}' has an invalid tray provider runtime marker {}: {error}",
            name,
            path.display()
        )
    })?;

    let process_path = PathBuf::from(format!("/proc/{pid}"));

    if !process_path.exists() {
        clear_tray_runtime_marker_if_pid(name, pid);

        return Ok(None);
    }

    /*
     * A live PID exists.
     *
     * From this point onward every inability to prove
     * provider ownership is an UNKNOWN/UNSAFE runtime
     * state, never "not running".
     */
    let contract =
        resolved_tray_contract(name)
            .map_err(|error| {
                format!(
                    "module '{}' tray provider process {} exists, but its tray contract cannot be verified: {}",
                    name,
                    pid,
                    error
                )
            })?;

    let expected =
        PathBuf::from(
            &contract.provider
        )
        .canonicalize()
        .map_err(|error| {
            format!(
                "module '{}' tray provider process {} exists, but its provider path cannot be canonicalized: {error}",
                name,
                pid
            )
        })?;

    /*
     * Native provider.
     */
    let proc_exe = PathBuf::from(format!("/proc/{pid}/exe"));

    if let Ok(actual_exe) = fs::read_link(&proc_exe) {
        if let Ok(actual_exe) = actual_exe.canonicalize() {
            if actual_exe == expected {
                return Ok(Some(pid));
            }
        }
    }

    /*
     * Interpreted provider:
     * Python/Bash/Node/etc.
     */
    let cmdline =
        fs::read(
            format!("/proc/{pid}/cmdline")
        )
        .map_err(|error| {
            format!(
                "module '{}' tray provider process {} exists, but its command line cannot be inspected: {error}",
                name,
                pid
            )
        })?;

    let expected_bytes = expected.as_os_str().as_bytes();

    let matches_provider = cmdline
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .any(|argument| argument == expected_bytes);

    if matches_provider {
        return Ok(Some(pid));
    }

    /*
     * Preserve the marker. A live PID with unverified
     * ownership requires explicit intervention.
     */
    Err(format!(
        "module '{}' tray provider marker points to live process {}, but Boss cannot verify that the process still belongs to the declared provider",
        name,
        pid
    ))
}

pub fn tray_provider_running(name: &str) -> bool {
    match probe_tray_provider_pid(name) {
        Ok(Some(_)) => true,
        Ok(None) => false,

        /*
         * Unknown provider state is reported as running
         * to avoid presenting an unsafe false "stopped"
         * state.
         */
        Err(_) => true,
    }
}

pub fn verify_tray_provider_process(name: &str, pid: u32) -> Result<(), String> {
    let contract = resolved_tray_contract(name)?;

    let expected = PathBuf::from(&contract.provider)
        .canonicalize()
        .map_err(|error| {
            format!(
                "could not canonicalize tray provider for module '{}': {error}",
                name
            )
        })?;

    /*
     * Native binary:
     *
     * /proc/<pid>/exe points directly to the
     * executable declared by the module.
     */
    let proc_exe = PathBuf::from(format!("/proc/{pid}/exe"));

    if let Ok(actual_exe) = fs::read_link(&proc_exe) {
        if let Ok(actual_exe) = actual_exe.canonicalize() {
            if actual_exe == expected {
                return Ok(());
            }
        }
    }

    /*
     * Interpreted provider:
     *
     * Python, Bash, Node, etc. expose the runtime
     * through /proc/<pid>/exe. The provider script
     * must therefore appear as one exact argv entry.
     */
    let cmdline = fs::read(format!("/proc/{pid}/cmdline")).map_err(|error| {
        format!(
            "could not inspect tray provider process {} for module '{}': {error}",
            pid, name
        )
    })?;

    let expected_bytes = expected.as_os_str().as_bytes();

    let matches_provider = cmdline
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .any(|argument| argument == expected_bytes);

    if matches_provider {
        return Ok(());
    }

    Err(format!(
        "process {} is not the tray provider declared by module '{}'",
        pid, name
    ))
}

pub fn start_tray_provider(name: &str) -> Result<bool, String> {
    if !config::module_enabled(name)? {
        return Ok(false);
    }

    let module_dir = find_module_dir(name)?;

    let manifest = read_manifest(&module_dir.join("manifest.json"))?;

    /*
     * No tray capability means there is no tray
     * lifecycle to manage. This is not an error.
     */
    if manifest.tray.is_none() {
        return Ok(false);
    }

    if tray_provider_running(name) {
        return Ok(false);
    }

    let socket_path = crate::tray::protocol::socket_path();

    /*
     * The provider belongs to the Tray Manager
     * lifecycle. If no manager socket exists yet,
     * enabling/installing the module remains valid;
     * the provider will be started when tray serve
     * comes online.
     */
    if !socket_path.exists() {
        return Ok(false);
    }

    let contract = resolved_tray_contract(name)?;

    let provider = PathBuf::from(&contract.provider);

    let metadata = fs::metadata(&provider).map_err(|error| {
        format!(
            "could not inspect tray provider {}: {error}",
            provider.display()
        )
    })?;

    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!(
            "module '{}' tray provider is not executable: {}",
            name,
            provider.display()
        ));
    }

    let config_path = config::config_path()?;

    let requested_language = config::load_or_initialize()?.language;

    let language = resolve_module_language(&module_dir, &requested_language)?;

    let domestic_environment = BTreeMap::from([
        ("NEEBLES_LANGUAGE".to_string(), language),
        ("NEEBLES_CALLER".to_string(), "tray-manager".to_string()),
        ("NEEBLES_MODULE".to_string(), name.to_string()),
        (
            "NEEBLES_CONFIG".to_string(),
            config_path.display().to_string(),
        ),
        (
            "NEEBLES_TRAY_SOCKET".to_string(),
            socket_path.display().to_string(),
        ),
    ]);

    let mut command = neebles_backend::domestic_environment::build_process_command(
        &provider,
        neebles_backend::domestic_environment::ProcessEnvironmentClass::Pure,
        &domestic_environment,
        &BTreeMap::new(),
        &std::collections::BTreeSet::new(),
    )?;

    command
        .current_dir(&module_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = command.spawn().map_err(|error| {
        format!(
            "could not launch tray provider for module '{}': {error}",
            name
        )
    })?;

    let pid = child.id();

    if let Err(error) = write_tray_runtime_marker(name, pid) {
        let _ = child.kill();
        let _ = child.wait();

        return Err(error);
    }

    let owned_name = name.to_string();

    /*
     * Reap the provider when it exits and remove
     * only the marker belonging to this exact PID.
     * This avoids zombies and avoids an old provider
     * deleting a marker belonging to a newer one.
     */
    std::thread::spawn(move || {
        let _ = child.wait();

        clear_tray_runtime_marker_if_pid(&owned_name, pid);
    });

    Ok(true)
}

pub fn stop_tray_provider(name: &str) -> Result<bool, String> {
    let Some(pid) = probe_tray_provider_pid(name)? else {
        return Ok(false);
    };

    let Some(identity) = capture_process_identity(pid)? else {
        clear_tray_runtime_marker_if_pid(name, pid);
        return Ok(false);
    };

    if probe_tray_provider_pid(name)? != Some(pid) || !process_identity_is_alive(identity)? {
        return Err(format!(
            "module '{}' tray provider process {} changed identity while lifecycle ownership was being verified",
            name, pid
        ));
    }

    let signal_result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };

    if signal_result != 0 {
        return Err(format!(
            "could not send SIGTERM to tray provider for module '{}' process {}: {}",
            name,
            pid,
            std::io::Error::last_os_error()
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

    while process_identity_is_alive(identity)? && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if !process_identity_is_alive(identity)? {
        clear_tray_runtime_marker_if_pid(name, pid);
        return Ok(true);
    }

    let signal_result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };

    if signal_result != 0 {
        return Err(format!(
            "could not send SIGKILL to tray provider for module '{}' process {}: {}",
            name,
            pid,
            std::io::Error::last_os_error()
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);

    while process_identity_is_alive(identity)? && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if process_identity_is_alive(identity)? {
        return Err(format!(
            "tray provider for module '{}' process {} did not terminate after SIGKILL",
            name, pid
        ));
    }

    clear_tray_runtime_marker_if_pid(name, pid);

    Ok(true)
}

pub fn start_enabled_tray_providers() -> Result<(), String> {
    let root = modules_root();

    if !root.exists() {
        return Ok(());
    }

    let mut errors = Vec::new();

    for entry in fs::read_dir(&root)
        .map_err(|error| format!("could not read {}: {error}", root.display()))?
    {
        let entry =
            entry.map_err(|error| format!("could not read module directory entry: {error}"))?;

        if !entry.path().is_dir() {
            continue;
        }

        let manifest_path = entry.path().join("manifest.json");

        if !manifest_path.exists() {
            continue;
        }

        let manifest = match read_manifest(&manifest_path) {
            Ok(manifest) => manifest,

            Err(error) => {
                errors.push(error);
                continue;
            }
        };

        if manifest.tray.is_none() {
            continue;
        }

        match config::module_enabled(&manifest.name) {
            Ok(true) => {}

            Ok(false) => continue,

            Err(error) => {
                errors.push(error);
                continue;
            }
        }

        if let Err(error) = start_tray_provider(&manifest.name) {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "one or more tray providers could not be started: {}",
            errors.join(" | ")
        ))
    }
}

fn registry_url() -> Result<String, String> {
    /*
     * Explicit override is respected exactly.
     * Useful for development/testing and alternate registries.
     */
    if let Ok(value) = env::var("NEEBLES_MODULES_REGISTRY") {
        if !value.trim().is_empty() {
            return Ok(value);
        }
    }

    /*
     * Never read the production registry through raw/main.
     *
     * raw.githubusercontent.com may keep the branch URL cached
     * for several minutes after main has advanced.
     *
     * Boss first resolves the exact main commit and then reads
     * registry/modules.json from that immutable commit SHA.
     */
    let arguments = [
        std::ffi::OsString::from("ls-remote"),
        std::ffi::OsString::from("https://github.com/krockzs/neebles-boss.git"),
        std::ffi::OsString::from("refs/heads/main"),
    ];

    let output = build_network_boundary_command("boss.git", &arguments)?
        .output()
        .map_err(|error| format!("could not resolve N.E.E.B.L.E.S. registry revision: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "could not resolve N.E.E.B.L.E.S. registry revision: {}",
            output.status
        ));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        format!("invalid git ls-remote output while resolving registry: {error}")
    })?;

    let commit = stdout.split_whitespace().next().ok_or_else(|| {
        "git ls-remote returned no revision for N.E.E.B.L.E.S. Boss main".to_string()
    })?;

    if commit.len() != 40
        || !commit
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(format!(
            "invalid N.E.E.B.L.E.S. registry revision returned by git: {commit}"
        ));
    }

    Ok(format!(
        "https://raw.githubusercontent.com/krockzs/neebles-boss/{commit}/registry/modules.json"
    ))
}

pub fn fetch_registry() -> Result<Registry, String> {
    let url = registry_url()?;
    let arguments = [
        std::ffi::OsString::from("-fsSL"),
        std::ffi::OsString::from(url.as_str()),
    ];

    let output = build_network_boundary_command("boss.curl", &arguments)?
        .output()
        .map_err(|error| format!("could not start curl: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "could not read N.E.E.B.L.E.S. module registry: {}",
            output.status
        ));
    }

    let registry: Registry = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid N.E.E.B.L.E.S. module registry: {error}"))?;

    validate_registry(&registry)?;

    Ok(registry)
}

pub fn installed_modules_json() -> Result<Value, String> {
    let mut result = Vec::new();
    let root = modules_root();
    if !root.exists() {
        return Ok(json!([]));
    }

    for entry in fs::read_dir(&root)
        .map_err(|error| format!("could not read {}: {error}", root.display()))?
    {
        let entry =
            entry.map_err(|error| format!("could not read module directory entry: {error}"))?;
        if !entry.path().is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }
        let manifest = read_manifest(&manifest_path)?;
        let enabled = config::module_enabled(&manifest.name)?;
        let icon = local_module_icon(&entry.path());
        let running = module_running(&manifest.name);

        let launcher_action = launcher_action_from(&entry.path(), &manifest)?;

        let mut item = json!({
            "name": manifest.name,
            "version": manifest.version,
            "enabled": enabled,
            "running": running,
            "path": entry.path(),
            "icon": icon,
            "launcher_action": launcher_action,
            "notifications": manifest.notifications.as_ref().map(|contract| {
                json!({
                    "protocol": contract.protocol,
                })
            }),
        });

        if let Some(tray) = manifest.tray.as_ref() {
            item.as_object_mut()
                .expect("module JSON must be an object")
                .insert(
                    "tray".to_string(),
                    json!({
                        "protocol": tray.protocol,
                        "icon": tray.icon,
                        "provider": tray.provider,
                    }),
                );
        }

        result.push(item);
    }

    result.sort_by(|a, b| {
        a.get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .cmp(b.get("name").and_then(Value::as_str).unwrap_or_default())
    });
    Ok(Value::Array(result))
}

pub fn available_modules_json() -> Result<Value, String> {
    let registry = fetch_registry()?;
    let installed = installed_names()?;
    let mut result = Vec::new();

    for (name, module) in registry.modules {
        let is_installed = installed.contains(&name);

        let icon = remote_module_icon(&module.repo, &module.commit);

        result.push(json!({
            "name": name,
            "version": module.version,
            "repo": module.repo,
            "icon": icon,
            "installed": is_installed,
        }));
    }

    Ok(Value::Array(result))
}

fn installed_names() -> Result<HashSet<String>, String> {
    let value = installed_modules_json()?;
    Ok(value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("name").and_then(Value::as_str).map(str::to_string))
        .collect())
}

pub fn install(name: &str) -> Result<(), String> {
    let registry = fetch_registry()?;
    install_internal(name, &registry)
}

fn install_internal(name: &str, registry: &Registry) -> Result<(), String> {
    let lifecycle_runtime = module_governor_lifecycle_runtime()?;

    install_require_tree(name, registry, &lifecycle_runtime)
}

pub fn module_has_local_state(name: &str) -> Result<bool, String> {
    /*
     * Require an installed module.
     *
     * This keeps the query aligned with uninstall semantics
     * and prevents arbitrary module ids from becoming settings
     * filesystem lookups.
     */
    let _ = find_module_dir(name)?;

    let local_settings_path = settings::module_settings_path(&neebles_root(), name);

    let module_settings_have_state = if local_settings_path.exists() {
        let local = settings::load(&local_settings_path)?;

        let default = installed_module_settings_default(name)?;

        settings::validate_local(&local, &default)?;

        settings::has_local_values(&local)?
    } else {
        false
    };

    let boss_settings_have_state = config::module_has_user_state(name)?;

    Ok(module_settings_have_state || boss_settings_have_state)
}

pub fn uninstall(name: &str, remove_settings: bool) -> Result<(), String> {
    let lifecycle_runtime = module_governor_lifecycle_runtime()?;

    uninstall_internal(name, remove_settings, Some(&lifecycle_runtime))
}

fn uninstall_without_lifecycle(name: &str, remove_settings: bool) -> Result<(), String> {
    uninstall_internal(name, remove_settings, None)
}

fn uninstall_internal(
    name: &str,
    remove_settings: bool,
    lifecycle_runtime: Option<&crate::lifecycle_governor_runtime::GovernorLifecycleRuntime>,
) -> Result<(), String> {
    /*
     * Uninstall is inherently destructive and therefore
     * owns the complete module lifecycle.
     *
     * A module cannot remain alive after its installation
     * has been removed.
     */
    if let Some(lifecycle_runtime) = lifecycle_runtime {
        let lifecycle = installed_module_lifecycle_contract(name)?;

        execute_module_governor_lifecycle(lifecycle_runtime, name, "uninstall", &lifecycle)?;
    }

    deactivate_module(name, "uninstall")?;

    let path = find_module_dir(name)?;

    let local_settings_path = settings::module_settings_path(&neebles_root(), name);

    let local_settings_has_state = module_has_local_state(name)?;

    /*
     * No meaningful user state:
     * remove everything automatically.
     *
     * Meaningful local state:
     * preserve it unless removal was explicitly requested.
     */
    let remove_local_settings = !local_settings_has_state || remove_settings;

    /*
     * Removing an installed module is transactional at the
     * active-installation boundary.
     *
     * First move it outside modules_root(), so Boss no
     * longer considers it installed, while retaining the
     * possibility of restoring it if Boss-owned state
     * cannot be cleaned.
     */
    let removal_root = neebles_root().join("shared/tmp");

    fs::create_dir_all(&removal_root).map_err(|error| {
        format!(
            "could not create uninstall staging directory {}: {error}",
            removal_root.display()
        )
    })?;

    let removal_path = removal_root.join(format!("uninstall-{name}-{}", transaction_id()));

    if removal_path.exists() {
        return Err(format!(
            "uninstall staging path already exists and will not be overwritten: {}",
            removal_path.display()
        ));
    }

    fs::rename(&path, &removal_path).map_err(|error| {
        format!(
            "could not move module '{}' into uninstall staging {}: {error}",
            name,
            removal_path.display()
        )
    })?;

    /*
     * When settings must be removed, stage them instead of
     * destroying them immediately.
     *
     * This lets Boss restore them if the uninstall transaction
     * has to roll back.
     */
    let staged_settings_path = if remove_local_settings && local_settings_path.exists() {
        let staged =
            local_settings_path.with_extension(format!("json.uninstall-{}", transaction_id()));

        if staged.exists() {
            let _ = fs::rename(&removal_path, &path);

            return Err(format!(
                "settings uninstall staging path already exists and will not be overwritten: {}",
                staged.display()
            ));
        }

        if let Err(error) = fs::rename(&local_settings_path, &staged) {
            let rollback_error = fs::rename(&removal_path, &path).err();

            return match rollback_error {
                    None => Err(format!(
                        "could not stage local settings for module '{}': {}; module installation was restored",
                        name,
                        error
                    )),

                    Some(rollback_error) =>
                        Err(format!(
                            "CRITICAL: could not stage local settings for module '{}': {}; module rollback also failed: {}",
                            name,
                            error,
                            rollback_error
                        )),
                };
        }

        Some(staged)
    } else {
        None
    };

    /*
     * Boss-owned state for an uninstalled module must not
     * survive the uninstall.
     *
     * If configuration cleanup fails, restore the module
     * to its original active path.
     */
    let config_cleanup = if remove_local_settings {
        config::remove_module_state(name)
    } else {
        config::remove_module_transient_state(name)
    };

    if let Err(config_error) = config_cleanup {
        if let Some(staged) = staged_settings_path.as_ref() {
            let _ = fs::rename(staged, &local_settings_path);
        }

        match fs::rename(&removal_path, &path) {
            Ok(()) => {
                return Err(format!(
                    "could not clean Boss state while uninstalling module '{}': {}; module installation and local settings were restored",
                    name,
                    config_error
                ));
            }

            Err(rollback_error) => {
                return Err(format!(
                    "CRITICAL: could not clean Boss state while uninstalling module '{}': {}; rollback also failed: {}; module remains at {}",
                    name,
                    config_error,
                    rollback_error,
                    removal_path.display()
                ));
            }
        }
    }

    /*
     * Installation and Boss state are now detached.
     * Destroy the staged copy last.
     */
    if let Err(error) = fs::remove_dir_all(&removal_path) {
        /*
         * Module files could not be destroyed.
         * Do not destroy staged local settings either.
         */
        if let Some(staged) = staged_settings_path.as_ref() {
            let _ = fs::rename(staged, &local_settings_path);
        }

        return Err(format!(
            "module '{}' was removed from the active installation and its Boss state was cleaned, but uninstall staging {} could not be deleted: {error}",
            name,
            removal_path.display()
        ));
    }

    /*
     * Module uninstall is committed.
     *
     * Only now destroy local settings that were selected for
     * removal. Preserved settings never left their final path.
     */
    if let Some(staged) = staged_settings_path.as_ref() {
        if let Err(error) = fs::remove_file(staged) {
            let _ = fs::rename(staged, &local_settings_path);

            return Err(format!(
                "module '{}' was uninstalled, but its local settings could not be removed and were preserved instead: {error}",
                name
            ));
        }
    }

    remove_deactivate_entry(name)?;

    if let Err(error) = crate::module_ipc::runtime_registry().broadcast_event(
        "module.lifecycle",
        "uninstalled",
        serde_json::json!({
            "module": name,
            "settings_preserved": !remove_local_settings
        }),
    ) {
        eprintln!(
            "N.E.E.B.L.E.S.: module '{}' uninstalled but lifecycle event broadcast failed: {}",
            name, error
        );
    }

    Ok(())
}

pub fn update(name: &str, close_running: bool) -> Result<(), String> {
    /*
     * Without explicit permission Boss must never close
     * live module processes as part of an update.
     *
     * This first check fails fast, before doing network or
     * staging work.
     */
    if !close_running && module_has_active_resources(name)? {
        return Err(format!("module '{name}' currently has active resources"));
    }

    let current_path = find_module_dir(name)?;

    let registry = fetch_registry()?;

    let entry = registry
        .modules
        .get(name)
        .ok_or_else(|| format!("module '{name}' does not exist in the N.E.E.B.L.E.S. registry"))?;

    /*
     * Stage the candidate INSIDE modules_root().
     *
     * That keeps the final rename on the same filesystem,
     * allowing the commit/rollback operation to remain
     * atomic at filesystem rename level.
     */
    let root = modules_root();

    fs::create_dir_all(&root)
        .map_err(|error| format!("could not create module root {}: {error}", root.display()))?;

    let transaction = transaction_id();

    let staging_path = root.join(format!(".neebles-update-{name}-{transaction}"));

    let backup_path = root.join(format!(".neebles-backup-{name}-{transaction}"));

    /*
     * Transaction paths are unique.
     *
     * Boss must never destroy a pre-existing staging or
     * backup directory merely because its name resembles
     * an old transaction. Such a directory may contain
     * recovery data from a previous failure.
     */
    let staging = ModuleInstallStaging::prepare(staging_path.clone())?;

    if backup_path.exists() {
        return Err(format!(
            "transactional backup path already exists and will not be removed automatically: {}",
            backup_path.display()
        ));
    }

    /*
     * Clone the candidate instead of mutating the current
     * checkout with git reset.
     */
    checkout_registry_commit(name, entry, staging.path(), "boss.modules.update_staging")?;

    /*
     * read_manifest() is now the schema-3 gate.
     *
     * Before touching the installed copy this validates:
     * - schema
     * - name format
     * - semantic version
     * - entrypoint
     * - commands
     * - lifecycle values through serde
     * - launcher contract
     * - tray contract
     * - notifications protocol
     * - language contract
     * - safe paths
     */
    let manifest = read_manifest(&staging.path().join("manifest.json"))?;

    let lifecycle = lifecycle_contract_from_module(staging.path(), &manifest)?;

    let settings_default = module_settings_default_from(staging.path(), &manifest)?;

    if manifest.name != name {
        return Err(format!(
            "staged module manifest name '{}' does not match registry id '{name}'",
            manifest.name
        ));
    }

    if manifest.version != entry.version {
        return Err(format!(
            "module '{}' staged version '{}' does not match registry version '{}'",
            name, manifest.version, entry.version
        ));
    }

    /*
     * Candidate is now completely staged and validated.
     *
     * Only at this boundary may an explicitly authorized
     * update close live processes. Keeping the currently
     * installed module alive until this point minimizes
     * downtime and guarantees that download/validation
     * failures never stop the working version.
     *
     * Re-probe here as well: a process may have started
     * after the initial preflight.
     */
    if module_has_active_resources(name)? {
        if !close_running {
            return Err(format!(
                "module '{name}' acquired active resources while the update was being prepared"
            ));
        }

        deactivate_module(name, "update")?;
    }

    /*
     * Candidate is valid and runtime users are out.
     *
     * Perform the smallest possible filesystem transaction:
     *
     * current -> backup
     * staged  -> current
     */
    fs::rename(&current_path, &backup_path).map_err(|error| {
        format!(
            "could not move current module '{}' into transactional backup {}: {error}",
            name,
            backup_path.display()
        )
    })?;

    match staging.commit(&current_path) {
        Ok(()) => {}

        Err(commit_error) => {
            /*
             * The new version could not take the live path.
             * Restore the previous known-good copy.
             */
            match fs::rename(&backup_path, &current_path) {
                Ok(()) => {
                    return Err(format!(
                        "could not commit staged update for module '{}': {}; previous version was restored",
                        name,
                        commit_error
                    ));
                }

                Err(rollback_error) => {
                    return Err(format!(
                        "CRITICAL: could not commit staged update for module '{}': {}; rollback also failed: {}; previous module remains at {}",
                        name,
                        commit_error,
                        rollback_error,
                        backup_path.display()
                    ));
                }
            }
        }
    }

    let lifecycle_runtime = module_governor_lifecycle_runtime()?;

    if let Err(lifecycle_error) =
        execute_module_governor_lifecycle(&lifecycle_runtime, name, "update", &lifecycle)
    {
        let failed_update_path = root.join(format!(
            ".neebles-failed-lifecycle-update-{name}-{transaction}"
        ));

        if let Err(error) = fs::rename(&current_path, &failed_update_path) {
            return Err(format!(
                "CRITICAL: update Lifecycle failed for module '{}': {}; new module could not be moved aside for rollback: {}",
                name,
                lifecycle_error,
                error
            ));
        }

        match fs::rename(&backup_path, &current_path) {
            Ok(()) => {
                let _ = fs::remove_dir_all(&failed_update_path);

                return Err(format!(
                    "update Lifecycle failed for module '{}': {}; previous module version was restored",
                    name,
                    lifecycle_error
                ));
            }

            Err(rollback_error) => {
                return Err(format!(
                    "CRITICAL: update Lifecycle failed for module '{}': {}; rollback also failed: {}; previous version remains at {} and failed update remains at {}",
                    name,
                    lifecycle_error,
                    rollback_error,
                    backup_path.display(),
                    failed_update_path.display()
                ));
            }
        }
    }

    /*
     * The new copy is now live.
     *
     * Reconcile persistent settings against the new module
     * default before destroying the known-good backup.
     */
    let settings_path = settings::module_settings_path(&neebles_root(), name);

    if let Err(settings_error) = settings::update_from_default(&settings_path, &settings_default) {
        /*
         * Settings reconciliation is part of the update
         * transaction. Restore the previous module version.
         */
        let failed_update_path = root.join(format!(".neebles-failed-update-{name}-{transaction}"));

        if let Err(error) = fs::rename(&current_path, &failed_update_path) {
            return Err(format!(
                "CRITICAL: module '{}' was updated but settings reconciliation failed: {}; new module could not be moved aside for rollback: {}",
                name,
                settings_error,
                error
            ));
        }

        match fs::rename(&backup_path, &current_path) {
            Ok(()) => {
                let _ = fs::remove_dir_all(&failed_update_path);

                return Err(format!(
                    "could not reconcile settings for module '{}': {}; previous module version was restored",
                    name,
                    settings_error
                ));
            }

            Err(rollback_error) => {
                return Err(format!(
                    "CRITICAL: could not reconcile settings for module '{}': {}; module rollback also failed: {}; previous version remains at {} and failed update remains at {}",
                    name,
                    settings_error,
                    rollback_error,
                    backup_path.display(),
                    failed_update_path.display()
                ));
            }
        }
    }

    /*
     * Module and persistent settings are now committed.
     * Remove the old known-good copy last.
     */
    if let Err(error) = fs::remove_dir_all(&backup_path) {
        eprintln!(
            "N.E.E.B.L.E.S.: module '{}' update was committed successfully, but transactional backup {} could not be removed: {error}",
            name,
            backup_path.display()
        );
    }

    if let Err(error) = register_deactivate_entry(name, &manifest) {
        eprintln!(
            "N.E.E.B.L.E.S.: module '{}' update committed, but deactivate registry refresh failed: {}; Boss will reconcile it on the next lifecycle operation",
            name,
            error
        );
    }

    Ok(())
}
pub fn set_enabled(name: &str, enabled: bool) -> Result<(), String> {
    let _ = find_module_dir(name)?;

    let previous = config::module_enabled(name)?;

    let lifecycle = installed_module_lifecycle_contract(name)?;

    let lifecycle_runtime = module_governor_lifecycle_runtime()?;

    if !enabled {
        if previous {
            execute_module_governor_lifecycle(&lifecycle_runtime, name, "disable", &lifecycle)?;
        }

        deactivate_module(name, "disabled")?;

        config::set_module_enabled(name, false)?;

        if previous {
            if let Err(error) = crate::module_ipc::runtime_registry().broadcast_event(
                "module.lifecycle",
                "disabled",
                serde_json::json!({
                    "module": name
                }),
            ) {
                eprintln!(
                    "N.E.E.B.L.E.S.: module '{}' disabled but lifecycle event broadcast failed: {}",
                    name, error
                );
            }
        }

        return Ok(());
    }

    config::set_module_enabled(name, true)?;

    if let Err(error) = start_tray_provider(name) {
        /*
         * Enabling is one operation. Do not leave
         * Boss saying "enabled" when the declared
         * tray lifecycle could not be started.
         */
        let _ = config::set_module_enabled(name, false);

        return Err(error);
    }

    if !previous {
        if let Err(error) =
            execute_module_governor_lifecycle(&lifecycle_runtime, name, "enable", &lifecycle)
        {
            let tray_cleanup = stop_tray_provider(name);

            let config_cleanup = config::set_module_enabled(name, false);

            let tray_text = match tray_cleanup {
                Ok(_) => "tray compensation completed".to_string(),

                Err(cleanup_error) => {
                    format!("tray compensation failed: {cleanup_error}")
                }
            };

            let config_text = match config_cleanup {
                Ok(_) => "enabled flag was restored".to_string(),

                Err(cleanup_error) => {
                    format!("enabled flag compensation failed: {cleanup_error}")
                }
            };

            return Err(format!(
                "enable Lifecycle failed for module '{}': {}; {}; {}",
                name, error, tray_text, config_text
            ));
        }
        if let Err(error) = crate::module_ipc::runtime_registry().broadcast_event(
            "module.lifecycle",
            "enabled",
            serde_json::json!({
                "module": name
            }),
        ) {
            eprintln!(
                "N.E.E.B.L.E.S.: module '{}' enabled but lifecycle event broadcast failed: {}",
                name, error
            );
        }
    }

    Ok(())
}

fn resolve_module_language(module_dir: &Path, requested: &str) -> Result<String, String> {
    let manifest_path = module_dir.join("languages").join("manifest.json");

    /*
     * Schema 3 modules always have a validated language
     * contract. No implicit legacy behavior exists here.
     */
    validate_module_language_contract(module_dir)?;

    let raw = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "could not read module language manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    let manifest: ModuleLanguageManifest = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid module language manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    let requested = languages::normalize_locale(requested);

    if let Some(language) = manifest
        .languages
        .iter()
        .find(|language| languages::normalize_locale(&language.code) == requested)
    {
        return Ok(language.code.clone());
    }

    let normalized_default = languages::normalize_locale(manifest.default.trim());

    let language = manifest
        .languages
        .iter()
        .find(|language| languages::normalize_locale(&language.code) == normalized_default)
        .ok_or_else(|| {
            format!(
                "module language manifest {} defines invalid default '{}'",
                manifest_path.display(),
                manifest.default
            )
        })?;

    Ok(language.code.clone())
}

fn desktop_session_environment(
    desktop_uid: libc::uid_t,
    desktop_gid: libc::gid_t,
) -> Result<(BTreeMap<String, String>, String, String), String> {
    let runtime_dir = format!("/run/user/{desktop_uid}");

    let session_bus = format!("unix:path={runtime_dir}/bus");

    let current_uid = unsafe { libc::geteuid() };

    let mut command;

    let protocol_environment = BTreeMap::from([
        ("XDG_RUNTIME_DIR".to_string(), runtime_dir.clone()),
        ("DBUS_SESSION_BUS_ADDRESS".to_string(), session_bus.clone()),
    ]);

    if current_uid == desktop_uid {
        command = neebles_backend::domestic_environment::build_process_command(
            neebles_backend::domestic_runtime_authority::resolve_boss_executable("boss.systemctl")?,
            neebles_backend::domestic_environment::ProcessEnvironmentClass::SystemInterface,
            &protocol_environment,
            &BTreeMap::new(),
            &std::collections::BTreeSet::new(),
        )?;

        command.arg("--user").arg("show-environment");
    } else if current_uid == 0 {
        command = neebles_backend::domestic_environment::build_process_command(
            neebles_backend::domestic_runtime_authority::resolve_boss_executable("boss.setpriv")?,
            neebles_backend::domestic_environment::ProcessEnvironmentClass::SystemInterface,
            &protocol_environment,
            &BTreeMap::new(),
            &std::collections::BTreeSet::new(),
        )?;

        command
            .arg(format!("--reuid={desktop_uid}"))
            .arg(format!("--regid={desktop_gid}"))
            .arg("--init-groups")
            .arg(
                neebles_backend::domestic_runtime_authority::resolve_boss_executable(
                    "boss.systemctl",
                )?,
            )
            .arg("--user")
            .arg("show-environment");
    } else {
        return Err(format!(
            "Boss process uid {current_uid} cannot inspect desktop session for uid {desktop_uid}"
        ));
    }

    let output = command
        .output()
        .map_err(|error| format!("could not inspect desktop user session environment: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        return Err(format!(
            "desktop user manager rejected environment query with status {}: {}",
            output.status,
            stderr.trim()
        ));
    }

    let raw = String::from_utf8(output.stdout).map_err(|error| {
        format!("desktop user manager returned non-UTF-8 environment data: {error}")
    })?;

    Ok((parse_environment_lines(&raw), runtime_dir, session_bus))
}

fn build_module_execution_command(
    entrypoint: &Path,
    args: &[String],
    contract: &CommandContract,
    language: &str,
    caller: &str,
    name: &str,
    config_path: &Path,
) -> Result<Command, String> {
    let mut domestic = BTreeMap::<String, String>::new();

    domestic.insert("NEEBLES_LANGUAGE".to_string(), language.to_string());

    domestic.insert("NEEBLES_CALLER".to_string(), caller.to_string());

    domestic.insert("NEEBLES_MODULE".to_string(), name.to_string());

    domestic.insert(
        "NEEBLES_CONFIG".to_string(),
        config_path.display().to_string(),
    );

    if contract.requires_root {
        let mut command = neebles_backend::domestic_environment::build_process_command(
            entrypoint,
            neebles_backend::domestic_environment::ProcessEnvironmentClass::Pure,
            &domestic,
            &BTreeMap::new(),
            &std::collections::BTreeSet::new(),
        )?;

        command.args(args);

        return Ok(command);
    }

    let (session_environment, runtime_dir, session_bus) = desktop_session_environment(
        crate::runtime_identity::desktop_identity()?.0,
        crate::runtime_identity::desktop_identity()?.1,
    )?;

    domestic.insert("XDG_RUNTIME_DIR".to_string(), runtime_dir);

    domestic.insert("DBUS_SESSION_BUS_ADDRESS".to_string(), session_bus);

    let allowed_session_inputs = contract
        .allowed_session_inputs
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();

    let (desktop_uid, desktop_gid) = crate::runtime_identity::desktop_identity()?;

    let current_uid = unsafe { libc::geteuid() };

    if current_uid == desktop_uid {
        let mut command = neebles_backend::domestic_environment::build_process_command(
            entrypoint,
            neebles_backend::domestic_environment::ProcessEnvironmentClass::Session,
            &domestic,
            &session_environment,
            &allowed_session_inputs,
        )?;

        command.args(args);

        return Ok(command);
    }

    if current_uid != 0 {
        return Err(
            format!(
                "Boss process uid {current_uid} cannot execute non-root module action as desktop uid {desktop_uid}"
            )
        );
    }

    let sealed = neebles_backend::domestic_environment::build_process_environment(
        neebles_backend::domestic_environment::ProcessEnvironmentClass::Session,
        &domestic,
        &session_environment,
        &allowed_session_inputs,
    )?;

    let mut command = neebles_backend::domestic_environment::build_pure_process_command(
        neebles_backend::domestic_runtime_authority::resolve_boss_executable("boss.setpriv")?,
    )?;

    command
        .arg(format!("--reuid={desktop_uid}"))
        .arg(format!("--regid={desktop_gid}"))
        .arg("--init-groups")
        .arg("--reset-env")
        .arg(neebles_backend::domestic_runtime_authority::resolve_boss_executable("boss.env")?);

    for (key, value) in sealed {
        command.arg(format!("{key}={value}"));
    }

    command.arg(entrypoint).args(args);

    Ok(command)
}

pub fn execute(name: &str, args: &[String], caller: &str) -> Result<i32, String> {
    if !config::module_enabled(name)? {
        return Err(format!("module '{name}' is disabled"));
    }

    let module_dir = find_module_dir(name)?;

    let manifest = read_manifest(&module_dir.join("manifest.json"))?;

    let action = args.first().map(String::as_str).unwrap_or("default");

    let contract = manifest
        .commands
        .get(action)
        .ok_or_else(|| format!("module '{}' does not declare command '{}'", name, action))?;

    let track_runtime = contract.lifecycle == CommandLifecycle::Tracked;

    if track_runtime && module_running(name) {
        return Err(format!("module '{name}' is already running"));
    }

    privileges::ensure_root(contract.requires_root)?;

    let entrypoint = resolve_entrypoint(&module_dir, &manifest.entrypoint)?;

    let requested_language = config::load_or_initialize()?.language;

    let language = resolve_module_language(&module_dir, &requested_language)?;

    let config_path = config::config_path()?;

    let mut command = build_module_execution_command(
        &entrypoint,
        args,
        contract,
        &language,
        caller,
        name,
        &config_path,
    )?;

    command
        .current_dir(&module_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = command
        .spawn()
        .map_err(|error| format!("could not launch module '{name}': {error}"))?;

    if track_runtime {
        if let Err(error) = write_runtime_marker(name, child.id()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    }

    let result = child
        .wait()
        .map_err(|error| format!("could not wait for module '{name}': {error}"));

    if track_runtime {
        clear_runtime_marker(name);
    }

    let status = result?;

    Ok(status.code().unwrap_or(1))
}
pub fn find_module_dir(name: &str) -> Result<PathBuf, String> {
    if !valid_module_id(name) {
        return Err(format!("invalid module id: {name}"));
    }

    /*
     * Fast path: the normal installation layout uses the
     * module id as its directory name.
     *
     * If that exact path exists, it is the candidate the
     * caller asked for, so validate it fully.
     */
    let direct = modules_root().join(name);
    let direct_manifest = direct.join("manifest.json");

    if direct_manifest.exists() {
        let manifest = read_manifest(&direct_manifest)?;

        if manifest.name != name {
            return Err(format!(
                "module directory '{}' contains manifest for '{}', expected '{}'",
                direct.display(),
                manifest.name,
                name
            ));
        }

        return Ok(direct);
    }

    /*
     * Compatibility path: registry entries may install a
     * module under a folder whose name differs from its
     * manifest identity.
     *
     * Do not fully validate every unrelated module while
     * searching. One broken installation must not poison
     * lookup of another module.
     *
     * First inspect only enough JSON to discover identity.
     * Full validation happens only when the requested
     * module name matches.
     */
    let root = modules_root();

    if root.exists() {
        for entry in fs::read_dir(&root)
            .map_err(|error| format!("could not read {}: {error}", root.display()))?
        {
            let entry =
                entry.map_err(|error| format!("could not read module directory entry: {error}"))?;

            let manifest_path = entry.path().join("manifest.json");

            if !manifest_path.is_file() {
                continue;
            }

            let raw = match fs::read_to_string(&manifest_path) {
                Ok(value) => value,
                Err(_) => continue,
            };

            let value: Value = match serde_json::from_str(&raw) {
                Ok(value) => value,
                Err(_) => continue,
            };

            let Some(candidate_name) = value.get("name").and_then(Value::as_str) else {
                continue;
            };

            if candidate_name != name {
                continue;
            }

            /*
             * This is now the module requested by the caller.
             * From this point onward failures are relevant and
             * must be surfaced rather than ignored.
             */
            let manifest = read_manifest(&manifest_path)?;

            if manifest.name != name {
                return Err(format!(
                    "module identity changed while resolving '{}'",
                    name
                ));
            }

            return Ok(entry.path());
        }
    }

    Err(format!("module '{name}' is not installed"))
}

fn validate_module_language_contract(module_dir: &Path) -> Result<(), String> {
    let manifest_path = module_dir.join("languages").join("manifest.json");

    if !manifest_path.is_file() {
        return Err(format!(
            "module schema {} requires a language manifest: {}",
            MODULE_SCHEMA_VERSION,
            manifest_path.display()
        ));
    }

    let raw = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "could not read module language manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    let manifest: ModuleLanguageManifest = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid module language manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    if manifest.schema != MODULE_LANGUAGE_SCHEMA_VERSION {
        return Err(format!(
            "module language manifest {} declares unsupported schema {}; expected {}",
            manifest_path.display(),
            manifest.schema,
            MODULE_LANGUAGE_SCHEMA_VERSION
        ));
    }

    if manifest.languages.is_empty() {
        return Err(format!(
            "module language manifest {} declares no languages",
            manifest_path.display()
        ));
    }

    let mut normalized_languages = HashSet::new();

    for language in &manifest.languages {
        let code = language.code.trim();

        if code.is_empty() {
            return Err(format!(
                "module language manifest {} contains an empty language code",
                manifest_path.display()
            ));
        }

        let normalized = languages::normalize_locale(code);

        if normalized.is_empty() {
            return Err(format!(
                "module language manifest {} contains invalid language code '{}'",
                manifest_path.display(),
                language.code
            ));
        }

        if !normalized_languages.insert(normalized.clone()) {
            return Err(format!(
                "module language manifest {} declares duplicate language '{}'",
                manifest_path.display(),
                normalized
            ));
        }
    }

    let default = manifest.default.trim();

    if default.is_empty() {
        return Err(format!(
            "module language manifest {} must declare a default language",
            manifest_path.display()
        ));
    }

    let normalized_default = languages::normalize_locale(default);

    if !normalized_languages.contains(&normalized_default) {
        return Err(format!(
            "module language manifest {} defines default '{}' but that language is not declared",
            manifest_path.display(),
            manifest.default
        ));
    }

    Ok(())
}

fn resolve_module_file(module_dir: &Path, value: &str, field: &str) -> Result<PathBuf, String> {
    let value = value.trim();

    if value.is_empty() {
        return Err(format!("module {field} cannot be empty"));
    }

    let relative = Path::new(value);

    if relative.is_absolute() {
        return Err(format!(
            "module {field} must be relative to the module directory"
        ));
    }

    for component in relative.components() {
        use std::path::Component;

        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("module {field} contains an invalid path: {value}"));
            }
        }
    }

    let candidate = module_dir.join(relative);

    let canonical = candidate.canonicalize().map_err(|error| {
        format!(
            "could not resolve module {field} {}: {error}",
            candidate.display()
        )
    })?;

    let canonical_module = module_dir.canonicalize().map_err(|error| {
        format!(
            "could not resolve module directory {}: {error}",
            module_dir.display()
        )
    })?;

    if !canonical.starts_with(&canonical_module) {
        return Err(format!("module {field} escapes module directory"));
    }

    if !canonical.is_file() {
        return Err(format!(
            "module {field} is not a file: {}",
            canonical.display()
        ));
    }

    Ok(canonical)
}

/*
 * Resolve the single logical action that Boss may expose through
 * its global Quick Access / Launcher surface.
 *
 * Modules do not own launchers.
 *
 * A module may mark one action as launcher-capable through:
 *
 *   1. legacy Schema 3 manifest.commands.<action>.launcher
 *   2. dynamic "commands" contract endpoint <action>.launcher
 *
 * During migration the same logical action may be declared by both
 * sources. That is one action, not two. The dispatcher already gives
 * the dynamic "commands" endpoint execution precedence.
 *
 * launcher=true is invalid in every dynamic contract other than
 * "commands", because the Boss Launcher invokes the normal module
 * command surface.
 */
fn launcher_action_from_sources(
    module_name: &str,
    legacy_commands: &BTreeMap<String, CommandContract>,
    contracts: &ModuleContracts,
) -> Result<Option<String>, String> {
    let mut actions = std::collections::BTreeSet::<String>::new();

    for (name, command) in legacy_commands {
        if command.launcher {
            actions.insert(name.clone());
        }
    }

    for (contract_type, contract) in &contracts.contracts {
        for (name, endpoint) in &contract.endpoints {
            if !endpoint.launcher {
                continue;
            }

            if contract_type != "commands" {
                return Err(format!(
                    "module '{}' marks endpoint '{}.{}' as launcher=true, but Boss Launcher actions must belong to the 'commands' contract",
                    module_name, contract_type, name
                ));
            }

            actions.insert(name.clone());
        }
    }

    match actions.len() {
        0 => Ok(None),

        1 => Ok(actions.into_iter().next()),

        _ => Err(format!(
            "module '{}' declares more than one launcher action: {}",
            module_name,
            actions.into_iter().collect::<Vec<_>>().join(", ")
        )),
    }
}

fn launcher_action_from(
    module_dir: &Path,
    manifest: &ModuleManifest,
) -> Result<Option<String>, String> {
    let contracts = load_module_contracts(&manifest.name, module_dir, &manifest.contracts)?;

    launcher_action_from_sources(&manifest.name, &manifest.commands, &contracts)
}

pub fn installed_module_launcher_action(name: &str) -> Result<Option<String>, String> {
    if !valid_module_id(name) {
        return Err(format!("invalid module id: {name}"));
    }

    let module_dir = find_module_dir(name)?;
    let manifest = read_manifest(&module_dir.join("manifest.json"))?;

    launcher_action_from(&module_dir, &manifest)
}

fn validate_module_manifest(module_dir: &Path, manifest: &ModuleManifest) -> Result<(), String> {
    if manifest.schema != MODULE_SCHEMA_VERSION {
        return Err(format!(
            "module '{}' declares unsupported schema {}; expected {}",
            manifest.name, manifest.schema, MODULE_SCHEMA_VERSION
        ));
    }

    if !valid_module_id(&manifest.name) {
        return Err(format!("invalid module id: {}", manifest.name));
    }

    if manifest.version.trim().is_empty() {
        return Err(format!("module '{}' must declare a version", manifest.name));
    }

    Version::parse(manifest.version.trim()).map_err(|error| {
        format!(
            "module '{}' declares invalid semantic version '{}': {error}",
            manifest.name, manifest.version
        )
    })?;

    let entrypoint = resolve_module_file(module_dir, &manifest.entrypoint, "entrypoint")?;

    let metadata = fs::metadata(&entrypoint).map_err(|error| {
        format!(
            "could not inspect module '{}' entrypoint {}: {error}",
            manifest.name,
            entrypoint.display()
        )
    })?;

    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!(
            "module '{}' entrypoint is not executable: {}",
            manifest.name,
            entrypoint.display()
        ));
    }

    /*
     * A module may expose capabilities through legacy embedded
     * commands, dynamic contracts, or both.
     *
     * Boss must not require a specific contract type such as
     * "commands": contract names are intentionally generic.
     */
    if manifest.commands.is_empty() && manifest.contracts.is_empty() {
        return Err(format!(
            "module '{}' must declare at least one legacy command or dynamic contract",
            manifest.name
        ));
    }

    for action in manifest.commands.keys() {
        if !valid_module_id(action) {
            return Err(format!(
                "module '{}' declares invalid command id '{}'",
                manifest.name, action
            ));
        }
    }

    /*
     * Dynamic contracts are validated here as part of the installed
     * module contract, not lazily after installation.
     *
     * This also certifies the Boss Launcher invariant:
     * only the dynamic "commands" contract may expose launcher=true,
     * and legacy + dynamic declarations must resolve to one logical
     * launcher action.
     */
    let contracts = load_module_contracts(&manifest.name, module_dir, &manifest.contracts)?;

    let _ = launcher_action_from_sources(&manifest.name, &manifest.commands, &contracts)?;

    if let Some(tray) = &manifest.tray {
        if tray.protocol != crate::tray::protocol::TRAY_PROTOCOL_VERSION {
            return Err(format!(
                "module '{}' declares unsupported tray protocol {}; expected {}",
                manifest.name,
                tray.protocol,
                crate::tray::protocol::TRAY_PROTOCOL_VERSION
            ));
        }

        let _ = resolve_module_contract_path(module_dir, &tray.icon, "icon")?;
        let _ = resolve_module_contract_path(module_dir, &tray.provider, "provider")?;
    }

    if let Some(notifications) = &manifest.notifications {
        if notifications.protocol != MODULE_NOTIFICATIONS_PROTOCOL_VERSION {
            return Err(format!(
                "module '{}' declares unsupported notifications protocol {}; expected {}",
                manifest.name, notifications.protocol, MODULE_NOTIFICATIONS_PROTOCOL_VERSION
            ));
        }
    }

    validate_module_language_contract(module_dir)?;

    /*
     * Lifecycle is part of the installed module contract.
     *
     * Validate the referenced declarative contract before an
     * installation or update may commit.
     */
    let _ = lifecycle_contract_from_module(module_dir, manifest)?;

    /*
     * Settings are part of the installed module contract.
     *
     * Validation happens before installation/update commit,
     * exactly like the other module-owned resources.
     */
    let _ = module_settings_default_from(module_dir, manifest)?;

    Ok(())
}

fn read_manifest(path: &Path) -> Result<ModuleManifest, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("could not read module manifest {}: {error}", path.display()))?;

    let manifest: ModuleManifest = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid module manifest {}: {error}", path.display()))?;

    let module_dir = path.parent().ok_or_else(|| {
        format!(
            "module manifest has no parent directory: {}",
            path.display()
        )
    })?;

    validate_module_manifest(module_dir, &manifest)?;

    Ok(manifest)
}

pub fn module_settings_default_from(
    module_dir: &Path,
    manifest: &ModuleManifest,
) -> Result<Value, String> {
    let Some(settings_path) = manifest.settings.as_deref() else {
        return Ok(json!({}));
    };

    let settings_path = resolve_module_file(module_dir, settings_path, "settings")?;

    let raw = fs::read_to_string(&settings_path).map_err(|error| {
        format!(
            "could not read module '{}' settings default {}: {error}",
            manifest.name,
            settings_path.display()
        )
    })?;

    let value: Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid module '{}' settings default {}: {error}",
            manifest.name,
            settings_path.display()
        )
    })?;

    settings::validate_module_default(&value).map_err(|error| {
        format!(
            "module '{}' settings default is invalid: {}",
            manifest.name, error
        )
    })?;

    Ok(value)
}

pub fn installed_module_settings_default(name: &str) -> Result<Value, String> {
    let module_dir = find_module_dir(name)?;
    let manifest = read_manifest(&module_dir.join("manifest.json"))?;

    module_settings_default_from(&module_dir, &manifest)
}

fn resolve_entrypoint(module_dir: &Path, entrypoint: &str) -> Result<PathBuf, String> {
    resolve_module_file(module_dir, entrypoint, "entrypoint")
}

fn valid_module_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

#[allow(dead_code)]
fn _language_contract_example() -> Result<String, String> {
    Ok(languages::load_manifest()?.default)
}

#[cfg(test)]
mod require_candidate_contract_tests {
    use super::*;

    fn temporary_module_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "neebles-require-candidate-test-{}-{}-{}",
            std::process::id(),
            transaction_id(),
            label
        ));

        fs::create_dir_all(&path).unwrap();

        path
    }

    fn manifest(name: &str, lifecycle: Option<&str>) -> ModuleManifest {
        ModuleManifest {
            schema: MODULE_SCHEMA_VERSION,

            name: name.to_string(),

            version: "1.0.0".to_string(),

            entrypoint: "entrypoint".to_string(),

            lifecycle: lifecycle.map(str::to_string),

            contracts: Vec::new(),

            commands: BTreeMap::new(),

            tray: None,

            notifications: None,

            settings: None,
        }
    }

    #[test]
    fn lifecycle_contract_from_module_returns_empty_contract_when_absent() {
        let module_dir = temporary_module_dir("empty");

        let manifest = manifest("alpha", None);

        let contract = lifecycle_contract_from_module(&module_dir, &manifest).unwrap();

        assert!(contract.require.is_empty());

        assert!(contract.transitions.is_empty());

        fs::remove_dir_all(module_dir).unwrap();
    }

    #[test]
    fn lifecycle_contract_from_module_loads_require_from_candidate_directory() {
        let module_dir = temporary_module_dir("require");

        fs::write(
            module_dir.join("lifecycle.json"),
            r#"{
                "require": {
                    "beta": {
                        "future.channel": "stable"
                    }
                }
            }"#,
        )
        .unwrap();

        let manifest = manifest("alpha", Some("lifecycle.json"));

        let contract = lifecycle_contract_from_module(&module_dir, &manifest).unwrap();

        assert_eq!(
            contract.require.get("beta").unwrap().get("future.channel"),
            Some(&"stable".to_string())
        );

        fs::remove_dir_all(module_dir).unwrap();
    }

    #[test]
    fn candidate_inventory_represents_baseline_by_membership() {
        let inventory = RequireCandidateInventory {
            baseline: BTreeSet::from(["alpha".to_string()]),

            staged: BTreeMap::new(),
        };

        assert!(inventory.baseline().contains("alpha"));

        assert!(inventory.staged().is_empty());
    }

    #[test]
    fn candidate_inventory_staged_ids_are_derived_from_map_membership() {
        let staging_path = temporary_module_dir("staged");

        fs::remove_dir_all(&staging_path).unwrap();

        let staging = ModuleInstallStaging::prepare(staging_path.clone()).unwrap();

        fs::create_dir_all(staging.path()).unwrap();

        let candidate = RequireCandidate {
            module_id: "beta".to_string(),

            folder: "beta".to_string(),

            staging,

            manifest: manifest("beta", None),

            lifecycle: crate::lifecycle::LifecycleContract::default(),
        };

        let inventory = RequireCandidateInventory {
            baseline: BTreeSet::new(),

            staged: BTreeMap::from([("beta".to_string(), candidate)]),
        };

        assert_eq!(inventory.staged_ids(), BTreeSet::from(["beta".to_string()]));

        drop(inventory);

        assert!(!staging_path.exists());
    }

    #[test]
    fn candidate_keeps_module_identity_without_installation_flags() {
        let staging_path = temporary_module_dir("identity");

        fs::remove_dir_all(&staging_path).unwrap();

        let staging = ModuleInstallStaging::prepare(staging_path.clone()).unwrap();

        fs::create_dir_all(staging.path()).unwrap();

        let candidate = RequireCandidate {
            module_id: "gamma".to_string(),

            folder: "gamma-folder".to_string(),

            staging,

            manifest: manifest("gamma", None),

            lifecycle: crate::lifecycle::LifecycleContract::default(),
        };

        assert_eq!(candidate.module_id(), "gamma");

        assert_eq!(candidate.folder(), "gamma-folder");

        assert_eq!(candidate.manifest().name, "gamma");

        assert!(candidate.staging_path().exists());

        drop(candidate);

        assert!(!staging_path.exists());
    }
}

#[cfg(test)]
mod lifecycle_contract_reference_tests {
    use super::*;
    use std::fs;

    fn temporary_module_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "neebles-lifecycle-contract-test-{}-{}-{}",
            std::process::id(),
            transaction_id(),
            label
        ));

        fs::create_dir_all(&path).unwrap();

        path
    }

    fn manifest_with_lifecycle(path: &str) -> ModuleManifest {
        ModuleManifest {
            schema: MODULE_SCHEMA_VERSION,
            name: "synthetic-lifecycle-module".to_string(),
            version: "1.0.0".to_string(),
            entrypoint: "entrypoint".to_string(),
            lifecycle: Some(path.to_string()),
            contracts: Vec::new(),
            commands: BTreeMap::new(),
            tray: None,
            notifications: None,
            settings: None,
        }
    }

    #[test]
    fn lifecycle_reference_loads_valid_module_local_file() {
        let module_dir = temporary_module_dir("valid");

        let lifecycle_path = module_dir.join("lifecycle.json");

        fs::write(
            &lifecycle_path,
            r#"{
                "hardcoded": {
                    "alpha": "one"
                },
                "transitions": {
                    "start": {
                        "operations": {
                            "engage": {
                                "artillery": "synthetic.capability",
                                "objective": "synthetic.start",
                                "munition": {},
                                "tactics": {},
                                "intelligence": {}
                            }
                        }
                    },
                    "stop": {
                        "operations": {}
                    }
                }
            }"#,
        )
        .unwrap();

        let resolved = resolve_module_file(&module_dir, "lifecycle.json", "lifecycle").unwrap();

        let contract = crate::lifecycle::load(&resolved).unwrap();

        assert_eq!(contract.hardcoded.get("alpha"), Some(&"one".to_string()));

        assert!(contract
            .transitions
            .get("start")
            .unwrap()
            .operations
            .contains_key("engage"));

        fs::remove_dir_all(module_dir).unwrap();
    }

    #[test]
    fn lifecycle_reference_rejects_escape_from_module_directory() {
        let module_dir = temporary_module_dir("escape");

        let error =
            resolve_module_file(&module_dir, "../outside-lifecycle.json", "lifecycle").unwrap_err();

        assert!(
            error.contains("invalid path")
                || error.contains("must be relative")
                || error.contains("escapes")
        );

        fs::remove_dir_all(module_dir).unwrap();
    }

    #[test]
    fn lifecycle_reference_rejects_missing_file() {
        let module_dir = temporary_module_dir("missing");

        let error =
            resolve_module_file(&module_dir, "missing-lifecycle.json", "lifecycle").unwrap_err();

        assert!(error.contains("could not resolve") || error.contains("not a file"));

        fs::remove_dir_all(module_dir).unwrap();
    }

    #[test]
    fn manifest_carries_lifecycle_reference_as_string() {
        let manifest = manifest_with_lifecycle("lifecycle.json");

        assert_eq!(manifest.lifecycle.as_deref(), Some("lifecycle.json"));
    }
}

#[cfg(test)]
mod launcher_contract_tests {
    use super::*;

    fn dynamic_contract(contract_type: &str, endpoints: serde_json::Value) -> ContractDefinition {
        serde_json::from_value(json!({
            "schema": 1,
            "contract": contract_type,
            "endpoints": endpoints
        }))
        .expect("test dynamic contract must parse")
    }

    #[test]
    fn launcher_contract_resolves_dynamic_commands_endpoint() {
        let legacy = BTreeMap::<String, CommandContract>::new();

        let mut contracts = ModuleContracts::default();
        contracts.contracts.insert(
            "commands".to_string(),
            dynamic_contract(
                "commands",
                json!({
                    "open": {
                        "endpoint": "ui.open",
                        "launcher": true
                    }
                }),
            ),
        );

        assert_eq!(
            launcher_action_from_sources("test-module", &legacy, &contracts)
                .expect("dynamic launcher action must resolve"),
            Some("open".to_string())
        );
    }

    #[test]
    fn launcher_contract_deduplicates_same_legacy_and_dynamic_action() {
        let mut legacy = BTreeMap::<String, CommandContract>::new();

        legacy.insert(
            "open".to_string(),
            CommandContract {
                launcher: true,
                ..Default::default()
            },
        );

        let mut contracts = ModuleContracts::default();

        contracts.contracts.insert(
            "commands".to_string(),
            dynamic_contract(
                "commands",
                json!({
                    "open": {
                        "endpoint": "ui.open",
                        "launcher": true
                    }
                }),
            ),
        );

        assert_eq!(
            launcher_action_from_sources("test-module", &legacy, &contracts)
                .expect("same logical action must be migration-compatible"),
            Some("open".to_string())
        );
    }

    #[test]
    fn launcher_contract_rejects_two_distinct_launcher_actions() {
        let mut legacy = BTreeMap::<String, CommandContract>::new();

        legacy.insert(
            "open".to_string(),
            CommandContract {
                launcher: true,
                ..Default::default()
            },
        );

        let mut contracts = ModuleContracts::default();

        contracts.contracts.insert(
            "commands".to_string(),
            dynamic_contract(
                "commands",
                json!({
                    "settings": {
                        "endpoint": "ui.settings",
                        "launcher": true
                    }
                }),
            ),
        );

        let error = launcher_action_from_sources("test-module", &legacy, &contracts)
            .expect_err("distinct launcher actions must be rejected");

        assert!(error.contains("more than one launcher action"));
        assert!(error.contains("open"));
        assert!(error.contains("settings"));
    }

    #[test]
    fn launcher_contract_rejects_launcher_flag_outside_commands_contract() {
        let legacy = BTreeMap::<String, CommandContract>::new();

        let mut contracts = ModuleContracts::default();

        contracts.contracts.insert(
            "service".to_string(),
            dynamic_contract(
                "service",
                json!({
                    "start": {
                        "endpoint": "service.start",
                        "launcher": true
                    }
                }),
            ),
        );

        let error = launcher_action_from_sources("test-module", &legacy, &contracts)
            .expect_err("non-command launcher flag must be rejected");

        assert!(error.contains("'commands' contract"));
        assert!(error.contains("service.start"));
    }

    #[test]
    fn launcher_contract_allows_module_without_launcher_action() {
        let legacy = BTreeMap::<String, CommandContract>::new();
        let contracts = ModuleContracts::default();

        assert_eq!(
            launcher_action_from_sources("test-module", &legacy, &contracts)
                .expect("launcher capability is optional"),
            None
        );
    }
}

#[cfg(test)]
mod module_transaction_certification_tests {
    use super::{transaction_id, ModuleInstallStaging};

    use std::fs;

    fn test_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "neebles-module-transaction-test-{}-{}",
            label,
            transaction_id()
        ))
    }

    #[test]
    fn certification_staging_refuses_existing_recovery_data() {
        let path = test_path("existing");

        fs::create_dir_all(&path).expect("could not create existing staging fixture");

        let marker = path.join("recovery-data");

        fs::write(&marker, b"preserve").expect("could not create recovery marker");

        let result = ModuleInstallStaging::prepare(path.clone());

        assert!(result.is_err(), "existing staging path must be rejected");

        assert_eq!(
            fs::read(&marker).expect("existing recovery marker was destroyed"),
            b"preserve"
        );

        fs::remove_dir_all(path).expect("could not clean existing staging fixture");
    }

    #[test]
    fn certification_uncommitted_staging_is_cleaned_on_drop() {
        let path = test_path("drop");

        {
            let staging =
                ModuleInstallStaging::prepare(path.clone()).expect("could not prepare staging");

            fs::create_dir_all(staging.path()).expect("could not materialize staging");

            fs::write(staging.path().join("candidate"), b"candidate")
                .expect("could not create staged candidate");

            assert!(path.exists(), "staging fixture must exist before drop");
        }

        assert!(!path.exists(), "uncommitted staging survived Drop cleanup");
    }

    #[test]
    fn certification_commit_publishes_candidate_and_preserves_it() {
        let staging_path = test_path("commit-source");

        let destination = test_path("commit-destination");

        let staging =
            ModuleInstallStaging::prepare(staging_path.clone()).expect("could not prepare staging");

        fs::create_dir_all(staging.path()).expect("could not materialize staging");

        fs::write(staging.path().join("candidate"), b"known-good")
            .expect("could not create candidate");

        staging
            .commit(&destination)
            .expect("candidate commit failed");

        assert!(
            !staging_path.exists(),
            "staging path survived successful commit"
        );

        assert_eq!(
            fs::read(destination.join("candidate")).expect("committed candidate is missing"),
            b"known-good"
        );

        fs::remove_dir_all(destination).expect("could not clean committed fixture");
    }

    #[test]
    fn certification_failed_commit_cleans_unpublished_candidate() {
        let staging_path = test_path("failed-source");

        let missing_parent = test_path("missing-parent");

        let destination = missing_parent.join("destination");

        let staging =
            ModuleInstallStaging::prepare(staging_path.clone()).expect("could not prepare staging");

        fs::create_dir_all(staging.path()).expect("could not materialize staging");

        fs::write(staging.path().join("candidate"), b"candidate")
            .expect("could not create candidate");

        let result = staging.commit(&destination);

        assert!(
            result.is_err(),
            "commit unexpectedly succeeded without destination parent"
        );

        assert!(
            !staging_path.exists(),
            "failed unpublished candidate survived cleanup"
        );

        assert!(
            !destination.exists(),
            "failed commit published a destination"
        );

        if missing_parent.exists() {
            fs::remove_dir_all(missing_parent).expect("could not clean failed commit fixture");
        }
    }
}

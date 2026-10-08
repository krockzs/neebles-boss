use crate::config;
use crate::contracts::{
    load_module_contracts, ContractDefinition, ContractEndpoint, ContractReference, ModuleContracts,
};
use crate::languages;
use crate::settings;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::env;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MODULE_SCHEMA_VERSION: u32 = 4;
pub const MODULE_LANGUAGE_SCHEMA_VERSION: u32 = 1;
pub const MODULE_NOTIFICATIONS_PROTOCOL_VERSION: u32 = 4;
pub const REGISTRY_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleSettingWrite {
    pub value: String,
    pub changed: bool,
}

static MODULE_SETTINGS_WRITE_LOCKS: OnceLock<Mutex<BTreeMap<String, Arc<Mutex<()>>>>> =
    OnceLock::new();

fn module_settings_write_lock(module: &str) -> Result<Arc<Mutex<()>>, String> {
    let registry = MODULE_SETTINGS_WRITE_LOCKS.get_or_init(|| Mutex::new(BTreeMap::new()));

    let mut guard = registry
        .lock()
        .map_err(|_| "module settings lock registry poisoned".to_string())?;

    Ok(guard
        .entry(module.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone())
}

pub fn module_setting_get(module: &str, setting_path: &str) -> Result<String, String> {
    let lock = module_settings_write_lock(module)?;

    let _guard = lock
        .lock()
        .map_err(|_| format!("module settings lock poisoned for '{}'", module))?;

    let default = installed_module_settings_default(module)?;

    let path = settings::module_settings_path(&neebles_root(), module);

    let local = settings::load_or_create(&path, &default)?;

    settings::get_effective_path(&local, &default, setting_path)
}

pub fn module_setting_set(
    module: &str,
    setting_path: &str,
    requested_value: String,
) -> Result<ModuleSettingWrite, String> {
    let lock = module_settings_write_lock(module)?;

    let _guard = lock
        .lock()
        .map_err(|_| format!("module settings lock poisoned for '{}'", module))?;

    let default = installed_module_settings_default(module)?;

    let path = settings::module_settings_path(&neebles_root(), module);

    let local = settings::load_or_create(&path, &default)?;

    let previous = settings::get_effective_path(&local, &default, setting_path)?;

    let value = settings::set_path(&path, &default, setting_path, requested_value)?;

    let changed = previous != value;

    if changed {
        let topic = format!("settings.{}", module);

        let payload = serde_json::json!({
            "target": module,
            "path": setting_path,
            "value": value
        });

        if let Err(error) = crate::module_ipc::runtime_registry().broadcast_event(
            &topic,
            "changed",
            payload.clone(),
        ) {
            eprintln!(
                "N.E.E.B.L.E.S.: module settings persisted but Module IPC broadcast failed for '{}': {}",
                module,
                error
            );
        }

        crate::ipc::broadcast_event(topic, "changed", payload);
    }

    Ok(ModuleSettingWrite { value, changed })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayContract {
    #[serde(default = "default_tray_protocol")]
    pub protocol: u32,

    pub icon: String,

    pub provider: String,

    pub construction_step: String,
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

#[cfg(test)]
mod notification_protocol_version_tests {
    use super::*;

    #[test]
    fn notifications_capability_protocol_is_v4() {
        assert_eq!(MODULE_NOTIFICATIONS_PROTOCOL_VERSION, 4);
    }

    #[test]
    fn notification_contract_default_uses_current_protocol() {
        let contract: NotificationContract =
            serde_json::from_str("{}").expect("empty notification contract must deserialize");

        assert_eq!(contract.protocol, MODULE_NOTIFICATIONS_PROTOCOL_VERSION);
    }

    #[test]
    fn notification_contract_preserves_explicit_legacy_version() {
        let contract: NotificationContract = serde_json::from_str(r#"{"protocol":2}"#)
            .expect("legacy notification contract must deserialize");

        assert_eq!(contract.protocol, 2);
        assert_ne!(contract.protocol, MODULE_NOTIFICATIONS_PROTOCOL_VERSION);
    }
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
     * Optional module-owned Boss surface declaration.
     *
     * This file declares presentation projections only.
     * Functional state remains owned by Lifecycle.
     */
    #[serde(default)]
    pub surfaces: Option<String>,

    /*
     * Dynamic module contracts.
     *
     * A module may extend the N.E.E.B.L.E.S. ecosystem by
     * declaring contract files without requiring Boss to be
     * recompiled for each module, command or contract type.
     *
     * Schema 4 uses this generic list as the module contract extension index.
     */
    #[serde(default)]
    pub contracts: Vec<ContractReference>,

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

    #[serde(default)]
    pub file: String,
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
    pub construction_step: String,
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

pub fn installed_module_dir(name: &str) -> Result<PathBuf, String> {
    if !valid_module_id(name) {
        return Err(format!("invalid module id: {name}"));
    }

    find_module_dir(name)
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
 * Resolve the module-owned Boss surface declaration.
 *
 * Absence means that the module contributes no projected content.
 *
 * The declaration is validated against the same module-owned
 * Lifecycle contract that owns functional object/transition identity.
 */
fn surface_projection_from_module(
    module_dir: &Path,
    manifest: &ModuleManifest,
    lifecycle: &crate::lifecycle::LifecycleContract,
) -> Result<crate::surface_projection::SurfaceProjection, String> {
    let Some(reference) = manifest.surfaces.as_deref() else {
        return Ok(crate::surface_projection::SurfaceProjection::new());
    };

    let path = resolve_module_file(module_dir, reference, "surfaces")?;

    crate::surface_contract::load(&manifest.name, &path, lifecycle)
}

pub fn installed_module_surface_projection(
    name: &str,
) -> Result<crate::surface_projection::SurfaceProjection, String> {
    if !valid_module_id(name) {
        return Err(format!("invalid module id: {name}"));
    }

    let module_dir = find_module_dir(name)?;

    let manifest = read_manifest(&module_dir.join("manifest.json"))?;

    let lifecycle = lifecycle_contract_from_module(&module_dir, &manifest)?;

    /*
     * Surface item visibility remains module contract truth.
     *
     * User preference is whole-module visibility and is
     * materialized independently for each Boss surface.
     */
    surface_projection_from_module(
        &module_dir,
        &manifest,
        &lifecycle,
    )
}

/*
 * Canonical Boss Surface Content Resolver entry point.
 *
 * Consumers ask:
 *
 *   module + arbitrary surface name
 *
 * and receive generic Boss-owned content.
 *
 * No consumer needs to know how the module declared Lifecycle,
 * what technology implements the module, or whether that surface
 * happens to be UI, Launcher, Tray or something introduced later.
 */
pub fn installed_module_all_surface_content(
    name: &str,
) -> Result<Vec<crate::surface_content::SurfaceContentItem>, String> {
    let projection = installed_module_surface_projection(name)?;

    Ok(crate::surface_content::resolve_all(&projection))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SurfaceActionTarget {
    action: String,
    object_id: Option<String>,
    transition_id: Option<String>,
}

fn declared_surface_action_target(
    item: &crate::surface_content::SurfaceContentItem,
    requested_action: &str,
) -> Result<SurfaceActionTarget, String> {
    let requested_action = requested_action.trim();

    if requested_action.is_empty() {
        return Err("surface action cannot be empty".to_string());
    }

    let data = item.data();
    let mut matches = Vec::<Option<String>>::new();

    if data
        .get("action")
        .map(String::as_str)
        .map(str::trim)
        == Some(requested_action)
    {
        matches.push(item.transition().map(str::to_string));
    }

    if data
        .get("action_on")
        .map(String::as_str)
        .map(str::trim)
        == Some(requested_action)
    {
        let transition = data
            .get("transition_on")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                format!(
                    "surface item {} declares action_on {} without transition_on",
                    item.identity().item_id(),
                    requested_action
                )
            })?;

        matches.push(Some(transition.to_string()));
    }

    if data
        .get("action_off")
        .map(String::as_str)
        .map(str::trim)
        == Some(requested_action)
    {
        let transition = data
            .get("transition_off")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                format!(
                    "surface item {} declares action_off {} without transition_off",
                    item.identity().item_id(),
                    requested_action
                )
            })?;

        matches.push(Some(transition.to_string()));
    }

    match matches.len() {
        0 => {
            return Err(format!(
                "surface item {} does not declare action {}",
                item.identity().item_id(),
                requested_action
            ));
        }

        1 => {}

        _ => {
            return Err(format!(
                "surface item {} declares ambiguous action {}",
                item.identity().item_id(),
                requested_action
            ));
        }
    }

    Ok(SurfaceActionTarget {
        action: requested_action.to_string(),
        object_id: item.object_id().map(str::to_string),
        transition_id: matches.pop().unwrap(),
    })
}

fn execute_resolved_surface_action_with<Requirements, Execute>(
    owner: &str,
    item: &crate::surface_content::SurfaceContentItem,
    requested_action: &str,
    requirements_satisfied: Requirements,
    execute: Execute,
) -> Result<(), String>
where
    Requirements: FnOnce(
        &str,
        &crate::surface_projection::SurfaceRequirements,
    ) -> bool,
    Execute: FnOnce(&str, &SurfaceActionTarget) -> Result<(), String>,
{
    if item.identity().owner_module() != owner {
        return Err(format!(
            "surface item {} belongs to {}, not {}",
            item.identity().item_id(),
            item.identity().owner_module(),
            owner
        ));
    }

    let target =
        declared_surface_action_target(item, requested_action)?;

    if !requirements_satisfied(owner, item.requirements()) {
        return Err(format!(
            "surface requirements are not satisfied for {}:{}",
            owner,
            item.identity().item_id()
        ));
    }

    execute(owner, &target)
}

pub fn execute_surface_action(
    owner: &str,
    item_id: &str,
    requested_action: &str,
) -> Result<(), String> {
    let owner = owner.trim();
    let item_id = item_id.trim();

    if !valid_module_id(owner) {
        return Err(format!("invalid module id: {owner}"));
    }

    if item_id.is_empty() {
        return Err("surface item id cannot be empty".to_string());
    }

    /*
     * Always re-resolve from the currently installed Surface
     * declaration. Boss Settings is only a materialized projection
     * and caller-supplied object/transition identity is never trusted.
     */
    let content = installed_module_all_surface_content(owner)?;

    let item = content
        .iter()
        .find(|item| {
            item.identity().owner_module() == owner
                && item.identity().item_id() == item_id
        })
        .ok_or_else(|| {
            format!(
                "installed module {} does not declare surface item {}",
                owner, item_id
            )
        })?;

    execute_resolved_surface_action_with(
        owner,
        item,
        requested_action,
        crate::surface_requirement_resolver::requirements_satisfied,
        |module, target| {
            execute_governor_target(
                module,
                &target.action,
                target.object_id.as_deref(),
                target.transition_id.as_deref(),
            )
        },
    )
}

#[cfg(test)]
mod surface_action_execution_tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::BTreeMap;

    fn content_item(
        object_id: Option<&str>,
        data: BTreeMap<String, String>,
        requirements: crate::surface_projection::SurfaceRequirements,
    ) -> crate::surface_content::SurfaceContentItem {
        let mut lifecycle = crate::lifecycle::LifecycleContract::default();

        if let Some(object_id) = object_id {
            let mut object = crate::lifecycle::ObjectContract::default();

            object.transitions.insert(
                "feature-on".to_string(),
                crate::lifecycle::Battleplan::default(),
            );

            object.transitions.insert(
                "feature-off".to_string(),
                crate::lifecycle::Battleplan::default(),
            );

            lifecycle.objects.insert(object_id.to_string(), object);
        }

        let mut projection =
            crate::surface_projection::SurfaceProjection::new();

        projection
            .register_for_lifecycle(
                &lifecycle,
                crate::surface_projection::SurfaceProjectionItem
                    ::with_data_and_requirements(
                        "config.feature",
                        "module.alpha",
                        "ui",
                        object_id.map(str::to_string),
                        None,
                        true,
                        data,
                        requirements,
                    )
                    .unwrap(),
            )
            .unwrap();

        crate::surface_content::resolve_all(&projection)
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn button_action_is_derived_from_fresh_surface_item() {
        let item = content_item(
            None,
            BTreeMap::from([(
                "action".to_string(),
                "notify-demo".to_string(),
            )]),
            crate::surface_projection::SurfaceRequirements::default(),
        );

        let target =
            declared_surface_action_target(&item, "notify-demo")
                .unwrap();

        assert_eq!(target.action, "notify-demo");
        assert_eq!(target.object_id, None);
        assert_eq!(target.transition_id, None);
    }

    #[test]
    fn switch_action_derives_declared_object_and_transition() {
        let item = content_item(
            Some("notify-switch"),
            BTreeMap::from([
                ("action_on".to_string(), "feature-on".to_string()),
                ("action_off".to_string(), "feature-off".to_string()),
                ("transition_on".to_string(), "feature-on".to_string()),
                ("transition_off".to_string(), "feature-off".to_string()),
            ]),
            crate::surface_projection::SurfaceRequirements::default(),
        );

        let target =
            declared_surface_action_target(&item, "feature-on")
                .unwrap();

        assert_eq!(
            target.object_id.as_deref(),
            Some("notify-switch")
        );

        assert_eq!(
            target.transition_id.as_deref(),
            Some("feature-on")
        );
    }

    #[test]
    fn caller_cannot_invent_surface_action() {
        let item = content_item(
            None,
            BTreeMap::from([(
                "action".to_string(),
                "notify-demo".to_string(),
            )]),
            crate::surface_projection::SurfaceRequirements::default(),
        );

        let error =
            declared_surface_action_target(&item, "invented")
                .unwrap_err();

        assert!(error.contains("does not declare action"));
    }

    #[test]
    fn ambiguous_surface_action_fails_closed() {
        let item = content_item(
            Some("notify-switch"),
            BTreeMap::from([
                ("action".to_string(), "same".to_string()),
                ("action_on".to_string(), "same".to_string()),
                ("transition_on".to_string(), "feature-on".to_string()),
            ]),
            crate::surface_projection::SurfaceRequirements::default(),
        );

        let error =
            declared_surface_action_target(&item, "same")
                .unwrap_err();

        assert!(error.contains("ambiguous action"));
    }

    #[test]
    fn unmet_requirements_block_lifecycle_execution() {
        let requirements = serde_json::from_value(
            serde_json::json!({"self": "open"}),
        )
        .unwrap();

        let item = content_item(
            None,
            BTreeMap::from([(
                "action".to_string(),
                "notify-demo".to_string(),
            )]),
            requirements,
        );

        let executed = Cell::new(false);

        let result = execute_resolved_surface_action_with(
            "module.alpha",
            &item,
            "notify-demo",
            |_, _| false,
            |_, _| {
                executed.set(true);
                Ok(())
            },
        );

        assert!(result.is_err());
        assert!(!executed.get());
    }

    #[test]
    fn satisfied_requirements_execute_only_derived_target() {
        let item = content_item(
            Some("notify-switch"),
            BTreeMap::from([
                ("action_on".to_string(), "feature-on".to_string()),
                ("transition_on".to_string(), "feature-on".to_string()),
            ]),
            crate::surface_projection::SurfaceRequirements::default(),
        );

        let executed = Cell::new(false);

        execute_resolved_surface_action_with(
            "module.alpha",
            &item,
            "feature-on",
            |_, _| true,
            |owner, target| {
                assert_eq!(owner, "module.alpha");
                assert_eq!(target.action, "feature-on");
                assert_eq!(
                    target.object_id.as_deref(),
                    Some("notify-switch")
                );
                assert_eq!(
                    target.transition_id.as_deref(),
                    Some("feature-on")
                );

                executed.set(true);
                Ok(())
            },
        )
        .unwrap();

        assert!(executed.get());
    }
}

fn installed_module_object_state_store(
    name: &str,
    lifecycle: &crate::lifecycle::LifecycleContract,
) -> Result<crate::lifecycle_objects::ObjectStateStore, String> {
    let canonical = config::reconcile_module_object_states(name, lifecycle)?;

    let mut states = crate::lifecycle_objects::ObjectStateStore::new();

    for (object_id, active) in canonical {
        states.register(lifecycle, object_id, active)?;
    }

    Ok(states)
}

fn surface_presentation_label(
    item: &crate::surface_content::SurfaceContentItem,
    module_strings: &BTreeMap<String, String>,
) -> Result<Option<String>, String> {
    if let Some(label_key) = item
        .data()
        .get("label_key")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let label = module_strings
            .get(label_key)
            .ok_or_else(|| {
                format!(
                    "module {} Surface item {} declares missing translation key {}",
                    item.identity().owner_module(),
                    item.identity().item_id(),
                    label_key
                )
            })?;

        if label.trim().is_empty() {
            return Err(format!(
                "module {} Surface item {} resolves translation key {} to an empty label",
                item.identity().owner_module(),
                item.identity().item_id(),
                label_key
            ));
        }

        return Ok(Some(label.clone()));
    }

    Ok(
        item.data()
            .get("label")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    )
}

fn surface_content_item_json(
    item: &crate::surface_content::SurfaceContentItem,
    states: &crate::lifecycle_objects::ObjectStateStore,
    module_strings: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let active = match item.object_id() {
        Some(object_id) if states.contains(object_id) =>
            item.canonical_active(states)?,

        _ => None,
    };

    let label =
        surface_presentation_label(item, module_strings)?;

    Ok(json!({
        "owner_module":
            item.identity().owner_module(),

        "item_id":
            item.identity().item_id(),

        "surface":
            item.surface(),

        "object_id":
            item.object_id(),

        "transition":
            item.transition(),

        "require":
            item.requirements(),

        "visible":
            item.visible(),

        "active":
            active,

        "label":
            label,

        "data":
            item.data(),
    }))
}
#[cfg(test)]
mod surface_content_json_contract_tests {
    use super::*;

    #[test]
    fn productive_surface_json_preserves_typed_requirements() {
        let requirements:
            crate::surface_projection::SurfaceRequirements =
            serde_json::from_value(
                serde_json::json!({
                    "self": "open",
                    "modules": {
                        "module.beta": "active"
                    }
                })
            )
            .unwrap();

        let lifecycle =
            crate::lifecycle::LifecycleContract::default();

        let mut projection =
            crate::surface_projection::SurfaceProjection::new();

        projection
            .register_for_lifecycle(
                &lifecycle,
                crate::surface_projection::SurfaceProjectionItem
                    ::with_data_and_requirements(
                        "notify.ui",
                        "module.alpha",
                        "ui",
                        None,
                        None,
                        true,
                        std::collections::BTreeMap::from([(
                            "control".to_string(),
                            "button".to_string(),
                        )]),
                        requirements,
                    )
                    .unwrap(),
            )
            .unwrap();

        let content =
            crate::surface_content::resolve_all(&projection);

        let states =
            crate::lifecycle_objects::ObjectStateStore::new();

        let value =
            surface_content_item_json(&content[0], &states, &std::collections::BTreeMap::new())
                .unwrap();

        assert_eq!(
            value["require"]["self"],
            serde_json::json!("open")
        );

        assert_eq!(
            value["require"]["modules"]["module.beta"],
            serde_json::json!("active")
        );
    }

    #[test]
    fn productive_surface_json_preserves_empty_requirements() {
        let lifecycle =
            crate::lifecycle::LifecycleContract::default();

        let mut projection =
            crate::surface_projection::SurfaceProjection::new();

        projection
            .register_for_lifecycle(
                &lifecycle,
                crate::surface_projection::SurfaceProjectionItem::with_data(
                    "open.launcher",
                    "module.alpha",
                    "launcher",
                    None,
                    None,
                    true,
                    std::collections::BTreeMap::from([(
                        "control".to_string(),
                        "button".to_string(),
                    )]),
                )
                .unwrap(),
            )
            .unwrap();

        let content =
            crate::surface_content::resolve_all(&projection);

        let states =
            crate::lifecycle_objects::ObjectStateStore::new();

        let value =
            surface_content_item_json(&content[0], &states, &std::collections::BTreeMap::new())
                .unwrap();

        assert_eq!(
            value["require"],
            serde_json::json!({})
        );
    }
    #[test]
    fn productive_surface_json_materializes_owner_scoped_label() {
        let lifecycle =
            crate::lifecycle::LifecycleContract::default();

        let mut projection =
            crate::surface_projection::SurfaceProjection::new();

        projection
            .register_for_lifecycle(
                &lifecycle,
                crate::surface_projection::SurfaceProjectionItem::with_data(
                    "open.launcher",
                    "module.alpha",
                    "launcher",
                    None,
                    None,
                    true,
                    std::collections::BTreeMap::from([
                        (
                            "control".to_string(),
                            "button".to_string(),
                        ),
                        (
                            "label_key".to_string(),
                            "surface.open".to_string(),
                        ),
                    ]),
                )
                .unwrap(),
            )
            .unwrap();

        let content =
            crate::surface_content::resolve_all(&projection);

        let states =
            crate::lifecycle_objects::ObjectStateStore::new();

        let strings =
            std::collections::BTreeMap::from([(
                "surface.open".to_string(),
                "Abrir".to_string(),
            )]);

        let value =
            surface_content_item_json(
                &content[0],
                &states,
                &strings,
            )
            .unwrap();

        assert_eq!(
            value["label"],
            serde_json::json!("Abrir")
        );

        assert_eq!(
            value["data"]["label_key"],
            serde_json::json!("surface.open")
        );
    }

    #[test]
    fn productive_surface_json_rejects_missing_owner_translation() {
        let lifecycle =
            crate::lifecycle::LifecycleContract::default();

        let mut projection =
            crate::surface_projection::SurfaceProjection::new();

        projection
            .register_for_lifecycle(
                &lifecycle,
                crate::surface_projection::SurfaceProjectionItem::with_data(
                    "open.launcher",
                    "module.alpha",
                    "launcher",
                    None,
                    None,
                    true,
                    std::collections::BTreeMap::from([
                        (
                            "label_key".to_string(),
                            "surface.missing".to_string(),
                        ),
                    ]),
                )
                .unwrap(),
            )
            .unwrap();

        let content =
            crate::surface_content::resolve_all(&projection);

        let states =
            crate::lifecycle_objects::ObjectStateStore::new();

        let error =
            surface_content_item_json(
                &content[0],
                &states,
                &std::collections::BTreeMap::new(),
            )
            .unwrap_err();

        assert!(error.contains("missing translation key"));
    }
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

    let construction_step = tray.construction_step.trim();

    if construction_step.is_empty() {
        return Err(format!(
            "module '{}' tray construction_step cannot be empty",
            manifest.name
        ));
    }

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
        construction_step: construction_step.to_string(),
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
    fn folder(&self) -> &str {
        &self.folder
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
 * owns: module tree, settings contract and lifecycle installation
 * event.
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

    if let Err(error) = settings::load_or_create(&settings_path, &settings_default) {
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
    };

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

pub(crate) fn broadcast_surface_module_change(name: &str, reason: &str) {
    crate::ipc::broadcast_event(
        "module.lifecycle",
        "state_changed",
        serde_json::json!({
            "module": name,
            "reason": reason
        }),
    );
}

fn module_governor_lifecycle_runtime(
) -> Result<crate::lifecycle_governor_runtime::GovernorLifecycleRuntime, String> {
    let available = crate::lifecycle_available_capabilities::productive_catalog()?;

    Ok(crate::lifecycle_governor_runtime::GovernorLifecycleRuntime::from_available(available))
}

pub type ModuleLifecycleObserverFactory = std::sync::Arc<
    dyn Fn(&str, &str) -> crate::lifecycle_observer::LifecycleObserver + Send + Sync + 'static,
>;

fn execute_module_governor_lifecycle(
    runtime: &crate::lifecycle_governor_runtime::GovernorLifecycleRuntime,
    module_id: &str,
    action: &str,
    contract: &crate::lifecycle::LifecycleContract,
    observer_factory: Option<&ModuleLifecycleObserverFactory>,
) -> Result<(), String> {
    let observer = observer_factory
        .map(|factory| factory(module_id, action))
        .unwrap_or_else(crate::lifecycle_observer::LifecycleObserver::none);

    runtime
        .execute_action_blocking_observed(
            module_id,
            &format!("module.{action}"),
            contract,
            action,
            std::collections::BTreeMap::new(),
            observer,
        )
        .map(|_| ())
}

pub fn execute_governor_target(
    name: &str,
    action: &str,
    object_id: Option<&str>,
    transition_id: Option<&str>,
) -> Result<(), String> {
    let result =
        execute_governor_target_with_observer(name, action, object_id, transition_id, None);

    if result.is_ok() {
        broadcast_surface_module_change(name, action);
    }

    result
}

pub fn execute_governor_target_observed(
    name: &str,
    action: &str,
    object_id: Option<&str>,
    transition_id: Option<&str>,
    observer_factory: &ModuleLifecycleObserverFactory,
) -> Result<(), String> {
    let result = execute_governor_target_with_observer(
        name,
        action,
        object_id,
        transition_id,
        Some(observer_factory),
    );

    if result.is_ok() {
        broadcast_surface_module_change(name, action);
    }

    result
}

fn execute_governor_target_with_observer(
    name: &str,
    action: &str,
    object_id: Option<&str>,
    transition_id: Option<&str>,
    observer_factory: Option<&ModuleLifecycleObserverFactory>,
) -> Result<(), String> {
    let action = action.trim();

    if action.is_empty() {
        return Err("module Governor action cannot be empty".to_string());
    }

    let object_id = object_id.map(str::trim).filter(|value| !value.is_empty());

    let transition_id = transition_id
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let lifecycle = installed_module_lifecycle_contract(name)?;

    config::reconcile_module_object_states(name, &lifecycle)?;

    let lifecycle_runtime = module_governor_lifecycle_runtime()?;

    let observer = observer_factory
        .map(|factory| factory(name, action))
        .unwrap_or_else(crate::lifecycle_observer::LifecycleObserver::none);

    let canonical_open =
        action == "open";

    let (
        opening_generation,
        lifecycle_execution_id,
    ) = if canonical_open {
        if !crate::surface_requirement_resolver::module_active(name) {
            return Err(format!(
                "module {} is not active and cannot open",
                name
            ));
        }

        let (generation, execution_id) =
            crate::module_ipc::runtime_registry()
                .begin_opening(name)?;

        broadcast_surface_module_change(
            name,
            "runtime_opening",
        );

        (Some(generation), execution_id)
    } else {
        (None, format!("module.{action}"))
    };

    let execution =
        match lifecycle_runtime
            .execute_target_blocking_observed(
                name,
                &lifecycle_execution_id,
                &lifecycle,
                action,
                object_id,
                transition_id,
                std::collections::BTreeMap::new(),
                observer,
            )
        {
            Ok(execution) => execution,

            Err(error) => {
                if let Some(generation) =
                    opening_generation
                {
                    match crate::module_ipc::runtime_registry()
                        .seal_opening(
                            name,
                            generation,
                        )
                    {
                        Ok(false) => {
                            broadcast_surface_module_change(
                                name,
                                "runtime_open_failed",
                            );
                        }

                        Ok(true) => {}

                        Err(seal_error) => {
                            return Err(format!(
                                "{}; opening seal failed: {}",
                                error,
                                seal_error
                            ));
                        }
                    }
                }

                return Err(error);
            }
        };

    if let Some(generation) =
        opening_generation
    {
        if execution.is_none() {
            let _ = crate::module_ipc::runtime_registry()
                .seal_opening(
                    name,
                    generation,
                )?;

            broadcast_surface_module_change(
                name,
                "runtime_open_failed",
            );

            return Err(format!(
                "module {} does not declare canonical Governor open binding",
                name
            ));
        }

        let opening_alive =
            crate::module_ipc::runtime_registry()
                .seal_opening(
                    name,
                    generation,
                )?;

        if !opening_alive
            && crate::module_ipc::runtime_registry()
                .state(name)?
                != crate::module_ipc::registry::ModuleRuntimeState::Open
        {
            broadcast_surface_module_change(
                name,
                "runtime_open_failed",
            );

            return Err(format!(
                "module {} open transition completed without a live modules.runtime owner",
                name
            ));
        }
    }

    if let (Some(object_id), Some(transition_id)) = (object_id, transition_id) {
        if let Some(active) = lifecycle
            .objects
            .get(object_id)
            .and_then(|object| object.transition_active.get(transition_id))
            .copied()
        {
            config::update_module_object_state(name, object_id, active)?;
        }
    }

    Ok(())
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
    observer_factory: Option<&ModuleLifecycleObserverFactory>,
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

        let prepared = match crate::module_preinstall::ensure_module_packages(&module_id) {
            Ok(prepared) => prepared,

            Err(error) => {
                return Err(require_install_failure(
                    format!("module '{}' preinstall failed: {}", module_id, error),
                    &mut transaction,
                ));
            }
        };

        eprintln!(
            "N.E.E.B.L.E.S.: module '{}' preinstall GREEN: required={} reused={} downloaded={} custom_revision={}",
            module_id,
            prepared.report.required,
            prepared.report.reused,
            prepared.report.downloaded,
            prepared.report.custom_revision
        );

        let prepared_binding =
            match neebles_backend::module_material_binding::prepare_material_binding(
                &prepared.material_root,
                prepared.binding_input.clone(),
            ) {
                Ok(binding) => binding,

                Err(error) => {
                    return Err(require_install_failure(
                        format!(
                            "module {} material binding preparation failed: {}",
                            module_id, error
                        ),
                        &mut transaction,
                    ));
                }
            };

        let active_binding_path = match prepared_binding.activate_install() {
            Ok(path) => path,

            Err(error) => {
                return Err(require_install_failure(
                    format!(
                        "module {} material binding activation failed: {}",
                        module_id, error
                    ),
                    &mut transaction,
                ));
            }
        };

        eprintln!(
            "N.E.E.B.L.E.S.: module {} material binding ACTIVE: {}",
            module_id,
            active_binding_path.display()
        );

        let feature_reconciliation =
            installed_module_all_surface_content(&module_id)
                .and_then(|content| {
                    config::reconcile_module_features(&module_id, &content)
                        .map(|_| ())
                });

        if let Err(error) = feature_reconciliation {
            return Err(require_install_failure(
                format!(
                    "module {} feature materialization failed: {}",
                    module_id, error
                ),
                &mut transaction,
            ));
        }

        if let Err(error) = execute_module_governor_lifecycle(
            lifecycle_runtime,
            &module_id,
            "install",
            &lifecycle,
            observer_factory,
        ) {
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
            &std::collections::BTreeMap::new(),
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

        let mut command = match crate::network_boundary::command("boss.curl", &arguments) {
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

fn module_has_active_resources(name: &str) -> Result<bool, String> {
    let runtime_active = crate::module_ipc::runtime_registry().get(name)?.is_some();

    if runtime_active {
        return Ok(true);
    }

    /*
     * Tray provider runtime is infrastructure owned by
     * Tray Manager and intentionally remains independent
     * from the normal Module IPC runtime.
     */
    Ok(probe_tray_provider_identity(name)?.is_some())
}

fn request_runtime_shutdown(name: &str, reason: &str) -> Result<(), String> {
    let Some(record) = crate::module_ipc::runtime_registry().get(name)? else {
        return Ok(());
    };

    let identity = ProcessIdentity {
        pid: record.pid,
        start_time_ticks: record.start_time_ticks,
    };

    /*
     * If the authenticated process incarnation is already gone,
     * there is nothing physical left to stop.
     */
    if !process_identity_is_alive(identity)? {
        return Ok(());
    }

    if let Err(error) = record
        .writer
        .send(crate::module_ipc::protocol::ModuleMessage::Shutdown {
            module: name.to_string(),
            session_id: record.session_id.clone(),
            reason: Some(reason.to_string()),
        })
    {
        eprintln!(
            "N.E.E.B.L.E.S.: cooperative shutdown delivery failed for module '{}' session '{}': {}; authenticated process shutdown will continue",
            name,
            record.session_id,
            error
        );
    }

    /*
     * Give the exact registered runtime a cooperative window.
     *
     * We intentionally track BOTH:
     * - the exact session incarnation;
     * - the exact kernel process incarnation.
     *
     * A later session or reused PID can never inherit ownership.
     */
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);

    while std::time::Instant::now() < deadline {
        let same_session = matches!(
            crate::module_ipc::runtime_registry()
                .get(name)?,
            Some(ref current)
                if current.session_id
                    == record.session_id
        );

        let process_alive = process_identity_is_alive(identity)?;

        if !same_session && !process_alive {
            return Ok(());
        }

        if !process_alive {
            return Ok(());
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    /*
     * Cooperative shutdown failed.
     * Terminate ONLY the exact kernel-authenticated process
     * incarnation captured during registration.
     */
    if !process_identity_is_alive(identity)? {
        return Ok(());
    }

    let signal_result = unsafe { libc::kill(identity.pid as libc::pid_t, libc::SIGTERM) };

    if signal_result != 0 {
        return Err(format!(
            "could not send SIGTERM to authenticated module '{}' runtime process {}: {}",
            name,
            identity.pid,
            std::io::Error::last_os_error()
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

    while process_identity_is_alive(identity)? && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if !process_identity_is_alive(identity)? {
        return Ok(());
    }

    let signal_result = unsafe { libc::kill(identity.pid as libc::pid_t, libc::SIGKILL) };

    if signal_result != 0 {
        return Err(format!(
            "could not send SIGKILL to authenticated module '{}' runtime process {}: {}",
            name,
            identity.pid,
            std::io::Error::last_os_error()
        ));
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);

    while process_identity_is_alive(identity)? && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if process_identity_is_alive(identity)? {
        return Err(format!(
            "authenticated module '{}' runtime process {} did not terminate after SIGKILL",
            name, identity.pid
        ));
    }

    Ok(())
}

pub fn deactivate_runtime_resources_local(name: &str, reason: &str) -> Result<(), String> {
    /*
     * Persistent Boss runtime authority owns Module IPC RuntimeRegistry
     * and governed Tray RuntimeLease/process ownership.
     */
    request_runtime_shutdown(name, reason)?;

    stop_tray_provider(name)?;

    if crate::module_ipc::runtime_registry().get(name)?.is_some() {
        return Err(format!(
            "module '{}' runtime is still registered after deactivation",
            name
        ));
    }

    if probe_tray_provider_identity(name)?.is_some() {
        return Err(format!(
            "module '{}' tray provider is still active after deactivation",
            name
        ));
    }

    Ok(())
}

fn request_persistent_boss_action(action: &str, args: Vec<String>) -> Result<Value, String> {
    let response = crate::ipc::request(&crate::request::ExecutionRequest {
        target: "boss".to_string(),
        action: Some(action.to_string()),
        args,
        context: crate::request::ExecutionContext {
            caller: "module-lifecycle".to_string(),
        },
    })?;

    if response.ok {
        return Ok(response.result.unwrap_or(Value::Null));
    }

    let detail = response
        .error
        .map(|error| error.message)
        .unwrap_or_else(|| {
            format!(
                "Boss returned code {} without an error payload",
                response.code
            )
        });

    Err(format!(
        "persistent Boss action '{}' failed: {}",
        action, detail
    ))
}

fn request_persistent_tray_start(name: &str) -> Result<(), String> {
    request_persistent_boss_action("tray-provider-start", vec![name.to_string()]).map(|_| ())
}

fn request_persistent_tray_stop(name: &str) -> Result<(), String> {
    request_persistent_boss_action("tray-provider-stop", vec![name.to_string()]).map(|_| ())
}

fn deactivate_module(name: &str, reason: &str) -> Result<(), String> {
    request_persistent_boss_action(
        "module-runtime-deactivate",
        vec![name.to_string(), reason.to_string()],
    )
    .map(|_| ())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

pub fn module_running(name: &str) -> bool {
    match crate::module_ipc::runtime_registry().get(name) {
        Ok(Some(_)) => true,
        Ok(None) => false,

        /*
         * Unknown registry state is reported as running
         * to avoid presenting an unsafe false "stopped"
         * state in non-destructive views.
         */
        Err(_) => true,
    }
}

/*
 * Prove physical ownership of a Module IPC peer.
 *
 * The kernel PID obtained through SO_PEERCRED is checked directly
 * against the installed module entrypoint.
 *
 * Module IPC registration is the runtime ownership authority.
 * Physical identity is retained by RuntimeRegistry after this
 * verification succeeds.
 */
pub fn verify_module_runtime_process(name: &str, pid: u32) -> Result<(u32, u64), String> {
    if pid == 0 {
        return Err(format!(
            "module '{}' IPC peer reported invalid process id 0",
            name
        ));
    }

    /*
     * Capture the exact process incarnation before ownership
     * verification. PID alone is never sufficient because Linux
     * may reuse it after a process exits.
     */
    let Some(identity) = capture_process_identity(pid)? else {
        return Err(format!(
            "module '{}' IPC peer process {} does not exist",
            name, pid
        ));
    };

    let module_dir = find_module_dir(name).map_err(|error| {
        format!(
            "module '{}' IPC peer process {} exists, but its installation cannot be resolved: {}",
            name, pid, error
        )
    })?;

    let manifest = read_manifest(&module_dir.join("manifest.json")).map_err(|error| {
        format!(
            "module '{}' IPC peer process {} exists, but its manifest cannot be verified: {}",
            name, pid, error
        )
    })?;

    if manifest.name != name {
        return Err(format!(
            "module '{}' IPC peer process {} resolved installation for '{}'",
            name, pid, manifest.name
        ));
    }

    let expected = resolve_entrypoint(&module_dir, &manifest.entrypoint).map_err(|error| {
        format!(
            "module '{}' IPC peer process {} exists, but its entrypoint cannot be verified: {}",
            name, pid, error
        )
    })?;

    let proc_exe = PathBuf::from(format!("/proc/{pid}/exe"));

    let mut ownership_verified = false;

    if let Ok(actual_exe) = fs::read_link(&proc_exe) {
        if let Ok(actual_exe) = actual_exe.canonicalize() {
            if actual_exe == expected {
                ownership_verified = true;
            }
        }
    }

    if !ownership_verified {
        let cmdline = fs::read(format!("/proc/{pid}/cmdline")).map_err(|error| {
            format!(
                "could not inspect IPC peer process {} for module '{}': {error}",
                pid, name
            )
        })?;

        let expected_bytes = expected.as_os_str().as_bytes();

        ownership_verified = cmdline
            .split(|byte| *byte == 0)
            .filter(|argument| !argument.is_empty())
            .any(|argument| argument == expected_bytes);
    }

    if !ownership_verified {
        return Err(format!(
            "process {} is not the installed entrypoint declared by module '{}'",
            pid, name
        ));
    }

    /*
     * Close the PID-reuse race:
     * the process verified above must still be the same
     * incarnation captured before verification.
     */
    if !process_identity_is_alive(identity)? {
        return Err(format!(
            "module '{}' IPC peer process {} changed identity during registration verification",
            name, pid
        ));
    }

    Ok((identity.pid, identity.start_time_ticks))
}

fn tray_runtime_marker_path(name: &str) -> PathBuf {
    runtime_modules_root().join(format!("{name}.tray.pid"))
}

fn clear_tray_runtime_marker_if_identity(name: &str, identity: ProcessIdentity) {
    let path = tray_runtime_marker_path(name);

    let matches = fs::read_to_string(&path)
        .ok()
        .and_then(|value| serde_json::from_str::<ProcessIdentity>(&value).ok())
        == Some(identity);

    if matches {
        let _ = fs::remove_file(path);
    }
}

fn write_tray_runtime_marker(name: &str, identity: ProcessIdentity) -> Result<(), String> {
    let root = runtime_modules_root();

    fs::create_dir_all(&root).map_err(|error| {
        format!(
            "could not create module runtime directory {}: {error}",
            root.display()
        )
    })?;

    let path = tray_runtime_marker_path(name);

    let payload = serde_json::to_string(&identity).map_err(|error| {
        format!(
            "could not serialize tray runtime ownership for module '{}': {error}",
            name
        )
    })?;

    fs::write(&path, format!("{payload}\n")).map_err(|error| {
        format!(
            "could not write tray provider runtime marker {}: {error}",
            path.display()
        )
    })?;

    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        format!(
            "could not seal tray provider runtime marker {}: {error}",
            path.display()
        )
    })
}

fn tray_process_group_exists(process_group: u32) -> Result<bool, String> {
    if process_group <= 1 || process_group > libc::pid_t::MAX as u32 {
        return Err(format!(
            "invalid tray process group identity: {}",
            process_group
        ));
    }

    let result = unsafe { libc::kill(-(process_group as libc::pid_t), 0) };

    if result == 0 {
        return Ok(true);
    }

    let error = std::io::Error::last_os_error();

    match error.raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err(format!(
            "could not inspect tray process group {}: {}",
            process_group, error
        )),
    }
}

fn tray_runtime_group_alive(identity: ProcessIdentity) -> Result<bool, String> {
    let group_exists = tray_process_group_exists(identity.pid)?;

    match process_start_time_ticks(identity.pid)? {
        Some(start_time_ticks) if start_time_ticks == identity.start_time_ticks => Ok(group_exists),

        Some(start_time_ticks) => {
            if group_exists {
                return Err(format!(
                    "tray runtime process group {} remains live but leader identity changed: expected_start={} actual_start={}",
                    identity.pid,
                    identity.start_time_ticks,
                    start_time_ticks
                ));
            }

            Ok(false)
        }

        None => {
            if group_exists {
                return Err(format!(
                    "tray runtime process group {} remains live after its authenticated leader exited; ownership is unknown",
                    identity.pid
                ));
            }

            Ok(false)
        }
    }
}

fn signal_tray_runtime_group(
    identity: ProcessIdentity,
    signal: libc::c_int,
) -> Result<bool, String> {
    if !tray_runtime_group_alive(identity)? {
        return Ok(false);
    }

    let result = unsafe { libc::kill(-(identity.pid as libc::pid_t), signal) };

    if result == 0 {
        return Ok(true);
    }

    let error = std::io::Error::last_os_error();

    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(false);
    }

    Err(format!(
        "could not signal tray runtime process group {} with signal {}: {}",
        identity.pid, signal, error
    ))
}

fn probe_tray_provider_identity(name: &str) -> Result<Option<ProcessIdentity>, String> {
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

    let identity = match serde_json::from_str::<ProcessIdentity>(&raw) {
        Ok(identity) => identity,

        Err(json_error) => {
            if let Ok(pid) = raw.trim().parse::<u32>() {
                if process_start_time_ticks(pid)?.is_none() {
                    let _ = fs::remove_file(&path);
                    return Ok(None);
                }

                return Err(format!(
                        "module '{}' has a live legacy tray PID marker {} for process {}; cleanup is required before governed tray ownership can be established",
                        name,
                        path.display(),
                        pid
                    ));
            }

            return Err(format!(
                "module '{}' has an invalid tray runtime marker {}: {}",
                name,
                path.display(),
                json_error
            ));
        }
    };

    if identity.pid <= 1 || identity.start_time_ticks == 0 {
        return Err(format!(
            "module '{}' has invalid tray runtime ownership identity in {}",
            name,
            path.display()
        ));
    }

    if !process_identity_is_alive(identity)? {
        if tray_runtime_group_alive(identity)? {
            return Ok(Some(identity));
        }

        clear_tray_runtime_marker_if_identity(name, identity);

        return Ok(None);
    }

    let contract = resolved_tray_contract(name)?;

    let expected = PathBuf::from(&contract.provider)
        .canonicalize()
        .map_err(|error| {
            format!(
                "module '{}' tray runtime process {} exists, but its provider path cannot be canonicalized: {error}",
                name,
                identity.pid
            )
        })?;

    let proc_exe = PathBuf::from(format!("/proc/{}/exe", identity.pid));

    if let Ok(actual_exe) = fs::read_link(&proc_exe) {
        if let Ok(actual_exe) = actual_exe.canonicalize() {
            if actual_exe == expected {
                if !process_identity_is_alive(identity)? {
                    return Err(format!(
                        "module '{}' tray runtime process {} changed identity during ownership verification",
                        name,
                        identity.pid
                    ));
                }

                return Ok(Some(identity));
            }
        }
    }

    let cmdline =
        fs::read(format!("/proc/{}/cmdline", identity.pid))
            .map_err(|error| {
                format!(
                    "module '{}' tray runtime process {} exists, but its command line cannot be inspected: {error}",
                    name,
                    identity.pid
                )
            })?;

    let expected_bytes = expected.as_os_str().as_bytes();

    let matches_provider = cmdline
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .any(|argument| argument == expected_bytes);

    if !matches_provider {
        return Err(format!(
            "module '{}' tray runtime marker points to live process {}, but Boss cannot prove that its governed command owns provider {}",
            name,
            identity.pid,
            expected.display()
        ));
    }

    if !process_identity_is_alive(identity)? {
        return Err(format!(
            "module '{}' tray runtime process {} changed identity during ownership verification",
            name, identity.pid
        ));
    }

    Ok(Some(identity))
}

pub fn tray_provider_running(name: &str) -> bool {
    match probe_tray_provider_identity(name) {
        Ok(Some(_)) => true,
        Ok(None) => false,
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

    if manifest.tray.is_none() {
        return Ok(false);
    }

    if tray_provider_running(name) {
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

    let declaration =
        neebles_backend::domestic_construction::load_module_domestic_construction_declaration(
            name,
        )?;

    let step = declaration.step(&contract.construction_step)?;

    if step.execution
        != neebles_backend::domestic_construction::DomesticConstructionExecution::Persistent
    {
        return Err(format!(
            "module '{}' tray construction step '{}' must use persistent execution",
            name, contract.construction_step
        ));
    }

    let provider_argument = provider.as_os_str().to_string_lossy();

    if !step
        .arguments
        .iter()
        .any(|argument| argument == provider_argument.as_ref())
    {
        return Err(format!(
            "module '{}' tray construction step '{}' does not execute declared provider {}",
            name,
            contract.construction_step,
            provider.display()
        ));
    }

    let socket_path = crate::tray::protocol::socket_path();

    if !socket_path.exists() {
        return Ok(false);
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

    let registry =
        neebles_backend::domestic_authority_supply_process::process_supplied_authority_registry()?;

    let prepared =
        neebles_backend::domestic_construction::build_construction_step_command_with_environment(
            &declaration,
            &contract.construction_step,
            registry,
            &domestic_environment,
        )?;

    let (mut command, runtime_lease) = prepared.into_parts();

    command.process_group(0);

    command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = command.spawn().map_err(|error| {
        format!(
            "could not launch governed tray provider for module '{}': {error}",
            name
        )
    })?;

    let pid = child.id();

    let identity = match capture_process_identity(pid)? {
        Some(identity) => identity,

        None => {
            let _ = child.wait();

            return Err(format!(
                    "governed tray runtime for module '{}' exited before Boss could capture process identity",
                    name
                ));
        }
    };

    if let Err(error) = write_tray_runtime_marker(name, identity) {
        let _ = signal_tray_runtime_group(identity, libc::SIGKILL);

        let _ = child.wait();

        return Err(error);
    }

    let owned_name = name.to_string();

    std::thread::spawn(move || {
        let runtime_lease_guard = runtime_lease;

        let _ = child.wait();

        loop {
            match tray_runtime_group_alive(identity) {
                Ok(false) => break,

                Ok(true) => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }

                Err(error) => {
                    eprintln!(
                        "N.E.E.B.L.E.S.: retaining tray RuntimeLease for module '{}' because process-group lifetime is unknown: {}",
                        owned_name,
                        error
                    );

                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }

        clear_tray_runtime_marker_if_identity(&owned_name, identity);

        drop(runtime_lease_guard);
    });

    Ok(true)
}

pub fn stop_tray_provider(name: &str) -> Result<bool, String> {
    let Some(identity) = probe_tray_provider_identity(name)? else {
        return Ok(false);
    };

    if !tray_runtime_group_alive(identity)? {
        clear_tray_runtime_marker_if_identity(name, identity);

        return Ok(false);
    }

    signal_tray_runtime_group(identity, libc::SIGTERM)?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

    while tray_runtime_group_alive(identity)? && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if !tray_runtime_group_alive(identity)? {
        clear_tray_runtime_marker_if_identity(name, identity);

        return Ok(true);
    }

    signal_tray_runtime_group(identity, libc::SIGKILL)?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);

    while tray_runtime_group_alive(identity)? && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if tray_runtime_group_alive(identity)? {
        return Err(format!(
            "tray runtime for module '{}' process group {} did not terminate after SIGKILL",
            name, identity.pid
        ));
    }

    clear_tray_runtime_marker_if_identity(name, identity);

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

    let output = crate::network_boundary::command("boss.git", &arguments)?
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

    let output = crate::network_boundary::command("boss.curl", &arguments)?
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

fn feature_startup_transaction_directory(name: &str) -> bool {
    name.starts_with(".neebles-update-")
        || name.starts_with(".neebles-backup-")
        || name.starts_with(".neebles-failed-")
}

fn installed_feature_inventory(
) -> Result<config::FeatureInventory, String> {
    let root = modules_root();
    let mut inventory = config::FeatureInventory::new();
    let mut owners = BTreeMap::<String, PathBuf>::new();

    if !root.exists() {
        return Ok(inventory);
    }

    for entry in fs::read_dir(&root)
        .map_err(|error| format!(
            "could not read {} while rebuilding Boss Features: {error}",
            root.display()
        ))?
    {
        let entry = entry.map_err(|error| format!(
            "could not read module directory entry while rebuilding Boss Features: {error}"
        ))?;

        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let directory_name = entry.file_name();
        let directory_name = directory_name.to_string_lossy();

        if feature_startup_transaction_directory(&directory_name) {
            continue;
        }

        let manifest_path = path.join("manifest.json");

        if !manifest_path.is_file() {
            continue;
        }

        let manifest = read_manifest(&manifest_path).map_err(|error| {
            format!(
                "invalid installed module contract {} during Feature startup reconciliation: {}",
                manifest_path.display(),
                error
            )
        })?;

        if let Some(previous) = owners.insert(
            manifest.name.clone(),
            path.clone(),
        ) {
            return Err(format!(
                "duplicate installed module identity {} while rebuilding Boss Features: {} and {}",
                manifest.name,
                previous.display(),
                path.display()
            ));
        }

        let lifecycle =
            lifecycle_contract_from_module(&path, &manifest)?;

        let projection =
            surface_projection_from_module(
                &path,
                &manifest,
                &lifecycle,
            )?;

        let content =
            crate::surface_content::resolve_all(&projection);

        let canonical =
            config::canonical_module_features(
                &manifest.name,
                &content,
            )?;

        if !canonical.is_empty() {
            inventory.insert(
                manifest.name.clone(),
                canonical,
            );
        }
    }

    Ok(inventory)
}

pub fn reconcile_installed_module_features(
) -> Result<config::FeatureInventory, String> {
    let canonical = installed_feature_inventory()?;

    config::replace_feature_inventory(&canonical)?;

    Ok(canonical)
}

#[cfg(test)]
mod startup_feature_inventory_tests {
    use super::*;

    #[test]
    fn update_transaction_directories_are_not_active_installations() {
        assert!(feature_startup_transaction_directory(
            ".neebles-update-test-module-1"
        ));

        assert!(feature_startup_transaction_directory(
            ".neebles-backup-test-module-1"
        ));

        assert!(feature_startup_transaction_directory(
            ".neebles-failed-features-update-test-module-1"
        ));

        assert!(!feature_startup_transaction_directory(
            "test-module"
        ));
    }
}
fn materialize_surface_requirements_with<Satisfied>(
    value: &mut Value,
    mut satisfied: Satisfied,
) -> Result<(), String>
where
    Satisfied: FnMut(
        &str,
        &crate::surface_projection::SurfaceRequirements,
    ) -> bool,
{
    let modules = value
        .as_array_mut()
        .ok_or_else(|| {
            "installed module presentation model must be an array"
                .to_string()
        })?;

    for module in modules.iter_mut() {
        let module_object = module
            .as_object_mut()
            .ok_or_else(|| {
                "installed module presentation entry must be an object"
                    .to_string()
            })?;

        let module_name = module_object
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "installed module presentation entry has no module name"
                    .to_string()
            })?
            .to_string();

        let content = module_object
            .get_mut("surface_content")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| {
                format!(
                    "installed module {} has no SurfaceContent array",
                    module_name
                )
            })?;

        for item in content.iter_mut() {
            let item_object = item
                .as_object_mut()
                .ok_or_else(|| {
                    format!(
                        "installed module {} contains a non-object SurfaceContent item",
                        module_name
                    )
                })?;

            let item_id = item_object
                .get("item_id")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>")
                .to_string();

            let owner = item_object
                .get("owner_module")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    format!(
                        "SurfaceContent item {} has no owner module",
                        item_id
                    )
                })?
                .to_string();

            if owner != module_name {
                return Err(format!(
                    "SurfaceContent owner mismatch: module={} item={} owner={}",
                    module_name,
                    item_id,
                    owner
                ));
            }

            let require_value = item_object
                .get("require")
                .cloned()
                .unwrap_or_else(|| json!({}));

            let requirements:
                crate::surface_projection::SurfaceRequirements =
                serde_json::from_value(require_value)
                    .map_err(|error| {
                        format!(
                            "invalid Surface requirements for {}:{}: {error}",
                            owner,
                            item_id
                        )
                    })?;

            let requirements_met =
                satisfied(&owner, &requirements);

            item_object.insert(
                "requirements_met".to_string(),
                Value::Bool(requirements_met),
            );
        }
    }

    Ok(())
}

fn materialize_runtime_presentation_with<State>(
    value: &mut Value,
    mut state: State,
) -> Result<(), String>
where
    State: FnMut(
        &str,
    ) -> Result<
        crate::module_ipc::registry::ModuleRuntimeState,
        String,
    >,
{
    let modules = value
        .as_array_mut()
        .ok_or_else(|| {
            "runtime presentation model must be an array"
                .to_string()
        })?;

    for module in modules.iter_mut() {
        let object = module
            .as_object_mut()
            .ok_or_else(|| {
                "runtime presentation entry must be an object"
                    .to_string()
            })?;

        let name = object
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "runtime presentation module has no name"
                    .to_string()
            })?
            .to_string();

        let runtime_state = state(&name)?;

        let enabled = object
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let open_available =
            enabled
            && runtime_state
                == crate::module_ipc::registry::ModuleRuntimeState::Closed;

        object.insert(
            "runtime_state".to_string(),
            Value::String(
                runtime_state.as_str().to_string()
            ),
        );

        object.insert(
            "open_available".to_string(),
            Value::Bool(open_available),
        );

        let content = object
            .get_mut("surface_content")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| {
                format!(
                    "module {} has no SurfaceContent array",
                    name
                )
            })?;

        for item in content.iter_mut() {
            let item_object = item
                .as_object_mut()
                .ok_or_else(|| {
                    format!(
                        "module {} has invalid SurfaceContent",
                        name
                    )
                })?;

            let requirements_met = item_object
                .get("requirements_met")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            let canonical_open_action = item_object
                .get("data")
                .and_then(Value::as_object)
                .and_then(|data| data.get("action"))
                .and_then(Value::as_str)
                .map(str::trim)
                == Some("open");

            let action_available =
                requirements_met
                && (!canonical_open_action
                    || open_available);

            item_object.insert(
                "action_available".to_string(),
                Value::Bool(action_available),
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod surface_presentation_model_tests {
    use super::*;

    #[test]
    fn presentation_model_materializes_requirements_without_replacing_contract() {
        let mut value = json!([
            {
                "name": "module.alpha",
                "surface_content": [
                    {
                        "owner_module": "module.alpha",
                        "item_id": "feature.active",
                        "require": {
                            "self": "active"
                        }
                    },
                    {
                        "owner_module": "module.alpha",
                        "item_id": "feature.cross",
                        "require": {
                            "modules": {
                                "module.beta": "open"
                            }
                        }
                    }
                ]
            }
        ]);

        let mut queried = Vec::<String>::new();

        materialize_surface_requirements_with(
            &mut value,
            |owner, requirements| {
                queried.push(owner.to_string());

                requirements.self_state()
                    == Some(
                        crate::surface_projection::SurfaceRequirementState::Active
                    )
            },
        )
        .unwrap();

        let content = value[0]["surface_content"]
            .as_array()
            .unwrap();

        assert_eq!(
            content[0]["requirements_met"],
            json!(true)
        );

        assert_eq!(
            content[1]["requirements_met"],
            json!(false)
        );

        assert_eq!(
            content[0]["require"]["self"],
            json!("active")
        );

        assert_eq!(
            content[1]["require"]["modules"]["module.beta"],
            json!("open")
        );

        assert_eq!(
            queried,
            vec![
                "module.alpha".to_string(),
                "module.alpha".to_string(),
            ]
        );
    }

    #[test]
    fn presentation_model_rejects_surface_owner_mismatch() {
        let mut value = json!([
            {
                "name": "module.alpha",
                "surface_content": [
                    {
                        "owner_module": "module.beta",
                        "item_id": "foreign.feature",
                        "require": {}
                    }
                ]
            }
        ]);

        let mut queried = false;

        let error =
            materialize_surface_requirements_with(
                &mut value,
                |_, _| {
                    queried = true;
                    true
                },
            )
            .unwrap_err();

        assert!(!queried);
        assert!(error.contains("owner mismatch"));
    }
}
pub fn installed_modules_json() -> Result<Value, String> {
    let mut result = Vec::new();

    let boss_config =
        config::load_or_initialize()?;

    let requested_language =
        boss_config.language.clone();

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

        let module_strings =
            load_module_strings(
                &entry.path(),
                &requested_language,
            )?;

        let lifecycle = installed_module_lifecycle_contract(&manifest.name)?;

        let object_states = installed_module_object_state_store(&manifest.name, &lifecycle)?;

        let enabled = config::module_enabled(&manifest.name)?;
        let icon = local_module_icon(&entry.path());
        let running = module_running(&manifest.name);

        /*
         * Generic Boss surface content.
         *
         * Do not filter to UI here.
         * Launcher, Tray and future Boss surfaces
         * consume the exact same canonical material.
         */
        let declared_surface_content =
            installed_module_all_surface_content(
                &manifest.name
            )?;

        let tray_default =
            declared_surface_content
                .iter()
                .any(|item| {
                    item.surface() == "tray"
                        && item.visible()
                });

        let launcher_default =
            declared_surface_content
                .iter()
                .any(|item| {
                    item.surface() == "launcher"
                        && item.visible()
                });

        let tray_visible =
            config::effective_surface_module_visibility(
                &boss_config,
                "tray",
                &manifest.name,
                tray_default,
            )?;

        let launcher_visible =
            config::effective_surface_module_visibility(
                &boss_config,
                "launcher",
                &manifest.name,
                launcher_default,
            )?;

        let surface_content =
            declared_surface_content
                .iter()
                .map(|item| {
                    surface_content_item_json(
                        item,
                        &object_states,
                        &module_strings,
                    )
                })
                .collect::<Result<Vec<_>, String>>()?;

        let mut item = json!({
            "name": manifest.name,
            "version": manifest.version,
            "enabled": enabled,
            "running": running,
            "path": entry.path(),
            "icon": icon,
            "surface_content": surface_content,
            "surface_visibility": {
                "tray": tray_visible,
                "launcher": launcher_visible
            },

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

pub fn installed_modules_presentation_json() -> Result<Value, String> {
    let mut value = installed_modules_json()?;

    materialize_surface_requirements_with(
        &mut value,
        |owner, requirements| {
            crate::surface_requirement_resolver::requirements_satisfied(
                owner,
                requirements,
            )
        },
    )?;

    materialize_runtime_presentation_with(
        &mut value,
        |module| {
            crate::module_ipc::runtime_registry()
                .state(module)
        },
    )?;

    Ok(value)
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

fn installed_dependency_graph(
) -> Result<crate::module_dependency_policy::InstalledDependencyGraph, String> {
    let mut names = installed_names()?.into_iter().collect::<Vec<_>>();

    names.sort();

    let mut contracts = BTreeMap::new();

    for module_id in names {
        let lifecycle = installed_module_lifecycle_contract(&module_id)?;

        contracts.insert(module_id, lifecycle);
    }

    crate::module_dependency_policy::InstalledDependencyGraph::from_contracts(&contracts)
}

pub fn module_dependency_preflight(action: &str, name: &str) -> Result<Value, String> {
    let _ = find_module_dir(name)?;

    let graph = installed_dependency_graph()?;

    match action {
        "uninstall" => {
            let blockers = graph.uninstall_blockers(name)?;

            Ok(serde_json::json!({
                "action": "uninstall",
                "module": name,
                "allowed": blockers.is_empty(),
                "affected": [name],
                "blockers": blockers
            }))
        }

        "disable" => {
            let affected = graph.disable_order(name)?;

            Ok(serde_json::json!({
                "action": "disable",
                "module": name,
                "allowed": true,
                "affected": affected,
                "blockers": []
            }))
        }

        "enable" => {
            let affected = graph.enable_order(name)?;

            Ok(serde_json::json!({
                "action": "enable",
                "module": name,
                "allowed": true,
                "affected": affected,
                "blockers": []
            }))
        }

        _ => Err(format!(
            "unsupported module dependency preflight action: {}",
            action
        )),
    }
}

pub fn install(name: &str) -> Result<(), String> {
    let result = install_with_observer(name, None);

    if result.is_ok() {
        broadcast_surface_module_change(name, "install");
    }

    result
}

pub fn install_observed(
    name: &str,
    observer_factory: &ModuleLifecycleObserverFactory,
) -> Result<(), String> {
    let result = install_with_observer(name, Some(observer_factory));

    if result.is_ok() {
        broadcast_surface_module_change(name, "install");
    }

    result
}

fn install_with_observer(
    name: &str,
    observer_factory: Option<&ModuleLifecycleObserverFactory>,
) -> Result<(), String> {
    let registry = fetch_registry()?;

    install_internal(name, &registry, observer_factory)
}

fn install_internal(
    name: &str,
    registry: &Registry,
    observer_factory: Option<&ModuleLifecycleObserverFactory>,
) -> Result<(), String> {
    let lifecycle_runtime = module_governor_lifecycle_runtime()?;

    install_require_tree(name, registry, &lifecycle_runtime, observer_factory)
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
    let result = uninstall_with_observer(name, remove_settings, None);

    if result.is_ok() {
        broadcast_surface_module_change(name, "uninstall");
    }

    result
}

pub fn uninstall_observed(
    name: &str,
    remove_settings: bool,
    observer_factory: &ModuleLifecycleObserverFactory,
) -> Result<(), String> {
    let result = uninstall_with_observer(name, remove_settings, Some(observer_factory));

    if result.is_ok() {
        broadcast_surface_module_change(name, "uninstall");
    }

    result
}

fn uninstall_with_observer(
    name: &str,
    remove_settings: bool,
    observer_factory: Option<&ModuleLifecycleObserverFactory>,
) -> Result<(), String> {
    /*
     * Dependency legality is checked before Lifecycle,
     * process shutdown, filesystem movement or config mutation.
     *
     * A required installed module cannot be removed while
     * any installed dependent still declares it in require.
     */
    let graph = installed_dependency_graph()?;

    graph.can_uninstall(name)?;

    let lifecycle_runtime = module_governor_lifecycle_runtime()?;

    uninstall_internal(
        name,
        remove_settings,
        Some(&lifecycle_runtime),
        observer_factory,
    )
}

fn uninstall_without_lifecycle(name: &str, remove_settings: bool) -> Result<(), String> {
    uninstall_internal(name, remove_settings, None, None)
}

fn uninstall_internal(
    name: &str,
    remove_settings: bool,
    lifecycle_runtime: Option<&crate::lifecycle_governor_runtime::GovernorLifecycleRuntime>,
    observer_factory: Option<&ModuleLifecycleObserverFactory>,
) -> Result<(), String> {
    let material_root =
        neebles_backend::module_material_territory::resolve_module_material_territory()?
            .material_root;

    /*
     * Uninstall is inherently destructive and therefore
     * owns the complete module lifecycle.
     *
     * A module cannot remain alive after its installation
     * has been removed.
     */
    if let Some(lifecycle_runtime) = lifecycle_runtime {
        let lifecycle = installed_module_lifecycle_contract(name)?;

        execute_module_governor_lifecycle(
            lifecycle_runtime,
            name,
            "uninstall",
            &lifecycle,
            observer_factory,
        )?;
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

    let mut staged_material_binding =
        match neebles_backend::module_material_binding::stage_material_binding_removal(
            &material_root,
            name,
        ) {
            Ok(removal) => removal,

            Err(binding_error) => {
                if let Some(staged) = staged_settings_path.as_ref() {
                    let _ = fs::rename(staged, &local_settings_path);
                }

                return match fs::rename(&removal_path, &path) {
                    Ok(()) => Err(format!(
                        "could not stage material binding removal for module {}: {}; module installation and local settings were restored",
                        name,
                        binding_error
                    )),

                    Err(rollback_error) => Err(format!(
                        "CRITICAL: could not stage material binding removal for module {}: {}; module rollback also failed: {}; active material binding state may require recovery",
                        name,
                        binding_error,
                        rollback_error
                    )),
                };
            }
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
                if let Some(binding) = staged_material_binding.take() {
                    binding.finalize();
                }

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
     * The module no longer owns an active installation path and
     * Boss-owned state cleanup succeeded.
     *
     * The material binding must therefore stop being active before
     * destruction of the detached module copy is attempted.
     */
    if let Some(binding) = staged_material_binding.take() {
        binding.finalize();
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
     * Module uninstall is finalized.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LiveUpdateRollbackState {
    PreviousRestored,
    NewPreserved,
    NoActiveModule,
}

#[derive(Debug)]
struct LiveUpdateRollbackReport {
    state: LiveUpdateRollbackState,
    detail: String,
}

fn rollback_live_update_paths(
    current_path: &Path,
    backup_path: &Path,
    failed_update_path: &Path,
) -> LiveUpdateRollbackReport {
    match fs::rename(current_path, failed_update_path) {
        Ok(()) => {}

        Err(move_error) => match fs::symlink_metadata(current_path) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                return LiveUpdateRollbackReport {
                        state: LiveUpdateRollbackState::NewPreserved,
                        detail: format!(
                            "new module could not be moved aside: {move_error}; active new version remains at {} and previous version remains at {}",
                            current_path.display(),
                            backup_path.display()
                        ),
                    };
            }

            Ok(_) => {
                return LiveUpdateRollbackReport {
                        state: LiveUpdateRollbackState::NoActiveModule,
                        detail: format!(
                            "new module could not be moved aside: {move_error}; active path {} exists but is not a module directory; previous version remains at {}",
                            current_path.display(),
                            backup_path.display()
                        ),
                    };
            }

            Err(inspect_error) if inspect_error.kind() == std::io::ErrorKind::NotFound => {
                return match fs::rename(backup_path, current_path) {
                        Ok(()) => LiveUpdateRollbackReport {
                            state: LiveUpdateRollbackState::PreviousRestored,
                            detail: format!(
                                "new active path had disappeared after rollback move failed: {move_error}; previous version was restored"
                            ),
                        },

                        Err(restore_error) => LiveUpdateRollbackReport {
                            state: LiveUpdateRollbackState::NoActiveModule,
                            detail: format!(
                                "new active path is absent after rollback move failed: {move_error}; previous version also could not be restored from {}: {restore_error}",
                                backup_path.display()
                            ),
                        },
                    };
            }

            Err(inspect_error) => {
                return LiveUpdateRollbackReport {
                        state: LiveUpdateRollbackState::NewPreserved,
                        detail: format!(
                            "new module could not be moved aside: {move_error}; active path state could not be inspected safely: {inspect_error}; previous version remains at {}",
                            backup_path.display()
                        ),
                    };
            }
        },
    }

    match fs::rename(backup_path, current_path) {
        Ok(()) => {
            let detail = match fs::remove_dir_all(failed_update_path) {
                Ok(()) => "failed new copy was removed".to_string(),

                Err(error) => format!(
                    "previous version was restored but failed new copy {} could not be removed: {error}",
                    failed_update_path.display()
                ),
            };

            LiveUpdateRollbackReport {
                state: LiveUpdateRollbackState::PreviousRestored,
                detail,
            }
        }

        Err(rollback_error) => match fs::rename(failed_update_path, current_path) {
            Ok(()) => LiveUpdateRollbackReport {
                state: LiveUpdateRollbackState::NewPreserved,
                detail: format!(
                    "previous version could not be restored from {}: {rollback_error}; new version was returned to the active path",
                    backup_path.display()
                ),
            },

            Err(recovery_error) => LiveUpdateRollbackReport {
                state: LiveUpdateRollbackState::NoActiveModule,
                detail: format!(
                    "previous version could not be restored from {}: {rollback_error}; new version also could not be returned from {}: {recovery_error}",
                    backup_path.display(),
                    failed_update_path.display()
                ),
            },
        },
    }
}

fn format_live_update_failure(
    name: &str,
    phase: &str,
    cause: &str,
    rollback: &LiveUpdateRollbackReport,
) -> String {
    match rollback.state {
        LiveUpdateRollbackState::PreviousRestored => format!(
            "module {} update {} failed: {}; previous module version was restored; {}",
            name,
            phase,
            cause,
            rollback.detail
        ),

        LiveUpdateRollbackState::NewPreserved => format!(
            "CRITICAL: module {} update {} failed: {}; rollback could not restore the previous version, but the new version remains active; {}",
            name,
            phase,
            cause,
            rollback.detail
        ),

        LiveUpdateRollbackState::NoActiveModule => format!(
            "CRITICAL: module {} update {} failed: {}; rollback could not leave either version active; {}",
            name,
            phase,
            cause,
            rollback.detail
        ),
    }
}

fn finalize_active_material_binding_absence(
    material_root: &Path,
    module_id: &str,
) -> Result<(), String> {
    if let Some(removal) = neebles_backend::module_material_binding::stage_material_binding_removal(
        material_root,
        module_id,
    )? {
        removal.finalize();
    }

    Ok(())
}

fn synchronize_existing_material_binding_after_update_rollback(
    material_root: &Path,
    module_id: &str,
    state: LiveUpdateRollbackState,
) -> Result<(), String> {
    match state {
        LiveUpdateRollbackState::PreviousRestored => Ok(()),

        LiveUpdateRollbackState::NewPreserved | LiveUpdateRollbackState::NoActiveModule => {
            finalize_active_material_binding_absence(material_root, module_id)
        }
    }
}

fn synchronize_activated_material_binding_after_update_rollback(
    binding: neebles_backend::module_material_binding::ActivatedMaterialBindingUpdate,
    material_root: &Path,
    module_id: &str,
    state: LiveUpdateRollbackState,
) -> Result<(), String> {
    match state {
        LiveUpdateRollbackState::PreviousRestored => binding.rollback(),

        LiveUpdateRollbackState::NewPreserved => {
            binding.finalize();
            Ok(())
        }

        LiveUpdateRollbackState::NoActiveModule => {
            binding.finalize();
            finalize_active_material_binding_absence(material_root, module_id)
        }
    }
}

fn synchronize_feature_inventory_after_update_rollback(
    name: &str,
    state: LiveUpdateRollbackState,
) -> Result<(), String> {
    match state {
        LiveUpdateRollbackState::PreviousRestored
        | LiveUpdateRollbackState::NewPreserved => {
            let content = installed_module_all_surface_content(name)?;

            config::reconcile_module_features(name, &content)
                .map(|_| ())
        }

        LiveUpdateRollbackState::NoActiveModule =>
            config::remove_module_features(name).map(|_| ()),
    }
}

fn format_live_update_failure_with_binding(
    name: &str,
    phase: &str,
    cause: &str,
    rollback: &LiveUpdateRollbackReport,
    binding_sync: Result<(), String>,
) -> String {
    let base = format_live_update_failure(name, phase, cause, rollback);

    let base = match binding_sync {
        Ok(()) => base,

        Err(error) => {
            format!("{base}; CRITICAL: material binding synchronization also failed: {error}")
        }
    };

    match synchronize_feature_inventory_after_update_rollback(
        name,
        rollback.state,
    ) {
        Ok(()) => base,

        Err(error) => format!(
            "{base}; CRITICAL: feature inventory synchronization also failed: {error}"
        ),
    }
}

pub fn update(name: &str, close_running: bool) -> Result<(), String> {
    let result = update_with_observer(name, close_running, None);

    if result.is_ok() {
        broadcast_surface_module_change(name, "update");
    }

    result
}

pub fn update_observed(
    name: &str,
    close_running: bool,
    observer_factory: &ModuleLifecycleObserverFactory,
) -> Result<(), String> {
    let result = update_with_observer(name, close_running, Some(observer_factory));

    if result.is_ok() {
        broadcast_surface_module_change(name, "update");
    }

    result
}

fn update_with_observer(
    name: &str,
    close_running: bool,
    observer_factory: Option<&ModuleLifecycleObserverFactory>,
) -> Result<(), String> {
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

    let material_root =
        neebles_backend::module_material_territory::resolve_module_material_territory()?
            .material_root;

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
     * read_manifest() is the schema-4 gate.
     *
     * Before touching the installed copy this validates:
     * - schema
     * - name format
     * - semantic version
     * - entrypoint
     * - dynamic contracts
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

        Err(publication_error) => {
            let failed_update_path = root.join(format!(
                ".neebles-failed-publication-update-{name}-{transaction}"
            ));

            let rollback =
                rollback_live_update_paths(&current_path, &backup_path, &failed_update_path);

            let binding_sync = synchronize_existing_material_binding_after_update_rollback(
                &material_root,
                name,
                rollback.state,
            );

            return Err(format_live_update_failure_with_binding(
                name,
                "candidate publication",
                &publication_error,
                &rollback,
                binding_sync,
            ));
        }
    }

    let prepared = match crate::module_preinstall::ensure_module_packages(name) {
        Ok(prepared) => prepared,

        Err(preinstall_error) => {
            let failed_update_path = root.join(format!(
                ".neebles-failed-preinstall-update-{name}-{transaction}"
            ));

            let rollback =
                rollback_live_update_paths(&current_path, &backup_path, &failed_update_path);

            let binding_sync = synchronize_existing_material_binding_after_update_rollback(
                &material_root,
                name,
                rollback.state,
            );

            return Err(format_live_update_failure_with_binding(
                name,
                "preinstall",
                &preinstall_error,
                &rollback,
                binding_sync,
            ));
        }
    };

    eprintln!(
        "N.E.E.B.L.E.S.: module '{}' update preinstall GREEN: required={} reused={} downloaded={} custom_revision={}",
        name,
        prepared.report.required,
        prepared.report.reused,
        prepared.report.downloaded,
        prepared.report.custom_revision
    );

    let prepared_binding = match neebles_backend::module_material_binding::prepare_material_binding(
        &prepared.material_root,
        prepared.binding_input.clone(),
    ) {
        Ok(binding) => binding,

        Err(binding_error) => {
            let failed_update_path = root.join(format!(
                ".neebles-failed-binding-prepare-update-{name}-{transaction}"
            ));

            let rollback =
                rollback_live_update_paths(&current_path, &backup_path, &failed_update_path);

            let binding_sync = synchronize_existing_material_binding_after_update_rollback(
                &material_root,
                name,
                rollback.state,
            );

            return Err(format_live_update_failure_with_binding(
                name,
                "material binding preparation",
                &binding_error,
                &rollback,
                binding_sync,
            ));
        }
    };

    let material_binding_update = match prepared_binding.activate_update() {
        Ok(binding) => binding,

        Err(binding_error) => {
            let failed_update_path = root.join(format!(
                ".neebles-failed-binding-activation-update-{name}-{transaction}"
            ));

            let rollback =
                rollback_live_update_paths(&current_path, &backup_path, &failed_update_path);

            let binding_sync = synchronize_existing_material_binding_after_update_rollback(
                &material_root,
                name,
                rollback.state,
            );

            return Err(format_live_update_failure_with_binding(
                name,
                "material binding activation",
                &binding_error,
                &rollback,
                binding_sync,
            ));
        }
    };

    eprintln!(
        "N.E.E.B.L.E.S.: module {} update material binding ACTIVE: {}",
        name,
        material_binding_update.active_path().display()
    );

    let lifecycle_runtime = match module_governor_lifecycle_runtime() {
        Ok(runtime) => runtime,

        Err(error) => {
            let failed_update_path = root.join(format!(
                ".neebles-failed-lifecycle-runtime-update-{name}-{transaction}"
            ));

            let rollback =
                rollback_live_update_paths(&current_path, &backup_path, &failed_update_path);

            let binding_sync = synchronize_activated_material_binding_after_update_rollback(
                material_binding_update,
                &material_root,
                name,
                rollback.state,
            );

            return Err(format_live_update_failure_with_binding(
                name,
                "Lifecycle runtime acquisition",
                &error,
                &rollback,
                binding_sync,
            ));
        }
    };

    if let Err(lifecycle_error) = execute_module_governor_lifecycle(
        &lifecycle_runtime,
        name,
        "update",
        &lifecycle,
        observer_factory,
    ) {
        let failed_update_path = root.join(format!(
            ".neebles-failed-lifecycle-update-{name}-{transaction}"
        ));

        let rollback = rollback_live_update_paths(&current_path, &backup_path, &failed_update_path);

        let binding_sync = synchronize_activated_material_binding_after_update_rollback(
            material_binding_update,
            &material_root,
            name,
            rollback.state,
        );

        return Err(format_live_update_failure_with_binding(
            name,
            "Lifecycle",
            &lifecycle_error,
            &rollback,
            binding_sync,
        ));
    }

    /*
     * The new copy is now live.
     *
     * Reconcile persistent settings against the new module
     * default before destroying the known-good backup.
     */
    let settings_path = settings::module_settings_path(&neebles_root(), name);

    if let Err(settings_error) = settings::update_from_default(&settings_path, &settings_default) {
        let failed_update_path = root.join(format!(
            ".neebles-failed-settings-update-{name}-{transaction}"
        ));

        let rollback = rollback_live_update_paths(&current_path, &backup_path, &failed_update_path);

        let binding_sync = synchronize_activated_material_binding_after_update_rollback(
            material_binding_update,
            &material_root,
            name,
            rollback.state,
        );

        return Err(format_live_update_failure_with_binding(
            name,
            "settings reconciliation",
            &settings_error,
            &rollback,
            binding_sync,
        ));
    }

    let feature_reconciliation =
        installed_module_all_surface_content(name)
            .and_then(|content| {
                config::reconcile_module_features(name, &content)
                    .map(|_| ())
            });

    if let Err(feature_error) = feature_reconciliation {
        let failed_update_path = root.join(format!(
            ".neebles-failed-features-update-{name}-{transaction}"
        ));

        let rollback =
            rollback_live_update_paths(
                &current_path,
                &backup_path,
                &failed_update_path,
            );

        let binding_sync =
            synchronize_activated_material_binding_after_update_rollback(
                material_binding_update,
                &material_root,
                name,
                rollback.state,
            );

        return Err(format_live_update_failure_with_binding(
            name,
            "feature reconciliation",
            &feature_error,
            &rollback,
            binding_sync,
        ));
    }

    /*
     * All fallible update phases that require rollback have completed.
     * Module, settings and Boss-owned Feature projection now agree.
     * The new material binding is now the persistent active truth.
     */
    material_binding_update.finalize();

    /*
     * Module and persistent settings are now finalized.
     * Remove the old known-good copy last.
     */
    if let Err(error) = fs::remove_dir_all(&backup_path) {
        eprintln!(
            "N.E.E.B.L.E.S.: module '{}' update was finalized successfully, but transactional backup {} could not be removed: {error}",
            name,
            backup_path.display()
        );
    }

    Ok(())
}
fn set_enabled_single(name: &str, enabled: bool) -> Result<(), String> {
    let _ = find_module_dir(name)?;

    let previous = config::module_enabled(name)?;

    let lifecycle = installed_module_lifecycle_contract(name)?;

    let lifecycle_runtime = module_governor_lifecycle_runtime()?;

    if !enabled {
        if previous {
            execute_module_governor_lifecycle(
                &lifecycle_runtime,
                name,
                "disable",
                &lifecycle,
                None,
            )?;
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

    if let Err(error) = request_persistent_tray_start(name) {
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
            execute_module_governor_lifecycle(&lifecycle_runtime, name, "enable", &lifecycle, None)
        {
            let tray_cleanup = request_persistent_tray_stop(name);

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

pub fn set_enabled(name: &str, enabled: bool) -> Result<(), String> {
    let _ = find_module_dir(name)?;

    let graph = installed_dependency_graph()?;

    let plan = if enabled {
        graph.enable_order(name)?
    } else {
        graph.disable_order(name)?
    };

    /*
     * ENABLE:
     *
     * requirements first, requested module last.
     *
     * Re-enabling a requirement does NOT reactivate
     * its dependents because enable_order only follows
     * requires downward from the requested module.
     *
     * DISABLE:
     *
     * dependents first, requested module last.
     *
     * This prevents a live dependent from being left
     * enabled after one of its requirements is disabled.
     */
    crate::module_dependency_policy::execute_state_plan_with_compensation(
        &plan,
        |module_id| {
            let previous = config::module_enabled(module_id)?;

            if previous == enabled {
                return Ok(false);
            }

            set_enabled_single(module_id, enabled)?;

            Ok(true)
        },
        |module_id| set_enabled_single(module_id, !enabled),
    )?;

    broadcast_surface_module_change(name, if enabled { "enabled" } else { "disabled" });

    Ok(())
}

fn read_module_language_manifest(
    module_dir: &Path,
) -> Result<ModuleLanguageManifest, String> {
    let manifest_path =
        module_dir.join("languages").join("manifest.json");

    let raw = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "could not read module language manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid module language manifest {}: {error}",
            manifest_path.display()
        )
    })
}

fn selected_module_language_entry(
    manifest: &ModuleLanguageManifest,
    requested: &str,
) -> Result<ModuleLanguageEntry, String> {
    let requested = languages::normalize_locale(requested);

    if let Some(language) = manifest
        .languages
        .iter()
        .find(|language| {
            languages::normalize_locale(&language.code)
                == requested
        })
    {
        return Ok(language.clone());
    }

    let normalized_default =
        languages::normalize_locale(manifest.default.trim());

    manifest
        .languages
        .iter()
        .find(|language| {
            languages::normalize_locale(&language.code)
                == normalized_default
        })
        .cloned()
        .ok_or_else(|| {
            format!(
                "module language manifest defines invalid default {}",
                manifest.default
            )
        })
}

fn module_language_file_path(
    module_dir: &Path,
    language: &ModuleLanguageEntry,
) -> Result<PathBuf, String> {
    let file = language.file.trim();

    if file.is_empty() {
        return Err(format!(
            "module language {} does not declare a file",
            language.code
        ));
    }

    let relative = Path::new(file);

    if relative.is_absolute() {
        return Err(format!(
            "module language {} declares an absolute file path",
            language.code
        ));
    }

    let relative =
        Path::new("languages").join(relative);

    let relative_text =
        relative.to_string_lossy();

    let resolved = resolve_module_file(
        module_dir,
        relative_text.as_ref(),
        "language file",
    )?;

    let language_root =
        module_dir
            .join("languages")
            .canonicalize()
            .map_err(|error| {
                format!(
                    "could not resolve module languages directory {}: {error}",
                    module_dir.join("languages").display()
                )
            })?;

    if !resolved.starts_with(&language_root) {
        return Err(format!(
            "module language {} escapes languages directory",
            language.code
        ));
    }

    Ok(resolved)
}
fn load_module_strings(
    module_dir: &Path,
    requested: &str,
) -> Result<BTreeMap<String, String>, String> {
    validate_module_language_contract(module_dir)?;

    let manifest =
        read_module_language_manifest(module_dir)?;

    let language =
        selected_module_language_entry(&manifest, requested)?;

    let path =
        module_language_file_path(module_dir, &language)?;

    let raw = fs::read_to_string(&path).map_err(|error| {
        format!(
            "could not read module language file {}: {error}",
            path.display()
        )
    })?;

    serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid module language file {}: {error}",
            path.display()
        )
    })
}

fn resolve_module_language(
    module_dir: &Path,
    requested: &str,
) -> Result<String, String> {
    validate_module_language_contract(module_dir)?;

    let manifest =
        read_module_language_manifest(module_dir)?;

    Ok(
        selected_module_language_entry(
            &manifest,
            requested,
        )?
        .code
    )
}
#[cfg(test)]
mod module_language_presentation_tests {
    use super::*;

    fn temporary_language_module(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "neebles-module-language-{}-{}-{}",
            std::process::id(),
            transaction_id(),
            label
        ));

        fs::create_dir_all(path.join("languages")).unwrap();

        path
    }

    fn write_valid_language_contract(path: &Path) {
        fs::write(
            path.join("languages/manifest.json"),
            serde_json::to_vec(&serde_json::json!({
                "schema": MODULE_LANGUAGE_SCHEMA_VERSION,
                "default": "es_CL",
                "languages": [
                    {
                        "code": "es_CL",
                        "file": "es_CL.json"
                    },
                    {
                        "code": "en_US",
                        "file": "en_US.json"
                    }
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        fs::write(
            path.join("languages/es_CL.json"),
            serde_json::to_vec(&serde_json::json!({
                "surface.open": "Abrir"
            }))
            .unwrap(),
        )
        .unwrap();

        fs::write(
            path.join("languages/en_US.json"),
            serde_json::to_vec(&serde_json::json!({
                "surface.open": "Open"
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn module_strings_use_requested_language_and_default_fallback() {
        let path =
            temporary_language_module("selection");

        write_valid_language_contract(&path);

        let english =
            load_module_strings(&path, "en-US").unwrap();

        assert_eq!(
            english.get("surface.open").map(String::as_str),
            Some("Open")
        );

        let fallback =
            load_module_strings(&path, "fr_FR").unwrap();

        assert_eq!(
            fallback.get("surface.open").map(String::as_str),
            Some("Abrir")
        );

        assert_eq!(
            resolve_module_language(&path, "en-US").unwrap(),
            "en_US"
        );

        assert_eq!(
            resolve_module_language(&path, "fr_FR").unwrap(),
            "es_CL"
        );

        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn module_language_file_cannot_escape_languages_directory() {
        let path =
            temporary_language_module("escape");

        fs::write(
            path.join("languages/manifest.json"),
            serde_json::to_vec(&serde_json::json!({
                "schema": MODULE_LANGUAGE_SCHEMA_VERSION,
                "default": "es_CL",
                "languages": [
                    {
                        "code": "es_CL",
                        "file": "../escape.json"
                    }
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        fs::write(
            path.join("escape.json"),
            serde_json::to_vec(&serde_json::json!({
                "surface.open": "escape"
            }))
            .unwrap(),
        )
        .unwrap();

        let error =
            validate_module_language_contract(&path).unwrap_err();

        assert!(
            error.contains("invalid path")
                || error.contains("escapes languages directory")
        );

        fs::remove_dir_all(path).unwrap();
    }
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
     * Registry folder-resolution path:
     *
     * a registry entry may declare an installation folder
     * whose name differs from the module manifest identity.
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

        let language_path =
            module_language_file_path(module_dir, language)?;

        let language_raw =
            fs::read_to_string(&language_path).map_err(|error| {
                format!(
                    "could not read module language file {}: {error}",
                    language_path.display()
                )
            })?;

        let _: BTreeMap<String, String> =
            serde_json::from_str(&language_raw).map_err(|error| {
                format!(
                    "invalid module language file {}: {error}",
                    language_path.display()
                )
            })?;
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

pub(crate) fn resolve_module_file(
    module_dir: &Path,
    value: &str,
    field: &str,
) -> Result<PathBuf, String> {
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

fn launcher_action_from_contracts(
    module_name: &str,
    contracts: &ModuleContracts,
) -> Result<Option<String>, String> {
    let mut actions = std::collections::BTreeSet::<String>::new();

    for (contract_type, contract) in &contracts.contracts {
        for (name, endpoint) in &contract.endpoints {
            if !endpoint.launcher {
                continue;
            }

            if contract_type != "commands" {
                return Err(format!(
                    "module '{}' marks endpoint '{}.{}' as launcher=true, but Boss Launcher actions must belong to the 'commands' contract",
                    module_name,
                    contract_type,
                    name
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
     * Dynamic contracts are validated here as part of the installed
     * module contract, not lazily after installation.
     *
     * This also certifies the Boss Launcher invariant:
     * only the dynamic "commands" contract may expose launcher=true,
     */
    let contracts = load_module_contracts(&manifest.name, module_dir, &manifest.contracts)?;

    let _ = launcher_action_from_contracts(&manifest.name, &contracts)?;

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
    let lifecycle = lifecycle_contract_from_module(module_dir, manifest)?;

    /*
     * Boss surface declarations are part of the installed module
     * contract and must resolve only real Lifecycle identities.
     */
    let _ = surface_projection_from_module(module_dir, manifest, &lifecycle)?;

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

    let raw_value: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid module manifest {}: {error}", path.display()))?;

    if raw_value
        .as_object()
        .is_some_and(|object| object.contains_key("commands"))
    {
        return Err(format!(
            "module manifest {} contains deprecated field 'commands'; schema {} uses dynamic contracts",
            path.display(),
            MODULE_SCHEMA_VERSION
        ));
    }

    let manifest: ModuleManifest = serde_json::from_value(raw_value)
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

            surfaces: None,

            contracts: Vec::new(),

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

        assert_eq!(candidate.module_id, "gamma");

        assert_eq!(candidate.folder(), "gamma-folder");

        assert_eq!(candidate.manifest.name, "gamma");

        assert!(candidate.staging.path().exists());

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
            surfaces: None,
            contracts: Vec::new(),
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
        .expect("dynamic contract must parse")
    }

    #[test]
    fn launcher_contract_resolves_dynamic_commands_endpoint() {
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
            launcher_action_from_contracts("test-module", &contracts)
                .expect("dynamic Launcher action must resolve"),
            Some("open".to_string())
        );
    }

    #[test]
    fn launcher_contract_rejects_two_dynamic_launcher_actions() {
        let mut contracts = ModuleContracts::default();

        contracts.contracts.insert(
            "commands".to_string(),
            dynamic_contract(
                "commands",
                json!({
                    "open": {
                        "endpoint": "ui.open",
                        "launcher": true
                    },
                    "settings": {
                        "endpoint": "ui.settings",
                        "launcher": true
                    }
                }),
            ),
        );

        let error = launcher_action_from_contracts("test-module", &contracts)
            .expect_err("multiple Launcher actions must be rejected");

        assert!(error.contains("more than one launcher action"));
    }

    #[test]
    fn launcher_contract_rejects_launcher_outside_commands_contract() {
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

        let error = launcher_action_from_contracts("test-module", &contracts)
            .expect_err("launcher=true outside commands must fail");

        assert!(error.contains("'commands' contract"));
    }

    #[test]
    fn launcher_contract_allows_no_launcher_action() {
        let contracts = ModuleContracts::default();

        assert_eq!(
            launcher_action_from_contracts("test-module", &contracts)
                .expect("Launcher is optional"),
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
    fn certification_live_update_rollback_restores_previous_version() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let current = temporary.path().join("current");
        let backup = temporary.path().join("backup");
        let failed = temporary.path().join("failed");

        std::fs::create_dir(&current).expect("current fixture");
        std::fs::create_dir(&backup).expect("backup fixture");

        std::fs::write(current.join("identity"), b"new").expect("new identity");
        std::fs::write(backup.join("identity"), b"old").expect("old identity");

        let report = super::rollback_live_update_paths(&current, &backup, &failed);

        assert_eq!(
            report.state,
            super::LiveUpdateRollbackState::PreviousRestored
        );

        assert_eq!(
            std::fs::read(current.join("identity")).expect("restored identity"),
            b"old"
        );

        assert!(!backup.exists());
        assert!(!failed.exists());
    }

    #[test]
    fn certification_live_update_rollback_preserves_new_when_move_aside_fails() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let current = temporary.path().join("current");
        let backup = temporary.path().join("backup");

        let failed = temporary.path().join("missing-parent").join("failed");

        std::fs::create_dir(&current).expect("current fixture");
        std::fs::create_dir(&backup).expect("backup fixture");

        std::fs::write(current.join("identity"), b"new").expect("new identity");
        std::fs::write(backup.join("identity"), b"old").expect("old identity");

        let report = super::rollback_live_update_paths(&current, &backup, &failed);

        assert_eq!(report.state, super::LiveUpdateRollbackState::NewPreserved);

        assert_eq!(
            std::fs::read(current.join("identity")).expect("active identity"),
            b"new"
        );

        assert_eq!(
            std::fs::read(backup.join("identity")).expect("backup identity"),
            b"old"
        );
    }

    #[test]
    fn certification_live_update_rollback_reports_no_active_module_when_both_are_absent() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let current = temporary.path().join("current");
        let backup = temporary.path().join("backup");
        let failed = temporary.path().join("failed");

        let report = super::rollback_live_update_paths(&current, &backup, &failed);

        assert_eq!(report.state, super::LiveUpdateRollbackState::NoActiveModule);

        assert!(!current.exists());
        assert!(!backup.exists());
        assert!(!failed.exists());
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

#[cfg(test)]
mod point2_tray_governed_runtime_tests {
    use super::*;

    fn unique_marker_name(label: &str) -> String {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock must work")
            .as_nanos();

        format!("point2-{label}-{}-{unique}", std::process::id())
    }

    #[test]
    fn tray_runtime_identity_serialization_preserves_pid_incarnation() {
        let identity = ProcessIdentity {
            pid: 4242,
            start_time_ticks: 987654321,
        };

        let encoded = serde_json::to_string(&identity).expect("identity must serialize");

        let decoded: ProcessIdentity =
            serde_json::from_str(&encoded).expect("identity must deserialize");

        assert_eq!(decoded, identity);
    }

    #[test]
    fn stale_waiter_cannot_clear_newer_tray_runtime_marker() {
        let name = unique_marker_name("marker");

        let old = ProcessIdentity {
            pid: 10001,
            start_time_ticks: 111,
        };

        let new = ProcessIdentity {
            pid: 10002,
            start_time_ticks: 222,
        };

        write_tray_runtime_marker(&name, old).expect("old marker must write");

        write_tray_runtime_marker(&name, new).expect("new marker must replace old marker");

        clear_tray_runtime_marker_if_identity(&name, old);

        let raw = fs::read_to_string(tray_runtime_marker_path(&name))
            .expect("new marker must survive stale waiter cleanup");

        let observed: ProcessIdentity =
            serde_json::from_str(&raw).expect("marker must remain valid");

        assert_eq!(observed, new);

        clear_tray_runtime_marker_if_identity(&name, new);

        assert!(
            !tray_runtime_marker_path(&name).exists(),
            "exact current identity must clear marker"
        );
    }

    #[test]
    fn stale_process_incarnation_is_not_accepted_as_live_tray_group() {
        let pid = std::process::id();

        let current = capture_process_identity(pid)
            .expect("identity inspection must work")
            .expect("test process must exist");

        let stale = ProcessIdentity {
            pid,
            start_time_ticks: current.start_time_ticks.saturating_add(1),
        };

        assert!(
            !tray_runtime_group_alive(stale).expect("stale identity probe must work"),
            "PID reuse must never inherit tray ownership"
        );
    }

    #[test]
    fn governed_tray_process_group_can_be_stopped_as_one_owned_tree() {
        let mut command = std::process::Command::new("/bin/sh");

        command
            .arg("-c")
            .arg("sleep 30 & wait")
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut child = command.spawn().expect("fixture group must spawn");

        let identity = capture_process_identity(child.id())
            .expect("fixture identity must inspect")
            .expect("fixture leader must exist");

        assert!(
            tray_runtime_group_alive(identity).expect("fixture group must probe"),
            "fresh governed process group must be alive"
        );

        assert!(
            signal_tray_runtime_group(identity, libc::SIGTERM,)
                .expect("SIGTERM delivery must work"),
            "live governed process group must accept SIGTERM"
        );

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);

        while tray_runtime_group_alive(identity).expect("fixture group probe must work")
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        if tray_runtime_group_alive(identity).expect("final fixture probe must work") {
            let _ = signal_tray_runtime_group(identity, libc::SIGKILL);
        }

        let _ = child.wait();

        assert!(
            !tray_runtime_group_alive(identity).expect("dead fixture group must probe"),
            "whole governed process group must terminate"
        );
    }

    #[test]
    fn stale_identity_cannot_authorize_live_process_group() {
        let mut command = std::process::Command::new("/bin/sh");

        command
            .arg("-c")
            .arg("sleep 30 & wait")
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut child = command.spawn().expect("fixture process group must spawn");

        let identity = capture_process_identity(child.id())
            .expect("fixture identity inspection must work")
            .expect("fixture leader must exist");

        let stale = ProcessIdentity {
            pid: identity.pid,
            start_time_ticks: identity.start_time_ticks.saturating_add(1),
        };

        let error = tray_runtime_group_alive(stale)
            .expect_err("live group with stale leader identity must be unsafe");

        assert!(
            error.contains("identity changed"),
            "unexpected ownership error: {error}"
        );

        signal_tray_runtime_group(identity, libc::SIGKILL)
            .expect("authenticated identity must still control its own group");

        let _ = child.wait();
    }
}

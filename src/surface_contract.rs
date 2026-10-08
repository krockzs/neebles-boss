use crate::lifecycle::LifecycleContract;
use crate::surface_projection::{SurfaceProjection, SurfaceProjectionItem, SurfaceRequirements};

use serde::Deserialize;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/*
 * N.E.E.B.L.E.S. Boss Surface Contract.
 *
 * This is a module-owned presentation declaration.
 *
 * Lifecycle owns:
 * - objects
 * - transitions
 * - execution
 * - canonical functional state
 *
 * Surface Contract owns only:
 * - what module-owned Lifecycle identity may be projected
 * - onto which Boss surface
 * - its default presentation visibility
 *
 * Surface names remain arbitrary Strings.
 *
 * Boss UI, Launcher and Tray are current consumers.
 * Future Boss surfaces require no schema change.
 */

pub const SURFACE_CONTRACT_SCHEMA_VERSION: u32 = 2;

fn default_schema() -> u32 {
    SURFACE_CONTRACT_SCHEMA_VERSION
}

fn default_visible() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceDeclaration {
    pub surface: String,

    #[serde(default)]
    pub object_id: Option<String>,

    #[serde(default)]
    pub transition: Option<String>,

    #[serde(default)]
    pub require: SurfaceRequirements,

    #[serde(default = "default_visible")]
    pub visible: bool,

    /*
     * Arbitrary module-owned presentation payload.
     *
     * Boss core does not interpret these keys.
     * Individual Boss surfaces may consume keys they understand.
     */
    #[serde(default)]
    pub data: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceContract {
    #[serde(default = "default_schema")]
    pub schema: u32,

    #[serde(default)]
    pub items: BTreeMap<String, SurfaceDeclaration>,
}

pub fn load(
    module_id: &str,
    path: &Path,
    lifecycle: &LifecycleContract,
) -> Result<SurfaceProjection, String> {
    if module_id.trim().is_empty() {
        return Err("surface contract owner module cannot be empty".to_string());
    }

    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "could not read surface contract {}: {error}",
            path.display()
        )
    })?;

    let contract: SurfaceContract = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid surface contract {}: {error}", path.display()))?;

    if contract.schema != SURFACE_CONTRACT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported surface contract schema {}; Boss supports {}",
            contract.schema, SURFACE_CONTRACT_SCHEMA_VERSION
        ));
    }

    let mut projection = SurfaceProjection::new();

    for (id, declaration) in contract.items {
        if id.trim().is_empty() {
            return Err("surface contract item id cannot be empty".to_string());
        }

        let item = SurfaceProjectionItem::with_data_and_requirements(
            id,
            module_id,
            declaration.surface,
            declaration.object_id,
            declaration.transition,
            declaration.visible,
            declaration.data,
            declaration.require,
        )?;

        projection.register_for_lifecycle(lifecycle, item)?;
    }

    Ok(projection)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle::{Battleplan, LifecycleContract, ObjectContract};

    use std::time::{SystemTime, UNIX_EPOCH};

    fn lifecycle() -> LifecycleContract {
        let mut lifecycle = LifecycleContract::default();

        let mut server = ObjectContract::default();

        server
            .transitions
            .insert("enable".to_string(), Battleplan::default());

        server
            .transitions
            .insert("disable".to_string(), Battleplan::default());

        lifecycle.objects.insert("server".to_string(), server);

        lifecycle
            .transitions
            .insert("repair".to_string(), Battleplan::default());

        lifecycle
    }

    fn temporary_file(label: &str, raw: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "neebles-surface-contract-{label}-{}-{nonce}.json",
            std::process::id()
        ));

        fs::write(&path, raw).unwrap();

        path
    }

    #[test]
    fn empty_contract_is_valid() {
        let path = temporary_file(
            "empty",
            r#"{
                "schema": 2,
                "items": {}
            }"#,
        );

        let projection = load("module.alpha", &path, &lifecycle()).unwrap();

        assert!(projection.items().is_empty());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn arbitrary_surface_name_is_valid() {
        let path = temporary_file(
            "future-surface",
            r#"{
                "schema": 2,
                "items": {
                    "server.future": {
                        "surface": "whatever.future.surface",
                        "object_id": "server",
                        "visible": true
                    }
                }
            }"#,
        );

        let projection = load("module.alpha", &path, &lifecycle()).unwrap();

        assert_eq!(
            projection.get("server.future").unwrap().surface(),
            "whatever.future.surface"
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn same_object_can_be_declared_for_many_surfaces() {
        let path = temporary_file(
            "shared-object",
            r#"{
                "schema": 2,
                "items": {
                    "server.ui": {
                        "surface": "ui",
                        "object_id": "server",
                        "transition": "enable"
                    },
                    "server.launcher": {
                        "surface": "launcher",
                        "object_id": "server",
                        "transition": "enable"
                    },
                    "server.tray": {
                        "surface": "tray",
                        "object_id": "server",
                        "transition": "enable"
                    }
                }
            }"#,
        );

        let projection = load("module.alpha", &path, &lifecycle()).unwrap();

        assert_eq!(projection.items().len(), 3);

        for item in projection.items().values() {
            assert_eq!(item.object_id(), Some("server"));
        }

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn many_items_can_target_same_surface() {
        let mut lifecycle = lifecycle();

        lifecycle
            .objects
            .insert("monitor".to_string(), ObjectContract::default());

        let path = temporary_file(
            "many-items",
            r#"{
                "schema": 2,
                "items": {
                    "server.ui": {
                        "surface": "ui",
                        "object_id": "server"
                    },
                    "monitor.ui": {
                        "surface": "ui",
                        "object_id": "monitor"
                    }
                }
            }"#,
        );

        let projection = load("module.alpha", &path, &lifecycle).unwrap();

        assert_eq!(projection.for_surface("ui").len(), 2);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn declaration_cannot_invent_object() {
        let path = temporary_file(
            "ghost-object",
            r#"{
                "schema": 2,
                "items": {
                    "ghost.ui": {
                        "surface": "ui",
                        "object_id": "ghost"
                    }
                }
            }"#,
        );

        let error = load("module.alpha", &path, &lifecycle()).unwrap_err();

        assert!(error.contains("unknown lifecycle object 'ghost'"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn declaration_cannot_invent_object_transition() {
        let path = temporary_file(
            "ghost-transition",
            r#"{
                "schema": 2,
                "items": {
                    "server.ui": {
                        "surface": "ui",
                        "object_id": "server",
                        "transition": "explode"
                    }
                }
            }"#,
        );

        let error = load("module.alpha", &path, &lifecycle()).unwrap_err();

        assert!(error.contains("unknown transition 'explode'"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn module_transition_can_be_projected_without_object() {
        let path = temporary_file(
            "module-transition",
            r#"{
                "schema": 2,
                "items": {
                    "repair.launcher": {
                        "surface": "launcher",
                        "transition": "repair"
                    }
                }
            }"#,
        );

        let projection = load("module.alpha", &path, &lifecycle()).unwrap();

        assert_eq!(
            projection.get("repair.launcher").unwrap().transition(),
            Some("repair")
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn empty_declaration_without_identity_or_data_is_rejected() {
        let path = temporary_file(
            "empty-content",
            r#"{
                "schema": 2,
                "items": {
                    "nothing": {
                        "surface": "ui"
                    }
                }
            }"#,
        );

        let error = load("module.alpha", &path, &lifecycle()).unwrap_err();

        assert!(error.contains("Lifecycle identity, presentation data, or both"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn presentation_only_content_is_valid() {
        let path = temporary_file(
            "presentation-only",
            r#"{
                "schema": 2,
                "items": {
                    "information.ui": {
                        "surface": "ui",
                        "data": {
                            "whatever.future": "banana"
                        }
                    }
                }
            }"#,
        );

        let projection = load("module.alpha", &path, &lifecycle()).unwrap();

        let item = projection.get("information.ui").unwrap();

        assert_eq!(item.object_id(), None);

        assert_eq!(item.transition(), None);

        assert_eq!(
            item.data().get("whatever.future").map(String::as_str),
            Some("banana")
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn typed_requirements_are_projected() {
        let path = temporary_file(
            "typed-requirements",
            r#"{
                "schema": 2,
                "items": {
                    "notify.ui": {
                        "surface": "ui",
                        "require": {
                            "self": "open",
                            "modules": {
                                "network-core": "active",
                                "remote-core": "open"
                            }
                        },
                        "data": {
                            "control": "button",
                            "action": "notify-demo",
                            "label_key": "features.notify"
                        }
                    }
                }
            }"#,
        );

        let projection = load("module.alpha", &path, &lifecycle()).unwrap();

        let requirements = projection.get("notify.ui").unwrap().requirements();

        assert_eq!(
            requirements.self_state(),
            Some(crate::surface_projection::SurfaceRequirementState::Open)
        );

        assert_eq!(
            requirements.modules().get("network-core"),
            Some(&crate::surface_projection::SurfaceRequirementState::Active)
        );

        assert_eq!(
            requirements.modules().get("remote-core"),
            Some(&crate::surface_projection::SurfaceRequirementState::Open)
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_requirement_state_is_rejected() {
        let path = temporary_file(
            "invalid-requirement-state",
            r#"{
                "schema": 2,
                "items": {
                    "notify.ui": {
                        "surface": "ui",
                        "require": {
                            "self": "automatic"
                        },
                        "data": {
                            "control": "button"
                        }
                    }
                }
            }"#,
        );

        let error = load("module.alpha", &path, &lifecycle()).unwrap_err();

        assert!(error.contains("unknown variant"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_requirement_module_id_is_rejected() {
        let path = temporary_file(
            "invalid-requirement-module",
            r#"{
                "schema": 2,
                "items": {
                    "notify.ui": {
                        "surface": "ui",
                        "require": {
                            "modules": {
                                "bad/module": "active"
                            }
                        },
                        "data": {
                            "control": "button"
                        }
                    }
                }
            }"#,
        );

        let error = load("module.alpha", &path, &lifecycle()).unwrap_err();

        assert!(error.contains("invalid surface requirement module id 'bad/module'"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn unknown_requirement_fields_are_rejected() {
        let path = temporary_file(
            "unknown-requirement-field",
            r#"{
                "schema": 2,
                "items": {
                    "notify.ui": {
                        "surface": "ui",
                        "require": {
                            "self": "active",
                            "automatic": true
                        },
                        "data": {
                            "control": "button"
                        }
                    }
                }
            }"#,
        );

        let error = load("module.alpha", &path, &lifecycle()).unwrap_err();

        assert!(error.contains("unknown field"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn schema_one_is_rejected_after_v2_cutover() {
        let path = temporary_file(
            "schema-one-cutover",
            r#"{
                "schema": 1,
                "items": {}
            }"#,
        );

        let error = load("module.alpha", &path, &lifecycle()).unwrap_err();

        assert!(error.contains("unsupported surface contract schema 1"));

        fs::remove_file(path).unwrap();
    }
    #[test]
    fn unknown_fields_are_rejected() {
        let path = temporary_file(
            "unknown-field",
            r#"{
                "schema": 2,
                "items": {
                    "server.ui": {
                        "surface": "ui",
                        "object_id": "server",
                        "magic": true
                    }
                }
            }"#,
        );

        let error = load("module.alpha", &path, &lifecycle()).unwrap_err();

        assert!(error.contains("unknown field"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn unsupported_schema_is_rejected() {
        let path = temporary_file(
            "schema",
            r#"{
                "schema": 999,
                "items": {}
            }"#,
        );

        let error = load("module.alpha", &path, &lifecycle()).unwrap_err();

        assert!(error.contains("unsupported surface contract schema"));

        fs::remove_file(path).unwrap();
    }
}

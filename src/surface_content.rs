use std::collections::BTreeMap;

use crate::lifecycle_objects::ObjectStateStore;
use crate::surface_projection::{SurfaceProjection, SurfaceProjectionItem};

/*
 * Generic Boss Surface Content Resolver.
 *
 * This is the canonical presentation-facing output boundary.
 *
 * It knows:
 * - module ownership
 * - module-local item identity
 * - arbitrary Boss surface name
 * - optional Lifecycle object identity
 * - optional Lifecycle transition identity
 * - presentation visibility
 * - arbitrary presentation data
 *
 * It does NOT know:
 * - Qt widgets
 * - Launcher implementation
 * - Tray provider technology
 * - Plasma
 * - IPC transport
 * - module implementation technology
 *
 * Functional state remains owned by Lifecycle.
 */

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SurfaceContentIdentity {
    owner_module: String,
    item_id: String,
}

impl SurfaceContentIdentity {
    pub fn owner_module(&self) -> &str {
        &self.owner_module
    }

    pub fn item_id(&self) -> &str {
        &self.item_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceContentItem {
    identity: SurfaceContentIdentity,
    surface: String,
    object_id: Option<String>,
    transition: Option<String>,
    visible: bool,
    data: BTreeMap<String, String>,
}

impl SurfaceContentItem {
    fn from_projection(item: &SurfaceProjectionItem) -> Self {
        Self {
            identity: SurfaceContentIdentity {
                owner_module: item.owner_module().to_string(),

                item_id: item.id().to_string(),
            },

            surface: item.surface().to_string(),

            object_id: item.object_id().map(str::to_string),

            transition: item.transition().map(str::to_string),

            visible: item.visible(),

            data: item.data().clone(),
        }
    }

    pub fn identity(&self) -> &SurfaceContentIdentity {
        &self.identity
    }

    pub fn surface(&self) -> &str {
        &self.surface
    }

    pub fn object_id(&self) -> Option<&str> {
        self.object_id.as_deref()
    }

    pub fn transition(&self) -> Option<&str> {
        self.transition.as_deref()
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn data(&self) -> &BTreeMap<String, String> {
        &self.data
    }

    /*
     * Functional state is resolved only from Lifecycle.
     *
     * Multiple UI / Launcher / Tray content items bound to the same
     * owner_module + object_id therefore read the same functional truth.
     */
    pub fn canonical_active(&self, states: &ObjectStateStore) -> Result<Option<bool>, String> {
        let Some(object_id) = self.object_id() else {
            return Ok(None);
        };

        states.active(object_id).map(Some)
    }
}

pub fn resolve_all(projection: &SurfaceProjection) -> Vec<SurfaceContentItem> {
    projection
        .items()
        .values()
        .map(SurfaceContentItem::from_projection)
        .collect()
}

pub fn resolve_for_surface(
    projection: &SurfaceProjection,
    surface: &str,
) -> Vec<SurfaceContentItem> {
    projection
        .for_surface(surface)
        .into_iter()
        .map(SurfaceContentItem::from_projection)
        .collect()
}

pub fn resolve_visible_for_surface(
    projection: &SurfaceProjection,
    surface: &str,
) -> Vec<SurfaceContentItem> {
    projection
        .visible_for_surface(surface)
        .into_iter()
        .map(SurfaceContentItem::from_projection)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::lifecycle::{Battleplan, LifecycleContract, ObjectContract};

    use crate::surface_projection::{SurfaceProjection, SurfaceProjectionItem};

    fn lifecycle() -> LifecycleContract {
        let mut contract = LifecycleContract::default();

        let mut server = ObjectContract::default();

        server
            .transitions
            .insert("enable".to_string(), Battleplan::default());

        server
            .transitions
            .insert("disable".to_string(), Battleplan::default());

        contract.objects.insert("server".to_string(), server);

        contract
    }

    fn projection() -> SurfaceProjection {
        let lifecycle = lifecycle();

        let mut projection = SurfaceProjection::new();

        for surface in ["ui", "launcher", "tray"] {
            projection
                .register_for_lifecycle(
                    &lifecycle,
                    SurfaceProjectionItem::with_data(
                        format!("server.{surface}"),
                        "module.alpha",
                        surface,
                        Some("server".to_string()),
                        Some("enable".to_string()),
                        true,
                        BTreeMap::from([(
                            "future.anything".to_string(),
                            format!("data-for-{surface}"),
                        )]),
                    )
                    .unwrap(),
                )
                .unwrap();
        }

        projection
            .register_for_lifecycle(
                &lifecycle,
                SurfaceProjectionItem::with_data(
                    "information.ui",
                    "module.alpha",
                    "ui",
                    None,
                    None,
                    true,
                    BTreeMap::from([("presentation.only".to_string(), "yes".to_string())]),
                )
                .unwrap(),
            )
            .unwrap();

        projection
    }

    #[test]
    fn resolver_accepts_arbitrary_surface_names() {
        let lifecycle = lifecycle();

        let mut projection = SurfaceProjection::new();

        projection
            .register_for_lifecycle(
                &lifecycle,
                SurfaceProjectionItem::new(
                    "server.future",
                    "module.alpha",
                    "whatever.future.surface",
                    Some("server".to_string()),
                    None,
                    true,
                )
                .unwrap(),
            )
            .unwrap();

        let content = resolve_for_surface(&projection, "whatever.future.surface");

        assert_eq!(content.len(), 1);

        assert_eq!(content[0].surface(), "whatever.future.surface");
    }

    #[test]
    fn resolver_allows_many_items_on_same_surface() {
        let content = resolve_for_surface(&projection(), "ui");

        assert_eq!(content.len(), 2);
    }

    #[test]
    fn identity_is_module_scoped_not_globally_item_only() {
        let projection = projection();

        let content = resolve_for_surface(&projection, "tray");

        assert_eq!(content[0].identity().owner_module(), "module.alpha");

        assert_eq!(content[0].identity().item_id(), "server.tray");
    }

    #[test]
    fn arbitrary_data_survives_resolver_unchanged() {
        let projection = projection();

        let content = resolve_for_surface(&projection, "launcher");

        assert_eq!(
            content[0].data().get("future.anything").map(String::as_str),
            Some("data-for-launcher")
        );
    }

    #[test]
    fn presentation_only_content_survives_resolver() {
        let projection = projection();

        let content = resolve_for_surface(&projection, "ui");

        let item = content
            .iter()
            .find(|item| item.identity().item_id() == "information.ui")
            .unwrap();

        assert_eq!(item.object_id(), None);

        assert_eq!(item.transition(), None);

        assert_eq!(
            item.data().get("presentation.only").map(String::as_str),
            Some("yes")
        );
    }

    #[test]
    fn all_surface_controls_read_same_canonical_object_state() {
        let lifecycle = lifecycle();

        let mut states = ObjectStateStore::new();

        states.register(&lifecycle, "server", false).unwrap();

        let projection = projection();

        let all = resolve_all(&projection);

        let functional = all
            .iter()
            .filter(|item| item.object_id() == Some("server"))
            .collect::<Vec<_>>();

        assert_eq!(functional.len(), 3);

        for item in &functional {
            assert_eq!(item.canonical_active(&states).unwrap(), Some(false));
        }

        states.update("server", true).unwrap();

        for item in &functional {
            assert_eq!(item.canonical_active(&states).unwrap(), Some(true));
        }
    }

    #[test]
    fn visibility_filters_content_without_touching_functional_state() {
        let lifecycle = lifecycle();

        let mut states = ObjectStateStore::new();

        states.register(&lifecycle, "server", true).unwrap();

        let mut projection = projection();

        projection.set_visibility("server.tray", false).unwrap();

        assert!(resolve_visible_for_surface(&projection, "tray",).is_empty());

        let all_tray = resolve_for_surface(&projection, "tray");

        assert_eq!(all_tray[0].canonical_active(&states).unwrap(), Some(true));
    }
}

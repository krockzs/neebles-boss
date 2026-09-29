use std::collections::BTreeMap;

use crate::lifecycle::LifecycleContract;
use crate::lifecycle_objects::ObjectStateStore;

/*
 * Generic Boss surface projection model.
 *
 * Boss owns presentation surfaces such as UI, Launcher and Tray.
 * Modules do not own those surfaces and Lifecycle does not know
 * presentation technology.
 *
 * This model describes Boss-readable content that may be projected
 * onto one or more Boss surfaces.
 *
 * Functional state does NOT live here.
 *
 * enabled/disabled/running/open/etc. remain governed by Lifecycle.
 *
 * `visible` is presentation state only.
 *
 * A projection may reference one Lifecycle object, one Lifecycle
 * transition, or an object-owned transition.
 *
 * Projection references must resolve against the real module-owned
 * LifecycleContract before becoming productive.
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceProjectionItem {
    id: String,
    owner_module: String,
    surface: String,
    object_id: Option<String>,
    transition: Option<String>,
    visible: bool,
    data: BTreeMap<String, String>,
}

impl SurfaceProjectionItem {
    pub fn new(
        id: impl Into<String>,
        owner_module: impl Into<String>,
        surface: impl Into<String>,
        object_id: Option<String>,
        transition: Option<String>,
        visible: bool,
    ) -> Result<Self, String> {
        Self::with_data(
            id,
            owner_module,
            surface,
            object_id,
            transition,
            visible,
            BTreeMap::new(),
        )
    }

    pub fn with_data(
        id: impl Into<String>,
        owner_module: impl Into<String>,
        surface: impl Into<String>,
        object_id: Option<String>,
        transition: Option<String>,
        visible: bool,
        data: BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let id = normalized("surface item id", id.into())?;

        let owner_module = normalized("surface owner module", owner_module.into())?;

        let surface = normalized("surface name", surface.into())?;

        let object_id = normalize_optional("surface object id", object_id)?;

        let transition = normalize_optional("surface transition", transition)?;

        let mut normalized_data = BTreeMap::new();

        for (key, value) in data {
            let key = normalized("surface data key", key)?;

            if normalized_data.insert(key.clone(), value).is_some() {
                return Err(format!(
                    "surface data key '{}' is duplicated after normalization",
                    key
                ));
            }
        }

        if object_id.is_none() && transition.is_none() && normalized_data.is_empty() {
            return Err(
                "surface projection item must declare Lifecycle identity, presentation data, or both"
                    .to_string(),
            );
        }

        Ok(Self {
            id,
            owner_module,
            surface,
            object_id,
            transition,
            visible,
            data: normalized_data,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn owner_module(&self) -> &str {
        &self.owner_module
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

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /*
     * Resolve this presentation declaration against the real
     * module-owned Lifecycle contract.
     *
     * Surface Projection does not create Lifecycle vocabulary.
     *
     * object + transition
     *     => transition must belong to that object
     *
     * object only
     *     => object must exist
     *
     * transition only
     *     => transition must belong to the module-level contract
     */
    pub fn validate_lifecycle(&self, contract: &LifecycleContract) -> Result<(), String> {
        match (self.object_id(), self.transition()) {
            (Some(object_id), Some(transition_id)) => {
                let object = contract.objects.get(object_id).ok_or_else(|| {
                    format!(
                        "surface projection item '{}' references unknown lifecycle object '{}'",
                        self.id(),
                        object_id
                    )
                })?;

                if !object.transitions.contains_key(transition_id) {
                    return Err(format!(
                        "surface projection item '{}' references unknown transition '{}' on lifecycle object '{}'",
                        self.id(),
                        transition_id,
                        object_id
                    ));
                }

                Ok(())
            }

            (Some(object_id), None) => {
                if !contract.objects.contains_key(object_id) {
                    return Err(format!(
                        "surface projection item '{}' references unknown lifecycle object '{}'",
                        self.id(),
                        object_id
                    ));
                }

                Ok(())
            }

            (None, Some(transition_id)) => {
                if !contract.transitions.contains_key(transition_id) {
                    return Err(format!(
                        "surface projection item '{}' references unknown lifecycle transition '{}'",
                        self.id(),
                        transition_id
                    ));
                }

                Ok(())
            }

            (None, None) => {
                if self.data.is_empty() {
                    Err(
                        "surface projection item must declare Lifecycle identity, presentation data, or both"
                            .to_string(),
                    )
                } else {
                    Ok(())
                }
            }
        }
    }

    /*
     * Read functional state from the canonical Lifecycle object store.
     *
     * No surface owns or caches an independent active/enabled state.
     *
     * An action-only projection has no object state and therefore
     * resolves to None.
     */
    pub fn canonical_active(&self, states: &ObjectStateStore) -> Result<Option<bool>, String> {
        let Some(object_id) = self.object_id() else {
            return Ok(None);
        };

        states.active(object_id).map(Some)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SurfaceProjection {
    items: BTreeMap<String, SurfaceProjectionItem>,
}

impl SurfaceProjection {
    pub fn new() -> Self {
        Self::default()
    }

    /*
     * Structural registration.
     *
     * This remains useful while constructing or testing a projection,
     * but productive module-owned content should use
     * register_for_lifecycle so references are authenticated against
     * the real Lifecycle contract.
     */
    pub fn register(&mut self, item: SurfaceProjectionItem) -> Result<(), String> {
        if self.items.contains_key(item.id()) {
            return Err(format!(
                "surface projection item '{}' is already registered",
                item.id()
            ));
        }

        self.items.insert(item.id().to_string(), item);

        Ok(())
    }

    pub fn register_for_lifecycle(
        &mut self,
        contract: &LifecycleContract,
        item: SurfaceProjectionItem,
    ) -> Result<(), String> {
        item.validate_lifecycle(contract)?;
        self.register(item)
    }

    pub fn get(&self, id: &str) -> Option<&SurfaceProjectionItem> {
        self.items.get(id)
    }

    pub fn items(&self) -> &BTreeMap<String, SurfaceProjectionItem> {
        &self.items
    }

    pub fn for_surface(&self, surface: &str) -> Vec<&SurfaceProjectionItem> {
        self.items
            .values()
            .filter(|item| item.surface() == surface)
            .collect()
    }

    pub fn for_module(&self, module: &str) -> Vec<&SurfaceProjectionItem> {
        self.items
            .values()
            .filter(|item| item.owner_module() == module)
            .collect()
    }

    pub fn visible_for_surface(&self, surface: &str) -> Vec<&SurfaceProjectionItem> {
        self.items
            .values()
            .filter(|item| item.surface() == surface && item.visible())
            .collect()
    }

    pub fn set_visibility(&mut self, id: &str, visible: bool) -> Result<(), String> {
        let item = self
            .items
            .get_mut(id)
            .ok_or_else(|| format!("surface projection item '{id}' is not registered"))?;

        item.set_visible(visible);

        Ok(())
    }
}

fn normalized(label: &str, value: String) -> Result<String, String> {
    let value = value.trim();

    if value.is_empty() {
        return Err(format!("{label} cannot be empty"));
    }

    Ok(value.to_string())
}

fn normalize_optional(label: &str, value: Option<String>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };

    Ok(Some(normalized(label, value)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::{Battleplan, LifecycleContract, ObjectContract};

    fn item(
        id: &str,
        module: &str,
        surface: &str,
        object: Option<&str>,
        transition: Option<&str>,
        visible: bool,
    ) -> SurfaceProjectionItem {
        SurfaceProjectionItem::new(
            id,
            module,
            surface,
            object.map(str::to_string),
            transition.map(str::to_string),
            visible,
        )
        .unwrap()
    }

    fn lifecycle_contract() -> LifecycleContract {
        let mut contract = LifecycleContract::default();

        let mut server = ObjectContract::default();
        server
            .transitions
            .insert("enable".to_string(), Battleplan::default());
        server
            .transitions
            .insert("disable".to_string(), Battleplan::default());

        let mut monitor = ObjectContract::default();
        monitor
            .transitions
            .insert("wake".to_string(), Battleplan::default());

        contract.objects.insert("server".to_string(), server);

        contract.objects.insert("monitor".to_string(), monitor);

        contract
            .transitions
            .insert("repair".to_string(), Battleplan::default());

        contract
    }

    #[test]
    fn accepts_arbitrary_surface_names() {
        let item = item(
            "alpha.control",
            "module.alpha",
            "future.surface",
            Some("alpha"),
            None,
            true,
        );

        assert_eq!(item.surface(), "future.surface");
    }

    #[test]
    fn one_module_can_publish_many_items() {
        let mut projection = SurfaceProjection::new();

        projection
            .register(item(
                "alpha.first",
                "module.alpha",
                "ui",
                Some("first"),
                None,
                true,
            ))
            .unwrap();

        projection
            .register(item(
                "alpha.second",
                "module.alpha",
                "ui",
                Some("second"),
                None,
                true,
            ))
            .unwrap();

        projection
            .register(item(
                "alpha.third",
                "module.alpha",
                "tray",
                Some("third"),
                None,
                true,
            ))
            .unwrap();

        assert_eq!(projection.for_module("module.alpha").len(), 3);
        assert_eq!(projection.for_surface("ui").len(), 2);
        assert_eq!(projection.for_surface("tray").len(), 1);
    }

    #[test]
    fn same_object_can_be_projected_on_many_surfaces() {
        let mut projection = SurfaceProjection::new();

        projection
            .register(item(
                "server.ui",
                "module.alpha",
                "ui",
                Some("server"),
                Some("enable"),
                true,
            ))
            .unwrap();

        projection
            .register(item(
                "server.launcher",
                "module.alpha",
                "launcher",
                Some("server"),
                Some("enable"),
                true,
            ))
            .unwrap();

        projection
            .register(item(
                "server.tray",
                "module.alpha",
                "tray",
                Some("server"),
                Some("enable"),
                true,
            ))
            .unwrap();

        for item in projection.items().values() {
            assert_eq!(item.object_id(), Some("server"));
            assert_eq!(item.transition(), Some("enable"));
        }
    }

    #[test]
    fn visibility_is_per_projection_item_not_per_module() {
        let mut projection = SurfaceProjection::new();

        projection
            .register(item(
                "alpha.ui",
                "module.alpha",
                "ui",
                Some("alpha"),
                None,
                true,
            ))
            .unwrap();

        projection
            .register(item(
                "beta.ui",
                "module.alpha",
                "ui",
                Some("beta"),
                None,
                true,
            ))
            .unwrap();

        projection.set_visibility("alpha.ui", false).unwrap();

        assert!(!projection.get("alpha.ui").unwrap().visible());

        assert!(projection.get("beta.ui").unwrap().visible());
    }

    #[test]
    fn visibility_does_not_create_functional_state() {
        let item = item(
            "server.tray",
            "module.alpha",
            "tray",
            Some("server"),
            Some("disable"),
            false,
        );

        assert_eq!(item.object_id(), Some("server"));

        assert_eq!(item.transition(), Some("disable"));

        assert!(!item.visible());
    }

    #[test]
    fn empty_item_without_identity_or_data_is_rejected() {
        let error = SurfaceProjectionItem::new("invalid", "module.alpha", "ui", None, None, true)
            .unwrap_err();

        assert!(error.contains("Lifecycle identity, presentation data, or both"));
    }

    #[test]
    fn presentation_only_item_is_valid() {
        let item = SurfaceProjectionItem::with_data(
            "information.ui",
            "module.alpha",
            "ui",
            None,
            None,
            true,
            BTreeMap::from([("anything.future".to_string(), "whatever".to_string())]),
        )
        .unwrap();

        assert_eq!(item.object_id(), None);

        assert_eq!(item.transition(), None);

        assert_eq!(
            item.data().get("anything.future").map(String::as_str),
            Some("whatever")
        );
    }

    #[test]
    fn duplicate_item_is_rejected() {
        let mut projection = SurfaceProjection::new();

        projection
            .register(item(
                "same",
                "module.alpha",
                "ui",
                Some("alpha"),
                None,
                true,
            ))
            .unwrap();

        let error = projection
            .register(item(
                "same",
                "module.alpha",
                "tray",
                Some("alpha"),
                None,
                true,
            ))
            .unwrap_err();

        assert!(error.contains("already registered"));
    }

    #[test]
    fn unknown_visibility_target_is_rejected() {
        let mut projection = SurfaceProjection::new();

        let error = projection.set_visibility("missing", false).unwrap_err();

        assert!(error.contains("not registered"));
    }

    #[test]
    fn productive_registration_accepts_real_object() {
        let contract = lifecycle_contract();
        let mut projection = SurfaceProjection::new();

        projection
            .register_for_lifecycle(
                &contract,
                item(
                    "server.ui",
                    "module.alpha",
                    "ui",
                    Some("server"),
                    None,
                    true,
                ),
            )
            .unwrap();

        assert_eq!(
            projection.get("server.ui").unwrap().object_id(),
            Some("server")
        );
    }

    #[test]
    fn productive_registration_rejects_unknown_object() {
        let contract = lifecycle_contract();
        let mut projection = SurfaceProjection::new();

        let error = projection
            .register_for_lifecycle(
                &contract,
                item("ghost.ui", "module.alpha", "ui", Some("ghost"), None, true),
            )
            .unwrap_err();

        assert!(error.contains("unknown lifecycle object 'ghost'"));
    }

    #[test]
    fn object_transition_must_belong_to_object() {
        let contract = lifecycle_contract();
        let mut projection = SurfaceProjection::new();

        let error = projection
            .register_for_lifecycle(
                &contract,
                item(
                    "server.invalid",
                    "module.alpha",
                    "ui",
                    Some("server"),
                    Some("wake"),
                    true,
                ),
            )
            .unwrap_err();

        assert!(error.contains("unknown transition 'wake'"));

        assert!(error.contains("lifecycle object 'server'"));
    }

    #[test]
    fn global_transition_can_be_projected_without_object() {
        let contract = lifecycle_contract();
        let mut projection = SurfaceProjection::new();

        projection
            .register_for_lifecycle(
                &contract,
                item(
                    "repair.launcher",
                    "module.alpha",
                    "launcher",
                    None,
                    Some("repair"),
                    true,
                ),
            )
            .unwrap();

        assert_eq!(
            projection.get("repair.launcher").unwrap().transition(),
            Some("repair")
        );
    }

    #[test]
    fn unknown_global_transition_is_rejected() {
        let contract = lifecycle_contract();
        let mut projection = SurfaceProjection::new();

        let error = projection
            .register_for_lifecycle(
                &contract,
                item(
                    "unknown.launcher",
                    "module.alpha",
                    "launcher",
                    None,
                    Some("destroy_everything"),
                    true,
                ),
            )
            .unwrap_err();

        assert!(error.contains("unknown lifecycle transition"));
    }

    #[test]
    fn all_surfaces_read_same_canonical_object_state() {
        let contract = lifecycle_contract();

        let mut states = ObjectStateStore::new();

        states.register(&contract, "server", false).unwrap();

        let mut projection = SurfaceProjection::new();

        for surface in ["ui", "launcher", "tray"] {
            projection
                .register_for_lifecycle(
                    &contract,
                    item(
                        &format!("server.{surface}"),
                        "module.alpha",
                        surface,
                        Some("server"),
                        Some("enable"),
                        true,
                    ),
                )
                .unwrap();
        }

        for item in projection.items().values() {
            assert_eq!(item.canonical_active(&states).unwrap(), Some(false));
        }

        states.update("server", true).unwrap();

        for item in projection.items().values() {
            assert_eq!(item.canonical_active(&states).unwrap(), Some(true));
        }
    }

    #[test]
    fn visibility_change_does_not_change_canonical_object_state() {
        let contract = lifecycle_contract();

        let mut states = ObjectStateStore::new();

        states.register(&contract, "server", true).unwrap();

        let mut projection = SurfaceProjection::new();

        projection
            .register_for_lifecycle(
                &contract,
                item(
                    "server.tray",
                    "module.alpha",
                    "tray",
                    Some("server"),
                    Some("disable"),
                    true,
                ),
            )
            .unwrap();

        projection.set_visibility("server.tray", false).unwrap();

        assert_eq!(
            projection
                .get("server.tray")
                .unwrap()
                .canonical_active(&states)
                .unwrap(),
            Some(true)
        );
    }

    #[test]
    fn action_only_projection_has_no_object_state() {
        let contract = lifecycle_contract();
        let states = ObjectStateStore::new();

        let item = item(
            "repair.launcher",
            "module.alpha",
            "launcher",
            None,
            Some("repair"),
            true,
        );

        item.validate_lifecycle(&contract).unwrap();

        assert_eq!(item.canonical_active(&states).unwrap(), None);
    }
}

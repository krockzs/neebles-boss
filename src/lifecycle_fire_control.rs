use std::collections::BTreeMap;

/*
 * N.E.E.B.L.E.S. lore: fire_control
 *
 * Technical meaning:
 * Dynamic routing layer between an Operation declaration and an
 * implementation registered inside Boss.
 *
 * Fire Control does not prescribe artillery names, objective names,
 * technologies, Rust crates or module semantics.
 *
 * This layer currently performs registration and resolution only.
 * Execution is deliberately left for the execution engine.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FireControlTarget {
    implementation: String,
}

impl FireControlTarget {
    pub fn new(implementation: impl Into<String>) -> Result<Self, String> {
        let implementation = implementation.into();

        if implementation.trim().is_empty() {
            return Err("fire control implementation id cannot be empty".to_string());
        }

        Ok(Self { implementation })
    }

    pub fn implementation(&self) -> &str {
        &self.implementation
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FireControl {
    artillery: BTreeMap<String, BTreeMap<String, FireControlTarget>>,
}

impl FireControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        artillery: impl Into<String>,
        objective: impl Into<String>,
        target: FireControlTarget,
    ) -> Result<(), String> {
        let artillery = artillery.into();
        let objective = objective.into();

        if artillery.trim().is_empty() {
            return Err("fire control artillery id cannot be empty".to_string());
        }

        if objective.trim().is_empty() {
            return Err("fire control objective id cannot be empty".to_string());
        }

        let objectives = self.artillery.entry(artillery.clone()).or_default();

        if objectives.contains_key(&objective) {
            return Err(format!(
                "fire control route already registered for artillery '{artillery}' objective '{objective}'"
            ));
        }

        objectives.insert(objective, target);

        Ok(())
    }

    pub fn resolve(&self, artillery: &str, objective: &str) -> Result<&FireControlTarget, String> {
        if artillery.trim().is_empty() {
            return Err("fire control artillery id cannot be empty".to_string());
        }

        if objective.trim().is_empty() {
            return Err("fire control objective id cannot be empty".to_string());
        }

        self.artillery
            .get(artillery)
            .and_then(|objectives| objectives.get(objective))
            .ok_or_else(|| {
                format!(
                    "fire control has no route for artillery '{artillery}' objective '{objective}'"
                )
            })
    }

    pub fn contains(&self, artillery: &str, objective: &str) -> bool {
        self.artillery
            .get(artillery)
            .is_some_and(|objectives| objectives.contains_key(objective))
    }

    pub fn unregister(&mut self, artillery: &str, objective: &str) -> Option<FireControlTarget> {
        let objectives = self.artillery.get_mut(artillery)?;

        let removed = objectives.remove(objective);

        if objectives.is_empty() {
            self.artillery.remove(artillery);
        }

        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_control_starts_empty() {
        let fire_control = FireControl::new();

        assert!(!fire_control.contains("anything", "anything"));
    }

    #[test]
    fn fire_control_accepts_arbitrary_artillery_and_objective_names() {
        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "future.capability",
                "future.technical.objective",
                FireControlTarget::new("synthetic-implementation").unwrap(),
            )
            .unwrap();

        assert!(fire_control.contains("future.capability", "future.technical.objective"));
    }

    #[test]
    fn fire_control_resolves_registered_target() {
        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "alpha",
                "one",
                FireControlTarget::new("implementation-alpha-one").unwrap(),
            )
            .unwrap();

        let target = fire_control.resolve("alpha", "one").unwrap();

        assert_eq!(target.implementation(), "implementation-alpha-one");
    }

    #[test]
    fn fire_control_keeps_objectives_isolated() {
        let mut fire_control = FireControl::new();

        fire_control
            .register("alpha", "one", FireControlTarget::new("first").unwrap())
            .unwrap();

        fire_control
            .register("alpha", "two", FireControlTarget::new("second").unwrap())
            .unwrap();

        assert_eq!(
            fire_control
                .resolve("alpha", "one")
                .unwrap()
                .implementation(),
            "first"
        );

        assert_eq!(
            fire_control
                .resolve("alpha", "two")
                .unwrap()
                .implementation(),
            "second"
        );
    }

    #[test]
    fn fire_control_keeps_artillery_isolated() {
        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "alpha",
                "same",
                FireControlTarget::new("alpha-implementation").unwrap(),
            )
            .unwrap();

        fire_control
            .register(
                "beta",
                "same",
                FireControlTarget::new("beta-implementation").unwrap(),
            )
            .unwrap();

        assert_eq!(
            fire_control
                .resolve("alpha", "same")
                .unwrap()
                .implementation(),
            "alpha-implementation"
        );

        assert_eq!(
            fire_control
                .resolve("beta", "same")
                .unwrap()
                .implementation(),
            "beta-implementation"
        );
    }

    #[test]
    fn fire_control_rejects_duplicate_route() {
        let mut fire_control = FireControl::new();

        fire_control
            .register("alpha", "one", FireControlTarget::new("first").unwrap())
            .unwrap();

        let error = fire_control
            .register("alpha", "one", FireControlTarget::new("second").unwrap())
            .unwrap_err();

        assert!(error.contains("already registered"));
    }

    #[test]
    fn fire_control_rejects_empty_artillery() {
        let mut fire_control = FireControl::new();

        let error = fire_control
            .register(
                "   ",
                "one",
                FireControlTarget::new("implementation").unwrap(),
            )
            .unwrap_err();

        assert!(error.contains("artillery id cannot be empty"));
    }

    #[test]
    fn fire_control_rejects_empty_objective() {
        let mut fire_control = FireControl::new();

        let error = fire_control
            .register(
                "alpha",
                "   ",
                FireControlTarget::new("implementation").unwrap(),
            )
            .unwrap_err();

        assert!(error.contains("objective id cannot be empty"));
    }

    #[test]
    fn fire_control_rejects_empty_implementation_id() {
        let error = FireControlTarget::new("   ").unwrap_err();

        assert!(error.contains("implementation id cannot be empty"));
    }

    #[test]
    fn fire_control_reports_unknown_route() {
        let fire_control = FireControl::new();

        let error = fire_control.resolve("unknown", "unknown").unwrap_err();

        assert!(error.contains("has no route"));
    }

    #[test]
    fn fire_control_can_unregister_route() {
        let mut fire_control = FireControl::new();

        fire_control
            .register(
                "alpha",
                "one",
                FireControlTarget::new("implementation").unwrap(),
            )
            .unwrap();

        let removed = fire_control.unregister("alpha", "one").unwrap();

        assert_eq!(removed.implementation(), "implementation");

        assert!(!fire_control.contains("alpha", "one"));
    }
}

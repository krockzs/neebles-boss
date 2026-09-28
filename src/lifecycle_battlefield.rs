use std::collections::BTreeMap;

/*
 * N.E.E.B.L.E.S. lore: battlefield
 *
 * Technical meaning:
 * Dynamic execution context owned by one Lifecycle execution.
 *
 * Battlefield stores opaque String values grouped by arbitrary namespaces.
 * It does not prescribe module semantics, technologies, objective vocabularies
 * or namespace names.
 */
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Battlefield {
    namespaces: BTreeMap<String, BTreeMap<String, String>>,
}

impl Battlefield {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        namespace: impl Into<String>,
        path: impl Into<String>,
        value: impl Into<String>,
    ) {
        self.namespaces
            .entry(namespace.into())
            .or_default()
            .insert(path.into(), value.into());
    }

    pub fn get(&self, namespace: &str, path: &str) -> Option<&str> {
        self.namespaces
            .get(namespace)
            .and_then(|values| values.get(path))
            .map(String::as_str)
    }

    pub fn contains_namespace(&self, namespace: &str) -> bool {
        self.namespaces.contains_key(namespace)
    }

    pub fn contains(&self, namespace: &str, path: &str) -> bool {
        self.namespaces
            .get(namespace)
            .is_some_and(|values| values.contains_key(path))
    }

    pub fn remove(&mut self, namespace: &str, path: &str) -> Option<String> {
        let values = self.namespaces.get_mut(namespace)?;

        let removed = values.remove(path);

        if values.is_empty() {
            self.namespaces.remove(namespace);
        }

        removed
    }

    pub fn clear_namespace(&mut self, namespace: &str) -> bool {
        self.namespaces.remove(namespace).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battlefield_accepts_arbitrary_namespaces() {
        let mut battlefield = Battlefield::new();

        battlefield.insert("anything", "technical.path", "value");

        assert_eq!(battlefield.get("anything", "technical.path"), Some("value"));
    }

    #[test]
    fn battlefield_accepts_opaque_dotted_paths() {
        let mut battlefield = Battlefield::new();

        battlefield.insert("namespace", "operation.output.value", "opaque-value");

        assert_eq!(
            battlefield.get("namespace", "operation.output.value"),
            Some("opaque-value")
        );
    }

    #[test]
    fn battlefield_keeps_namespaces_isolated() {
        let mut battlefield = Battlefield::new();

        battlefield.insert("alpha", "same.path", "alpha-value");
        battlefield.insert("beta", "same.path", "beta-value");

        assert_eq!(battlefield.get("alpha", "same.path"), Some("alpha-value"));

        assert_eq!(battlefield.get("beta", "same.path"), Some("beta-value"));
    }

    #[test]
    fn battlefield_can_replace_existing_value() {
        let mut battlefield = Battlefield::new();

        battlefield.insert("alpha", "value", "first");
        battlefield.insert("alpha", "value", "second");

        assert_eq!(battlefield.get("alpha", "value"), Some("second"));
    }

    #[test]
    fn battlefield_can_remove_single_value() {
        let mut battlefield = Battlefield::new();

        battlefield.insert("alpha", "one", "first");
        battlefield.insert("alpha", "two", "second");

        assert_eq!(
            battlefield.remove("alpha", "one"),
            Some("first".to_string())
        );

        assert!(!battlefield.contains("alpha", "one"));
        assert!(battlefield.contains("alpha", "two"));
    }

    #[test]
    fn battlefield_removes_empty_namespace_after_last_value() {
        let mut battlefield = Battlefield::new();

        battlefield.insert("alpha", "one", "first");

        battlefield.remove("alpha", "one");

        assert!(!battlefield.contains_namespace("alpha"));
    }

    #[test]
    fn battlefield_can_clear_namespace() {
        let mut battlefield = Battlefield::new();

        battlefield.insert("alpha", "one", "first");
        battlefield.insert("alpha", "two", "second");

        assert!(battlefield.clear_namespace("alpha"));
        assert!(!battlefield.contains_namespace("alpha"));
    }

    #[test]
    fn battlefield_does_not_prescribe_namespace_vocabulary() {
        let mut battlefield = Battlefield::new();

        battlefield.insert(
            "future_namespace_that_does_not_exist_today",
            "future.path",
            "future-value",
        );

        assert_eq!(
            battlefield.get("future_namespace_that_does_not_exist_today", "future.path",),
            Some("future-value")
        );
    }
}

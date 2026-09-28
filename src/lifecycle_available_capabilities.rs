use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use crate::lifecycle_capabilities::CapabilityHandler;

#[derive(Clone)]
pub struct AvailableCapability {
    implementation_id: String,
    handler: CapabilityHandler,
}

impl AvailableCapability {
    pub fn new(
        implementation_id: impl Into<String>,
        handler: CapabilityHandler,
    ) -> Result<Self, String> {
        let implementation_id = implementation_id.into();

        if implementation_id.trim().is_empty() {
            return Err("available capability implementation id cannot be empty".to_string());
        }

        Ok(Self {
            implementation_id,
            handler,
        })
    }

    pub fn implementation_id(&self) -> &str {
        &self.implementation_id
    }

    pub fn handler(&self) -> CapabilityHandler {
        self.handler.clone()
    }
}

#[derive(Default)]
pub struct AvailableCapabilityCatalog {
    capabilities: BTreeMap<String, AvailableCapability>,
}

impl AvailableCapabilityCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        artillery: impl Into<String>,
        capability: AvailableCapability,
    ) -> Result<(), String> {
        let artillery = artillery.into();

        if artillery.trim().is_empty() {
            return Err("available capability artillery id cannot be empty".to_string());
        }

        if self.capabilities.contains_key(&artillery) {
            return Err(format!(
                "available capability artillery '{}' is already registered",
                artillery
            ));
        }

        self.capabilities.insert(artillery, capability);

        Ok(())
    }

    pub fn resolve(&self, artillery: &str) -> Result<&AvailableCapability, String> {
        if artillery.trim().is_empty() {
            return Err("available capability artillery id cannot be empty".to_string());
        }

        self.capabilities.get(artillery).ok_or_else(|| {
            format!(
                "Boss has no available Rust capability for artillery '{}'",
                artillery
            )
        })
    }

    pub fn contains(&self, artillery: &str) -> bool {
        self.capabilities.contains_key(artillery)
    }
}

static PRODUCTIVE_CATALOG: OnceLock<Arc<AvailableCapabilityCatalog>> = OnceLock::new();

fn build_productive_catalog() -> Result<AvailableCapabilityCatalog, String> {
    /*
     * Productive Rust arsenal entrypoint.
     *
     * Lifecycle does not know module semantics and does not know
     * individual Rust crates.
     *
     * Rust adapters are registered here as Boss gains them.
     * Adding future adapters never changes Lifecycle, Governor,
     * DAG, FireControl or module contracts.
     */
    Ok(AvailableCapabilityCatalog::new())
}

pub fn productive_catalog() -> Result<Arc<AvailableCapabilityCatalog>, String> {
    if let Some(catalog) = PRODUCTIVE_CATALOG.get() {
        return Ok(Arc::clone(catalog));
    }

    let catalog = Arc::new(build_productive_catalog()?);

    match PRODUCTIVE_CATALOG.set(Arc::clone(&catalog)) {
        Ok(()) => Ok(catalog),

        Err(_) => PRODUCTIVE_CATALOG.get().cloned().ok_or_else(|| {
            "productive Lifecycle capability catalog initialization failed".to_string()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use crate::lifecycle_execution::ExecutionPayload;

    fn capability(implementation: &str) -> AvailableCapability {
        AvailableCapability::new(
            implementation,
            Arc::new(|_| Box::pin(async { Ok(ExecutionPayload::new()) })),
        )
        .unwrap()
    }

    #[test]
    fn catalog_starts_empty() {
        let catalog = AvailableCapabilityCatalog::new();

        assert!(!catalog.contains("anything"));
    }

    #[test]
    fn catalog_accepts_arbitrary_artillery() {
        let mut catalog = AvailableCapabilityCatalog::new();

        catalog
            .register("future.tool", capability("rust.future.tool"))
            .unwrap();

        assert!(catalog.contains("future.tool"));
    }

    #[test]
    fn catalog_keeps_implementation_private_to_boss() {
        let mut catalog = AvailableCapabilityCatalog::new();

        catalog
            .register("module.requests.this", capability("boss.private.adapter"))
            .unwrap();

        assert_eq!(
            catalog
                .resolve("module.requests.this")
                .unwrap()
                .implementation_id(),
            "boss.private.adapter"
        );
    }

    #[test]
    fn unavailable_artillery_fails_explicitly() {
        let catalog = AvailableCapabilityCatalog::new();

        let error = match catalog.resolve("missing.tool") {
            Ok(_) => {
                panic!("missing capability unexpectedly resolved");
            }

            Err(error) => error,
        };

        assert!(error.contains("no available Rust capability"));
    }
}

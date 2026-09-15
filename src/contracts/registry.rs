use crate::contracts::schema::{ContractDefinition, ContractEndpoint, ModuleContracts};

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Default)]
pub struct ContractRegistry {
    inner: Arc<RwLock<BTreeMap<String, ModuleContracts>>>,
}

impl ContractRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn replace_module(&self, contracts: ModuleContracts) -> Result<(), String> {
        let mut registry = self
            .inner
            .write()
            .map_err(|_| "contract registry write lock poisoned".to_string())?;

        registry.insert(contracts.module.clone(), contracts);

        Ok(())
    }

    pub fn remove_module(&self, module: &str) -> Result<(), String> {
        let mut registry = self
            .inner
            .write()
            .map_err(|_| "contract registry write lock poisoned".to_string())?;

        registry.remove(module);

        Ok(())
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut registry = self
            .inner
            .write()
            .map_err(|_| "contract registry write lock poisoned".to_string())?;

        registry.clear();

        Ok(())
    }

    pub fn contract(
        &self,
        module: &str,
        contract_type: &str,
    ) -> Result<Option<ContractDefinition>, String> {
        let registry = self
            .inner
            .read()
            .map_err(|_| "contract registry read lock poisoned".to_string())?;

        Ok(registry
            .get(module)
            .and_then(|module| module.contracts.get(contract_type))
            .cloned())
    }

    pub fn endpoint(
        &self,
        module: &str,
        contract_type: &str,
        name: &str,
    ) -> Result<Option<ContractEndpoint>, String> {
        let registry = self
            .inner
            .read()
            .map_err(|_| "contract registry read lock poisoned".to_string())?;

        Ok(registry
            .get(module)
            .and_then(|module| module.contracts.get(contract_type))
            .and_then(|contract| contract.endpoints.get(name))
            .cloned())
    }

    pub fn modules(&self) -> Result<Vec<String>, String> {
        let registry = self
            .inner
            .read()
            .map_err(|_| "contract registry read lock poisoned".to_string())?;

        Ok(registry.keys().cloned().collect())
    }
}

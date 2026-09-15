use crate::contracts::schema::{
    ContractDefinition, ContractReference, ModuleContracts, CONTRACT_SCHEMA_VERSION,
};

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn resolve_contract_path(module_root: &Path, contract_file: &str) -> Result<PathBuf, String> {
    let module_root = module_root.canonicalize().map_err(|error| {
        format!(
            "could not canonicalize module root {}: {error}",
            module_root.display()
        )
    })?;

    let requested = module_root.join(contract_file);

    let resolved = requested.canonicalize().map_err(|error| {
        format!(
            "could not resolve contract file {}: {error}",
            requested.display()
        )
    })?;

    if !resolved.starts_with(&module_root) {
        return Err(format!(
            "contract path escapes module root: {}",
            resolved.display()
        ));
    }

    if !resolved.is_file() {
        return Err(format!(
            "contract path is not a file: {}",
            resolved.display()
        ));
    }

    Ok(resolved)
}

pub fn load_contract(
    module_root: &Path,
    reference: &ContractReference,
) -> Result<ContractDefinition, String> {
    if !reference.enabled {
        return Err(format!(
            "contract '{}' is disabled",
            reference.contract_type
        ));
    }

    let path = resolve_contract_path(module_root, &reference.file)?;

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("could not read contract {}: {error}", path.display()))?;

    let contract: ContractDefinition = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid contract JSON {}: {error}", path.display()))?;

    if contract.schema != reference.schema {
        return Err(format!(
            "contract '{}' schema mismatch: manifest={} file={}",
            reference.contract_type, reference.schema, contract.schema
        ));
    }

    if contract.schema != CONTRACT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported contract schema {} for '{}'; Boss supports {}",
            contract.schema, reference.contract_type, CONTRACT_SCHEMA_VERSION
        ));
    }

    if contract.contract != reference.contract_type {
        return Err(format!(
            "contract type mismatch: manifest='{}' file='{}'",
            reference.contract_type, contract.contract
        ));
    }

    if contract.contract.trim().is_empty() {
        return Err("contract type cannot be empty".to_string());
    }

    for (name, endpoint) in &contract.endpoints {
        if name.trim().is_empty() {
            return Err(format!(
                "contract '{}' contains an empty endpoint name",
                contract.contract
            ));
        }

        if endpoint.endpoint.trim().is_empty() {
            return Err(format!(
                "contract '{}' endpoint '{}' has an empty runtime endpoint",
                contract.contract, name
            ));
        }
    }

    Ok(contract)
}

pub fn load_module_contracts(
    module_name: &str,
    module_root: &Path,
    references: &[ContractReference],
) -> Result<ModuleContracts, String> {
    let mut contracts = BTreeMap::new();

    for reference in references {
        if !reference.enabled {
            continue;
        }

        if contracts.contains_key(&reference.contract_type) {
            return Err(format!(
                "module '{}' declares contract '{}' more than once",
                module_name, reference.contract_type
            ));
        }

        let contract = load_contract(module_root, reference)?;

        contracts.insert(reference.contract_type.clone(), contract);
    }

    Ok(ModuleContracts { contracts })
}

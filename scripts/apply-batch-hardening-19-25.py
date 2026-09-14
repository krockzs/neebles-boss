#!/usr/bin/env python3
from pathlib import Path

path = Path("src/modules.rs")
text = path.read_text()

old = '''fn validate_registry(registry: &Registry) -> Result<(), String> {
    if registry.schema != REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported N.E.E.B.L.E.S. registry schema {}; expected {}",
            registry.schema, REGISTRY_SCHEMA_VERSION
        ));
    }

    for (name, module) in &registry.modules {
'''
new = '''fn validate_registry(registry: &Registry) -> Result<(), String> {
    if registry.schema != REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported N.E.E.B.L.E.S. registry schema {}; expected {}",
            registry.schema, REGISTRY_SCHEMA_VERSION
        ));
    }

    let mut folder_owners = BTreeMap::<String, String>::new();

    for (name, module) in &registry.modules {
'''
assert text.count(old) == 1, "validate_registry header mismatch"
text = text.replace(old, new)

old = '''        if let Some(version) = module.version.as_deref() {
            let version = version.trim();

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
        }

        if let Some(folder) = module.folder.as_deref() {
            if !valid_module_id(folder) {
                return Err(format!(
                    "invalid module folder '{}' in registry for '{}'",
                    folder, name
                ));
            }
        }
'''
new = '''        let version = module.version.as_deref().ok_or_else(|| {
            format!("module '{}' registry version is required", name)
        })?;
        let version = version.trim();

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

        let folder = module.folder.as_deref().unwrap_or(name);
        if !valid_module_id(folder) {
            return Err(format!(
                "invalid module folder '{}' in registry for '{}'",
                folder, name
            ));
        }

        if let Some(owner) = folder_owners.insert(folder.to_string(), name.clone()) {
            return Err(format!(
                "module registry folder collision: '{}' is declared by both '{}' and '{}'",
                folder, owner, name
            ));
        }
'''
assert text.count(old) == 1, "registry version/folder block mismatch"
text = text.replace(old, new)

old = '''    if let Err(error) = fs::remove_dir_all(&backup_path) {
        return Err(format!(
            "module '{}' update was committed successfully, but transactional backup {} could not be removed: {error}",
            name,
            backup_path.display()
        ));
    }

    Ok(())
}
'''
new = '''    if let Err(error) = fs::remove_dir_all(&backup_path) {
        /*
         * The new version is already live. Backup cleanup is maintenance,
         * not transaction failure. Preserve the backup and report a warning
         * instead of lying to callers that the update failed.
         */
        eprintln!(
            "N.E.E.B.L.E.S.: module '{}' update committed successfully, but transactional backup {} could not be removed: {error}",
            name,
            backup_path.display()
        );
    }

    Ok(())
}
'''
assert text.count(old) == 1, "transaction cleanup block mismatch"
text = text.replace(old, new)

marker = '''#[allow(dead_code)]
fn _language_contract_example() -> Result<String, String> {
    Ok(languages::load_manifest()?.default)
}
'''
assert text.count(marker) == 1, "file trailer mismatch"

tests = r'''

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_module(version: Option<&str>, folder: Option<&str>) -> RegistryModule {
        RegistryModule {
            repo: "https://github.com/example/module.git".to_string(),
            commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
            version: version.map(str::to_string),
            folder: folder.map(str::to_string),
        }
    }

    #[test]
    fn registry_requires_semver_version() {
        let mut modules = BTreeMap::new();
        modules.insert("alpha".to_string(), registry_module(None, None));
        let registry = Registry {
            schema: REGISTRY_SCHEMA_VERSION,
            modules,
        };

        let error = validate_registry(&registry).expect_err("missing version must fail");
        assert!(error.contains("version is required"));
    }

    #[test]
    fn registry_rejects_effective_folder_collisions() {
        let mut modules = BTreeMap::new();
        modules.insert(
            "alpha".to_string(),
            registry_module(Some("1.0.0"), Some("shared")),
        );
        modules.insert(
            "beta".to_string(),
            registry_module(Some("1.0.0"), Some("shared")),
        );
        let registry = Registry {
            schema: REGISTRY_SCHEMA_VERSION,
            modules,
        };

        let error = validate_registry(&registry).expect_err("folder collision must fail");
        assert!(error.contains("folder collision"));
    }

    #[test]
    fn registry_accepts_unique_semver_entries() {
        let mut modules = BTreeMap::new();
        modules.insert(
            "alpha".to_string(),
            registry_module(Some("1.2.3"), None),
        );
        modules.insert(
            "beta".to_string(),
            registry_module(Some("2.0.0"), Some("beta-runtime")),
        );
        let registry = Registry {
            schema: REGISTRY_SCHEMA_VERSION,
            modules,
        };

        validate_registry(&registry).expect("valid registry must pass");
    }
}
'''
text = text.replace(marker, marker + tests)
path.write_text(text)
print("BATCH MODULE HARDENING: APPLIED")

use serde::Deserialize;

use std::fs;
use std::path::{Path, PathBuf};

pub const RUNTIME_MANIFEST_AUTHORITY_SCHEMA: &str = "1";
pub const RUNTIME_MANIFEST_AUTHORITY_NAME: &str = "neebles-runtime-manifest-authority";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeManifestAuthorityDescriptor {
    pub schema: String,
    pub name: String,
    pub authority: String,
    pub manifest: PathBuf,
}

fn require_absolute_clean_path(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute: {}", path.display()));
    }

    for component in path.components() {
        if matches!(
            component,
            std::path::Component::ParentDir | std::path::Component::CurDir
        ) {
            return Err(format!(
                "{label} must not contain traversal components: {}",
                path.display()
            ));
        }
    }

    Ok(())
}

pub fn load_runtime_manifest_authority_descriptor(
    descriptor_path: &Path,
    expected_authority: &str,
) -> Result<RuntimeManifestAuthorityDescriptor, String> {
    if expected_authority.trim().is_empty() {
        return Err("runtime manifest authority identity cannot be empty".to_string());
    }

    require_absolute_clean_path(
        descriptor_path,
        "runtime manifest authority descriptor path",
    )?;

    let raw = fs::read_to_string(descriptor_path).map_err(|error| {
        format!(
            "could not read runtime manifest authority descriptor {}: {error}",
            descriptor_path.display()
        )
    })?;

    let descriptor: RuntimeManifestAuthorityDescriptor =
        serde_json::from_str(&raw).map_err(|error| {
            format!(
                "invalid runtime manifest authority descriptor {}: {error}",
                descriptor_path.display()
            )
        })?;

    if descriptor.schema != RUNTIME_MANIFEST_AUTHORITY_SCHEMA {
        return Err("unsupported runtime manifest authority schema".to_string());
    }

    if descriptor.name != RUNTIME_MANIFEST_AUTHORITY_NAME {
        return Err("invalid runtime manifest authority descriptor identity".to_string());
    }

    if descriptor.authority != expected_authority {
        return Err(format!(
            "runtime manifest authority mismatch: expected={} declared={}",
            expected_authority, descriptor.authority
        ));
    }

    require_absolute_clean_path(
        &descriptor.manifest,
        "runtime manifest authority manifest path",
    )?;

    Ok(descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "neebles-runtime-manifest-authority-{name}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn descriptor_resolves_absolute_manifest() {
        let base = fixture_root("valid");
        let descriptor_path = base.join("runtime.json");

        fs::create_dir_all(&base).expect("fixture directory");

        fs::write(
            &descriptor_path,
            serde_json::json!({
                "schema": "1",
                "name": "neebles-runtime-manifest-authority",
                "authority": "fixture.runtime",
                "manifest": "/opt/fixture/domestic-runtime.json"
            })
            .to_string(),
        )
        .expect("fixture descriptor");

        let descriptor =
            load_runtime_manifest_authority_descriptor(&descriptor_path, "fixture.runtime")
                .expect("descriptor must load");

        assert_eq!(
            descriptor.manifest,
            PathBuf::from("/opt/fixture/domestic-runtime.json")
        );

        fs::remove_dir_all(base).expect("fixture cleanup");
    }

    #[test]
    fn descriptor_rejects_wrong_authority() {
        let base = fixture_root("identity");
        let descriptor_path = base.join("runtime.json");

        fs::create_dir_all(&base).expect("fixture directory");

        fs::write(
            &descriptor_path,
            serde_json::json!({
                "schema": "1",
                "name": "neebles-runtime-manifest-authority",
                "authority": "fixture.runtime",
                "manifest": "/opt/fixture/domestic-runtime.json"
            })
            .to_string(),
        )
        .expect("fixture descriptor");

        assert!(
            load_runtime_manifest_authority_descriptor(&descriptor_path, "other.runtime",).is_err()
        );

        fs::remove_dir_all(base).expect("fixture cleanup");
    }
}

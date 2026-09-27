use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domestic_platform_control::authenticate_platform_controlled_file;

pub const PLATFORM_AUTHORITY_SCHEMA: &str = "1";
pub const PLATFORM_AUTHORITY_NAME: &str = "neebles-platform-authority";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformAuthorityDescriptor {
    authority: String,
    descriptor_path: PathBuf,
    protocol: String,
    provider: PathBuf,
}

impl PlatformAuthorityDescriptor {
    pub fn authority(&self) -> &str {
        &self.authority
    }

    pub fn descriptor_path(&self) -> &Path {
        &self.descriptor_path
    }

    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    pub fn provider(&self) -> &Path {
        &self.provider
    }
}

#[cfg(test)]
pub(crate) fn test_platform_authority_descriptor(
    authority: String,
    descriptor_path: PathBuf,
    protocol: String,
    provider: PathBuf,
) -> PlatformAuthorityDescriptor {
    PlatformAuthorityDescriptor {
        authority,
        descriptor_path,
        protocol,
        provider,
    }
}

pub fn load_platform_authority_descriptor(
    descriptor_path: &Path,
    expected_authority: &str,
) -> Result<PlatformAuthorityDescriptor, String> {
    if !descriptor_path.is_absolute() {
        return Err(format!(
            "platform authority descriptor path must be absolute: {}",
            descriptor_path.display()
        ));
    }

    if expected_authority.trim().is_empty() {
        return Err("expected platform authority cannot be empty".to_string());
    }

    let raw = fs::read_to_string(descriptor_path).map_err(|error| {
        format!(
            "could not read platform authority descriptor {}: {error}",
            descriptor_path.display()
        )
    })?;

    let value: Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid platform authority descriptor JSON {}: {error}",
            descriptor_path.display()
        )
    })?;

    if value.get("schema").and_then(Value::as_str) != Some(PLATFORM_AUTHORITY_SCHEMA) {
        return Err("platform authority schema must be String 1".to_string());
    }

    if value.get("name").and_then(Value::as_str) != Some(PLATFORM_AUTHORITY_NAME) {
        return Err("invalid platform authority descriptor name".to_string());
    }

    let authority = value
        .get("authority")
        .and_then(Value::as_str)
        .ok_or_else(|| "platform authority must declare authority String".to_string())?
        .trim();

    if authority != expected_authority {
        return Err(format!(
            "platform authority identity mismatch: expected={expected_authority} descriptor={authority}"
        ));
    }

    let protocol = value
        .get("protocol")
        .and_then(Value::as_str)
        .ok_or_else(|| "platform authority must declare protocol String".to_string())?
        .trim();

    if protocol.is_empty() {
        return Err("platform authority protocol cannot be empty".to_string());
    }

    let provider = value
        .get("provider")
        .and_then(Value::as_str)
        .ok_or_else(|| "platform authority must declare provider String".to_string())?
        .trim();

    let provider = PathBuf::from(provider);

    if !provider.is_absolute() {
        return Err(format!(
            "platform authority provider must be absolute: {}",
            provider.display()
        ));
    }

    if !provider.is_file() {
        return Err(format!(
            "platform authority provider is not a file: {}",
            provider.display()
        ));
    }

    authenticate_platform_controlled_file(&provider).map_err(|error| {
        format!(
            "platform authority provider is not platform-controlled: {}: {error}",
            provider.display()
        )
    })?;

    Ok(PlatformAuthorityDescriptor {
        authority: authority.to_string(),
        descriptor_path: descriptor_path.to_path_buf(),
        protocol: protocol.to_string(),
        provider,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock must work")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "neebles-platform-authority-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    fn write_descriptor(path: &Path, provider: &Path) {
        fs::write(
            path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name": "neebles-platform-authority",
                "authority": "platform.filesystem_boundary",
                "protocol": "neebles-filesystem-boundary-v1",
                "provider": provider
            }))
            .expect("fixture descriptor must serialize"),
        )
        .expect("fixture descriptor must exist");
    }

    #[test]
    fn user_controlled_provider_cannot_resolve_platform_authority() {
        let base = fixture_root("user-provider");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let provider = base.join("provider");

        let descriptor = base.join("platform.json");

        fs::write(&provider, b"fixture provider\n").expect("fixture provider must exist");

        write_descriptor(&descriptor, &provider);

        let error = load_platform_authority_descriptor(&descriptor, "platform.filesystem_boundary")
            .expect_err("user-controlled provider must be rejected");

        assert!(
            error.contains("platform-controlled"),
            "unexpected error: {error}"
        );

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn current_system_boundary_provider_can_resolve_platform_authority() {
        let provider = Path::new("/usr/bin/bwrap");

        if !provider.exists() {
            return;
        }

        let base = fixture_root("system-provider");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let descriptor = base.join("platform.json");

        write_descriptor(&descriptor, provider);

        let authority =
            load_platform_authority_descriptor(&descriptor, "platform.filesystem_boundary")
                .expect("platform-controlled provider must resolve");

        assert_eq!(authority.authority(), "platform.filesystem_boundary");

        assert_eq!(authority.protocol(), "neebles-filesystem-boundary-v1");

        assert_eq!(authority.provider(), provider);

        assert_eq!(authority.descriptor_path(), descriptor);

        fs::remove_dir_all(&base).expect("fixture must clean");
    }
}

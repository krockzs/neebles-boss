use serde_json::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const EXTERNAL_DATA_AUTHORITY_SCHEMA: &str = "1";
pub const EXTERNAL_DATA_AUTHORITY_NAME: &str = "neebles-external-data-authority";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDataAuthorityDescriptor {
    pub authority: String,
    pub descriptor_path: PathBuf,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub access: String,
}

fn validate_absolute_clean_path(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute: {}", path.display()));
    }

    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(format!(
                "{label} cannot contain parent traversal: {}",
                path.display()
            ));
        }
    }

    Ok(())
}

pub fn load_external_data_authority_descriptor(
    descriptor_path: &Path,
    expected_authority: &str,
) -> Result<ExternalDataAuthorityDescriptor, String> {
    if !descriptor_path.is_absolute() {
        return Err(format!(
            "external data authority descriptor path must be absolute: {}",
            descriptor_path.display()
        ));
    }

    if expected_authority.trim().is_empty() {
        return Err("expected external data authority cannot be empty".to_string());
    }

    let raw = fs::read_to_string(descriptor_path).map_err(|error| {
        format!(
            "could not read external data authority descriptor {}: {error}",
            descriptor_path.display()
        )
    })?;

    let value: Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid external data authority descriptor JSON {}: {error}",
            descriptor_path.display()
        )
    })?;

    if value.get("schema").and_then(Value::as_str) != Some(EXTERNAL_DATA_AUTHORITY_SCHEMA) {
        return Err("external data authority schema must be String 1".to_string());
    }

    if value.get("name").and_then(Value::as_str) != Some(EXTERNAL_DATA_AUTHORITY_NAME) {
        return Err("invalid external data authority descriptor name".to_string());
    }

    let authority = value
        .get("authority")
        .and_then(Value::as_str)
        .ok_or_else(|| "external data authority must declare authority String".to_string())?
        .trim();

    if authority != expected_authority {
        return Err(format!(
            "external data authority identity mismatch: expected={expected_authority} descriptor={authority}"
        ));
    }

    let source = value
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| "external data authority must declare source String".to_string())?
        .trim();

    let destination = value
        .get("destination")
        .and_then(Value::as_str)
        .ok_or_else(|| "external data authority must declare destination String".to_string())?
        .trim();

    let access = value
        .get("access")
        .and_then(Value::as_str)
        .ok_or_else(|| "external data authority must declare access String".to_string())?
        .trim();

    if access != "read_only" {
        return Err(format!("unsupported external data access mode: {access}"));
    }

    let source = PathBuf::from(source);
    let destination = PathBuf::from(destination);

    validate_absolute_clean_path(&source, "external data source")?;

    validate_absolute_clean_path(&destination, "external data destination")?;

    if !source.exists() {
        return Err(format!(
            "external data source does not exist: {}",
            source.display()
        ));
    }

    Ok(ExternalDataAuthorityDescriptor {
        authority: authority.to_string(),
        descriptor_path: descriptor_path.to_path_buf(),
        source,
        destination,
        access: access.to_string(),
    })
}

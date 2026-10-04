use serde_json::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const WRITABLE_DATA_AUTHORITY_SCHEMA: &str = "1";
pub const WRITABLE_DATA_AUTHORITY_NAME: &str = "neebles-writable-data-authority";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritableDataAuthorityDescriptor {
    pub authority: String,
    pub descriptor_path: PathBuf,
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritableDataGrant {
    pub authority: String,
    pub source: PathBuf,
    pub destination: PathBuf,
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

pub fn load_writable_data_authority_descriptor(
    descriptor_path: &Path,
    expected_authority: &str,
) -> Result<WritableDataAuthorityDescriptor, String> {
    if !descriptor_path.is_absolute() {
        return Err(format!(
            "writable data authority descriptor path must be absolute: {}",
            descriptor_path.display()
        ));
    }

    if expected_authority.trim().is_empty() {
        return Err("expected writable data authority cannot be empty".to_string());
    }

    let raw = fs::read_to_string(descriptor_path).map_err(|error| {
        format!(
            "could not read writable data authority descriptor {}: {error}",
            descriptor_path.display()
        )
    })?;

    let value: Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid writable data authority descriptor JSON {}: {error}",
            descriptor_path.display()
        )
    })?;

    if value.get("schema").and_then(Value::as_str) != Some(WRITABLE_DATA_AUTHORITY_SCHEMA) {
        return Err("writable data authority schema must be String 1".to_string());
    }

    if value.get("name").and_then(Value::as_str) != Some(WRITABLE_DATA_AUTHORITY_NAME) {
        return Err("invalid writable data authority descriptor name".to_string());
    }

    let authority = value
        .get("authority")
        .and_then(Value::as_str)
        .ok_or_else(|| "writable data authority must declare authority String".to_string())?
        .trim();

    if authority != expected_authority {
        return Err(format!(
            "writable data authority identity mismatch: expected={expected_authority} descriptor={authority}"
        ));
    }

    let root = value
        .get("root")
        .and_then(Value::as_str)
        .ok_or_else(|| "writable data authority must declare root String".to_string())?
        .trim();

    let root = PathBuf::from(root);
    validate_absolute_clean_path(&root, "writable data authority root")?;

    Ok(WritableDataAuthorityDescriptor {
        authority: authority.to_string(),
        descriptor_path: descriptor_path.to_path_buf(),
        root,
    })
}

pub fn grant_writable_data_file_subpath(
    descriptor: &WritableDataAuthorityDescriptor,
    source: &Path,
    destination: &Path,
) -> Result<WritableDataGrant, String> {
    validate_absolute_clean_path(source, "writable data file source")?;
    validate_absolute_clean_path(destination, "writable data file destination")?;

    if !descriptor.root.is_dir() {
        return Err(format!(
            "writable data authority root is not a directory: {}",
            descriptor.root.display()
        ));
    }

    if !source.is_file() {
        return Err(format!(
            "writable data file source is not a regular file: {}",
            source.display()
        ));
    }

    let canonical_root = fs::canonicalize(&descriptor.root).map_err(|error| {
        format!(
            "could not canonicalize writable data authority root {}: {error}",
            descriptor.root.display()
        )
    })?;

    let canonical_source = fs::canonicalize(source).map_err(|error| {
        format!(
            "could not canonicalize writable data file source {}: {error}",
            source.display()
        )
    })?;

    if canonical_source == canonical_root {
        return Err(format!(
            "writable data file grant must select a strict subpath of authority root: {}",
            source.display()
        ));
    }

    canonical_source
        .strip_prefix(&canonical_root)
        .map_err(|_| {
            format!(
                "writable data file source escapes authority root: source={} root={}",
                source.display(),
                descriptor.root.display()
            )
        })?;

    Ok(WritableDataGrant {
        authority: descriptor.authority.clone(),
        source: canonical_source,
        destination: destination.to_path_buf(),
    })
}

pub fn grant_writable_data_subpath(
    descriptor: &WritableDataAuthorityDescriptor,
    source: &Path,
    destination: &Path,
) -> Result<WritableDataGrant, String> {
    validate_absolute_clean_path(source, "writable data source")?;
    validate_absolute_clean_path(destination, "writable data destination")?;

    if !descriptor.root.is_dir() {
        return Err(format!(
            "writable data authority root is not a directory: {}",
            descriptor.root.display()
        ));
    }

    if !source.is_dir() {
        return Err(format!(
            "writable data source is not a directory: {}",
            source.display()
        ));
    }

    let canonical_root = fs::canonicalize(&descriptor.root).map_err(|error| {
        format!(
            "could not canonicalize writable data authority root {}: {error}",
            descriptor.root.display()
        )
    })?;

    let canonical_source = fs::canonicalize(source).map_err(|error| {
        format!(
            "could not canonicalize writable data source {}: {error}",
            source.display()
        )
    })?;

    if canonical_source == canonical_root {
        return Err(format!(
            "writable data grant must select a strict subpath of authority root: {}",
            source.display()
        ));
    }

    canonical_source
        .strip_prefix(&canonical_root)
        .map_err(|_| {
            format!(
                "writable data source escapes authority root: source={} root={}",
                source.display(),
                descriptor.root.display()
            )
        })?;

    Ok(WritableDataGrant {
        authority: descriptor.authority.clone(),
        source: canonical_source,
        destination: destination.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock must work")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "neebles-writable-data-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn descriptor_authorizes_strict_dynamic_subpath() {
        let base = fixture_root("grant");
        let root = base.join("root");
        let selected = root.join("transaction");
        let descriptor_path = base.join("descriptor.json");

        fs::create_dir_all(&selected).expect("selected staging must exist");

        fs::write(
            &descriptor_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name": "neebles-writable-data-authority",
                "authority": "boss.fixture.staging",
                "root": root,
            }))
            .expect("descriptor fixture must serialize"),
        )
        .expect("descriptor fixture must exist");

        let descriptor =
            load_writable_data_authority_descriptor(&descriptor_path, "boss.fixture.staging")
                .expect("descriptor must load");

        let grant =
            grant_writable_data_subpath(&descriptor, &selected, Path::new("/work/transaction"))
                .expect("strict child must grant");

        assert_eq!(grant.authority, "boss.fixture.staging");
        assert_eq!(grant.destination, PathBuf::from("/work/transaction"));

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn grant_rejects_root_itself_and_escape() {
        let base = fixture_root("escape");
        let root = base.join("root");
        let outside = base.join("outside");
        let descriptor_path = base.join("descriptor.json");

        fs::create_dir_all(&root).expect("authority root must exist");
        fs::create_dir_all(&outside).expect("outside fixture must exist");

        fs::write(
            &descriptor_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name": "neebles-writable-data-authority",
                "authority": "boss.fixture.staging",
                "root": root,
            }))
            .expect("descriptor fixture must serialize"),
        )
        .expect("descriptor fixture must exist");

        let descriptor =
            load_writable_data_authority_descriptor(&descriptor_path, "boss.fixture.staging")
                .expect("descriptor must load");

        assert!(
            grant_writable_data_subpath(&descriptor, &descriptor.root, Path::new("/work"),)
                .is_err()
        );

        assert!(
            grant_writable_data_subpath(&descriptor, &outside, Path::new("/work/outside"),)
                .is_err()
        );

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn descriptor_authorizes_strict_regular_file_subpath() {
        let base = fixture_root("file-grant");
        let root = base.join("root");
        let selected = root.join("runtime.json");
        let descriptor_path = base.join("descriptor.json");

        fs::create_dir_all(&root).expect("authority root must exist");
        fs::write(&selected, b"fixture").expect("selected file must exist");

        fs::write(
            &descriptor_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name": "neebles-writable-data-authority",
                "authority": "boss.fixture.staging",
                "root": root,
            }))
            .expect("descriptor fixture must serialize"),
        )
        .expect("descriptor fixture must exist");

        let descriptor =
            load_writable_data_authority_descriptor(&descriptor_path, "boss.fixture.staging")
                .expect("descriptor must load");

        let grant = grant_writable_data_file_subpath(
            &descriptor,
            &selected,
            Path::new("/work/runtime.json"),
        )
        .expect("strict regular file child must grant");

        assert_eq!(grant.authority, "boss.fixture.staging");
        assert_eq!(grant.source, fs::canonicalize(&selected).unwrap());
        assert_eq!(grant.destination, PathBuf::from("/work/runtime.json"));

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn file_grant_rejects_directory_and_escape() {
        let base = fixture_root("file-reject");
        let root = base.join("root");
        let directory = root.join("directory");
        let outside = base.join("outside.json");
        let descriptor_path = base.join("descriptor.json");

        fs::create_dir_all(&directory).expect("directory fixture must exist");
        fs::write(&outside, b"outside").expect("outside fixture must exist");

        fs::write(
            &descriptor_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name": "neebles-writable-data-authority",
                "authority": "boss.fixture.staging",
                "root": root,
            }))
            .expect("descriptor fixture must serialize"),
        )
        .expect("descriptor fixture must exist");

        let descriptor =
            load_writable_data_authority_descriptor(&descriptor_path, "boss.fixture.staging")
                .expect("descriptor must load");

        assert!(grant_writable_data_file_subpath(
            &descriptor,
            &directory,
            Path::new("/work/directory"),
        )
        .is_err());

        assert!(grant_writable_data_file_subpath(
            &descriptor,
            &outside,
            Path::new("/work/outside.json"),
        )
        .is_err());

        fs::remove_dir_all(&base).expect("fixture must clean");
    }
}

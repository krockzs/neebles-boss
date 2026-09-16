use serde::{Deserialize, Serialize};
use serde_json::Map;
use serde_json::Value;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

const CACHED_NIGHTMARE_CATALOG_PATH: &str =
    "/opt/neebles/shared/cache/nightmare/insert.nightmare.json";

const NEEBLES_OS_REPOSITORY: &str = "https://github.com/krockzs/neebles-os.git";

const NIGHTMARE_REMOTE_PATH: &str = "config/nightmare/insert.nightmare.json";
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistentDocument {
    pub descriptor: Map<String, Value>,
    pub content: Map<String, Value>,
    pub meta: Map<String, Value>,
    pub formula: Map<String, Value>,
}

impl PersistentDocument {
    pub fn validate(&self) -> Result<(), String> {
        self.raw_content()?;
        self.formula_id()?;
        Ok(())
    }

    pub fn raw_content(&self) -> Result<&str, String> {
        self.content
            .get("raw")
            .and_then(Value::as_str)
            .ok_or_else(|| "PersistentDocument content.raw must be a String".to_string())
    }

    pub fn formula_id(&self) -> Result<&str, String> {
        self.formula
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "PersistentDocument formula.id must be a non-empty String".to_string())
    }
}

pub fn validate_catalog(catalog: &Value) -> Result<(), String> {
    let root = catalog
        .as_object()
        .ok_or_else(|| "Nightmare catalog root must be an Object".to_string())?;

    let formulas = root
        .get("formulas")
        .and_then(Value::as_object)
        .ok_or_else(|| "Nightmare catalog must contain a 'formulas' object".to_string())?;

    for (formula_id, formula_value) in formulas {
        if formula_id.trim().is_empty() {
            return Err("Nightmare catalog contains an empty formula id".to_string());
        }

        let formula = formula_value
            .as_object()
            .ok_or_else(|| format!("Nightmare formula '{formula_id}' must be an Object"))?;

        if !formula.get("reader").is_some_and(Value::is_object) {
            return Err(format!(
                "Nightmare formula '{formula_id}' must contain a 'reader' object"
            ));
        }

        if !formula.get("writer").is_some_and(Value::is_object) {
            return Err(format!(
                "Nightmare formula '{formula_id}' must contain a 'writer' object"
            ));
        }
    }

    Ok(())
}

fn load_catalog_from_path(path: &Path) -> Result<Option<Value>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!(
            "Nightmare could not inspect catalog {}: {error}",
            path.display()
        )
    })?;

    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Nightmare refuses symlink catalog {}",
            path.display()
        ));
    }

    if !metadata.is_file() {
        return Err(format!(
            "Nightmare catalog must be a regular file: {}",
            path.display()
        ));
    }

    let payload = std::fs::read(path).map_err(|error| {
        format!(
            "Nightmare could not read catalog {}: {error}",
            path.display()
        )
    })?;

    let catalog: Value = serde_json::from_slice(&payload).map_err(|error| {
        format!(
            "Nightmare catalog {} is invalid JSON: {error}",
            path.display()
        )
    })?;

    validate_catalog(&catalog)?;

    Ok(Some(catalog))
}

pub fn load_cached_catalog() -> Result<Option<Value>, String> {
    load_catalog_from_path(Path::new(CACHED_NIGHTMARE_CATALOG_PATH))
}

fn resolve_neebles_os_main_commit() -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(["ls-remote", NEEBLES_OS_REPOSITORY, "refs/heads/main"])
        .output()
        .map_err(|error| {
            format!("Nightmare could not start git while resolving neebles-os main: {error}")
        })?;

    if !output.status.success() {
        return Err(format!(
            "Nightmare could not resolve neebles-os main: {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let commit = stdout
        .split_whitespace()
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Nightmare git ls-remote did not return a neebles-os commit".to_string())?;

    if !commit.chars().all(|value| value.is_ascii_hexdigit()) {
        return Err(format!(
            "Nightmare received invalid neebles-os commit '{}'",
            commit
        ));
    }

    Ok(commit.to_string())
}

fn remote_catalog_url(commit: &str) -> Result<String, String> {
    if commit.is_empty() || !commit.chars().all(|value| value.is_ascii_hexdigit()) {
        return Err("Nightmare remote catalog requires a hexadecimal commit id".to_string());
    }

    Ok(format!(
        "https://raw.githubusercontent.com/krockzs/neebles-os/{commit}/{NIGHTMARE_REMOTE_PATH}"
    ))
}

fn fetch_remote_catalog() -> Result<Value, String> {
    let commit = resolve_neebles_os_main_commit()?;
    let url = remote_catalog_url(&commit)?;

    let output = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "15", &url])
        .output()
        .map_err(|error| {
            format!("Nightmare could not start curl while reading remote catalog: {error}")
        })?;

    if !output.status.success() {
        return Err(format!(
            "Nightmare could not read catalog from neebles-os commit '{}': {}: {}",
            commit,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let catalog: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "Nightmare remote catalog at commit '{}' is invalid JSON: {error}",
            commit
        )
    })?;

    validate_catalog(&catalog)?;

    Ok(catalog)
}

fn publish_catalog_cache(catalog: &Value) -> Result<(), String> {
    validate_catalog(catalog)?;

    let cache_path = Path::new(CACHED_NIGHTMARE_CATALOG_PATH);

    let parent = cache_path.parent().ok_or_else(|| {
        format!(
            "Nightmare cached catalog path has no parent: {}",
            cache_path.display()
        )
    })?;

    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Nightmare could not create catalog cache directory {}: {error}",
            parent.display()
        )
    })?;

    let parent_metadata = std::fs::symlink_metadata(parent).map_err(|error| {
        format!(
            "Nightmare could not inspect catalog cache directory {}: {error}",
            parent.display()
        )
    })?;

    if parent_metadata.file_type().is_symlink() {
        return Err(format!(
            "Nightmare catalog cache directory must not be a symlink: {}",
            parent.display()
        ));
    }

    if !parent_metadata.is_dir() {
        return Err(format!(
            "Nightmare catalog cache parent must be a directory: {}",
            parent.display()
        ));
    }

    let payload = serde_json::to_vec_pretty(catalog)
        .map_err(|error| format!("Nightmare could not serialize validated catalog: {error}"))?;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Nightmare could not generate catalog cache nonce: {error}"))?
        .as_nanos();

    let temporary = parent.join(format!(
        ".insert.nightmare.json.tmp.{}.{}",
        std::process::id(),
        nonce
    ));

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temporary)
        .map_err(|error| {
            format!(
                "Nightmare could not create catalog cache temporary {}: {error}",
                temporary.display()
            )
        })?;

    let cleanup = || {
        let _ = std::fs::remove_file(&temporary);
    };

    if let Err(error) = file.write_all(&payload) {
        cleanup();

        return Err(format!(
            "Nightmare could not write catalog cache temporary {}: {error}",
            temporary.display()
        ));
    }

    if let Err(error) = file.sync_all() {
        cleanup();

        return Err(format!(
            "Nightmare could not sync catalog cache temporary {}: {error}",
            temporary.display()
        ));
    }

    drop(file);

    let directory = std::fs::File::open(parent).map_err(|error| {
        cleanup();

        format!(
            "Nightmare could not open catalog cache directory {}: {error}",
            parent.display()
        )
    })?;

    if let Err(error) = std::fs::rename(&temporary, cache_path) {
        cleanup();

        return Err(format!(
            "Nightmare could not publish catalog cache {}: {error}",
            cache_path.display()
        ));
    }

    directory.sync_all().map_err(|error| {
        format!(
            "Nightmare published catalog cache but could not sync {}: {error}",
            parent.display()
        )
    })?;

    Ok(())
}

pub fn refresh_catalog_cache() -> Result<Value, String> {
    let catalog = fetch_remote_catalog()?;
    publish_catalog_cache(&catalog)?;
    Ok(catalog)
}

pub fn load_catalog() -> Result<Value, String> {
    let mut failures = Vec::new();

    match load_cached_catalog() {
        Ok(Some(catalog)) => return Ok(catalog),
        Ok(None) => {}
        Err(error) => failures.push(error),
    }

    match refresh_catalog_cache() {
        Ok(catalog) => Ok(catalog),
        Err(error) => {
            failures.push(error);

            Err(format!(
                "Nightmare has no usable formula catalog: {}",
                failures.join("; ")
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFormula {
    pub id: String,
    pub reader: Map<String, Value>,
    pub writer: Map<String, Value>,
}

pub fn resolve_formula_from_catalog(
    catalog: &Value,
    formula_id: &str,
) -> Result<ResolvedFormula, String> {
    let formulas = catalog
        .get("formulas")
        .and_then(Value::as_object)
        .ok_or_else(|| "Nightmare catalog must contain a 'formulas' object".to_string())?;

    let formula = formulas
        .get(formula_id)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("Nightmare formula '{formula_id}' does not exist"))?;

    let reader = formula
        .get("reader")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| {
            format!("Nightmare formula '{formula_id}' must contain a 'reader' object")
        })?;

    let writer = formula
        .get("writer")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| {
            format!("Nightmare formula '{formula_id}' must contain a 'writer' object")
        })?;

    Ok(ResolvedFormula {
        id: formula_id.to_string(),
        reader,
        writer,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReaderContract {
    pub record_separator: Option<String>,
    pub field_separator: Option<String>,
    pub trim: bool,
    pub ignore_prefixes: Vec<String>,
}

pub fn resolve_reader_contract(reader: &Map<String, Value>) -> Result<ReaderContract, String> {
    let record_separator = match reader.get("record_separator") {
        Some(value) => Some(
            value
                .as_str()
                .ok_or_else(|| "reader.record_separator must be a String".to_string())?
                .to_string(),
        ),
        None => None,
    };

    let field_separator = match reader.get("field_separator") {
        Some(value) => Some(
            value
                .as_str()
                .ok_or_else(|| "reader.field_separator must be a String".to_string())?
                .to_string(),
        ),
        None => None,
    };

    let trim = match reader.get("trim") {
        Some(value) => value
            .as_bool()
            .ok_or_else(|| "reader.trim must be a Boolean".to_string())?,
        None => false,
    };

    let ignore_prefixes = match reader.get("ignore_prefixes") {
        Some(value) => value
            .as_array()
            .ok_or_else(|| "reader.ignore_prefixes must be an Array".to_string())?
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| "reader.ignore_prefixes entries must be Strings".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };

    Ok(ReaderContract {
        record_separator,
        field_separator,
        trim,
        ignore_prefixes,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalDocument {
    pub fields: Vec<LogicalField>,
}

pub fn read_logical_document(
    raw: &str,
    reader: &ReaderContract,
) -> Result<LogicalDocument, String> {
    let record_separator = reader.record_separator.as_deref().ok_or_else(|| {
        "reader.record_separator is required to extract logical fields".to_string()
    })?;

    let field_separator = reader.field_separator.as_deref().ok_or_else(|| {
        "reader.field_separator is required to extract logical fields".to_string()
    })?;

    if record_separator.is_empty() {
        return Err("reader.record_separator must not be empty".to_string());
    }

    if field_separator.is_empty() {
        return Err("reader.field_separator must not be empty".to_string());
    }

    let mut fields = Vec::new();

    for original_record in raw.split(record_separator) {
        let record = if reader.trim {
            original_record.trim()
        } else {
            original_record
        };

        if record.is_empty() {
            continue;
        }

        if reader
            .ignore_prefixes
            .iter()
            .any(|prefix| record.starts_with(prefix))
        {
            continue;
        }

        let Some((key, value)) = record.split_once(field_separator) else {
            return Err(format!(
                "Nightmare reader could not split record '{}' using field separator '{}'",
                record, field_separator
            ));
        };

        let key = if reader.trim { key.trim() } else { key };
        let value = if reader.trim { value.trim() } else { value };

        if key.is_empty() {
            return Err("Nightmare reader produced an empty field key".to_string());
        }

        fields.push(LogicalField {
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    Ok(LogicalDocument { fields })
}

#[derive(Debug, Clone, PartialEq)]
pub struct WriterContract {
    pub record_separator: String,
    pub field_separator: String,
    pub final_record_separator: bool,
}

pub fn resolve_writer_contract(writer: &Map<String, Value>) -> Result<WriterContract, String> {
    let record_separator = writer
        .get("record_separator")
        .and_then(Value::as_str)
        .ok_or_else(|| "writer.record_separator must be a String".to_string())?
        .to_string();

    let field_separator = writer
        .get("field_separator")
        .and_then(Value::as_str)
        .ok_or_else(|| "writer.field_separator must be a String".to_string())?
        .to_string();

    if record_separator.is_empty() {
        return Err("writer.record_separator must not be empty".to_string());
    }

    if field_separator.is_empty() {
        return Err("writer.field_separator must not be empty".to_string());
    }

    let final_record_separator = match writer.get("final_record_separator") {
        Some(value) => value
            .as_bool()
            .ok_or_else(|| "writer.final_record_separator must be a Boolean".to_string())?,
        None => false,
    };

    Ok(WriterContract {
        record_separator,
        field_separator,
        final_record_separator,
    })
}

pub fn write_logical_document(
    document: &LogicalDocument,
    writer: &WriterContract,
) -> Result<String, String> {
    if writer.record_separator.is_empty() {
        return Err("writer.record_separator must not be empty".to_string());
    }

    if writer.field_separator.is_empty() {
        return Err("writer.field_separator must not be empty".to_string());
    }

    let mut records = Vec::with_capacity(document.fields.len());

    for field in &document.fields {
        if field.key.is_empty() {
            return Err("Nightmare writer cannot render an empty field key".to_string());
        }

        records.push(format!(
            "{}{}{}",
            field.key, writer.field_separator, field.value
        ));
    }

    let mut rendered = records.join(&writer.record_separator);

    if writer.final_record_separator && !rendered.is_empty() {
        rendered.push_str(&writer.record_separator);
    }

    Ok(rendered)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicalOperation {
    Set { key: String, value: String },
    Add { key: String, value: String },
    Remove { key: String },
    Rename { from: String, to: String },
}

pub fn apply_logical_operations(
    document: &LogicalDocument,
    operations: &[LogicalOperation],
) -> Result<LogicalDocument, String> {
    let mut result = document.clone();

    for operation in operations {
        match operation {
            LogicalOperation::Set { key, value } => {
                if key.is_empty() {
                    return Err("Nightmare set operation requires a non-empty key".to_string());
                }

                let mut matched = false;

                for field in &mut result.fields {
                    if field.key == *key {
                        field.value = value.clone();
                        matched = true;
                    }
                }

                if !matched {
                    return Err(format!(
                        "Nightmare set operation could not find field '{}'",
                        key
                    ));
                }
            }

            LogicalOperation::Add { key, value } => {
                if key.is_empty() {
                    return Err("Nightmare add operation requires a non-empty key".to_string());
                }

                result.fields.push(LogicalField {
                    key: key.clone(),
                    value: value.clone(),
                });
            }

            LogicalOperation::Remove { key } => {
                if key.is_empty() {
                    return Err("Nightmare remove operation requires a non-empty key".to_string());
                }

                result.fields.retain(|field| field.key != *key);
            }

            LogicalOperation::Rename { from, to } => {
                if from.is_empty() || to.is_empty() {
                    return Err(
                        "Nightmare rename operation requires non-empty source and destination keys"
                            .to_string(),
                    );
                }

                let mut matched = false;

                for field in &mut result.fields {
                    if field.key == *from {
                        field.key = to.clone();
                        matched = true;
                    }
                }

                if !matched {
                    return Err(format!(
                        "Nightmare rename operation could not find field '{}'",
                        from
                    ));
                }
            }
        }
    }

    Ok(result)
}

pub fn preserve_logical_values(
    source: &LogicalDocument,
    target: &LogicalDocument,
    keys: &[String],
) -> Result<LogicalDocument, String> {
    let mut result = target.clone();

    for key in keys {
        if key.is_empty() {
            return Err("Nightmare preserve requires a non-empty key".to_string());
        }

        let source_values = source
            .fields
            .iter()
            .filter(|field| field.key == *key)
            .map(|field| field.value.clone())
            .collect::<Vec<_>>();

        /*
         * Missing source value means there is nothing to preserve.
         * Target remains authoritative.
         */
        if source_values.is_empty() {
            continue;
        }

        let mut occurrence = 0usize;

        for field in &mut result.fields {
            if field.key != *key {
                continue;
            }

            /*
             * TARGET owns structure and number of occurrences.
             * OLD owns preserved values.
             *
             * If TARGET contains more occurrences than OLD,
             * reuse the final preserved value deterministically.
             */
            let value = source_values
                .get(occurrence)
                .or_else(|| source_values.last())
                .ok_or_else(|| {
                    format!("Nightmare preserve lost source values for key '{}'", key)
                })?;

            field.value = value.clone();
            occurrence += 1;
        }
    }

    Ok(result)
}

pub fn atomic_replace_file(path: &Path, content: &str) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!(
            "Nightmare could not inspect target file {}: {error}",
            path.display()
        )
    })?;

    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Nightmare refuses to replace symlink target {}",
            path.display()
        ));
    }

    if !metadata.is_file() {
        return Err(format!(
            "Nightmare target must be a regular file: {}",
            path.display()
        ));
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("Nightmare target path has no parent: {}", path.display()))?;

    let parent_metadata = std::fs::symlink_metadata(parent).map_err(|error| {
        format!(
            "Nightmare could not inspect target directory {}: {error}",
            parent.display()
        )
    })?;

    if parent_metadata.file_type().is_symlink() {
        return Err(format!(
            "Nightmare refuses to publish through symlink directory {}",
            parent.display()
        ));
    }

    if !parent_metadata.is_dir() {
        return Err(format!(
            "Nightmare target parent must be a directory: {}",
            parent.display()
        ));
    }

    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            format!(
                "Nightmare target filename is not valid UTF-8: {}",
                path.display()
            )
        })?;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Nightmare could not generate temporary file nonce: {error}"))?
        .as_nanos();

    let temporary = parent.join(format!(
        ".{filename}.nightmare.tmp.{}.{}",
        std::process::id(),
        nonce
    ));

    let mut temporary_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temporary)
        .map_err(|error| {
            format!(
                "Nightmare could not create temporary file {}: {error}",
                temporary.display()
            )
        })?;

    let cleanup = || {
        let _ = std::fs::remove_file(&temporary);
    };

    if let Err(error) = temporary_file.write_all(content.as_bytes()) {
        cleanup();
        return Err(format!(
            "Nightmare could not write temporary file {}: {error}",
            temporary.display()
        ));
    }

    let temporary_metadata = match temporary_file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            cleanup();
            return Err(format!(
                "Nightmare could not inspect temporary file {}: {error}",
                temporary.display()
            ));
        }
    };

    if temporary_metadata.uid() != metadata.uid() || temporary_metadata.gid() != metadata.gid() {
        let result =
            unsafe { libc::fchown(temporary_file.as_raw_fd(), metadata.uid(), metadata.gid()) };

        if result != 0 {
            let error = std::io::Error::last_os_error();
            cleanup();
            return Err(format!(
                "Nightmare could not preserve ownership for {}: {error}",
                path.display()
            ));
        }
    }

    let permissions = std::fs::Permissions::from_mode(metadata.mode() & 0o7777);

    if let Err(error) = std::fs::set_permissions(&temporary, permissions) {
        cleanup();
        return Err(format!(
            "Nightmare could not preserve permissions for {}: {error}",
            path.display()
        ));
    }

    if let Err(error) = temporary_file.sync_all() {
        cleanup();
        return Err(format!(
            "Nightmare could not sync temporary file {}: {error}",
            temporary.display()
        ));
    }

    drop(temporary_file);

    let parent_directory = match std::fs::File::open(parent) {
        Ok(directory) => directory,
        Err(error) => {
            cleanup();
            return Err(format!(
                "Nightmare could not open target directory {} for synchronization: {error}",
                parent.display()
            ));
        }
    };

    if let Err(error) = std::fs::rename(&temporary, path) {
        cleanup();
        return Err(format!(
            "Nightmare could not atomically replace {}: {error}",
            path.display()
        ));
    }

    parent_directory.sync_all().map_err(|error| {
        format!(
            "Nightmare replaced {} but could not sync directory {}: {error}",
            path.display(),
            parent.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_replace_publishes_complete_content_and_preserves_metadata() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock must be valid")
            .as_nanos();

        let directory = std::env::temp_dir().join(format!(
            "neebles-nightmare-test-{}-{}",
            std::process::id(),
            unique
        ));

        std::fs::create_dir(&directory).expect("test directory must be created");

        let target = directory.join("config.txt");

        std::fs::write(&target, "OLD=1\n").expect("test target must be created");

        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640))
            .expect("test permissions must be set");

        let before = std::fs::metadata(&target).expect("target metadata must exist");

        atomic_replace_file(&target, "NEW=2\nSECOND=3\n").expect("atomic replacement must succeed");

        let rendered = std::fs::read_to_string(&target).expect("target must be readable");

        let after = std::fs::metadata(&target).expect("target metadata must exist");

        assert_eq!(rendered, "NEW=2\nSECOND=3\n");
        assert_eq!(after.mode() & 0o7777, before.mode() & 0o7777);
        assert_eq!(after.uid(), before.uid());
        assert_eq!(after.gid(), before.gid());

        std::fs::remove_dir_all(&directory).expect("test directory must be removed");
    }

    #[test]
    fn atomic_replace_refuses_symlink_target_without_touching_referent() {
        use std::os::unix::fs::symlink;

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock must be valid")
            .as_nanos();

        let directory = std::env::temp_dir().join(format!(
            "neebles-nightmare-symlink-test-{}-{}",
            std::process::id(),
            unique
        ));

        std::fs::create_dir(&directory).expect("test directory must be created");

        let real = directory.join("real.txt");
        let link = directory.join("link.txt");

        std::fs::write(&real, "ORIGINAL\n").expect("real file must be created");

        symlink(&real, &link).expect("test symlink must be created");

        let error =
            atomic_replace_file(&link, "MUTATED\n").expect_err("symlink targets must be rejected");

        assert!(error.contains("refuses to replace symlink"));

        let original = std::fs::read_to_string(&real).expect("real file must remain");

        assert_eq!(original, "ORIGINAL\n");

        std::fs::remove_dir_all(&directory).expect("test directory must be removed");
    }

    #[test]
    fn preserve_uses_old_values_and_target_structure() {
        let old = LogicalDocument {
            fields: vec![
                LogicalField {
                    key: "USER".to_string(),
                    value: "Pablo".to_string(),
                },
                LogicalField {
                    key: "THEME".to_string(),
                    value: "dark".to_string(),
                },
                LogicalField {
                    key: "LEGACY".to_string(),
                    value: "keep-me-out".to_string(),
                },
            ],
        };

        let target = LogicalDocument {
            fields: vec![
                LogicalField {
                    key: "USER".to_string(),
                    value: "default-user".to_string(),
                },
                LogicalField {
                    key: "THEME".to_string(),
                    value: "light".to_string(),
                },
                LogicalField {
                    key: "TELEMETRY".to_string(),
                    value: "false".to_string(),
                },
            ],
        };

        let result = preserve_logical_values(
            &old,
            &target,
            &[
                "USER".to_string(),
                "THEME".to_string(),
                "LEGACY".to_string(),
            ],
        )
        .expect("preserve must merge old values into target structure");

        assert_eq!(
            result.fields,
            vec![
                LogicalField {
                    key: "USER".to_string(),
                    value: "Pablo".to_string(),
                },
                LogicalField {
                    key: "THEME".to_string(),
                    value: "dark".to_string(),
                },
                LogicalField {
                    key: "TELEMETRY".to_string(),
                    value: "false".to_string(),
                },
            ]
        );
    }

    #[test]
    fn preserve_never_resurrects_removed_target_fields() {
        let old = LogicalDocument {
            fields: vec![LogicalField {
                key: "REMOVED".to_string(),
                value: "old-value".to_string(),
            }],
        };

        let target = LogicalDocument {
            fields: vec![LogicalField {
                key: "CURRENT".to_string(),
                value: "new-value".to_string(),
            }],
        };

        let result = preserve_logical_values(&old, &target, &["REMOVED".to_string()])
            .expect("removed fields must simply remain removed");

        assert_eq!(result, target);
    }

    #[test]
    fn preserve_maps_duplicate_values_by_occurrence() {
        let old = LogicalDocument {
            fields: vec![
                LogicalField {
                    key: "PATH".to_string(),
                    value: "old-a".to_string(),
                },
                LogicalField {
                    key: "PATH".to_string(),
                    value: "old-b".to_string(),
                },
            ],
        };

        let target = LogicalDocument {
            fields: vec![
                LogicalField {
                    key: "PATH".to_string(),
                    value: "new-a".to_string(),
                },
                LogicalField {
                    key: "OTHER".to_string(),
                    value: "untouched".to_string(),
                },
                LogicalField {
                    key: "PATH".to_string(),
                    value: "new-b".to_string(),
                },
                LogicalField {
                    key: "PATH".to_string(),
                    value: "new-c".to_string(),
                },
            ],
        };

        let result = preserve_logical_values(&old, &target, &["PATH".to_string()])
            .expect("duplicate preserved fields must remain deterministic");

        assert_eq!(result.fields[0].value, "old-a");
        assert_eq!(result.fields[1].value, "untouched");
        assert_eq!(result.fields[2].value, "old-b");
        assert_eq!(result.fields[3].value, "old-b");
    }

    #[test]
    fn preserve_does_not_mutate_source_or_target() {
        let old = LogicalDocument {
            fields: vec![LogicalField {
                key: "A".to_string(),
                value: "old".to_string(),
            }],
        };

        let target = LogicalDocument {
            fields: vec![LogicalField {
                key: "A".to_string(),
                value: "new".to_string(),
            }],
        };

        let result = preserve_logical_values(&old, &target, &["A".to_string()])
            .expect("preserve must succeed");

        assert_eq!(result.fields[0].value, "old");
        assert_eq!(old.fields[0].value, "old");
        assert_eq!(target.fields[0].value, "new");
    }

    #[test]
    fn logical_operations_set_all_duplicate_keys() {
        let document = LogicalDocument {
            fields: vec![
                LogicalField {
                    key: "A".to_string(),
                    value: "1".to_string(),
                },
                LogicalField {
                    key: "B".to_string(),
                    value: "2".to_string(),
                },
                LogicalField {
                    key: "A".to_string(),
                    value: "3".to_string(),
                },
            ],
        };

        let result = apply_logical_operations(
            &document,
            &[LogicalOperation::Set {
                key: "A".to_string(),
                value: "changed".to_string(),
            }],
        )
        .expect("set must update every matching field");

        assert_eq!(result.fields[0].value, "changed");
        assert_eq!(result.fields[1].value, "2");
        assert_eq!(result.fields[2].value, "changed");
    }

    #[test]
    fn logical_operations_add_remove_and_rename_preserve_order() {
        let document = LogicalDocument {
            fields: vec![
                LogicalField {
                    key: "FIRST".to_string(),
                    value: "1".to_string(),
                },
                LogicalField {
                    key: "REMOVE".to_string(),
                    value: "gone".to_string(),
                },
                LogicalField {
                    key: "OLD".to_string(),
                    value: "2".to_string(),
                },
            ],
        };

        let operations = vec![
            LogicalOperation::Remove {
                key: "REMOVE".to_string(),
            },
            LogicalOperation::Rename {
                from: "OLD".to_string(),
                to: "NEW".to_string(),
            },
            LogicalOperation::Add {
                key: "LAST".to_string(),
                value: "3".to_string(),
            },
        ];

        let result = apply_logical_operations(&document, &operations)
            .expect("logical operations must apply");

        assert_eq!(
            result.fields,
            vec![
                LogicalField {
                    key: "FIRST".to_string(),
                    value: "1".to_string(),
                },
                LogicalField {
                    key: "NEW".to_string(),
                    value: "2".to_string(),
                },
                LogicalField {
                    key: "LAST".to_string(),
                    value: "3".to_string(),
                },
            ]
        );
    }

    #[test]
    fn logical_operations_are_transactional_in_memory() {
        let document = LogicalDocument {
            fields: vec![LogicalField {
                key: "A".to_string(),
                value: "original".to_string(),
            }],
        };

        let operations = vec![
            LogicalOperation::Set {
                key: "A".to_string(),
                value: "temporary".to_string(),
            },
            LogicalOperation::Rename {
                from: "MISSING".to_string(),
                to: "OTHER".to_string(),
            },
        ];

        let error = apply_logical_operations(&document, &operations)
            .expect_err("failed operation set must not mutate source document");

        assert!(error.contains("could not find field 'MISSING'"));

        assert_eq!(
            document.fields,
            vec![LogicalField {
                key: "A".to_string(),
                value: "original".to_string(),
            }]
        );
    }

    #[test]
    fn remove_missing_key_is_idempotent() {
        let document = LogicalDocument {
            fields: vec![LogicalField {
                key: "A".to_string(),
                value: "1".to_string(),
            }],
        };

        let result = apply_logical_operations(
            &document,
            &[LogicalOperation::Remove {
                key: "MISSING".to_string(),
            }],
        )
        .expect("remove of an absent field must already be converged");

        assert_eq!(result, document);
    }

    #[test]
    fn writer_renders_ordered_logical_fields() {
        let document = LogicalDocument {
            fields: vec![
                LogicalField {
                    key: "FIRST".to_string(),
                    value: "one".to_string(),
                },
                LogicalField {
                    key: "SECOND".to_string(),
                    value: "two=three".to_string(),
                },
                LogicalField {
                    key: "FIRST".to_string(),
                    value: "four".to_string(),
                },
            ],
        };

        let writer = WriterContract {
            record_separator: "\n".to_string(),
            field_separator: "=".to_string(),
            final_record_separator: true,
        };

        let rendered =
            write_logical_document(&document, &writer).expect("writer must render logical fields");

        assert_eq!(rendered, "FIRST=one\nSECOND=two=three\nFIRST=four\n");
    }

    #[test]
    fn writer_contract_resolves_from_formula_object() {
        let writer = serde_json::json!({
            "record_separator": "\n",
            "field_separator": "=",
            "final_record_separator": true
        });

        let resolved = resolve_writer_contract(
            writer
                .as_object()
                .expect("writer test fixture must be an object"),
        )
        .expect("writer contract must resolve");

        assert_eq!(resolved.record_separator, "\n");
        assert_eq!(resolved.field_separator, "=");
        assert!(resolved.final_record_separator);
    }

    #[test]
    fn writer_rejects_empty_separators() {
        let mut writer = Map::new();
        writer.insert(
            "record_separator".to_string(),
            Value::String("".to_string()),
        );
        writer.insert(
            "field_separator".to_string(),
            Value::String("=".to_string()),
        );

        let error =
            resolve_writer_contract(&writer).expect_err("empty record separator must be rejected");

        assert!(error.contains("writer.record_separator must not be empty"));

        writer.insert(
            "record_separator".to_string(),
            Value::String("\n".to_string()),
        );
        writer.insert("field_separator".to_string(), Value::String("".to_string()));

        let error =
            resolve_writer_contract(&writer).expect_err("empty field separator must be rejected");

        assert!(error.contains("writer.field_separator must not be empty"));
    }

    #[test]
    fn reader_writer_round_trip_preserves_logical_fields() {
        let reader = ReaderContract {
            record_separator: Some("\n".to_string()),
            field_separator: Some("=".to_string()),
            trim: true,
            ignore_prefixes: vec!["#".to_string()],
        };

        let logical = read_logical_document("A=1\nB=two=three\nA=4\n", &reader)
            .expect("reader must decode fields");

        let writer = WriterContract {
            record_separator: "\n".to_string(),
            field_separator: "=".to_string(),
            final_record_separator: true,
        };

        let rendered =
            write_logical_document(&logical, &writer).expect("writer must encode fields");

        assert_eq!(rendered, "A=1\nB=two=three\nA=4\n");
    }

    #[test]
    fn reader_extracts_ordered_logical_fields() {
        let reader = ReaderContract {
            record_separator: Some("\n".to_string()),
            field_separator: Some("=".to_string()),
            trim: true,
            ignore_prefixes: vec!["#".to_string()],
        };

        let logical = read_logical_document(
            "FIRST = one\n# ignored\nSECOND = two=three\nFIRST = four\n",
            &reader,
        )
        .expect("reader must decode logical fields");

        assert_eq!(
            logical.fields,
            vec![
                LogicalField {
                    key: "FIRST".to_string(),
                    value: "one".to_string(),
                },
                LogicalField {
                    key: "SECOND".to_string(),
                    value: "two=three".to_string(),
                },
                LogicalField {
                    key: "FIRST".to_string(),
                    value: "four".to_string(),
                },
            ]
        );
    }

    #[test]
    fn reader_preserves_whitespace_when_trim_is_disabled() {
        let reader = ReaderContract {
            record_separator: Some("\n".to_string()),
            field_separator: Some("=".to_string()),
            trim: false,
            ignore_prefixes: Vec::new(),
        };

        let logical = read_logical_document(" key = value ", &reader)
            .expect("reader must preserve declared raw field whitespace");

        assert_eq!(
            logical.fields,
            vec![LogicalField {
                key: " key ".to_string(),
                value: " value ".to_string(),
            }]
        );
    }

    #[test]
    fn reader_rejects_records_that_do_not_match_formula() {
        let reader = ReaderContract {
            record_separator: Some("\n".to_string()),
            field_separator: Some("=".to_string()),
            trim: true,
            ignore_prefixes: Vec::new(),
        };

        let error = read_logical_document("VALID=yes\nthis-is-not-a-field\n", &reader)
            .expect_err("unmatched records must not be silently discarded");

        assert!(error.contains("could not split record"));
    }

    #[test]
    fn reader_requires_non_empty_separators_for_field_extraction() {
        let reader = ReaderContract {
            record_separator: Some("".to_string()),
            field_separator: Some("=".to_string()),
            trim: true,
            ignore_prefixes: Vec::new(),
        };

        let error = read_logical_document("A=1", &reader)
            .expect_err("empty record separator must be rejected");

        assert!(error.contains("record_separator must not be empty"));

        let reader = ReaderContract {
            record_separator: Some("\n".to_string()),
            field_separator: Some("".to_string()),
            trim: true,
            ignore_prefixes: Vec::new(),
        };

        let error = read_logical_document("A=1", &reader)
            .expect_err("empty field separator must be rejected");

        assert!(error.contains("field_separator must not be empty"));
    }

    #[test]
    fn resolves_minimal_reader_contract() {
        let reader = serde_json::json!({
            "record_separator": "\n",
            "field_separator": "=",
            "trim": true,
            "ignore_prefixes": ["#", ";"]
        });

        let resolved = resolve_reader_contract(
            reader
                .as_object()
                .expect("reader test fixture must be an object"),
        )
        .expect("reader contract must resolve");

        assert_eq!(resolved.record_separator.as_deref(), Some("\n"));
        assert_eq!(resolved.field_separator.as_deref(), Some("="));
        assert!(resolved.trim);
        assert_eq!(resolved.ignore_prefixes, vec!["#", ";"]);
    }

    #[test]
    fn reader_contract_defaults_optional_fields() {
        let reader = Map::new();

        let resolved =
            resolve_reader_contract(&reader).expect("empty reader contract must be valid");

        assert_eq!(resolved.record_separator, None);
        assert_eq!(resolved.field_separator, None);
        assert!(!resolved.trim);
        assert!(resolved.ignore_prefixes.is_empty());
    }

    #[test]
    fn reader_contract_rejects_invalid_primitive_types() {
        let mut reader = Map::new();
        reader.insert("trim".to_string(), Value::String("yes".to_string()));

        let error =
            resolve_reader_contract(&reader).expect_err("non-boolean trim must be rejected");

        assert!(error.contains("reader.trim must be a Boolean"));
    }

    #[test]
    fn nightmare_end_to_end_migrates_and_atomically_publishes() {
        let catalog = serde_json::json!({
            "formulas": {
                "nightmare.example.v1": {
                    "reader": {
                        "record_separator": "\n",
                        "field_separator": "=",
                        "trim": true,
                        "ignore_prefixes": ["#"]
                    },
                    "writer": {
                        "record_separator": "\n",
                        "field_separator": "=",
                        "final_record_separator": true
                    }
                }
            }
        });

        validate_catalog(&catalog).expect("end-to-end catalog must validate");

        let formula = resolve_formula_from_catalog(&catalog, "nightmare.example.v1")
            .expect("formula must resolve");

        let reader =
            resolve_reader_contract(&formula.reader).expect("reader contract must resolve");

        let writer =
            resolve_writer_contract(&formula.writer).expect("writer contract must resolve");

        let old = read_logical_document("USER=Pablo\nTHEME=dark\nLEGACY=true\n", &reader)
            .expect("old document must decode");

        let target = read_logical_document("USER=default\nTHEME=light\nTELEMETRY=true\n", &reader)
            .expect("target document must decode");

        let preserved = preserve_logical_values(
            &old,
            &target,
            &[
                "USER".to_string(),
                "THEME".to_string(),
                "LEGACY".to_string(),
            ],
        )
        .expect("preserve must succeed");

        let migrated = apply_logical_operations(
            &preserved,
            &[
                LogicalOperation::Set {
                    key: "TELEMETRY".to_string(),
                    value: "false".to_string(),
                },
                LogicalOperation::Add {
                    key: "NEW_FEATURE".to_string(),
                    value: "enabled".to_string(),
                },
            ],
        )
        .expect("logical operations must succeed");

        let rendered =
            write_logical_document(&migrated, &writer).expect("writer must render final document");

        assert_eq!(
            rendered,
            "USER=Pablo\nTHEME=dark\nTELEMETRY=false\nNEW_FEATURE=enabled\n"
        );

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock must be valid")
            .as_nanos();

        let directory = std::env::temp_dir().join(format!(
            "neebles-nightmare-e2e-{}-{}",
            std::process::id(),
            unique
        ));

        std::fs::create_dir(&directory).expect("end-to-end test directory must be created");

        let destination = directory.join("config.txt");

        std::fs::write(&destination, "USER=default\nTHEME=light\nTELEMETRY=true\n")
            .expect("end-to-end target must exist");

        atomic_replace_file(&destination, &rendered)
            .expect("final document must publish atomically");

        let published =
            std::fs::read_to_string(&destination).expect("published document must be readable");

        assert_eq!(published, rendered);

        std::fs::remove_dir_all(&directory).expect("end-to-end test directory must be removed");
    }

    #[test]
    fn remote_catalog_url_is_pinned_to_commit() {
        let commit = "0123456789abcdef0123456789abcdef01234567";

        let url = remote_catalog_url(commit).expect("valid commit must build URL");

        assert_eq!(
            url,
            "https://raw.githubusercontent.com/krockzs/neebles-os/0123456789abcdef0123456789abcdef01234567/config/nightmare/insert.nightmare.json"
        );

        assert!(!url.contains("/main/"));
    }

    #[test]
    fn remote_catalog_url_rejects_non_hex_revision() {
        let error = remote_catalog_url("main").expect_err("floating branch name must be rejected");

        assert!(error.contains("hexadecimal commit"));
    }

    #[test]
    fn catalog_accepts_empty_formulas_object() {
        let catalog = serde_json::json!({
            "formulas": {}
        });

        validate_catalog(&catalog).expect("empty Nightmare formula catalog must be valid");
    }

    #[test]
    fn catalog_rejects_formula_without_reader_or_writer() {
        let catalog = serde_json::json!({
            "formulas": {
                "nightmare.bad.v1": {
                    "reader": {}
                }
            }
        });

        let error = validate_catalog(&catalog)
            .expect_err("formula without writer must fail catalog validation");

        assert!(error.contains("'writer' object"));
    }

    #[test]
    fn catalog_loader_reads_valid_local_catalog() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock must be valid")
            .as_nanos();

        let directory = std::env::temp_dir().join(format!(
            "neebles-nightmare-catalog-test-{}-{}",
            std::process::id(),
            unique
        ));

        std::fs::create_dir(&directory).expect("test catalog directory must be created");

        let catalog_path = directory.join("insert.nightmare.json");

        std::fs::write(
            &catalog_path,
            r#"{
                "formulas": {
                    "nightmare.example.v1": {
                        "reader": {},
                        "writer": {}
                    }
                }
            }"#,
        )
        .expect("test catalog must be written");

        let catalog = load_catalog_from_path(&catalog_path)
            .expect("local catalog load must succeed")
            .expect("local catalog must exist");

        resolve_formula_from_catalog(&catalog, "nightmare.example.v1")
            .expect("loaded formula must resolve");

        std::fs::remove_dir_all(&directory).expect("test catalog directory must be removed");
    }

    #[test]
    fn catalog_loader_refuses_symlink_catalog() {
        use std::os::unix::fs::symlink;

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock must be valid")
            .as_nanos();

        let directory = std::env::temp_dir().join(format!(
            "neebles-nightmare-catalog-symlink-test-{}-{}",
            std::process::id(),
            unique
        ));

        std::fs::create_dir(&directory).expect("test catalog directory must be created");

        let real = directory.join("real.json");
        let link = directory.join("insert.nightmare.json");

        std::fs::write(&real, r#"{"formulas":{}}"#).expect("real catalog must be written");

        symlink(&real, &link).expect("catalog symlink must be created");

        let error = load_catalog_from_path(&link).expect_err("catalog symlink must be rejected");

        assert!(error.contains("refuses symlink catalog"));

        std::fs::remove_dir_all(&directory).expect("test catalog directory must be removed");
    }

    #[test]
    fn resolves_formula_from_catalog() {
        let catalog = serde_json::json!({
            "formulas": {
                "nightmare.example.v1": {
                    "reader": {
                        "separator": "="
                    },
                    "writer": {
                        "separator": "="
                    }
                }
            }
        });

        let resolved = resolve_formula_from_catalog(&catalog, "nightmare.example.v1")
            .expect("formula must resolve");

        assert_eq!(resolved.id, "nightmare.example.v1");
        assert_eq!(
            resolved.reader.get("separator").and_then(Value::as_str),
            Some("=")
        );
        assert_eq!(
            resolved.writer.get("separator").and_then(Value::as_str),
            Some("=")
        );
    }

    #[test]
    fn rejects_unknown_formula() {
        let catalog = serde_json::json!({
            "formulas": {}
        });

        let error = resolve_formula_from_catalog(&catalog, "nightmare.missing.v1")
            .expect_err("unknown formula must fail");

        assert!(error.contains("does not exist"));
    }

    #[test]
    fn rejects_formula_without_reader_or_writer_objects() {
        let missing_reader = serde_json::json!({
            "formulas": {
                "nightmare.example.v1": {
                    "writer": {}
                }
            }
        });

        let error = resolve_formula_from_catalog(&missing_reader, "nightmare.example.v1")
            .expect_err("reader object is mandatory");

        assert!(error.contains("'reader' object"));

        let missing_writer = serde_json::json!({
            "formulas": {
                "nightmare.example.v1": {
                    "reader": {}
                }
            }
        });

        let error = resolve_formula_from_catalog(&missing_writer, "nightmare.example.v1")
            .expect_err("writer object is mandatory");

        assert!(error.contains("'writer' object"));
    }

    #[test]
    fn persistent_document_accepts_raw_string_content() {
        let mut content = Map::new();
        content.insert(
            "raw".to_string(),
            Value::String("{\"name\":\"Neebles\"}".to_string()),
        );

        let mut formula = Map::new();
        formula.insert(
            "id".to_string(),
            Value::String("nightmare.example.v1".to_string()),
        );

        let document = PersistentDocument {
            descriptor: Map::new(),
            content,
            meta: Map::new(),
            formula,
        };

        document
            .validate()
            .expect("content.raw String must form a valid PersistentDocument");

        assert_eq!(
            document.raw_content().expect("raw content must exist"),
            "{\"name\":\"Neebles\"}"
        );
    }

    #[test]
    fn persistent_document_exposes_formula_id() {
        let mut content = Map::new();
        content.insert("raw".to_string(), Value::String("anything".to_string()));

        let mut formula = Map::new();
        formula.insert(
            "id".to_string(),
            Value::String("nightmare.example.v1".to_string()),
        );

        let document = PersistentDocument {
            descriptor: Map::new(),
            content,
            meta: Map::new(),
            formula,
        };

        assert_eq!(
            document.formula_id().expect("formula id must exist"),
            "nightmare.example.v1"
        );
    }

    #[test]
    fn persistent_document_rejects_missing_formula_id() {
        let mut content = Map::new();
        content.insert("raw".to_string(), Value::String("anything".to_string()));

        let document = PersistentDocument {
            descriptor: Map::new(),
            content,
            meta: Map::new(),
            formula: Map::new(),
        };

        let error = document
            .validate()
            .expect_err("missing formula.id must be rejected");

        assert!(error.contains("formula.id must be a non-empty String"));
    }

    #[test]
    fn persistent_document_rejects_empty_formula_id() {
        let mut content = Map::new();
        content.insert("raw".to_string(), Value::String("anything".to_string()));

        let mut formula = Map::new();
        formula.insert("id".to_string(), Value::String("   ".to_string()));

        let document = PersistentDocument {
            descriptor: Map::new(),
            content,
            meta: Map::new(),
            formula,
        };

        let error = document
            .validate()
            .expect_err("empty formula.id must be rejected");

        assert!(error.contains("formula.id must be a non-empty String"));
    }

    #[test]
    fn persistent_document_rejects_missing_raw_content() {
        let document = PersistentDocument {
            descriptor: Map::new(),
            content: Map::new(),
            meta: Map::new(),
            formula: Map::new(),
        };

        let error = document
            .validate()
            .expect_err("missing content.raw must be rejected");

        assert!(error.contains("content.raw must be a String"));
    }

    #[test]
    fn persistent_document_rejects_non_string_raw_content() {
        let mut content = Map::new();
        content.insert("raw".to_string(), Value::Bool(true));

        let document = PersistentDocument {
            descriptor: Map::new(),
            content,
            meta: Map::new(),
            formula: Map::new(),
        };

        let error = document
            .validate()
            .expect_err("non-string content.raw must be rejected");

        assert!(error.contains("content.raw must be a String"));
    }
}

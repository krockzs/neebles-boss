use super::distribution;
use super::LocalInstallerRequest;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const CURRENT_ARCHITECTURE: &str = "amd64";
const CURRENT_DICTIONARY_SCHEMA: u64 = 4;
const NEEBLES_OS_REPOSITORY: &str = "https://github.com/krockzs/neebles-os.git";
const BUNDLED_DICTIONARY_PATH: &str =
    "/opt/neebles/client/config/installers/instaladores_amd64.json";
const CACHED_DICTIONARY_PATH: &str = "/opt/neebles/shared/cache/installers/instaladores_amd64.json";

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedOperation {
    pub architecture: String,
    pub detected_distribution: String,
    pub distribution: String,
    pub distribution_match: String,
    pub distribution_candidates: Vec<String>,
    pub installer: String,
    pub operation: String,
    pub command: String,
    pub args: Vec<String>,
    pub root_mode: String,
    pub expect: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DictionaryAudit {
    pub ok: bool,
    pub architecture: String,
    pub schema: Option<u64>,
    pub version: Option<u64>,
    pub distributions: usize,
    pub installers: usize,
    pub operations: usize,
    pub flow_items: usize,
    pub root_modes: BTreeMap<String, usize>,
    pub flow_kinds: BTreeMap<String, usize>,
    pub variable_sources: BTreeMap<String, usize>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn resolve(request: &LocalInstallerRequest) -> Result<ResolvedOperation, String> {
    let dictionary = fetch_dictionary()?;

    validate_architecture(&dictionary)?;

    let distributions = dictionary
        .get("distributions")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "installer dictionary does not contain a valid 'distributions' object".to_string()
        })?;

    let distribution_resolution = distribution::resolve(distributions)?;

    let selected_distribution = &distribution_resolution.distribution;

    let distribution = distributions.get(selected_distribution).ok_or_else(|| {
        format!(
            "resolved distribution '{}' does not exist in installer dictionary",
            selected_distribution
        )
    })?;

    let installers = distribution
        .get("installers")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "distribution '{}' does not contain a valid installers object",
                selected_distribution
            )
        })?;

    let installer = installers.get(&request.installer).ok_or_else(|| {
        format!(
            "installer '{}' does not exist for distribution '{}'",
            request.installer, selected_distribution
        )
    })?;

    let operations = installer
        .get("operations")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "installer '{}' does not contain a valid operations object",
                request.installer
            )
        })?;

    let operation = operations.get(&request.operation).ok_or_else(|| {
        format!(
            "operation '{}' does not exist for installer '{}'",
            request.operation, request.installer
        )
    })?;

    let command = operation
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "operation '{}:{}' does not define a command",
                request.installer, request.operation
            )
        })?
        .to_string();

    let flow = operation
        .get("flow")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "operation '{}:{}' does not define a flow array",
                request.installer, request.operation
            )
        })?;

    let mut args = Vec::new();

    for item in flow {
        resolve_flow_item(item, request, &mut args)?;
    }

    let root_mode = operation
        .get("root_mode")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "operation '{}:{}' does not define root_mode",
                request.installer, request.operation
            )
        })?
        .to_string();

    match root_mode.as_str() {
        "required" | "not_required" | "contextual" => {}
        other => {
            return Err(format!(
                "operation '{}:{}' defines unsupported root_mode '{}'",
                request.installer, request.operation, other
            ));
        }
    }

    let expect = operation.get("expect").cloned();
    validate_expect(
        &format!("{}:{}", request.installer, request.operation),
        expect.as_ref(),
    )?;

    Ok(ResolvedOperation {
        architecture: CURRENT_ARCHITECTURE.to_string(),
        detected_distribution: distribution_resolution.detected.id.clone(),
        distribution: selected_distribution.clone(),
        distribution_match: distribution_resolution.matched_by.clone(),
        distribution_candidates: distribution_resolution.candidates.clone(),
        installer: request.installer.clone(),
        operation: request.operation.clone(),
        command,
        args,
        root_mode,
        expect,
    })
}

pub fn audit_dictionary() -> Result<DictionaryAudit, String> {
    let dictionary = fetch_dictionary()?;
    audit_dictionary_value(&dictionary)
}

fn audit_dictionary_value(dictionary: &Value) -> Result<DictionaryAudit, String> {
    let architecture = dictionary
        .get("architecture")
        .and_then(Value::as_str)
        .unwrap_or("<missing>")
        .to_string();

    let schema = dictionary.get("schema").and_then(Value::as_u64);
    let version = dictionary.get("version").and_then(Value::as_u64);

    let mut audit = DictionaryAudit {
        ok: true,
        architecture: architecture.clone(),
        schema,
        version,
        distributions: 0,
        installers: 0,
        operations: 0,
        flow_items: 0,
        root_modes: BTreeMap::new(),
        flow_kinds: BTreeMap::new(),
        variable_sources: BTreeMap::new(),
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    if architecture != CURRENT_ARCHITECTURE {
        audit.errors.push(format!(
            "dictionary architecture '{}' does not match Boss architecture '{}'",
            architecture, CURRENT_ARCHITECTURE
        ));
    }

    match schema {
        Some(schema) if schema == CURRENT_DICTIONARY_SCHEMA => {}
        Some(schema) => audit.errors.push(format!(
            "dictionary schema '{}' is incompatible with Boss schema '{}'",
            schema, CURRENT_DICTIONARY_SCHEMA
        )),
        None => audit
            .errors
            .push("installer dictionary does not declare a valid schema".to_string()),
    }

    let Some(distributions) = dictionary.get("distributions").and_then(Value::as_object) else {
        audit.errors.push(
            "installer dictionary does not contain a valid 'distributions' object".to_string(),
        );
        audit.ok = false;
        return Ok(audit);
    };

    let allowed_root_modes: BTreeSet<&str> = ["required", "not_required", "contextual"]
        .into_iter()
        .collect();

    for (distribution_id, distribution) in distributions {
        audit.distributions += 1;

        let Some(installers) = distribution.get("installers").and_then(Value::as_object) else {
            audit.errors.push(format!(
                "distribution '{}' does not contain a valid installers object",
                distribution_id
            ));
            continue;
        };

        for (installer_id, installer) in installers {
            audit.installers += 1;

            let Some(operations) = installer.get("operations").and_then(Value::as_object) else {
                audit.errors.push(format!(
                    "{}:{} does not contain a valid operations object",
                    distribution_id, installer_id
                ));
                continue;
            };

            if operations.is_empty() {
                audit.errors.push(format!(
                    "{}:{} contains zero operations",
                    distribution_id, installer_id
                ));
            }

            for (operation_id, operation) in operations {
                audit.operations += 1;
                let prefix = format!("{}:{}:{}", distribution_id, installer_id, operation_id);

                let Some(operation_object) = operation.as_object() else {
                    audit
                        .errors
                        .push(format!("{} operation must be a JSON object", prefix));
                    continue;
                };

                match operation_object.get("command").and_then(Value::as_str) {
                    Some(command) if !command.is_empty() => {}
                    _ => audit
                        .errors
                        .push(format!("{} does not define a valid command", prefix)),
                }

                match operation_object.get("root_mode").and_then(Value::as_str) {
                    Some(root_mode) if allowed_root_modes.contains(root_mode) => {
                        *audit.root_modes.entry(root_mode.to_string()).or_insert(0) += 1;
                    }
                    Some(root_mode) => audit.errors.push(format!(
                        "{} defines unsupported root_mode '{}'",
                        prefix, root_mode
                    )),
                    None => audit
                        .errors
                        .push(format!("{} does not define root_mode", prefix)),
                }

                let Some(flow) = operation_object.get("flow").and_then(Value::as_array) else {
                    audit
                        .errors
                        .push(format!("{} does not define a flow array", prefix));
                    continue;
                };

                for (index, item) in flow.iter().enumerate() {
                    audit.flow_items += 1;
                    audit_flow_item(&prefix, index, item, &mut audit);
                }

                if let Err(error) = validate_expect(&prefix, operation_object.get("expect")) {
                    audit.errors.push(error);
                }

                if let Some(variables) =
                    operation_object.get("variables").and_then(Value::as_object)
                {
                    for (name, spec) in variables {
                        let expected = spec
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("<missing>");
                        if !matches!(expected, "scalar" | "array" | "scalar_or_array") {
                            audit.errors.push(format!(
                                "{} variable '{}' has unsupported type '{}'",
                                prefix, name, expected
                            ));
                        }
                    }
                }
            }
        }
    }

    if let Some(stats) = dictionary.get("dictionary_stats") {
        if let Some(expected) = stats.get("operations").and_then(Value::as_u64) {
            if expected as usize != audit.operations {
                audit.warnings.push(format!(
                    "dictionary_stats.operations={} but audit counted {}",
                    expected, audit.operations
                ));
            }
        }
    }

    audit.ok = audit.errors.is_empty();
    Ok(audit)
}

fn audit_flow_item(prefix: &str, index: usize, item: &Value, audit: &mut DictionaryAudit) {
    let Some(object) = item.as_object() else {
        audit
            .errors
            .push(format!("{} flow[{}] is not an object", prefix, index));
        return;
    };

    let Some(kind) = object.get("kind").and_then(Value::as_str) else {
        audit
            .errors
            .push(format!("{} flow[{}] does not define kind", prefix, index));
        return;
    };

    *audit.flow_kinds.entry(kind.to_string()).or_insert(0) += 1;

    if let Some(source) = object.get("source").and_then(Value::as_str) {
        *audit
            .variable_sources
            .entry(source.to_string())
            .or_insert(0) += 1;
    }

    let result = validate_flow_shape(object, kind);
    if let Err(error) = result {
        audit
            .errors
            .push(format!("{} flow[{}]: {}", prefix, index, error));
    }
}

fn validate_flow_shape(object: &Map<String, Value>, kind: &str) -> Result<(), String> {
    match kind {
        "subcommand" | "flag" | "literal" | "separator" => {
            let value = object
                .get("value")
                .ok_or_else(|| format!("flow kind '{}' requires value", kind))?;
            scalar_to_string(value, kind).map(|_| ())
        }
        "operand" => validate_value_or_source_shape(object, kind),
        "operand_list" => {
            object
                .get("source")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "flow kind 'operand_list' requires source".to_string())?;
            Ok(())
        }
        "option" => {
            object
                .get("name")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "flow kind 'option' requires name".to_string())?;
            validate_value_or_source_shape(object, kind)
        }
        other => Err(format!("unsupported flow kind '{}'", other)),
    }
}

fn validate_value_or_source_shape(object: &Map<String, Value>, kind: &str) -> Result<(), String> {
    let has_value = object.contains_key("value");
    let has_source = object
        .get("source")
        .and_then(Value::as_str)
        .map(|value| !value.is_empty())
        .unwrap_or(false);

    if has_value == has_source {
        return Err(format!(
            "flow kind '{}' requires exactly one of value/source",
            kind
        ));
    }

    if let Some(value) = object.get("value") {
        scalar_to_string(value, kind)?;
    }

    Ok(())
}

fn validate_architecture(dictionary: &Value) -> Result<(), String> {
    let architecture = dictionary
        .get("architecture")
        .and_then(Value::as_str)
        .ok_or_else(|| "installer dictionary does not declare architecture".to_string())?;

    if architecture != CURRENT_ARCHITECTURE {
        return Err(format!(
            "installer dictionary architecture '{}' does not match Boss architecture '{}'",
            architecture, CURRENT_ARCHITECTURE
        ));
    }

    Ok(())
}

fn resolve_flow_item(
    item: &Value,
    request: &LocalInstallerRequest,
    args: &mut Vec<String>,
) -> Result<(), String> {
    let object = item.as_object().ok_or_else(|| {
        format!(
            "operation '{}:{}' contains a non-object flow item",
            request.installer, request.operation
        )
    })?;

    let kind = object.get("kind").and_then(Value::as_str).ok_or_else(|| {
        format!(
            "operation '{}:{}' contains a flow item without kind",
            request.installer, request.operation
        )
    })?;

    validate_flow_shape(object, kind)?;

    match kind {
        "subcommand" | "flag" | "literal" | "separator" => {
            let value = object.get("value").expect("validated value");
            args.push(scalar_to_string(value, kind)?);
        }
        "operand" => {
            let value = resolve_value_or_source(object, request, kind)?;
            args.push(scalar_to_string(value, kind)?);
        }
        "operand_list" => {
            let source = object
                .get("source")
                .and_then(Value::as_str)
                .expect("validated source");

            let value = request.variables.get(source).ok_or_else(|| {
                format!(
                    "missing variable '{}' for installer '{}', operation '{}'",
                    source, request.installer, request.operation
                )
            })?;

            let values = value.as_array().ok_or_else(|| {
                format!("variable '{}' must be an array for operand_list", source)
            })?;

            if values.is_empty() {
                return Err(format!(
                    "variable '{}' must contain at least one value for operand_list",
                    source
                ));
            }

            for value in values {
                args.push(scalar_to_string(value, kind)?);
            }
        }
        "option" => {
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .expect("validated option name");

            let value = resolve_value_or_source(object, request, kind)?;

            args.push(name.to_string());
            args.push(scalar_to_string(value, kind)?);
        }
        _ => unreachable!("validated flow kind"),
    }

    Ok(())
}

fn resolve_value_or_source<'a>(
    object: &'a Map<String, Value>,
    request: &'a LocalInstallerRequest,
    kind: &str,
) -> Result<&'a Value, String> {
    if let Some(value) = object.get("value") {
        return Ok(value);
    }

    let source = object
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("flow kind '{}' requires value or source", kind))?;

    request.variables.get(source).ok_or_else(|| {
        format!(
            "missing variable '{}' for installer '{}', operation '{}'",
            source, request.installer, request.operation
        )
    })
}

fn scalar_to_string(value: &Value, context: &str) -> Result<String, String> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        _ => Err(format!("{} requires a scalar JSON value", context)),
    }
}

fn validate_expect(context: &str, expect: Option<&Value>) -> Result<(), String> {
    let Some(expect) = expect else {
        return Ok(());
    };

    let object = expect
        .as_object()
        .ok_or_else(|| format!("{} expect field must be a JSON object", context))?;

    for key in object.keys() {
        if key != "stdout_equals" {
            return Err(format!(
                "{} uses unsupported expectation key '{}'",
                context, key
            ));
        }
    }

    if let Some(value) = object.get("stdout_equals") {
        if !value.is_string() {
            return Err(format!("{} expect.stdout_equals must be a string", context));
        }
    }

    Ok(())
}

fn fetch_dictionary() -> Result<Value, String> {
    fetch_dictionary_with(
        Path::new(CACHED_DICTIONARY_PATH),
        Path::new(BUNDLED_DICTIONARY_PATH),
        fetch_remote_dictionary,
    )
}

fn fetch_dictionary_with<F>(
    cached_path: &Path,
    bundled_path: &Path,
    fetch_remote: F,
) -> Result<Value, String>
where
    F: FnOnce() -> Result<Value, String>,
{
    let mut failures = Vec::new();

    for (label, path) in [("cached", cached_path), ("bundled", bundled_path)] {
        match load_local_dictionary(path, label) {
            Ok(Some(dictionary)) => return Ok(dictionary),
            Ok(None) => {}
            Err(error) => failures.push(error),
        }
    }

    match fetch_remote() {
        Ok(dictionary) => Ok(dictionary),
        Err(error) => {
            failures.push(error);

            Err(format!(
                "no usable installer dictionary is available: {}",
                failures.join("; ")
            ))
        }
    }
}

fn load_local_dictionary(path: &Path, label: &str) -> Result<Option<Value>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let payload = fs::read(path).map_err(|error| {
        format!(
            "could not read {label} installer dictionary {}: {error}",
            path.display()
        )
    })?;

    let dictionary: Value = serde_json::from_slice(&payload).map_err(|error| {
        format!(
            "invalid {label} installer dictionary {}: {error}",
            path.display()
        )
    })?;

    let audit = audit_dictionary_value(&dictionary)?;

    if !audit.ok {
        return Err(format!(
            "invalid {label} installer dictionary {}: {}",
            path.display(),
            audit.errors.join("; ")
        ));
    }

    Ok(Some(dictionary))
}

pub fn refresh_dictionary_cache() -> Result<DictionaryAudit, String> {
    let dictionary = fetch_remote_dictionary()?;
    let audit = audit_dictionary_value(&dictionary)?;

    if !audit.ok {
        return Err(format!(
            "remote installer dictionary failed validation: {}",
            audit.errors.join("; ")
        ));
    }

    let cache_path = Path::new(CACHED_DICTIONARY_PATH);
    let parent = cache_path.parent().ok_or_else(|| {
        format!(
            "cached installer dictionary path has no parent: {}",
            cache_path.display()
        )
    })?;

    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "could not create installer dictionary cache directory {}: {error}",
            parent.display()
        )
    })?;

    let parent_metadata = fs::symlink_metadata(parent).map_err(|error| {
        format!(
            "could not inspect installer dictionary cache directory {}: {error}",
            parent.display()
        )
    })?;

    if parent_metadata.file_type().is_symlink() {
        return Err(format!(
            "installer dictionary cache directory must not be a symlink: {}",
            parent.display()
        ));
    }

    let payload = serde_json::to_vec_pretty(&dictionary)
        .map_err(|error| format!("could not serialize validated installer dictionary: {error}"))?;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("could not generate cache temporary timestamp: {error}"))?
        .as_nanos();

    let temporary = parent.join(format!(
        ".instaladores_amd64.json.tmp.{}.{}",
        std::process::id(),
        nonce
    ));

    let mut temporary_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temporary)
        .map_err(|error| {
            format!(
                "could not create temporary installer dictionary cache {}: {error}",
                temporary.display()
            )
        })?;

    if let Err(error) = temporary_file.write_all(&payload) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "could not write temporary installer dictionary cache {}: {error}",
            temporary.display()
        ));
    }

    drop(temporary_file);

    if let Err(error) = fs::rename(&temporary, cache_path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "could not publish installer dictionary cache {}: {error}",
            cache_path.display()
        ));
    }

    Ok(audit)
}

fn fetch_remote_dictionary() -> Result<Value, String> {
    let ref_output = Command::new("git")
        .args(["ls-remote", NEEBLES_OS_REPOSITORY, "refs/heads/main"])
        .output()
        .map_err(|error| {
            format!("could not start git while resolving neebles-os main ref: {error}")
        })?;

    if !ref_output.status.success() {
        return Err(format!(
            "could not resolve neebles-os main ref with git: {}: {}",
            ref_output.status,
            String::from_utf8_lossy(&ref_output.stderr).trim()
        ));
    }

    let ref_stdout = String::from_utf8_lossy(&ref_output.stdout);
    let commit_sha = ref_stdout
        .split_whitespace()
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "git ls-remote did not return a commit SHA for neebles-os main".to_string()
        })?;

    if !commit_sha.chars().all(|value| value.is_ascii_hexdigit()) {
        return Err(format!(
            "git ls-remote returned an invalid commit SHA '{}'",
            commit_sha
        ));
    }

    let dictionary_url = format!(
        "https://raw.githubusercontent.com/krockzs/neebles-os/{}/config/installers/instaladores_amd64.json",
        commit_sha
    );

    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", "15", &dictionary_url])
        .output()
        .map_err(|error| {
            format!("could not start curl while reading installer dictionary: {error}")
        })?;

    if !output.status.success() {
        return Err(format!(
            "could not read installer dictionary from GitHub commit '{}': {}: {}",
            commit_sha,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "invalid remote installer dictionary at commit '{}': {error}",
            commit_sha
        )
    })
}

#[cfg(test)]
mod certification_tests {
    use super::*;
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_root(label: &str) -> std::path::PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);

        std::env::temp_dir().join(format!(
            "neebles-dictionary-cert-{}-{}-{}",
            label,
            std::process::id(),
            id
        ))
    }

    fn valid_dictionary(marker: &str) -> Value {
        let mut dictionary: Value = serde_json::from_str(include_str!(
            "../../client/config/installers/instaladores_amd64.json"
        ))
        .expect("bundled repository dictionary must parse");

        dictionary
            .as_object_mut()
            .expect("dictionary must be object")
            .insert(
                "_certification_marker".to_string(),
                Value::String(marker.to_string()),
            );

        dictionary
    }

    fn write_dictionary(path: &Path, dictionary: &Value) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        std::fs::write(path, serde_json::to_vec_pretty(dictionary).unwrap()).unwrap();
    }

    #[test]
    fn certification_dictionary_prefers_cache_without_remote() {
        let root = temp_root("cache");
        let cached = root.join("cached.json");
        let bundled = root.join("bundled.json");

        write_dictionary(&cached, &valid_dictionary("cached"));
        write_dictionary(&bundled, &valid_dictionary("bundled"));

        let remote_called = Cell::new(false);

        let resolved = fetch_dictionary_with(&cached, &bundled, || {
            remote_called.set(true);
            Ok(valid_dictionary("remote"))
        })
        .unwrap();

        assert_eq!(resolved["_certification_marker"], "cached");
        assert!(!remote_called.get());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn certification_dictionary_falls_back_to_bundled_without_remote() {
        let root = temp_root("bundled");
        let cached = root.join("missing-cache.json");
        let bundled = root.join("bundled.json");

        write_dictionary(&bundled, &valid_dictionary("bundled"));

        let remote_called = Cell::new(false);

        let resolved = fetch_dictionary_with(&cached, &bundled, || {
            remote_called.set(true);
            Ok(valid_dictionary("remote"))
        })
        .unwrap();

        assert_eq!(resolved["_certification_marker"], "bundled");
        assert!(!remote_called.get());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn certification_dictionary_uses_remote_only_when_local_sources_are_unusable() {
        let root = temp_root("remote");
        let cached = root.join("cached.json");
        let bundled = root.join("bundled.json");

        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&cached, b"{broken-json").unwrap();
        std::fs::write(&bundled, b"{also-broken").unwrap();

        let remote_called = Cell::new(false);

        let resolved = fetch_dictionary_with(&cached, &bundled, || {
            remote_called.set(true);
            Ok(valid_dictionary("remote"))
        })
        .unwrap();

        assert_eq!(resolved["_certification_marker"], "remote");
        assert!(remote_called.get());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn certification_dictionary_survives_remote_failure_with_valid_local_data() {
        let root = temp_root("offline");
        let cached = root.join("cached.json");
        let bundled = root.join("bundled.json");

        write_dictionary(&bundled, &valid_dictionary("offline-bundled"));

        let resolved =
            fetch_dictionary_with(&cached, &bundled, || Err("network unavailable".to_string()))
                .expect("valid local recovery data must make remote failure irrelevant");

        assert_eq!(resolved["_certification_marker"], "offline-bundled");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn certification_dictionary_reports_failure_when_no_source_is_usable() {
        let root = temp_root("none");
        let cached = root.join("cached.json");
        let bundled = root.join("bundled.json");

        let error =
            fetch_dictionary_with(&cached, &bundled, || Err("network unavailable".to_string()))
                .expect_err("all unavailable sources must fail");

        assert!(error.contains("no usable installer dictionary"));
        assert!(error.contains("network unavailable"));

        let _ = std::fs::remove_dir_all(root);
    }
}

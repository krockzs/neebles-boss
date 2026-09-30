use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct TargetInspection {
    pub declared_target: String,
    pub resolved_target: String,
    pub exists: bool,
    pub is_file: bool,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DomesticTestReport {
    pub category: String,
    pub value: String,
    pub root: String,
    pub targets: Vec<TargetInspection>,
    pub ok: bool,
    pub missing: Vec<String>,
}

pub fn load_categories(contract_path: &Path) -> Result<BTreeSet<String>, String> {
    let raw = fs::read_to_string(contract_path).map_err(|error| {
        format!(
            "could not read domestic contract {}: {error}",
            contract_path.display()
        )
    })?;

    let contract: Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid domestic contract JSON {}: {error}",
            contract_path.display()
        )
    })?;

    if contract.get("schema").and_then(Value::as_str) != Some("1") {
        return Err("domestic contract schema must be String 1".to_string());
    }

    if contract.get("name").and_then(Value::as_str) != Some("domesticacion-elfica") {
        return Err("invalid domestic contract name".to_string());
    }

    if contract.get("value_type").and_then(Value::as_str) != Some("String") {
        return Err("domestic contract value_type must be String".to_string());
    }

    if contract.get("unknown_category").and_then(Value::as_str) != Some("error") {
        return Err("unknown domestic categories must fail".to_string());
    }

    let categories = contract
        .get("categories")
        .and_then(Value::as_object)
        .ok_or_else(|| "domestic categories must be an object".to_string())?;

    if categories.is_empty() {
        return Err("domestic categories cannot be empty".to_string());
    }

    let mut result = BTreeSet::<String>::new();

    for (name, value_type) in categories {
        if name.trim().is_empty() {
            return Err("domestic category name cannot be empty".to_string());
        }

        if value_type.as_str() != Some("String") {
            return Err(format!("domestic category {name} must receive String"));
        }

        result.insert(name.clone());
    }

    Ok(result)
}

fn validate_relative_target(target: &Path) -> Result<(), String> {
    if target.as_os_str().is_empty() {
        return Err("every target must be non-empty".to_string());
    }

    if target.is_absolute() {
        return Err(format!(
            "domestic target must be relative to root: {}",
            target.display()
        ));
    }

    for component in target.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(format!(
                "domestic target cannot escape root: {}",
                target.display()
            ));
        }
    }

    Ok(())
}

fn inspect_target(root: &Path, declared: &Path) -> TargetInspection {
    let resolved = root.join(declared);

    let is_symlink = fs::symlink_metadata(&resolved)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);

    let symlink_target = if is_symlink {
        fs::read_link(&resolved)
            .ok()
            .map(|target| target.display().to_string())
    } else {
        None
    };

    TargetInspection {
        declared_target: declared.display().to_string(),

        resolved_target: resolved.display().to_string(),

        exists: resolved.exists(),

        is_file: resolved.is_file(),

        is_dir: resolved.is_dir(),

        is_symlink,

        symlink_target,
    }
}

pub fn test_category(
    contract_path: &Path,
    root: &Path,
    category: &str,
    value: &str,
    targets: &[PathBuf],
) -> Result<DomesticTestReport, String> {
    if category.trim().is_empty() {
        return Err("category must be non-empty String".to_string());
    }

    if value.trim().is_empty() {
        return Err("value must be non-empty String".to_string());
    }

    if targets.is_empty() {
        return Err("targets cannot be empty".to_string());
    }

    if !root.is_absolute() {
        return Err("domestic root must be absolute".to_string());
    }

    if !root.is_dir() {
        return Err(format!(
            "domestic root is not a directory: {}",
            root.display()
        ));
    }

    for target in targets {
        validate_relative_target(target)?;
    }

    let categories = load_categories(contract_path)?;

    if !categories.contains(category) {
        return Err(format!("unknown domestic category: {category}"));
    }

    let inspections = targets
        .iter()
        .map(|target| inspect_target(root, target))
        .collect::<Vec<_>>();

    let missing = inspections
        .iter()
        .filter(|inspection| !inspection.exists)
        .map(|inspection| inspection.resolved_target.clone())
        .collect::<Vec<_>>();

    Ok(DomesticTestReport {
        category: category.to_string(),

        value: value.to_string(),

        root: root.display().to_string(),

        targets: inspections,

        ok: missing.is_empty(),

        missing,
    })
}

pub fn validate_tester_definition(path: &Path, expected_category: &str) -> Result<usize, String> {
    let raw = fs::read_to_string(path).map_err(|error| {
        format!(
            "could not read tester definition {}: {error}",
            path.display()
        )
    })?;

    let definition: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid tester definition JSON {}: {error}", path.display()))?;

    let object = definition
        .as_object()
        .ok_or_else(|| "tester definition must be an object".to_string())?;

    let allowed_root = BTreeSet::from([
        "schema",
        "category",
        "input",
        "tester",
        "technology_specific",
        "spells",
    ]);

    let actual_root = object.keys().map(String::as_str).collect::<BTreeSet<_>>();

    if actual_root != allowed_root {
        return Err(format!(
            "tester definition {} contains unexpected root fields",
            path.display()
        ));
    }

    if object.get("schema").and_then(Value::as_str) != Some("3") {
        return Err("tester definition schema must be String 3".to_string());
    }

    if object.get("category").and_then(Value::as_str) != Some(expected_category) {
        return Err(format!(
            "tester category does not match directory: {expected_category}"
        ));
    }

    if object.get("tester").and_then(Value::as_str) != Some("matrix") {
        return Err("tester implementation must be matrix".to_string());
    }

    if object.get("technology_specific").and_then(Value::as_bool) != Some(false) {
        return Err("tester must remain technology independent".to_string());
    }

    let spells = object
        .get("spells")
        .and_then(Value::as_array)
        .ok_or_else(|| "tester spells must be an array".to_string())?;

    if spells.is_empty() {
        return Err("tester must declare at least one spell".to_string());
    }

    let mut total_cases = 0usize;

    for spell_value in spells {
        let spell = spell_value
            .as_object()
            .ok_or_else(|| "tester spell must be an object".to_string())?;

        let spell_name = spell
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "tester spell name must be String".to_string())?;

        if spell_name.trim().is_empty() {
            return Err("tester spell name cannot be empty".to_string());
        }

        let required_spell = BTreeSet::from(["name", "options", "cases"]);

        let actual_spell = spell.keys().map(String::as_str).collect::<BTreeSet<_>>();

        if !required_spell.is_subset(&actual_spell) {
            return Err(format!(
                "tester spell {spell_name} is missing required fields"
            ));
        }

        let options = spell
            .get("options")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("tester spell {spell_name} options must be an object"))?;

        if options.is_empty() {
            return Err(format!("tester spell {spell_name} must declare options"));
        }

        let mut known_options = std::collections::BTreeMap::<String, BTreeSet<String>>::new();

        for (option_name, values) in options {
            if option_name.trim().is_empty() {
                return Err(format!(
                    "tester spell {spell_name} contains empty option name"
                ));
            }

            let values = values.as_array().ok_or_else(|| {
                format!("tester spell {spell_name} option {option_name} must be array")
            })?;

            if values.is_empty() {
                return Err(format!(
                    "tester spell {spell_name} option {option_name} cannot be empty"
                ));
            }

            let mut known = BTreeSet::<String>::new();

            for value in values {
                let value = value.as_str().ok_or_else(|| {
                    format!("tester spell {spell_name} option {option_name} values must be String")
                })?;

                if value.trim().is_empty() {
                    return Err(format!(
                        "tester spell {spell_name} option {option_name} contains empty value"
                    ));
                }

                if !known.insert(value.to_string()) {
                    return Err(
                        format!(
                            "tester spell {spell_name} option {option_name} contains duplicate value {value}"
                        )
                    );
                }
            }

            known_options.insert(option_name.clone(), known);
        }

        let cases = spell
            .get("cases")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("tester spell {spell_name} cases must be array"))?;

        if cases.is_empty() {
            return Err(format!("tester spell {spell_name} must declare cases"));
        }

        let mut case_ids = BTreeSet::<String>::new();

        for case_value in cases {
            let case = case_value
                .as_object()
                .ok_or_else(|| format!("tester spell {spell_name} case must be object"))?;

            let allowed_case = BTreeSet::from(["id", "select", "expect", "assert"]);

            let actual_case = case.keys().map(String::as_str).collect::<BTreeSet<_>>();

            if !actual_case.is_subset(&allowed_case) {
                return Err(format!(
                    "tester spell {spell_name} case contains unexpected fields"
                ));
            }

            for required in ["id", "select", "expect"] {
                if !actual_case.contains(required) {
                    return Err(format!(
                        "tester spell {spell_name} case is missing required field {required}"
                    ));
                }
            }

            let id = case
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("tester spell {spell_name} case id must be String"))?;

            if id.trim().is_empty() {
                return Err(format!("tester spell {spell_name} case id cannot be empty"));
            }

            if !case_ids.insert(id.to_string()) {
                return Err(format!(
                    "tester spell {spell_name} contains duplicate case id {id}"
                ));
            }

            let expect = case.get("expect").and_then(Value::as_str).ok_or_else(|| {
                format!("tester spell {spell_name} case {id} expect must be String")
            })?;

            if expect.trim().is_empty() {
                return Err(format!(
                    "tester spell {spell_name} case {id} expect cannot be empty"
                ));
            }

            let select = case
                .get("select")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    format!("tester spell {spell_name} case {id} select must be object")
                })?;

            let selected_names = select.keys().cloned().collect::<BTreeSet<_>>();

            let option_names = known_options.keys().cloned().collect::<BTreeSet<_>>();

            if selected_names != option_names {
                return Err(
                    format!(
                        "tester spell {spell_name} case {id} must select every declared option exactly once"
                    )
                );
            }

            for (option_name, selected_value) in select {
                let selected_value = selected_value.as_str().ok_or_else(|| {
                    format!(
                        "tester spell {spell_name} case {id} option {option_name} must select String"
                    )
                })?;

                let known = known_options.get(option_name).ok_or_else(|| {
                    format!(
                        "tester spell {spell_name} case {id} selects unknown option {option_name}"
                    )
                })?;

                if !known.contains(selected_value) {
                    return Err(
                        format!(
                            "tester spell {spell_name} case {id} selects unknown value {selected_value} for option {option_name}"
                        )
                    );
                }
            }

            if let Some(assertions) = case.get("assert") {
                let assertions = assertions.as_object().ok_or_else(|| {
                    format!("tester spell {spell_name} case {id} assert must be object")
                })?;

                let allowed_assertions =
                    BTreeSet::from(["contains", "absent", "size", "findings_size"]);

                let actual_assertions = assertions
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();

                if !actual_assertions.is_subset(&allowed_assertions) {
                    return Err(format!(
                        "tester spell {spell_name} case {id} contains unknown assertion"
                    ));
                }

                if let Some(contains) = assertions.get("contains") {
                    let contains = contains.as_object().ok_or_else(|| {
                        format!(
                            "tester spell {spell_name} case {id} contains assertion must be object"
                        )
                    })?;

                    for (key, value) in contains {
                        if key.trim().is_empty() {
                            return Err("contains assertion key cannot be empty".to_string());
                        }

                        if value.as_str().is_none() {
                            return Err("contains assertion values must be String".to_string());
                        }
                    }
                }

                if let Some(absent) = assertions.get("absent") {
                    let absent = absent
                        .as_array()
                        .ok_or_else(|| "absent assertion must be array".to_string())?;

                    for value in absent {
                        let value = value
                            .as_str()
                            .ok_or_else(|| "absent assertion values must be String".to_string())?;

                        if value.trim().is_empty() {
                            return Err("absent assertion value cannot be empty".to_string());
                        }
                    }
                }

                if let Some(size) = assertions.get("size") {
                    if size.as_u64().is_none() {
                        return Err("size assertion must be unsigned integer".to_string());
                    }
                }

                if let Some(size) = assertions.get("findings_size") {
                    if size.as_u64().is_none() {
                        return Err("findings_size assertion must be unsigned integer".to_string());
                    }
                }
            }

            total_cases += 1;
        }
    }

    Ok(total_cases)
}

#[derive(Debug, Serialize)]
pub struct MatrixExecutionReport {
    pub category: String,
    pub executed: usize,
    pub cases: Vec<String>,
}

#[derive(Debug)]
struct MatrixCaseOutcome {
    status: String,
    environment: Option<std::collections::BTreeMap<String, String>>,
    observation: crate::domestic_observation::DomesticObservation,
}

impl MatrixCaseOutcome {
    fn status(status: &str) -> Self {
        Self {
            status: status.to_string(),
            environment: None,
            observation: crate::domestic_observation::DomesticObservation::empty(),
        }
    }

    fn environment(status: &str, environment: std::collections::BTreeMap<String, String>) -> Self {
        Self {
            status: status.to_string(),
            environment: Some(environment),
            observation: crate::domestic_observation::DomesticObservation::empty(),
        }
    }

    fn observation(
        status: &str,
        observation: crate::domestic_observation::DomesticObservation,
    ) -> Self {
        Self {
            status: status.to_string(),
            environment: None,
            observation,
        }
    }
}

fn matrix_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("matrix field {key} must be String"))
}

fn command_environment_map(
    command: &std::process::Command,
) -> std::collections::BTreeMap<String, String> {
    command
        .get_envs()
        .filter_map(|(key, value)| {
            value.map(|value| {
                (
                    key.to_string_lossy().to_string(),
                    value.to_string_lossy().to_string(),
                )
            })
        })
        .collect()
}

fn apply_matrix_assertions(
    outcome: &MatrixCaseOutcome,
    assertions: Option<&Value>,
) -> Result<(), String> {
    let Some(assertions) = assertions else {
        return Ok(());
    };

    let assertions = assertions
        .as_object()
        .ok_or_else(|| "matrix assertions must be object".to_string())?;

    let environment = outcome.environment.as_ref();

    let evidence = &outcome.observation.evidence;

    let findings = &outcome.observation.findings;

    if let Some(contains) = assertions.get("contains") {
        let environment = environment
            .ok_or_else(|| "contains assertion requires an observable environment".to_string())?;

        let contains = contains
            .as_object()
            .ok_or_else(|| "contains assertion must be object".to_string())?;

        for (key, expected) in contains {
            let expected = expected
                .as_str()
                .ok_or_else(|| "contains assertion value must be String".to_string())?;

            let actual = environment
                .get(key)
                .ok_or_else(|| format!("matrix assertion expected environment key {key}"))?;

            if actual != expected {
                return Err(format!(
                    "matrix assertion for {key} expected {expected}, found {actual}"
                ));
            }
        }
    }

    if let Some(absent) = assertions.get("absent") {
        let environment = environment
            .ok_or_else(|| "absent assertion requires an observable environment".to_string())?;

        let absent = absent
            .as_array()
            .ok_or_else(|| "absent assertion must be array".to_string())?;

        for key in absent {
            let key = key
                .as_str()
                .ok_or_else(|| "absent assertion key must be String".to_string())?;

            if environment.contains_key(key) {
                return Err(format!(
                    "matrix assertion expected environment key {key} to be absent"
                ));
            }
        }
    }

    if let Some(size) = assertions.get("size") {
        let expected = size
            .as_u64()
            .ok_or_else(|| "size assertion must be unsigned integer".to_string())?
            as usize;

        let actual = if !evidence.is_empty() {
            evidence.len()
        } else if let Some(environment) = environment {
            environment.len()
        } else {
            0
        };

        if actual != expected {
            return Err(format!(
                "matrix assertion expected observable size {expected}, found {actual}"
            ));
        }
    }

    if let Some(size) = assertions.get("findings_size") {
        let expected = size
            .as_u64()
            .ok_or_else(|| "findings_size assertion must be unsigned integer".to_string())?
            as usize;

        if findings.len() != expected {
            return Err(format!(
                "matrix assertion expected findings size {expected}, found {}",
                findings.len()
            ));
        }
    }

    Ok(())
}

fn execute_call_contract_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let caller_return = matrix_string(select, "caller_return")?;

    let error_policy = matrix_string(select, "error_policy")?;

    let valid = match (caller_return, error_policy) {
        ("result", "propagate") => true,

        ("result", "fallback") => true,

        ("result", "ignore") => true,

        ("plain_value", "fallback") => true,

        ("plain_value", "ignore") => true,

        ("plain_value", "propagate") => false,

        _ => {
            return Err(format!(
                "unsupported call contract combination: {caller_return} + {error_policy}"
            ));
        }
    };

    Ok(MatrixCaseOutcome::status(if valid {
        "valid"
    } else {
        "invalid"
    }))
}

fn execute_argument_contract_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let required_argument = matrix_string(select, "required_argument")?;

    let forbidden_argument = matrix_string(select, "forbidden_argument")?;

    match required_argument {
        "present" | "absent" => {}

        other => {
            return Err(format!("unknown required argument state: {other}"));
        }
    }

    match forbidden_argument {
        "present" | "absent" => {}

        other => {
            return Err(format!("unknown forbidden argument state: {other}"));
        }
    }

    let valid = required_argument == "present" && forbidden_argument == "absent";

    Ok(MatrixCaseOutcome::status(if valid {
        "valid"
    } else {
        "invalid"
    }))
}

fn execute_reference_requirement_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let reachability = matrix_string(select, "reachability")?;

    let requirement = matrix_string(select, "requirement")?;

    let presence = matrix_string(select, "presence")?;

    match reachability {
        "reachable" | "unreachable" => {}

        other => {
            return Err(format!("unknown reference reachability: {other}"));
        }
    }

    match requirement {
        "required" | "optional" => {}

        other => {
            return Err(format!("unknown reference requirement: {other}"));
        }
    }

    match presence {
        "present" | "missing" => {}

        other => {
            return Err(format!("unknown reference presence: {other}"));
        }
    }

    let status = if reachability == "unreachable" {
        "unreachable"
    } else if presence == "present" {
        "pass"
    } else if requirement == "optional" {
        "optional_missing"
    } else {
        "report_missing"
    };

    Ok(MatrixCaseOutcome::status(status))
}

fn execute_transport_reference_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let source_form = matrix_string(select, "source_form")?;

    let semantic_target = matrix_string(select, "semantic_target")?;

    let transported_form = matrix_string(select, "transported_form")?;

    if !matches!(source_form, "absolute" | "relative") {
        return Err(format!("unknown transport source form: {source_form}"));
    }

    if !matches!(
        semantic_target,
        "inside_present" | "inside_missing" | "outside"
    ) {
        return Err(format!(
            "unknown transport semantic target: {semantic_target}"
        ));
    }

    if !matches!(transported_form, "relative" | "absolute") {
        return Err(format!("unknown transported form: {transported_form}"));
    }

    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = format!(
        "neebles-transport-reference-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                format!("system clock failed while building transport fixture: {error}")
            })?
            .as_nanos()
    );

    let base = std::env::temp_dir().join(unique);

    let world = base.join("world");

    let origin = world.join("usr/lib/neebles");

    let inside = world.join("usr/share/neebles/target");

    let outside = base.join("outside/target");

    std::fs::create_dir_all(&origin)
        .map_err(|error| format!("could not create transport origin fixture: {error}"))?;

    if semantic_target == "inside_present" {
        if let Some(parent) = inside.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("could not create transport inside fixture parent: {error}")
            })?;
        }

        std::fs::write(&inside, b"inside\n")
            .map_err(|error| format!("could not create transport inside fixture: {error}"))?;
    }

    if semantic_target == "outside" {
        if let Some(parent) = outside.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("could not create transport outside fixture parent: {error}")
            })?;
        }

        std::fs::write(&outside, b"outside\n")
            .map_err(|error| format!("could not create transport outside fixture: {error}"))?;
    }

    let reference = match (source_form, semantic_target) {
        ("absolute", "inside_present" | "inside_missing") => {
            std::path::PathBuf::from("/usr/share/neebles/target")
        }

        ("relative", "inside_present" | "inside_missing") => {
            std::path::PathBuf::from("../../share/neebles/target")
        }

        ("relative", "outside") => std::path::PathBuf::from("../../../../outside/target"),

        ("absolute", "outside") => std::path::PathBuf::from("/../outside/target"),

        _ => {
            return Err("unsupported transport reference matrix combination".to_string());
        }
    };

    let result = crate::domestic_world::transport_world_reference(&world, &origin, &reference);

    let outcome = match result {
        Ok(transported) => {
            let actual_form = if transported.is_absolute() {
                "absolute"
            } else {
                "relative"
            };

            if actual_form == transported_form {
                MatrixCaseOutcome::status("pass")
            } else {
                MatrixCaseOutcome::status("error")
            }
        }

        Err(_) => MatrixCaseOutcome::status("error"),
    };

    std::fs::remove_dir_all(&base)
        .map_err(|error| format!("could not remove transport reference fixture: {error}"))?;

    Ok(outcome)
}

fn execute_runtime_authority_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let manifest_identity = matrix_string(select, "manifest_identity")?;

    let root_form = matrix_string(select, "root_form")?;

    let root_presence = matrix_string(select, "root_presence")?;

    let world_state = matrix_string(select, "world_state")?;

    let category_state = matrix_string(select, "category_state")?;

    let target_cardinality = matrix_string(select, "target_cardinality")?;

    let target_form = matrix_string(select, "target_form")?;

    let target_presence = matrix_string(select, "target_presence")?;

    if !matches!(manifest_identity, "valid" | "invalid") {
        return Err(format!(
            "unknown runtime manifest identity: {manifest_identity}"
        ));
    }

    if !matches!(root_form, "relative" | "absolute" | "escape") {
        return Err(format!("unknown runtime root form: {root_form}"));
    }

    if !matches!(root_presence, "present" | "missing") {
        return Err(format!("unknown runtime root presence: {root_presence}"));
    }

    if !matches!(world_state, "present" | "missing") {
        return Err(format!("unknown runtime world state: {world_state}"));
    }

    if !matches!(category_state, "present" | "missing") {
        return Err(format!("unknown runtime category state: {category_state}"));
    }

    if !matches!(target_cardinality, "one" | "zero" | "many") {
        return Err(format!(
            "unknown runtime target cardinality: {target_cardinality}"
        ));
    }

    if !matches!(target_form, "relative" | "absolute" | "escape") {
        return Err(format!("unknown runtime target form: {target_form}"));
    }

    if !matches!(target_presence, "present" | "missing") {
        return Err(format!(
            "unknown runtime target presence: {target_presence}"
        ));
    }

    let unique = format!(
        "neebles-runtime-authority-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                format!("system clock failed while building runtime authority fixture: {error}")
            })?
            .as_nanos()
    );

    let base = std::env::temp_dir().join(unique);

    let authority = base.join("authority");

    let domestic_root = authority.join("rootfs");

    let escaped_root = base.join("escaped-root");

    std::fs::create_dir_all(&authority)
        .map_err(|error| format!("could not create runtime authority fixture: {error}"))?;

    let declared_root = match root_form {
        "relative" => "rootfs".to_string(),

        "absolute" => domestic_root.to_string_lossy().to_string(),

        "escape" => "../escaped-root".to_string(),

        _ => {
            unreachable!()
        }
    };

    let physical_declared_root = if root_form == "escape" {
        escaped_root.clone()
    } else {
        domestic_root.clone()
    };

    if root_presence == "present" {
        std::fs::create_dir_all(&physical_declared_root)
            .map_err(|error| format!("could not create declared runtime root fixture: {error}"))?;
    }

    let ordinary_target = physical_declared_root.join("usr/bin/tool");

    let escaped_target = physical_declared_root
        .parent()
        .unwrap_or(&base)
        .join("outside/tool");

    let target_value = match target_form {
        "relative" => "usr/bin/tool".to_string(),

        "absolute" => ordinary_target.to_string_lossy().to_string(),

        "escape" => "../outside/tool".to_string(),

        _ => {
            unreachable!()
        }
    };

    if root_presence == "present" && target_presence == "present" {
        let target = if target_form == "escape" {
            &escaped_target
        } else {
            &ordinary_target
        };

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("could not create runtime target parent fixture: {error}")
            })?;
        }

        std::fs::write(target, b"fixture\n")
            .map_err(|error| format!("could not create runtime target fixture: {error}"))?;
    }

    let targets = match target_cardinality {
        "zero" => Vec::<String>::new(),

        "one" => {
            vec![target_value.clone()]
        }

        "many" => {
            vec![target_value.clone(), "usr/bin/second".to_string()]
        }

        _ => {
            unreachable!()
        }
    };

    if root_presence == "present"
        && target_presence == "present"
        && target_cardinality == "many"
        && target_form != "escape"
    {
        let second = physical_declared_root.join("usr/bin/second");

        if let Some(parent) = second.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("could not create second runtime target parent fixture: {error}")
            })?;
        }

        std::fs::write(second, b"second\n")
            .map_err(|error| format!("could not create second runtime target fixture: {error}"))?;
    }

    let categories = if category_state == "present" {
        serde_json::json!({
            "executable": {
                "category": "executable",
                "value": "tool",
                "declared_targets": [
                    "usr/bin/tool"
                ],
                "resolved_targets": targets
            }
        })
    } else {
        serde_json::json!({})
    };

    let worlds = serde_json::json!({
        "boss.fixture": {
            "categories": categories
        }
    });

    let manifest = serde_json::json!({
        "schema": "1",
        "name": (
            if manifest_identity
                == "valid"
            {
                "neebles-domestic-runtime"
            } else {
                "invalid-runtime"
            }
        ),
        "root": declared_root,
        "worlds": worlds
    });

    let manifest_path = authority.join("domestic-runtime.json");

    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("could not serialize runtime authority fixture: {error}"))?,
    )
    .map_err(|error| format!("could not write runtime authority fixture: {error}"))?;

    let requested_world = if world_state == "present" {
        "boss.fixture"
    } else {
        "boss.missing"
    };

    let result = crate::domestic_runtime_authority::resolve_materialized_runtime_target(
        &manifest_path,
        requested_world,
        "executable",
    );

    let outcome = match result {
        Ok(_) => MatrixCaseOutcome::status("resolved"),

        Err(_) => MatrixCaseOutcome::status("error"),
    };

    std::fs::remove_dir_all(&base)
        .map_err(|error| format!("could not remove runtime authority fixture: {error}"))?;

    Ok(outcome)
}

fn create_matrix_target(root: &Path, relative: &Path, shape: &str) -> Result<(), String> {
    match shape {
        "file" => {
            if let Some(parent) = relative.parent() {
                fs::create_dir_all(root.join(parent))
                    .map_err(|error| format!("could not create matrix parent: {error}"))?;
            }

            fs::write(root.join(relative), b"matrix\n")
                .map_err(|error| format!("could not create matrix file: {error}"))?;
        }

        "directory" => {
            fs::create_dir_all(root.join(relative))
                .map_err(|error| format!("could not create matrix directory: {error}"))?;
        }

        "symlink" => {
            let source = root.join("matrix-symlink-source");

            fs::write(&source, b"matrix\n")
                .map_err(|error| format!("could not create matrix symlink source: {error}"))?;

            std::os::unix::fs::symlink("matrix-symlink-source", root.join(relative))
                .map_err(|error| format!("could not create matrix symlink: {error}"))?;
        }

        "missing" => {}

        other => {
            return Err(format!("unknown matrix target shape: {other}"));
        }
    }

    Ok(())
}

fn execute_physical_case(
    contract_path: &Path,
    category: &str,
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let shape = matrix_string(select, "target_shape")?;

    let count = matrix_string(select, "target_count")?;

    let path_form = matrix_string(select, "path_form")?;

    let unique = format!(
        "neebles-matrix-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| { format!("invalid system clock: {error}") })?
            .as_nanos()
    );

    let root = std::env::temp_dir().join(unique);

    fs::create_dir_all(&root).map_err(|error| format!("could not create matrix root: {error}"))?;

    let result = (|| -> Result<MatrixCaseOutcome, String> {
        let targets = match path_form {
            "relative" => match (shape, count) {
                ("mixed", "multiple") => {
                    create_matrix_target(&root, Path::new("alpha"), "file")?;

                    create_matrix_target(&root, Path::new("beta"), "directory")?;

                    vec![PathBuf::from("alpha"), PathBuf::from("beta")]
                }

                (current_shape, "single") => {
                    create_matrix_target(&root, Path::new("alpha"), current_shape)?;

                    vec![PathBuf::from("alpha")]
                }

                (current_shape, "multiple") => {
                    create_matrix_target(&root, Path::new("alpha"), current_shape)?;

                    create_matrix_target(&root, Path::new("beta"), current_shape)?;

                    vec![PathBuf::from("alpha"), PathBuf::from("beta")]
                }

                _ => {
                    return Err("unsupported physical matrix combination".to_string());
                }
            },

            "absolute" => {
                let absolute = root.join("absolute-target");

                fs::write(&absolute, b"matrix\n")
                    .map_err(|error| format!("could not create absolute matrix target: {error}"))?;

                vec![absolute]
            }

            "parent_escape" => {
                vec![PathBuf::from("../escape")]
            }

            other => {
                return Err(format!("unknown path form: {other}"));
            }
        };

        match test_category(contract_path, &root, category, "matrix-value", &targets) {
            Ok(report) if report.ok => Ok(MatrixCaseOutcome::status("pass")),

            Ok(_) => Ok(MatrixCaseOutcome::status("report_missing")),

            Err(_) => Ok(MatrixCaseOutcome::status("error")),
        }
    })();

    let _ = fs::remove_dir_all(&root);

    result
}

fn execute_path_contract_case(
    contract_path: &Path,
    category: &str,
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let path_form = matrix_string(select, "path_form")?;

    let cardinality = matrix_string(select, "cardinality")?;

    let presence = matrix_string(select, "presence")?;

    let shape = if presence == "missing" {
        "missing"
    } else {
        "file"
    };

    let mut translated = serde_json::Map::<String, Value>::new();

    translated.insert("target_shape".to_string(), Value::String(shape.to_string()));

    translated.insert(
        "target_count".to_string(),
        Value::String(cardinality.to_string()),
    );

    translated.insert(
        "path_form".to_string(),
        Value::String(path_form.to_string()),
    );

    execute_physical_case(contract_path, category, &translated)
}

fn execute_environment_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use crate::domestic_environment::{
        build_process_command, build_process_environment, ProcessEnvironmentClass,
    };

    let class_name = matrix_string(select, "class")?;

    let domestic_mode = matrix_string(select, "domestic")?;

    let session_mode = matrix_string(select, "session")?;

    let allowlist_mode = matrix_string(select, "allowlist")?;

    let application = matrix_string(select, "application")?;

    let class = match class_name {
        "pure" => ProcessEnvironmentClass::Pure,

        "session" => ProcessEnvironmentClass::Session,

        "system_interface" => ProcessEnvironmentClass::SystemInterface,

        other => {
            return Err(format!("unknown process environment class: {other}"));
        }
    };

    let mut domestic = std::collections::BTreeMap::<String, String>::new();

    match domestic_mode {
        "empty" => {}

        "explicit" => {
            if class_name == "system_interface" {
                domestic.insert("XDG_RUNTIME_DIR".to_string(), "/run/user/1000".to_string());

                domestic.insert(
                    "DBUS_SESSION_BUS_ADDRESS".to_string(),
                    "unix:path=/run/user/1000/bus".to_string(),
                );
            } else {
                domestic.insert("NEEBLES_TEST".to_string(), "inside".to_string());
            }
        }

        "private_path" => {
            domestic.insert("PATH".to_string(), "/opt/neebles/private/bin".to_string());
        }

        other => {
            return Err(format!("unknown domestic environment fixture: {other}"));
        }
    }

    let mut session = std::collections::BTreeMap::<String, String>::new();

    match session_mode {
        "empty" => {}

        "allowed" => {
            session.insert("DISPLAY".to_string(), ":0".to_string());

            session.insert("NEEBLES_TEST".to_string(), "outside".to_string());
        }

        "graphical_bridge" => {
            session.insert("DISPLAY".to_string(), ":0".to_string());

            session.insert("WAYLAND_DISPLAY".to_string(), "wayland-test".to_string());

            session.insert(
                "XAUTHORITY".to_string(),
                "/run/user/test/xauthority".to_string(),
            );

            session.insert("XDG_RUNTIME_DIR".to_string(), "/run/user/test".to_string());

            session.insert(
                "DBUS_SESSION_BUS_ADDRESS".to_string(),
                "unix:path=/run/user/test/bus".to_string(),
            );

            session.insert("PATH".to_string(), "/host/bin".to_string());

            session.insert("LD_PRELOAD".to_string(), "/host/evil.so".to_string());

            session.insert(
                "XDG_CURRENT_DESKTOP".to_string(),
                "HOST_DESKTOP".to_string(),
            );

            session.insert("DESKTOP_SESSION".to_string(), "host-session".to_string());
        }

        "hostile" => {
            session.insert("PATH".to_string(), "/host/bin".to_string());

            session.insert("LD_PRELOAD".to_string(), "/tmp/evil.so".to_string());

            session.insert("DISPLAY".to_string(), ":666".to_string());
        }

        other => {
            return Err(format!("unknown session fixture: {other}"));
        }
    }

    let mut allowed = BTreeSet::<String>::new();

    match allowlist_mode {
        "none" => {}

        "exact" => {
            allowed.insert("DISPLAY".to_string());
        }

        "graphical_bridge" => {
            for key in [
                "DISPLAY",
                "WAYLAND_DISPLAY",
                "XAUTHORITY",
                "XDG_RUNTIME_DIR",
                "DBUS_SESSION_BUS_ADDRESS",
            ] {
                allowed.insert(key.to_string());
            }
        }

        "forbidden" => {
            allowed.insert("PATH".to_string());
        }

        "override" => {
            allowed.insert("NEEBLES_TEST".to_string());
        }

        other => {
            return Err(format!("unknown allowlist fixture: {other}"));
        }
    }

    match application {
        "map" => match build_process_environment(class, &domestic, &session, &allowed) {
            Ok(environment) => Ok(MatrixCaseOutcome::environment("pass", environment)),

            Err(_) => Ok(MatrixCaseOutcome::status("error")),
        },

        "command" => {
            match build_process_command("not-executed", class, &domestic, &session, &allowed) {
                Ok(command) => Ok(MatrixCaseOutcome::environment(
                    "pass",
                    command_environment_map(&command),
                )),

                Err(_) => Ok(MatrixCaseOutcome::status("error")),
            }
        }

        other => Err(format!("unknown environment application: {other}")),
    }
}

fn execute_session_policy_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use crate::domestic_environment::build_sealed_environment;

    let request = matrix_string(select, "request")?;

    let source = matrix_string(select, "source")?;

    let collision = matrix_string(select, "domestic_collision")?;

    let mut domestic = std::collections::BTreeMap::<String, String>::new();

    let mut session = std::collections::BTreeMap::<String, String>::new();

    let mut allowed = BTreeSet::<String>::new();

    if collision == "yes" {
        domestic.insert("NEEBLES_CONFIG".to_string(), "/domestic/config".to_string());
    }

    match request {
        "absent" => {}

        "exact" => {
            allowed.insert("DISPLAY".to_string());

            if source == "present" {
                session.insert("DISPLAY".to_string(), ":0".to_string());
            }
        }

        "forbidden" => {
            allowed.insert("PATH".to_string());

            if source == "present" {
                session.insert("PATH".to_string(), "/host/bin".to_string());
            }
        }

        "override" => {
            allowed.insert("NEEBLES_CONFIG".to_string());

            if source == "present" {
                session.insert("NEEBLES_CONFIG".to_string(), "/host/config".to_string());
            }
        }

        other => {
            return Err(format!("unknown session policy request: {other}"));
        }
    }

    match build_sealed_environment(&domestic, &session, &allowed) {
        Ok(environment) => Ok(MatrixCaseOutcome::environment("pass", environment)),

        Err(_) => Ok(MatrixCaseOutcome::status("error")),
    }
}

fn execute_raw_environment_parser_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use crate::domestic_environment::{build_sealed_environment, parse_environment_lines};

    let input_shape = matrix_string(select, "input_shape")?;

    let policy = matrix_string(select, "policy")?;

    let raw = match input_shape {
        "mixed" => "PATH=/host/bin\nDISPLAY=:0\nLD_PRELOAD=/tmp/evil.so\nBROKEN\n",

        other => {
            return Err(format!("unknown raw environment input shape: {other}"));
        }
    };

    let parsed = parse_environment_lines(raw);

    match policy {
        "parse_only" => Ok(MatrixCaseOutcome::environment("pass", parsed)),

        "seal_exact" => {
            let allowed = BTreeSet::from(["DISPLAY".to_string()]);

            match build_sealed_environment(&std::collections::BTreeMap::new(), &parsed, &allowed) {
                Ok(environment) => Ok(MatrixCaseOutcome::environment("pass", environment)),

                Err(_) => Ok(MatrixCaseOutcome::status("error")),
            }
        }

        other => Err(format!("unknown raw environment policy: {other}")),
    }
}

fn execute_capability_provider_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use crate::domestic_capability_provider::parse_capability_provider_output;

    let shape = matrix_string(select, "shape")?;

    let raw = match shape {
        "valid" => {
            r#"{
            "schema": "1",
            "name": "neebles-capability-provider-output",
            "authority": "platform.example",
            "protocol": "neebles-example-v1",
            "values": {
                "EXAMPLE_ALPHA": "one",
                "EXAMPLE_BETA": "two"
            }
        }"#
        }

        "empty_values" => {
            r#"{
            "schema": "1",
            "name": "neebles-capability-provider-output",
            "authority": "platform.example",
            "protocol": "neebles-example-v1",
            "values": {}
        }"#
        }

        "wrong_schema" => {
            r#"{
            "schema": "2",
            "name": "neebles-capability-provider-output",
            "authority": "platform.example",
            "protocol": "neebles-example-v1",
            "values": {}
        }"#
        }

        "wrong_name" => {
            r#"{
            "schema": "1",
            "name": "wrong-output",
            "authority": "platform.example",
            "protocol": "neebles-example-v1",
            "values": {}
        }"#
        }

        "wrong_authority" => {
            r#"{
            "schema": "1",
            "name": "neebles-capability-provider-output",
            "authority": "platform.other",
            "protocol": "neebles-example-v1",
            "values": {}
        }"#
        }

        "wrong_protocol" => {
            r#"{
            "schema": "1",
            "name": "neebles-capability-provider-output",
            "authority": "platform.example",
            "protocol": "neebles-other-v1",
            "values": {}
        }"#
        }

        "unknown_field" => {
            r#"{
            "schema": "1",
            "name": "neebles-capability-provider-output",
            "authority": "platform.example",
            "protocol": "neebles-example-v1",
            "values": {},
            "magic": true
        }"#
        }

        "non_string_value" => {
            r#"{
            "schema": "1",
            "name": "neebles-capability-provider-output",
            "authority": "platform.example",
            "protocol": "neebles-example-v1",
            "values": {
                "EXAMPLE": 7
            }
        }"#
        }

        "empty_key" => {
            r#"{
            "schema": "1",
            "name": "neebles-capability-provider-output",
            "authority": "platform.example",
            "protocol": "neebles-example-v1",
            "values": {
                "": "invalid"
            }
        }"#
        }

        "trailing_garbage" => {
            r#"{
            "schema": "1",
            "name": "neebles-capability-provider-output",
            "authority": "platform.example",
            "protocol": "neebles-example-v1",
            "values": {}
        }
        garbage"#
        }

        other => {
            return Err(format!("unknown capability provider fixture: {other}"));
        }
    };

    match parse_capability_provider_output(raw.as_bytes(), "platform.example", "neebles-example-v1")
    {
        Ok(values) => Ok(MatrixCaseOutcome::environment("pass", values)),

        Err(_) => Ok(MatrixCaseOutcome::status("error")),
    }
}

fn execute_command_sealing_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use crate::domestic_environment::apply_sealed_environment;

    let preseed = matrix_string(select, "preseed")?;

    let sealed = matrix_string(select, "sealed")?;

    let mut command = std::process::Command::new("not-executed");

    match preseed {
        "hostile" => {
            command.env("PATH", "/host/bin");

            command.env("LD_PRELOAD", "/host/evil.so");

            command.env("UNRELATED_HOST_VALUE", "must-die");
        }

        other => {
            return Err(format!("unknown command preseed fixture: {other}"));
        }
    }

    let environment = match sealed {
        "explicit" => std::collections::BTreeMap::from([
            ("NEEBLES_MODULE".to_string(), "test-module".to_string()),
            ("DISPLAY".to_string(), ":0".to_string()),
        ]),

        other => {
            return Err(format!("unknown sealed environment fixture: {other}"));
        }
    };

    apply_sealed_environment(&mut command, &environment);

    Ok(MatrixCaseOutcome::environment(
        "pass",
        command_environment_map(&command),
    ))
}

fn execute_session_selection_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use crate::domestic_environment::build_sealed_environment;

    let selection = matrix_string(select, "selection")?;

    let source = matrix_string(select, "source")?;

    if selection != "exact" {
        return Err(format!("unknown session selection rule: {selection}"));
    }

    let session = match source {
        "neighbors" => std::collections::BTreeMap::from([
            ("DISPLAY".to_string(), ":0".to_string()),
            ("XDG_SESSION_TYPE".to_string(), "wayland".to_string()),
            ("XDG_CURRENT_DESKTOP".to_string(), "KDE".to_string()),
            ("QT_SCALE_FACTOR".to_string(), "2".to_string()),
            ("KDE_FULL_SESSION".to_string(), "true".to_string()),
        ]),

        other => {
            return Err(format!("unknown session source fixture: {other}"));
        }
    };

    let allowed = BTreeSet::from(["DISPLAY".to_string(), "XDG_SESSION_TYPE".to_string()]);

    match build_sealed_environment(&std::collections::BTreeMap::new(), &session, &allowed) {
        Ok(environment) => Ok(MatrixCaseOutcome::environment("pass", environment)),

        Err(_) => Ok(MatrixCaseOutcome::status("error")),
    }
}

fn execute_forbidden_session_key_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use crate::domestic_environment::build_sealed_environment;

    let key = matrix_string(select, "key")?;

    let session =
        std::collections::BTreeMap::from([(key.to_string(), "host-controlled".to_string())]);

    let allowed = BTreeSet::from([key.to_string()]);

    match build_sealed_environment(&std::collections::BTreeMap::new(), &session, &allowed) {
        Ok(environment) => Ok(MatrixCaseOutcome::environment("pass", environment)),

        Err(_) => Ok(MatrixCaseOutcome::status("error")),
    }
}

fn execute_observation_contract_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let shape = matrix_string(select, "shape")?;

    let observation = match shape {
        "empty" => crate::domestic_observation::DomesticObservation::empty(),

        "finding" => crate::domestic_observation::DomesticObservation::with_findings(vec![
            "finding.alpha".to_string(),
        ]),

        "multiple_findings" => {
            crate::domestic_observation::DomesticObservation::with_findings(vec![
                "finding.alpha".to_string(),
                "finding.beta".to_string(),
            ])
        }

        "evidence_and_findings" => {
            crate::domestic_observation::DomesticObservation::with_evidence_and_findings(
                vec!["evidence.alpha".to_string(), "evidence.beta".to_string()],
                vec!["finding.alpha".to_string()],
            )
        }

        other => {
            return Err(format!("unknown observation contract fixture: {other}"));
        }
    };

    Ok(MatrixCaseOutcome::observation("pass", observation))
}

fn execute_elf_interpreter_resolution_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let shape = matrix_string(select, "shape")?;

    let world_root =
        std::env::temp_dir().join(format!("neebles-interpreter-matrix-{}", std::process::id()));

    if world_root.exists() {
        fs::remove_dir_all(&world_root).map_err(|error| {
            format!(
                "could not clean interpreter matrix world {}: {error}",
                world_root.display()
            )
        })?;
    }

    fs::create_dir_all(&world_root).map_err(|error| {
        format!(
            "could not create interpreter matrix world {}: {error}",
            world_root.display()
        )
    })?;

    let interpreter = match shape {
        "none" => None,

        "absolute_inside" => {
            let loader = world_root.join("lib64/ld-linux-x86-64.so.2");

            if let Some(parent) = loader.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("could not create interpreter parent: {error}"))?;
            }

            fs::write(&loader, b"loader")
                .map_err(|error| format!("could not create interpreter fixture: {error}"))?;

            Some(PathBuf::from("/lib64/ld-linux-x86-64.so.2"))
        }

        "absolute_missing" => Some(PathBuf::from("/lib64/missing-loader.so")),

        "relative" => Some(PathBuf::from("relative-loader")),

        other => {
            return Err(format!(
                "unknown ELF interpreter resolution fixture: {other}"
            ));
        }
    };

    let metadata = crate::domestic_elf::DomesticElfMetadata {
        path: world_root.join("bin/fixture"),
        interpreter,
        needed: Vec::new(),
        raw_rpath: Vec::new(),
        raw_runpath: Vec::new(),
        rpath: Vec::new(),
        runpath: Vec::new(),
    };

    match crate::domestic_elf::resolve_interpreter_authority(&world_root, &metadata) {
        Ok(_) => Ok(MatrixCaseOutcome::status("pass")),

        Err(_) => Ok(MatrixCaseOutcome::status("error")),
    }
}

fn execute_elf_interpreter_observation_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let interpreter_shape = matrix_string(select, "interpreter_shape")?;

    let interpreter = match interpreter_shape {
        "none" => None,

        "absolute" => Some(PathBuf::from("/lib64/ld-linux-x86-64.so.2")),

        "relative" => Some(PathBuf::from("loader-relative")),

        other => {
            return Err(format!("unknown ELF interpreter fixture: {other}"));
        }
    };

    let metadata = crate::domestic_elf::DomesticElfMetadata {
        path: PathBuf::from("/world/bin/fixture"),
        interpreter,
        needed: Vec::new(),
        raw_rpath: Vec::new(),
        raw_runpath: Vec::new(),
        rpath: Vec::new(),
        runpath: Vec::new(),
    };

    let observed = crate::domestic_elf::observed_interpreter_authorities(&metadata);

    if observed
        .iter()
        .any(|value| !value.starts_with("elf.interpreter:"))
    {
        return Err("ELF interpreter observation escaped its namespace".to_string());
    }

    Ok(MatrixCaseOutcome::observation(
        "pass",
        crate::domestic_observation::DomesticObservation::with_evidence(observed),
    ))
}

fn execute_search_authority_grant_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let shape = matrix_string(select, "shape")?;

    let base =
        std::env::temp_dir().join(format!("neebles-search-authority-{}", std::process::id()));

    if base.exists() {
        std::fs::remove_dir_all(&base).map_err(|error| {
            format!(
                "could not reset search authority fixture {}: {error}",
                base.display()
            )
        })?;
    }

    let world = base.join("world");

    let descriptors = base.join("descriptors");

    std::fs::create_dir_all(world.join("usr/lib/x86_64-linux-gnu"))
        .map_err(|error| format!("could not create first domestic authority path: {error}"))?;

    std::fs::create_dir_all(world.join("opt/secondary/lib"))
        .map_err(|error| format!("could not create second domestic authority path: {error}"))?;

    std::fs::create_dir_all(&descriptors).map_err(|error| {
        format!("could not create external authority descriptor directory: {error}")
    })?;

    let first_descriptor = descriptors.join("multiarch.json");

    let second_descriptor = descriptors.join("secondary.json");

    let relative_descriptor = PathBuf::from("relative-authority.json");

    let first_json = serde_json::json!(
        {
            "schema": "1",
            "name": "domestic-search-authority",
            "authority": "core.multiarch",
            "domestic_path": "/usr/lib/x86_64-linux-gnu"
        }
    );

    let second_json = serde_json::json!(
        {
            "schema": "1",
            "name": "domestic-search-authority",
            "authority": "x.secondary",
            "domestic_path": "/opt/secondary/lib"
        }
    );

    std::fs::write(
        &first_descriptor,
        serde_json::to_vec_pretty(&first_json)
            .map_err(|error| format!("could not serialize first authority fixture: {error}"))?,
    )
    .map_err(|error| format!("could not write first authority fixture: {error}"))?;

    std::fs::write(
        &second_descriptor,
        serde_json::to_vec_pretty(&second_json)
            .map_err(|error| format!("could not serialize second authority fixture: {error}"))?,
    )
    .map_err(|error| format!("could not write second authority fixture: {error}"))?;

    let registry_path = base.join("registry.json");

    let (registry_entries, grants) = match shape {
        "zero" => (Vec::<Value>::new(), Vec::<String>::new()),

        "one" => (
            vec![serde_json::json!(
                {
                    "name": "core.multiarch",
                    "location": first_descriptor.display().to_string()
                }
            )],
            vec!["core.multiarch".to_string()],
        ),

        "multiple" => (
            vec![
                serde_json::json!(
                    {
                        "name": "core.multiarch",
                        "location": first_descriptor.display().to_string()
                    }
                ),
                serde_json::json!(
                    {
                        "name": "x.secondary",
                        "location": second_descriptor.display().to_string()
                    }
                ),
            ],
            vec!["core.multiarch".to_string(), "x.secondary".to_string()],
        ),

        "duplicate_grant" => (
            vec![serde_json::json!(
                {
                    "name": "core.multiarch",
                    "location": first_descriptor.display().to_string()
                }
            )],
            vec!["core.multiarch".to_string(), "core.multiarch".to_string()],
        ),

        "unknown" => (Vec::<Value>::new(), vec!["missing.authority".to_string()]),

        "relative_descriptor_location" => (
            vec![serde_json::json!(
                {
                    "name": "core.multiarch",
                    "location": relative_descriptor.display().to_string()
                }
            )],
            vec!["core.multiarch".to_string()],
        ),

        "relative_domestic_path" => {
            let bad_descriptor = descriptors.join("relative-domestic.json");

            let bad_json = serde_json::json!(
                {
                    "schema": "1",
                    "name": "domestic-search-authority",
                    "authority": "bad.relative",
                    "domestic_path": "usr/lib"
                }
            );

            std::fs::write(
                &bad_descriptor,
                serde_json::to_vec_pretty(&bad_json).map_err(|error| {
                    format!("could not serialize relative domestic fixture: {error}")
                })?,
            )
            .map_err(|error| format!("could not write relative domestic fixture: {error}"))?;

            (
                vec![serde_json::json!(
                    {
                        "name": "bad.relative",
                        "location": bad_descriptor.display().to_string()
                    }
                )],
                vec!["bad.relative".to_string()],
            )
        }

        other => {
            return Err(format!("unknown search authority grant fixture: {other}"));
        }
    };

    let registry_json = serde_json::json!(
        {
            "schema": "1",
            "name": "domestic-search-authority-registry",
            "entries": registry_entries
        }
    );

    std::fs::write(
        &registry_path,
        serde_json::to_vec_pretty(&registry_json)
            .map_err(|error| format!("could not serialize authority registry fixture: {error}"))?,
    )
    .map_err(|error| format!("could not write authority registry fixture: {error}"))?;

    let registry = match crate::domestic_search_authority::DomesticSearchAuthorityRegistry::load(
        &registry_path,
    ) {
        Ok(registry) => registry,

        Err(_) => return Ok(MatrixCaseOutcome::status("error")),
    };

    match crate::domestic_search_authority::resolve_execution_search_authorities(
        &world, &registry, &grants,
    ) {
        Ok(authorities) => Ok(MatrixCaseOutcome::observation(
            "pass",
            crate::domestic_observation::DomesticObservation::with_evidence(
                authorities
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect(),
            ),
        )),

        Err(_) => Ok(MatrixCaseOutcome::status("error")),
    }
}

fn execute_search_context_normalization_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let shape = matrix_string(select, "shape")?;

    let (entries, expected) = match shape {
        "empty" => (Vec::new(), Vec::new()),

        "unique" => (
            vec![PathBuf::from("/alpha"), PathBuf::from("/beta")],
            vec![PathBuf::from("/alpha"), PathBuf::from("/beta")],
        ),

        "duplicates" => (
            vec![
                PathBuf::from("/alpha"),
                PathBuf::from("/alpha"),
                PathBuf::from("/beta"),
                PathBuf::from("/alpha"),
                PathBuf::from("/beta"),
            ],
            vec![PathBuf::from("/alpha"), PathBuf::from("/beta")],
        ),

        "first_occurrence_order" => (
            vec![
                PathBuf::from("/beta"),
                PathBuf::from("/alpha"),
                PathBuf::from("/beta"),
                PathBuf::from("/gamma"),
                PathBuf::from("/alpha"),
            ],
            vec![
                PathBuf::from("/beta"),
                PathBuf::from("/alpha"),
                PathBuf::from("/gamma"),
            ],
        ),

        other => {
            return Err(format!(
                "unknown search context normalization fixture: {other}"
            ));
        }
    };

    let normalized = crate::domestic_elf::normalize_search_entries(&entries);

    if normalized != expected {
        return Ok(MatrixCaseOutcome::status("error"));
    }

    Ok(MatrixCaseOutcome::observation(
        "pass",
        crate::domestic_observation::DomesticObservation::with_evidence(
            normalized
                .iter()
                .map(|entry| entry.display().to_string())
                .collect(),
        ),
    ))
}

fn execute_effective_search_authority_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let shape = matrix_string(select, "shape")?;

    let (entries, controlled_defaults, expected_status) = match shape {
        "explicit_only" => (vec![PathBuf::from("/private/lib")], Vec::new(), "pass"),

        "defaults_only" => (
            Vec::new(),
            vec![
                PathBuf::from("/controlled/lib"),
                PathBuf::from("/controlled/usr/lib"),
            ],
            "pass",
        ),

        "explicit_then_defaults" => (
            vec![PathBuf::from("/private/lib")],
            vec![
                PathBuf::from("/controlled/lib"),
                PathBuf::from("/controlled/usr/lib"),
            ],
            "pass",
        ),

        "origin_then_defaults" => (
            vec![PathBuf::from(format!("{}ORIGIN/../lib", 36u8 as char))],
            vec![PathBuf::from("/controlled/lib")],
            "pass",
        ),

        "invalid_default" => (
            Vec::new(),
            vec![PathBuf::from("host-relative-lib")],
            "error",
        ),

        other => {
            return Err(format!(
                "unknown effective search authority fixture: {other}"
            ));
        }
    };

    let composition = crate::domestic_elf::EffectiveSearchComposition {
        entries,
        next_transitive: crate::domestic_elf::TransitiveSearchContext {
            source: crate::domestic_elf::TransitiveSearchSource::None,
            entries: Vec::new(),
        },
    };

    match crate::domestic_elf::effective_search_authorities(&composition, &controlled_defaults) {
        Ok(authorities) => {
            if expected_status == "error" {
                return Ok(MatrixCaseOutcome::status("pass"));
            }

            Ok(MatrixCaseOutcome::observation(
                "pass",
                crate::domestic_observation::DomesticObservation::with_evidence(
                    authorities
                        .iter()
                        .map(|authority| {
                            format!(
                                "{:?}:{}",
                                authority.authority,
                                authority.reference.display()
                            )
                        })
                        .collect(),
                ),
            ))
        }

        Err(_) => {
            if expected_status == "error" {
                Ok(MatrixCaseOutcome::status("error"))
            } else {
                Ok(MatrixCaseOutcome::status("error"))
            }
        }
    }
}

fn execute_elf_recursive_closure_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let shape = matrix_string(select, "shape")?;

    let root = std::env::temp_dir().join(format!("neebles-elf-recursive-{}", std::process::id()));

    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|error| {
            format!(
                "could not reset recursive ELF fixture root {}: {error}",
                root.display()
            )
        })?;
    }

    std::fs::create_dir_all(root.join("bin"))
        .map_err(|error| format!("could not create recursive ELF bin directory: {error}"))?;

    std::fs::create_dir_all(root.join("lib"))
        .map_err(|error| format!("could not create recursive ELF lib directory: {error}"))?;

    let root_path = root.join("bin/root");

    std::fs::write(&root_path, b"root")
        .map_err(|error| format!("could not create recursive ELF root: {error}"))?;

    let empty_context = crate::domestic_elf::TransitiveSearchContext {
        source: crate::domestic_elf::TransitiveSearchSource::None,
        entries: Vec::new(),
    };

    let controlled_defaults = vec![PathBuf::from("/lib")];

    match shape {
        "chain" => {
            for name in ["libalpha.so", "libbeta.so"] {
                std::fs::write(root.join("lib").join(name), b"fixture").map_err(|error| {
                    format!("could not create chain dependency {name}: {error}")
                })?;
            }

            let closure = crate::domestic_elf::resolve_elf_recursive_closure(
                &root,
                &root_path,
                &empty_context,
                &controlled_defaults,
                |path| {
                    let name = path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .ok_or_else(|| {
                            format!(
                                "recursive fixture path has no UTF-8 name: {}",
                                path.display()
                            )
                        })?;

                    let needed = match name {
                        "root" => vec!["libalpha.so".to_string()],

                        "libalpha.so" => vec!["libbeta.so".to_string()],

                        "libbeta.so" => Vec::new(),

                        other => return Err(format!("unknown recursive chain node: {other}")),
                    };

                    Ok(crate::domestic_elf::DomesticElfMetadata {
                        path: path.to_path_buf(),
                        interpreter: None,
                        needed,
                        raw_rpath: Vec::new(),
                        raw_runpath: Vec::new(),
                        rpath: Vec::new(),
                        runpath: Vec::new(),
                    })
                },
            );

            match closure {
                Ok(states) => Ok(MatrixCaseOutcome::observation(
                    "pass",
                    crate::domestic_observation::DomesticObservation::with_evidence(
                        states
                            .iter()
                            .map(|state| state.path.display().to_string())
                            .collect(),
                    ),
                )),

                Err(_) => Ok(MatrixCaseOutcome::status("error")),
            }
        }

        "cycle" => {
            let alpha = root.join("lib/libalpha.so");

            std::fs::write(&alpha, b"fixture")
                .map_err(|error| format!("could not create cycle alpha: {error}"))?;

            let cycle_root = root.join("lib/libroot.so");

            std::fs::rename(&root_path, &cycle_root)
                .map_err(|error| format!("could not move cycle root: {error}"))?;

            let closure = crate::domestic_elf::resolve_elf_recursive_closure(
                &root,
                &cycle_root,
                &empty_context,
                &controlled_defaults,
                |path| {
                    let name = path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .ok_or_else(|| {
                            format!("cycle fixture path has no UTF-8 name: {}", path.display())
                        })?;

                    let needed = match name {
                        "libroot.so" => vec!["libalpha.so".to_string()],

                        "libalpha.so" => vec!["libroot.so".to_string()],

                        other => return Err(format!("unknown recursive cycle node: {other}")),
                    };

                    Ok(crate::domestic_elf::DomesticElfMetadata {
                        path: path.to_path_buf(),
                        interpreter: None,
                        needed,
                        raw_rpath: Vec::new(),
                        raw_runpath: Vec::new(),
                        rpath: Vec::new(),
                        runpath: Vec::new(),
                    })
                },
            );

            match closure {
                Ok(states) => Ok(MatrixCaseOutcome::observation(
                    "pass",
                    crate::domestic_observation::DomesticObservation::with_evidence(
                        states
                            .iter()
                            .map(|state| state.path.display().to_string())
                            .collect(),
                    ),
                )),

                Err(_) => Ok(MatrixCaseOutcome::status("error")),
            }
        }

        "same_path_distinct_context" => {
            for name in ["libbranch_a.so", "libbranch_b.so", "libshared.so"] {
                std::fs::write(root.join("lib").join(name), b"fixture").map_err(|error| {
                    format!("could not create contextual dependency {name}: {error}")
                })?;
            }

            let closure = crate::domestic_elf::resolve_elf_recursive_closure(
                &root,
                &root_path,
                &empty_context,
                &controlled_defaults,
                |path| {
                    let name = path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .ok_or_else(|| {
                            format!("context fixture path has no UTF-8 name: {}", path.display())
                        })?;

                    let (needed, rpath) = match name {
                        "root" => (
                            vec!["libbranch_a.so".to_string(), "libbranch_b.so".to_string()],
                            Vec::new(),
                        ),

                        "libbranch_a.so" => (
                            vec!["libshared.so".to_string()],
                            vec![PathBuf::from("/branch-a")],
                        ),

                        "libbranch_b.so" => (
                            vec!["libshared.so".to_string()],
                            vec![PathBuf::from("/branch-b")],
                        ),

                        "libshared.so" => (Vec::new(), Vec::new()),

                        other => return Err(format!("unknown contextual recursive node: {other}")),
                    };

                    Ok(crate::domestic_elf::DomesticElfMetadata {
                        path: path.to_path_buf(),
                        interpreter: None,
                        needed,
                        raw_rpath: Vec::new(),
                        raw_runpath: Vec::new(),
                        rpath,
                        runpath: Vec::new(),
                    })
                },
            );

            match closure {
                Ok(states) => Ok(MatrixCaseOutcome::observation(
                    "pass",
                    crate::domestic_observation::DomesticObservation::with_evidence(
                        states
                            .iter()
                            .map(|state| {
                                format!("{}::{:?}", state.path.display(), state.inherited.entries)
                            })
                            .collect(),
                    ),
                )),

                Err(_) => Ok(MatrixCaseOutcome::status("error")),
            }
        }

        "missing_dependency" => {
            let closure = crate::domestic_elf::resolve_elf_recursive_closure(
                &root,
                &root_path,
                &empty_context,
                &controlled_defaults,
                |path| {
                    Ok(crate::domestic_elf::DomesticElfMetadata {
                        path: path.to_path_buf(),
                        interpreter: None,
                        needed: vec!["libmissing.so".to_string()],
                        raw_rpath: Vec::new(),
                        raw_runpath: Vec::new(),
                        rpath: Vec::new(),
                        runpath: Vec::new(),
                    })
                },
            );

            match closure {
                Ok(_) => Ok(MatrixCaseOutcome::status("pass")),

                Err(_) => Ok(MatrixCaseOutcome::status("error")),
            }
        }

        other => Err(format!("unknown recursive ELF closure fixture: {other}")),
    }
}

fn execute_elf_needed_frontier_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let shape = matrix_string(select, "shape")?;

    let root = std::env::temp_dir().join(format!("neebles-needed-frontier-{}", std::process::id()));

    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|error| {
            format!(
                "could not reset NEEDED frontier fixture root {}: {error}",
                root.display()
            )
        })?;
    }

    std::fs::create_dir_all(root.join("private/lib"))
        .map_err(|error| format!("could not create NEEDED frontier private directory: {error}"))?;

    std::fs::create_dir_all(root.join("controlled/lib")).map_err(|error| {
        format!("could not create NEEDED frontier controlled directory: {error}")
    })?;

    std::fs::create_dir_all(root.join("app/bin"))
        .map_err(|error| format!("could not create NEEDED frontier app directory: {error}"))?;

    let elf_path = root.join("app/bin/fixture");

    std::fs::write(&elf_path, b"fixture")
        .map_err(|error| format!("could not create NEEDED frontier fixture path: {error}"))?;

    let (needed, rpath, controlled_defaults) = match shape {
        "two_children" => {
            std::fs::write(root.join("private/lib/libalpha.so"), b"fixture")
                .map_err(|error| format!("could not create alpha dependency: {error}"))?;

            std::fs::write(root.join("controlled/lib/libbeta.so"), b"fixture")
                .map_err(|error| format!("could not create beta dependency: {error}"))?;

            (
                vec!["libalpha.so".to_string(), "libbeta.so".to_string()],
                vec![PathBuf::from("/private/lib")],
                vec![PathBuf::from("/controlled/lib")],
            )
        }

        "no_children" => (
            Vec::new(),
            Vec::new(),
            vec![PathBuf::from("/controlled/lib")],
        ),

        "missing_child" => (
            vec!["libmissing.so".to_string()],
            Vec::new(),
            vec![PathBuf::from("/controlled/lib")],
        ),

        other => {
            return Err(format!("unknown ELF NEEDED frontier fixture: {other}"));
        }
    };

    let metadata = crate::domestic_elf::DomesticElfMetadata {
        path: elf_path,
        interpreter: None,
        needed,
        raw_rpath: Vec::new(),
        raw_runpath: Vec::new(),
        rpath,
        runpath: Vec::new(),
    };

    let inherited = crate::domestic_elf::TransitiveSearchContext {
        source: crate::domestic_elf::TransitiveSearchSource::None,
        entries: Vec::new(),
    };

    match crate::domestic_elf::resolve_needed_authorities(
        &root,
        &metadata,
        &inherited,
        &controlled_defaults,
    ) {
        Ok(resolved) => Ok(MatrixCaseOutcome::observation(
            "pass",
            crate::domestic_observation::DomesticObservation::with_evidence(
                resolved
                    .dependencies
                    .iter()
                    .map(|dependency| dependency.path.display().to_string())
                    .collect(),
            ),
        )),

        Err(_) => Ok(MatrixCaseOutcome::status("error")),
    }
}

fn execute_elf_needed_resolution_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let shape = matrix_string(select, "shape")?;

    let root = std::env::temp_dir().join(format!("neebles-needed-matrix-{}", std::process::id()));

    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|error| {
            format!(
                "could not reset NEEDED fixture root {}: {error}",
                root.display()
            )
        })?;
    }

    std::fs::create_dir_all(root.join("app/bin"))
        .map_err(|error| format!("could not create NEEDED fixture app directory: {error}"))?;

    std::fs::create_dir_all(root.join("controlled/lib")).map_err(|error| {
        format!("could not create NEEDED fixture controlled directory: {error}")
    })?;

    let elf_path = root.join("app/bin/fixture");

    std::fs::write(&elf_path, b"fixture")
        .map_err(|error| format!("could not create NEEDED fixture ELF path: {error}"))?;

    let (rpath, runpath, controlled_defaults, needed) = match shape {
        "runpath_hit" => {
            std::fs::create_dir_all(root.join("private/lib"))
                .map_err(|error| format!("could not create private runpath directory: {error}"))?;

            std::fs::write(root.join("private/lib/libalpha.so"), b"fixture")
                .map_err(|error| format!("could not create runpath candidate: {error}"))?;

            (
                Vec::new(),
                vec![PathBuf::from("/private/lib")],
                Vec::new(),
                "libalpha.so",
            )
        }

        "rpath_hit" => {
            std::fs::create_dir_all(root.join("private/lib"))
                .map_err(|error| format!("could not create private rpath directory: {error}"))?;

            std::fs::write(root.join("private/lib/libalpha.so"), b"fixture")
                .map_err(|error| format!("could not create rpath candidate: {error}"))?;

            (
                vec![PathBuf::from("/private/lib")],
                Vec::new(),
                Vec::new(),
                "libalpha.so",
            )
        }

        "default_hit" => {
            std::fs::write(root.join("controlled/lib/libalpha.so"), b"fixture")
                .map_err(|error| format!("could not create default candidate: {error}"))?;

            (
                Vec::new(),
                Vec::new(),
                vec![PathBuf::from("/controlled/lib")],
                "libalpha.so",
            )
        }

        "missing" => (
            Vec::new(),
            Vec::new(),
            vec![PathBuf::from("/controlled/lib")],
            "libmissing.so",
        ),

        other => {
            return Err(format!("unknown ELF NEEDED resolution fixture: {other}"));
        }
    };

    let metadata = crate::domestic_elf::DomesticElfMetadata {
        path: elf_path,
        interpreter: None,
        needed: vec![needed.to_string()],
        raw_rpath: Vec::new(),
        raw_runpath: Vec::new(),
        rpath,
        runpath,
    };

    let inherited = crate::domestic_elf::TransitiveSearchContext {
        source: crate::domestic_elf::TransitiveSearchSource::None,
        entries: Vec::new(),
    };

    let outcome = match crate::domestic_elf::resolve_needed_authority(
        &root,
        &metadata,
        &inherited,
        &controlled_defaults,
        needed,
    ) {
        Ok(_) => MatrixCaseOutcome::status("pass"),

        Err(_) => MatrixCaseOutcome::status("error"),
    };

    Ok(outcome)
}

fn execute_elf_needed_observation_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let needed_shape = matrix_string(select, "needed_shape")?;

    let needed = match needed_shape {
        "none" => Vec::new(),

        "single" => {
            vec!["libalpha.so".to_string()]
        }

        "multiple" => {
            vec!["libalpha.so".to_string(), "libbeta.so".to_string()]
        }

        "duplicate" => {
            vec!["libalpha.so".to_string(), "libalpha.so".to_string()]
        }

        other => {
            return Err(format!("unknown ELF NEEDED fixture: {other}"));
        }
    };

    let metadata = crate::domestic_elf::DomesticElfMetadata {
        path: PathBuf::from("/world/bin/fixture"),
        interpreter: None,
        needed,
        raw_rpath: Vec::new(),
        raw_runpath: Vec::new(),
        rpath: Vec::new(),
        runpath: Vec::new(),
    };

    let observed = crate::domestic_elf::observed_needed_authorities(&metadata);

    if observed
        .iter()
        .any(|value| !value.starts_with("elf.needed:"))
    {
        return Err("ELF NEEDED observation escaped its namespace".to_string());
    }

    Ok(MatrixCaseOutcome::observation(
        "pass",
        crate::domestic_observation::DomesticObservation::with_evidence(observed),
    ))
}

fn execute_observed_resolution_authority_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let declaration = matrix_string(select, "declaration")?;

    let observation = matrix_string(select, "observation")?;

    let observed = match observation {
        "single" => {
            vec!["authority.alpha".to_string()]
        }

        "multiple" => {
            vec!["authority.alpha".to_string(), "authority.beta".to_string()]
        }

        other => {
            return Err(format!("unknown observed authority fixture: {other}"));
        }
    };

    let declared = match declaration {
        "exact" => observed.clone(),

        "superset" => {
            let mut values = observed.clone();

            values.push("authority.unused".to_string());

            values
        }

        "missing_observed" => match observation {
            "single" => Vec::new(),

            "multiple" => {
                vec!["authority.alpha".to_string()]
            }

            other => {
                return Err(format!(
                    "unknown observation while building declaration fixture: {other}"
                ));
            }
        },

        other => {
            return Err(format!("unknown authority declaration fixture: {other}"));
        }
    };

    let result = crate::domestic_authority::certify_observed_authorities(&declared, &observed);

    Ok(if result.is_ok() {
        MatrixCaseOutcome::status("pass")
    } else {
        MatrixCaseOutcome::status("error")
    })
}

fn execute_world_reference_resolution_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    let reference_form = matrix_string(select, "reference_form")?;

    let target_state = matrix_string(select, "target_state")?;

    let binding = matrix_string(select, "binding")?;

    let unique = format!(
        "neebles-world-reference-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                format!("system clock failed while building world fixture: {error}")
            },)?
            .as_nanos()
    );

    let base = std::env::temp_dir().join(unique);

    let world = base.join("world");

    let inside = world.join("usr/lib/neebles");

    let outside = base.join("outside");

    fs::create_dir_all(&inside)
        .map_err(|error| format!("could not create world fixture: {error}"))?;

    fs::create_dir_all(&outside)
        .map_err(|error| format!("could not create outside fixture: {error}"))?;

    let reference = match reference_form {
        "absolute" => PathBuf::from("/usr/lib/neebles/target.so"),

        "relative" => PathBuf::from("target.so"),

        "parent_escape" => PathBuf::from("../../../../outside/target.so"),

        other => {
            return Err(format!("unknown world reference form: {other}"));
        }
    };

    let reference_path = match reference_form {
        "absolute" => world.join("usr/lib/neebles/target.so"),

        _ => inside.join(&reference),
    };

    if target_state == "present" && binding == "direct" && reference_form != "parent_escape" {
        fs::write(&reference_path, b"domestic-target\n")
            .map_err(|error| format!("could not create direct target: {error}"))?;
    }

    if target_state == "present" && binding == "direct" && reference_form == "parent_escape" {
        fs::write(outside.join("target.so"), b"outside-target\n")
            .map_err(|error| format!("could not create outside direct target: {error}"))?;
    }

    if binding == "symlink_inside" {
        let real = world.join("usr/lib/neebles/inside-real.so");

        fs::write(&real, b"inside-real\n")
            .map_err(|error| format!("could not create inside symlink target: {error}"))?;

        symlink("/usr/lib/neebles/inside-real.so", &reference_path)
            .map_err(|error| format!("could not create inside symlink: {error}"))?;
    }

    if binding == "symlink_outside" {
        let real = outside.join("outside-real.so");

        fs::write(&real, b"outside-real\n")
            .map_err(|error| format!("could not create outside symlink target: {error}"))?;

        symlink("../../../outside/outside-real.so", &reference_path)
            .map_err(|error| format!("could not create escaping symlink: {error}"))?;
    }

    if binding == "symlink_broken" {
        symlink("missing-real.so", &reference_path)
            .map_err(|error| format!("could not create broken symlink fixture: {error}"))?;
    }

    let result = match binding {
        "direct" => crate::domestic_world::resolve_world_reference(&world, &inside, &reference),

        "symlink_inside" | "symlink_outside" | "symlink_broken" => {
            crate::domestic_world::resolve_world_symlink(&world, &reference_path)
        }

        other => {
            return Err(format!("unknown world reference binding: {other}"));
        }
    };

    let outcome = if result.is_ok() {
        MatrixCaseOutcome::status("pass")
    } else {
        MatrixCaseOutcome::status("error")
    };

    fs::remove_dir_all(&base)
        .map_err(|error| format!("could not remove world fixture: {error}"))?;

    Ok(outcome)
}

fn execute_relative_reference_transformation_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let origin_mode = matrix_string(select, "origin")?;

    let target_state = matrix_string(select, "target_state")?;

    if !matches!(target_state, "present" | "missing") {
        return Err(format!(
            "unknown relative transformation target state: {target_state}"
        ));
    }

    let unique = format!(
        "neebles-relative-reference-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                format!("system clock failed while building relative fixture: {error}")
            },)?
            .as_nanos()
    );

    let base = std::env::temp_dir().join(unique);

    let world = base.join("world");

    let outside = base.join("outside");

    fs::create_dir_all(&world)
        .map_err(|error| format!("could not create relative world root: {error}"))?;

    fs::create_dir_all(&outside)
        .map_err(|error| format!("could not create relative outside root: {error}"))?;

    let (origin, target) = match origin_mode {
        "same_directory" => (world.join("usr/lib/neebles"), world.join("usr/lib/neebles")),

        "parent_directory" => (world.join("usr/lib/neebles"), world.join("usr/lib")),

        "child_directory" => (
            world.join("usr/lib/neebles"),
            world.join("usr/lib/neebles/plugins"),
        ),

        "sibling_directory" => (world.join("usr/lib/neebles"), world.join("usr/lib/bin")),

        "deep_directory" => (world.join("tree/a/b/c"), world.join("tree/share")),

        "outside_world" => (world.join("usr/lib/neebles"), outside.join("lib")),

        other => {
            let _ = fs::remove_dir_all(&base);

            return Err(format!(
                "unknown relative transformation origin fixture: {other}"
            ));
        }
    };

    fs::create_dir_all(&origin)
        .map_err(|error| format!("could not create relative origin: {error}"))?;

    if target_state == "present" {
        fs::create_dir_all(&target)
            .map_err(|error| format!("could not create relative target: {error}"))?;
    }

    let outcome = (|| {
        if !target.exists() {
            return Ok(MatrixCaseOutcome::status("error"));
        }

        let relation =
            match crate::domestic_world::relative_world_relation(&world, &origin, &target) {
                Ok(value) => value,

                Err(_) => {
                    return Ok(MatrixCaseOutcome::status("error"));
                }
            };

        Ok(MatrixCaseOutcome::status(&relation.to_string_lossy()))
    })();

    let cleanup = fs::remove_dir_all(&base);

    if let Err(error) = cleanup {
        return Err(format!(
            "could not remove relative transformation fixture: {error}"
        ));
    }

    outcome
}

fn execute_search_reference_projection_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    let form = matrix_string(select, "form")?;

    let origin = format!("{}ORIGIN", 36u8 as char);

    let raw = match form {
        "absolute" => "/usr/lib/x86_64-linux-gnu/libproxy".to_string(),

        "origin_exact" => origin,

        "origin_child" => {
            format!("{}/libproxy", origin)
        }

        "bare_relative" => "libproxy".to_string(),

        "unknown_token" => {
            format!("{}LIB/libproxy", 36u8 as char)
        }

        "search_list_with_empty_component" => {
            let parsed = crate::domestic_elf::parse_search_path_entries("/first::/second");

            if parsed.len() != 3 {
                return Err(format!(
                    "search parser lost component count: {}",
                    parsed.len()
                ));
            }

            if !parsed[1].as_os_str().is_empty() {
                return Err("search parser did not preserve empty evidence".to_string());
            }

            parsed[1].to_string_lossy().into_owned()
        }

        "raw_empty_sequence" => {
            return match crate::domestic_elf::project_search_path_entries("::::::::::::::::::::::")
            {
                Ok(_) => Ok(MatrixCaseOutcome::status("pass")),
                Err(_) => Ok(MatrixCaseOutcome::status("error")),
            };
        }

        other => {
            return Err(format!("unknown search reference projection form: {other}"));
        }
    };

    match crate::domestic_elf::project_search_reference(&raw) {
        Ok(_) => Ok(MatrixCaseOutcome::status("pass")),

        Err(_) => Ok(MatrixCaseOutcome::status("error")),
    }
}

fn execute_search_candidate_resolution_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    let placement = matrix_string(select, "placement")?;

    let search_authority = matrix_string(select, "search_authority")?;

    if !matches!(
        placement,
        "first" | "second" | "missing" | "outside_symlink"
    ) {
        return Err(format!("unknown search candidate placement: {placement}"));
    }

    if !matches!(search_authority, "world_absolute" | "elf_origin") {
        return Err(format!(
            "unknown search candidate authority: {search_authority}"
        ));
    }

    let unique = format!(
        "neebles-search-candidate-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                format!("system clock failed while building candidate fixture: {error}")
            },)?
            .as_nanos()
    );

    let base = std::env::temp_dir().join(unique);

    let world = base.join("world");

    let outside = base.join("outside");

    let elf_directory = world.join("app/bin");

    let elf = elf_directory.join("prisoner");

    fs::create_dir_all(&elf_directory)
        .map_err(|error| format!("could not create ELF fixture directory: {error}"))?;

    fs::create_dir_all(&outside)
        .map_err(|error| format!("could not create outside fixture directory: {error}"))?;

    fs::write(&elf, b"fixture-elf\n")
        .map_err(|error| format!("could not create ELF fixture: {error}"))?;

    let needed = "libfixture.so";

    let (first_directory, second_directory, raw_searches) = match search_authority {
        "world_absolute" => (
            world.join("usr/lib/first"),
            world.join("usr/lib/second"),
            vec!["/usr/lib/first".to_string(), "/usr/lib/second".to_string()],
        ),

        "elf_origin" => {
            let origin = format!("{}ORIGIN", 36u8 as char);

            (
                elf_directory.join("first"),
                elf_directory.join("second"),
                vec![format!("{}/first", origin), format!("{}/second", origin)],
            )
        }

        _ => unreachable!(),
    };

    fs::create_dir_all(&first_directory)
        .map_err(|error| format!("could not create first search directory: {error}"))?;

    fs::create_dir_all(&second_directory)
        .map_err(|error| format!("could not create second search directory: {error}"))?;

    let first_candidate = first_directory.join(needed);

    let second_candidate = second_directory.join(needed);

    match placement {
        "first" => {
            fs::write(&first_candidate, b"first\n")
                .map_err(|error| format!("could not create first search candidate: {error}"))?;

            fs::write(&second_candidate, b"second\n")
                .map_err(|error| format!("could not create second search candidate: {error}"))?;
        }

        "second" => {
            fs::write(&second_candidate, b"second\n")
                .map_err(|error| format!("could not create second search candidate: {error}"))?;
        }

        "missing" => {}

        "outside_symlink" => {
            let outside_candidate = outside.join(needed);

            fs::write(&outside_candidate, b"outside\n")
                .map_err(|error| format!("could not create outside candidate: {error}"))?;

            symlink(&outside_candidate, &first_candidate)
                .map_err(|error| format!("could not create escaping candidate symlink: {error}"))?;

            fs::write(&second_candidate, b"legal-second\n")
                .map_err(|error| format!("could not create legal second candidate: {error}"))?;
        }

        _ => unreachable!(),
    }

    let projected = raw_searches
        .iter()
        .map(|raw| crate::domestic_elf::project_search_reference(raw))
        .collect::<Result<Vec<_>, _>>()?;

    let outcome =
        match crate::domestic_elf::resolve_search_candidate(&world, &elf, &projected, needed) {
            Ok(resolved) => match resolved.search_index {
                0 => MatrixCaseOutcome::status("first"),

                1 => MatrixCaseOutcome::status("second"),

                other => {
                    return Err(format!("unexpected resolved search index: {other}"));
                }
            },

            Err(_) => MatrixCaseOutcome::status("error"),
        };

    fs::remove_dir_all(&base)
        .map_err(|error| format!("could not remove candidate fixture: {error}"))?;

    Ok(outcome)
}

fn execute_direct_search_policy_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use std::path::PathBuf;

    let metadata_case = matrix_string(select, "metadata")?;

    let (rpath, runpath) = match metadata_case {
        "rpath_only" => (
            vec![
                PathBuf::from("/rpath/first"),
                PathBuf::from("/rpath/second"),
            ],
            Vec::new(),
        ),

        "runpath_only" => (
            Vec::new(),
            vec![
                PathBuf::from("/runpath/first"),
                PathBuf::from("/runpath/second"),
            ],
        ),

        "both" => (
            vec![PathBuf::from("/rpath/ignored")],
            vec![PathBuf::from("/runpath/selected")],
        ),

        "none" => (Vec::new(), Vec::new()),

        other => {
            return Err(format!("unknown direct search metadata case: {other}"));
        }
    };

    let metadata = crate::domestic_elf::DomesticElfMetadata {
        path: PathBuf::from("/world/prisoner"),
        interpreter: None,
        needed: Vec::new(),
        raw_rpath: Vec::new(),
        raw_runpath: Vec::new(),
        rpath,
        runpath,
    };

    let policy = crate::domestic_elf::direct_search_policy(&metadata);

    let status = match policy.source {
        crate::domestic_elf::DirectSearchSource::Rpath => "rpath",

        crate::domestic_elf::DirectSearchSource::Runpath => "runpath",

        crate::domestic_elf::DirectSearchSource::None => "none",
    };

    Ok(MatrixCaseOutcome::status(status))
}

fn execute_transitive_search_inheritance_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use std::path::PathBuf;

    let parent_source = matrix_string(select, "parent_source")?;

    let depth = matrix_string(select, "depth")?;

    if !matches!(depth, "child" | "descendant") {
        return Err(format!("unknown transitive inheritance depth: {depth}"));
    }

    let metadata = match parent_source {
        "rpath" => crate::domestic_elf::DomesticElfMetadata {
            path: PathBuf::from("/world/parent"),
            interpreter: None,
            needed: Vec::new(),
            raw_rpath: Vec::new(),
            raw_runpath: Vec::new(),
            rpath: vec![
                PathBuf::from("/inherited/first"),
                PathBuf::from("/inherited/second"),
            ],
            runpath: Vec::new(),
        },

        "runpath" => crate::domestic_elf::DomesticElfMetadata {
            path: PathBuf::from("/world/parent"),
            interpreter: None,
            needed: Vec::new(),
            raw_rpath: Vec::new(),
            raw_runpath: Vec::new(),
            rpath: Vec::new(),
            runpath: vec![PathBuf::from("/noninherited")],
        },

        "none" => crate::domestic_elf::DomesticElfMetadata {
            path: PathBuf::from("/world/parent"),
            interpreter: None,
            needed: Vec::new(),
            raw_rpath: Vec::new(),
            raw_runpath: Vec::new(),
            rpath: Vec::new(),
            runpath: Vec::new(),
        },

        other => {
            return Err(format!("unknown transitive parent source: {other}"));
        }
    };

    let direct = crate::domestic_elf::direct_search_policy(&metadata);

    let inherited = crate::domestic_elf::transitive_search_inheritance(&direct);

    let status = match inherited.source {
        crate::domestic_elf::TransitiveSearchSource::Rpath => {
            if inherited.entries.is_empty() {
                return Err("RPATH inheritance produced empty context".to_string());
            }

            "inherited"
        }

        crate::domestic_elf::TransitiveSearchSource::None => {
            if !inherited.entries.is_empty() {
                return Err("non-inherited context unexpectedly carried entries".to_string());
            }

            "none"
        }
    };

    Ok(MatrixCaseOutcome::status(status))
}

fn execute_effective_search_composition_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use std::path::PathBuf;

    let inherited_case = matrix_string(select, "inherited")?;

    let current_case = matrix_string(select, "current")?;

    let inherited = match inherited_case {
        "rpath" => crate::domestic_elf::TransitiveSearchContext {
            source: crate::domestic_elf::TransitiveSearchSource::Rpath,
            entries: vec![PathBuf::from("/inherited")],
        },

        "none" => crate::domestic_elf::TransitiveSearchContext {
            source: crate::domestic_elf::TransitiveSearchSource::None,
            entries: Vec::new(),
        },

        other => {
            return Err(format!("unknown inherited search context: {other}"));
        }
    };

    let current = match current_case {
        "rpath" => crate::domestic_elf::DirectSearchPolicy {
            source: crate::domestic_elf::DirectSearchSource::Rpath,
            entries: vec![PathBuf::from("/current-rpath")],
        },

        "runpath" => crate::domestic_elf::DirectSearchPolicy {
            source: crate::domestic_elf::DirectSearchSource::Runpath,
            entries: vec![PathBuf::from("/current-runpath")],
        },

        "none" => crate::domestic_elf::DirectSearchPolicy {
            source: crate::domestic_elf::DirectSearchSource::None,
            entries: Vec::new(),
        },

        other => {
            return Err(format!("unknown current search policy: {other}"));
        }
    };

    let effective = crate::domestic_elf::effective_search_composition(&inherited, &current);

    let inherited_path = PathBuf::from("/inherited");

    let current_rpath = PathBuf::from("/current-rpath");

    let current_runpath = PathBuf::from("/current-runpath");

    let status = match (inherited_case, current_case) {
        ("rpath", "none") => {
            if effective.entries != vec![inherited_path.clone()] {
                return Err("inherited-only effective order is wrong".to_string());
            }

            if effective.next_transitive.entries != vec![inherited_path] {
                return Err("inherited context did not survive".to_string());
            }

            "inherited"
        }

        ("none", "rpath") => {
            if effective.entries != vec![current_rpath.clone()] {
                return Err("current RPATH effective order is wrong".to_string());
            }

            if effective.next_transitive.entries != vec![current_rpath] {
                return Err("current RPATH did not become transitive".to_string());
            }

            "current_rpath"
        }

        ("none", "runpath") => {
            if effective.entries != vec![current_runpath] {
                return Err("current RUNPATH effective order is wrong".to_string());
            }

            if !effective.next_transitive.entries.is_empty() {
                return Err("RUNPATH incorrectly became transitive".to_string());
            }

            "current_runpath"
        }

        ("rpath", "rpath") => {
            if effective.entries != vec![current_rpath.clone(), inherited_path.clone()] {
                return Err("current plus inherited RPATH order is wrong".to_string());
            }

            if effective.next_transitive.entries != vec![current_rpath, inherited_path] {
                return Err("combined RPATH context was not preserved".to_string());
            }

            "current_then_inherited"
        }

        ("rpath", "runpath") => {
            if effective.entries != vec![inherited_path.clone(), current_runpath] {
                return Err("inherited plus RUNPATH order is wrong".to_string());
            }

            if effective.next_transitive.entries != vec![inherited_path] {
                return Err("RUNPATH contaminated transitive context".to_string());
            }

            "inherited_then_runpath"
        }

        ("none", "none") => {
            if !effective.entries.is_empty() {
                return Err("empty search composition produced entries".to_string());
            }

            if !effective.next_transitive.entries.is_empty() {
                return Err("empty search composition produced inheritance".to_string());
            }

            "none"
        }

        _ => {
            return Err(format!(
                "undeclared effective search combination: {inherited_case} + {current_case}"
            ));
        }
    };

    Ok(MatrixCaseOutcome::status(status))
}

fn execute_recursive_closure_certification_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use std::collections::BTreeMap;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    let topology = matrix_string(select, "topology")?;

    let closure_state = matrix_string(select, "closure_state")?;

    if !matches!(topology, "linear" | "branch" | "cycle") {
        return Err(format!("unknown recursive closure topology: {topology}"));
    }

    if !matches!(closure_state, "sealed" | "missing_leaf" | "outside_leaf") {
        return Err(format!("unknown recursive closure state: {closure_state}"));
    }

    let unique = format!(
        "neebles-recursive-closure-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                format!("system clock failed while building closure fixture: {error}")
            },)?
            .as_nanos()
    );

    let base = std::env::temp_dir().join(unique);

    let world = base.join("world");

    let outside = base.join("outside");

    let library = world.join("usr/lib/neebles");

    fs::create_dir_all(&library)
        .map_err(|error| format!("could not create closure world: {error}"))?;

    fs::create_dir_all(&outside)
        .map_err(|error| format!("could not create closure outside root: {error}"))?;

    let mut graph = BTreeMap::<PathBuf, Vec<PathBuf>>::new();

    let a = PathBuf::from("a.so");

    let b = PathBuf::from("b.so");

    let c = PathBuf::from("c.so");

    let d = PathBuf::from("d.so");

    let e = PathBuf::from("e.so");

    match topology {
        "linear" => {
            graph.insert(a.clone(), vec![b.clone()]);

            graph.insert(b.clone(), vec![c.clone()]);

            graph.insert(c.clone(), Vec::new());
        }

        "branch" => {
            graph.insert(a.clone(), vec![b.clone(), c.clone()]);

            graph.insert(b.clone(), vec![d.clone()]);

            graph.insert(c.clone(), vec![e.clone()]);

            graph.insert(d.clone(), Vec::new());

            graph.insert(e.clone(), Vec::new());
        }

        "cycle" => {
            graph.insert(a.clone(), vec![b.clone()]);

            graph.insert(b.clone(), vec![c.clone()]);

            graph.insert(c.clone(), vec![a.clone()]);
        }

        _ => unreachable!(),
    }

    if closure_state == "missing_leaf" {
        let missing = PathBuf::from("missing.so");

        match topology {
            "linear" => {
                graph.insert(b.clone(), vec![missing]);
            }

            "branch" => {
                graph.insert(c.clone(), vec![missing]);
            }

            "cycle" => {
                graph
                    .get_mut(&c)
                    .ok_or_else(|| "cycle fixture missing c node".to_string())?
                    .push(missing);
            }

            _ => unreachable!(),
        }
    }

    for node in graph.keys() {
        let target = library.join(node);

        fs::write(&target, b"domestic-node\n").map_err(|error| {
            format!(
                "could not create closure node {}: {error}",
                target.display()
            )
        })?;
    }

    if closure_state == "outside_leaf" {
        let outside_target = outside.join("rogue.so");

        fs::write(&outside_target, b"outside-world\n")
            .map_err(|error| format!("could not create outside closure leaf: {error}"))?;

        let internal_link = library.join("rogue.so");

        symlink(&outside_target, &internal_link)
            .map_err(|error| format!("could not create escaping closure symlink: {error}"))?;

        let owner = match topology {
            "linear" => b.clone(),
            "branch" => c.clone(),
            "cycle" => c.clone(),
            _ => unreachable!(),
        };

        graph
            .get_mut(&owner)
            .ok_or_else(|| format!("closure fixture missing owner {}", owner.display()))?
            .push(PathBuf::from("rogue.so"));
    }

    let mut canonical_graph = BTreeMap::<PathBuf, Vec<PathBuf>>::new();

    for (node, references) in &graph {
        let canonical = library.join(node).canonicalize().map_err(|error| {
            format!(
                "could not canonicalize closure node {}: {error}",
                node.display()
            )
        })?;

        canonical_graph.insert(canonical, references.clone());
    }

    let root = library
        .join(&a)
        .canonicalize()
        .map_err(|error| format!("could not canonicalize closure root: {error}"))?;

    let outcome = match crate::domestic_world::certify_recursive_closure(&world, &root, |current| {
        Ok(canonical_graph.get(current).cloned().unwrap_or_default())
    }) {
        Ok(_) => MatrixCaseOutcome::status("pass"),

        Err(_) => MatrixCaseOutcome::status("error"),
    };

    let cleanup = fs::remove_dir_all(&base);

    if let Err(error) = cleanup {
        return Err(format!(
            "could not remove recursive closure fixture: {error}"
        ));
    }

    Ok(outcome)
}

fn execute_workspace_execution_case(
    select: &serde_json::Map<String, Value>,
) -> Result<MatrixCaseOutcome, String> {
    use crate::domestic_workspace_execution::{
        validate_workspace_execution_request, WorkspaceExecutionRequest, WorkspaceReadonlyGrant,
        WorkspaceWritableGrant,
    };
    use std::ffi::OsString;

    let world = matrix_string(select, "world")?;
    let manifest_path = matrix_string(select, "manifest_path")?;
    let platform_path = matrix_string(select, "platform_path")?;
    let readonly = matrix_string(select, "readonly")?;
    let writable = matrix_string(select, "writable")?;
    let chdir = matrix_string(select, "chdir")?;
    let arguments = matrix_string(select, "arguments")?;

    let world = match world {
        "valid" => "boss.fixture.tool".to_string(),
        "empty" => String::new(),
        other => {
            return Err(format!(
                "unknown workspace execution world fixture: {other}"
            ));
        }
    };

    let manifest_path = match manifest_path {
        "absolute" => PathBuf::from("/fixture/runtime.json"),
        "relative" => PathBuf::from("runtime.json"),
        other => {
            return Err(format!("unknown workspace manifest path fixture: {other}"));
        }
    };

    let platform_descriptor_path = match platform_path {
        "absolute" => PathBuf::from("/fixture/platform.json"),
        "relative" => PathBuf::from("platform.json"),
        other => {
            return Err(format!("unknown workspace platform path fixture: {other}"));
        }
    };

    let readonly = match readonly {
        "zero" => Vec::new(),

        "multiple" => vec![
            WorkspaceReadonlyGrant {
                authority: "fixture.readonly.alpha".to_string(),
                descriptor_path: PathBuf::from("/fixture/readonly-alpha.json"),
            },
            WorkspaceReadonlyGrant {
                authority: "fixture.readonly.beta".to_string(),
                descriptor_path: PathBuf::from("/fixture/readonly-beta.json"),
            },
        ],

        other => {
            return Err(format!("unknown workspace readonly fixture: {other}"));
        }
    };

    let writable = match writable {
        "zero" => Vec::new(),

        "multiple" => vec![
            WorkspaceWritableGrant {
                authority: "fixture.writable.alpha".to_string(),
                descriptor_path: PathBuf::from("/fixture/writable-alpha.json"),
                source: PathBuf::from("/fixture/staging/alpha"),
                destination: PathBuf::from("/work/alpha"),
            },
            WorkspaceWritableGrant {
                authority: "fixture.writable.beta".to_string(),
                descriptor_path: PathBuf::from("/fixture/writable-beta.json"),
                source: PathBuf::from("/fixture/staging/beta"),
                destination: PathBuf::from("/work/beta"),
            },
        ],

        other => {
            return Err(format!("unknown workspace writable fixture: {other}"));
        }
    };

    let chdir = match chdir {
        "absent" => None,
        "absolute" => Some(PathBuf::from("/work")),
        "relative" => Some(PathBuf::from("work")),
        other => {
            return Err(format!("unknown workspace chdir fixture: {other}"));
        }
    };

    let arguments = match arguments {
        "none" => Vec::new(),

        "opaque_multiple" => vec![
            OsString::from("--opaque-one"),
            OsString::from("value with spaces"),
            OsString::from("--opaque-two=literal"),
        ],

        other => {
            return Err(format!("unknown workspace arguments fixture: {other}"));
        }
    };

    let readonly_count = readonly.len();
    let writable_count = writable.len();
    let argument_count = arguments.len();
    let chdir_state = if chdir.is_some() { "present" } else { "absent" };

    let request = WorkspaceExecutionRequest {
        manifest_path,
        world,
        platform_descriptor_path,
        readonly,
        writable,
        mount_proc: false,
        mount_dev: false,
        mount_tmp: false,
        chdir,
        arguments,
    };

    match validate_workspace_execution_request(&request) {
        Ok(()) => {
            let evidence = vec![
                format!("readonly:{readonly_count}"),
                format!("writable:{writable_count}"),
                format!("arguments:{argument_count}"),
                format!("chdir:{chdir_state}"),
            ];

            Ok(MatrixCaseOutcome::observation(
                "pass",
                crate::domestic_observation::DomesticObservation::with_evidence(evidence),
            ))
        }

        Err(_) => Ok(MatrixCaseOutcome::status("error")),
    }
}

pub fn execute_tester_matrix(
    contract_path: &Path,
    tester_path: &Path,
    expected_category: &str,
) -> Result<MatrixExecutionReport, String> {
    validate_tester_definition(tester_path, expected_category)?;

    let raw = fs::read_to_string(tester_path).map_err(|error| {
        format!(
            "could not read tester matrix {}: {error}",
            tester_path.display()
        )
    })?;

    let definition: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid tester matrix {}: {error}", tester_path.display()))?;

    let spells = definition
        .get("spells")
        .and_then(Value::as_array)
        .ok_or_else(|| "tester spells missing".to_string())?;

    let mut executed = Vec::<String>::new();

    for spell_value in spells {
        let spell = spell_value
            .as_object()
            .ok_or_else(|| "tester spell must be object".to_string())?;

        let spell_reference = spell
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "tester spell name must be String".to_string())?;

        let parsed_spell = crate::spell_world::SpellReference::parse(spell_reference)?;

        let spell_name = parsed_spell.spell.as_str();

        let cases = spell
            .get("cases")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("tester spell {spell_name} cases missing"))?;

        for case_value in cases {
            let case = case_value
                .as_object()
                .ok_or_else(|| format!("tester spell {spell_name} case must be object"))?;

            let id = matrix_string(case, "id")?;

            let expected = matrix_string(case, "expect")?;

            let select = case
                .get("select")
                .and_then(Value::as_object)
                .ok_or_else(|| format!("tester spell {spell_name} case {id} select missing"))?;

            let outcome = match spell_name {
                "physical" => execute_physical_case(contract_path, expected_category, select)?,

                "path_contract" => {
                    execute_path_contract_case(contract_path, expected_category, select)?
                }

                "call_contract" => execute_call_contract_case(select)?,

                "argument_contract" => execute_argument_contract_case(select)?,

                "reference_requirement" => execute_reference_requirement_case(select)?,

                "transport_reference" => execute_transport_reference_case(select)?,

                "runtime_authority" => execute_runtime_authority_case(select)?,

                "process_environment" => execute_environment_case(select)?,

                "session_policy" => execute_session_policy_case(select)?,

                "raw_environment_parser" => execute_raw_environment_parser_case(select)?,

                "capability_provider" => execute_capability_provider_case(select)?,

                "command_sealing" => execute_command_sealing_case(select)?,

                "selection_semantics" => execute_session_selection_case(select)?,

                "forbidden_catalog" => execute_forbidden_session_key_case(select)?,

                "observation_contract" => execute_observation_contract_case(select)?,
                "elf_interpreter_resolution" => execute_elf_interpreter_resolution_case(select)?,
                "elf_interpreter_observation" => execute_elf_interpreter_observation_case(select)?,
                "elf_needed_resolution" => execute_elf_needed_resolution_case(select)?,
                "elf_needed_frontier" => execute_elf_needed_frontier_case(select)?,

                "elf_recursive_closure" => execute_elf_recursive_closure_case(select)?,
                "elf_needed_observation" => execute_elf_needed_observation_case(select)?,
                "observed_resolution_authority" => {
                    execute_observed_resolution_authority_case(select)?
                }
                "world_reference_resolution" => execute_world_reference_resolution_case(select)?,

                "relative_reference_transformation" => {
                    execute_relative_reference_transformation_case(select)?
                }

                "recursive_closure_certification" => {
                    execute_recursive_closure_certification_case(select)?
                }

                "search_reference_projection" => execute_search_reference_projection_case(select)?,

                "search_candidate_resolution" => execute_search_candidate_resolution_case(select)?,

                "direct_search_policy" => execute_direct_search_policy_case(select)?,

                "transitive_search_inheritance" => {
                    execute_transitive_search_inheritance_case(select)?
                }

                "effective_search_composition" => {
                    execute_effective_search_composition_case(select)?
                }

                "search_context_normalization" => {
                    execute_search_context_normalization_case(select)?
                }

                "search_authority_grants" => execute_search_authority_grant_case(select)?,

                "effective_search_authority_projection" => {
                    execute_effective_search_authority_case(select)?
                }

                "workspace_execution" => execute_workspace_execution_case(select)?,

                other => {
                    return Err(format!("matrix runner does not know generic spell {other}"));
                }
            };

            if outcome.status != expected {
                return Err(
                    format!(
                        "matrix case {expected_category}/{spell_name}/{id} expected {expected} but produced {}",
                        outcome.status
                    )
                );
            }

            apply_matrix_assertions(&outcome, case.get("assert")).map_err(|error| {
                format!(
                    "matrix case {expected_category}/{spell_name}/{id} assertion failed: {error}"
                )
            })?;

            executed.push(format!("{spell_reference}/{id}"));
        }
    }

    Ok(MatrixExecutionReport {
        category: expected_category.to_string(),

        executed: executed.len(),

        cases: executed,
    })
}

use super::LocalInstallerRequest;
use serde::Serialize;
use serde_json::Value;
use std::process::Command;

const CURRENT_ARCHITECTURE: &str = "amd64";
const CURRENT_DISTRIBUTION: &str = "debian";

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedOperation {
    pub architecture: String,
    pub distribution: String,
    pub installer: String,
    pub operation: String,
    pub command: String,
    pub args: Vec<String>,
    pub root_mode: String,
    pub expect: Option<Value>,
}

pub fn resolve(
    request: &LocalInstallerRequest,
) -> Result<ResolvedOperation, String> {
    let dictionary = fetch_dictionary()?;

    validate_architecture(&dictionary)?;

    let distributions = dictionary
        .get("distributions")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "installer dictionary does not contain a valid 'distributions' object"
                .to_string()
        })?;

    let distribution = distributions
        .get(CURRENT_DISTRIBUTION)
        .ok_or_else(|| {
            format!(
                "distribution '{}' does not exist in installer dictionary",
                CURRENT_DISTRIBUTION
            )
        })?;

    let installers = distribution
        .get("installers")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "distribution '{}' does not contain a valid installers object",
                CURRENT_DISTRIBUTION
            )
        })?;

    let installer = installers
        .get(&request.installer)
        .ok_or_else(|| {
            format!(
                "installer '{}' does not exist for distribution '{}'",
                request.installer,
                CURRENT_DISTRIBUTION
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

    let operation = operations
        .get(&request.operation)
        .ok_or_else(|| {
            format!(
                "operation '{}' does not exist for installer '{}'",
                request.operation,
                request.installer
            )
        })?;

    let command = operation
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "operation '{}:{}' does not define a command",
                request.installer,
                request.operation
            )
        })?
        .to_string();

    let flow = operation
        .get("flow")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "operation '{}:{}' does not define a flow array",
                request.installer,
                request.operation
            )
        })?;

    let mut args = Vec::new();

    for item in flow {
        resolve_flow_item(
            item,
            request,
            &mut args,
        )?;
    }

    let root_mode = operation
        .get("root_mode")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "operation '{}:{}' does not define root_mode",
                request.installer,
                request.operation
            )
        })?
        .to_string();

    let expect =
        operation.get("expect").cloned();

    Ok(ResolvedOperation {
        architecture:
            CURRENT_ARCHITECTURE.to_string(),
        distribution:
            CURRENT_DISTRIBUTION.to_string(),
        installer:
            request.installer.clone(),
        operation:
            request.operation.clone(),
        command,
        args,
        root_mode,
        expect,
    })
}

fn validate_architecture(
    dictionary: &Value,
) -> Result<(), String> {
    let architecture = dictionary
        .get("architecture")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "installer dictionary does not declare architecture"
                .to_string()
        })?;

    if architecture != CURRENT_ARCHITECTURE {
        return Err(format!(
            "installer dictionary architecture '{}' does not match Boss architecture '{}'",
            architecture,
            CURRENT_ARCHITECTURE
        ));
    }

    Ok(())
}

fn resolve_flow_item(
    item: &Value,
    request: &LocalInstallerRequest,
    args: &mut Vec<String>,
) -> Result<(), String> {
    let object = item
        .as_object()
        .ok_or_else(|| {
            format!(
                "operation '{}:{}' contains a non-object flow item",
                request.installer,
                request.operation
            )
        })?;

    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "operation '{}:{}' contains a flow item without kind",
                request.installer,
                request.operation
            )
        })?;

    match kind {
        "subcommand"
        | "flag"
        | "literal"
        | "separator" => {
            let value = object
                .get("value")
                .ok_or_else(|| {
                    format!(
                        "flow kind '{}' requires value",
                        kind
                    )
                })?;

            args.push(
                scalar_to_string(
                    value,
                    kind,
                )?
            );
        }

        "operand" => {
            let value =
                resolve_value_or_source(
                    object,
                    request,
                    kind,
                )?;

            args.push(
                scalar_to_string(
                    value,
                    kind,
                )?
            );
        }

        "operand_list" => {
            let source = object
                .get("source")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    "flow kind 'operand_list' requires source"
                        .to_string()
                })?;

            let value = request
                .variables
                .get(source)
                .ok_or_else(|| {
                    format!(
                        "missing variable '{}' for installer '{}', operation '{}'",
                        source,
                        request.installer,
                        request.operation
                    )
                })?;

            let values = value
                .as_array()
                .ok_or_else(|| {
                    format!(
                        "variable '{}' must be an array for operand_list",
                        source
                    )
                })?;

            for value in values {
                args.push(
                    scalar_to_string(
                        value,
                        kind,
                    )?
                );
            }
        }

        "option" => {
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    "flow kind 'option' requires name"
                        .to_string()
                })?;

            let value =
                resolve_value_or_source(
                    object,
                    request,
                    kind,
                )?;

            args.push(name.to_string());
            args.push(
                scalar_to_string(
                    value,
                    kind,
                )?
            );
        }

        other => {
            return Err(format!(
                "unsupported flow kind '{}' for installer '{}', operation '{}'",
                other,
                request.installer,
                request.operation
            ));
        }
    }

    Ok(())
}

fn resolve_value_or_source<'a>(
    object: &'a serde_json::Map<String, Value>,
    request: &'a LocalInstallerRequest,
    kind: &str,
) -> Result<&'a Value, String> {
    if let Some(value) =
        object.get("value")
    {
        return Ok(value);
    }

    let source = object
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            format!(
                "flow kind '{}' requires value or source",
                kind
            )
        })?;

    request
        .variables
        .get(source)
        .ok_or_else(|| {
            format!(
                "missing variable '{}' for installer '{}', operation '{}'",
                source,
                request.installer,
                request.operation
            )
        })
}

fn scalar_to_string(
    value: &Value,
    context: &str,
) -> Result<String, String> {
    match value {
        Value::String(value) =>
            Ok(value.clone()),

        Value::Number(value) =>
            Ok(value.to_string()),

        Value::Bool(value) =>
            Ok(value.to_string()),

        _ => Err(format!(
            "{} requires a scalar JSON value",
            context
        )),
    }
}

fn fetch_dictionary() -> Result<Value, String> {
    const MAIN_REF_URL: &str =
        "https://api.github.com/repos/krockzs/neebles-os/git/ref/heads/main";

    let ref_output = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "15",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "X-GitHub-Api-Version: 2022-11-28",
            MAIN_REF_URL,
        ])
        .output()
        .map_err(|error| {
            format!(
                "could not start curl while resolving neebles-os main ref: {error}"
            )
        })?;

    if !ref_output.status.success() {
        return Err(format!(
            "could not resolve neebles-os main ref from GitHub: {}",
            ref_output.status
        ));
    }

    let ref_json: Value =
        serde_json::from_slice(&ref_output.stdout)
            .map_err(|error| {
                format!(
                    "invalid GitHub main ref response: {error}"
                )
            })?;

    let commit_sha = ref_json
        .get("object")
        .and_then(|value| value.get("sha"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "GitHub main ref response does not contain object.sha"
                .to_string()
        })?;

    let dictionary_url = format!(
        "https://raw.githubusercontent.com/krockzs/neebles-os/{}/config/installers/instaladores_amd64.json",
        commit_sha
    );

    let output = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "15",
            &dictionary_url,
        ])
        .output()
        .map_err(|error| {
            format!(
                "could not start curl while reading installer dictionary: {error}"
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "could not read installer dictionary from GitHub commit '{}': {}",
            commit_sha,
            output.status
        ));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|error| {
            format!(
                "invalid remote installer dictionary at commit '{}': {error}",
                commit_sha
            )
        })
}

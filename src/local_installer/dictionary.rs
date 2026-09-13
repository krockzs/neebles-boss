use super::LocalInstallerRequest;
use serde_json::Value;
use std::process::Command;

const INSTALLER_DICTIONARY_URL: &str =
    "https://raw.githubusercontent.com/krockzs/neebles-os/main/config/installers/instaladores.json";

const CURRENT_DISTRIBUTION: &str = "debian";

#[derive(Debug, Clone)]
pub struct ResolvedOperation {
    pub distribution: String,
    pub installer: String,
    pub operation: String,
    pub command: String,
    pub args: Vec<String>,
    pub root_mode: String,
    pub expect: Option<Value>,
}

pub fn resolve(request: &LocalInstallerRequest) -> Result<ResolvedOperation, String> {
    let dictionary = fetch_dictionary()?;

    let distributions = dictionary
        .get("distributions")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "installer dictionary does not contain a valid 'distributions' object".to_string()
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

    let raw_args = operation
        .get("args")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            format!(
                "operation '{}:{}' does not define an args array",
                request.installer,
                request.operation
            )
        })?;

    let mut args = Vec::with_capacity(raw_args.len());

    for raw_arg in raw_args {
        let raw_arg = raw_arg.as_str().ok_or_else(|| {
            format!(
                "operation '{}:{}' contains a non-string argument",
                request.installer,
                request.operation
            )
        })?;

        args.push(resolve_argument(raw_arg, request)?);
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

    let expect = operation.get("expect").cloned();

    Ok(ResolvedOperation {
        distribution: CURRENT_DISTRIBUTION.to_string(),
        installer: request.installer.clone(),
        operation: request.operation.clone(),
        command,
        args,
        root_mode,
        expect,
    })
}

fn fetch_dictionary() -> Result<Value, String> {
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "15",
            INSTALLER_DICTIONARY_URL,
        ])
        .output()
        .map_err(|error| {
            format!(
                "could not start curl while reading installer dictionary: {error}"
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "could not read installer dictionary from GitHub: {}",
            output.status
        ));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid remote installer dictionary: {error}"))
}

fn resolve_argument(
    raw: &str,
    request: &LocalInstallerRequest,
) -> Result<String, String> {
    if raw.starts_with('{') && raw.ends_with('}') {
        let variable = &raw[1..raw.len() - 1];

        return request
            .variables
            .get(variable)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "missing variable '{{{variable}}}' for installer '{}', operation '{}'",
                    request.installer,
                    request.operation
                )
            });
    }

    Ok(raw.to_string())
}

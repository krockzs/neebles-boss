use super::dictionary;
use super::{LocalInstallerRequest, LocalInstallerResult};
use serde_json::Value;
use std::process::Command;

pub fn execute(request: LocalInstallerRequest) -> Result<LocalInstallerResult, String> {
    let resolved = dictionary::resolve(&request)?;

    match resolved.root_mode.as_str() {
        "required" => {
            crate::privileges::ensure_root(true)?;
        }
        "not_required" | "contextual" => {}
        other => {
            return Err(format!(
                "unsupported root_mode '{}' for installer '{}', operation '{}'",
                other,
                resolved.installer,
                resolved.operation
            ));
        }
    }

    let output = Command::new(&resolved.command)
        .args(&resolved.args)
        .output()
        .map_err(|error| {
            format!(
                "could not execute '{}': {error}",
                resolved.command
            )
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit_code = output.status.code().unwrap_or(1);

    let success = evaluate_success(
        output.status.success(),
        &stdout,
        resolved.expect.as_ref(),
    )?;

    Ok(LocalInstallerResult {
        installer: resolved.installer,
        operation: resolved.operation,
        command: resolved.command,
        args: resolved.args,
        requires_root: resolved.root_mode == "required",
        stdout,
        stderr,
        exit_code,
        success,
    })
}

fn evaluate_success(
    status_success: bool,
    stdout: &str,
    expect: Option<&Value>,
) -> Result<bool, String> {
    if !status_success {
        return Ok(false);
    }

    let Some(expect) = expect else {
        return Ok(true);
    };

    let object = expect.as_object().ok_or_else(|| {
        "operation expect field must be a JSON object".to_string()
    })?;

    if let Some(expected) = object.get("stdout_equals") {
        let expected = expected.as_str().ok_or_else(|| {
            "expect.stdout_equals must be a string".to_string()
        })?;

        return Ok(stdout.trim() == expected);
    }

    Err(format!(
        "unsupported expectation contract: {}",
        Value::Object(object.clone())
    ))
}

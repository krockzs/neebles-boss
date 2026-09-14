use crate::local_installer;
use crate::modules;
use crate::notifications::{self, Severity};
use crate::request::{ExecutionRequest, ExecutionResponse};
use serde_json::json;
use std::process::Command;

pub fn launch_ui() -> Result<(), String> {
    let path = crate::languages::client_root().join("ui/neebles-ui");
    Command::new(&path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not launch UI at {}: {error}", path.display()))
}

pub fn dispatch(request: ExecutionRequest) -> ExecutionResponse {
    match request.target.as_str() {
        "boss" => dispatch_boss(request),
        "notifications" | "boss.notifications" => dispatch_notification(request),
        module => {
            let mut args = Vec::new();
            if let Some(action) = request.action {
                args.push(action);
            }
            args.extend(request.args);
            match modules::execute(module, &args, &request.context.caller) {
                Ok(code) if code == 0 => ExecutionResponse::ok(Some(json!({ "exit_code": code }))),
                Ok(code) => ExecutionResponse::fail(
                    code,
                    "module_exit",
                    format!("module exited with code {code}"),
                ),
                Err(error) => ExecutionResponse::fail(1, "module_execution", error),
            }
        }
    }
}

fn dispatch_boss(request: ExecutionRequest) -> ExecutionResponse {
    match request.action.as_deref() {
        Some("version") => ExecutionResponse::ok(Some(json!({ "version": crate::VERSION }))),
        Some("start") | Some("open") => match launch_ui() {
            Ok(()) => ExecutionResponse::ok(None),
            Err(error) => ExecutionResponse::fail(1, "ui_launch", error),
        },
        Some("local-installer-audit") => match local_installer::audit_dictionary() {
            Ok(result) => match serde_json::to_value(result) {
                Ok(value) => ExecutionResponse::ok(Some(value)),
                Err(error) => ExecutionResponse::fail(
                    1,
                    "local_installer_audit_serialization",
                    format!("could not serialize local-installer audit: {error}"),
                ),
            },
            Err(error) => ExecutionResponse::fail(1, "local_installer_audit", error),
        },
        Some("local-installer-resolve") => {
            let Some(raw) = request.args.first() else {
                return ExecutionResponse::fail(
                    2,
                    "missing_local_installer_request",
                    "Boss local-installer-resolve requires a JSON request",
                );
            };

            let parsed: local_installer::LocalInstallerRequest = match serde_json::from_str(raw) {
                Ok(value) => value,
                Err(error) => {
                    return ExecutionResponse::fail(
                        2,
                        "invalid_local_installer_request",
                        format!("invalid local-installer-resolve request: {error}"),
                    );
                }
            };

            match local_installer::resolve_only(&parsed) {
                Ok(result) => match serde_json::to_value(result) {
                    Ok(value) => ExecutionResponse::ok(Some(value)),
                    Err(error) => ExecutionResponse::fail(
                        1,
                        "local_installer_resolution_serialization",
                        format!("could not serialize local-installer resolution: {error}"),
                    ),
                },
                Err(error) => ExecutionResponse::fail(1, "local_installer_resolution", error),
            }
        }
        Some("local-installer") => {
            let Some(raw) = request.args.first() else {
                return ExecutionResponse::fail(
                    2,
                    "missing_local_installer_request",
                    "Boss local-installer requires a JSON request",
                );
            };

            let parsed: local_installer::LocalInstallerRequest = match serde_json::from_str(raw) {
                Ok(value) => value,
                Err(error) => {
                    return ExecutionResponse::fail(
                        2,
                        "invalid_local_installer_request",
                        format!("invalid local-installer request: {error}"),
                    );
                }
            };

            match local_installer::handle(parsed) {
                Ok(result) => match serde_json::to_value(result) {
                    Ok(value) => ExecutionResponse::ok(Some(value)),
                    Err(error) => ExecutionResponse::fail(
                        1,
                        "local_installer_serialization",
                        format!("could not serialize local-installer result: {error}"),
                    ),
                },
                Err(error) => ExecutionResponse::fail(1, "local_installer", error),
            }
        }
        Some(other) => ExecutionResponse::fail(
            2,
            "unknown_boss_action",
            format!("unknown Boss action: {other}"),
        ),
        None => {
            ExecutionResponse::fail(2, "missing_boss_action", "Boss request requires an action")
        }
    }
}

fn dispatch_notification(request: ExecutionRequest) -> ExecutionResponse {
    let severity = request
        .args
        .first()
        .cloned()
        .unwrap_or_else(|| "info".to_string());
    let title = request
        .args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "N.E.E.B.L.E.S.".to_string());
    let message = request.args.get(2).cloned().unwrap_or_default();

    let severity = match Severity::parse(&severity) {
        Ok(severity) => severity,

        Err(error) => {
            return ExecutionResponse::fail(1, "notification", error);
        }
    };

    /*
     * Module processes receive NEEBLES_MODULE from Boss.
     * If that context exists, every notification request
     * must pass through the module capability contract,
     * regardless of whether the request target was
     * "notifications" or "boss.notifications".
     *
     * That prevents a module from bypassing governance
     * simply by selecting the Boss target name.
     */
    let result = match std::env::var("NEEBLES_MODULE") {
        Ok(module) if !module.trim().is_empty() => {
            notifications::emit_for_module(module.trim(), severity, &title, &message)
        }

        _ => notifications::emit(severity, &title, &message),
    };

    match result {
        Ok(()) => ExecutionResponse::ok(None),
        Err(error) => ExecutionResponse::fail(1, "notification", error),
    }
}

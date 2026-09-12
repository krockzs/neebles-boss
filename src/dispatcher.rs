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
                Ok(code) => ExecutionResponse::fail(code, "module_exit", format!("module exited with code {code}")),
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
        Some(other) => ExecutionResponse::fail(2, "unknown_boss_action", format!("unknown Boss action: {other}")),
        None => ExecutionResponse::fail(2, "missing_boss_action", "Boss request requires an action"),
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

    match Severity::parse(&severity).and_then(|value| notifications::emit(value, &title, &message)) {
        Ok(()) => ExecutionResponse::ok(None),
        Err(error) => ExecutionResponse::fail(1, "notification", error),
    }
}

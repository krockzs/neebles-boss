use crate::config;
use crate::external;
use crate::module_ipc::protocol::ModuleMessage;
use crate::modules;
use crate::notifications::{self, Severity};
use crate::request::{ExecutionRequest, ExecutionResponse};
use crate::settings;
use crate::surface_state::{self, BossUiState};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn launch_ui() -> Result<(), String> {
    match surface_state::boss_ui_state() {
        BossUiState::Open | BossUiState::Opening => {
            return Ok(());
        }

        BossUiState::Closed => {}
    }

    let desktop_session =
        neebles_backend::domestic_desktop_session_interface::
            resolve_current_desktop_session_interface()?;

    let desktop_uid = desktop_session.desktop_uid();
    let desktop_gid = desktop_session.desktop_gid();

    let client_root = crate::languages::client_root()?;
    let ui_path = client_root.join("ui/neebles-ui");
    let runtime_resolver = client_root.join("runtime/neebles-runtime-resolve");
    let runtime_manifest =
        neebles_backend::domestic_runtime_authority::current_boss_runtime_manifest()?;

    /*
     * Boss UI does not discover, authenticate or grant Platform Authority.
     *
     * The running Boss process already owns the authenticated process-scoped
     * AuthoritySupply. UI receives only that exact transport reference so any
     * Boss child process can re-enter the canonical Rust registration/grant
     * boundary.
     */
    let authority_supply =
        neebles_backend::domestic_authority_supply_process::authority_supply_process_state()?
            .supply_path()
            .to_path_buf();

    if !runtime_resolver.is_file() {
        return Err(format!(
            "installed runtime resolver is missing: {}",
            runtime_resolver.display()
        ));
    }

    let opening_generation = surface_state::begin_boss_ui_opening()?;

    let setpriv =
        neebles_backend::domestic_runtime_authority::resolve_boss_executable("boss.setpriv")?;
    let systemd_run =
        neebles_backend::domestic_runtime_authority::resolve_boss_executable("boss.systemd-run")?;

    let protocol_environment = desktop_session.environment().clone();

    let mut command = neebles_backend::domestic_environment::build_process_command(
        setpriv,
        neebles_backend::domestic_environment::ProcessEnvironmentClass::SystemInterface,
        &protocol_environment,
        &BTreeMap::new(),
        &std::collections::BTreeSet::new(),
    )?;

    let result = command
        .arg(format!("--reuid={desktop_uid}"))
        .arg(format!("--regid={desktop_gid}"))
        .arg("--init-groups")
        .arg(systemd_run)
        .arg("--user")
        .arg("--collect")
        .arg("--quiet")
        .arg(format!(
            "--setenv=NEEBLES_CLIENT_ROOT={}",
            client_root.display()
        ))
        .arg(format!(
            "--setenv=NEEBLES_RUNTIME_RESOLVER={}",
            runtime_resolver.display()
        ))
        .arg(format!(
            "--setenv=NEEBLES_RUNTIME_MANIFEST={}",
            runtime_manifest.display()
        ))
        .arg(format!(
            "--setenv=NEEBLES_AUTHORITY_SUPPLY={}",
            authority_supply.display()
        ))
        .arg(&ui_path)
        .status();

    match result {
        Ok(status) if status.success() => {
            surface_state::watch_boss_ui_opening(opening_generation);
            Ok(())
        }

        Ok(status) => {
            surface_state::cancel_boss_ui_opening(opening_generation);

            Err(format!(
                "desktop user manager rejected Boss UI launch with status {status}"
            ))
        }

        Err(error) => {
            surface_state::cancel_boss_ui_opening(opening_generation);

            Err(format!(
                "could not launch Boss UI through the desktop user manager: {error}"
            ))
        }
    }
}

pub fn dispatch(request: ExecutionRequest) -> ExecutionResponse {
    let target = request.target.clone();

    let response = match target.as_str() {
        "boss" => dispatch_boss(request),

        "notifications" | "boss.notifications" => dispatch_notification(request),

        "settings" | "boss.settings" => dispatch_settings(request),

        module => dispatch_module(module, request),
    };

    emit_error_external(&response);

    response
}

fn emit_error_external(response: &ExecutionResponse) {
    let Some(error) = response.error.as_ref() else {
        return;
    };

    let Some(envelope) =
        external::build_external_envelope("error", "", serde_json::Map::new, || {
            external::error_message(&error.kind, &error.message)
        })
    else {
        return;
    };

    if let Err(send_error) = external::send(&envelope) {
        eprintln!(
            "N.E.E.B.L.E.S. Boss: external error delivery unavailable; continuing: {send_error}"
        );
    }
}

fn settings_target(target: &str) -> Result<(std::path::PathBuf, Value), String> {
    let target = target.trim();

    if target.is_empty() {
        return Err("settings target cannot be empty".to_string());
    }

    let default = modules::installed_module_settings_default(target)?;

    Ok((
        settings::module_settings_path(&modules::neebles_root(), target),
        default,
    ))
}

fn dispatch_boss_settings(request: ExecutionRequest) -> ExecutionResponse {
    match request.action.as_deref() {
        Some("show") => match config::settings_snapshot() {
            Ok(value) => ExecutionResponse::ok(Some(value)),

            Err(error) => ExecutionResponse::fail(1, "settings_read", error),
        },

        Some("get") => {
            let Some(setting_path) = request.args.get(1) else {
                return ExecutionResponse::fail(
                    2,
                    "missing_settings_path",
                    "settings get requires a path",
                );
            };

            match config::setting_value(setting_path) {
                Ok(value) => ExecutionResponse::ok(Some(json!(value))),

                Err(error) => ExecutionResponse::fail(1, "settings_path", error),
            }
        }

        Some("set") => {
            let Some(setting_path) = request.args.get(1) else {
                return ExecutionResponse::fail(
                    2,
                    "missing_settings_path",
                    "settings set requires a path",
                );
            };

            let Some(value) = request.args.get(2) else {
                return ExecutionResponse::fail(
                    2,
                    "missing_settings_value",
                    "settings set requires a String value",
                );
            };

            let previous = config::setting_value(setting_path);

            match config::set_setting_value(setting_path, value.clone()) {
                Ok(value) => {
                    let changed = previous
                        .as_ref()
                        .map(|previous| previous != &value)
                        .unwrap_or(true);

                    if changed {
                        if let Err(error) = crate::module_ipc::runtime_registry().broadcast_event(
                            "settings.boss",
                            "changed",
                            json!({
                                "target": "boss",
                                "path":
                                    setting_path,
                                "value":
                                    value
                            }),
                        ) {
                            eprintln!(
                                "N.E.E.B.L.E.S.: Boss settings persisted but event broadcast failed: {}",
                                error
                            );
                        }
                    }

                    ExecutionResponse::ok(Some(json!(value)))
                }

                Err(error) => ExecutionResponse::fail(1, "settings_write", error),
            }
        }

        Some("has-state") | Some("module-update") => ExecutionResponse::fail(
            2,
            "invalid_settings_target",
            "requested settings action requires a module target",
        ),

        Some(other) => ExecutionResponse::fail(
            2,
            "unknown_settings_action",
            format!("unknown settings action: {other}"),
        ),

        None => ExecutionResponse::fail(
            2,
            "missing_settings_action",
            "settings request requires an action",
        ),
    }
}

fn dispatch_settings(request: ExecutionRequest) -> ExecutionResponse {
    if request.args.first().map(String::as_str) == Some("boss") {
        return dispatch_boss_settings(request);
    }

    let Some(target) = request.args.first() else {
        return ExecutionResponse::fail(
            2,
            "missing_settings_target",
            "settings request requires a target",
        );
    };

    let (path, default) = match settings_target(target) {
        Ok(value) => value,

        Err(error) => {
            return ExecutionResponse::fail(1, "settings_target", error);
        }
    };

    match request.action.as_deref() {
        Some("has-state") => {
            if target == "boss" {
                return ExecutionResponse::fail(
                    2,
                    "invalid_settings_target",
                    "has-state requires a module target",
                );
            }

            match modules::module_has_local_state(target) {
                Ok(value) => ExecutionResponse::ok(Some(json!(value))),

                Err(error) => ExecutionResponse::fail(1, "settings_state", error),
            }
        }

        Some("show") => {
            let local = match settings::load_or_create(&path, &default) {
                Ok(value) => value,

                Err(error) => {
                    return ExecutionResponse::fail(1, "settings_read", error);
                }
            };

            match settings::effective_settings(&local, &default) {
                Ok(value) => ExecutionResponse::ok(Some(value)),

                Err(error) => ExecutionResponse::fail(1, "settings_read", error),
            }
        }

        Some("get") => {
            let Some(setting_path) = request.args.get(1) else {
                return ExecutionResponse::fail(
                    2,
                    "missing_settings_path",
                    "settings get requires a path",
                );
            };

            match modules::module_setting_get(target, setting_path) {
                Ok(value) => ExecutionResponse::ok(Some(json!(value))),

                Err(error) => ExecutionResponse::fail(1, "settings_path", error),
            }
        }

        Some("set") => {
            let Some(setting_path) = request.args.get(1) else {
                return ExecutionResponse::fail(
                    2,
                    "missing_settings_path",
                    "settings set requires a path",
                );
            };

            let Some(value) = request.args.get(2) else {
                return ExecutionResponse::fail(
                    2,
                    "missing_settings_value",
                    "settings set requires a String value",
                );
            };

            match modules::module_setting_set(target, setting_path, value.clone()) {
                Ok(write) => ExecutionResponse::ok(Some(json!(write.value))),

                Err(error) => ExecutionResponse::fail(1, "settings_write", error),
            }
        }

        Some("module-update") => {
            if target == "boss" {
                return ExecutionResponse::fail(
                    2,
                    "invalid_settings_target",
                    "module-update requires a module target",
                );
            }

            match settings::update_from_default(&path, &default) {
                Ok(value) => {
                    let topic = format!("settings.{}", target);

                    if let Err(error) = crate::module_ipc::runtime_registry().broadcast_event(
                        &topic,
                        "reconciled",
                        json!({
                            "target": target
                        }),
                    ) {
                        eprintln!(
                            "N.E.E.B.L.E.S.: settings reconciled but event broadcast failed for '{}': {}",
                            target,
                            error
                        );
                    }

                    ExecutionResponse::ok(Some(value))
                }

                Err(error) => ExecutionResponse::fail(1, "settings_update", error),
            }
        }

        Some(other) => ExecutionResponse::fail(
            2,
            "unknown_settings_action",
            format!("unknown settings action: {other}"),
        ),

        None => ExecutionResponse::fail(
            2,
            "missing_settings_action",
            "settings request requires an action",
        ),
    }
}

fn dispatch_module(module: &str, request: ExecutionRequest) -> ExecutionResponse {
    /*
     * Module actions resolve exclusively through the dynamic
     * "commands" contract and Module IPC.
     *
     * No direct-entrypoint execution fallback exists.
     */
    let action = request.action.as_deref().unwrap_or("default");

    match modules::installed_module_contract_endpoint(module, "commands", action) {
        Ok(Some(_)) => {
            let mut context = BTreeMap::<String, serde_json::Value>::new();

            context.insert("caller".to_string(), json!(request.context.caller));

            match crate::module_ipc::invoke_declared(
                module,
                "commands",
                action,
                request.args,
                None,
                context,
                None,
            ) {
                Ok(ModuleMessage::Response {
                    ok,
                    code,
                    result,
                    error,
                    ..
                }) => {
                    if ok {
                        ExecutionResponse {
                            ok: true,
                            code,
                            result,
                            error: None,
                        }
                    } else {
                        let message = error.map(|error| error.message).unwrap_or_else(|| {
                            format!("module '{}' endpoint '{}' failed", module, action)
                        });

                        ExecutionResponse::fail(code, "module_runtime", message)
                    }
                }

                Ok(ModuleMessage::Error { error, .. }) => {
                    ExecutionResponse::fail(1, error.kind, error.message)
                }

                Ok(message) => ExecutionResponse::fail(
                    1,
                    "unexpected_module_response",
                    format!(
                        "module '{}' returned unexpected runtime message: {:?}",
                        module, message
                    ),
                ),

                Err(error) => ExecutionResponse::fail(1, "module_runtime", error),
            }
        }

        Ok(None) => ExecutionResponse::fail(
            2,
            "unknown_module_action",
            format!(
                "module '{}' does not declare dynamic commands endpoint '{}'",
                module, action
            ),
        ),

        Err(error) => ExecutionResponse::fail(1, "module_contract", error),
    }
}

fn dispatch_domestic_construction(request: &ExecutionRequest) -> ExecutionResponse {
    if request.args.len() != 2 {
        return ExecutionResponse::fail(
            2,
            "invalid_domestic_construction_arguments",
            "Boss domestic-construction-execute requires exactly: <subject> <step>",
        );
    }

    let subject = &request.args[0];
    let step = &request.args[1];

    let declaration =
        match neebles_backend::domestic_construction::
            load_canonical_domestic_construction_declaration(subject)
        {
            Ok(declaration) => declaration,

            Err(error) => {
                return ExecutionResponse::fail(
                    1,
                    "domestic_construction_declaration",
                    error,
                );
            }
        };

    let registry =
        match neebles_backend::domestic_authority_supply_process::
            process_supplied_authority_registry()
        {
            Ok(registry) => registry,

            Err(error) => {
                return ExecutionResponse::fail(
                    1,
                    "domestic_construction_authority",
                    error,
                );
            }
        };

    let runtime_manifest =
        match neebles_backend::domestic_runtime_authority::current_boss_runtime_manifest() {
            Ok(runtime_manifest) => runtime_manifest,

            Err(error) => {
                return ExecutionResponse::fail(1, "domestic_construction_runtime", error);
            }
        };

    let exit_code = match neebles_backend::domestic_construction::execute_construction_step(
        &declaration,
        step,
        &runtime_manifest,
        registry,
    ) {
        Ok(exit_code) => exit_code,

        Err(error) => {
            return ExecutionResponse::fail(1, "domestic_construction_execution", error);
        }
    };

    if exit_code != 0 {
        return ExecutionResponse::fail(
            exit_code,
            "domestic_construction_process",
            format!(
                "domestic construction process failed: subject={subject} step={step} exit_code={exit_code}"
            ),
        );
    }

    ExecutionResponse::ok(Some(json!({
        "subject": subject,
        "step": step,
        "exit_code": exit_code
    })))
}

fn dispatch_boss(request: ExecutionRequest) -> ExecutionResponse {
    match request.action.as_deref() {
        Some("version") => ExecutionResponse::ok(Some(json!({ "version": crate::VERSION }))),

        Some("update-status") => match crate::boss_update::status_json() {
            Ok(status) => ExecutionResponse::ok(Some(status)),

            Err(error) => ExecutionResponse::fail(1, "boss_update_status", error),
        },

        Some("update-execute") => match crate::boss_update::execute_update_json() {
            Ok(result) => ExecutionResponse::ok(Some(result)),

            Err(error) => ExecutionResponse::fail(1, "boss_update_execute", error),
        },

        Some("domestic-construction-execute") => dispatch_domestic_construction(&request),

        Some("runtime-list") => match crate::module_ipc::runtime_registry().snapshot() {
            Ok(records) => match serde_json::to_value(records) {
                Ok(value) => ExecutionResponse::ok(Some(value)),

                Err(error) => ExecutionResponse::fail(
                    1,
                    "runtime_list_serialization",
                    format!("could not serialize runtime registry: {error}"),
                ),
            },

            Err(error) => ExecutionResponse::fail(1, "runtime_registry", error),
        },

        Some("runtime-status") => {
            let Some(module) = request.args.first() else {
                return ExecutionResponse::fail(
                    2,
                    "missing_runtime_module",
                    "Boss runtime-status requires a module name",
                );
            };

            match crate::module_ipc::runtime_registry().snapshot_module(module) {
                Ok(Some(record)) => match serde_json::to_value(record) {
                    Ok(value) => ExecutionResponse::ok(Some(value)),

                    Err(error) => ExecutionResponse::fail(
                        1,
                        "runtime_status_serialization",
                        format!("could not serialize runtime status: {error}"),
                    ),
                },

                Ok(None) => ExecutionResponse::ok(Some(json!({
                    "module": module,
                    "registered": false
                }))),

                Err(error) => ExecutionResponse::fail(1, "runtime_registry", error),
            }
        }

        Some("start") | Some("open") => match launch_ui() {
            Ok(()) => ExecutionResponse::ok(None),
            Err(error) => ExecutionResponse::fail(1, "ui_launch", error),
        },

        Some("config-changed") => match config::load_or_initialize() {
            Ok(config) => {
                let tray_visible = config.tray_enabled;

                let payload = match serde_json::to_value(config) {
                    Ok(payload) => payload,

                    Err(error) => {
                        return ExecutionResponse::fail(
                            1,
                            "config_event_serialization",
                            format!("could not serialize Boss config event: {error}"),
                        );
                    }
                };

                crate::ipc::broadcast_event("settings.boss", "state_changed", payload);

                match crate::tray::client::request(
                    &crate::tray::protocol::TrayMessage::SetRootVisibility {
                        visible: tray_visible,
                    },
                ) {
                    Ok(crate::tray::protocol::TrayMessage::Ack { .. }) => {}

                    Ok(response) => {
                        eprintln!(
                            "N.E.E.B.L.E.S.: unexpected root tray visibility response: {response:?}"
                        );
                    }

                    Err(error) => {
                        eprintln!(
                            "N.E.E.B.L.E.S.: Boss settings updated; live root tray sync unavailable: {error}"
                        );
                    }
                }

                ExecutionResponse::ok(None)
            }

            Err(error) => ExecutionResponse::fail(1, "config_event", error),
        },
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

#[cfg(test)]
mod domestic_construction_dispatch_tests {
    use super::*;
    use crate::request::ExecutionContext;

    fn request(args: Vec<&str>) -> ExecutionRequest {
        ExecutionRequest {
            target: "boss".to_string(),
            action: Some("domestic-construction-execute".to_string()),
            args: args.into_iter().map(str::to_string).collect(),
            context: ExecutionContext::default(),
        }
    }

    #[test]
    fn construction_dispatch_rejects_missing_arguments_before_execution() {
        let response = dispatch_domestic_construction(&request(Vec::new()));

        assert!(!response.ok);
        assert_eq!(response.code, 2);

        let error = response.error.expect("contract error expected");
        assert_eq!(error.kind, "invalid_domestic_construction_arguments");
    }

    #[test]
    fn construction_dispatch_rejects_single_argument_before_execution() {
        let response = dispatch_domestic_construction(&request(vec!["fixture.subject"]));

        assert!(!response.ok);
        assert_eq!(response.code, 2);

        let error = response.error.expect("contract error expected");
        assert_eq!(error.kind, "invalid_domestic_construction_arguments");
    }

    #[test]
    fn construction_dispatch_rejects_extra_arguments_before_execution() {
        let response = dispatch_domestic_construction(&request(vec![
            "fixture.subject",
            "build",
            "unexpected",
        ]));

        assert!(!response.ok);
        assert_eq!(response.code, 2);

        let error = response.error.expect("contract error expected");
        assert_eq!(error.kind, "invalid_domestic_construction_arguments");
    }
}

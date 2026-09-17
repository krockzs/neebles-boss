use crate::dependencies::{self, SystemDependency, SystemDependencyVersion, SystemVersionScheme};
use crate::external;
use crate::local_installer::{self, LocalInstallerRequest};
use crate::modules;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

const STAGE0_STATE_PATH: &str = "/run/neebles/stage0.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage0State {
    pub service: String,
    pub ready: bool,
    pub status: String,
}

fn write_state(ready: bool, status: &str) -> Result<(), String> {
    let path = Path::new(STAGE0_STATE_PATH);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "could not create Stage0 runtime directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let state = Stage0State {
        service: "service.stage0".to_string(),
        ready,
        status: status.to_string(),
    };

    let payload = serde_json::to_vec_pretty(&state)
        .map_err(|error| format!("could not serialize Stage0 state: {error}"))?;

    let temporary = path.with_extension("json.tmp");

    fs::write(&temporary, payload).map_err(|error| {
        format!(
            "could not write temporary Stage0 state {}: {error}",
            temporary.display()
        )
    })?;

    fs::rename(&temporary, path)
        .map_err(|error| format!("could not publish Stage0 state {}: {error}", path.display()))
}

pub fn wait(timeout: Duration) -> Result<(), String> {
    wait_for_state(
        Path::new(STAGE0_STATE_PATH),
        timeout,
        Duration::from_millis(100),
    )
}

fn wait_for_state(path: &Path, timeout: Duration, poll_interval: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;

    loop {
        match fs::read_to_string(path) {
            Ok(raw) => {
                let state: Stage0State = serde_json::from_str(&raw)
                    .map_err(|error| format!("invalid Stage0 state: {error}"))?;

                if state.ready && state.status == "ready" {
                    return Ok(());
                }

                if state.status == "failed" {
                    return Err("Stage0 preflight failed".to_string());
                }
            }

            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}

            Err(error) => {
                return Err(format!("could not read Stage0 state: {error}"));
            }
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "timed out waiting for Stage0 readiness after {} seconds",
                timeout.as_secs()
            ));
        }

        thread::sleep(poll_interval);
    }
}

pub fn run() -> Result<(), String> {
    run_with(
        |ready, status| write_state(ready, status),
        || emit_stage0_external("checking", "Stage0 preflight started"),
        refresh_dictionary_non_blocking,
        || {
            let dependencies = critical_dependencies();
            dependencies::resolve_system_dependencies(&dependencies)
        },
        modules::preflight_installed_modules,
    )
}

fn run_with<WriteState, EmitExternal, Refresh, Dependencies, Modules>(
    mut write_state_fn: WriteState,
    mut emit_external_fn: EmitExternal,
    mut refresh_fn: Refresh,
    mut dependencies_fn: Dependencies,
    mut modules_fn: Modules,
) -> Result<(), String>
where
    WriteState: FnMut(bool, &str) -> Result<(), String>,
    EmitExternal: FnMut(),
    Refresh: FnMut(),
    Dependencies: FnMut() -> Result<(), String>,
    Modules: FnMut() -> Result<(), String>,
{
    write_state_fn(false, "checking")?;

    emit_external_fn();

    refresh_fn();

    dependencies_fn()?;

    modules_fn()?;

    write_state_fn(true, "ready")
}

fn emit_stage0_external(state: &str, message: &str) {
    let Some(envelope) =
        external::build_external_envelope("stage0", "", serde_json::Map::new, || {
            external::stage0_message(state, message)
        })
    else {
        return;
    };

    if let Err(error) = external::send(&envelope) {
        eprintln!("N.E.E.B.L.E.S. Stage0: external delivery unavailable; continuing: {error}");
    }
}

fn installer_request(
    installer: &str,
    operation: &str,
    variables: &[(&str, serde_json::Value)],
) -> LocalInstallerRequest {
    let variables = variables
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect::<HashMap<_, _>>();

    LocalInstallerRequest {
        installer: installer.to_string(),
        operation: operation.to_string(),
        variables,
    }
}

fn system_package_dependency(package: &str, required: bool) -> SystemDependency {
    SystemDependency {
        name: package.to_string(),
        install: installer_request("apt", "install", &[("packages", json!([package]))]),
        verify: installer_request(
            "dpkg-query",
            "check_installed",
            &[("package", json!(package))],
        ),
        version: None,
        required,
    }
}

fn system_package_dependency_with_debian_version(
    package: &str,
    requirement: &str,
    required: bool,
) -> SystemDependency {
    let mut dependency = system_package_dependency(package, required);

    dependency.version = Some(SystemDependencyVersion {
        scheme: SystemVersionScheme::Debian,
        requirement: requirement.to_string(),
        query: installer_request("dpkg-query", "version", &[("package", json!(package))]),
    });

    dependency
}

fn critical_dependencies() -> Vec<SystemDependency> {
    vec![
        system_package_dependency("git", false),
        system_package_dependency("curl", false),
        system_package_dependency("libnotify-bin", true),
        system_package_dependency_with_debian_version("libqt6quick6", ">=6.5,<7.0", true),
        system_package_dependency_with_debian_version(
            "liblayershellqtinterface6",
            ">=6.3,<7.0",
            true,
        ),
    ]
}

fn refresh_dictionary_non_blocking() {
    if let Err(error) = local_installer::refresh_dictionary_cache() {
        eprintln!(
            "N.E.E.B.L.E.S. Stage0: remote installer dictionary refresh unavailable; continuing with local recovery data: {error}"
        );
    }
}

pub fn mark_failed() {
    let _ = write_state(false, "failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_state_path(label: &str) -> std::path::PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);

        std::env::temp_dir().join(format!(
            "neebles-stage0-{label}-{}-{id}.json",
            std::process::id()
        ))
    }

    fn write_test_state(path: &Path, ready: bool, status: &str) {
        let state = Stage0State {
            service: "service.stage0".to_string(),
            ready,
            status: status.to_string(),
        };

        fs::write(path, serde_json::to_vec(&state).unwrap()).unwrap();
    }

    #[test]
    fn certification_stage0_dependency_contracts_match_debian_trixie() {
        let dependencies = critical_dependencies();

        let qt_quick = dependencies
            .iter()
            .find(|dependency| dependency.name == "libqt6quick6")
            .expect("Qt Quick dependency must exist");

        let layer_shell = dependencies
            .iter()
            .find(|dependency| dependency.name == "liblayershellqtinterface6")
            .expect("LayerShellQt dependency must exist");

        assert_eq!(qt_quick.version.as_ref().unwrap().requirement, ">=6.5,<7.0");

        assert_eq!(
            layer_shell.version.as_ref().unwrap().requirement,
            ">=6.3,<7.0"
        );
    }

    #[test]
    fn certification_stage0_runs_complete_preflight_in_order() {
        use std::cell::RefCell;

        let calls = RefCell::new(Vec::<String>::new());

        run_with(
            |ready, status| {
                calls.borrow_mut().push(format!("state:{ready}:{status}"));
                Ok(())
            },
            || calls.borrow_mut().push("external".to_string()),
            || calls.borrow_mut().push("refresh".to_string()),
            || {
                calls.borrow_mut().push("dependencies".to_string());
                Ok(())
            },
            || {
                calls.borrow_mut().push("modules".to_string());
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(
            calls.into_inner(),
            vec![
                "state:false:checking",
                "external",
                "refresh",
                "dependencies",
                "modules",
                "state:true:ready",
            ]
        );
    }

    #[test]
    fn certification_stage0_stops_before_modules_when_required_dependency_fails() {
        use std::cell::RefCell;

        let calls = RefCell::new(Vec::<String>::new());

        let error = run_with(
            |ready, status| {
                calls.borrow_mut().push(format!("state:{ready}:{status}"));
                Ok(())
            },
            || calls.borrow_mut().push("external".to_string()),
            || calls.borrow_mut().push("refresh".to_string()),
            || {
                calls.borrow_mut().push("dependencies".to_string());
                Err("required dependency failed".to_string())
            },
            || {
                calls.borrow_mut().push("modules".to_string());
                Ok(())
            },
        )
        .unwrap_err();

        assert_eq!(error, "required dependency failed");

        assert_eq!(
            calls.into_inner(),
            vec![
                "state:false:checking",
                "external",
                "refresh",
                "dependencies",
            ]
        );
    }

    #[test]
    fn certification_stage0_does_not_publish_ready_when_module_preflight_fails() {
        use std::cell::RefCell;

        let calls = RefCell::new(Vec::<String>::new());

        let error = run_with(
            |ready, status| {
                calls.borrow_mut().push(format!("state:{ready}:{status}"));
                Ok(())
            },
            || calls.borrow_mut().push("external".to_string()),
            || calls.borrow_mut().push("refresh".to_string()),
            || {
                calls.borrow_mut().push("dependencies".to_string());
                Ok(())
            },
            || {
                calls.borrow_mut().push("modules".to_string());
                Err("module preflight failed".to_string())
            },
        )
        .unwrap_err();

        assert_eq!(error, "module preflight failed");

        assert_eq!(
            calls.into_inner(),
            vec![
                "state:false:checking",
                "external",
                "refresh",
                "dependencies",
                "modules",
            ]
        );
    }

    #[test]
    fn stage0_external_is_non_blocking() {
        emit_stage0_external("checking", "Stage0 preflight started");
    }

    #[test]
    fn wait_accepts_ready_state() {
        let path = test_state_path("ready");
        write_test_state(&path, true, "ready");

        let result = wait_for_state(&path, Duration::from_millis(50), Duration::from_millis(1));

        let _ = fs::remove_file(&path);
        assert!(result.is_ok());
    }

    #[test]
    fn wait_rejects_failed_state() {
        let path = test_state_path("failed");
        write_test_state(&path, false, "failed");

        let error =
            wait_for_state(&path, Duration::from_millis(50), Duration::from_millis(1)).unwrap_err();

        let _ = fs::remove_file(&path);
        assert!(error.contains("Stage0 preflight failed"));
    }

    #[test]
    fn wait_times_out_when_state_never_appears() {
        let path = test_state_path("timeout");
        let _ = fs::remove_file(&path);

        let error =
            wait_for_state(&path, Duration::from_millis(10), Duration::from_millis(1)).unwrap_err();

        assert!(error.contains("timed out waiting for Stage0 readiness"));
    }

    #[test]
    fn wait_rejects_corrupt_state() {
        let path = test_state_path("corrupt");
        fs::write(&path, b"{not-json").unwrap();

        let error =
            wait_for_state(&path, Duration::from_millis(50), Duration::from_millis(1)).unwrap_err();

        let _ = fs::remove_file(&path);
        assert!(error.contains("invalid Stage0 state"));
    }
}

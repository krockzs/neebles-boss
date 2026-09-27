use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::os::unix::process::CommandExt;
use std::path::Path;

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn ensure_root(required: bool) -> Result<(), String> {
    if !required || is_root() {
        return Ok(());
    }

    reexec_current_with_auth_agent()
}

pub fn reexec_current_with_auth_agent() -> Result<(), String> {
    let executable = env::current_exe()
        .map_err(|error| format!("could not resolve current executable: {error}"))?;
    let args = env::args_os().skip(1).collect::<Vec<_>>();

    const AUTH_AGENT: &str = "/opt/neebles/client/auth/neebles-auth-agent";

    if !Path::new(AUTH_AGENT).is_file() {
        return Err(format!(
            "N.E.E.B.L.E.S. authentication agent not found at {AUTH_AGENT}"
        ));
    }

    let mut session_environment = BTreeMap::<String, String>::new();

    let mut allowed_session_inputs = BTreeSet::<String>::new();

    for key in [
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
    ] {
        allowed_session_inputs.insert(key.to_string());

        if let Some(value) = env::var_os(key) {
            session_environment.insert(key.to_string(), value.to_string_lossy().into_owned());
        }
    }

    let mut command = neebles_backend::domestic_environment::build_process_command(
        AUTH_AGENT,
        neebles_backend::domestic_environment::ProcessEnvironmentClass::Session,
        &BTreeMap::new(),
        &session_environment,
        &allowed_session_inputs,
    )?;

    command
        .arg("--operation")
        .arg("generic")
        .arg("--")
        .arg(neebles_backend::domestic_runtime_authority::resolve_boss_executable("boss.env")?);

    if let Ok(path) = crate::config::config_path() {
        command.arg(format!("NEEBLES_CONFIG={}", path.display()));
    }

    let tray_socket = crate::tray::protocol::socket_path();

    command.arg(format!("NEEBLES_TRAY_SOCKET={}", tray_socket.display()));

    /*
     * Runtime PID markers belong to the original desktop
     * user, not to root.
     *
     * Preserve that identity explicitly across the
     * Polkit authorization boundary so update,
     * uninstall and close-running inspect the same
     * runtime namespace that created the markers.
     */
    let runtime_identity = unsafe { libc::geteuid() }.to_string();

    command.arg(format!("NEEBLES_RUNTIME_IDENTITY={}", runtime_identity));

    for key in [
        "NEEBLES_ROOT",
        "NEEBLES_CLIENT_ROOT",
        "NEEBLES_MODULES_REGISTRY",
        "NEEBLES_COMMAND",
    ] {
        if let Some(value) = env::var_os(key) {
            command.arg(format!("{key}={}", value.to_string_lossy()));
        }
    }

    let error = command.arg(executable).args(args).exec();

    Err(format!(
        "could not re-execute N.E.E.B.L.E.S. through authentication agent: {error}"
    ))
}

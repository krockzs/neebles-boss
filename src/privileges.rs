use std::env;
use std::os::unix::process::CommandExt;
use std::process::Command;

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn ensure_root(required: bool) -> Result<(), String> {
    if !required || is_root() {
        return Ok(());
    }

    reexec_current_with_sudo()
}

pub fn reexec_current_with_sudo() -> Result<(), String> {
    let executable = env::current_exe()
        .map_err(|error| format!("could not resolve current executable: {error}"))?;
    let args = env::args_os().skip(1).collect::<Vec<_>>();

    // Preserve the caller's N.E.E.B.L.E.S. context explicitly. In particular,
    // the user config must not silently switch to root's HOME after sudo.
    let mut command = Command::new("sudo");
    command.arg("--").arg("env");

    if let Ok(path) = crate::config::config_path() {
        command.arg(format!("NEEBLES_CONFIG={}", path.display()));
    }
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
    Err(format!("could not re-execute N.E.E.B.L.E.S. through sudo: {error}"))
}

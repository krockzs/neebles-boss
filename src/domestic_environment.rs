use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessEnvironmentClass {
    Pure,
    Session,
    SystemInterface,
}

const FORBIDDEN_SESSION_INPUTS: &[&str] = &[
    "PATH",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "PYTHONPATH",
    "PYTHONHOME",
    "QT_PLUGIN_PATH",
    "QT_QPA_PLATFORM_PLUGIN_PATH",
    "QML_IMPORT_PATH",
    "QML2_IMPORT_PATH",
    "GI_TYPELIB_PATH",
    "GIO_EXTRA_MODULES",
];

pub fn parse_environment_lines(raw: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::<String, String>::new();

    for line in raw.lines() {
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };

        let key = raw_key.trim();

        if key.is_empty() {
            continue;
        }

        result.insert(key.to_string(), raw_value.to_string());
    }

    result
}

pub fn session_input_is_forbidden(key: &str) -> bool {
    FORBIDDEN_SESSION_INPUTS.contains(&key)
}

pub fn build_sealed_environment(
    domestic: &BTreeMap<String, String>,

    session: &BTreeMap<String, String>,

    allowed_session_inputs: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>, String> {
    let mut sealed = BTreeMap::<String, String>::new();

    for (key, value) in domestic {
        if key.trim().is_empty() {
            return Err("domestic environment contains empty key".to_string());
        }

        sealed.insert(key.clone(), value.clone());
    }

    for key in allowed_session_inputs {
        if key.trim().is_empty() {
            return Err("allowed session input cannot be empty".to_string());
        }

        if session_input_is_forbidden(key) {
            return Err(format!(
                "session input is forbidden from controlling domestic execution: {key}"
            ));
        }

        if domestic.contains_key(key) {
            return Err(format!(
                "session input attempts to override domestic environment: {key}"
            ));
        }

        if let Some(value) = session.get(key) {
            sealed.insert(key.clone(), value.clone());
        }
    }

    Ok(sealed)
}

pub fn build_process_environment(
    class: ProcessEnvironmentClass,
    domestic: &BTreeMap<String, String>,
    session: &BTreeMap<String, String>,
    allowed_session_inputs: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>, String> {
    match class {
        ProcessEnvironmentClass::Session => {
            build_sealed_environment(domestic, session, allowed_session_inputs)
        }

        ProcessEnvironmentClass::Pure | ProcessEnvironmentClass::SystemInterface => {
            if !allowed_session_inputs.is_empty() {
                return Err(format!(
                    "process class {:?} cannot declare session inputs",
                    class
                ));
            }

            build_sealed_environment(domestic, session, &BTreeSet::new())
        }
    }
}

pub fn apply_sealed_environment(command: &mut Command, environment: &BTreeMap<String, String>) {
    command.env_clear();
    command.envs(environment);
}

pub fn build_process_command(
    program: impl AsRef<OsStr>,
    class: ProcessEnvironmentClass,
    domestic: &BTreeMap<String, String>,
    session: &BTreeMap<String, String>,
    allowed_session_inputs: &BTreeSet<String>,
) -> Result<Command, String> {
    let sealed = build_process_environment(class, domestic, session, allowed_session_inputs)?;

    let mut command = Command::new(program);

    apply_sealed_environment(&mut command, &sealed);

    Ok(command)
}

pub fn build_pure_process_command(program: impl AsRef<OsStr>) -> Result<Command, String> {
    build_process_command(
        program,
        ProcessEnvironmentClass::Pure,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeSet::new(),
    )
}

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::domestic_environment::ProcessEnvironmentClass;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedExecutionAuthority {
    pub executable: PathBuf,
    pub interpreter: PathBuf,
    pub library_paths: Vec<PathBuf>,
}

pub fn resolve_materialized_execution_authority(
    manifest_path: &Path,
    world_name: &str,
) -> Result<MaterializedExecutionAuthority, String> {
    let runtime_root =
        crate::domestic_runtime_authority::resolve_materialized_runtime_root(manifest_path)?;

    let executable = crate::domestic_runtime_authority::resolve_materialized_runtime_target(
        manifest_path,
        world_name,
        "executable",
    )?;

    let library_paths = crate::domestic_runtime_authority::resolve_materialized_runtime_targets(
        manifest_path,
        world_name,
        "library_paths",
    )?;

    for path in &library_paths {
        if !path.is_dir() {
            return Err(format!(
                "materialized execution library authority is not a directory for {world_name}: {}",
                path.display()
            ));
        }
    }

    let metadata = crate::domestic_elf::inspect_elf(&executable)?;

    let interpreter =
        crate::domestic_elf::resolve_interpreter_authority(
            &runtime_root,
            &metadata,
        )?
        .ok_or_else(|| {
            format!(
                "materialized execution authority requires a dynamic ELF interpreter for {world_name}"
            )
        })?;

    if !interpreter.is_file() {
        return Err(format!(
            "materialized execution interpreter is not a file for {world_name}: {}",
            interpreter.display()
        ));
    }

    Ok(MaterializedExecutionAuthority {
        executable,
        interpreter,
        library_paths,
    })
}

fn resolve_external_interpreter_authority(
    runtime_root: &Path,
    metadata: &crate::domestic_elf::DomesticElfMetadata,
) -> Result<PathBuf, String> {
    let declared = metadata.interpreter.as_ref().ok_or_else(|| {
        format!(
            "external execution authority requires a dynamic ELF interpreter: {}",
            metadata.path.display()
        )
    })?;

    const INSTALLED_RUNTIME_ROOT: &str = "/opt/neebles/client/runtime/boss/rootfs";

    let installed_root = Path::new(INSTALLED_RUNTIME_ROOT);

    if declared.starts_with(installed_root) {
        let relative = declared.strip_prefix(installed_root).map_err(|error| {
            format!(
                "could not relocate canonical Boss interpreter {}: {error}",
                declared.display()
            )
        })?;

        return crate::domestic_world::resolve_world_reference(
            runtime_root,
            runtime_root,
            relative,
        );
    }

    crate::domestic_elf::resolve_interpreter_authority(runtime_root, metadata)?.ok_or_else(|| {
        format!(
            "external execution authority requires a dynamic ELF interpreter: {}",
            metadata.path.display()
        )
    })
}

pub fn resolve_materialized_external_execution_authority(
    manifest_path: &Path,
    executable: &Path,
) -> Result<MaterializedExecutionAuthority, String> {
    if !manifest_path.is_absolute() {
        return Err(format!(
            "external bootstrap execution requires an absolute runtime manifest: {}",
            manifest_path.display()
        ));
    }

    if manifest_path.file_name().and_then(|value| value.to_str()) != Some("domestic-runtime.json") {
        return Err(format!(
            "external bootstrap execution requires domestic-runtime.json: {}",
            manifest_path.display()
        ));
    }

    let authority_directory = manifest_path.parent().ok_or_else(|| {
        format!(
            "external bootstrap execution manifest has no authority directory: {}",
            manifest_path.display()
        )
    })?;

    if authority_directory
        .file_name()
        .and_then(|value| value.to_str())
        != Some("runtime-authority")
    {
        return Err(format!(
            "external bootstrap execution requires runtime-authority staging: {}",
            authority_directory.display()
        ));
    }

    let staging_root = authority_directory.parent().ok_or_else(|| {
        format!(
            "external bootstrap execution authority has no staging root: {}",
            authority_directory.display()
        )
    })?;

    if !executable.is_absolute() {
        return Err(format!(
            "external execution target must be absolute: {}",
            executable.display()
        ));
    }

    if !executable.is_file() {
        return Err(format!(
            "external execution target is not a file: {}",
            executable.display()
        ));
    }

    let executable_parent = executable.parent().ok_or_else(|| {
        format!(
            "external bootstrap execution target has no parent: {}",
            executable.display()
        )
    })?;

    if executable_parent != staging_root {
        return Err(format!(
            "external bootstrap execution target is outside bootstrap staging: {}",
            executable.display()
        ));
    }

    let runtime_root =
        crate::domestic_runtime_authority::resolve_materialized_runtime_root(manifest_path)?;

    let installed_runtime_root = Path::new("/opt/neebles/client/runtime/boss/rootfs");

    if runtime_root == installed_runtime_root {
        return Err(
            "external bootstrap execution is forbidden with permanent Boss runtime authority"
                .to_string(),
        );
    }

    let library_paths =
        crate::domestic_runtime_authority::resolve_materialized_runtime_category_targets(
            manifest_path,
            "library_paths",
        )?;

    for path in &library_paths {
        if !path.is_dir() {
            return Err(format!(
                "external execution library authority is not a directory: {}",
                path.display()
            ));
        }
    }

    let metadata = crate::domestic_elf::inspect_elf(executable)?;

    let interpreter = resolve_external_interpreter_authority(&runtime_root, &metadata)?;

    if !interpreter.is_file() {
        return Err(format!(
            "external execution interpreter is not a file: {}",
            interpreter.display()
        ));
    }

    Ok(MaterializedExecutionAuthority {
        executable: executable.to_path_buf(),
        interpreter,
        library_paths,
    })
}

fn library_path_argument(paths: &[PathBuf]) -> Result<OsString, String> {
    std::env::join_paths(paths)
        .map_err(|error| format!("could not compose materialized execution library path: {error}"))
}

pub fn build_materialized_execution_command(
    manifest_path: &Path,
    world_name: &str,
    class: ProcessEnvironmentClass,
    domestic: &BTreeMap<String, String>,
    session: &BTreeMap<String, String>,
    allowed_session_inputs: &BTreeSet<String>,
) -> Result<Command, String> {
    let authority = resolve_materialized_execution_authority(manifest_path, world_name)?;

    let library_path = library_path_argument(&authority.library_paths)?;

    let mut command = crate::domestic_environment::build_process_command(
        &authority.interpreter,
        class,
        domestic,
        session,
        allowed_session_inputs,
    )?;

    command
        .arg("--inhibit-cache")
        .arg("--library-path")
        .arg(library_path)
        .arg(&authority.executable);

    Ok(command)
}

pub fn build_materialized_external_session_execution_command(
    manifest_path: &Path,
    executable: &Path,
    executable_arguments: &[OsString],
) -> Result<Command, String> {
    let authority = resolve_materialized_external_execution_authority(manifest_path, executable)?;

    let library_path = library_path_argument(&authority.library_paths)?;

    let mut command = Command::new(&authority.interpreter);

    command
        .arg("--inhibit-cache")
        .arg("--library-path")
        .arg(library_path)
        .arg(&authority.executable)
        .args(executable_arguments);

    for key in [
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
    ] {
        command.env_remove(key);
    }

    Ok(command)
}

pub fn build_pure_materialized_execution_command(
    manifest_path: &Path,
    world_name: &str,
) -> Result<Command, String> {
    build_materialized_execution_command(
        manifest_path,
        world_name,
        ProcessEnvironmentClass::Pure,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeSet::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ffi::OsString;
    use std::fs;

    #[test]
    fn relocated_execution_authority_composes_existing_owners() {
        let base = std::env::temp_dir().join(format!(
            "neebles-execution-authority-{}",
            std::process::id()
        ));

        if base.exists() {
            fs::remove_dir_all(&base).expect("old execution fixture must be removable");
        }

        let authority_a = base.join("authority-a");

        let authority_b = base.join("authority-b");

        let runtime_root = authority_a.join("rootfs");

        let executable = runtime_root.join("usr/bin/tool");

        let multiarch = runtime_root.join("usr/lib/x86_64-linux-gnu");

        let systemd = multiarch.join("systemd");

        fs::create_dir_all(
            executable
                .parent()
                .expect("fixture executable must have parent"),
        )
        .expect("fixture executable parent must exist");

        fs::create_dir_all(&systemd).expect("fixture library authorities must exist");

        let current = std::env::current_exe().expect("test executable must exist");

        fs::copy(&current, &executable).expect("test ELF must copy into fixture");

        let metadata =
            crate::domestic_elf::inspect_elf(&executable).expect("fixture executable must be ELF");

        let declared_interpreter = metadata
            .interpreter
            .clone()
            .expect("test binary must be dynamically linked");

        assert!(declared_interpreter.is_absolute());

        let loader = runtime_root.join(
            declared_interpreter
                .strip_prefix("/")
                .expect("absolute interpreter must strip root"),
        );

        fs::create_dir_all(loader.parent().expect("loader must have parent"))
            .expect("loader parent must exist");

        fs::write(&loader, b"fixture-loader").expect("fixture loader must exist");

        let manifest = authority_a.join("domestic-runtime.json");

        let value = serde_json::json!({
            "schema": "1",
            "name": "neebles-domestic-runtime",
            "root": "rootfs",
            "worlds": {
                "boss.fixture": {
                    "categories": {
                        "executable": {
                            "category": "executable",
                            "value": "fixture",
                            "declared_targets": [
                                "usr/bin/tool"
                            ],
                            "resolved_targets": [
                                "usr/bin/tool"
                            ]
                        },
                        "library_paths": {
                            "category": "library_paths",
                            "value": "fixture-libraries",
                            "declared_targets": [
                                "usr/lib/x86_64-linux-gnu",
                                "usr/lib/x86_64-linux-gnu/systemd"
                            ],
                            "resolved_targets": [
                                "usr/lib/x86_64-linux-gnu",
                                "usr/lib/x86_64-linux-gnu/systemd"
                            ]
                        }
                    }
                }
            }
        });

        fs::write(
            &manifest,
            serde_json::to_vec_pretty(&value).expect("fixture manifest must serialize"),
        )
        .expect("fixture manifest must exist");

        fs::rename(&authority_a, &authority_b).expect("execution authority must relocate");

        let relocated_manifest = authority_b.join("domestic-runtime.json");

        let relocated_root = authority_b.join("rootfs");

        let authority =
            resolve_materialized_execution_authority(&relocated_manifest, "boss.fixture")
                .expect("relocated execution authority must resolve");

        assert_eq!(authority.executable, relocated_root.join("usr/bin/tool"));

        assert_eq!(
            authority.interpreter,
            relocated_root.join(
                declared_interpreter
                    .strip_prefix("/")
                    .expect("interpreter root must strip"),
            )
        );

        assert_eq!(
            authority.library_paths,
            vec![
                relocated_root.join("usr/lib/x86_64-linux-gnu"),
                relocated_root.join("usr/lib/x86_64-linux-gnu/systemd"),
            ]
        );

        let command =
            build_pure_materialized_execution_command(&relocated_manifest, "boss.fixture")
                .expect("execution command must compose");

        assert_eq!(command.get_program(), authority.interpreter.as_os_str());

        let joined =
            std::env::join_paths(&authority.library_paths).expect("fixture library path must join");

        let args = command
            .get_args()
            .map(|value| value.to_os_string())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec![
                OsString::from("--inhibit-cache"),
                OsString::from("--library-path"),
                joined,
                authority.executable.into_os_string(),
            ]
        );

        fs::remove_dir_all(&base).expect("execution fixture must be removable");
    }
}

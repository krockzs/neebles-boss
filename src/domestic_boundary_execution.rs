use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::domestic_external_data_authority::ExternalDataAuthorityDescriptor;
use crate::domestic_filesystem_boundary::{
    build_filesystem_boundary_command, compose_filesystem_boundary_plan_with_writable_data,
    project_path_into_boundary, FilesystemBoundaryPlan,
};
use crate::domestic_platform_authority::PlatformAuthorityDescriptor;
use crate::domestic_writable_data_authority::WritableDataGrant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedBoundaryExecutionPlan {
    pub boundary: FilesystemBoundaryPlan,
    pub interpreter_inside: PathBuf,
    pub executable_inside: PathBuf,
    pub library_paths_inside: Vec<PathBuf>,
}

pub fn compose_materialized_boundary_execution_plan(
    manifest_path: &Path,
    world_name: &str,
    platform: &PlatformAuthorityDescriptor,
    readonly_data: &[ExternalDataAuthorityDescriptor],
    mount_proc: bool,
    mount_dev: bool,
    mount_tmp: bool,
    chdir: Option<&Path>,
) -> Result<MaterializedBoundaryExecutionPlan, String> {
    compose_materialized_boundary_execution_plan_with_writable_data(
        manifest_path,
        world_name,
        platform,
        readonly_data,
        &[],
        mount_proc,
        mount_dev,
        mount_tmp,
        chdir,
    )
}

pub fn compose_materialized_boundary_execution_plan_with_writable_data(
    manifest_path: &Path,
    world_name: &str,
    platform: &PlatformAuthorityDescriptor,
    readonly_data: &[ExternalDataAuthorityDescriptor],
    writable_data: &[WritableDataGrant],
    mount_proc: bool,
    mount_dev: bool,
    mount_tmp: bool,
    chdir: Option<&Path>,
) -> Result<MaterializedBoundaryExecutionPlan, String> {
    let runtime_root =
        crate::domestic_runtime_authority::resolve_materialized_runtime_root(manifest_path)?;

    let execution = crate::domestic_execution_authority::resolve_materialized_execution_authority(
        manifest_path,
        world_name,
    )?;

    let boundary = compose_filesystem_boundary_plan_with_writable_data(
        platform,
        &runtime_root,
        readonly_data,
        writable_data,
        mount_proc,
        mount_dev,
        mount_tmp,
        chdir,
    )?;

    let interpreter_inside = project_path_into_boundary(&runtime_root, &execution.interpreter)?;
    let executable_inside = project_path_into_boundary(&runtime_root, &execution.executable)?;
    let mut library_paths_inside = Vec::<PathBuf>::new();

    for library_path in execution.library_paths {
        library_paths_inside.push(project_path_into_boundary(&runtime_root, &library_path)?);
    }

    Ok(MaterializedBoundaryExecutionPlan {
        boundary,
        interpreter_inside,
        executable_inside,
        library_paths_inside,
    })
}

fn inside_library_path_argument(paths: &[PathBuf]) -> Result<OsString, String> {
    std::env::join_paths(paths)
        .map_err(|error| format!("could not compose boundary execution library path: {error}"))
}

pub fn build_pure_materialized_boundary_execution_command(
    plan: &MaterializedBoundaryExecutionPlan,
    executable_arguments: &[OsString],
) -> Result<Command, String> {
    let library_path = inside_library_path_argument(&plan.library_paths_inside)?;

    let mut interpreter_arguments = Vec::<OsString>::new();

    interpreter_arguments.push(OsString::from("--inhibit-cache"));

    interpreter_arguments.push(OsString::from("--library-path"));

    interpreter_arguments.push(library_path);

    interpreter_arguments.push(plan.executable_inside.clone().into_os_string());

    interpreter_arguments.extend_from_slice(executable_arguments);

    build_filesystem_boundary_command(
        &plan.boundary,
        &plan.interpreter_inside,
        &interpreter_arguments,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domestic_platform_authority::test_platform_authority_descriptor;

    use crate::domestic_elf::inspect_elf;
    use crate::domestic_filesystem_boundary::FILESYSTEM_BOUNDARY_PROTOCOL_V1;

    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock must work")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "neebles-boundary-execution-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    fn prepare_relocated_authority(base: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let authority_a = base.join("authority-a");

        let authority_b = base.join("authority-b");

        let runtime_root = authority_a.join("rootfs");

        let executable = runtime_root.join("usr/bin/tool");

        let multiarch = runtime_root.join("usr/lib/x86_64-linux-gnu");

        fs::create_dir_all(
            executable
                .parent()
                .expect("fixture executable must have parent"),
        )
        .expect("fixture executable parent must exist");

        fs::create_dir_all(&multiarch).expect("fixture library directory must exist");

        let current = std::env::current_exe().expect("test executable must exist");

        fs::copy(&current, &executable).expect("fixture executable must copy");

        let metadata = inspect_elf(&executable).expect("fixture executable must be ELF");

        let declared_interpreter = metadata
            .interpreter
            .expect("fixture executable must be dynamic");

        let interpreter = runtime_root.join(
            declared_interpreter
                .strip_prefix("/")
                .expect("declared interpreter must be absolute"),
        );

        fs::create_dir_all(
            interpreter
                .parent()
                .expect("fixture interpreter must have parent"),
        )
        .expect("fixture interpreter parent must exist");

        fs::write(&interpreter, b"fixture-loader").expect("fixture interpreter must exist");

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
                                "usr/lib/x86_64-linux-gnu"
                            ],
                            "resolved_targets": [
                                "usr/lib/x86_64-linux-gnu"
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

        fs::rename(&authority_a, &authority_b).expect("fixture authority must relocate");

        (
            authority_b.join("domestic-runtime.json"),
            authority_b.join("rootfs"),
            PathBuf::from(declared_interpreter),
        )
    }

    #[test]
    fn relocated_execution_authority_projects_inside_boundary() {
        let base = fixture_root("projection");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let (manifest, relocated_root, declared_interpreter) = prepare_relocated_authority(&base);

        let provider = base.join("provider");

        fs::write(&provider, b"provider").expect("fixture provider must exist");

        let platform = test_platform_authority_descriptor(
            "platform.filesystem_boundary".to_string(),
            base.join("platform.json"),
            FILESYSTEM_BOUNDARY_PROTOCOL_V1.to_string(),
            provider,
        );

        let plan = compose_materialized_boundary_execution_plan(
            &manifest,
            "boss.fixture",
            &platform,
            &[],
            true,
            true,
            true,
            Some(Path::new("/tmp")),
        )
        .expect("boundary execution plan must compose");

        assert_eq!(plan.boundary.root, relocated_root,);

        assert_eq!(plan.executable_inside, PathBuf::from("/usr/bin/tool",),);

        assert_eq!(plan.interpreter_inside, declared_interpreter,);

        assert_eq!(
            plan.library_paths_inside,
            vec![PathBuf::from("/usr/lib/x86_64-linux-gnu",),],
        );

        fs::remove_dir_all(&base).expect("fixture must clean");
    }

    #[test]
    fn boundary_execution_command_composes_one_provider_command() {
        let base = fixture_root("command");

        fs::create_dir_all(&base).expect("fixture base must exist");

        let (manifest, relocated_root, declared_interpreter) = prepare_relocated_authority(&base);

        let provider = base.join("provider");

        fs::write(&provider, b"provider").expect("fixture provider must exist");

        let platform = test_platform_authority_descriptor(
            "platform.filesystem_boundary".to_string(),
            base.join("platform.json"),
            FILESYSTEM_BOUNDARY_PROTOCOL_V1.to_string(),
            provider.clone(),
        );

        let plan = compose_materialized_boundary_execution_plan(
            &manifest,
            "boss.fixture",
            &platform,
            &[],
            true,
            true,
            true,
            Some(Path::new("/tmp")),
        )
        .expect("boundary execution plan must compose");

        let command = build_pure_materialized_boundary_execution_command(
            &plan,
            &[OsString::from("--fixture")],
        )
        .expect("boundary execution command must compose");

        assert_eq!(command.get_program(), provider.as_os_str(),);

        let args = command
            .get_args()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        let root_string = relocated_root.to_string_lossy().to_string();

        let interpreter_string = declared_interpreter.to_string_lossy().to_string();

        assert_eq!(
            args,
            vec![
                "--root".to_string(),
                root_string,
                "--proc".to_string(),
                "--dev".to_string(),
                "--tmp".to_string(),
                "--chdir".to_string(),
                "/tmp".to_string(),
                "--".to_string(),
                interpreter_string,
                "--inhibit-cache".to_string(),
                "--library-path".to_string(),
                "/usr/lib/x86_64-linux-gnu".to_string(),
                "/usr/bin/tool".to_string(),
                "--fixture".to_string(),
            ],
        );

        assert!(
            args.iter().all(|value| { !value.contains("bwrap",) },),
            "compositor must speak only NEEBLES provider protocol",
        );

        assert!(
            args.iter()
                .all(|value| { !value.contains("authority-a",) },),
            "command must not retain pre-relocation authority paths",
        );

        assert!(
            args.iter()
                .skip(8)
                .all(|value| { !value.contains("authority-b",) },),
            "program-side execution arguments must use inside-boundary paths",
        );

        fs::remove_dir_all(&base).expect("fixture must clean");
    }
}

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use crate::domestic_boundary_execution::{
    build_pure_materialized_boundary_execution_command,
    compose_materialized_boundary_execution_plan_with_writable_data,
};
use crate::domestic_dynamic_readonly_authority::{
    grant_dynamic_readonly_subpath, load_dynamic_readonly_authority_descriptor,
};
use crate::domestic_external_data_authority::{
    load_external_data_authority_descriptor, ExternalDataAuthorityDescriptor,
};
use crate::domestic_platform_authority::load_platform_authority_descriptor;
use crate::domestic_writable_data_authority::{
    grant_writable_data_subpath, load_writable_data_authority_descriptor, WritableDataGrant,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceReadonlyGrant {
    pub authority: String,
    pub descriptor_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDynamicReadonlyGrant {
    pub authority: String,
    pub descriptor_path: PathBuf,
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceWritableGrant {
    pub authority: String,
    pub descriptor_path: PathBuf,
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSessionReadonlyGrant {
    pub authority: String,
    pub descriptor_path: PathBuf,
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceExecutionRequest {
    pub manifest_path: PathBuf,
    pub world: String,
    pub platform_descriptor_path: PathBuf,
    pub readonly: Vec<WorkspaceReadonlyGrant>,
    pub dynamic_readonly: Vec<WorkspaceDynamicReadonlyGrant>,
    pub writable: Vec<WorkspaceWritableGrant>,
    pub session_readonly: Vec<WorkspaceSessionReadonlyGrant>,
    pub environment: BTreeMap<String, String>,
    pub desktop_identity: Option<(u32, u32)>,
    pub mount_proc: bool,
    pub mount_dev: bool,
    pub mount_tmp: bool,
    pub chdir: Option<PathBuf>,
    pub arguments: Vec<OsString>,
}

fn load_readonly_grants(
    requests: &[WorkspaceReadonlyGrant],
) -> Result<Vec<ExternalDataAuthorityDescriptor>, String> {
    requests
        .iter()
        .map(|request| {
            load_external_data_authority_descriptor(&request.descriptor_path, &request.authority)
        })
        .collect()
}

fn load_dynamic_readonly_grants(
    requests: &[WorkspaceDynamicReadonlyGrant],
) -> Result<Vec<ExternalDataAuthorityDescriptor>, String> {
    requests
        .iter()
        .map(|request| {
            let descriptor = load_dynamic_readonly_authority_descriptor(
                &request.descriptor_path,
                &request.authority,
            )?;

            let grant =
                grant_dynamic_readonly_subpath(&descriptor, &request.source, &request.destination)?;

            Ok(ExternalDataAuthorityDescriptor {
                authority: grant.authority,
                descriptor_path: request.descriptor_path.clone(),
                source: grant.source,
                destination: grant.destination,
                access: "read_only".to_string(),
            })
        })
        .collect()
}

fn load_session_readonly_grants(
    requests: &[WorkspaceSessionReadonlyGrant],
    desktop_identity: Option<(u32, u32)>,
) -> Result<Vec<ExternalDataAuthorityDescriptor>, String> {
    if requests.is_empty() {
        return Ok(Vec::new());
    }

    let Some((desktop_uid, desktop_gid)) = desktop_identity else {
        return Err("workspace session readonly grants require desktop identity".to_string());
    };

    requests
        .iter()
        .map(|request| {
            let descriptor =
                load_platform_authority_descriptor(
                    &request.descriptor_path,
                    &request.authority,
                )?;

            if descriptor.authority()
                != crate::domestic_desktop_session_interface::
                    DESKTOP_SESSION_INTERFACE_AUTHORITY
            {
                return Err(format!(
                    "workspace session readonly authority mismatch: {}",
                    descriptor.authority()
                ));
            }

            if descriptor.protocol()
                != crate::domestic_desktop_session_interface::
                    DESKTOP_SESSION_INTERFACE_PROTOCOL_V1
            {
                return Err(format!(
                    "workspace session readonly protocol mismatch: {}",
                    descriptor.protocol()
                ));
            }

            if !request.source.is_absolute()
                || !request.destination.is_absolute()
            {
                return Err(
                    "workspace session readonly paths must be absolute"
                        .to_string(),
                );
            }

            if request.source != request.destination {
                return Err(format!(
                    "workspace session readonly projection must preserve canonical path: {} -> {}",
                    request.source.display(),
                    request.destination.display(),
                ));
            }

            let interface =
                crate::domestic_desktop_session_interface::
                    resolve_desktop_session_interface_from_descriptor(
                        &descriptor,
                        desktop_uid as libc::uid_t,
                        desktop_gid as libc::gid_t,
                    )?;

            let explicit = interface
                .readonly_paths()
                .iter()
                .any(|path| {
                    path.source == request.source
                        && path.destination
                            == request.destination
                });

            if !explicit {
                let runtime_root =
                    std::fs::canonicalize(
                        interface.runtime_dir(),
                    )
                    .map_err(|error| {
                        format!(
                            "could not canonicalize workspace desktop runtime directory {}: {error}",
                            interface.runtime_dir()
                        )
                    })?;

                let canonical_source =
                    std::fs::canonicalize(
                        &request.source,
                    )
                    .map_err(|error| {
                        format!(
                            "could not canonicalize workspace session readonly source {}: {error}",
                            request.source.display()
                        )
                    })?;

                if canonical_source
                    != request.source
                {
                    return Err(format!(
                        "workspace session readonly source is not canonical: {}",
                        request.source.display()
                    ));
                }

                let relative =
                    canonical_source
                        .strip_prefix(
                            &runtime_root,
                        )
                        .map_err(|_| {
                            format!(
                                "workspace session readonly source is not owned by certified desktop runtime directory: {}",
                                canonical_source.display()
                            )
                        })?;

                let granted =
                    interface
                        .grant_runtime_readonly(
                            relative,
                        )?;

                if granted.source
                    != request.source
                    || granted.destination
                        != request.destination
                {
                    return Err(format!(
                        "workspace session readonly grant does not match certified desktop-session authority: {}",
                        request.source.display()
                    ));
                }
            }

            Ok(
                ExternalDataAuthorityDescriptor {
                    authority:
                        request.authority.clone(),
                    descriptor_path:
                        request
                            .descriptor_path
                            .clone(),
                    source:
                        request.source.clone(),
                    destination:
                        request
                            .destination
                            .clone(),
                    access:
                        "read_only".to_string(),
                },
            )
        })
        .collect()
}

fn load_writable_grants(
    requests: &[WorkspaceWritableGrant],
) -> Result<Vec<WritableDataGrant>, String> {
    requests
        .iter()
        .map(|request| {
            let descriptor = load_writable_data_authority_descriptor(
                &request.descriptor_path,
                &request.authority,
            )?;

            grant_writable_data_subpath(&descriptor, &request.source, &request.destination)
        })
        .collect()
}

pub fn validate_workspace_execution_request(
    request: &WorkspaceExecutionRequest,
) -> Result<(), String> {
    if request.world.trim().is_empty() {
        return Err("workspace execution world cannot be empty".to_string());
    }

    if !request.manifest_path.is_absolute() {
        return Err(format!(
            "workspace execution manifest path must be absolute: {}",
            request.manifest_path.display()
        ));
    }

    if !request.platform_descriptor_path.is_absolute() {
        return Err(format!(
            "workspace execution platform descriptor path must be absolute: {}",
            request.platform_descriptor_path.display()
        ));
    }

    if let Some(chdir) = &request.chdir {
        if !chdir.is_absolute() {
            return Err(format!(
                "workspace execution chdir must be absolute: {}",
                chdir.display()
            ));
        }
    }

    Ok(())
}

pub fn build_workspace_execution_command(
    request: &WorkspaceExecutionRequest,
) -> Result<Command, String> {
    validate_workspace_execution_request(request)?;

    let platform = load_platform_authority_descriptor(
        &request.platform_descriptor_path,
        "platform.filesystem_boundary",
    )?;

    let mut readonly = load_readonly_grants(&request.readonly)?;
    readonly.extend(load_dynamic_readonly_grants(&request.dynamic_readonly)?);
    readonly.extend(load_session_readonly_grants(
        &request.session_readonly,
        request.desktop_identity,
    )?);

    let writable = load_writable_grants(&request.writable)?;

    let plan = compose_materialized_boundary_execution_plan_with_writable_data(
        &request.manifest_path,
        &request.world,
        &platform,
        &readonly,
        &writable,
        &request.environment,
        request.mount_proc,
        request.mount_dev,
        request.mount_tmp,
        request.chdir.as_deref(),
    )?;

    let boundary = build_pure_materialized_boundary_execution_command(&plan, &request.arguments)?;

    let Some((desktop_uid, desktop_gid)) = request.desktop_identity else {
        return Ok(boundary);
    };

    let setpriv = crate::domestic_runtime_authority::resolve_boss_executable("boss.setpriv")?;

    let mut command = Command::new(setpriv);

    command.env_clear();

    command
        .arg(format!("--reuid={desktop_uid}"))
        .arg(format!("--regid={desktop_gid}"))
        .arg("--init-groups")
        .arg(boundary.get_program())
        .args(boundary.get_args());

    Ok(command)
}

pub fn execute_workspace_execution_request(
    request: &WorkspaceExecutionRequest,
) -> Result<i32, String> {
    let status = build_workspace_execution_command(request)?
        .status()
        .map_err(|error| format!("could not execute domestic workspace: {error}"))?;

    Ok(status.code().unwrap_or(125))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_request_rejects_empty_world_before_touching_authorities() {
        let request = WorkspaceExecutionRequest {
            manifest_path: PathBuf::from("/fixture/runtime.json"),
            world: String::new(),
            platform_descriptor_path: PathBuf::from("/fixture/platform.json"),
            readonly: Vec::new(),
            dynamic_readonly: Vec::new(),
            writable: Vec::new(),
            session_readonly: Vec::new(),
            environment: std::collections::BTreeMap::new(),
            desktop_identity: None,
            mount_proc: false,
            mount_dev: false,
            mount_tmp: false,
            chdir: None,
            arguments: Vec::new(),
        };

        assert!(build_workspace_execution_command(&request).is_err());
    }

    #[test]
    fn workspace_request_requires_absolute_manifest() {
        let request = WorkspaceExecutionRequest {
            manifest_path: PathBuf::from("runtime.json"),
            world: "fixture.tool".to_string(),
            platform_descriptor_path: PathBuf::from("/fixture/platform.json"),
            readonly: Vec::new(),
            dynamic_readonly: Vec::new(),
            writable: Vec::new(),
            session_readonly: Vec::new(),
            environment: std::collections::BTreeMap::new(),
            desktop_identity: None,
            mount_proc: false,
            mount_dev: false,
            mount_tmp: false,
            chdir: None,
            arguments: Vec::new(),
        };

        assert!(build_workspace_execution_command(&request).is_err());
    }

    #[test]
    fn workspace_request_requires_absolute_platform_descriptor() {
        let request = WorkspaceExecutionRequest {
            manifest_path: PathBuf::from("/fixture/runtime.json"),
            world: "fixture.tool".to_string(),
            platform_descriptor_path: PathBuf::from("platform.json"),
            readonly: Vec::new(),
            dynamic_readonly: Vec::new(),
            writable: Vec::new(),
            session_readonly: Vec::new(),
            environment: std::collections::BTreeMap::new(),
            desktop_identity: None,
            mount_proc: false,
            mount_dev: false,
            mount_tmp: false,
            chdir: None,
            arguments: Vec::new(),
        };

        assert!(build_workspace_execution_command(&request).is_err());
    }

    #[test]
    fn workspace_request_requires_absolute_chdir() {
        let request = WorkspaceExecutionRequest {
            manifest_path: PathBuf::from("/fixture/runtime.json"),
            world: "fixture.tool".to_string(),
            platform_descriptor_path: PathBuf::from("/fixture/platform.json"),
            readonly: Vec::new(),
            dynamic_readonly: Vec::new(),
            writable: Vec::new(),
            session_readonly: Vec::new(),
            environment: std::collections::BTreeMap::new(),
            desktop_identity: None,
            mount_proc: false,
            mount_dev: false,
            mount_tmp: false,
            chdir: Some(PathBuf::from("work")),
            arguments: Vec::new(),
        };

        assert!(build_workspace_execution_command(&request).is_err());
    }

    #[test]
    fn dynamic_readonly_grant_lowers_to_read_only_external_data() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock must work")
            .as_nanos();

        let base = std::env::temp_dir().join(format!(
            "neebles-workspace-dynamic-readonly-{}-{unique}",
            std::process::id()
        ));

        let root = base.join("root");
        let selected = root.join("module");
        let descriptor_path = base.join("descriptor.json");

        fs::create_dir_all(&selected).expect("selected directory must exist");

        fs::write(
            &descriptor_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "1",
                "name": "neebles-dynamic-readonly-authority",
                "authority": "fixture.dynamic",
                "root": root,
            }))
            .expect("descriptor must serialize"),
        )
        .expect("descriptor must exist");

        let grants = load_dynamic_readonly_grants(&[WorkspaceDynamicReadonlyGrant {
            authority: "fixture.dynamic".to_string(),
            descriptor_path: descriptor_path.clone(),
            source: selected.clone(),
            destination: PathBuf::from("/modules/fixture"),
        }])
        .expect("dynamic readonly grant must lower");

        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].authority, "fixture.dynamic");
        assert_eq!(grants[0].source, fs::canonicalize(selected).unwrap());
        assert_eq!(grants[0].destination, PathBuf::from("/modules/fixture"));
        assert_eq!(grants[0].access, "read_only");

        fs::remove_dir_all(base).expect("fixture must clean");
    }

    #[test]
    fn shared_execution_primitive_reuses_workspace_validation() {
        let request = WorkspaceExecutionRequest {
            manifest_path: PathBuf::from("/fixture/runtime.json"),
            world: String::new(),
            platform_descriptor_path: PathBuf::from("/fixture/platform.json"),
            readonly: Vec::new(),
            dynamic_readonly: Vec::new(),
            writable: Vec::new(),
            session_readonly: Vec::new(),
            environment: std::collections::BTreeMap::new(),
            desktop_identity: None,
            mount_proc: false,
            mount_dev: false,
            mount_tmp: false,
            chdir: None,
            arguments: Vec::new(),
        };

        let error = execute_workspace_execution_request(&request)
            .expect_err("invalid workspace request must fail before process execution");

        assert!(error.contains("world"));
    }
}

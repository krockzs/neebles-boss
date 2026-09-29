use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use crate::domestic_boundary_execution::{
    build_pure_materialized_boundary_execution_command,
    compose_materialized_boundary_execution_plan_with_writable_data,
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
pub struct WorkspaceWritableGrant {
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
    pub writable: Vec<WorkspaceWritableGrant>,
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

    let readonly = load_readonly_grants(&request.readonly)?;
    let writable = load_writable_grants(&request.writable)?;

    let plan = compose_materialized_boundary_execution_plan_with_writable_data(
        &request.manifest_path,
        &request.world,
        &platform,
        &readonly,
        &writable,
        request.mount_proc,
        request.mount_dev,
        request.mount_tmp,
        request.chdir.as_deref(),
    )?;

    build_pure_materialized_boundary_execution_command(&plan, &request.arguments)
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
            writable: Vec::new(),
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
            writable: Vec::new(),
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
            writable: Vec::new(),
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
            writable: Vec::new(),
            mount_proc: false,
            mount_dev: false,
            mount_tmp: false,
            chdir: Some(PathBuf::from("work")),
            arguments: Vec::new(),
        };

        assert!(build_workspace_execution_command(&request).is_err());
    }
}

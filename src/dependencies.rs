use crate::local_installer::{self, LocalInstallerRequest};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DependencySet {
    #[serde(default)]
    pub system: Vec<SystemDependency>,
    #[serde(default)]
    pub modules: Vec<ModuleDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDependency {
    pub name: String,
    pub install: LocalInstallerRequest,
    pub verify: LocalInstallerRequest,
    #[serde(default = "default_required")]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependency {
    pub name: String,
    #[serde(default)]
    pub minimum_version: Option<String>,
    #[serde(default = "default_required")]
    pub required: bool,
}

fn default_required() -> bool {
    true
}

pub fn resolve_system_dependencies(
    dependencies: &[SystemDependency],
) -> Result<(), String> {
    for dependency in dependencies {
        match resolve_one(dependency) {
            Ok(()) => {}
            Err(error) if dependency.required => {
                return Err(error);
            }
            Err(error) => {
                eprintln!(
                    "N.E.E.B.L.E.S.: optional dependency '{}' could not be resolved: {error}",
                    dependency.name
                );
            }
        }
    }

    Ok(())
}

fn resolve_one(
    dependency: &SystemDependency,
) -> Result<(), String> {
    if request_succeeds(&dependency.verify)? {
        return Ok(());
    }

    let install_result =
        local_installer::handle(
            dependency.install.clone(),
        )
        .map_err(|error| {
            format!(
                "could not install dependency '{}': {error}",
                dependency.name
            )
        })?;

    if !install_result.success {
        return Err(format!(
            "dependency '{}' installation failed: command='{}' args={:?} exit_code={} stderr='{}'",
            dependency.name,
            install_result.command,
            install_result.args,
            install_result.exit_code,
            install_result.stderr.trim()
        ));
    }

    if request_succeeds(&dependency.verify)? {
        return Ok(());
    }

    Err(format!(
        "dependency '{}' installation completed but verification still failed",
        dependency.name
    ))
}

fn request_succeeds(
    request: &LocalInstallerRequest,
) -> Result<bool, String> {
    let result =
        local_installer::handle(
            request.clone(),
        )?;

    Ok(result.success)
}

pub fn command_exists(command: &str) -> bool {
    if command.contains('/') {
        return Path::new(command).exists();
    }

    env::var_os("PATH")
        .map(|paths| {
            env::split_paths(&paths)
                .any(|path| path.join(command).exists())
        })
        .unwrap_or(false)
}

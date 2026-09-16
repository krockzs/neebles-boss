use crate::local_installer::{self, LocalInstallerRequest};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;
use std::process::Command;

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
    #[serde(default)]
    pub version: Option<SystemDependencyVersion>,
    #[serde(default = "default_required")]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDependencyVersion {
    #[serde(default)]
    pub scheme: SystemVersionScheme,
    pub requirement: String,
    pub query: LocalInstallerRequest,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SystemVersionScheme {
    #[default]
    Semver,
    Debian,
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

pub fn resolve_system_dependencies(dependencies: &[SystemDependency]) -> Result<(), String> {
    resolve_system_dependencies_with(dependencies, &local_installer::handle)
}

fn resolve_system_dependencies_with<F>(
    dependencies: &[SystemDependency],
    execute: &F,
) -> Result<(), String>
where
    F: Fn(LocalInstallerRequest) -> Result<crate::local_installer::LocalInstallerResult, String>,
{
    for dependency in dependencies {
        match resolve_one(dependency, execute) {
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

fn resolve_one<F>(dependency: &SystemDependency, execute: &F) -> Result<(), String>
where
    F: Fn(LocalInstallerRequest) -> Result<crate::local_installer::LocalInstallerResult, String>,
{
    if dependency_satisfied(dependency, execute)? {
        return Ok(());
    }

    let install_result = execute(dependency.install.clone()).map_err(|error| {
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

    if dependency_satisfied(dependency, execute)? {
        return Ok(());
    }

    Err(format!(
        "dependency '{}' installation completed but dependency contract is still not satisfied",
        dependency.name
    ))
}

fn dependency_satisfied<F>(dependency: &SystemDependency, execute: &F) -> Result<bool, String>
where
    F: Fn(LocalInstallerRequest) -> Result<crate::local_installer::LocalInstallerResult, String>,
{
    if !request_succeeds(&dependency.verify, execute)? {
        return Ok(false);
    }

    let Some(version) = &dependency.version else {
        return Ok(true);
    };

    let result = execute(version.query.clone()).map_err(|error| {
        format!(
            "could not query version for dependency '{}': {error}",
            dependency.name
        )
    })?;

    if !result.success {
        return Err(format!(
            "version query for dependency '{}' failed: command='{}' args={:?} exit_code={} stderr='{}'",
            dependency.name,
            result.command,
            result.args,
            result.exit_code,
            result.stderr.trim()
        ));
    }

    let detected = result.stdout.trim();

    if detected.is_empty() {
        return Err(format!(
            "version query for dependency '{}' returned empty stdout",
            dependency.name
        ));
    }

    match version.scheme {
        SystemVersionScheme::Semver => {
            let requirement = VersionReq::parse(version.requirement.trim()).map_err(|error| {
                format!(
                    "dependency '{}' declares invalid semantic version requirement '{}': {error}",
                    dependency.name, version.requirement
                )
            })?;

            let detected = Version::parse(detected).map_err(|error| {
                format!(
                    "version query for dependency '{}' returned invalid semantic version '{}': {error}",
                    dependency.name, detected
                )
            })?;

            Ok(requirement.matches(&detected))
        }
        SystemVersionScheme::Debian => {
            debian_requirement_matches(&dependency.name, detected, &version.requirement)
        }
    }
}

fn debian_requirement_matches(
    dependency_name: &str,
    detected: &str,
    requirement: &str,
) -> Result<bool, String> {
    let clauses: Vec<&str> = requirement
        .split(',')
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .collect();

    if clauses.is_empty() {
        return Err(format!(
            "dependency '{}' declares an empty Debian version requirement",
            dependency_name
        ));
    }

    for clause in clauses {
        let (operator, required) = parse_debian_requirement_clause(clause).map_err(|error| {
            format!(
                "dependency '{}' declares invalid Debian version requirement '{}': {error}",
                dependency_name, requirement
            )
        })?;

        let status = Command::new("dpkg")
            .args(["--compare-versions", detected, operator, required])
            .status()
            .map_err(|error| {
                format!(
                    "could not start dpkg while comparing version for dependency '{}': {error}",
                    dependency_name
                )
            })?;

        match status.code() {
            Some(0) => {}
            Some(1) => return Ok(false),
            Some(code) => {
                return Err(format!(
                    "dpkg --compare-versions failed for dependency '{}' with exit code {}",
                    dependency_name, code
                ));
            }
            None => {
                return Err(format!(
                    "dpkg --compare-versions terminated without an exit code for dependency '{}'",
                    dependency_name
                ));
            }
        }
    }

    Ok(true)
}

pub fn validate_debian_requirement(requirement: &str) -> Result<(), String> {
    let clauses: Vec<&str> = requirement
        .split(',')
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .collect();

    if clauses.is_empty() {
        return Err("Debian version requirement is empty".to_string());
    }

    for clause in clauses {
        parse_debian_requirement_clause(clause)?;
    }

    Ok(())
}

fn parse_debian_requirement_clause(clause: &str) -> Result<(&'static str, &str), String> {
    for (prefix, operator) in [
        (">=", "ge"),
        ("<=", "le"),
        (">", "gt"),
        ("<", "lt"),
        ("==", "eq"),
        ("=", "eq"),
    ] {
        if let Some(required) = clause.strip_prefix(prefix) {
            let required = required.trim();

            if required.is_empty() {
                return Err(format!("clause '{}' does not contain a version", clause));
            }

            return Ok((operator, required));
        }
    }

    Err(format!(
        "clause '{}' must begin with >=, <=, >, <, == or =",
        clause
    ))
}

fn request_succeeds<F>(request: &LocalInstallerRequest, execute: &F) -> Result<bool, String>
where
    F: Fn(LocalInstallerRequest) -> Result<crate::local_installer::LocalInstallerResult, String>,
{
    let result = execute(request.clone())?;

    Ok(result.success)
}

pub fn command_exists(command: &str) -> bool {
    if command.contains('/') {
        return Path::new(command).exists();
    }

    env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).any(|path| path.join(command).exists()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debian_requirement_accepts_matching_range() {
        assert!(debian_requirement_matches("qt", "6.10.2+dfsg-3", ">=6.5,<7.0").unwrap());
    }

    #[test]
    fn debian_requirement_rejects_upper_bound() {
        assert!(!debian_requirement_matches("qt", "7.0.0-1", ">=6.5,<7.0").unwrap());
    }

    #[test]
    fn debian_requirement_handles_epoch() {
        assert!(debian_requirement_matches("git", "1:2.53.0-1ubuntu1", ">=1:2.40.0").unwrap());
    }

    #[test]
    fn debian_requirement_rejects_invalid_clause() {
        assert!(debian_requirement_matches("qt", "6.10.2+dfsg-3", "6.5").is_err());
    }
}

#[cfg(test)]
mod resolver_tests {
    use super::*;
    use crate::local_installer::LocalInstallerResult;
    use std::cell::RefCell;

    fn result(installer: &str, operation: &str, success: bool) -> LocalInstallerResult {
        LocalInstallerResult {
            installer: installer.to_string(),
            operation: operation.to_string(),
            command: "fake".to_string(),
            args: Vec::new(),
            requires_root: false,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: if success { 0 } else { 1 },
            success,
        }
    }

    #[test]
    fn resolver_repairs_then_reverifies() {
        let dependency = SystemDependency {
            name: "fake-package".to_string(),
            install: LocalInstallerRequest {
                installer: "fake".to_string(),
                operation: "install".to_string(),
                variables: Default::default(),
            },
            verify: LocalInstallerRequest {
                installer: "fake".to_string(),
                operation: "verify".to_string(),
                variables: Default::default(),
            },
            version: None,
            required: true,
        };

        let calls = RefCell::new(Vec::<String>::new());
        let verify_count = RefCell::new(0usize);

        let execute = |request: LocalInstallerRequest| {
            calls.borrow_mut().push(request.operation.clone());

            match request.operation.as_str() {
                "verify" => {
                    let mut count = verify_count.borrow_mut();
                    *count += 1;

                    Ok(result(&request.installer, &request.operation, *count >= 2))
                }
                "install" => Ok(result(&request.installer, &request.operation, true)),
                other => Err(format!("unexpected fake operation: {other}")),
            }
        };

        resolve_system_dependencies_with(&[dependency], &execute).unwrap();

        assert_eq!(
            calls.into_inner(),
            vec![
                "verify".to_string(),
                "install".to_string(),
                "verify".to_string(),
            ]
        );
    }
}

#[cfg(test)]
mod resolver_policy_tests {
    use super::*;
    use crate::local_installer::LocalInstallerResult;
    use std::cell::RefCell;

    fn result(operation: &str, success: bool) -> LocalInstallerResult {
        LocalInstallerResult {
            installer: "fake".to_string(),
            operation: operation.to_string(),
            command: "fake".to_string(),
            args: Vec::new(),
            requires_root: false,
            stdout: String::new(),
            stderr: if success {
                String::new()
            } else {
                "fake failure".to_string()
            },
            exit_code: if success { 0 } else { 1 },
            success,
        }
    }

    fn dependency(required: bool) -> SystemDependency {
        SystemDependency {
            name: "fake-package".to_string(),
            install: LocalInstallerRequest {
                installer: "fake".to_string(),
                operation: "install".to_string(),
                variables: Default::default(),
            },
            verify: LocalInstallerRequest {
                installer: "fake".to_string(),
                operation: "verify".to_string(),
                variables: Default::default(),
            },
            version: None,
            required,
        }
    }

    #[test]
    fn healthy_dependency_does_not_install() {
        let calls = RefCell::new(Vec::<String>::new());

        let execute = |request: LocalInstallerRequest| {
            calls.borrow_mut().push(request.operation.clone());

            match request.operation.as_str() {
                "verify" => Ok(result("verify", true)),
                "install" => panic!("install must not run for healthy dependency"),
                other => Err(format!("unexpected operation: {other}")),
            }
        };

        resolve_system_dependencies_with(&[dependency(true)], &execute).unwrap();

        assert_eq!(calls.into_inner(), vec!["verify".to_string()]);
    }

    #[test]
    fn required_dependency_blocks_when_install_fails() {
        let execute = |request: LocalInstallerRequest| match request.operation.as_str() {
            "verify" => Ok(result("verify", false)),
            "install" => Ok(result("install", false)),
            other => Err(format!("unexpected operation: {other}")),
        };

        let error = resolve_system_dependencies_with(&[dependency(true)], &execute).unwrap_err();

        assert!(error.contains("installation failed"));
    }

    #[test]
    fn required_dependency_blocks_when_repair_does_not_satisfy_contract() {
        let calls = RefCell::new(0usize);

        let execute = |request: LocalInstallerRequest| match request.operation.as_str() {
            "verify" => {
                *calls.borrow_mut() += 1;
                Ok(result("verify", false))
            }
            "install" => Ok(result("install", true)),
            other => Err(format!("unexpected operation: {other}")),
        };

        let error = resolve_system_dependencies_with(&[dependency(true)], &execute).unwrap_err();

        assert!(error.contains("still not satisfied"));
        assert_eq!(*calls.borrow(), 2);
    }

    #[test]
    fn optional_dependency_does_not_block_system() {
        let execute = |request: LocalInstallerRequest| match request.operation.as_str() {
            "verify" => Ok(result("verify", false)),
            "install" => Ok(result("install", false)),
            other => Err(format!("unexpected operation: {other}")),
        };

        resolve_system_dependencies_with(&[dependency(false)], &execute).unwrap();
    }
}

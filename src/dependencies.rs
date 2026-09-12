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
    pub provider: String,
    pub package: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub check: Option<String>,
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

pub fn resolve_system_dependencies(dependencies: &[SystemDependency]) -> Result<(), String> {
    for dependency in dependencies {
        match resolve_one(dependency) {
            Ok(()) => {}
            Err(error) if dependency.required => return Err(error),
            Err(error) => eprintln!(
                "N.E.E.B.L.E.S.: optional dependency '{}' could not be resolved: {error}",
                dependency.name
            ),
        }
    }
    Ok(())
}

fn resolve_one(dependency: &SystemDependency) -> Result<(), String> {
    if dependency_satisfied(dependency)? {
        return Ok(());
    }

    ensure_provider(&dependency.provider)?;
    install_dependency(dependency)?;

    if dependency_satisfied(dependency)? {
        Ok(())
    } else {
        Err(format!(
            "dependency '{}' was installed but verification still failed",
            dependency.name
        ))
    }
}

fn dependency_satisfied(dependency: &SystemDependency) -> Result<bool, String> {
    let provider_ok = match dependency.provider.as_str() {
        "apt" => command_success(
            "dpkg-query",
            &["-W", "-f=${Status}", dependency.package.as_str()],
        ),
        "snap" => {
            command_exists("snap") && command_success("snap", &["list", dependency.package.as_str()])
        }
        "flatpak" => {
            command_exists("flatpak")
                && command_success("flatpak", &["info", dependency.package.as_str()])
        }
        other => return Err(format!("unsupported dependency provider: {other}")),
    };

    if !provider_ok {
        return Ok(false);
    }

    if let Some(check) = &dependency.check {
        return Ok(command_exists(check));
    }

    Ok(true)
}

fn ensure_provider(provider: &str) -> Result<(), String> {
    match provider {
        "apt" => {
            if command_exists("apt") {
                Ok(())
            } else {
                Err("apt is not available on this N.E.E.B.L.E.S./Debian environment".to_string())
            }
        }
        "snap" => {
            if command_exists("snap") {
                return Ok(());
            }
            install_with_apt("snapd", &[])?;
            if command_exists("snap") {
                Ok(())
            } else {
                Err("snapd installation completed but snap is still unavailable".to_string())
            }
        }
        "flatpak" => {
            if command_exists("flatpak") {
                return Ok(());
            }
            install_with_apt("flatpak", &[])?;
            if command_exists("flatpak") {
                Ok(())
            } else {
                Err("flatpak installation completed but flatpak is still unavailable".to_string())
            }
        }
        other => Err(format!("unsupported dependency provider: {other}")),
    }
}

fn install_dependency(dependency: &SystemDependency) -> Result<(), String> {
    match dependency.provider.as_str() {
        "apt" => install_with_apt(&dependency.package, &dependency.args),
        "snap" => run_install("snap", "install", &dependency.package, &dependency.args),
        "flatpak" => run_install("flatpak", "install", &dependency.package, &prepend_yes(&dependency.args)),
        other => Err(format!("unsupported dependency provider: {other}")),
    }
}

fn install_with_apt(package: &str, args: &[String]) -> Result<(), String> {
    let mut final_args = vec!["install".to_string(), "-y".to_string(), package.to_string()];
    final_args.extend(args.iter().cloned());
    run_command("apt", &final_args)
}

fn prepend_yes(args: &[String]) -> Vec<String> {
    let mut result = vec!["-y".to_string()];
    result.extend(args.iter().cloned());
    result
}

fn run_install(command: &str, verb: &str, package: &str, args: &[String]) -> Result<(), String> {
    let mut final_args = vec![verb.to_string(), package.to_string()];
    final_args.extend(args.iter().cloned());
    run_command(command, &final_args)
}

fn run_command(command: &str, args: &[String]) -> Result<(), String> {
    let status = Command::new(command)
        .args(args)
        .status()
        .map_err(|error| format!("could not start {command}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{command} exited with status {status}"))
    }
}

fn command_success(command: &str, args: &[&str]) -> bool {
    Command::new(command)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn command_exists(command: &str) -> bool {
    if command.contains('/') {
        return Path::new(command).exists();
    }

    env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).any(|path| path.join(command).exists()))
        .unwrap_or(false)
}

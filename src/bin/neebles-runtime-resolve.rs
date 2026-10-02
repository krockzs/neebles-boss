use neebles_backend::domestic_execution_authority::build_materialized_external_session_execution_command;

use neebles_backend::domestic_runtime_authority::resolve_materialized_runtime_target;

use std::ffi::OsString;
use std::path::PathBuf;

fn take_argument(arguments: &[String], name: &str) -> Result<String, String> {
    let positions = arguments
        .iter()
        .enumerate()
        .filter_map(
            |(index, value)| {
                if value == name {
                    Some(index)
                } else {
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    if positions.len() != 1 {
        return Err(format!("{name} must be supplied exactly once"));
    }

    arguments
        .get(positions[0] + 1)
        .cloned()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn run_resolve(arguments: &[String]) -> Result<i32, String> {
    if arguments.len() != 6 {
        return Err(
            "usage: neebles-runtime-resolve --manifest PATH --world WORLD --category CATEGORY"
                .to_string(),
        );
    }

    let manifest = PathBuf::from(take_argument(arguments, "--manifest")?);

    let world = take_argument(arguments, "--world")?;

    let category = take_argument(arguments, "--category")?;

    if !manifest.is_absolute() {
        return Err("runtime manifest path must be absolute".to_string());
    }

    if world.trim().is_empty() {
        return Err("runtime world is empty".to_string());
    }

    if category.trim().is_empty() {
        return Err("runtime category is empty".to_string());
    }

    let target = resolve_materialized_runtime_target(&manifest, &world, &category)?;

    println!("{}", target.display());

    Ok(0)
}

fn external_argument_separator(arguments: &[String]) -> Result<usize, String> {
    arguments
        .iter()
        .position(|value| value == "--")
        .ok_or_else(|| "external execution requires an argument separator".to_string())
}

fn run_external(arguments: &[String]) -> Result<i32, String> {
    let separator = external_argument_separator(arguments)?;

    let control = &arguments[..separator];

    if control.len() != 4 {
        return Err(
            "usage: neebles-runtime-resolve --manifest PATH --execute-external PATH -- [ARGS...]"
                .to_string(),
        );
    }

    let manifest = PathBuf::from(take_argument(control, "--manifest")?);

    let executable = PathBuf::from(take_argument(control, "--execute-external")?);

    if !manifest.is_absolute() {
        return Err("runtime manifest path must be absolute".to_string());
    }

    if !executable.is_absolute() {
        return Err("external execution target path must be absolute".to_string());
    }

    let executable_arguments = arguments[separator + 1..]
        .iter()
        .map(OsString::from)
        .collect::<Vec<_>>();

    let mut command = build_materialized_external_session_execution_command(
        &manifest,
        &executable,
        &executable_arguments,
    )?;

    let status = command
        .status()
        .map_err(|error| format!("external domestic execution could not start: {error}"))?;

    status
        .code()
        .ok_or_else(|| "external domestic execution terminated without an exit code".to_string())
}

fn run() -> Result<i32, String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();

    if arguments.iter().any(|value| value == "--execute-external") {
        return run_external(&arguments);
    }

    run_resolve(&arguments)
}

#[cfg(test)]
mod tests {
    use super::external_argument_separator;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn external_separator_is_required() {
        let arguments = args(&[
            "--manifest",
            "/tmp/runtime-authority/domestic-runtime.json",
            "--execute-external",
            "/tmp/neebles-auth-agent",
        ]);

        assert!(external_argument_separator(&arguments).is_err());
    }

    #[test]
    fn external_separator_marks_control_boundary() {
        let arguments = args(&[
            "--manifest",
            "/tmp/runtime-authority/domestic-runtime.json",
            "--execute-external",
            "/tmp/neebles-auth-agent",
            "--",
            "--locale",
            "es_CL",
        ]);

        assert_eq!(external_argument_separator(&arguments).unwrap(), 4);
    }

    #[test]
    fn nested_child_separator_is_not_rejected() {
        let arguments = args(&[
            "--manifest",
            "/tmp/runtime-authority/domestic-runtime.json",
            "--execute-external",
            "/tmp/neebles-auth-agent",
            "--",
            "--runtime-resolver",
            "/tmp/neebles-runtime-resolve",
            "--runtime-manifest",
            "/tmp/runtime-authority/domestic-runtime.json",
            "--",
            "/tmp/install.sh",
        ]);

        assert_eq!(external_argument_separator(&arguments).unwrap(), 4);
        assert_eq!(arguments[9], "--");
    }
}

fn main() {
    match run() {
        Ok(code) => {
            std::process::exit(code);
        }

        Err(error) => {
            eprintln!("FATAL :: {error}");

            std::process::exit(1);
        }
    }
}

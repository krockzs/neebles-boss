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

fn run_external(arguments: &[String]) -> Result<i32, String> {
    let separators = arguments
        .iter()
        .enumerate()
        .filter_map(
            |(index, value)| {
                if value == "--" {
                    Some(index)
                } else {
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    if separators.len() != 1 {
        return Err("external execution requires exactly one argument separator".to_string());
    }

    let separator = separators[0];

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

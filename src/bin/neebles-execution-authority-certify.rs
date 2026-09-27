use neebles_backend::domestic_execution_authority::resolve_materialized_execution_authority;

use std::path::PathBuf;

fn argument(arguments: &[String], name: &str) -> Result<String, String> {
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

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();

    if arguments.len() != 4 {
        return Err(
            "usage: neebles-execution-authority-certify --manifest PATH --world WORLD".to_string(),
        );
    }

    let manifest = PathBuf::from(argument(&arguments, "--manifest")?);

    let world = argument(&arguments, "--world")?;

    let authority = resolve_materialized_execution_authority(&manifest, &world)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "1",
            "name": "neebles-execution-authority-certification",
            "world": world,
            "executable": authority.executable,
            "interpreter": authority.interpreter,
            "library_paths": authority.library_paths
        }))
        .map_err(|error| {
            format!("could not serialize execution authority certification: {error}")
        })?
    );

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("FATAL :: {error}");

        std::process::exit(1);
    }
}

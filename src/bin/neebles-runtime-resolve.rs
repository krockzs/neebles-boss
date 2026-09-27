use neebles_backend::domestic_runtime_authority::resolve_materialized_runtime_target;

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

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();

    if arguments.len() != 6 {
        return Err(
            "usage: neebles-runtime-resolve --manifest PATH --world WORLD --category CATEGORY"
                .to_string(),
        );
    }

    let manifest = PathBuf::from(take_argument(&arguments, "--manifest")?);

    let world = take_argument(&arguments, "--world")?;

    let category = take_argument(&arguments, "--category")?;

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

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("FATAL :: {error}");

        std::process::exit(1);
    }
}

use neebles_backend::domestic_runtime_authority::resolve_materialized_runtime_targets;

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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

fn read_worlds(manifest_path: &Path) -> Result<Vec<String>, String> {
    let raw = fs::read_to_string(manifest_path).map_err(|error| {
        format!(
            "could not read runtime manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    let manifest: Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "invalid runtime manifest {}: {error}",
            manifest_path.display()
        )
    })?;

    let worlds = manifest
        .get("worlds")
        .and_then(Value::as_object)
        .ok_or_else(|| "runtime manifest worlds is not an object".to_string())?;

    let mut names = worlds.keys().cloned().collect::<Vec<_>>();

    names.sort();

    if names.is_empty() {
        return Err("runtime manifest declares no worlds".to_string());
    }

    Ok(names)
}

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();

    if arguments.len() != 4 {
        return Err(
            "usage: neebles-runtime-authority-plural-certify --manifest PATH --category CATEGORY"
                .to_string(),
        );
    }

    let manifest = PathBuf::from(argument(&arguments, "--manifest")?);

    let category = argument(&arguments, "--category")?;

    if !manifest.is_absolute() {
        return Err("runtime certifier manifest path must be absolute".to_string());
    }

    if category.trim().is_empty() {
        return Err("runtime certifier category is empty".to_string());
    }

    let worlds = read_worlds(&manifest)?;

    let mut resolved = BTreeMap::<String, Vec<String>>::new();

    for world in worlds {
        let targets = resolve_materialized_runtime_targets(&manifest, &world, &category)?;

        resolved.insert(
            world,
            targets
                .into_iter()
                .map(|target| target.to_string_lossy().to_string())
                .collect(),
        );
    }

    let output = serde_json::json!({
        "schema": "1",
        "name": "neebles-runtime-authority-plural-certification",
        "category": category,
        "count": resolved.len(),
        "resolved": resolved
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|error| {
            format!("could not serialize plural runtime certification: {error}")
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

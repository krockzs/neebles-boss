use neebles_backend::domestic_resolver::{resolve_world, ResolvedDomesticCategory};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct MaterializedCategory {
    category: String,
    value: String,
    declared_targets: Vec<String>,
    resolved_targets: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MaterializedWorld {
    categories: BTreeMap<String, MaterializedCategory>,
}

#[derive(Debug, Serialize)]
struct MaterializedRuntime {
    schema: &'static str,
    name: &'static str,
    root: &'static str,
    worlds: BTreeMap<String, MaterializedWorld>,
}

fn argument(args: &[String], name: &str) -> Result<PathBuf, String> {
    let index = args
        .iter()
        .position(|value| value == name)
        .ok_or_else(|| format!("missing argument {name}"))?;

    let raw = args
        .get(index + 1)
        .ok_or_else(|| format!("missing value for {name}"))?;

    Ok(PathBuf::from(raw))
}

fn relative_target(canonical_root: &Path, target: &Path) -> Result<String, String> {
    let canonical_target = target.canonicalize().map_err(|error| {
        format!(
            "could not canonicalize resolved domestic target {}: {error}",
            target.display()
        )
    })?;

    let relative = canonical_target.strip_prefix(canonical_root).map_err(|_| {
        format!(
            "resolved domestic target escaped materialized root: {}",
            canonical_target.display()
        )
    })?;

    if relative.as_os_str().is_empty() {
        return Err("resolved domestic target cannot be the world root itself".to_string());
    }

    if relative.is_absolute() {
        return Err(format!(
            "materialized domestic target remained absolute: {}",
            relative.display()
        ));
    }

    Ok(relative.to_string_lossy().into_owned())
}

fn materialize_category(
    canonical_root: &Path,
    category: ResolvedDomesticCategory,
) -> Result<MaterializedCategory, String> {
    let mut resolved_targets = Vec::<String>::new();

    for target in category.resolved_targets {
        resolved_targets.push(relative_target(canonical_root, &target)?);
    }

    Ok(MaterializedCategory {
        category: category.category,
        value: category.value,
        declared_targets: category.declared_targets,
        resolved_targets,
    })
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();

    let contract = argument(&args, "--contract")?;

    let grimoire = argument(&args, "--grimoire")?;

    let targets = argument(&args, "--targets")?;

    let root = argument(&args, "--root")?;

    let output = argument(&args, "--output")?;

    if !root.is_absolute() {
        return Err(format!(
            "domestic materialization root must be absolute: {}",
            root.display()
        ));
    }

    let canonical_root = root.canonicalize().map_err(|error| {
        format!(
            "could not canonicalize domestic root {}: {error}",
            root.display()
        )
    })?;

    if !canonical_root.is_dir() {
        return Err(format!(
            "domestic materialization root is not a directory: {}",
            canonical_root.display()
        ));
    }

    let raw = fs::read_to_string(&grimoire).map_err(|error| {
        format!(
            "could not read domestic grimoire {}: {error}",
            grimoire.display()
        )
    })?;

    let value: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("invalid domestic grimoire {}: {error}", grimoire.display()))?;

    let declared_worlds = value
        .get("worlds")
        .and_then(Value::as_object)
        .ok_or_else(|| "domestic grimoire worlds must be an object".to_string())?;

    let mut names = declared_worlds.keys().cloned().collect::<Vec<_>>();

    names.sort();

    let mut materialized_worlds = BTreeMap::<String, MaterializedWorld>::new();

    for world_name in names {
        let resolved = resolve_world(&contract, &grimoire, &targets, &canonical_root, &world_name)?;

        let mut categories = BTreeMap::<String, MaterializedCategory>::new();

        for (category_name, category) in resolved.categories {
            categories.insert(
                category_name,
                materialize_category(&canonical_root, category)?,
            );
        }

        materialized_worlds.insert(world_name, MaterializedWorld { categories });
    }

    let manifest = MaterializedRuntime {
        schema: "1",
        name: "neebles-domestic-runtime",
        root: "rootfs",
        worlds: materialized_worlds,
    };

    let parent = output
        .parent()
        .ok_or_else(|| format!("output has no parent directory: {}", output.display()))?;

    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "could not create output directory {}: {error}",
            parent.display()
        )
    })?;

    fs::write(
        &output,
        serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("could not serialize domestic runtime manifest: {error}"))?,
    )
    .map_err(|error| {
        format!(
            "could not write domestic runtime manifest {}: {error}",
            output.display()
        )
    })?;

    println!("MATERIALIZED WORLDS :: {}", manifest.worlds.len());

    println!("RUNTIME ROOT :: {}", manifest.root);

    println!("OUTPUT :: {}", output.display());

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("N.E.E.B.L.E.S. domestic runtime materialization failed: {error}");

        std::process::exit(1);
    }
}

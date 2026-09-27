use neebles_backend::domestic_world::{
    inspect_world_symlink, transport_world_reference, WorldSymlinkInspection,
};

use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::symlink;

fn collect_symlinks(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory).map_err(|error| {
        format!(
            "could not read transport directory {}: {error}",
            directory.display()
        )
    })?;

    let mut paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| format!("could not read transport directory entry: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    paths.sort();

    for path in paths {
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "could not inspect transport path {}: {error}",
                path.display()
            )
        })?;

        if metadata.file_type().is_symlink() {
            output.push(path);

            continue;
        }

        if metadata.is_dir() {
            collect_symlinks(&path, output)?;
        }
    }

    Ok(())
}

fn parse_staged_root() -> Result<PathBuf, String> {
    let mut arguments = std::env::args().skip(1);

    let mut root = None;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--staged-root" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--staged-root requires a value".to_string())?;

                if root.is_some() {
                    return Err("--staged-root may only be provided once".to_string());
                }

                root = Some(PathBuf::from(value));
            }

            other => {
                return Err(format!("unknown transport argument: {other}"));
            }
        }
    }

    let root = root.ok_or_else(|| "--staged-root is required".to_string())?;

    if !root.is_absolute() {
        return Err("staged transport root must be absolute".to_string());
    }

    if !root.is_dir() {
        return Err(format!(
            "staged transport root is not a directory: {}",
            root.display()
        ));
    }

    root.canonicalize().map_err(|error| {
        format!(
            "could not canonicalize staged transport root {}: {error}",
            root.display()
        )
    })
}

fn run() -> Result<(), String> {
    let root = parse_staged_root()?;

    let mut symlinks = Vec::<PathBuf>::new();

    collect_symlinks(&root, &mut symlinks)?;

    symlinks.sort();

    let mut rewritten = 0usize;

    let mut preserved = 0usize;

    let mut inside = 0usize;

    let mut broken = 0usize;

    for path in &symlinks {
        match inspect_world_symlink(&root, path)? {
            WorldSymlinkInspection::Inside(_) => {
                inside += 1;
            }

            WorldSymlinkInspection::Broken(_) => {
                broken += 1;
            }

            WorldSymlinkInspection::Outside(target) => {
                return Err(format!(
                    "transport symlink escapes domestic world: {} -> {}",
                    path.display(),
                    target.display()
                ));
            }
        }

        let raw = fs::read_link(path).map_err(|error| {
            format!(
                "could not read transport symlink {}: {error}",
                path.display()
            )
        })?;

        let parent = path
            .parent()
            .ok_or_else(|| format!("transport symlink has no parent: {}", path.display()))?;

        let transported = transport_world_reference(&root, parent, &raw)?;

        if transported.is_absolute() {
            return Err(format!(
                "transport authority produced absolute reference for {}: {}",
                path.display(),
                transported.display()
            ));
        }

        if transported == raw {
            preserved += 1;

            continue;
        }

        fs::remove_file(path).map_err(|error| {
            format!(
                "could not remove staged symlink {}: {error}",
                path.display()
            )
        })?;

        symlink(&transported, path).map_err(|error| {
            format!(
                "could not rewrite staged symlink {} -> {}: {error}",
                path.display(),
                transported.display()
            )
        })?;

        rewritten += 1;
    }

    println!("SYMLINKS :: {}", symlinks.len());

    println!("INSIDE :: {inside}");

    println!("BROKEN BUT DOMESTIC :: {broken}");

    println!("REWRITTEN :: {rewritten}");

    println!("PRESERVED :: {preserved}");

    println!("ABSOLUTE AFTER TRANSPORT :: 0");

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("FATAL :: {error}");

        std::process::exit(1);
    }
}

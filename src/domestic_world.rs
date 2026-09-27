use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

fn canonical_world_root(world_root: &Path) -> Result<PathBuf, String> {
    world_root.canonicalize().map_err(|error| {
        format!(
            "could not canonicalize domestic world root {}: {error}",
            world_root.display()
        )
    })
}

fn canonical_inside_world(
    canonical_world: &Path,
    path: &Path,
    label: &str,
) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("could not canonicalize {label} {}: {error}", path.display()))?;

    if !canonical.starts_with(canonical_world) {
        return Err(format!(
            "{label} escapes domestic world: {}",
            canonical.display()
        ));
    }

    Ok(canonical)
}

fn normalize_world_path(path: &Path) -> Result<PathBuf, String> {
    use std::path::Component;

    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::RootDir => {
                normalized.push("/");
            }

            Component::CurDir => {}

            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!("path escapes filesystem root: {}", path.display()));
                }
            }

            Component::Normal(part) => {
                normalized.push(part);
            }

            Component::Prefix(_) => {
                return Err(format!("unsupported path prefix: {}", path.display()));
            }
        }
    }

    Ok(normalized)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldSymlinkInspection {
    Inside(PathBuf),
    Outside(PathBuf),
    Broken(PathBuf),
}

pub fn inspect_world_symlink(
    world_root: &Path,
    link: &Path,
) -> Result<WorldSymlinkInspection, String> {
    let canonical_world = canonical_world_root(world_root)?;

    let link_parent = link
        .parent()
        .ok_or_else(|| format!("symlink has no parent: {}", link.display()))?;

    let canonical_parent = canonical_inside_world(&canonical_world, link_parent, "symlink parent")?;

    let link_name = link
        .file_name()
        .ok_or_else(|| format!("symlink has no file name: {}", link.display()))?;

    let mut current = canonical_parent.join(link_name);

    let mut visited = std::collections::BTreeSet::new();

    for _ in 0..64 {
        current = normalize_world_path(&current)?;

        if !current.starts_with(&canonical_world) {
            return Ok(WorldSymlinkInspection::Outside(current));
        }

        if !visited.insert(current.clone()) {
            return Err(format!("symlink cycle detected: {}", current.display()));
        }

        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,

            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(WorldSymlinkInspection::Broken(current));
            }

            Err(error) => {
                return Err(format!(
                    "could not inspect world symlink target {}: {error}",
                    current.display()
                ));
            }
        };

        if !metadata.file_type().is_symlink() {
            let target = canonical_inside_world(&canonical_world, &current, "symlink target")?;

            return Ok(WorldSymlinkInspection::Inside(target));
        }

        let target = std::fs::read_link(&current).map_err(|error| {
            format!(
                "could not read world symlink {}: {error}",
                current.display()
            )
        })?;

        current = if target.is_absolute() {
            let relative = target.strip_prefix("/").map_err(|error| {
                format!(
                    "could not root absolute symlink target {}: {error}",
                    target.display()
                )
            })?;

            canonical_world.join(relative)
        } else {
            current.parent().unwrap_or(&canonical_world).join(target)
        };
    }

    Err(format!(
        "symlink resolution depth exceeded: {}",
        link.display()
    ))
}

pub fn resolve_world_symlink(world_root: &Path, link: &Path) -> Result<PathBuf, String> {
    match inspect_world_symlink(world_root, link)? {
        WorldSymlinkInspection::Inside(target) => Ok(target),

        WorldSymlinkInspection::Outside(target) => Err(format!(
            "symlink escapes declared world: {}",
            target.display()
        )),

        WorldSymlinkInspection::Broken(target) => {
            Err(format!("broken world symlink target: {}", target.display()))
        }
    }
}

pub fn transport_world_reference(
    world_root: &Path,
    base_dir: &Path,
    reference: &Path,
) -> Result<PathBuf, String> {
    let canonical_world = canonical_world_root(world_root)?;

    let canonical_base =
        canonical_inside_world(&canonical_world, base_dir, "transport reference base")?;

    let candidate = if reference.is_absolute() {
        let relative = reference.strip_prefix("/").map_err(|error| {
            format!(
                "could not normalize absolute transport reference {}: {error}",
                reference.display()
            )
        })?;

        canonical_world.join(relative)
    } else {
        canonical_base.join(reference)
    };

    let normalized = normalize_world_path(&candidate)?;

    if !normalized.starts_with(&canonical_world) {
        return Err(format!(
            "transport reference escapes domestic world: {} -> {}",
            reference.display(),
            normalized.display()
        ));
    }

    if reference.is_absolute() {
        relative_path_between(&canonical_base, &normalized)
    } else {
        Ok(reference.to_path_buf())
    }
}

pub fn resolve_world_reference(
    world_root: &Path,
    base_dir: &Path,
    reference: &Path,
) -> Result<PathBuf, String> {
    let canonical_world = canonical_world_root(world_root)?;

    let canonical_base = canonical_inside_world(&canonical_world, base_dir, "reference base")?;

    let candidate = if reference.is_absolute() {
        let relative = reference.strip_prefix("/").map_err(|error| {
            format!(
                "could not normalize absolute world reference {}: {error}",
                reference.display()
            )
        })?;

        canonical_world.join(relative)
    } else {
        canonical_base.join(reference)
    };

    canonical_inside_world(&canonical_world, &candidate, "world reference target")
}

fn relative_path_between(from: &Path, to: &Path) -> Result<PathBuf, String> {
    let from_components = from.components().collect::<Vec<_>>();

    let to_components = to.components().collect::<Vec<_>>();

    let mut common = 0usize;

    while common < from_components.len()
        && common < to_components.len()
        && from_components[common] == to_components[common]
    {
        common += 1;
    }

    let mut relative = PathBuf::new();

    for component in &from_components[common..] {
        match component {
            Component::Normal(_) => {
                relative.push("..");
            }

            Component::CurDir => {}

            Component::ParentDir => {
                return Err("origin contains unresolved parent traversal".to_string());
            }

            Component::RootDir | Component::Prefix(_) => {
                return Err("origin diverges across filesystem roots".to_string());
            }
        }
    }

    for component in &to_components[common..] {
        match component {
            Component::Normal(value) => {
                relative.push(value);
            }

            Component::CurDir => {}

            Component::ParentDir => {
                return Err("target contains unresolved parent traversal".to_string());
            }

            Component::RootDir | Component::Prefix(_) => {
                return Err("target diverges across filesystem roots".to_string());
            }
        }
    }

    if relative.as_os_str().is_empty() {
        relative.push(".");
    }

    Ok(relative)
}

pub fn certify_world_path(world_root: &Path, path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical_world = canonical_world_root(world_root)?;

    canonical_inside_world(&canonical_world, path, label)
}

pub fn relative_world_relation(
    world_root: &Path,
    from: &Path,
    to: &Path,
) -> Result<PathBuf, String> {
    let canonical_world = canonical_world_root(world_root)?;

    let canonical_from = canonical_inside_world(&canonical_world, from, "relative origin")?;

    let canonical_to = canonical_inside_world(&canonical_world, to, "relative target")?;

    relative_path_between(&canonical_from, &canonical_to)
}

pub fn certify_stateful_recursive_closure<S, F>(
    root: S,
    transitions_for: F,
) -> Result<BTreeSet<S>, String>
where
    S: Clone + Ord,
    F: Fn(&S) -> Result<Vec<S>, String>,
{
    let mut pending = vec![root];

    let mut visited = BTreeSet::<S>::new();

    while let Some(current) = pending.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }

        for target in transitions_for(&current)? {
            if !visited.contains(&target) {
                pending.push(target);
            }
        }
    }

    Ok(visited)
}

pub fn certify_recursive_closure<F>(
    world_root: &Path,
    root: &Path,
    references_for: F,
) -> Result<BTreeSet<PathBuf>, String>
where
    F: Fn(&Path) -> Result<Vec<PathBuf>, String>,
{
    let canonical_world = canonical_world_root(world_root)?;

    let canonical_root = canonical_inside_world(&canonical_world, root, "closure root")?;

    let mut pending = vec![canonical_root];

    let mut visited = BTreeSet::<PathBuf>::new();

    while let Some(current) = pending.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }

        let base = current
            .parent()
            .ok_or_else(|| format!("closure node has no parent: {}", current.display()))?;

        let references = references_for(&current)?;

        for reference in references {
            let target = resolve_world_reference(&canonical_world, base, &reference)?;

            if !visited.contains(&target) {
                pending.push(target);
            }
        }
    }

    Ok(visited)
}

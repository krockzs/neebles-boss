use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;

use crate::module_material::{MaterialEntry, MaterialRecipe};

#[derive(Debug, Clone, PartialEq, Eq)]
struct DebDataMember {
    name: String,
    payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingSymlink {
    path: PathBuf,
    target: PathBuf,
}

fn safe_archive_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir
                    | Component::CurDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
}

fn parse_ar_decimal(field: &[u8], label: &str) -> Result<usize, String> {
    let text = std::str::from_utf8(field)
        .map_err(|error| format!("invalid ar {label} field: {error}"))?
        .trim();

    text.parse::<usize>()
        .map_err(|error| format!("invalid ar {label} value '{text}': {error}"))
}

fn deb_data_member(payload: &[u8]) -> Result<DebDataMember, String> {
    const MAGIC: &[u8] = b"!<arch>\n";

    if !payload.starts_with(MAGIC) {
        return Err("invalid Debian package ar magic".to_string());
    }

    let mut cursor = MAGIC.len();

    while cursor < payload.len() {
        if payload.len() - cursor < 60 {
            return Err("truncated Debian package ar header".to_string());
        }

        let header = &payload[cursor..cursor + 60];

        if &header[58..60] != b"`\n" {
            return Err("invalid Debian package ar header trailer".to_string());
        }

        let raw_name = std::str::from_utf8(&header[0..16])
            .map_err(|error| format!("invalid ar member name: {error}"))?
            .trim();

        let name = raw_name.strip_suffix("/").unwrap_or(raw_name).to_string();

        let size = parse_ar_decimal(&header[48..58], "size")?;

        cursor += 60;

        let end = cursor
            .checked_add(size)
            .ok_or_else(|| "Debian package ar member size overflow".to_string())?;

        if end > payload.len() {
            return Err(format!("truncated Debian package ar member: {name}"));
        }

        if matches!(
            name.as_str(),
            "data.tar" | "data.tar.gz" | "data.tar.xz" | "data.tar.zst"
        ) {
            return Ok(DebDataMember {
                name,
                payload: payload[cursor..end].to_vec(),
            });
        }

        cursor = end;

        if cursor % 2 != 0 {
            cursor += 1;
        }
    }

    Err("Debian package does not contain supported data.tar payload".to_string())
}

fn create_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("material path has no parent: {}", path.display()))?;

    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "could not create material parent {}: {error}",
            parent.display()
        )
    })
}

fn unpack_tar<R: Read>(reader: R, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| {
        format!(
            "could not create material staging root {}: {error}",
            destination.display()
        )
    })?;

    let mut archive = Archive::new(reader);
    let mut symlinks = Vec::<PendingSymlink>::new();

    let entries = archive
        .entries()
        .map_err(|error| format!("could not read material tar entries: {error}"))?;

    for item in entries {
        let mut entry = item.map_err(|error| format!("invalid material tar entry: {error}"))?;

        let path = entry
            .path()
            .map_err(|error| format!("invalid material tar path: {error}"))?
            .into_owned();

        if !safe_archive_path(&path) {
            return Err(format!("unsafe material tar path: {}", path.display()));
        }

        let target = destination.join(&path);
        let kind = entry.header().entry_type();

        if kind.is_dir() {
            fs::create_dir_all(&target).map_err(|error| {
                format!(
                    "could not create material directory {}: {error}",
                    target.display()
                )
            })?;

            if let Ok(mode) = entry.header().mode() {
                fs::set_permissions(&target, fs::Permissions::from_mode(mode)).map_err(
                    |error| {
                        format!(
                            "could not apply material directory mode {}: {error}",
                            target.display()
                        )
                    },
                )?;
            }

            continue;
        }

        if kind.is_file() {
            create_parent(&target)?;

            if fs::symlink_metadata(&target).is_ok() {
                return Err(format!("duplicate material tar path: {}", path.display()));
            }

            let mut output = fs::File::create(&target).map_err(|error| {
                format!(
                    "could not create material file {}: {error}",
                    target.display()
                )
            })?;

            std::io::copy(&mut entry, &mut output).map_err(|error| {
                format!(
                    "could not write material file {}: {error}",
                    target.display()
                )
            })?;

            let mode = entry.header().mode().map_err(|error| {
                format!(
                    "could not read material file mode {}: {error}",
                    path.display()
                )
            })?;

            fs::set_permissions(&target, fs::Permissions::from_mode(mode)).map_err(|error| {
                format!(
                    "could not apply material file mode {}: {error}",
                    target.display()
                )
            })?;

            continue;
        }

        if kind.is_symlink() {
            let link = entry
                .link_name()
                .map_err(|error| {
                    format!(
                        "could not read material symlink target {}: {error}",
                        path.display()
                    )
                })?
                .ok_or_else(|| format!("material symlink target missing: {}", path.display()))?
                .into_owned();

            symlinks.push(PendingSymlink {
                path: target,
                target: link,
            });

            continue;
        }

        return Err(format!(
            "unsupported material tar entry type for {}",
            path.display()
        ));
    }

    for pending in symlinks {
        create_parent(&pending.path)?;

        if fs::symlink_metadata(&pending.path).is_ok() {
            return Err(format!(
                "duplicate material symlink path: {}",
                pending.path.display()
            ));
        }

        symlink(&pending.target, &pending.path).map_err(|error| {
            format!(
                "could not create material symlink {} -> {}: {error}",
                pending.path.display(),
                pending.target.display()
            )
        })?;
    }

    Ok(())
}

fn unpack_data_member(member: &DebDataMember, destination: &Path) -> Result<(), String> {
    match member.name.as_str() {
        "data.tar" => unpack_tar(Cursor::new(&member.payload), destination),

        "data.tar.gz" => unpack_tar(GzDecoder::new(Cursor::new(&member.payload)), destination),

        "data.tar.xz" => unpack_tar(XzDecoder::new(Cursor::new(&member.payload)), destination),

        "data.tar.zst" => {
            let decoder = ZstdDecoder::new(Cursor::new(&member.payload))
                .map_err(|error| format!("could not decode material zstd payload: {error}"))?;

            unpack_tar(decoder, destination)
        }

        other => Err(format!("unsupported Debian package data member: {other}")),
    }
}

pub(crate) fn extract_deb_data_to_staging(
    deb_path: &Path,
    destination: &Path,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(deb_path).map_err(|error| {
        format!(
            "could not inspect module package {}: {error}",
            deb_path.display()
        )
    })?;

    if !metadata.file_type().is_file() {
        return Err(format!(
            "module package is not a regular file: {}",
            deb_path.display()
        ));
    }

    if fs::symlink_metadata(destination).is_ok() {
        return Err(format!(
            "module material staging destination already exists: {}",
            destination.display()
        ));
    }

    let payload = fs::read(deb_path).map_err(|error| {
        format!(
            "could not read module package {}: {error}",
            deb_path.display()
        )
    })?;

    let member = deb_data_member(&payload)?;

    unpack_data_member(&member, destination)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleMaterializationReport {
    pub(crate) module: String,
    pub(crate) version: String,
    pub(crate) packages: usize,
    pub(crate) entries: usize,
    pub(crate) published: usize,
    pub(crate) reused: usize,
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("could not open material file {}: {error}", path.display()))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];

    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("could not hash material file {}: {error}", path.display()))?;

        if count == 0 {
            break;
        }

        hasher.update(&buffer[..count]);
    }

    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn filesystem_mode(metadata: &fs::Metadata) -> u32 {
    metadata.permissions().mode() & 0o7777
}

fn collect_tree(root: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let mut collected = BTreeSet::new();

    fn walk(base: &Path, current: &Path, collected: &mut BTreeSet<PathBuf>) -> Result<(), String> {
        let entries = fs::read_dir(current).map_err(|error| {
            format!(
                "could not read material directory {}: {error}",
                current.display()
            )
        })?;

        for item in entries {
            let item = item.map_err(|error| {
                format!(
                    "could not read material directory entry in {}: {error}",
                    current.display()
                )
            })?;

            let path = item.path();

            let relative = path
                .strip_prefix(base)
                .map_err(|_| format!("material path escaped tree root: {}", path.display()))?
                .to_path_buf();

            collected.insert(relative);

            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                format!(
                    "could not inspect material path {}: {error}",
                    path.display()
                )
            })?;

            if metadata.file_type().is_dir() {
                walk(base, &path, collected)?;
            }
        }

        Ok(())
    }

    walk(root, root, &mut collected)?;

    Ok(collected)
}

fn rootfs_recipe_entries(
    recipe: &MaterialRecipe,
) -> Result<BTreeMap<PathBuf, &MaterialEntry>, String> {
    let mut entries = BTreeMap::<PathBuf, &MaterialEntry>::new();

    for entry in &recipe.entries {
        let Some(relative) = entry.path.strip_prefix("rootfs/") else {
            continue;
        };

        if relative.is_empty() {
            return Err("rootfs material entry cannot target root itself".to_string());
        }

        let relative = PathBuf::from(relative);

        if entries.insert(relative.clone(), entry).is_some() {
            return Err(format!(
                "duplicate rootfs material entry: {}",
                relative.display()
            ));
        }
    }

    Ok(entries)
}

fn verify_material_entry(
    root: &Path,
    relative: &Path,
    entry: &MaterialEntry,
) -> Result<(), String> {
    let path = root.join(relative);

    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("required material path missing {}: {error}", path.display()))?;

    let actual_mode = filesystem_mode(&metadata);

    if actual_mode != entry.mode {
        return Err(format!(
            "material mode mismatch {}: expected={:o} actual={:o}",
            relative.display(),
            entry.mode,
            actual_mode
        ));
    }

    match entry.kind.as_str() {
        "file" => {
            if !metadata.file_type().is_file() {
                return Err(format!(
                    "material type mismatch: expected file {}",
                    relative.display()
                ));
            }

            let expected_size = entry.size.ok_or_else(|| {
                format!(
                    "material file size missing from recipe: {}",
                    relative.display()
                )
            })?;

            if metadata.len() != expected_size {
                return Err(format!(
                    "material size mismatch {}: expected={} actual={}",
                    relative.display(),
                    expected_size,
                    metadata.len()
                ));
            }

            let expected_sha = entry.sha256.as_deref().ok_or_else(|| {
                format!(
                    "material file sha256 missing from recipe: {}",
                    relative.display()
                )
            })?;

            let actual_sha = sha256_file(&path)?;

            if actual_sha != expected_sha {
                return Err(format!("material sha256 mismatch: {}", relative.display()));
            }
        }

        "directory" | "dir" => {
            if !metadata.file_type().is_dir() {
                return Err(format!(
                    "material type mismatch: expected directory {}",
                    relative.display()
                ));
            }
        }

        "symlink" => {
            if !metadata.file_type().is_symlink() {
                return Err(format!(
                    "material type mismatch: expected symlink {}",
                    relative.display()
                ));
            }

            let expected_target = entry.target.as_deref().ok_or_else(|| {
                format!(
                    "material symlink target missing from recipe: {}",
                    relative.display()
                )
            })?;

            let actual_target = fs::read_link(&path).map_err(|error| {
                format!(
                    "could not read material symlink {}: {error}",
                    path.display()
                )
            })?;

            if actual_target != PathBuf::from(expected_target) {
                return Err(format!(
                    "material symlink target mismatch {}: expected={} actual={}",
                    relative.display(),
                    expected_target,
                    actual_target.display()
                ));
            }
        }

        other => {
            return Err(format!(
                "unsupported material recipe type '{other}' for {}",
                relative.display()
            ));
        }
    }

    Ok(())
}

fn apply_certified_metadata(staging: &Path, recipe: &MaterialRecipe) -> Result<(), String> {
    let entries = rootfs_recipe_entries(recipe)?;

    for (relative, entry) in entries {
        let path = staging.join(&relative);

        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "required material path missing before certification {}: {error}",
                path.display()
            )
        })?;

        match entry.kind.as_str() {
            "file" => {
                if !metadata.file_type().is_file() {
                    return Err(format!(
                        "material type mismatch before certification: expected file {}",
                        relative.display()
                    ));
                }

                fs::set_permissions(&path, fs::Permissions::from_mode(entry.mode)).map_err(
                    |error| {
                        format!(
                            "could not apply certified file mode {}: {error}",
                            path.display()
                        )
                    },
                )?;
            }

            "directory" | "dir" => {
                if !metadata.file_type().is_dir() {
                    return Err(format!(
                        "material type mismatch before certification: expected directory {}",
                        relative.display()
                    ));
                }

                fs::set_permissions(&path, fs::Permissions::from_mode(entry.mode)).map_err(
                    |error| {
                        format!(
                            "could not apply certified directory mode {}: {error}",
                            path.display()
                        )
                    },
                )?;
            }

            "symlink" => {
                if !metadata.file_type().is_symlink() {
                    return Err(format!(
                        "material type mismatch before certification: expected symlink {}",
                        relative.display()
                    ));
                }
            }

            other => {
                return Err(format!(
                    "unsupported certified material type '{other}' for {}",
                    relative.display()
                ));
            }
        }
    }

    Ok(())
}

fn verify_staging_exact(staging: &Path, recipe: &MaterialRecipe) -> Result<(), String> {
    let expected = rootfs_recipe_entries(recipe)?;

    let actual = collect_tree(staging)?;

    let expected_paths = expected.keys().cloned().collect::<BTreeSet<_>>();

    if actual != expected_paths {
        let missing = expected_paths
            .difference(&actual)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();

        let unexpected = actual
            .difference(&expected_paths)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();

        return Err(format!(
            "material staging membership mismatch: missing={missing:?} unexpected={unexpected:?}"
        ));
    }

    for (relative, entry) in expected {
        verify_material_entry(staging, &relative, entry)?;
    }

    Ok(())
}

fn identical_material_path(source: &Path, destination: &Path) -> Result<bool, String> {
    let source_metadata = fs::symlink_metadata(source).map_err(|error| {
        format!(
            "could not inspect source material {}: {error}",
            source.display()
        )
    })?;

    let destination_metadata = fs::symlink_metadata(destination).map_err(|error| {
        format!(
            "could not inspect destination material {}: {error}",
            destination.display()
        )
    })?;

    if source_metadata.file_type().is_dir() && destination_metadata.file_type().is_dir() {
        return Ok(filesystem_mode(&source_metadata) == filesystem_mode(&destination_metadata));
    }

    if source_metadata.file_type().is_file() && destination_metadata.file_type().is_file() {
        if filesystem_mode(&source_metadata) != filesystem_mode(&destination_metadata) {
            return Ok(false);
        }

        if source_metadata.len() != destination_metadata.len() {
            return Ok(false);
        }

        return Ok(sha256_file(source)? == sha256_file(destination)?);
    }

    if source_metadata.file_type().is_symlink() && destination_metadata.file_type().is_symlink() {
        return Ok(fs::read_link(source).map_err(|error| {
            format!(
                "could not read source material symlink {}: {error}",
                source.display()
            )
        })? == fs::read_link(destination).map_err(|error| {
            format!(
                "could not read destination material symlink {}: {error}",
                destination.display()
            )
        })?);
    }

    Ok(false)
}

fn preflight_merge(source_root: &Path, destination_root: &Path) -> Result<(usize, usize), String> {
    let paths = collect_tree(source_root)?;

    let mut published = 0usize;
    let mut reused = 0usize;

    for relative in paths {
        let source = source_root.join(&relative);
        let destination = destination_root.join(&relative);

        match fs::symlink_metadata(&destination) {
            Ok(_) => {
                if !identical_material_path(&source, &destination)? {
                    return Err(format!(
                        "material collision differs from certified source: {}",
                        destination.display()
                    ));
                }

                reused += 1;
            }

            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                published += 1;
            }

            Err(error) => {
                return Err(format!(
                    "could not inspect material destination {}: {error}",
                    destination.display()
                ));
            }
        }
    }

    Ok((published, reused))
}

fn merge_tree(source_root: &Path, destination_root: &Path) -> Result<(), String> {
    fs::create_dir_all(destination_root).map_err(|error| {
        format!(
            "could not create shared module rootfs {}: {error}",
            destination_root.display()
        )
    })?;

    let paths = collect_tree(source_root)?;

    for relative in &paths {
        let source = source_root.join(relative);
        let destination = destination_root.join(relative);

        let source_metadata = fs::symlink_metadata(&source).map_err(|error| {
            format!(
                "could not inspect staged material {}: {error}",
                source.display()
            )
        })?;

        if source_metadata.file_type().is_dir() {
            if fs::symlink_metadata(&destination).is_err() {
                fs::create_dir(&destination).map_err(|error| {
                    format!(
                        "could not publish material directory {}: {error}",
                        destination.display()
                    )
                })?;

                fs::set_permissions(
                    &destination,
                    fs::Permissions::from_mode(filesystem_mode(&source_metadata)),
                )
                .map_err(|error| {
                    format!(
                        "could not publish material directory mode {}: {error}",
                        destination.display()
                    )
                })?;
            }
        }
    }

    for relative in &paths {
        let source = source_root.join(relative);
        let destination = destination_root.join(relative);

        if fs::symlink_metadata(&destination).is_ok() {
            continue;
        }

        let source_metadata = fs::symlink_metadata(&source).map_err(|error| {
            format!(
                "could not inspect staged material {}: {error}",
                source.display()
            )
        })?;

        if source_metadata.file_type().is_file() {
            create_parent(&destination)?;

            fs::copy(&source, &destination).map_err(|error| {
                format!(
                    "could not publish material file {}: {error}",
                    destination.display()
                )
            })?;

            fs::set_permissions(
                &destination,
                fs::Permissions::from_mode(filesystem_mode(&source_metadata)),
            )
            .map_err(|error| {
                format!(
                    "could not publish material file mode {}: {error}",
                    destination.display()
                )
            })?;

            continue;
        }

        if source_metadata.file_type().is_symlink() {
            create_parent(&destination)?;

            let target = fs::read_link(&source).map_err(|error| {
                format!(
                    "could not read staged symlink {}: {error}",
                    source.display()
                )
            })?;

            symlink(&target, &destination).map_err(|error| {
                format!(
                    "could not publish material symlink {} -> {}: {error}",
                    destination.display(),
                    target.display()
                )
            })?;
        }
    }

    Ok(())
}

fn merge_package_into_candidate(package_tree: &Path, candidate: &Path) -> Result<(), String> {
    fs::create_dir_all(candidate).map_err(|error| {
        format!(
            "could not create candidate module rootfs {}: {error}",
            candidate.display()
        )
    })?;

    preflight_merge(package_tree, candidate)?;

    merge_tree(package_tree, candidate)
}

pub(crate) fn materialize_recipe(
    recipe: &MaterialRecipe,
    package_pool: &Path,
    shared_rootfs: &Path,
) -> Result<ModuleMaterializationReport, String> {
    let modules_root = package_pool.parent().ok_or_else(|| {
        format!(
            "module package pool has no parent: {}",
            package_pool.display()
        )
    })?;

    let temporary = tempfile::Builder::new()
        .prefix(".neebles-module-materialize-")
        .tempdir_in(modules_root)
        .map_err(|error| {
            format!(
                "could not create module materialization staging under {}: {error}",
                modules_root.display()
            )
        })?;

    let candidate = temporary.path().join("rootfs");

    fs::create_dir(&candidate).map_err(|error| {
        format!(
            "could not create candidate module rootfs {}: {error}",
            candidate.display()
        )
    })?;

    for (index, requirement) in recipe.packages.iter().enumerate() {
        let package = package_pool.join(&requirement.filename);

        let metadata = fs::symlink_metadata(&package).map_err(|error| {
            format!(
                "required module package missing {}: {error}",
                package.display()
            )
        })?;

        if !metadata.file_type().is_file() {
            return Err(format!(
                "required module package is not a regular file: {}",
                package.display()
            ));
        }

        let actual_sha = sha256_file(&package)?;

        if actual_sha != requirement.sha256 {
            return Err(format!(
                "required module package sha256 mismatch: {}",
                package.display()
            ));
        }

        let package_staging = temporary.path().join(format!("package-{index:04}"));

        extract_deb_data_to_staging(&package, &package_staging)?;

        merge_package_into_candidate(&package_staging, &candidate)?;
    }

    apply_certified_metadata(&candidate, recipe)?;

    verify_staging_exact(&candidate, recipe)?;

    let (published, reused) = preflight_merge(&candidate, shared_rootfs)?;

    merge_tree(&candidate, shared_rootfs)?;

    Ok(ModuleMaterializationReport {
        module: recipe.module.clone(),
        version: recipe.version.clone(),
        packages: recipe.packages.len(),
        entries: rootfs_recipe_entries(recipe)?.len(),
        published,
        reused,
    })
}

pub(crate) fn publish_runtime_manifest(payload: &[u8], target: &Path) -> Result<bool, String> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    if !target.is_absolute() {
        return Err(format!(
            "module runtime manifest target must be absolute: {}",
            target.display()
        ));
    }

    let parent = target.parent().ok_or_else(|| {
        format!(
            "module runtime manifest target has no parent: {}",
            target.display()
        )
    })?;

    if !parent.is_dir() {
        return Err(format!(
            "module runtime manifest parent is not a directory: {}",
            parent.display()
        ));
    }

    if let Ok(existing) = fs::read(target) {
        if existing == payload {
            neebles_backend::domestic_runtime_authority::validate_materialized_runtime_manifest(
                target,
            )?;

            return Ok(true);
        }
    }

    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        format!(
            "could not create runtime manifest staging file in {}: {error}",
            parent.display()
        )
    })?;

    temporary
        .write_all(payload)
        .map_err(|error| format!("could not write runtime manifest staging file: {error}"))?;

    temporary
        .flush()
        .map_err(|error| format!("could not flush runtime manifest staging file: {error}"))?;

    fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o644))
        .map_err(|error| format!("could not apply runtime manifest mode: {error}"))?;

    neebles_backend::domestic_runtime_authority::validate_materialized_runtime_manifest(
        temporary.path(),
    )?;

    temporary.persist(target).map_err(|error| {
        format!(
            "could not publish runtime manifest {}: {}",
            target.display(),
            error.error
        )
    })?;

    neebles_backend::domestic_runtime_authority::validate_materialized_runtime_manifest(target)?;

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    fn ar_member(name: &str, payload: &[u8]) -> Vec<u8> {
        let identifier = format!("{name}/");

        let header = format!(
            "{identifier:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
            0,
            0,
            0,
            "100644",
            payload.len()
        );

        assert_eq!(header.as_bytes().len(), 60);

        let mut output = header.into_bytes();
        output.extend_from_slice(payload);

        if output.len() % 2 != 0 {
            output.push(b'\n');
        }

        output
    }

    fn synthetic_deb(data_name: &str, data: &[u8]) -> Vec<u8> {
        let mut payload = b"!<arch>\n".to_vec();

        payload.extend_from_slice(&ar_member("debian-binary", b"2.0\n"));

        payload.extend_from_slice(&ar_member(data_name, data));

        payload
    }

    fn gzip_tar_with_file() -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());

        let mut builder = tar::Builder::new(encoder);

        let body = b"hello-neebles\n";

        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();

        builder
            .append_data(&mut header, "usr/bin/fixture", Cursor::new(body))
            .expect("fixture tar file must append");

        let encoder = builder.into_inner().expect("fixture tar must finish");

        encoder.finish().expect("fixture gzip must finish")
    }

    #[test]
    fn deb_parser_finds_data_member() {
        let payload = synthetic_deb("data.tar.gz", b"fixture");

        let member = deb_data_member(&payload).expect("data member must resolve");

        assert_eq!(member.name, "data.tar.gz");
        assert_eq!(member.payload, b"fixture");
    }

    #[test]
    fn deb_parser_rejects_invalid_magic() {
        let error = deb_data_member(b"not-a-deb").expect_err("invalid magic must fail");

        assert!(error.contains("ar magic"));
    }

    #[test]
    fn archive_path_rejects_escape() {
        assert!(!safe_archive_path(Path::new("../escape")));

        assert!(!safe_archive_path(Path::new("usr/../escape")));

        assert!(safe_archive_path(Path::new("usr/bin/python3")));
    }

    #[test]
    fn extracts_gzip_deb_payload_to_staging() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let deb = temporary.path().join("fixture.deb");

        let destination = temporary.path().join("rootfs");

        fs::write(&deb, synthetic_deb("data.tar.gz", &gzip_tar_with_file()))
            .expect("fixture deb must write");

        extract_deb_data_to_staging(&deb, &destination).expect("fixture deb must extract");

        let material =
            fs::read(destination.join("usr/bin/fixture")).expect("extracted fixture must exist");

        assert_eq!(material, b"hello-neebles\n");
    }

    #[test]
    fn materialize_recipe_publishes_certified_rootfs() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let packages = temporary.path().join("packages");

        let rootfs = temporary.path().join("rootfs");

        fs::create_dir(&packages).expect("package pool must exist");

        fs::create_dir(&rootfs).expect("shared rootfs must exist");

        let deb_payload = synthetic_deb("data.tar.gz", &gzip_tar_with_file());

        let deb = packages.join("fixture.deb");

        fs::write(&deb, &deb_payload).expect("fixture deb must write");

        let mut hasher = Sha256::new();
        hasher.update(&deb_payload);

        let deb_sha = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        let body = b"hello-neebles\n";

        let mut hasher = Sha256::new();
        hasher.update(body);

        let body_sha = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        let recipe = MaterialRecipe {
            module: "fixture".to_string(),
            version: "1.0.0".to_string(),
            packages: vec![crate::module_material::PackageRequirement {
                filename: "fixture.deb".to_string(),
                sha256: deb_sha,
                mode: 0o664,
            }],
            entries: vec![
                MaterialEntry {
                    path: "rootfs/usr".to_string(),
                    kind: "directory".to_string(),
                    mode: 0o755,
                    size: None,
                    sha256: None,
                    target: None,
                },
                MaterialEntry {
                    path: "rootfs/usr/bin".to_string(),
                    kind: "directory".to_string(),
                    mode: 0o755,
                    size: None,
                    sha256: None,
                    target: None,
                },
                MaterialEntry {
                    path: "rootfs/usr/bin/fixture".to_string(),
                    kind: "file".to_string(),
                    mode: 0o755,
                    size: Some(body.len() as u64),
                    sha256: Some(body_sha),
                    target: None,
                },
            ],
        };

        let report = materialize_recipe(&recipe, &packages, &rootfs)
            .expect("certified recipe must materialize");

        assert_eq!(
            fs::read(rootfs.join("usr/bin/fixture")).expect("published fixture"),
            body
        );

        assert_eq!(report.packages, 1);
        assert_eq!(report.entries, 3);
        assert_eq!(report.published, 3);
        assert_eq!(report.reused, 0);
    }

    #[test]
    fn materialize_recipe_is_idempotent() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let source = temporary.path().join("source");

        let destination = temporary.path().join("destination");

        fs::create_dir_all(source.join("usr/bin")).expect("source dirs");

        fs::write(source.join("usr/bin/fixture"), b"same").expect("source file");

        fs::set_permissions(source.join("usr"), fs::Permissions::from_mode(0o755))
            .expect("usr mode");

        fs::set_permissions(source.join("usr/bin"), fs::Permissions::from_mode(0o755))
            .expect("bin mode");

        fs::set_permissions(
            source.join("usr/bin/fixture"),
            fs::Permissions::from_mode(0o644),
        )
        .expect("fixture mode");

        fs::create_dir_all(destination.join("usr/bin")).expect("destination dirs");

        fs::write(destination.join("usr/bin/fixture"), b"same").expect("destination file");

        fs::set_permissions(destination.join("usr"), fs::Permissions::from_mode(0o755))
            .expect("destination usr mode");

        fs::set_permissions(
            destination.join("usr/bin"),
            fs::Permissions::from_mode(0o755),
        )
        .expect("destination bin mode");

        fs::set_permissions(
            destination.join("usr/bin/fixture"),
            fs::Permissions::from_mode(0o644),
        )
        .expect("destination fixture mode");

        let result = preflight_merge(&source, &destination).expect("identical tree must reuse");

        assert_eq!(result, (0, 3));
    }

    #[test]
    fn materialize_recipe_rejects_conflicting_shared_material() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let source = temporary.path().join("source");

        let destination = temporary.path().join("destination");

        fs::create_dir_all(source.join("usr/bin")).expect("source dirs");

        fs::create_dir_all(destination.join("usr/bin")).expect("destination dirs");

        fs::write(source.join("usr/bin/fixture"), b"new").expect("source file");

        fs::write(destination.join("usr/bin/fixture"), b"old").expect("destination file");

        fs::set_permissions(source.join("usr"), fs::Permissions::from_mode(0o755))
            .expect("source usr mode");

        fs::set_permissions(source.join("usr/bin"), fs::Permissions::from_mode(0o755))
            .expect("source bin mode");

        fs::set_permissions(
            source.join("usr/bin/fixture"),
            fs::Permissions::from_mode(0o644),
        )
        .expect("source fixture mode");

        fs::set_permissions(destination.join("usr"), fs::Permissions::from_mode(0o755))
            .expect("destination usr mode");

        fs::set_permissions(
            destination.join("usr/bin"),
            fs::Permissions::from_mode(0o755),
        )
        .expect("destination bin mode");

        fs::set_permissions(
            destination.join("usr/bin/fixture"),
            fs::Permissions::from_mode(0o644),
        )
        .expect("destination fixture mode");

        let error = preflight_merge(&source, &destination)
            .expect_err("different shared material must fail");

        assert!(error.contains("material collision differs"));
    }

    #[test]
    fn staging_destination_must_be_new() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let deb = temporary.path().join("fixture.deb");

        let destination = temporary.path().join("rootfs");

        fs::write(&deb, synthetic_deb("data.tar.gz", &gzip_tar_with_file()))
            .expect("fixture deb must write");

        fs::create_dir(&destination).expect("fixture destination must exist");

        let error = extract_deb_data_to_staging(&deb, &destination)
            .expect_err("existing staging destination must fail");

        assert!(error.contains("staging destination already exists"));
    }
}

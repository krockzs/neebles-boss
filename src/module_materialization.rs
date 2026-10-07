use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;

use crate::module_material::{MaterialEntry, MaterialLayer, MaterialRecipe, PackageRequirement};

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingHardlink {
    path: PathBuf,
    target: PathBuf,
}

fn normalize_archive_path(path: &Path) -> Result<Option<PathBuf>, String> {
    if path.as_os_str().is_empty() {
        return Err("empty material tar path".to_string());
    }

    if path.is_absolute() {
        return Err(format!(
            "absolute material tar path is forbidden: {}",
            path.display()
        ));
    }

    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("unsafe material tar path: {}", path.display()));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        Ok(None)
    } else {
        Ok(Some(normalized))
    }
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
    let mut hardlinks = Vec::<PendingHardlink>::new();
    let mut symlinks = Vec::<PendingSymlink>::new();

    let entries = archive
        .entries()
        .map_err(|error| format!("could not read material tar entries: {error}"))?;

    for item in entries {
        let mut entry = item.map_err(|error| format!("invalid material tar entry: {error}"))?;

        let raw_path = entry
            .path()
            .map_err(|error| format!("invalid material tar path: {error}"))?
            .into_owned();

        let kind = entry.header().entry_type();

        let Some(path) = normalize_archive_path(&raw_path)? else {
            if kind.is_dir() {
                continue;
            }

            return Err(format!(
                "material tar root entry is not a directory: {}",
                raw_path.display()
            ));
        };

        let target = destination.join(&path);

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

        if kind.is_file() || kind.is_contiguous() || kind.is_gnu_sparse() {
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

        if kind.is_hard_link() {
            let raw_link = entry
                .link_name()
                .map_err(|error| {
                    format!(
                        "could not read material hardlink target {}: {error}",
                        path.display()
                    )
                })?
                .ok_or_else(|| format!("material hardlink target missing: {}", path.display()))?
                .into_owned();

            let link = normalize_archive_path(&raw_link)?.ok_or_else(|| {
                format!(
                    "material hardlink target cannot be archive root: {}",
                    path.display()
                )
            })?;

            hardlinks.push(PendingHardlink {
                path: target,
                target: destination.join(link),
            });

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

        if kind.is_character_special() || kind.is_block_special() || kind.is_fifo() {
            return Err(format!(
                "forbidden material tar special entry type 0x{:02x} for {}",
                kind.as_byte(),
                path.display()
            ));
        }

        if kind.is_pax_global_extensions() {
            return Err(format!(
                "unsupported material tar global PAX header for {}",
                path.display()
            ));
        }

        return Err(format!(
            "unsupported material tar entry type 0x{:02x} for {}",
            kind.as_byte(),
            path.display()
        ));
    }

    let mut unresolved = hardlinks;

    while !unresolved.is_empty() {
        let mut next = Vec::<PendingHardlink>::new();
        let mut progress = false;

        for pending in unresolved {
            create_parent(&pending.path)?;

            if fs::symlink_metadata(&pending.path).is_ok() {
                return Err(format!(
                    "duplicate material hardlink path: {}",
                    pending.path.display()
                ));
            }

            match fs::symlink_metadata(&pending.target) {
                Ok(metadata) => {
                    if !metadata.file_type().is_file() {
                        return Err(format!(
                            "material hardlink target is not a regular file: {}",
                            pending.target.display()
                        ));
                    }

                    fs::hard_link(&pending.target, &pending.path).map_err(|error| {
                        format!(
                            "could not create material hardlink {} -> {}: {error}",
                            pending.path.display(),
                            pending.target.display()
                        )
                    })?;

                    progress = true;
                }

                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    next.push(pending);
                }

                Err(error) => {
                    return Err(format!(
                        "could not inspect material hardlink target {}: {error}",
                        pending.target.display()
                    ));
                }
            }
        }

        if !next.is_empty() && !progress {
            let pending = next
                .iter()
                .map(|item| format!("{} -> {}", item.path.display(), item.target.display()))
                .collect::<Vec<_>>();

            return Err(format!("unresolved material hardlink targets: {pending:?}"));
        }

        unresolved = next;
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

fn rootfs_material_entries(
    material_entries: &[MaterialEntry],
) -> Result<BTreeMap<PathBuf, &MaterialEntry>, String> {
    let mut entries = BTreeMap::<PathBuf, &MaterialEntry>::new();

    for entry in material_entries {
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

fn apply_certified_metadata(
    staging: &Path,
    material_entries: &[MaterialEntry],
) -> Result<(), String> {
    let entries = rootfs_material_entries(material_entries)?;

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

fn verify_staging_exact(staging: &Path, material_entries: &[MaterialEntry]) -> Result<(), String> {
    let expected = rootfs_material_entries(material_entries)?;

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

fn verify_destination_parent_confinement(
    destination_root: &Path,
    relative: &Path,
) -> Result<(), String> {
    if relative.is_absolute() {
        return Err(format!(
            "material merge path must be relative: {}",
            relative.display()
        ));
    }

    let root_metadata = fs::symlink_metadata(destination_root).map_err(|error| {
        format!(
            "could not inspect material destination root {}: {error}",
            destination_root.display()
        )
    })?;

    if !root_metadata.file_type().is_dir() {
        return Err(format!(
            "material destination root is not a directory: {}",
            destination_root.display()
        ));
    }

    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let mut current = destination_root.to_path_buf();

    for component in parent.components() {
        match component {
            Component::Normal(value) => current.push(value),
            Component::CurDir => continue,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "unsafe material merge path: {}",
                    relative.display()
                ));
            }
        }

        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!(
                        "material destination parent is a symlink: {}",
                        current.display()
                    ));
                }

                if !metadata.file_type().is_dir() {
                    return Err(format!(
                        "material destination parent is not a directory: {}",
                        current.display()
                    ));
                }
            }

            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }

            Err(error) => {
                return Err(format!(
                    "could not inspect material destination parent {}: {error}",
                    current.display()
                ));
            }
        }
    }

    Ok(())
}

fn preflight_merge(source_root: &Path, destination_root: &Path) -> Result<(usize, usize), String> {
    let paths = collect_tree(source_root)?;

    let mut published = 0usize;
    let mut reused = 0usize;

    for relative in paths {
        let source = source_root.join(&relative);
        let destination = destination_root.join(&relative);

        verify_destination_parent_confinement(destination_root, &relative)?;

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

        verify_destination_parent_confinement(destination_root, relative)?;

        let source_metadata = fs::symlink_metadata(&source).map_err(|error| {
            format!(
                "could not inspect staged material {}: {error}",
                source.display()
            )
        })?;

        if source_metadata.file_type().is_dir() {
            match fs::symlink_metadata(&destination) {
                Ok(metadata) => {
                    if !metadata.file_type().is_dir() {
                        return Err(format!(
                            "material destination directory collides with non-directory: {}",
                            destination.display()
                        ));
                    }
                }

                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
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

                Err(error) => {
                    return Err(format!(
                        "could not inspect material destination directory {}: {error}",
                        destination.display()
                    ));
                }
            }
        }
    }

    let mut hardlink_destinations = BTreeMap::<(u64, u64), PathBuf>::new();

    for relative in &paths {
        let source = source_root.join(relative);
        let destination = destination_root.join(relative);

        verify_destination_parent_confinement(destination_root, relative)?;

        let source_metadata = fs::symlink_metadata(&source).map_err(|error| {
            format!(
                "could not inspect staged material {}: {error}",
                source.display()
            )
        })?;

        if source_metadata.file_type().is_file() {
            let hardlink_key = (source_metadata.dev(), source_metadata.ino());

            match fs::symlink_metadata(&destination) {
                Ok(_) => {
                    if source_metadata.nlink() > 1 {
                        hardlink_destinations
                            .entry(hardlink_key)
                            .or_insert_with(|| destination.clone());
                    }

                    continue;
                }

                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}

                Err(error) => {
                    return Err(format!(
                        "could not inspect material destination file {}: {error}",
                        destination.display()
                    ));
                }
            }

            create_parent(&destination)?;

            if source_metadata.nlink() > 1 {
                if let Some(existing) = hardlink_destinations.get(&hardlink_key) {
                    fs::hard_link(existing, &destination).map_err(|error| {
                        format!(
                            "could not preserve material hardlink {} -> {}: {error}",
                            destination.display(),
                            existing.display()
                        )
                    })?;

                    continue;
                }
            }

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

            if source_metadata.nlink() > 1 {
                hardlink_destinations.insert(hardlink_key, destination.clone());
            }

            continue;
        }

        if source_metadata.file_type().is_symlink() {
            match fs::symlink_metadata(&destination) {
                Ok(_) => continue,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "could not inspect material destination symlink {}: {error}",
                        destination.display()
                    ));
                }
            }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialLayerMaterializationReport {
    pub authority: String,
    pub packages: usize,
    pub entries: usize,
    pub published: usize,
    pub reused: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMaterializationReport {
    pub essential: MaterialLayerMaterializationReport,
    pub module: MaterialLayerMaterializationReport,
}

fn materialize_certified_layer(
    authority: &str,
    packages: &[PackageRequirement],
    entries: &[MaterialEntry],
    package_pool: &Path,
    destination_rootfs: &Path,
) -> Result<MaterialLayerMaterializationReport, String> {
    if authority.trim().is_empty() {
        return Err("material layer authority cannot be empty".to_string());
    }

    let staging_parent = destination_rootfs.parent().ok_or_else(|| {
        format!(
            "material destination rootfs has no parent: {}",
            destination_rootfs.display()
        )
    })?;

    fs::create_dir_all(staging_parent).map_err(|error| {
        format!(
            "could not create material staging parent {}: {error}",
            staging_parent.display()
        )
    })?;

    let temporary = tempfile::Builder::new()
        .prefix(".neebles-material-layer-")
        .tempdir_in(staging_parent)
        .map_err(|error| {
            format!(
                "could not create certified material layer staging under {}: {error}",
                staging_parent.display()
            )
        })?;

    let candidate = temporary.path().join("rootfs");

    fs::create_dir(&candidate).map_err(|error| {
        format!(
            "could not create certified material layer rootfs {}: {error}",
            candidate.display()
        )
    })?;

    for (index, requirement) in packages.iter().enumerate() {
        let package = package_pool.join(&requirement.filename);

        let metadata = fs::symlink_metadata(&package).map_err(|error| {
            format!(
                "required {authority} package missing {}: {error}",
                package.display()
            )
        })?;

        if !metadata.file_type().is_file() {
            return Err(format!(
                "required {authority} package is not a regular file: {}",
                package.display()
            ));
        }

        let actual_sha = sha256_file(&package)?;

        if actual_sha != requirement.sha256 {
            return Err(format!(
                "required {authority} package sha256 mismatch: {}",
                package.display()
            ));
        }

        let package_staging = temporary.path().join(format!("package-{index:04}"));

        extract_deb_data_to_staging(&package, &package_staging)?;

        merge_package_into_candidate(&package_staging, &candidate)?;
    }

    apply_certified_metadata(&candidate, entries)?;

    verify_staging_exact(&candidate, entries)?;

    let (published, reused) = preflight_merge(&candidate, destination_rootfs)?;

    merge_tree(&candidate, destination_rootfs)?;

    Ok(MaterialLayerMaterializationReport {
        authority: authority.to_string(),
        packages: packages.len(),
        entries: rootfs_material_entries(entries)?.len(),
        published,
        reused,
    })
}

pub fn materialize_runtime_layers(
    essential: &MaterialLayer,
    essential_package_pool: &Path,
    module: &MaterialRecipe,
    module_package_pool: &Path,
    destination_rootfs: &Path,
) -> Result<RuntimeMaterializationReport, String> {
    if fs::symlink_metadata(destination_rootfs).is_ok() {
        return Err(format!(
            "runtime rootfs destination already exists: {}",
            destination_rootfs.display()
        ));
    }

    let parent = destination_rootfs.parent().ok_or_else(|| {
        format!(
            "runtime rootfs destination has no parent: {}",
            destination_rootfs.display()
        )
    })?;

    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "could not create runtime rootfs parent {}: {error}",
            parent.display()
        )
    })?;

    let temporary = tempfile::Builder::new()
        .prefix(".neebles-runtime-rootfs-")
        .tempdir_in(parent)
        .map_err(|error| {
            format!(
                "could not create runtime rootfs composition staging under {}: {error}",
                parent.display()
            )
        })?;

    let composed_rootfs = temporary.path().join("rootfs");

    fs::create_dir(&composed_rootfs).map_err(|error| {
        format!(
            "could not create runtime rootfs composition {}: {error}",
            composed_rootfs.display()
        )
    })?;

    let essential_report = materialize_certified_layer(
        "Essential",
        &essential.packages,
        &essential.entries,
        essential_package_pool,
        &composed_rootfs,
    )?;

    let module_report = materialize_certified_layer(
        "module",
        &module.packages,
        &module.entries,
        module_package_pool,
        &composed_rootfs,
    )?;

    fs::rename(&composed_rootfs, destination_rootfs).map_err(|error| {
        format!(
            "could not publish completed runtime rootfs {} -> {}: {error}",
            composed_rootfs.display(),
            destination_rootfs.display()
        )
    })?;

    Ok(RuntimeMaterializationReport {
        essential: essential_report,
        module: module_report,
    })
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

    fn gzip_tar_with_hardlink(link_first: bool) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());

        let mut builder = tar::Builder::new(encoder);

        let body = b"#!/usr/bin/perl\n";

        let mut file = tar::Header::new_gnu();
        file.set_size(body.len() as u64);
        file.set_mode(0o755);
        file.set_cksum();

        let mut link = tar::Header::new_gnu();
        link.set_entry_type(tar::EntryType::Link);
        link.set_size(0);
        link.set_mode(0o755);

        if link_first {
            builder
                .append_link(&mut link, "usr/bin/perl5.40.1", "./usr/bin/perl")
                .expect("hardlink fixture must append");

            builder
                .append_data(&mut file, "usr/bin/perl", Cursor::new(body))
                .expect("hardlink target fixture must append");
        } else {
            builder
                .append_data(&mut file, "usr/bin/perl", Cursor::new(body))
                .expect("hardlink target fixture must append");

            builder
                .append_link(&mut link, "usr/bin/perl5.40.1", "./usr/bin/perl")
                .expect("hardlink fixture must append");
        }

        let encoder = builder.into_inner().expect("fixture tar must finish");

        encoder.finish().expect("fixture gzip must finish")
    }

    fn gzip_tar_with_missing_hardlink_target() -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());

        let mut builder = tar::Builder::new(encoder);

        let mut link = tar::Header::new_gnu();
        link.set_entry_type(tar::EntryType::Link);
        link.set_size(0);
        link.set_mode(0o755);

        builder
            .append_link(&mut link, "usr/bin/perl5.40.1", "./usr/bin/missing")
            .expect("missing hardlink fixture must append");

        let encoder = builder.into_inner().expect("fixture tar must finish");

        encoder.finish().expect("fixture gzip must finish")
    }

    fn gzip_tar_with_root_and_file() -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());

        let mut builder = tar::Builder::new(encoder);

        let mut root = tar::Header::new_gnu();
        root.set_entry_type(tar::EntryType::Directory);
        root.set_size(0);
        root.set_mode(0o755);
        root.set_cksum();

        builder
            .append_data(&mut root, "./", Cursor::new(Vec::<u8>::new()))
            .expect("root tar directory must append");

        let body = b"hello-neebles-root\n";

        let mut file = tar::Header::new_gnu();
        file.set_size(body.len() as u64);
        file.set_mode(0o755);
        file.set_cksum();

        builder
            .append_data(&mut file, "./usr/bin/fixture", Cursor::new(body))
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
    fn archive_path_normalizes_current_directory_components() {
        assert_eq!(
            normalize_archive_path(Path::new("./usr/bin/python3")).unwrap(),
            Some(PathBuf::from("usr/bin/python3"))
        );

        assert_eq!(normalize_archive_path(Path::new("./")).unwrap(), None);

        assert_eq!(
            normalize_archive_path(Path::new("usr/bin/python3")).unwrap(),
            Some(PathBuf::from("usr/bin/python3"))
        );
    }

    #[test]
    fn archive_path_rejects_escape_and_absolute_paths() {
        assert!(normalize_archive_path(Path::new("../escape")).is_err());

        assert!(normalize_archive_path(Path::new("usr/../escape")).is_err());

        assert!(normalize_archive_path(Path::new("/absolute")).is_err());
    }

    #[test]
    fn extracts_gzip_deb_payload_with_root_entry_to_staging() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let deb = temporary.path().join("fixture-root.deb");
        let destination = temporary.path().join("rootfs");

        fs::write(
            &deb,
            synthetic_deb("data.tar.gz", &gzip_tar_with_root_and_file()),
        )
        .expect("fixture deb must write");

        extract_deb_data_to_staging(&deb, &destination)
            .expect("root-prefixed fixture deb must extract");

        assert_eq!(
            fs::read(destination.join("usr/bin/fixture"))
                .expect("root-prefixed fixture must exist"),
            b"hello-neebles-root\n"
        );
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
    fn extracts_hardlink_to_existing_target_inside_staging() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let deb = temporary.path().join("fixture-hardlink.deb");
        let destination = temporary.path().join("rootfs");

        fs::write(
            &deb,
            synthetic_deb("data.tar.gz", &gzip_tar_with_hardlink(false)),
        )
        .expect("hardlink fixture deb must write");

        extract_deb_data_to_staging(&deb, &destination).expect("hardlink fixture deb must extract");

        let target =
            fs::metadata(destination.join("usr/bin/perl")).expect("hardlink target must exist");

        let link =
            fs::metadata(destination.join("usr/bin/perl5.40.1")).expect("hardlink path must exist");

        assert_eq!(target.ino(), link.ino());
        assert_eq!(target.dev(), link.dev());
    }

    #[test]
    fn extracts_hardlink_whose_target_appears_later() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let deb = temporary.path().join("fixture-hardlink-late.deb");
        let destination = temporary.path().join("rootfs");

        fs::write(
            &deb,
            synthetic_deb("data.tar.gz", &gzip_tar_with_hardlink(true)),
        )
        .expect("late hardlink fixture deb must write");

        extract_deb_data_to_staging(&deb, &destination)
            .expect("late hardlink fixture deb must extract");

        let target = fs::metadata(destination.join("usr/bin/perl"))
            .expect("late hardlink target must exist");

        let link = fs::metadata(destination.join("usr/bin/perl5.40.1"))
            .expect("late hardlink path must exist");

        assert_eq!(target.ino(), link.ino());
        assert_eq!(target.dev(), link.dev());
    }

    #[test]
    fn missing_hardlink_target_fails_closed() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let deb = temporary.path().join("fixture-hardlink-missing.deb");
        let destination = temporary.path().join("rootfs");

        fs::write(
            &deb,
            synthetic_deb("data.tar.gz", &gzip_tar_with_missing_hardlink_target()),
        )
        .expect("missing hardlink fixture deb must write");

        let error = extract_deb_data_to_staging(&deb, &destination)
            .expect_err("missing hardlink target must fail");

        assert!(error.contains("unresolved material hardlink targets"));
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

    #[test]
    fn merge_tree_preserves_hardlink_identity() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let source = temporary.path().join("source");
        let destination = temporary.path().join("destination");

        fs::create_dir_all(source.join("usr/bin")).expect("source directories must exist");
        fs::create_dir(&destination).expect("destination root must exist");

        fs::write(source.join("usr/bin/perl"), b"perl-hardlink\n")
            .expect("hardlink source file must write");

        fs::hard_link(
            source.join("usr/bin/perl"),
            source.join("usr/bin/perl5.40.1"),
        )
        .expect("source hardlink must exist");

        preflight_merge(&source, &destination).expect("hardlink merge preflight must pass");
        merge_tree(&source, &destination).expect("hardlink merge must pass");

        let target = fs::metadata(destination.join("usr/bin/perl"))
            .expect("merged hardlink target must exist");

        let link = fs::metadata(destination.join("usr/bin/perl5.40.1"))
            .expect("merged hardlink path must exist");

        assert_eq!(target.dev(), link.dev());
        assert_eq!(target.ino(), link.ino());
    }

    #[test]
    fn merge_tree_rejects_symlink_parent_escape_without_writing_outside() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let source = temporary.path().join("source");
        let destination = temporary.path().join("destination");
        let outside = temporary.path().join("outside");

        fs::create_dir_all(source.join("usr/bin")).expect("source directories must exist");
        fs::create_dir(&destination).expect("destination root must exist");
        fs::create_dir(&outside).expect("outside root must exist");

        fs::write(source.join("usr/bin/fixture"), b"must-stay-contained\n")
            .expect("source fixture must write");

        symlink(&outside, destination.join("usr")).expect("destination escape symlink must exist");

        let error = preflight_merge(&source, &destination)
            .expect_err("symlink parent escape must fail preflight");

        assert!(error.contains("symlink") || error.contains("collision"));
        assert!(!outside.join("bin/fixture").exists());

        let error =
            merge_tree(&source, &destination).expect_err("symlink parent escape must fail merge");

        assert!(error.contains("symlink") || error.contains("non-directory"));
        assert!(!outside.join("bin/fixture").exists());
    }

    #[test]
    fn runtime_layers_publish_one_fresh_composed_rootfs() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let essential_pool = temporary.path().join("essentials");
        let module_pool = temporary.path().join("packages");
        let runtime_rootfs = temporary.path().join("lease").join("rootfs");

        fs::create_dir_all(&essential_pool).expect("Essential pool");
        fs::create_dir_all(&module_pool).expect("module pool");

        let essential = MaterialLayer {
            packages: Vec::new(),
            entries: Vec::new(),
        };

        let module = MaterialRecipe {
            module: "fixture".to_string(),
            version: "1.0.0".to_string(),
            packages: Vec::new(),
            entries: Vec::new(),
        };

        let report = materialize_runtime_layers(
            &essential,
            &essential_pool,
            &module,
            &module_pool,
            &runtime_rootfs,
        )
        .expect("runtime layers must compose");

        assert!(runtime_rootfs.is_dir());
        assert_eq!(report.essential.authority, "Essential");
        assert_eq!(report.module.authority, "module");
        assert_eq!(report.essential.packages, 0);
        assert_eq!(report.module.packages, 0);
    }

    #[test]
    fn runtime_layers_refuse_existing_destination() {
        let temporary = tempfile::tempdir().expect("temporary directory");

        let essential_pool = temporary.path().join("essentials");
        let module_pool = temporary.path().join("packages");
        let runtime_rootfs = temporary.path().join("rootfs");

        fs::create_dir_all(&essential_pool).expect("Essential pool");
        fs::create_dir_all(&module_pool).expect("module pool");
        fs::create_dir(&runtime_rootfs).expect("existing runtime rootfs");

        let essential = MaterialLayer {
            packages: Vec::new(),
            entries: Vec::new(),
        };

        let module = MaterialRecipe {
            module: "fixture".to_string(),
            version: "1.0.0".to_string(),
            packages: Vec::new(),
            entries: Vec::new(),
        };

        let error = materialize_runtime_layers(
            &essential,
            &essential_pool,
            &module,
            &module_pool,
            &runtime_rootfs,
        )
        .expect_err("existing runtime rootfs must be rejected");

        assert!(error.contains("already exists"));
    }
}

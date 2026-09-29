import json
import os
import shutil
from pathlib import Path

def stage_from_inventory_manifest(
    source_root,
    staged_root,
    source_manifest_path,
    expected_component=None,
    inventory_prefix="rootfs",
):
    import stat

    manifest = json.loads(
        source_manifest_path.read_text(
            encoding="utf-8"
        )
    )

    component = manifest.get("component")

    if expected_component is not None and component != expected_component:
        raise RuntimeError(
            "source inventory manifest component mismatch"
        )

    if not isinstance(inventory_prefix, str) or not inventory_prefix:
        raise RuntimeError(
            "source inventory prefix is invalid"
        )

    entries = manifest.get(
        "entries"
    )

    if not isinstance(
        entries,
        list,
    ):
        raise RuntimeError(
            "source inventory manifest entries is not a list"
        )

    normalized = {}

    for item in entries:
        if not isinstance(
            item,
            dict,
        ):
            raise RuntimeError(
                "source inventory contains invalid entry"
            )

        path_name = item.get(
            "path"
        )

        kind = item.get(
            "type"
        )

        mode_text = item.get(
            "mode"
        )

        if not isinstance(
            path_name,
            str,
        ):
            raise RuntimeError(
                "source inventory entry path is invalid"
            )

        prefix = inventory_prefix.rstrip("/")

        if (
            path_name != prefix
            and not path_name.startswith(
                prefix + "/"
            )
        ):
            continue

        if path_name == prefix:
            relative = Path(".")
        else:
            relative = Path(
                path_name
            ).relative_to(
                prefix
            )

        if relative.is_absolute():
            raise RuntimeError(
                "source inventory contains absolute staged entry: "
                + path_name
            )

        if ".." in relative.parts:
            raise RuntimeError(
                "source inventory staged entry escapes root: "
                + path_name
            )

        if kind not in {
            "file",
            "dir",
            "symlink",
        }:
            raise RuntimeError(
                "unsupported source inventory type "
                + str(
                    kind
                )
                + " for "
                + path_name
            )

        if not isinstance(
            mode_text,
            str,
        ):
            raise RuntimeError(
                "source inventory mode is invalid for "
                + path_name
            )

        key = relative.as_posix()

        if key in normalized:
            raise RuntimeError(
                "duplicate source inventory staged entry: "
                + key
            )

        normalized[
            key
        ] = {
            "relative": relative,
            "kind": kind,
            "mode": int(
                mode_text,
                8,
            ),
            "target": item.get(
                "target"
            ),
        }

    root_entry = normalized.get(
        "."
    )

    if (
        root_entry is None
        or root_entry["kind"] != "dir"
    ):
        raise RuntimeError(
            "source inventory does not declare staged root directory"
        )

    staged_root.mkdir(
        parents=True,
        exist_ok=False,
    )

    directories = [
        item
        for item in normalized.values()
        if item["kind"] == "dir"
        and item["relative"] != Path(".")
    ]

    directories.sort(
        key=lambda item: (
            len(
                item["relative"].parts
            ),
            item["relative"].as_posix(),
        )
    )

    files = [
        item
        for item in normalized.values()
        if item["kind"] == "file"
    ]

    files.sort(
        key=lambda item:
            item["relative"].as_posix()
    )

    symlinks = [
        item
        for item in normalized.values()
        if item["kind"] == "symlink"
    ]

    symlinks.sort(
        key=lambda item:
            item["relative"].as_posix()
    )

    created_directories = 1
    copied_files = 0
    created_symlinks = 0

    for item in directories:
        relative = item[
            "relative"
        ]

        source = (
            source_root
            / relative
        )

        destination = (
            staged_root
            / relative
        )

        try:
            source_stat = os.lstat(
                source
            )
        except OSError as error:
            raise RuntimeError(
                "could not inspect manifest directory "
                + relative.as_posix()
                + ": "
                + repr(
                    error
                )
            )

        if not stat.S_ISDIR(
            source_stat.st_mode
        ):
            raise RuntimeError(
                "manifest directory type differs from source: "
                + relative.as_posix()
            )

        parent = destination.parent

        if (
            not parent.is_dir()
            or parent.is_symlink()
        ):
            raise RuntimeError(
                "manifest directory parent is unsafe: "
                + relative.as_posix()
            )

        destination.mkdir(
            mode=0o700,
        )

        created_directories += 1

    for item in files:
        relative = item[
            "relative"
        ]

        source = (
            source_root
            / relative
        )

        destination = (
            staged_root
            / relative
        )

        try:
            source_stat = os.lstat(
                source
            )
        except OSError as error:
            raise RuntimeError(
                "could not inspect manifest file "
                + relative.as_posix()
                + ": "
                + repr(
                    error
                )
            )

        if not stat.S_ISREG(
            source_stat.st_mode
        ):
            raise RuntimeError(
                "manifest file type differs from source: "
                + relative.as_posix()
            )

        if (
            not destination.parent.is_dir()
            or destination.parent.is_symlink()
        ):
            raise RuntimeError(
                "manifest file parent is unsafe: "
                + relative.as_posix()
            )

        try:
            shutil.copy2(
                source,
                destination,
                follow_symlinks=False,
            )
        except PermissionError as error:
            raise RuntimeError(
                "manifest declares payload below an unreadable source boundary: "
                + relative.as_posix()
                + " :: "
                + repr(
                    error
                )
            )

        os.chmod(
            destination,
            item[
                "mode"
            ],
            follow_symlinks=False,
        )

        copied_files += 1

    for item in symlinks:
        relative = item[
            "relative"
        ]

        source = (
            source_root
            / relative
        )

        destination = (
            staged_root
            / relative
        )

        target = item.get(
            "target"
        )

        if not isinstance(
            target,
            str,
        ):
            raise RuntimeError(
                "manifest symlink target is invalid: "
                + relative.as_posix()
            )

        try:
            source_stat = os.lstat(
                source
            )
        except OSError as error:
            raise RuntimeError(
                "could not inspect manifest symlink "
                + relative.as_posix()
                + ": "
                + repr(
                    error
                )
            )

        if not stat.S_ISLNK(
            source_stat.st_mode
        ):
            raise RuntimeError(
                "manifest symlink type differs from source: "
                + relative.as_posix()
            )

        try:
            source_target = os.readlink(
                source
            )
        except PermissionError as error:
            raise RuntimeError(
                "manifest declares symlink below an unreadable source boundary: "
                + relative.as_posix()
                + " :: "
                + repr(
                    error
                )
            )

        if source_target != target:
            raise RuntimeError(
                "manifest symlink target differs from source: "
                + relative.as_posix()
            )

        if (
            not destination.parent.is_dir()
            or destination.parent.is_symlink()
        ):
            raise RuntimeError(
                "manifest symlink parent is unsafe: "
                + relative.as_posix()
            )

        os.symlink(
            target,
            destination,
        )

        created_symlinks += 1

    directories_for_modes = [
        root_entry
    ] + directories

    directories_for_modes.sort(
        key=lambda item: (
            len(
                item["relative"].parts
            ),
            item["relative"].as_posix(),
        ),
        reverse=True,
    )

    for item in directories_for_modes:
        relative = item[
            "relative"
        ]

        destination = (
            staged_root
            if relative == Path(".")
            else staged_root
            / relative
        )

        os.chmod(
            destination,
            item[
                "mode"
            ],
            follow_symlinks=False,
        )

    return {
        "entries": len(
            normalized
        ),
        "directories": created_directories,
        "files": copied_files,
        "symlinks": created_symlinks,
    }

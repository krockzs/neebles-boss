#!/usr/bin/env python3

import argparse
import gzip
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


def inside(root, path):
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False



def stage_from_source_manifest(
    source_root,
    staged_root,
    source_manifest_path,
):
    import stat

    manifest = json.loads(
        source_manifest_path.read_text(
            encoding="utf-8"
        )
    )

    if manifest.get("component") != "boss":
        raise RuntimeError(
            "source inventory manifest is not boss"
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

        if (
            path_name != "rootfs"
            and not path_name.startswith(
                "rootfs/"
            )
        ):
            continue

        if path_name == "rootfs":
            relative = Path(".")
        else:
            relative = Path(
                path_name
            ).relative_to(
                "rootfs"
            )

        if relative.is_absolute():
            raise RuntimeError(
                "source inventory contains absolute rootfs entry: "
                + path_name
            )

        if ".." in relative.parts:
            raise RuntimeError(
                "source inventory rootfs entry escapes root: "
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
                "duplicate source inventory rootfs entry: "
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
            "source inventory does not declare rootfs directory"
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


def main():
    parser = argparse.ArgumentParser()

    parser.add_argument(
        "--root",
        required=True,
    )

    parser.add_argument(
        "--manifest",
        required=True,
    )

    parser.add_argument(
        "--output",
        required=True,
    )

    parser.add_argument(
        "--report",
        required=True,
    )

    parser.add_argument(
        "--transport-tool",
        required=True,
    )

    parser.add_argument(
        "--source-manifest",
        required=True,
    )

    args = parser.parse_args()

    source_root = Path(
        args.root
    ).resolve()

    root = source_root

    manifest_path = Path(
        args.manifest
    ).resolve()

    output = Path(
        args.output
    ).resolve()

    report = Path(
        args.report
    ).resolve()

    transport_tool = Path(
        args.transport_tool
    ).resolve()

    source_manifest_path = Path(
        args.source_manifest
    ).resolve()

    lines = []

    def emit(value=""):
        value = str(value)

        print(value)

        lines.append(
            value
            + "\n"
        )

    def finish(ok):
        report.parent.mkdir(
            parents=True,
            exist_ok=True,
        )

        report.write_text(
            "".join(lines),
            encoding="utf-8",
        )

        if not ok:
            if output.exists():
                output.unlink()

            raise SystemExit(1)

    emit(
        "============================================================"
    )

    emit(
        "N.E.E.B.L.E.S. BOSS RUNTIME TRANSPORT CERTIFICATION"
    )

    emit(
        "============================================================"
    )

    emit(
        "ROOT :: "
        + str(root)
    )

    emit(
        "MANIFEST :: "
        + str(manifest_path)
    )

    if not root.is_dir():
        emit(
            "FATAL :: runtime root does not exist"
        )

        finish(False)

    if not manifest_path.is_file():
        emit(
            "FATAL :: domestic runtime manifest does not exist"
        )

        finish(False)

    manifest = json.loads(
        manifest_path.read_text(
            encoding="utf-8"
        )
    )

    if manifest.get("schema") != "1":
        emit(
            "FATAL :: runtime manifest schema is not 1"
        )

        finish(False)

    if (
        manifest.get("name")
        != "neebles-domestic-runtime"
    ):
        emit(
            "FATAL :: runtime manifest identity is invalid"
        )

        finish(False)

    if manifest.get("root") != "rootfs":
        emit(
            "FATAL :: runtime manifest root is not relocatable rootfs"
        )

        finish(False)

    if not transport_tool.is_file():
        emit(
            "FATAL :: domestic transport tool is not a file"
        )

        finish(False)

    if not os.access(
        transport_tool,
        os.X_OK,
    ):
        emit(
            "FATAL :: domestic transport tool is not executable"
        )

        finish(False)

    transport_stage = tempfile.TemporaryDirectory(
        prefix="neebles-runtime-transport-"
    )

    staged_root = (
        Path(
            transport_stage.name
        )
        / "rootfs"
    )

    emit()

    emit(
        "============================================================"
    )

    emit(
        "DOMESTIC TRANSPORT MATERIALIZATION"
    )

    emit(
        "============================================================"
    )

    emit(
        "SOURCE ROOT :: "
        + str(
            source_root
        )
    )

    emit(
        "STAGED ROOT :: "
        + str(
            staged_root
        )
    )

    if not source_manifest_path.is_file():
        emit(
            "FATAL :: source inventory manifest is not a file"
        )

        finish(False)

    try:
        staged = stage_from_source_manifest(
            source_root,
            staged_root,
            source_manifest_path,
        )
    except Exception as error:
        emit(
            "FATAL :: source manifest staging failed :: "
            + repr(
                error
            )
        )

        finish(False)

    emit(
        "SOURCE INVENTORY ENTRIES :: "
        + str(
            staged[
                "entries"
            ]
        )
    )

    emit(
        "STAGED DIRECTORIES :: "
        + str(
            staged[
                "directories"
            ]
        )
    )

    emit(
        "STAGED FILES :: "
        + str(
            staged[
                "files"
            ]
        )
    )

    emit(
        "STAGED SYMLINKS :: "
        + str(
            staged[
                "symlinks"
            ]
        )
    )

    transport_result = subprocess.run(
        [
            str(
                transport_tool
            ),
            "--staged-root",
            str(
                staged_root
            ),
        ],
        capture_output=True,
        text=True,
    )

    emit(
        "TRANSPORT EXIT :: "
        + str(
            transport_result.returncode
        )
    )

    for line in transport_result.stdout.splitlines():
        emit(
            "TRANSPORT :: "
            + line
        )

    for line in transport_result.stderr.splitlines():
        emit(
            "TRANSPORT STDERR :: "
            + line
        )

    if transport_result.returncode != 0:
        finish(False)

    root = staged_root

    canonical_root = root.resolve(
        strict=True
    )

    emit()

    emit(
        "============================================================"
    )

    emit(
        "DECLARED TARGET CERTIFICATION"
    )

    emit(
        "============================================================"
    )

    target_count = 0
    target_errors = []

    worlds = manifest.get(
        "worlds",
        {}
    )

    if not isinstance(
        worlds,
        dict,
    ):
        emit(
            "FATAL :: worlds is not an object"
        )

        finish(False)

    for world_name in sorted(
        worlds
    ):
        world = worlds[
            world_name
        ]

        categories = world.get(
            "categories",
            {}
        )

        if not isinstance(
            categories,
            dict,
        ):
            target_errors.append(
                world_name
                + " :: categories is not an object"
            )

            continue

        for category_name in sorted(
            categories
        ):
            category = categories[
                category_name
            ]

            targets = category.get(
                "resolved_targets",
                []
            )

            if not isinstance(
                targets,
                list,
            ):
                target_errors.append(
                    world_name
                    + " :: "
                    + category_name
                    + " :: resolved_targets is not an array"
                )

                continue

            for raw_target in targets:
                target_count += 1

                if not isinstance(
                    raw_target,
                    str,
                ):
                    target_errors.append(
                        world_name
                        + " :: "
                        + category_name
                        + " :: non String target"
                    )

                    continue

                relative = Path(
                    raw_target
                )

                if (
                    relative.is_absolute()
                    or ".." in relative.parts
                ):
                    target_errors.append(
                        world_name
                        + " :: "
                        + category_name
                        + " :: invalid relative target :: "
                        + raw_target
                    )

                    continue

                physical = (
                    root
                    / relative
                )

                try:
                    resolved = physical.resolve(
                        strict=True
                    )
                except OSError as error:
                    target_errors.append(
                        world_name
                        + " :: "
                        + category_name
                        + " :: unresolved :: "
                        + raw_target
                        + " :: "
                        + str(error)
                    )

                    continue

                if not inside(
                    canonical_root,
                    resolved,
                ):
                    target_errors.append(
                        world_name
                        + " :: "
                        + category_name
                        + " :: escaped runtime :: "
                        + raw_target
                        + " -> "
                        + str(resolved)
                    )

    emit(
        "TARGETS :: "
        + str(target_count)
    )

    emit(
        "TARGET ERRORS :: "
        + str(
            len(
                target_errors
            )
        )
    )

    for error in target_errors:
        emit(
            "TARGET FINDING :: "
            + error
        )

    emit()

    emit(
        "============================================================"
    )

    emit(
        "TRANSPORTED ROOTFS SYMLINK POSTCONDITION"
    )

    emit(
        "============================================================"
    )

    symlink_count = 0
    symlink_findings = []

    for current, directories, files in os.walk(
        root,
        topdown=True,
        followlinks=False,
    ):
        current_path = Path(
            current
        )

        names = list(
            directories
        ) + list(
            files
        )

        for name in names:
            path = (
                current_path
                / name
            )

            if not path.is_symlink():
                continue

            symlink_count += 1

            try:
                raw_link = os.readlink(
                    path
                )
            except OSError as error:
                symlink_findings.append(
                    str(
                        path.relative_to(
                            root
                        )
                    )
                    + " :: READLINK ERROR :: "
                    + repr(
                        error
                    )
                )

                continue

            if raw_link.startswith("/"):
                symlink_findings.append(
                    str(
                        path.relative_to(
                            root
                        )
                    )
                    + " :: ABSOLUTE SURVIVED TRANSPORT :: "
                    + raw_link
                )

    emit(
        "SYMLINKS :: "
        + str(
            symlink_count
        )
    )

    emit(
        "ABSOLUTE TRANSPORT VIOLATIONS :: "
        + str(
            len(
                symlink_findings
            )
        )
    )

    for finding in symlink_findings:
        emit(
            "SYMLINK FINDING :: "
            + finding
        )

    if (
        target_errors
        or symlink_findings
    ):
        emit()

        emit(
            "NOT GREEN :: transport frontier is not closed"
        )

        finish(False)

    emit()

    emit(
        "GREEN :: filesystem transport frontier is closed"
    )

    output.parent.mkdir(
        parents=True,
        exist_ok=True,
    )

    if output.exists():
        output.unlink()

    with tempfile.TemporaryDirectory(
        prefix="neebles-runtime-package-"
    ) as temporary_name:
        temporary = Path(
            temporary_name
        )

        staged_manifest = (
            temporary
            / "domestic-runtime.json"
        )

        staged_manifest.write_bytes(
            manifest_path.read_bytes()
        )

        tar = subprocess.Popen(
            [
                "tar",
                "--sort=name",
                "--mtime=@0",
                "--owner=0",
                "--group=0",
                "--numeric-owner",
                "--hard-dereference",
                "-C",
                str(
                    temporary
                ),
                "-cf",
                "-",
                "domestic-runtime.json",
                "-C",
                str(
                    root.parent
                ),
                "rootfs",
            ],
            stdout=subprocess.PIPE,
        )

        if tar.stdout is None:
            emit(
                "FATAL :: tar stdout unavailable"
            )

            finish(False)

        with output.open(
            "wb"
        ) as output_handle:
            gzip_process = subprocess.Popen(
                [
                    "gzip",
                    "-n",
                ],
                stdin=tar.stdout,
                stdout=output_handle,
            )

            tar.stdout.close()

            gzip_code = gzip_process.wait()

        tar_code = tar.wait()

        if (
            tar_code != 0
            or gzip_code != 0
        ):
            emit(
                "FATAL :: runtime archive construction failed"
            )

            finish(False)

    emit()

    emit(
        "============================================================"
    )

    emit(
        "ARCHIVE CERTIFICATION"
    )

    emit(
        "============================================================"
    )

    listing = subprocess.run(
        [
            "tar",
            "-tzf",
            str(
                output
            ),
        ],
        capture_output=True,
        text=True,
    )

    if listing.returncode != 0:
        emit(
            "FATAL :: packaged runtime cannot be listed"
        )

        emit(
            listing.stderr
        )

        finish(False)

    entries = [
        value
        for value in listing.stdout.splitlines()
        if value
    ]

    invalid_entries = []

    for entry in entries:
        normalized = (
            entry[2:]
            if entry.startswith("./")
            else entry
        )

        candidate = Path(
            normalized
        )

        if (
            candidate.is_absolute()
            or ".." in candidate.parts
        ):
            invalid_entries.append(
                entry
            )

    emit(
        "ARCHIVE ENTRIES :: "
        + str(
            len(
                entries
            )
        )
    )

    emit(
        "INVALID ARCHIVE PATHS :: "
        + str(
            len(
                invalid_entries
            )
        )
    )

    for entry in invalid_entries:
        emit(
            "ARCHIVE FINDING :: "
            + entry
        )

    if invalid_entries:
        finish(False)

    with tempfile.TemporaryDirectory(
        prefix="neebles-runtime-extract-"
    ) as extraction_name:
        extraction = Path(
            extraction_name
        )

        extracted = subprocess.run(
            [
                "tar",
                "-xzf",
                str(
                    output
                ),
                "-C",
                str(
                    extraction
                ),
            ],
            capture_output=True,
            text=True,
        )

        if extracted.returncode != 0:
            emit(
                "FATAL :: packaged runtime cannot be extracted"
            )

            emit(
                extracted.stderr
            )

            finish(False)

        extracted_manifest = (
            extraction
            / "domestic-runtime.json"
        )

        extracted_root = (
            extraction
            / "rootfs"
        )

        if not extracted_manifest.is_file():
            emit(
                "FATAL :: extracted manifest missing"
            )

            finish(False)

        if not extracted_root.is_dir():
            emit(
                "FATAL :: extracted rootfs missing"
            )

            finish(False)

        extracted_data = json.loads(
            extracted_manifest.read_text(
                encoding="utf-8"
            )
        )

        if (
            extracted_data
            != manifest
        ):
            emit(
                "FATAL :: extracted manifest changed during transport"
            )

            finish(False)

        qdbus = (
            extracted_root
            / "usr/bin/qdbus6"
        )

        if not qdbus.is_symlink():
            emit(
                "FATAL :: qdbus6 symlink was not preserved"
            )

            finish(False)

        qdbus_target = os.readlink(
            qdbus
        )

        emit(
            "QDBUS6 LINK :: "
            + qdbus_target
        )

        try:
            qdbus_resolved = qdbus.resolve(
                strict=True
            )
        except OSError as error:
            emit(
                "FATAL :: transported qdbus6 does not resolve :: "
                + str(error)
            )

            finish(False)

        if not inside(
            extracted_root.resolve(),
            qdbus_resolved,
        ):
            emit(
                "FATAL :: transported qdbus6 escaped extracted root"
            )

            finish(False)

    emit(
        "ARCHIVE SIZE :: "
        + str(
            output.stat().st_size
        )
        + " bytes"
    )

    emit(
        "GREEN :: BOSS RUNTIME TRANSPORT CERTIFIED"
    )

    emit(
        "ARTIFACT :: "
        + str(
            output
        )
    )

    finish(True)


if __name__ == "__main__":
    main()

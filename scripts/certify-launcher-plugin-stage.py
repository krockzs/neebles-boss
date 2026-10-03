#!/usr/bin/env python3

import argparse
import shutil
import subprocess
from pathlib import Path


REQUIRED = (
    "libneebles-launcher-events.so",
    "libneebles-launcher-eventsplugin.so",
    "neebles-launcher-events.qmltypes",
    "qmldir",
)

ELF_NAMES = (
    "libneebles-launcher-events.so",
    "libneebles-launcher-eventsplugin.so",
)


def fail(message):
    raise SystemExit("FATAL: " + message)


def inspect_elf(path):
    result = subprocess.run(
        ["readelf", "-d", str(path)],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )

    if result.returncode != 0:
        fail(
            "readelf failed for staged BossEvents ELF: "
            + str(path)
        )

    dynamic_lines = [
        line
        for line in result.stdout.splitlines()
        if "(RPATH)" in line or "(RUNPATH)" in line
    ]

    if not any(
        chr(36) + "ORIGIN" in line
        for line in dynamic_lines
    ):
        fail(
            "staged BossEvents ELF missing ORIGIN-relative "
            "RPATH/RUNPATH: "
            + str(path)
        )

    if any(
        "client/launcher-plugin/build" in line
        or "/work/neebles-launcher-plugin-build" in line
        for line in dynamic_lines
    ):
        fail(
            "staged BossEvents ELF retains build-tree "
            "RPATH/RUNPATH: "
            + str(path)
        )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--stage",
        required=True,
        type=Path,
    )
    parser.add_argument(
        "--output",
        required=True,
        type=Path,
    )
    args = parser.parse_args()

    stage = args.stage.resolve()
    output = args.output.resolve()

    if not stage.is_dir():
        fail("launcher plugin stage missing: " + str(stage))

    candidates = [
        path
        for path in stage.rglob("BossEvents")
        if path.is_dir()
    ]

    if len(candidates) != 1:
        fail(
            "expected exactly one BossEvents install directory: "
            + repr([str(path) for path in candidates])
        )

    source = candidates[0]

    missing = [
        name
        for name in REQUIRED
        if not (source / name).is_file()
    ]

    if missing:
        fail(
            "installed BossEvents artifacts missing: "
            + repr(missing)
        )

    for name in ELF_NAMES:
        inspect_elf(source / name)

    if output.exists():
        shutil.rmtree(output)

    output.mkdir(parents=True)

    for name in REQUIRED:
        shutil.copy2(
            source / name,
            output / name,
        )

    print("BOSSEVENTS STAGED ELF CERTIFIED :: 2/2")
    print("BOSSEVENTS RELEASE INPUT :: " + str(output))


if __name__ == "__main__":
    main()

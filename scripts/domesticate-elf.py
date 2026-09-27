#!/usr/bin/env python3

import argparse
import subprocess
from pathlib import Path


FINAL_ROOT = Path("/opt/neebles/client/runtime/boss/rootfs")

FINAL_LOADER = (
    FINAL_ROOT
    / "lib64/ld-linux-x86-64.so.2"
)

FINAL_LIBRARY_DIRS = [
    FINAL_ROOT / "lib/x86_64-linux-gnu",
    FINAL_ROOT / "usr/lib/x86_64-linux-gnu",
    FINAL_ROOT / "usr/lib/x86_64-linux-gnu/systemd",
    FINAL_ROOT / "lib64",
    FINAL_ROOT / "usr/lib64",
]


def run(args):
    subprocess.run(
        args,
        check=True,
    )


def main():
    parser = argparse.ArgumentParser()

    parser.add_argument(
        "elf",
    )

    parser.add_argument(
        "--shared",
        action="store_true",
    )

    parser.add_argument(
        "--origin",
        action="store_true",
    )

    args = parser.parse_args()

    target = Path(args.elf)

    if not target.is_file():
        raise SystemExit(
            "ELF not found: "
            + str(target)
        )

    origin = chr(36) + "ORIGIN"

    rpath_parts = []

    if args.origin:
        rpath_parts.append(
            origin
        )

    rpath_parts.extend(
        str(path)
        for path in FINAL_LIBRARY_DIRS
    )

    rpath = ":".join(
        rpath_parts
    )

    command = [
        "patchelf",
    ]

    if not args.shared:
        command.extend([
            "--set-interpreter",
            str(FINAL_LOADER),
        ])

    command.extend([
        "--force-rpath",
        "--set-rpath",
        rpath,
        str(target),
    ])

    run(
        command
    )

    print(
        "DOMESTICATED ::",
        target
    )

    if not args.shared:
        print(
            "INTERP ::",
            subprocess.check_output(
                [
                    "patchelf",
                    "--print-interpreter",
                    str(target),
                ],
                text=True,
            ).strip()
        )

    print(
        "RPATH ::",
        subprocess.check_output(
            [
                "patchelf",
                "--print-rpath",
                str(target),
            ],
            text=True,
        ).strip()
    )


if __name__ == "__main__":
    main()

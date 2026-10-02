#!/usr/bin/env python3

import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--prefix", default="rootfs")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    manifest_path = Path(args.manifest).resolve()
    prefix = args.prefix.rstrip("/")

    if not root.is_dir():
        raise SystemExit("runtime root does not exist: " + str(root))

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    entries = manifest.get("entries")
    if not isinstance(entries, list):
        raise SystemExit("inventory entries are not a list")

    directories = []
    for item in entries:
        if not isinstance(item, dict) or item.get("type") != "dir":
            continue
        path_name = item.get("path")
        if not isinstance(path_name, str):
            continue
        if path_name == prefix:
            continue
        if not path_name.startswith(prefix + "/"):
            continue
        relative = Path(path_name).relative_to(prefix)
        if relative.is_absolute() or ".." in relative.parts:
            raise SystemExit("unsafe inventory directory: " + path_name)
        directories.append(relative)

    directories.sort(key=lambda value: (len(value.parts), value.as_posix()))

    created = []
    for relative in directories:
        target = root / relative
        if target.exists() or target.is_symlink():
            if not target.is_dir() or target.is_symlink():
                raise SystemExit("inventory directory collides with non-directory: " + relative.as_posix())
            continue

        parent = target.parent
        if not parent.is_dir() or parent.is_symlink():
            raise SystemExit("unsafe parent while rehydrating directory: " + relative.as_posix())

        target.mkdir(mode=0o700)
        created.append(relative.as_posix())

    print("REHYDRATED_EMPTY_DIRECTORIES=" + str(len(created)))


if __name__ == "__main__":
    main()

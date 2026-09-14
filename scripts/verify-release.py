#!/usr/bin/env python3

import argparse
import hashlib
import json
import re
import stat
import tarfile
from pathlib import Path, PurePosixPath

REQUIRED_ASSETS = {
    "backend": "neebles-backend",
    "ui": "neebles-ui",
    "installer": "neebles-installer",
    "tray": "neebles-tray-host",
    "auth_agent": "neebles-auth-agent",
    "client_data": "client-data.tar.gz",
    "install": "install.sh",
}

EXECUTABLE_ASSETS = {
    "backend",
    "ui",
    "installer",
    "tray",
    "auth_agent",
    "install",
}

REQUIRED_CLIENT_DATA_ROOTS = {
    "assets",
    "languages",
    "config",
    "launcher",
    "spacer",
    "notifications",
    "systemd",
    "xdg",
}

REQUIRED_CLIENT_DATA_FILES = {
    "assets/branding/neebles-boss-launcher-icon.png",
    "languages/manifest.json",
    "config/defaults.json",
    "launcher/metadata.json",
    "launcher/contents/ui/main.qml",
    "spacer/metadata.json",
    "spacer/contents/ui/main.qml",
    "systemd/neebles-tray-manager.service",
    "xdg/neebles-tray-host.desktop",
}

SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
SEMVER_RE = re.compile(
    r"^(0|[1-9]\d*)\."
    r"(0|[1-9]\d*)\."
    r"(0|[1-9]\d*)"
    r"(?:-[0-9A-Za-z.-]+)?"
    r"(?:\+[0-9A-Za-z.-]+)?$"
)


def fail(message: str) -> None:
    raise SystemExit(f"RELEASE INVALID: {message}")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()

    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)

    return digest.hexdigest()


def cargo_version(cargo_toml: Path) -> str:
    in_package = False

    for raw_line in cargo_toml.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()

        if line.startswith("[") and line.endswith("]"):
            in_package = line == "[package]"
            continue

        if in_package and line.startswith("version"):
            _, value = line.split("=", 1)
            version = value.strip().strip('"').strip("'")

            if not version:
                fail("Cargo.toml [package].version is empty")

            return version

    fail("could not resolve [package].version from Cargo.toml")


def parse_checksums(path: Path) -> dict[str, str]:
    result: dict[str, str] = {}

    for number, raw_line in enumerate(
        path.read_text(encoding="utf-8").splitlines(),
        start=1,
    ):
        line = raw_line.strip()

        if not line:
            continue

        parts = line.split()

        if len(parts) != 2:
            fail(f"invalid SHA256SUMS line {number}: {raw_line!r}")

        digest, filename = parts

        filename = filename.lstrip("*")

        if not SHA256_RE.fullmatch(digest):
            fail(f"invalid SHA-256 in SHA256SUMS for {filename!r}")

        if Path(filename).name != filename:
            fail(f"SHA256SUMS contains non-basename path: {filename!r}")

        if filename in result:
            fail(f"duplicate SHA256SUMS entry: {filename!r}")

        result[filename] = digest

    return result


def validate_tar(path: Path) -> None:
    try:
        archive = tarfile.open(path, mode="r:gz")
    except (tarfile.TarError, OSError) as exc:
        fail(f"client-data archive cannot be opened: {exc}")

    roots: set[str] = set()
    files: set[str] = set()

    with archive:
        members = archive.getmembers()

        if not members:
            fail("client-data archive is empty")

        for member in members:
            pure = PurePosixPath(member.name)

            if pure.is_absolute():
                fail(f"client-data contains absolute path: {member.name!r}")

            if ".." in pure.parts:
                fail(f"client-data contains parent traversal: {member.name!r}")

            clean_parts = [
                part
                for part in pure.parts
                if part not in ("", ".")
            ]

            if not clean_parts:
                continue

            normalized = "/".join(clean_parts)
            roots.add(clean_parts[0])

            if not (member.isdir() or member.isfile()):
                fail(
                    "client-data contains unsupported filesystem node: "
                    f"{member.name!r}"
                )

            if member.uid != 0 or member.gid != 0:
                fail(
                    "client-data ownership must be root:root for "
                    f"{member.name!r}; got {member.uid}:{member.gid}"
                )

            if member.mtime != 0:
                fail(
                    "client-data mtime must be exactly 0 for "
                    f"{member.name!r}; got {member.mtime}"
                )

            mode = member.mode & 0o777

            if member.isdir():
                if mode != 0o755:
                    fail(
                        "client-data directory mode must be 0755 for "
                        f"{member.name!r}; got {mode:04o}"
                    )
            else:
                if mode != 0o644:
                    fail(
                        "client-data file mode must be 0644 for "
                        f"{member.name!r}; got {mode:04o}"
                    )

                files.add(normalized)

    if roots != REQUIRED_CLIENT_DATA_ROOTS:
        missing = REQUIRED_CLIENT_DATA_ROOTS - roots
        extra = roots - REQUIRED_CLIENT_DATA_ROOTS

        fail(
            "client-data root contract mismatch; "
            f"missing={sorted(missing)}, extra={sorted(extra)}"
        )

    missing_files = REQUIRED_CLIENT_DATA_FILES - files

    if missing_files:
        fail(
            "client-data missing required files: "
            + ", ".join(sorted(missing_files))
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dist", required=True)
    parser.add_argument("--cargo-toml", default="Cargo.toml")
    args = parser.parse_args()

    dist = Path(args.dist).resolve()
    cargo_toml = Path(args.cargo_toml).resolve()

    if not dist.is_dir():
        fail(f"dist directory not found: {dist}")

    bootstrap_path = dist / "bootstrap.json"
    sums_path = dist / "SHA256SUMS"

    if not bootstrap_path.is_file():
        fail("bootstrap.json is missing")

    if not sums_path.is_file():
        fail("SHA256SUMS is missing")

    try:
        bootstrap = json.loads(
            bootstrap_path.read_text(encoding="utf-8")
        )
    except (json.JSONDecodeError, OSError) as exc:
        fail(f"bootstrap.json is invalid: {exc}")

    if bootstrap.get("schema") != 2:
        fail("bootstrap schema must be exactly 2")

    if bootstrap.get("channel") != "stable":
        fail("bootstrap channel must be exactly 'stable'")

    stable = bootstrap.get("stable")

    if not isinstance(stable, dict):
        fail("bootstrap.stable must be an object")

    version = stable.get("version")

    if not isinstance(version, str) or not SEMVER_RE.fullmatch(version):
        fail(f"invalid stable.version: {version!r}")

    source_version = cargo_version(cargo_toml)

    if version != source_version:
        fail(
            f"release version {version!r} does not match "
            f"Cargo.toml version {source_version!r}"
        )

    expected_base_url = (
        "https://github.com/krockzs/neebles-boss/"
        f"releases/download/v{version}"
    )

    if stable.get("base_url") != expected_base_url:
        fail(
            "bootstrap base_url mismatch: "
            f"expected {expected_base_url!r}, "
            f"got {stable.get('base_url')!r}"
        )

    assets = stable.get("assets")

    if not isinstance(assets, dict):
        fail("bootstrap.stable.assets must be an object")

    if set(assets) != set(REQUIRED_ASSETS):
        missing = set(REQUIRED_ASSETS) - set(assets)
        extra = set(assets) - set(REQUIRED_ASSETS)

        fail(
            f"bootstrap asset contract mismatch; "
            f"missing={sorted(missing)}, extra={sorted(extra)}"
        )

    seen_files: set[str] = set()

    for key, expected_file in REQUIRED_ASSETS.items():
        entry = assets[key]

        if not isinstance(entry, dict):
            fail(f"asset {key!r} must be an object")

        filename = entry.get("file")
        digest = entry.get("sha256")

        if filename != expected_file:
            fail(
                f"asset {key!r} must use file {expected_file!r}, "
                f"got {filename!r}"
            )

        if Path(filename).name != filename:
            fail(f"asset {key!r} contains unsafe file path")

        if filename in seen_files:
            fail(f"duplicate release filename: {filename!r}")

        seen_files.add(filename)

        if not isinstance(digest, str) or not SHA256_RE.fullmatch(digest):
            fail(f"asset {key!r} has invalid SHA-256")

        asset_path = dist / filename

        if not asset_path.is_file():
            fail(f"asset {key!r} missing from dist: {filename}")

        actual = sha256(asset_path)

        if actual != digest:
            fail(
                f"asset {key!r} SHA mismatch: "
                f"bootstrap={digest}, actual={actual}"
            )

        if key in EXECUTABLE_ASSETS:
            mode = asset_path.stat().st_mode

            if not mode & stat.S_IXUSR:
                fail(f"asset {key!r} is not executable: {filename}")

    validate_tar(dist / REQUIRED_ASSETS["client_data"])

    checksums = parse_checksums(sums_path)

    expected_sum_files = set(REQUIRED_ASSETS.values()) | {
        "bootstrap.json"
    }

    if set(checksums) != expected_sum_files:
        missing = expected_sum_files - set(checksums)
        extra = set(checksums) - expected_sum_files

        fail(
            f"SHA256SUMS contract mismatch; "
            f"missing={sorted(missing)}, extra={sorted(extra)}"
        )

    for filename, expected_digest in checksums.items():
        target = dist / filename

        if not target.is_file():
            fail(f"SHA256SUMS references missing file: {filename}")

        actual_digest = sha256(target)

        if actual_digest != expected_digest:
            fail(
                f"SHA256SUMS mismatch for {filename}: "
                f"listed={expected_digest}, actual={actual_digest}"
            )

    print("RELEASE PAYLOAD: VALID")
    print(f"VERSION: {version}")
    print(f"SCHEMA: {bootstrap['schema']}")
    print(f"ASSETS: {len(REQUIRED_ASSETS)}")
    print("CLIENT DATA: VALID")
    print("SHA256SUMS: VALID")


if __name__ == "__main__":
    main()

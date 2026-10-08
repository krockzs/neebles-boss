#!/usr/bin/env python3

import argparse
import hashlib
import json
import re
import stat
import subprocess
import tarfile
import tempfile
from pathlib import Path, PurePosixPath

REQUIRED_ASSETS = {
    "backend": "neebles-backend",
    "ui": "neebles-ui",
    "installer": "neebles-installer",
    "auth_agent": "neebles-auth-agent",
    "client_data": "client-data.tar.gz",
    "install": "install.sh",
    "runtime_resolver": "neebles-runtime-resolve",
    "runtime_archive": "boss-runtime.tar.gz",
    "critical_update": "critical-update-manifest.json",
}

EXECUTABLE_ASSETS = {
    "backend",
    "ui",
    "installer",
    "auth_agent",
    "install",
    "runtime_resolver",
}

REQUIRED_CLIENT_DATA_ROOTS = {
    "assets",
    "languages",
    "config",
    "launcher",
    "spacer",
    "notifications",
    "applications",
    "systemd",
    "runtime",
}

REQUIRED_CLIENT_DATA_FILES = {
    "assets/branding/neebles-boss-launcher-icon.png",
    "assets/branding/neebles-boss-icon.png",
    "applications/org.neebles.Boss.desktop",
    "languages/manifest.json",
    "config/defaults.json",
    "launcher/metadata.json",
    "launcher/contents/ui/main.qml",
    "spacer/metadata.json",
    "spacer/contents/ui/main.qml",
    "systemd/neebles-tray-manager.service",
    "systemd/neebles-tray-host.service",
    "systemd/neebles-notification-presenter.service",
    "runtime/tray-host/neebles-tray-host",
    "runtime/qml/NEEBLES/BossEvents/libneebles-launcher-events.so",
    "runtime/qml/NEEBLES/BossEvents/libneebles-launcher-eventsplugin.so",
    "runtime/qml/NEEBLES/BossEvents/neebles-launcher-events.qmltypes",
    "runtime/qml/NEEBLES/BossEvents/qmldir",
}

EXECUTABLE_CLIENT_DATA_FILES = {
    "runtime/tray-host/neebles-tray-host",
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
                expected_mode = (
                    0o755
                    if normalized in EXECUTABLE_CLIENT_DATA_FILES
                    else 0o644
                )

                if mode != expected_mode:
                    fail(
                        "client-data file mode mismatch for "
                        f"{member.name!r}; expected {expected_mode:04o}, "
                        f"got {mode:04o}"
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


def validate_tray_host_rpath(path: Path) -> None:
    member_name = "runtime/tray-host/neebles-tray-host"

    expected_interpreter = (
        "/opt/neebles/client/runtime/boss/rootfs/"
        "lib64/ld-linux-x86-64.so.2"
    )

    required_rpath_parts = (
        "/opt/neebles/client/runtime/boss/rootfs/lib/x86_64-linux-gnu",
        "/opt/neebles/client/runtime/boss/rootfs/usr/lib/x86_64-linux-gnu",
        "/opt/neebles/client/runtime/boss/rootfs/usr/lib/x86_64-linux-gnu/systemd",
        "/opt/neebles/client/runtime/boss/rootfs/lib64",
        "/opt/neebles/client/runtime/boss/rootfs/usr/lib64",
    )

    forbidden = (
        "client/tray-host/build",
        "/work/neebles-tray-host-build",
        "/work/neebles-boss-source",
        "build_sysroot_6.8.2",
    )

    with tempfile.TemporaryDirectory(prefix="neebles-tray-host-") as temp_name:
        temp_root = Path(temp_name)

        try:
            with tarfile.open(path, mode="r:gz") as archive:
                member = archive.getmember(member_name)
                handle = archive.extractfile(member)

                if handle is None:
                    fail(
                        "Tray Host ELF cannot be read from client-data: "
                        + member_name
                    )

                staged = temp_root / "neebles-tray-host"
                staged.write_bytes(handle.read())

                program_headers = subprocess.run(
                    ["readelf", "-l", str(staged)],
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                )

                if program_headers.returncode != 0:
                    fail(
                        "could not inspect Tray Host ELF program headers: "
                        + member_name
                    )

                if expected_interpreter not in program_headers.stdout:
                    fail(
                        "Tray Host ELF interpreter is not domestic: "
                        + member_name
                    )

                dynamic = subprocess.run(
                    ["readelf", "-d", str(staged)],
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                )

                if dynamic.returncode != 0:
                    fail(
                        "could not inspect Tray Host ELF dynamic section: "
                        + member_name
                    )

                dynamic_lines = [
                    line
                    for line in dynamic.stdout.splitlines()
                    if "(RPATH)" in line or "(RUNPATH)" in line
                ]

                if not dynamic_lines:
                    fail(
                        "Tray Host ELF has no domestic RPATH/RUNPATH: "
                        + member_name
                    )

                dynamic_text = "\n".join(dynamic_lines)

                if not all(
                    part in dynamic_text
                    for part in required_rpath_parts
                ):
                    fail(
                        "Tray Host ELF domestic RPATH contract mismatch: "
                        + member_name
                    )

                if any(
                    marker in line
                    for line in dynamic_lines
                    for marker in forbidden
                ):
                    fail(
                        "Tray Host ELF retains build-world "
                        "RPATH/RUNPATH: "
                        + member_name
                    )

        except (tarfile.TarError, KeyError, OSError) as exc:
            fail("Tray Host ELF cannot be inspected: " + str(exc))


def validate_bossevents_rpath(path: Path) -> None:
    member_names = (
        "runtime/qml/NEEBLES/BossEvents/libneebles-launcher-events.so",
        "runtime/qml/NEEBLES/BossEvents/libneebles-launcher-eventsplugin.so",
    )

    with tempfile.TemporaryDirectory(prefix="neebles-bossevents-") as temp_name:
        temp_root = Path(temp_name)

        try:
            with tarfile.open(path, mode="r:gz") as archive:
                for member_name in member_names:
                    member = archive.getmember(member_name)
                    handle = archive.extractfile(member)

                    if handle is None:
                        fail(
                            "BossEvents ELF cannot be read from client-data: "
                            + member_name
                        )

                    staged = temp_root / Path(member_name).name
                    staged.write_bytes(handle.read())

                    dynamic = subprocess.run(
                        ["readelf", "-d", str(staged)],
                        text=True,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.STDOUT,
                    )

                    if dynamic.returncode != 0:
                        fail(
                            "could not inspect BossEvents ELF dynamic section: "
                            + member_name
                        )

                    dynamic_lines = [
                        line
                        for line in dynamic.stdout.splitlines()
                        if "(RPATH)" in line or "(RUNPATH)" in line
                    ]

                    if not any(
                        chr(36) + "ORIGIN" in line
                        for line in dynamic_lines
                    ):
                        fail(
                            "BossEvents ELF must retain ORIGIN-relative "
                            "RPATH/RUNPATH: "
                            + member_name
                        )

                    if any(
                        "client/launcher-plugin/build" in line
                        or "/work/neebles-launcher-plugin-build" in line
                        for line in dynamic_lines
                    ):
                        fail(
                            "BossEvents ELF retains build-tree "
                            "RPATH/RUNPATH: "
                            + member_name
                        )
        except (tarfile.TarError, KeyError, OSError) as exc:
            fail("BossEvents ELF cannot be inspected: " + str(exc))

def validate_critical_update_manifest(
    path: Path,
) -> list[str]:
    try:
        payload = path.read_bytes()
    except OSError as exc:
        fail(
            "Critical Update manifest cannot be read: "
            + str(exc)
        )

    if not payload:
        return []

    try:
        value = json.loads(
            payload.decode("utf-8")
        )
    except (
        UnicodeDecodeError,
        json.JSONDecodeError,
    ) as exc:
        fail(
            "Critical Update manifest is invalid: "
            + str(exc)
        )

    if not isinstance(value, list):
        fail(
            "Critical Update manifest must be an array of strings"
        )

    if not all(
        isinstance(item, str)
        for item in value
    ):
        fail(
            "Critical Update manifest entries must be strings"
        )

    return value


def validate_boss_qt_domestic_elf(path: Path) -> None:
    name = path.name
    if name not in {'neebles-ui', 'neebles-installer', 'neebles-auth-agent'}:
        fail(f'unknown domestic Boss Qt ELF: {name}')
    if not path.is_file():
        fail(f'missing domestic Boss Qt ELF: {path}')
    try:
        with path.open('rb') as handle:
            if handle.read(4) != b'\x7fELF':
                fail(f'not a domestic Boss Qt ELF: {path}')
    except OSError as exc:
        fail(f'could not open domestic Boss Qt ELF {path}: {exc}')
    def inspection(arguments):
        result = subprocess.run(arguments, text=True, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT)
        if result.returncode:
            fail(f'could not inspect domestic Boss Qt ELF {name}: {result.stdout}')
        return result.stdout
    interpreter = inspection(['readelf', '-l', str(path)])
    expected = '/opt/neebles/client/runtime/boss/rootfs/lib64/ld-linux-x86-64.so.2'
    if expected not in interpreter:
        fail(f'{name}: missing domestic ELF interpreter')
    dynamic = inspection(['readelf', '-d', str(path)])
    if 'Shared library: [libQt6Core.so.6]' not in dynamic:
        fail(f'{name}: missing Qt 6 NEEDED library')
    rpath = '\n'.join(line for line in dynamic.splitlines()
                      if '(RPATH)' in line or '(RUNPATH)' in line)
    required_paths = (
        '/opt/neebles/client/runtime/boss/rootfs/lib/x86_64-linux-gnu',
        '/opt/neebles/client/runtime/boss/rootfs/usr/lib/x86_64-linux-gnu',
    )
    if not rpath or any(required not in rpath for required in required_paths):
        fail(f'{name}: missing domestic RPATH/RUNPATH')
    for forbidden in ('/work/neebles-', 'neebles-boss-source', 'build_sysroot_6.8.2'):
        if forbidden in rpath:
            fail(f'{name}: build tree path leaked into domestic RPATH')
    version_info = inspection(['readelf', '--version-info', str(path)])
    if 'Qt_6.10' in version_info or 'Qt_6.9' in version_info:
        fail(f'{name}: forbidden Qt versions found')
    print('DOMESTIC BOSS QT ELF: VALID', name)

def validate_runtime_archive(path: Path) -> None:
    try:
        archive = tarfile.open(path, mode="r:gz")
    except (tarfile.TarError, OSError) as exc:
        fail(f"domestic runtime archive cannot be opened: {exc}")

    manifest_bytes = None
    root_seen = False

    with archive:
        members = archive.getmembers()

        if not members:
            fail("domestic runtime archive is empty")

        for member in members:
            pure = PurePosixPath(member.name)

            if pure.is_absolute() or ".." in pure.parts:
                fail(
                    "domestic runtime archive contains unsafe path: "
                    + repr(member.name)
                )

            normalized = "/".join(
                part
                for part in pure.parts
                if part not in ("", ".")
            )

            if normalized == "rootfs" or normalized.startswith("rootfs/"):
                root_seen = True

            if normalized == "domestic-runtime.json":
                if not member.isfile():
                    fail("domestic runtime manifest is not a regular file")

                handle = archive.extractfile(member)

                if handle is None:
                    fail("domestic runtime manifest cannot be read")

                manifest_bytes = handle.read()

    if not root_seen:
        fail("domestic runtime archive does not contain rootfs")

    if manifest_bytes is None:
        fail(
            "domestic runtime archive does not contain domestic-runtime.json"
        )

    try:
        manifest = json.loads(
            manifest_bytes.decode("utf-8")
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        fail(
            "domestic runtime manifest is invalid: "
            + str(exc)
        )

    if manifest.get("schema") != "1":
        fail('domestic runtime manifest schema must be exactly "1"')

    if manifest.get("name") != "neebles-domestic-runtime":
        fail("domestic runtime manifest identity mismatch")

    if manifest.get("root") != "rootfs":
        fail("domestic runtime manifest root must be exactly rootfs")


def validate_runtime_resolver(path: Path) -> None:
    program_headers = subprocess.run(
        ["readelf", "-l", str(path)],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )

    if program_headers.returncode != 0:
        fail(
            "could not inspect bootstrap runtime resolver program headers"
        )

    if " INTERP " in program_headers.stdout:
        fail(
            "bootstrap runtime resolver must not require an ELF interpreter"
        )

    dynamic = subprocess.run(
        ["readelf", "-d", str(path)],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )

    if dynamic.returncode != 0:
        fail(
            "could not inspect bootstrap runtime resolver dynamic section"
        )

    if "(NEEDED)" in dynamic.stdout:
        fail(
            "bootstrap runtime resolver must not require shared libraries"
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

    for name in ('neebles-ui', 'neebles-installer', 'neebles-auth-agent'):
        validate_boss_qt_domestic_elf(dist / name)
    validate_tar(dist / REQUIRED_ASSETS["client_data"])
    validate_tray_host_rpath(
        dist / REQUIRED_ASSETS["client_data"]
    )
    validate_bossevents_rpath(
        dist / REQUIRED_ASSETS["client_data"]
    )

    validate_critical_update_manifest(
        dist / REQUIRED_ASSETS["critical_update"]
    )

    validate_runtime_archive(
        dist / REQUIRED_ASSETS["runtime_archive"]
    )
    validate_runtime_resolver(
        dist / REQUIRED_ASSETS["runtime_resolver"]
    )

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

#!/usr/bin/env python3

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path


import hashlib
import json
import tomllib

ROOT = Path(__file__).resolve().parent.parent

DOMESTICATOR = (
    ROOT
    / "scripts"
    / "domesticate-elf.py"
)

BUILD_CLIENT_DATA = (
    ROOT
    / "scripts"
    / "build-client-data.sh"
)


def require_file(path, label):
    path = Path(path).resolve()

    if not path.is_file():
        raise SystemExit(
            label
            + " missing: "
            + str(path)
        )

    return path


def run(args):
    subprocess.run(
        [str(item) for item in args],
        check=True,
    )


def domesticate(source, destination):
    shutil.copy2(
        source,
        destination,
    )

    destination.chmod(0o755)

    run([
        "python3",
        DOMESTICATOR,
        destination,
    ])



RELEASE_ASSETS = {
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


def sha256(path):
    digest = hashlib.sha256()

    with path.open("rb") as handle:
        for chunk in iter(
            lambda: handle.read(1024 * 1024),
            b"",
        ):
            digest.update(chunk)

    return digest.hexdigest()


def cargo_version():
    cargo_toml = ROOT / "Cargo.toml"

    data = tomllib.loads(
        cargo_toml.read_text(
            encoding="utf-8"
        )
    )

    package = data.get("package")

    if not isinstance(package, dict):
        raise RuntimeError(
            "Cargo.toml package table is missing"
        )

    version = package.get("version")

    if not isinstance(version, str) or not version:
        raise RuntimeError(
            "Cargo.toml package version is invalid"
        )

    return version


def write_release_metadata(dist):
    version = cargo_version()

    assets = {}

    for key, filename in RELEASE_ASSETS.items():
        path = dist / filename

        if not path.is_file():
            raise RuntimeError(
                "release asset is missing before metadata: "
                + filename
            )

        assets[key] = {
            "file": filename,
            "sha256": sha256(path),
        }

    bootstrap = {
        "schema": 2,
        "channel": "stable",
        "stable": {
            "version": version,
            "base_url": (
                "https://github.com/krockzs/neebles-boss/"
                + "releases/download/v"
                + version
            ),
            "assets": assets,
        },
    }

    bootstrap_path = dist / "bootstrap.json"

    bootstrap_path.write_text(
        json.dumps(
            bootstrap,
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )

    checksum_names = [
        *RELEASE_ASSETS.values(),
        "bootstrap.json",
    ]

    sums_path = dist / "SHA256SUMS"

    sums_path.write_text(
        "".join(
            sha256(dist / filename)
            + "  "
            + filename
            + "\n"
            for filename in checksum_names
        ),
        encoding="utf-8",
    )


def main():
    parser = argparse.ArgumentParser()

    parser.add_argument(
        "--dist",
        required=True,
    )

    parser.add_argument(
        "--backend",
        required=True,
    )

    parser.add_argument(
        "--ui",
        required=True,
    )

    parser.add_argument(
        "--installer",
        required=True,
    )

    parser.add_argument(
        "--auth-agent",
        required=True,
    )

    parser.add_argument(
        "--tray-host",
        required=True,
    )

    parser.add_argument(
        "--launcher-plugin-build",
        required=True,
    )

    parser.add_argument(
        "--client-source",
        default="client",
    )

    parser.add_argument(
        "--runtime-resolver",
        required=True,
    )

    parser.add_argument(
        "--runtime-archive",
        required=True,
    )

    args = parser.parse_args()

    dist = Path(args.dist).resolve()
    dist.mkdir(
        parents=True,
        exist_ok=True,
    )

    backend = require_file(
        args.backend,
        "Backend RAW ELF",
    )

    ui = require_file(
        args.ui,
        "UI RAW ELF",
    )

    installer = require_file(
        args.installer,
        "Installer RAW ELF",
    )

    auth = require_file(
        args.auth_agent,
        "Auth Agent RAW ELF",
    )

    tray = require_file(
        args.tray_host,
        "Tray Host RAW ELF",
    )

    launcher = Path(
        args.launcher_plugin_build
    ).resolve()

    client = Path(
        args.client_source
    ).resolve()

    runtime_resolver = require_file(
        args.runtime_resolver,
        "Bootstrap runtime resolver",
    )

    runtime_archive = require_file(
        args.runtime_archive,
        "Domestic runtime archive",
    )

    critical_update_manifest = require_file(
        ROOT
        / "critical-update"
        / "manifest.json",
        "Critical Update manifest",
    )

    if not launcher.is_dir():
        raise SystemExit(
            "Launcher plugin build missing: "
            + str(launcher)
        )

    if not client.is_dir():
        raise SystemExit(
            "Client source missing: "
            + str(client)
        )

    with tempfile.TemporaryDirectory(
        prefix="neebles-materialize-"
    ) as temp_name:
        temp = Path(temp_name)

        staged_tray = (
            temp
            / "neebles-tray-host"
        )

        domesticate(
            backend,
            dist / "neebles-backend",
        )

        domesticate(
            ui,
            dist / "neebles-ui",
        )

        domesticate(
            installer,
            dist / "neebles-installer",
        )

        domesticate(
            auth,
            dist / "neebles-auth-agent",
        )

        shutil.copy2(
            runtime_resolver,
            dist / "neebles-runtime-resolve",
        )

        (
            dist
            / "neebles-runtime-resolve"
        ).chmod(0o755)

        shutil.copy2(
            runtime_archive,
            dist / "boss-runtime.tar.gz",
        )

        shutil.copy2(
            critical_update_manifest,
            dist / "critical-update-manifest.json",
        )

        domesticate(
            tray,
            staged_tray,
        )

        shutil.copy2(
            ROOT
            / "scripts"
            / "install.sh",
            dist / "install.sh",
        )

        (
            dist
            / "install.sh"
        ).chmod(0o755)

        run([
            BUILD_CLIENT_DATA,
            dist
            / "client-data.tar.gz",
            client,
            launcher,
            staged_tray,
        ])

    write_release_metadata(dist)

    print(
        "MATERIALIZED RELEASE PAYLOAD ::",
        dist,
    )


if __name__ == "__main__":
    main()

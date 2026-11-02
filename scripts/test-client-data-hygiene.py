#!/usr/bin/env python3
"""Regression: client-data excludes transient Python build/test artifacts.

This test creates a minimal isolated client tree. It does not install the OS,
write to the repository, or require root/Qt/Esbirro binaries.
"""
from __future__ import annotations

import hashlib
from pathlib import Path
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "scripts/build-client-data.sh"
REQUIRED = (
    "handoff", "assets", "languages", "config", "launcher", "spacer",
    "notifications", "release-notes", "applications", "systemd",
    "platform-operations",
)
PLUGINS = (
    "libneebles-launcher-events.so",
    "libneebles-launcher-eventsplugin.so",
    "neebles-launcher-events.qmltypes",
    "qmldir",
)


def check() -> None:
    with tempfile.TemporaryDirectory(prefix="neebles-f25-client-data-") as scratch:
        root = Path(scratch)
        source = root / "client"
        plugin = root / "plugin"
        tray = root / "neebles-tray-host"
        for name in REQUIRED:
            (source / name).mkdir(parents=True, exist_ok=True)
        plugin.mkdir()
        for name in PLUGINS:
            (plugin / name).write_bytes(b"fixture\n")
        tray.write_bytes(b"fixture tray\n")
        tray.chmod(0o755)
        (source / "platform-operations" / "platform_provider.py").write_text("# fixture\n")
        (source / "release-notes" / "es_cl_2_0_12.txt").write_text("2.0.12\n")
        (source / "release-notes" / "en_us_2_0_12.txt").write_text("2.0.12\n")
        (source / "handoff" / "handoff.py").write_text("# fixture\n")
        for path in (
            source / "platform-operations" / "__pycache__" / "provider.cpython-313.pyc",
            source / "handoff" / "__pycache__" / "handoff.cpython-313.pyc",
            source / "handoff" / "legacy.pyo",
        ):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"NONRELEASE ARTIFACT")

        archives = [root / "client-a.tar.gz", root / "client-b.tar.gz"]
        for output in archives:
            subprocess.run(
                ["bash", str(BUILD), str(output), str(source), str(plugin), str(tray)],
                cwd=ROOT, check=True, text=True, capture_output=True,
            )
        a = hashlib.sha256(archives[0].read_bytes()).hexdigest()
        b = hashlib.sha256(archives[1].read_bytes()).hexdigest()
        if a != b:
            raise AssertionError("client-data build is not reproducible")

        with tarfile.open(archives[0], "r:gz") as archive:
            names = {m.name.rstrip("/") for m in archive.getmembers()}
        if any("__pycache__" in x.split("/") or x.endswith((".pyc", ".pyo")) for x in names):
            raise AssertionError("Python bytecode or __pycache__ leaked into client-data")
        for expected in (
            "platform-operations/platform_provider.py",
            "release-notes/es_cl_2_0_12.txt",
            "release-notes/en_us_2_0_12.txt",
            "runtime/tray-host/neebles-tray-host",
            "runtime/qml/NEEBLES/BossEvents/qmldir",
        ):
            if expected not in names:
                raise AssertionError("Required output missing: " + expected)
        print("PASS: reproducible client-data; Python caches excluded; 2.0.12 notes present")
        print("SHA256 fixture:", a)


if __name__ == "__main__":
    check()

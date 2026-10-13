#!/usr/bin/env python3
"""Independent Boss upgrade -> 1.0.43 desktop handoff; runs as desktop user.

This is intentionally not called from Boss 1.0.35. Fail closed on identity
mismatch or unavailable Plasma systemd user unit. No broad process matching.
"""
import json
import os
import select
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

TARGET = "1.0.52"
BUS_NAME = "org.neebles.Boss"
UI_PATH = "/opt/neebles/client/ui/neebles-ui"
BOSS_PATH = "/opt/neebles/client/bin/neebles"


def event(message):
    print(f"HANDOFF: {message}", flush=True)


def run(args, timeout=8):
    return subprocess.run(args, capture_output=True, text=True, timeout=timeout, check=False)


# NEEBLES_1036_HANDOFF_HARDEN_24
# Distinguish a legitimately absent service from a broken D-Bus request.
def owner_pid(bus_path):
    address = "--address=unix:path=" + bus_path
    probe = run([
        "/usr/bin/busctl", address, "--no-pager", "call",
        "org.freedesktop.DBus", "/org/freedesktop/DBus",
        "org.freedesktop.DBus", "NameHasOwner", "s", BUS_NAME,
    ])
    if probe.returncode != 0:
        raise RuntimeError("D-Bus ownership probe failed: " + probe.stderr.strip())
    response = probe.stdout.strip()
    if response == "b false":
        return None
    if response != "b true":
        raise RuntimeError("Unexpected D-Bus ownership response: " + response)
    result = run([
        "/usr/bin/busctl", address, "--no-pager", "status", BUS_NAME,
    ])
    if result.returncode != 0:
        raise RuntimeError("D-Bus owner exists but PID query failed: " + result.stderr.strip())
    m = re.search(r"(?m)^\s*PID\s*[:=]\s*(\d+)\s*$", result.stdout)
    if not m:
        raise RuntimeError("D-Bus owner exists without a verifiable PID")
    return int(m.group(1))



# NEEBLES_1036_HANDOFF_BACKUP_PATH_30
# install.sh renames /opt/neebles/client to a unique client-backup.*
# under /opt/neebles/shared/tmp, then deletes that backup after commit.
# The OLD running ELF therefore retains the deleted backup location in /proc.
# New Boss binary at /opt/neebles/client/ui/neebles-ui is never eligible.
def retired_boss_executable(executable):
    if executable == UI_PATH + " (deleted)":
        return True
    return bool(re.fullmatch(
        r"/opt/neebles/shared/tmp/client-backup\.[A-Za-z0-9]{6}/ui/neebles-ui \(deleted\)",
        executable,
    ))

def validated_owner(pid, uid):
    proc = Path(f"/proc/{pid}")
    status = (proc / "status").read_text()
    match = re.search(r"(?m)^Uid:\s*(\d+)\s", status)
    if not match or int(match.group(1)) != uid:
        raise RuntimeError("Boss D-Bus PID does not belong to desktop user")
    executable = os.readlink(proc / "exe")
    if not retired_boss_executable(executable):
        raise RuntimeError("D-Bus owner executable is not the retired Boss UI: " + executable)
    # PID reuse defense; caller also holds pidfd during the signal.
    return (pid, int((proc / "stat").read_text().rsplit(") ", 1)[1].split()[19]))


# NEEBLES_1036_HANDOFF_SESSION_FIX_32
# Read graphical display authority from the authenticated user manager;
# never read arbitrary shell files, or copy the installer's root environment.
def restore_graphical_environment():
    result = run(["/usr/bin/systemctl", "--user", "show-environment"])
    if result.returncode:
        raise RuntimeError("Could not read authenticated user manager environment")
    allowed = {"DISPLAY", "XAUTHORITY", "WAYLAND_DISPLAY", "XDG_SESSION_TYPE", "QT_QPA_PLATFORM"}
    values = {}
    for line in result.stdout.splitlines():
        key, delimiter, value = line.partition("=")
        if delimiter and key in allowed:
            if "\x00" in value or "\n" in value or len(value) > 4096:
                raise RuntimeError("Malformed graphical environment value")
            values[key] = value
    kind = values.get("XDG_SESSION_TYPE", "").lower()
    if kind == "x11" or (not kind and values.get("DISPLAY")):
        if not re.fullmatch(r":[0-9]+(?:\.[0-9]+)?", values.get("DISPLAY", "")):
            raise RuntimeError("Missing or unsupported X11 DISPLAY from user manager")
        # KDE can use default ~/.Xauthority when XAUTHORITY is unset.
        authority = values.get("XAUTHORITY", "")
        if authority and (not os.path.isabs(authority) or not Path(authority).is_file()):
            raise RuntimeError("Unavailable X11 authority file")
    elif kind == "wayland":
        if not re.fullmatch(r"wayland-[A-Za-z0-9_-]+", values.get("WAYLAND_DISPLAY", "")):
            raise RuntimeError("Missing Wayland display from user manager")
    else:
        raise RuntimeError("User manager does not expose a supported desktop session")
    os.environ.update(values)
    event("Restored display environment from authenticated user manager")


def main():
    if os.geteuid() == 0:
        raise RuntimeError("Handoff must never run as root")
    home = Path.home()
    logfile = home / ".config/neebles/boss-handoff.log"
    if logfile.is_symlink():
        raise RuntimeError("Refusing symlinked handoff log")
    logfd = os.open(logfile, os.O_CREAT | os.O_APPEND | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
    if os.fstat(logfd).st_uid != os.getuid():
        os.close(logfd)
        raise RuntimeError("Handoff log is not desktop-user owned")
    logstream = os.fdopen(logfd, "a", buffering=1)
    sys.stdout = logstream
    sys.stderr = logstream
    record = home / ".config/neebles/boss-update-pending.json"
    if record.is_symlink() or not record.is_file() or record.stat().st_uid != os.getuid():
        raise RuntimeError("Owned upgrade receipt missing")
    data = json.loads(record.read_text(encoding="utf-8"))
    if (data.get("schema") != 1 or data.get("target") != "boss"
        or data.get("to_version") != TARGET
        or not re.fullmatch(r"\d+\.\d+\.\d+", str(data.get("from_version", "")))
        or data["from_version"] == TARGET):
        raise RuntimeError("Receipt does not authorize an actual target-version update")
    installed = run([BOSS_PATH, "--version"])
    if installed.returncode or installed.stdout.strip().split()[-1] != TARGET:
        raise RuntimeError("New Boss binary does not report target version")
    runtime = Path(f"/run/user/{os.getuid()}")
    bus_path = str(runtime / "bus")
    if runtime.is_symlink() or runtime.stat().st_uid != os.getuid():
        raise RuntimeError("Unauthenticated desktop runtime")
    if not (runtime / "bus").is_socket():
        raise RuntimeError("Session bus socket missing")
    os.environ["XDG_RUNTIME_DIR"] = str(runtime)
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = "unix:path=" + bus_path

    # NEEBLES_1036_HANDOFF_INSTALLER_BARRIER_25
    # Never signal old UI while install.sh is still executing. The previous
    # six-second sleep was only a guess and could race the old updater.
    installer_pid_text = os.environ.get("NEEBLES_INSTALLER_PID", "")
    installer_start_text = os.environ.get("NEEBLES_INSTALLER_START_TICKS", "")
    if not (re.fullmatch(r"[1-9][0-9]*", installer_pid_text)
            and re.fullmatch(r"[0-9]+", installer_start_text)):
        raise RuntimeError("Trusted installer lifetime identity missing")
    installer_pid = int(installer_pid_text)
    installer_start = int(installer_start_text)
    if installer_pid == os.getpid():
        raise RuntimeError("Installer PID cannot equal helper PID")

    def installer_still_running():
        stat_path = Path(f"/proc/{installer_pid}/stat")
        try:
            raw = stat_path.read_text()
        except FileNotFoundError:
            return False
        # /proc stat comm may contain spaces or parentheses; split from last ') '.
        fields = raw.rsplit(") ", 1)
        if len(fields) != 2:
            raise RuntimeError("Installer process stat malformed")
        details = fields[1].split()
        if len(details) < 20:
            raise RuntimeError("Installer process stat missing starttime")
        if int(details[19]) != installer_start:
            return False
        # A zombie is no longer executing, even if not yet reaped.
        return details[0] not in ("Z", "X", "x")

    deadline = time.monotonic() + 120
    while installer_still_running():
        if time.monotonic() >= deadline:
            raise RuntimeError("Installer did not finish within 120 seconds; no UI termination")
        time.sleep(0.2)
    event("Observed installer process exit (exit status not independently known)")
    # Allow the previous updater event loop to consume the installer result.
    # This is a grace period, not claimed as backend acknowledgement.
    time.sleep(2)
    # Validate display before ending the last usable Boss UI.
    restore_graphical_environment()
    # A hard safety gate prevents a regression to the old run-p*.service.
    helper_cgroup = Path("/proc/self/cgroup").read_text(encoding="utf-8")
    # Match the exact versioned transient unit, not a hardcoded prior release.
    # Prefixes/suffixes must not authorize a different or untrusted cgroup.
    handoff_unit = f"neebles-boss-upgrade-handoff-{TARGET.replace('.', '-')}.service"
    cgroup_paths = [line.partition('::')[2] for line in helper_cgroup.splitlines() if '::' in line]
    if not any(path.endswith('/' + handoff_unit) or ('/' + handoff_unit + '/') in path
               for path in cgroup_paths):
        raise RuntimeError("Handoff is not isolated in its dedicated user service")
    event("Verified isolated systemd user-service cgroup")
    old = owner_pid(bus_path)
    if old is None:
        event("No prior UI owner; no unrelated process terminated")
    else:
        if not hasattr(os, "pidfd_open") or not hasattr(signal, "pidfd_send_signal"):
            raise RuntimeError("Kernel/Python pidfd signal support is required for safe UI retirement")
        process_fd = os.pidfd_open(old, 0)
        try:
            identity = validated_owner(old, os.getuid())
            if owner_pid(bus_path) != old or validated_owner(old, os.getuid()) != identity:
                raise RuntimeError("Boss UI owner identity changed before retirement")
            signal.pidfd_send_signal(process_fd, signal.SIGTERM, None, 0)
            event(f"SIGTERM sent to verified retired Boss UI PID={old} via pidfd")
            event("Waiting for old Boss UI process exit via pidfd")
            # pidfd becomes readable only when the exact old process terminates.
            poller = select.poll()
            poller.register(process_fd, select.POLLIN)
            if not poller.poll(24000):
                raise RuntimeError("Old Boss UI did not terminate within 24 seconds")
            event("Verified retired Boss UI process has exited")
            # Process exit closes its held QLocalSocket. Separately require
            # org.neebles.Boss to be free, not transferred to a new owner.
            for _ in range(20):
                observed = owner_pid(bus_path)
                if observed is None:
                    break
                if observed != old:
                    raise RuntimeError("Different owner acquired Boss singleton during retirement")
                time.sleep(0.1)
            else:
                raise RuntimeError("Old Boss D-Bus name was not released")
        finally:
            os.close(process_fd)
    # 1.0.42 selective desktop refresh: no plasmashell restart.
    # This version intentionally retains unchanged BossEvents native libraries.
    event("Preparing selective NEEBLES surface refresh")
    check = run(["/usr/bin/systemctl", "--user", "is-active", "--quiet",
                 "plasma-plasmashell.service"])
    if check.returncode:
        raise RuntimeError("Plasma is not active; refusing surface replacement")

    # Module trays require infrastructure even if settings.tray.enabled is false.
    # Stop dependents first; start manager before its dependent hosts.
    tray_units = (
        "neebles-tray-host.service",
        "neebles-tray-sni-host.service",
        "neebles-tray-manager.service",
    )
    for unit in tray_units:
        result = run(["/usr/bin/systemctl", "--user", "stop", unit], timeout=25)
        if result.returncode:
            raise RuntimeError("Could not stop " + unit + ": " + result.stderr.strip())
        event("Stopped " + unit)
    for unit in reversed(tray_units):
        result = run(["/usr/bin/systemctl", "--user", "start", unit], timeout=25)
        if result.returncode:
            raise RuntimeError("Could not start " + unit + ": " + result.stderr.strip())
        active = run(["/usr/bin/systemctl", "--user", "is-active", "--quiet", unit])
        if active.returncode:
            raise RuntimeError("Tray service did not stay active: " + unit)
        event("Started and verified " + unit)

    # 1.0.43: never delete/recreate live Plasma applets inside evaluateScript.
    # Qt/Plasma asynchronously destroy widgets; replacing them during update
    # produced an applet-destructor heap corruption crash on 1.0.42.
    # Existing Launcher/Spacer keep their exact placement and configuration.
    # Updated QML is loaded by Plasma at the next graphical session startup.
    event("Preserved applet IDs; 1.0.48+ surfaces poll revised QML and reload visual children")
    event("Selective refresh complete; starting new Boss entrypoint")
    launch = subprocess.Popen([BOSS_PATH, "start"], stdin=subprocess.DEVNULL,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                              start_new_session=True, close_fds=True)
    event(f"Launched new Boss entrypoint PID={launch.pid}; receipt left for UI acknowledgment")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        event("FAILED: " + str(exc))
        sys.exit(1)

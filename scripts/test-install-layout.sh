#!/bin/bash
set -euo pipefail

REPO_ROOT="$(
    cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." &&
    pwd
)"

cd "$REPO_ROOT"

BACKEND="$REPO_ROOT/target/debug/neebles-backend"
UI="$REPO_ROOT/ui/client/build/neebles-ui"
AUTH="$REPO_ROOT/ui/auth-agent/build/neebles-auth-agent"

for binary in "$BACKEND" "$UI" "$AUTH"; do
    [[ -f "$binary" ]] || {
        echo "PACKAGING TEST INVALID: missing built binary: $binary" >&2
        echo "Run ./scripts/build-all.sh first." >&2
        exit 1
    }
done

ROOT="$(mktemp -d)"
ARCHIVE="$ROOT/client-data.tar.gz"
ARCHIVE_SECOND="$ROOT/client-data-second.tar.gz"

cleanup() {
    rm -rf "$ROOT"
}

trap cleanup EXIT

"$REPO_ROOT/scripts/build-client-data.sh" \
    "$ARCHIVE" \
    "$REPO_ROOT/client"

"$REPO_ROOT/scripts/build-client-data.sh" \
    "$ARCHIVE_SECOND" \
    "$REPO_ROOT/client"

cmp -s "$ARCHIVE" "$ARCHIVE_SECOND" || {
    echo "PACKAGING TEST INVALID: client-data archive is not reproducible" >&2
    exit 1
}

echo "REPRODUCIBLE CLIENT DATA: VALID"

run_install_archive() {
    DESTDIR="$ROOT" \
        "$REPO_ROOT/scripts/install.sh" \
        "$BACKEND" \
        "$UI" \
        "$ARCHIVE" \
        "$AUTH"
}

run_install_directory() {
    DESTDIR="$ROOT" \
        "$REPO_ROOT/scripts/install.sh" \
        "$BACKEND" \
        "$UI" \
        "$REPO_ROOT/client" \
        "$AUTH"
}

assert_file() {
    [[ -f "$1" ]] || {
        echo "PACKAGING TEST INVALID: expected file missing: $1" >&2
        exit 1
    }
}

assert_dir() {
    [[ -d "$1" ]] || {
        echo "PACKAGING TEST INVALID: expected directory missing: $1" >&2
        exit 1
    }
}

assert_mode() {
    local expected="$1"
    local path="$2"
    local actual

    actual="$(stat -c '%a' "$path")"

    [[ "$actual" == "$expected" ]] || {
        echo "PACKAGING TEST INVALID: mode $actual != $expected for $path" >&2
        exit 1
    }
}

echo "=== CLEAN INSTALL FROM TAR ==="

run_install_archive

CLIENT="$ROOT/opt/neebles/client"

assert_file "$CLIENT/backend/neebles-backend"
assert_file "$CLIENT/ui/neebles-ui"
assert_file "$CLIENT/auth/neebles-auth-agent"
assert_file "$CLIENT/languages/manifest.json"
assert_file "$CLIENT/config/defaults.json"

assert_file "$ROOT/usr/lib/systemd/user/neebles-tray-manager.service"

assert_dir "$ROOT/usr/share/plasma/plasmoids/org.neebles.launcher"
assert_dir "$ROOT/usr/share/plasma/plasmoids/org.neebles.spacer"

[[ -L "$CLIENT/bin/neebles" ]] || {
    echo "PACKAGING TEST INVALID: client/bin/neebles is not a symlink" >&2
    exit 1
}

[[ "$(readlink "$CLIENT/bin/neebles")" == "../backend/neebles-backend" ]] || {
    echo "PACKAGING TEST INVALID: internal Boss symlink target is wrong" >&2
    exit 1
}

[[ -L "$ROOT/usr/local/bin/neebles" ]] || {
    echo "PACKAGING TEST INVALID: global neebles command is not a symlink" >&2
    exit 1
}

[[ "$(readlink "$ROOT/usr/local/bin/neebles")" == "/opt/neebles/client/bin/neebles" ]] || {
    echo "PACKAGING TEST INVALID: global Boss symlink target is wrong" >&2
    exit 1
}

[[ -L "$ROOT/etc/systemd/user/default.target.wants/neebles-tray-manager.service" ]] || {
    echo "PACKAGING TEST INVALID: Tray Manager enable symlink missing" >&2
    exit 1
}

[[ "$(readlink "$ROOT/etc/systemd/user/default.target.wants/neebles-tray-manager.service")" == "/usr/lib/systemd/user/neebles-tray-manager.service" ]] || {
    echo "PACKAGING TEST INVALID: Tray Manager enable symlink target is wrong" >&2
    exit 1
}

assert_mode 755 "$CLIENT/backend/neebles-backend"
assert_mode 755 "$CLIENT/ui/neebles-ui"
assert_mode 755 "$CLIENT/auth/neebles-auth-agent"

for tree in \
    "$CLIENT/assets" \
    "$CLIENT/languages" \
    "$CLIENT/config" \
    "$CLIENT/launcher" \
    "$CLIENT/spacer" \
    "$CLIENT/notifications"
do
    BAD_DIR="$(find "$tree" -type d ! -perm 0755 -print -quit)"

    [[ -z "$BAD_DIR" ]] || {
        echo "PACKAGING TEST INVALID: non-0755 directory: $BAD_DIR" >&2
        exit 1
    }

    BAD_FILE="$(find "$tree" -type f ! -perm 0644 -print -quit)"

    [[ -z "$BAD_FILE" ]] || {
        echo "PACKAGING TEST INVALID: non-0644 data file: $BAD_FILE" >&2
        exit 1
    }
done

echo "=== REINSTALL CLEANLINESS ==="

touch "$CLIENT/assets/obsolete-from-old-release.txt"
touch "$ROOT/usr/share/plasma/plasmoids/org.neebles.launcher/obsolete-from-old-release.txt"
touch "$ROOT/usr/share/plasma/plasmoids/org.neebles.spacer/obsolete-from-old-release.txt"

touch "$ROOT/opt/neebles/modules/must-survive-reinstall"
touch "$ROOT/opt/neebles/shared/must-survive-reinstall"

run_install_directory

[[ ! -e "$CLIENT/assets/obsolete-from-old-release.txt" ]] || {
    echo "PACKAGING TEST INVALID: stale client-data survived reinstall" >&2
    exit 1
}

[[ ! -e "$ROOT/usr/share/plasma/plasmoids/org.neebles.launcher/obsolete-from-old-release.txt" ]] || {
    echo "PACKAGING TEST INVALID: stale launcher file survived reinstall" >&2
    exit 1
}

[[ ! -e "$ROOT/usr/share/plasma/plasmoids/org.neebles.spacer/obsolete-from-old-release.txt" ]] || {
    echo "PACKAGING TEST INVALID: stale spacer file survived reinstall" >&2
    exit 1
}

[[ -f "$ROOT/opt/neebles/modules/must-survive-reinstall" ]] || {
    echo "PACKAGING TEST INVALID: modules tree was destroyed by reinstall" >&2
    exit 1
}

[[ -f "$ROOT/opt/neebles/shared/must-survive-reinstall" ]] || {
    echo "PACKAGING TEST INVALID: shared tree was destroyed by reinstall" >&2
    exit 1
}

cmp -s \
    client/systemd/neebles-tray-manager.service \
    "$ROOT/usr/lib/systemd/user/neebles-tray-manager.service" \
    || {
        echo "PACKAGING TEST INVALID: installed systemd unit differs from source" >&2
        exit 1
    }

cmp -s \
    || {
        echo "PACKAGING TEST INVALID: installed XDG autostart differs from source" >&2
        exit 1
    }

echo "=== ROLLBACK AFTER GLOBAL MUTATION ==="

printf 'rollback-client\n' > "$CLIENT/assets/rollback-marker.txt"
printf 'rollback-launcher\n' > "$ROOT/usr/share/plasma/plasmoids/org.neebles.launcher/rollback-marker.txt"
printf 'rollback-spacer\n' > "$ROOT/usr/share/plasma/plasmoids/org.neebles.spacer/rollback-marker.txt"
printf 'rollback-icon\n' > "$ROOT/usr/share/icons/hicolor/256x256/apps/neebles-boss-launcher-icon.png"

printf '\n# rollback-systemd\n' >> "$ROOT/usr/lib/systemd/user/neebles-tray-manager.service"

rm -f "$ROOT/usr/local/bin/neebles"
ln -s /baseline/neebles "$ROOT/usr/local/bin/neebles"

rm -f "$ROOT/etc/systemd/user/default.target.wants/neebles-tray-manager.service"
ln -s /baseline/neebles-tray-manager.service \
    "$ROOT/etc/systemd/user/default.target.wants/neebles-tray-manager.service"

set +e
NEEBLES_INSTALL_TEST_FAIL_AFTER_GLOBALS=1 \
DESTDIR="$ROOT" \
    "$REPO_ROOT/scripts/install.sh" \
    "$BACKEND" \
    "$UI" \
    "$TRAY" \
    "$ARCHIVE" \
    "$AUTH"
ROLLBACK_RC=$?
set -e

[[ "$ROLLBACK_RC" -eq 97 ]] || {
    echo "PACKAGING TEST INVALID: rollback failpoint returned $ROLLBACK_RC, expected 97" >&2
    exit 1
}

[[ -f "$CLIENT/assets/rollback-marker.txt" ]] || {
    echo "PACKAGING TEST INVALID: client tree was not restored after failed reinstall" >&2
    exit 1
}

[[ -f "$ROOT/usr/share/plasma/plasmoids/org.neebles.launcher/rollback-marker.txt" ]] || {
    echo "PACKAGING TEST INVALID: launcher was not restored after failed reinstall" >&2
    exit 1
}

[[ -f "$ROOT/usr/share/plasma/plasmoids/org.neebles.spacer/rollback-marker.txt" ]] || {
    echo "PACKAGING TEST INVALID: spacer was not restored after failed reinstall" >&2
    exit 1
}

grep -q '^rollback-icon$' \
    "$ROOT/usr/share/icons/hicolor/256x256/apps/neebles-boss-launcher-icon.png" \
    || {
        echo "PACKAGING TEST INVALID: global icon was not restored after failed reinstall" >&2
        exit 1
    }

grep -q '^# rollback-systemd$' \
    "$ROOT/usr/lib/systemd/user/neebles-tray-manager.service" \
    || {
        echo "PACKAGING TEST INVALID: systemd unit was not restored after failed reinstall" >&2
        exit 1
    }

grep -q '^# rollback-xdg$' \
    || {
        echo "PACKAGING TEST INVALID: XDG autostart was not restored after failed reinstall" >&2
        exit 1
    }

[[ "$(readlink "$ROOT/usr/local/bin/neebles")" == "/baseline/neebles" ]] || {
    echo "PACKAGING TEST INVALID: global command symlink was not restored" >&2
    exit 1
}

[[ "$(readlink "$ROOT/etc/systemd/user/default.target.wants/neebles-tray-manager.service")" == "/baseline/neebles-tray-manager.service" ]] || {
    echo "PACKAGING TEST INVALID: systemd wants symlink was not restored" >&2
    exit 1
}

[[ -f "$ROOT/opt/neebles/modules/must-survive-reinstall" ]] || {
    echo "PACKAGING TEST INVALID: modules tree changed during failed reinstall" >&2
    exit 1
}

[[ -f "$ROOT/opt/neebles/shared/must-survive-reinstall" ]] || {
    echo "PACKAGING TEST INVALID: shared tree changed during failed reinstall" >&2
    exit 1
}

echo "=== FINAL CLEAN REINSTALL ==="

run_install_archive

[[ ! -e "$CLIENT/assets/rollback-marker.txt" ]] || {
    echo "PACKAGING TEST INVALID: rollback client marker survived final reinstall" >&2
    exit 1
}

[[ ! -e "$ROOT/usr/share/plasma/plasmoids/org.neebles.launcher/rollback-marker.txt" ]] || {
    echo "PACKAGING TEST INVALID: rollback launcher marker survived final reinstall" >&2
    exit 1
}

[[ ! -e "$ROOT/usr/share/plasma/plasmoids/org.neebles.spacer/rollback-marker.txt" ]] || {
    echo "PACKAGING TEST INVALID: rollback spacer marker survived final reinstall" >&2
    exit 1
}

cmp -s \
    client/systemd/neebles-tray-manager.service \
    "$ROOT/usr/lib/systemd/user/neebles-tray-manager.service" \
    || {
        echo "PACKAGING TEST INVALID: final systemd unit differs from source" >&2
        exit 1
    }

cmp -s \
    || {
        echo "PACKAGING TEST INVALID: final XDG autostart differs from source" >&2
        exit 1
    }

[[ "$(readlink "$ROOT/usr/local/bin/neebles")" == "/opt/neebles/client/bin/neebles" ]] || {
    echo "PACKAGING TEST INVALID: final global Boss symlink is wrong" >&2
    exit 1
}

[[ "$(readlink "$ROOT/etc/systemd/user/default.target.wants/neebles-tray-manager.service")" == "/usr/lib/systemd/user/neebles-tray-manager.service" ]] || {
    echo "PACKAGING TEST INVALID: final Tray Manager enable symlink is wrong" >&2
    exit 1
}

echo "PACKAGING INSTALL CONTRACT: VALID"

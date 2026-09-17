#!/bin/bash
set -euo pipefail

OUTPUT="${1:-}"
SOURCE="${2:-client}"
LAUNCHER_PLUGIN_BUILD="${3:-client/launcher-plugin/build}"
TRAY_HOST_BUILD="${4:-client/tray-host/build}"

if [[ -z "$OUTPUT" ]]; then
    echo "Usage: $0 <output.tar.gz> [client-source-directory] [launcher-plugin-build-directory] [tray-host-build-directory]" >&2
    exit 1
fi

[[ -d "$SOURCE" ]] || {
    echo "Client source directory not found: $SOURCE" >&2
    exit 1
}

REQUIRED_ROOTS=(
    assets
    languages
    config
    launcher
    spacer
    notifications
    applications
    systemd
)

for root in "${REQUIRED_ROOTS[@]}"; do
    [[ -d "$SOURCE/$root" ]] || {
        echo "Client source is missing required root: $root" >&2
        exit 1
    }
done

OUTPUT="$(realpath -m "$OUTPUT")"
SOURCE="$(realpath "$SOURCE")"
LAUNCHER_PLUGIN_BUILD="$(realpath "$LAUNCHER_PLUGIN_BUILD")"
TRAY_HOST_BUILD="$(realpath "$TRAY_HOST_BUILD")"

LAUNCHER_RUNTIME_FILES=(
    libneebles-launcher-events.so
    libneebles-launcher-eventsplugin.so
    neebles-launcher-events.qmltypes
    qmldir
)

for item in "${LAUNCHER_RUNTIME_FILES[@]}"; do
    [[ -f "$LAUNCHER_PLUGIN_BUILD/$item" ]] || {
        echo "Launcher plugin runtime artifact missing: $LAUNCHER_PLUGIN_BUILD/$item" >&2
        exit 1
    }
done

[[ -f "$TRAY_HOST_BUILD/neebles-tray-host" ]] || {
    echo "Tray Host runtime artifact missing: $TRAY_HOST_BUILD/neebles-tray-host" >&2
    exit 1
}

install -d -m 0755 "$(dirname "$OUTPUT")"

TMP="${OUTPUT}.tmp.$$"
STAGE="$(mktemp -d)"

cleanup() {
    rm -f "$TMP"
    rm -rf "$STAGE"
}

trap cleanup EXIT

for root in "${REQUIRED_ROOTS[@]}"; do
    cp -a "$SOURCE/$root" "$STAGE/$root"
done

install -d -m 0755     "$STAGE/runtime/tray-host"     "$STAGE/runtime/qml/NEEBLES/BossEvents"

install -m 0755     "$TRAY_HOST_BUILD/neebles-tray-host"     "$STAGE/runtime/tray-host/neebles-tray-host"

for item in "${LAUNCHER_RUNTIME_FILES[@]}"; do
    install -m 0644         "$LAUNCHER_PLUGIN_BUILD/$item"         "$STAGE/runtime/qml/NEEBLES/BossEvents/$item"
done

tar     --sort=name     --mtime='@0'     --owner=0     --group=0     --numeric-owner     --mode='u+rwX,go+rX,go-w'     -C "$STAGE"     -cf -     "${REQUIRED_ROOTS[@]}"     runtime     | gzip -n > "$TMP"

mv "$TMP" "$OUTPUT"
trap - EXIT
rm -rf "$STAGE"

echo "Built reproducible client data: $OUTPUT"

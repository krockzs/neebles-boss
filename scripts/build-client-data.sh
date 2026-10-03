#!/bin/bash
set -euo pipefail

OUTPUT="${1:-}"
SOURCE="${2:-client}"
LAUNCHER_PLUGIN_INSTALL="${3:-client/launcher-plugin/install}"
TRAY_HOST_BINARY="${4:-client/tray-host/build/neebles-tray-host}"

if [[ -z "$OUTPUT" ]]; then
    echo "Usage: $0 <output.tar.gz> [client-source-directory] [launcher-plugin-install-directory] [tray-host-binary]" >&2
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
LAUNCHER_PLUGIN_INSTALL="$(realpath "$LAUNCHER_PLUGIN_INSTALL")"
TRAY_HOST_BINARY="$(realpath "$TRAY_HOST_BINARY")"

LAUNCHER_RUNTIME_FILES=(
    libneebles-launcher-events.so
    libneebles-launcher-eventsplugin.so
    neebles-launcher-events.qmltypes
    qmldir
)

[[ -f "$TRAY_HOST_BINARY" ]] || {
    echo "Tray Host runtime artifact missing: $TRAY_HOST_BINARY" >&2
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

for item in "${LAUNCHER_RUNTIME_FILES[@]}"; do
    [[ -f "$LAUNCHER_PLUGIN_INSTALL/$item" ]] || {
        echo "Launcher plugin runtime artifact missing: $LAUNCHER_PLUGIN_INSTALL/$item" >&2
        exit 1
    }
done

install -d -m 0755     "$STAGE/runtime/tray-host"     "$STAGE/runtime/qml/NEEBLES/BossEvents"

install -m 0755     "$TRAY_HOST_BINARY"     "$STAGE/runtime/tray-host/neebles-tray-host"

for item in "${LAUNCHER_RUNTIME_FILES[@]}"; do
    install -m 0644         "$LAUNCHER_PLUGIN_INSTALL/$item"         "$STAGE/runtime/qml/NEEBLES/BossEvents/$item"
done

tar     --sort=name     --mtime='@0'     --owner=0     --group=0     --numeric-owner     --mode='u+rwX,go+rX,go-w'     -C "$STAGE"     -cf -     "${REQUIRED_ROOTS[@]}"     runtime     | gzip -n > "$TMP"

mv "$TMP" "$OUTPUT"
trap - EXIT
rm -rf "$STAGE"

echo "Built reproducible client data: $OUTPUT"

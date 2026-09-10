#!/bin/bash
set -euo pipefail

NEEBLES_ROOT="/opt/neebles"
CLIENT_ROOT="$NEEBLES_ROOT/client"
BIN_DIR="$CLIENT_ROOT/bin"
BACKEND_DIR="$CLIENT_ROOT/backend"
UI_DIR="$CLIENT_ROOT/ui"
MODULES_DIR="$NEEBLES_ROOT/modules"
SHARED_DIR="$NEEBLES_ROOT/shared"

progress() {
    echo "NEEBLES_PROGRESS=$1"
}

status() {
    echo "NEEBLES_STATUS=$1"
}

if [[ ${EUID} -ne 0 ]]; then
    echo "This installer must run as root." >&2
    exit 1
fi

if [[ $# -lt 2 ]]; then
    echo "Usage: $0 <neebles-backend-binary> <neebles-ui-binary>" >&2
    exit 1
fi

BACKEND_SOURCE="$1"
UI_SOURCE="$2"

progress 5
status "Administrator authorization accepted."

progress 12
status "Validating N.E.E.B.L.E.S. payload..."
[[ -f "$BACKEND_SOURCE" ]] || { echo "Backend binary not found: $BACKEND_SOURCE" >&2; exit 1; }
[[ -f "$UI_SOURCE" ]] || { echo "UI binary not found: $UI_SOURCE" >&2; exit 1; }

progress 25
status "Creating N.E.E.B.L.E.S. directory structure..."
install -d "$BIN_DIR" "$BACKEND_DIR" "$UI_DIR" "$MODULES_DIR" "$SHARED_DIR"

progress 45
status "Installing N.E.E.B.L.E.S. backend..."
install -m 0755 "$BACKEND_SOURCE" "$BACKEND_DIR/neebles-backend"

progress 65
status "Installing N.E.E.B.L.E.S. UI..."
install -m 0755 "$UI_SOURCE" "$UI_DIR/neebles-ui"

progress 78
status "Creating N.E.E.B.L.E.S. client entrypoint..."
ln -sfn "$BACKEND_DIR/neebles-backend" "$BIN_DIR/neebles"
ln -sfn "$BIN_DIR/neebles" /usr/local/bin/neebles

progress 90
status "Verifying installed backend..."
"$BIN_DIR/neebles" --version

progress 100
status "N.E.E.B.L.E.S. Boss installed successfully."
echo "Installed backend: $BACKEND_DIR/neebles-backend"
echo "Installed UI: $UI_DIR/neebles-ui"
echo "OS entrypoint: $BIN_DIR/neebles"
echo "Global command: /usr/local/bin/neebles"

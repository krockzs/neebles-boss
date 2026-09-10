#!/bin/bash
set -euo pipefail

NEEBLES_ROOT="/opt/neebles"
BOSS_ROOT="$NEEBLES_ROOT/boss"
BIN_DIR="$BOSS_ROOT/bin"
BACKEND_DIR="$BOSS_ROOT/backend"
UI_DIR="$BOSS_ROOT/ui"
MODULES_DIR="$NEEBLES_ROOT/modules"
SHARED_DIR="$NEEBLES_ROOT/shared"

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

install -d "$BIN_DIR" "$BACKEND_DIR" "$UI_DIR" "$MODULES_DIR" "$SHARED_DIR"

install -m 0755 "$BACKEND_SOURCE" "$BACKEND_DIR/neebles-backend"
install -m 0755 "$UI_SOURCE" "$UI_DIR/neebles-ui"

ln -sfn "$BACKEND_DIR/neebles-backend" "$BIN_DIR/neebles"
ln -sfn "$BIN_DIR/neebles" /usr/local/bin/neebles

echo "N.E.E.B.L.E.S. Boss installed in $BOSS_ROOT"

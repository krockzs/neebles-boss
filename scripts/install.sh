#!/bin/bash
set -euo pipefail

NEEBLES_ROOT="/opt/neebles"
CLIENT_ROOT="$NEEBLES_ROOT/client"
BIN_DIR="$CLIENT_ROOT/bin"
BACKEND_DIR="$CLIENT_ROOT/backend"
UI_DIR="$CLIENT_ROOT/ui"
TRAY_DIR="$CLIENT_ROOT/tray"
AUTH_DIR="$CLIENT_ROOT/auth"
LAUNCHER_DIR="$CLIENT_ROOT/launcher"
SPACER_DIR="$CLIENT_ROOT/spacer"
NOTIFICATIONS_DIR="$CLIENT_ROOT/notifications"
ASSETS_DIR="$CLIENT_ROOT/assets"
LANGUAGES_DIR="$CLIENT_ROOT/languages"
CONFIG_DIR="$CLIENT_ROOT/config"
MODULES_DIR="$NEEBLES_ROOT/modules"
SHARED_DIR="$NEEBLES_ROOT/shared"

progress() { echo "NEEBLES_PROGRESS=$1"; }
status() { echo "NEEBLES_STATUS=$1"; }

if [[ ${EUID} -ne 0 ]]; then
    echo "This installer must run as root." >&2
    exit 1
fi

if [[ $# -lt 4 ]]; then
    echo "Usage: $0 <neebles-backend-binary> <neebles-ui-binary> <neebles-tray-binary> <client-data.tar.gz>" >&2
    exit 1
fi

BACKEND_SOURCE="$1"
UI_SOURCE="$2"
TRAY_SOURCE="$3"
CLIENT_DATA_ARCHIVE="$4"
AUTH_AGENT_SOURCE="${5:-}"
CLIENT_DATA_SOURCE=""
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_CLIENT_DIR="$(cd -- "$SCRIPT_DIR/.." 2>/dev/null && pwd)/client"

CLIENT_DATA_TMP=""

cleanup() {
    if [[ -n "$CLIENT_DATA_TMP" && -d "$CLIENT_DATA_TMP" ]]; then
        rm -rf "$CLIENT_DATA_TMP"
    fi
}
trap cleanup EXIT

if [[ -f "$CLIENT_DATA_ARCHIVE" ]]; then
    CLIENT_DATA_TMP="$(mktemp -d)"
    tar -xzf "$CLIENT_DATA_ARCHIVE" -C "$CLIENT_DATA_TMP"
    CLIENT_DATA_SOURCE="$CLIENT_DATA_TMP"
elif [[ -d "$CLIENT_DATA_ARCHIVE" ]]; then
    CLIENT_DATA_SOURCE="$CLIENT_DATA_ARCHIVE"
else
    echo "Client data payload not found: $CLIENT_DATA_ARCHIVE" >&2
    exit 1
fi

progress 5
status "Administrator authorization accepted."

progress 10
status "Validating N.E.E.B.L.E.S. payload..."
[[ -f "$BACKEND_SOURCE" ]] || { echo "Backend binary not found: $BACKEND_SOURCE" >&2; exit 1; }
[[ -f "$UI_SOURCE" ]] || { echo "UI binary not found: $UI_SOURCE" >&2; exit 1; }
[[ -f "$TRAY_SOURCE" ]] || { echo "Tray binary not found: $TRAY_SOURCE" >&2; exit 1; }
[[ -f "$CLIENT_DATA_ARCHIVE" || -d "$CLIENT_DATA_ARCHIVE" ]] || {
    echo "Client data payload not found: $CLIENT_DATA_ARCHIVE" >&2
    exit 1
}

progress 16
status "Checking Boss runtime dependencies..."
if command -v apt >/dev/null 2>&1; then
    MISSING=()
    command -v git >/dev/null 2>&1 || MISSING+=(git)
    command -v curl >/dev/null 2>&1 || MISSING+=(curl)
    command -v notify-send >/dev/null 2>&1 || MISSING+=(libnotify-bin)
    if (( ${#MISSING[@]} > 0 )); then
        apt install -y "${MISSING[@]}"
    fi
fi

progress 25
status "Creating N.E.E.B.L.E.S. directory structure..."
install -d "$BIN_DIR" "$BACKEND_DIR" "$UI_DIR" "$TRAY_DIR" "$LAUNCHER_DIR" "$SPACER_DIR" \
    "$NOTIFICATIONS_DIR" "$ASSETS_DIR" "$LANGUAGES_DIR" "$CONFIG_DIR" \
    "$AUTH_DIR" "$MODULES_DIR" "$SHARED_DIR"

progress 38
status "Installing N.E.E.B.L.E.S. backend..."
install -m 0755 "$BACKEND_SOURCE" "$BACKEND_DIR/neebles-backend"

progress 50
status "Installing N.E.E.B.L.E.S. UI..."
install -m 0755 "$UI_SOURCE" "$UI_DIR/neebles-ui"

if [[ -n "$AUTH_AGENT_SOURCE" ]]; then
    status "Installing N.E.E.B.L.E.S. authorization agent..."

    [[ -x "$AUTH_AGENT_SOURCE" ]] || {
        echo "Authorization agent not found or not executable: $AUTH_AGENT_SOURCE" >&2
        exit 1
    }

    install -m 0755         "$AUTH_AGENT_SOURCE"         "$AUTH_DIR/neebles-auth-agent"
fi

if [[ -n "$TRAY_SOURCE" ]]; then
    progress 58
    status "Installing N.E.E.B.L.E.S. tray..."
    install -m 0755 "$TRAY_SOURCE" "$TRAY_DIR/neebles-tray"
fi

if [[ -n "$CLIENT_DATA_SOURCE" && -d "$CLIENT_DATA_SOURCE" ]]; then
    progress 66
    status "Installing N.E.E.B.L.E.S. client data..."
    [[ -d "$CLIENT_DATA_SOURCE/assets" ]] && cp -a "$CLIENT_DATA_SOURCE/assets/." "$ASSETS_DIR/"
    [[ -d "$CLIENT_DATA_SOURCE/languages" ]] && cp -a "$CLIENT_DATA_SOURCE/languages/." "$LANGUAGES_DIR/"
    [[ -d "$CLIENT_DATA_SOURCE/config" ]] && cp -a "$CLIENT_DATA_SOURCE/config/." "$CONFIG_DIR/"
    [[ -d "$CLIENT_DATA_SOURCE/notifications" ]] && cp -a "$CLIENT_DATA_SOURCE/notifications/." "$NOTIFICATIONS_DIR/"
    if [[ -d "$CLIENT_DATA_SOURCE/launcher" ]]; then
        rm -rf "$LAUNCHER_DIR"
        cp -a "$CLIENT_DATA_SOURCE/launcher" "$LAUNCHER_DIR"
    fi
    if [[ -d "$CLIENT_DATA_SOURCE/spacer" ]]; then
        rm -rf "$SPACER_DIR"
        cp -a "$CLIENT_DATA_SOURCE/spacer" "$SPACER_DIR"
    fi
fi

progress 75
status "Creating N.E.E.B.L.E.S. client entrypoint..."
ln -sfn "$BACKEND_DIR/neebles-backend" "$BIN_DIR/neebles"
ln -sfn "$BIN_DIR/neebles" /usr/local/bin/neebles

if [[ -f "$ASSETS_DIR/branding/neebles-boss-launcher-icon.png" ]]; then
    install -d /usr/share/icons/hicolor/256x256/apps
    install -m 0644 "$ASSETS_DIR/branding/neebles-boss-launcher-icon.png" \
        /usr/share/icons/hicolor/256x256/apps/neebles-boss-launcher-icon.png
fi

if [[ -x "$TRAY_DIR/neebles-tray" ]]; then
    progress 82
    status "Registering N.E.E.B.L.E.S. tray autostart..."
    install -d /etc/xdg/autostart
    cat > /etc/xdg/autostart/neebles-tray.desktop <<DESKTOP
[Desktop Entry]
Type=Application
Name=N.E.E.B.L.E.S. Tray
Exec=$TRAY_DIR/neebles-tray
Icon=neebles-boss-launcher-icon
Terminal=false
X-KDE-autostart-after=panel
DESKTOP
fi

if [[ -f "$LAUNCHER_DIR/metadata.json" && -d "$LAUNCHER_DIR/contents" ]]; then
    progress 88
    status "Installing N.E.E.B.L.E.S. Plasma launcher package..."
    install -d /usr/share/plasma/plasmoids/org.neebles.launcher
    cp -a "$LAUNCHER_DIR/." /usr/share/plasma/plasmoids/org.neebles.launcher/
fi

if [[ -f "$SPACER_DIR/metadata.json" && -d "$SPACER_DIR/contents" ]]; then
    status "Installing N.E.E.B.L.E.S. Plasma spacer package..."
    rm -rf /usr/share/plasma/plasmoids/org.neebles.spacer
    install -d /usr/share/plasma/plasmoids/org.neebles.spacer
    cp -a "$SPACER_DIR/." /usr/share/plasma/plasmoids/org.neebles.spacer/
fi

progress 94
status "Verifying installed backend..."
"$BIN_DIR/neebles" --version

progress 100
status "N.E.E.B.L.E.S. Boss installed successfully."
echo "Installed backend: $BACKEND_DIR/neebles-backend"
echo "Installed UI: $UI_DIR/neebles-ui"
[[ -x "$TRAY_DIR/neebles-tray" ]] && echo "Installed tray: $TRAY_DIR/neebles-tray"
echo "OS entrypoint: $BIN_DIR/neebles"
echo "Global command: /usr/local/bin/neebles"

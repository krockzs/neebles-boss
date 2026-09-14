#!/bin/bash
set -euo pipefail

NEEBLES_ROOT="/opt/neebles"
CLIENT_ROOT="$NEEBLES_ROOT/client"
BIN_DIR="$CLIENT_ROOT/bin"
BACKEND_DIR="$CLIENT_ROOT/backend"
UI_DIR="$CLIENT_ROOT/ui"
TRAY_DIR="$CLIENT_ROOT/tray-host"
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
status_key() { echo "NEEBLES_STATUS_KEY=$1"; }

if [[ ${EUID} -ne 0 ]]; then
    echo "This installer must run as root." >&2
    exit 1
fi

if [[ $# -lt 5 ]]; then
    echo "Usage: $0 <neebles-backend-binary> <neebles-ui-binary> <neebles-tray-host-binary> <client-data.tar.gz> <neebles-auth-agent-binary>" >&2
    exit 1
fi

BACKEND_SOURCE="$1"
UI_SOURCE="$2"
TRAY_SOURCE="$3"
CLIENT_DATA_ARCHIVE="$4"
AUTH_AGENT_SOURCE="$5"
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
status_key "installer.progress.authorization_accepted"

progress 10
status_key "installer.progress.validating_payload"
[[ -f "$BACKEND_SOURCE" ]] || { echo "Backend binary not found: $BACKEND_SOURCE" >&2; exit 1; }
[[ -f "$UI_SOURCE" ]] || { echo "UI binary not found: $UI_SOURCE" >&2; exit 1; }
[[ -f "$TRAY_SOURCE" ]] || { echo "Tray binary not found: $TRAY_SOURCE" >&2; exit 1; }
[[ -f "$CLIENT_DATA_ARCHIVE" || -d "$CLIENT_DATA_ARCHIVE" ]] || {
    echo "Client data payload not found: $CLIENT_DATA_ARCHIVE" >&2
    exit 1
}

[[ -f "$CLIENT_DATA_SOURCE/systemd/neebles-tray-manager.service" ]] || {
    echo "Client data is missing systemd/neebles-tray-manager.service" >&2
    exit 1
}

[[ -f "$CLIENT_DATA_SOURCE/xdg/neebles-tray-host.desktop" ]] || {
    echo "Client data is missing xdg/neebles-tray-host.desktop" >&2
    exit 1
}

progress 16
status_key "installer.progress.checking_dependencies"
if command -v apt >/dev/null 2>&1; then
    MISSING=()
    command -v git >/dev/null 2>&1 || MISSING+=(git)
    command -v curl >/dev/null 2>&1 || MISSING+=(curl)
    command -v notify-send >/dev/null 2>&1 || MISSING+=(libnotify-bin)

    dpkg-query -W -f='${Status}' libqt6quick6 2>/dev/null         | grep -q "install ok installed"         || MISSING+=(libqt6quick6)

    dpkg-query -W -f='${Status}' liblayershellqtinterface6 2>/dev/null         | grep -q "install ok installed"         || MISSING+=(liblayershellqtinterface6)
    if (( ${#MISSING[@]} > 0 )); then
        apt install -y "${MISSING[@]}"
    fi
fi

progress 25
status_key "installer.progress.creating_structure"
install -d "$BIN_DIR" "$BACKEND_DIR" "$UI_DIR" "$TRAY_DIR" "$LAUNCHER_DIR" "$SPACER_DIR" \
    "$NOTIFICATIONS_DIR" "$ASSETS_DIR" "$LANGUAGES_DIR" "$CONFIG_DIR" \
    "$AUTH_DIR" "$MODULES_DIR" "$SHARED_DIR"

progress 38
status_key "installer.progress.installing_backend"
install -m 0755 "$BACKEND_SOURCE" "$BACKEND_DIR/neebles-backend"

progress 50
status_key "installer.progress.installing_ui"
install -m 0755 "$UI_SOURCE" "$UI_DIR/neebles-ui"

status_key "installer.progress.installing_auth_agent"

[[ -x "$AUTH_AGENT_SOURCE" ]] || {
    echo "Authorization agent not found or not executable: $AUTH_AGENT_SOURCE" >&2
    exit 1
}

install -m 0755 \
    "$AUTH_AGENT_SOURCE" \
    "$AUTH_DIR/neebles-auth-agent"

if [[ -n "$TRAY_SOURCE" ]]; then
    progress 58
    status_key "installer.progress.installing_tray_host"
    install -m 0755 "$TRAY_SOURCE" "$TRAY_DIR/neebles-tray-host"
fi

if [[ -n "$CLIENT_DATA_SOURCE" && -d "$CLIENT_DATA_SOURCE" ]]; then
    progress 66
    status_key "installer.progress.installing_client_data"
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
status_key "installer.progress.creating_entrypoint"
ln -sfn "$BACKEND_DIR/neebles-backend" "$BIN_DIR/neebles"
ln -sfn "$BIN_DIR/neebles" /usr/local/bin/neebles

if [[ -f "$ASSETS_DIR/branding/neebles-boss-launcher-icon.png" ]]; then
    install -d /usr/share/icons/hicolor/256x256/apps
    install -m 0644 "$ASSETS_DIR/branding/neebles-boss-launcher-icon.png" \
        /usr/share/icons/hicolor/256x256/apps/neebles-boss-launcher-icon.png
fi

progress 80
status_key "installer.progress.installing_tray_manager"
install -d /usr/lib/systemd/user
install -m 0644 \
    "$CLIENT_DATA_SOURCE/systemd/neebles-tray-manager.service" \
    /usr/lib/systemd/user/neebles-tray-manager.service

install -d /etc/systemd/user/default.target.wants
ln -sfn     /usr/lib/systemd/user/neebles-tray-manager.service     /etc/systemd/user/default.target.wants/neebles-tray-manager.service

if [[ -x "$TRAY_DIR/neebles-tray-host" ]]; then
    progress 82
    status_key "installer.progress.registering_tray_autostart"
    install -d /etc/xdg/autostart
    install -m 0644 \
        "$CLIENT_DATA_SOURCE/xdg/neebles-tray-host.desktop" \
        /etc/xdg/autostart/neebles-tray-host.desktop
fi

if [[ -f "$LAUNCHER_DIR/metadata.json" && -d "$LAUNCHER_DIR/contents" ]]; then
    progress 88
    status_key "installer.progress.installing_launcher"
    install -d /usr/share/plasma/plasmoids/org.neebles.launcher
    cp -a "$LAUNCHER_DIR/." /usr/share/plasma/plasmoids/org.neebles.launcher/
fi

if [[ -f "$SPACER_DIR/metadata.json" && -d "$SPACER_DIR/contents" ]]; then
    status_key "installer.progress.installing_spacer"
    rm -rf /usr/share/plasma/plasmoids/org.neebles.spacer
    install -d /usr/share/plasma/plasmoids/org.neebles.spacer
    cp -a "$SPACER_DIR/." /usr/share/plasma/plasmoids/org.neebles.spacer/
fi

progress 94
status_key "installer.progress.verifying_backend"
"$BIN_DIR/neebles" --version

progress 100
status_key "installer.progress.completed"
echo "Installed backend: $BACKEND_DIR/neebles-backend"
echo "Installed UI: $UI_DIR/neebles-ui"
[[ -x "$TRAY_DIR/neebles-tray-host" ]] && echo "Installed tray host: $TRAY_DIR/neebles-tray-host"
echo "OS entrypoint: $BIN_DIR/neebles"
echo "Global command: /usr/local/bin/neebles"

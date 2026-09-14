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
[[ -x "$AUTH_AGENT_SOURCE" ]] || { echo "Authorization agent not found or not executable: $AUTH_AGENT_SOURCE" >&2; exit 1; }

for required_dir in assets languages config launcher spacer notifications; do
    [[ -d "$CLIENT_DATA_SOURCE/$required_dir" ]] || {
        echo "Client data payload is missing required directory: $required_dir" >&2
        exit 1
    }
done

for required_file in \
    languages/manifest.json \
    launcher/metadata.json \
    spacer/metadata.json
do
    [[ -f "$CLIENT_DATA_SOURCE/$required_file" ]] || {
        echo "Client data payload is missing required file: $required_file" >&2
        exit 1
    }
done

progress 16
status_key "installer.progress.checking_dependencies"
if command -v apt >/dev/null 2>&1; then
    MISSING=()

    command -v git >/dev/null 2>&1 || MISSING+=(git)
    command -v curl >/dev/null 2>&1 || MISSING+=(curl)
    command -v python3 >/dev/null 2>&1 || MISSING+=(python3)
    command -v notify-send >/dev/null 2>&1 || MISSING+=(libnotify-bin)

    require_package_minimum() {
        local package="$1"
        local minimum="$2"
        local installed=""

        installed="$(dpkg-query -W -f='${Version}' "$package" 2>/dev/null || true)"

        if [[ -z "$installed" ]] || ! dpkg --compare-versions "$installed" ge "$minimum"; then
            MISSING+=("$package")
        fi
    }

    # N.E.E.B.L.E.S. OS currently targets Debian Trixie. These are compatibility
    # floors, not exact pins: normal compatible Debian updates remain allowed.
    require_package_minimum libqt6core6t64 6.8.2
    require_package_minimum libqt6dbus6 6.8.2
    require_package_minimum libqt6gui6 6.8.2
    require_package_minimum libqt6network6 6.8.2
    require_package_minimum libqt6qml6 6.8.2
    require_package_minimum libqt6quick6 6.8.2
    require_package_minimum libqt6quickcontrols2-6 6.8.2
    require_package_minimum libqt6widgets6 6.8.2
    require_package_minimum qml6-module-qtqml 6.8.2
    require_package_minimum qml6-module-qtquick 6.8.2
    require_package_minimum qml6-module-qtquick-controls 6.8.2
    require_package_minimum qml6-module-qtquick-layouts 6.8.2
    require_package_minimum qml6-module-qtquick-window 6.8.2
    require_package_minimum qt6-wayland 6.8.2
    require_package_minimum liblayershellqtinterface6 6.3.4
    require_package_minimum libpolkit-qt6-1-1 0.200.0

    if (( ${#MISSING[@]} > 0 )); then
        apt install -y "${MISSING[@]}"
    fi
else
    echo "N.E.E.B.L.E.S. Boss installer requires an apt-compatible Debian system." >&2
    exit 1
fi

# Fail closed if apt completed but the runtime floor is still not satisfied.
verify_package_minimum() {
    local package="$1"
    local minimum="$2"
    local installed=""

    installed="$(dpkg-query -W -f='${Version}' "$package" 2>/dev/null || true)"
    if [[ -z "$installed" ]] || ! dpkg --compare-versions "$installed" ge "$minimum"; then
        echo "Runtime compatibility failure: $package requires >= $minimum, installed=${installed:-missing}" >&2
        exit 1
    fi
}

verify_package_minimum libqt6core6t64 6.8.2
verify_package_minimum libqt6qml6 6.8.2
verify_package_minimum libqt6quick6 6.8.2
verify_package_minimum libqt6quickcontrols2-6 6.8.2
verify_package_minimum liblayershellqtinterface6 6.3.4
verify_package_minimum libpolkit-qt6-1-1 0.200.0

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
install -m 0755 \
    "$AUTH_AGENT_SOURCE" \
    "$AUTH_DIR/neebles-auth-agent"

progress 58
status_key "installer.progress.installing_tray_host"
install -m 0755 "$TRAY_SOURCE" "$TRAY_DIR/neebles-tray-host"

progress 66
status_key "installer.progress.installing_client_data"
cp -a "$CLIENT_DATA_SOURCE/assets/." "$ASSETS_DIR/"
cp -a "$CLIENT_DATA_SOURCE/languages/." "$LANGUAGES_DIR/"
cp -a "$CLIENT_DATA_SOURCE/config/." "$CONFIG_DIR/"
cp -a "$CLIENT_DATA_SOURCE/notifications/." "$NOTIFICATIONS_DIR/"
rm -rf "$LAUNCHER_DIR"
cp -a "$CLIENT_DATA_SOURCE/launcher" "$LAUNCHER_DIR"
rm -rf "$SPACER_DIR"
cp -a "$CLIENT_DATA_SOURCE/spacer" "$SPACER_DIR"

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

cat > /usr/lib/systemd/user/neebles-tray-manager.service <<SERVICE
[Unit]
Description=N.E.E.B.L.E.S. Tray Manager
After=graphical-session-pre.target
PartOf=graphical-session.target

[Service]
Type=simple
ExecStart=$BACKEND_DIR/neebles-backend tray serve
Restart=on-failure
RestartSec=1

[Install]
WantedBy=default.target
SERVICE

install -d /etc/systemd/user/default.target.wants
ln -sfn \
    /usr/lib/systemd/user/neebles-tray-manager.service \
    /etc/systemd/user/default.target.wants/neebles-tray-manager.service

progress 82
status_key "installer.progress.registering_tray_autostart"
install -d /etc/xdg/autostart
cat > /etc/xdg/autostart/neebles-tray-host.desktop <<DESKTOP
[Desktop Entry]
Type=Application
Name=N.E.E.B.L.E.S. Tray Host
Exec=$TRAY_DIR/neebles-tray-host
Icon=neebles-boss-launcher-icon
Terminal=false
X-KDE-autostart-after=panel
X-KDE-StartupNotify=false
DESKTOP

progress 88
status_key "installer.progress.installing_launcher"
install -d /usr/share/plasma/plasmoids/org.neebles.launcher
cp -a "$LAUNCHER_DIR/." /usr/share/plasma/plasmoids/org.neebles.launcher/

status_key "installer.progress.installing_spacer"
rm -rf /usr/share/plasma/plasmoids/org.neebles.spacer
install -d /usr/share/plasma/plasmoids/org.neebles.spacer
cp -a "$SPACER_DIR/." /usr/share/plasma/plasmoids/org.neebles.spacer/

progress 94
status_key "installer.progress.verifying_backend"
"$BIN_DIR/neebles" --version

progress 100
status_key "installer.progress.completed"
echo "Installed backend: $BACKEND_DIR/neebles-backend"
echo "Installed UI: $UI_DIR/neebles-ui"
echo "Installed tray host: $TRAY_DIR/neebles-tray-host"
echo "OS entrypoint: $BIN_DIR/neebles"
echo "Global command: /usr/local/bin/neebles"

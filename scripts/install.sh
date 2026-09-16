#!/bin/bash
set -euo pipefail

NEEBLES_ROOT="/opt/neebles"

DESTDIR="${DESTDIR:-}"

if [[ -n "$DESTDIR" ]]; then
    [[ "$DESTDIR" == /* ]] || {
        echo "DESTDIR must be an absolute path." >&2
        exit 1
    }

    DESTDIR="${DESTDIR%/}"
fi

INSTALL_ROOT="${DESTDIR}${NEEBLES_ROOT}"
CLIENT_ROOT="$INSTALL_ROOT/client"
BIN_DIR="$CLIENT_ROOT/bin"
BACKEND_DIR="$CLIENT_ROOT/backend"
UI_DIR="$CLIENT_ROOT/ui"
AUTH_DIR="$CLIENT_ROOT/auth"
LAUNCHER_DIR="$CLIENT_ROOT/launcher"
SPACER_DIR="$CLIENT_ROOT/spacer"
NOTIFICATIONS_DIR="$CLIENT_ROOT/notifications"
APPLICATIONS_DIR="$CLIENT_ROOT/applications"
ASSETS_DIR="$CLIENT_ROOT/assets"
LANGUAGES_DIR="$CLIENT_ROOT/languages"
CONFIG_DIR="$CLIENT_ROOT/config"
MODULES_DIR="$INSTALL_ROOT/modules"
SHARED_DIR="$INSTALL_ROOT/shared"
TMP_DIR="$SHARED_DIR/tmp"
SETTINGS_DIR="$SHARED_DIR/settings"

GLOBAL_BIN="${DESTDIR}/usr/local/bin/neebles"
GLOBAL_ICON="${DESTDIR}/usr/share/icons/hicolor/256x256/apps/neebles-boss-launcher-icon.png"
GLOBAL_BOSS_ICON="${DESTDIR}/usr/share/icons/hicolor/256x256/apps/neebles-boss-icon.png"
GLOBAL_DESKTOP="${DESTDIR}/usr/share/applications/org.neebles.Boss.desktop"
SYSTEMD_SERVICE="${DESTDIR}/usr/lib/systemd/user/neebles-tray-manager.service"
TRAY_HOST_SERVICE="${DESTDIR}/usr/lib/systemd/user/neebles-tray-host.service"
SYSTEMD_WANTS="${DESTDIR}/etc/systemd/user/default.target.wants"
STAGE0_SERVICE="${DESTDIR}/usr/lib/systemd/system/neebles-stage0.service"
RUNTIME_SERVICE="${DESTDIR}/usr/lib/systemd/system/neebles-runtime.service"
RUNTIME_WANTS="${DESTDIR}/etc/systemd/system/multi-user.target.wants"
RUNTIME_ENV="${DESTDIR}/etc/neebles/runtime.env"
PLASMA_LAUNCHER="${DESTDIR}/usr/share/plasma/plasmoids/org.neebles.launcher"
PLASMA_SPACER="${DESTDIR}/usr/share/plasma/plasmoids/org.neebles.spacer"

progress() {
    echo "NEEBLES_PROGRESS=$1"
}

status_key() {
    echo "NEEBLES_STATUS_KEY=$1"
}

CLIENT_DATA_TMP=""
CLIENT_STAGE=""
CLIENT_BACKUP=""
CLIENT_SWAPPED=0
GLOBAL_BACKUP_ROOT=""

declare -a GLOBAL_PATHS=()
declare -a GLOBAL_BACKUPS=()
declare -a GLOBAL_EXISTED=()

backup_global_path() {
    local path="$1"
    local key="$2"
    local backup="$GLOBAL_BACKUP_ROOT/$key"
    local existed=0

    if [[ -e "$path" || -L "$path" ]]; then
        existed=1
        cp -a -- "$path" "$backup"
    fi

    GLOBAL_PATHS+=("$path")
    GLOBAL_BACKUPS+=("$backup")
    GLOBAL_EXISTED+=("$existed")
}

restore_global_paths() {
    local index
    local path
    local backup
    local existed

    for (( index=${#GLOBAL_PATHS[@]}-1; index>=0; index-- )); do
        path="${GLOBAL_PATHS[$index]}"
        backup="${GLOBAL_BACKUPS[$index]}"
        existed="${GLOBAL_EXISTED[$index]}"

        rm -rf -- "$path"

        if [[ "$existed" == "1" ]]; then
            install -d -m 0755 "$(dirname "$path")"
            cp -a -- "$backup" "$path"
        fi
    done
}

cleanup() {
    local rc=$?
    trap - EXIT

    if (( rc != 0 )); then
        if (( ${#GLOBAL_PATHS[@]} > 0 )); then
            echo "N.E.E.B.L.E.S.: installation failed; restoring global integrations." >&2
            restore_global_paths
        fi

        if (( CLIENT_SWAPPED == 1 )); then
            echo "N.E.E.B.L.E.S.: installation failed; restoring previous client tree." >&2

            rm -rf "$CLIENT_ROOT"

            if [[ -n "$CLIENT_BACKUP" && -e "$CLIENT_BACKUP" ]]; then
                mv "$CLIENT_BACKUP" "$CLIENT_ROOT"
                CLIENT_BACKUP=""
            fi
        fi
    fi

    if [[ -n "$CLIENT_STAGE" && -e "$CLIENT_STAGE" ]]; then
        rm -rf "$CLIENT_STAGE"
    fi

    if [[ -n "$CLIENT_DATA_TMP" && -d "$CLIENT_DATA_TMP" ]]; then
        rm -rf "$CLIENT_DATA_TMP"
    fi

    if [[ -n "$GLOBAL_BACKUP_ROOT" && -d "$GLOBAL_BACKUP_ROOT" ]]; then
        rm -rf "$GLOBAL_BACKUP_ROOT"
    fi

    if (( rc == 0 )) && [[ -n "$CLIENT_BACKUP" && -e "$CLIENT_BACKUP" ]]; then
        rm -rf "$CLIENT_BACKUP"
    fi

    exit "$rc"
}

validate_client_archive() {
    local archive="$1"
    local entry
    local normalized
    local verbose
    local node_type

    tar -tzf "$archive" >/dev/null || {
        echo "Client data archive cannot be listed: $archive" >&2
        return 1
    }

    while IFS= read -r entry; do
        normalized="${entry#./}"

        [[ -n "$normalized" ]] || continue

        if [[ "$normalized" == /* ]]; then
            echo "Client data archive contains absolute path: $entry" >&2
            return 1
        fi

        case "/$normalized/" in
            */../*)
                echo "Client data archive contains parent traversal: $entry" >&2
                return 1
                ;;
        esac
    done < <(tar -tzf "$archive")

    while IFS= read -r verbose; do
        [[ -n "$verbose" ]] || continue

        node_type="${verbose:0:1}"

        case "$node_type" in
            -|d)
                ;;
            *)
                echo "Client data archive contains unsupported node type '$node_type': $verbose" >&2
                return 1
                ;;
        esac
    done < <(LC_ALL=C tar -tvzf "$archive")
}

trap cleanup EXIT

DESKTOP_UID=""
DESKTOP_GID=""

resolve_desktop_identity() {
    if [[ -n "${NEEBLES_DESKTOP_UID:-}" || -n "${NEEBLES_DESKTOP_GID:-}" ]]; then
        [[ -n "${NEEBLES_DESKTOP_UID:-}" && -n "${NEEBLES_DESKTOP_GID:-}" ]] || {
            echo "NEEBLES_DESKTOP_UID and NEEBLES_DESKTOP_GID must be provided together." >&2
            exit 1
        }

        DESKTOP_UID="$NEEBLES_DESKTOP_UID"
        DESKTOP_GID="$NEEBLES_DESKTOP_GID"

    elif [[ -n "${PKEXEC_UID:-}" ]]; then
        DESKTOP_UID="$PKEXEC_UID"
        DESKTOP_GID="$(
            getent passwd "$DESKTOP_UID" |
            awk -F: 'NR == 1 { print $4 }'
        )"

    elif [[ -n "${SUDO_UID:-}" ]]; then
        DESKTOP_UID="$SUDO_UID"
        DESKTOP_GID="$(
            getent passwd "$DESKTOP_UID" |
            awk -F: 'NR == 1 { print $4 }'
        )"

    else
        DESKTOP_UID="$(id -u)"
        DESKTOP_GID="$(id -g)"
    fi

    [[ "$DESKTOP_UID" =~ ^[0-9]+$ ]] || {
        echo "Invalid desktop UID: $DESKTOP_UID" >&2
        exit 1
    }

    [[ "$DESKTOP_GID" =~ ^[0-9]+$ ]] || {
        echo "Invalid desktop GID: $DESKTOP_GID" >&2
        exit 1
    }
}

resolve_desktop_identity

if [[ -z "$DESTDIR" && ${EUID} -ne 0 ]]; then
    echo "This installer must run as root." >&2
    exit 1
fi

if [[ $# -ne 4 ]]; then
    echo "Usage: $0 <neebles-backend-binary> <neebles-ui-binary> <client-data.tar.gz|client-data-directory> <neebles-auth-agent-binary>" >&2
    exit 1
fi

BACKEND_SOURCE="$1"
UI_SOURCE="$2"
CLIENT_DATA_ARCHIVE="$3"
AUTH_AGENT_SOURCE="$4"
CLIENT_DATA_SOURCE=""

progress 5
status_key "installer.progress.authorization_accepted"

progress 10
status_key "installer.progress.validating_payload"

for item in \
    "$BACKEND_SOURCE" \
    "$UI_SOURCE" \
    "$AUTH_AGENT_SOURCE"
do
    [[ -f "$item" && -r "$item" ]] || {
        echo "Required binary payload is missing or unreadable: $item" >&2
        exit 1
    }
done

if [[ -f "$CLIENT_DATA_ARCHIVE" ]]; then
    validate_client_archive "$CLIENT_DATA_ARCHIVE"

    CLIENT_DATA_TMP="$(mktemp -d)"

    tar \
        --no-same-owner \
        --no-same-permissions \
        -xzf "$CLIENT_DATA_ARCHIVE" \
        -C "$CLIENT_DATA_TMP"

    CLIENT_DATA_SOURCE="$CLIENT_DATA_TMP"

elif [[ -d "$CLIENT_DATA_ARCHIVE" ]]; then
    CLIENT_DATA_SOURCE="$CLIENT_DATA_ARCHIVE"

else
    echo "Client data payload not found: $CLIENT_DATA_ARCHIVE" >&2
    exit 1
fi

REQUIRED_ROOTS=(
    assets
    languages
    config
    launcher
    spacer
    notifications
    systemd
)

for root in "${REQUIRED_ROOTS[@]}"; do
    [[ -d "$CLIENT_DATA_SOURCE/$root" ]] || {
        echo "Client data is missing required root: $root" >&2
        exit 1
    }

    INVALID_NODE="$(
        find "$CLIENT_DATA_SOURCE/$root" \
            ! -type d \
            ! -type f \
            -print \
            -quit
    )"

    if [[ -n "$INVALID_NODE" ]]; then
        echo "Client data contains unsupported filesystem node: $INVALID_NODE" >&2
        exit 1
    fi
done

REQUIRED_FILES=(
    assets/branding/neebles-boss-launcher-icon.png
    languages/manifest.json
    config/defaults.json
    launcher/metadata.json
    launcher/contents/ui/main.qml
    spacer/metadata.json
    spacer/contents/ui/main.qml
    systemd/neebles-tray-manager.service
    systemd/neebles-stage0.service
    systemd/neebles-runtime.service
)

for item in "${REQUIRED_FILES[@]}"; do
    [[ -f "$CLIENT_DATA_SOURCE/$item" ]] || {
        echo "Client data is missing required file: $item" >&2
        exit 1
    }
done

for path in "$INSTALL_ROOT" "$CLIENT_ROOT" "$MODULES_DIR" "$SHARED_DIR"; do
    if [[ -L "$path" ]]; then
        echo "Refusing symlinked installation path: $path" >&2
        exit 1
    fi
done

progress 16
status_key "installer.progress.checking_dependencies"

if [[ -z "$DESTDIR" ]] && command -v apt >/dev/null 2>&1; then
    BOSS_RUNTIME_PACKAGES=(
        git
        curl
        libnotify-bin
        libqt6quick6
        liblayershellqtinterface6
    )

    MISSING=()

    command -v git >/dev/null 2>&1 || MISSING+=(git)
    command -v curl >/dev/null 2>&1 || MISSING+=(curl)
    command -v notify-send >/dev/null 2>&1 || MISSING+=(libnotify-bin)

    dpkg-query -W -f='${Status}' libqt6quick6 2>/dev/null \
        | grep -q "install ok installed" \
        || MISSING+=(libqt6quick6)

    dpkg-query -W -f='${Status}' liblayershellqtinterface6 2>/dev/null \
        | grep -q "install ok installed" \
        || MISSING+=(liblayershellqtinterface6)

    if (( ${#MISSING[@]} > 0 )); then
        apt install -y "${MISSING[@]}"
    fi

    apt-mark manual "${BOSS_RUNTIME_PACKAGES[@]}"
fi

progress 25
status_key "installer.progress.creating_structure"

install -d -m 0755 \
    "$INSTALL_ROOT" \
    "$MODULES_DIR" \
    "$SHARED_DIR" \
    "$TMP_DIR"

install -d -m 0700 "$SETTINGS_DIR"
install -d -m 0755 "$SHARED_DIR/cache/installers"

if [[ -z "$DESTDIR" ]]; then
    chown -R "$DESKTOP_UID:$DESKTOP_GID" "$SETTINGS_DIR"

    find "$SETTINGS_DIR" -type d -exec chmod 0700 {} +
    find "$SETTINGS_DIR" -type f -exec chmod 0600 {} +
fi

CLIENT_STAGE="$(mktemp -d "$TMP_DIR/client-install.XXXXXX")"
chmod 0755 "$CLIENT_STAGE"

install -d -m 0755 \
    "$CLIENT_STAGE/bin" \
    "$CLIENT_STAGE/backend" \
    "$CLIENT_STAGE/ui" \
    "$CLIENT_STAGE/auth"

copy_data_tree() {
    local source="$1"
    local destination="$2"

    rm -rf "$destination"
    install -d -m 0755 "$destination"

    cp -R \
        --no-preserve=ownership,mode,timestamps \
        "$source/." \
        "$destination/"

    find "$destination" -type d -exec chmod 0755 {} +
    find "$destination" -type f -exec chmod 0644 {} +
}

progress 38
status_key "installer.progress.installing_backend"

install -m 0755 \
    "$BACKEND_SOURCE" \
    "$CLIENT_STAGE/backend/neebles-backend"

progress 44
status_key "installer.progress.installing_auth_agent"

install -m 0755 \
    "$AUTH_AGENT_SOURCE" \
    "$CLIENT_STAGE/auth/neebles-auth-agent"

progress 50
status_key "installer.progress.installing_ui"

install -m 0755 \
    "$UI_SOURCE" \
    "$CLIENT_STAGE/ui/neebles-ui"

progress 66
status_key "installer.progress.installing_client_data"

copy_data_tree "$CLIENT_DATA_SOURCE/assets" "$CLIENT_STAGE/assets"
copy_data_tree "$CLIENT_DATA_SOURCE/languages" "$CLIENT_STAGE/languages"
copy_data_tree "$CLIENT_DATA_SOURCE/config" "$CLIENT_STAGE/config"
copy_data_tree "$CLIENT_DATA_SOURCE/launcher" "$CLIENT_STAGE/launcher"
copy_data_tree "$CLIENT_DATA_SOURCE/spacer" "$CLIENT_STAGE/spacer"
copy_data_tree "$CLIENT_DATA_SOURCE/notifications" "$CLIENT_STAGE/notifications"
copy_data_tree "$CLIENT_DATA_SOURCE/applications" "$CLIENT_STAGE/applications"

progress 75
status_key "installer.progress.creating_entrypoint"

ln -sfnT \
    ../backend/neebles-backend \
    "$CLIENT_STAGE/bin/neebles"

if [[ -z "$DESTDIR" ]]; then
    chown -R root:root "$CLIENT_STAGE"
fi

"$CLIENT_STAGE/bin/neebles" --version

if [[ -e "$CLIENT_ROOT" ]]; then
    CLIENT_BACKUP="$(mktemp -d "$TMP_DIR/client-backup.XXXXXX")"
    rmdir "$CLIENT_BACKUP"
    mv "$CLIENT_ROOT" "$CLIENT_BACKUP"
fi

mv "$CLIENT_STAGE" "$CLIENT_ROOT"
CLIENT_STAGE=""
CLIENT_SWAPPED=1

GLOBAL_BACKUP_ROOT="$(mktemp -d "$TMP_DIR/global-backup.XXXXXX")"
chmod 0700 "$GLOBAL_BACKUP_ROOT"

backup_global_path "$GLOBAL_BIN" "global-bin"
backup_global_path "$GLOBAL_ICON" "global-icon"
backup_global_path "$GLOBAL_BOSS_ICON" "global-boss-icon"
backup_global_path "$GLOBAL_DESKTOP" "global-desktop"
backup_global_path "$SYSTEMD_SERVICE" "systemd-service"
backup_global_path "$TRAY_HOST_SERVICE" "tray-host-service"
backup_global_path "$SYSTEMD_WANTS/neebles-tray-manager.service" "systemd-wants"
backup_global_path "$SYSTEMD_WANTS/neebles-tray-host.service" "tray-host-wants"
backup_global_path "$STAGE0_SERVICE" "stage0-service"
backup_global_path "$RUNTIME_SERVICE" "runtime-service"
backup_global_path "$RUNTIME_WANTS/neebles-stage0.service" "stage0-wants"
backup_global_path "$RUNTIME_WANTS/neebles-runtime.service" "runtime-wants"
backup_global_path "$RUNTIME_ENV" "runtime-env"
backup_global_path "$PLASMA_LAUNCHER" "plasma-launcher"
backup_global_path "$PLASMA_SPACER" "plasma-spacer"

install -d -m 0755 "$(dirname "$GLOBAL_BIN")"

ln -sfnT \
    "$NEEBLES_ROOT/client/bin/neebles" \
    "$GLOBAL_BIN"

install -d -m 0755 "$(dirname "$GLOBAL_ICON")"

install -m 0644 \
    "$ASSETS_DIR/branding/neebles-boss-launcher-icon.png" \
    "$GLOBAL_ICON"

install -m 0644 \
    "$ASSETS_DIR/branding/neebles-boss-icon.png" \
    "$GLOBAL_BOSS_ICON"

install -d -m 0755 "$(dirname "$GLOBAL_DESKTOP")"

install -m 0644 \
    "$APPLICATIONS_DIR/org.neebles.Boss.desktop" \
    "$GLOBAL_DESKTOP"

progress 80
status_key "installer.progress.installing_tray_manager"

install -d -m 0755 "$(dirname "$RUNTIME_ENV")"

cat > "$RUNTIME_ENV" <<EOF
NEEBLES_DESKTOP_UID=$DESKTOP_UID
NEEBLES_DESKTOP_GID=$DESKTOP_GID
EOF

chmod 0644 "$RUNTIME_ENV"

if [[ -z "$DESTDIR" ]]; then
    chown root:root "$RUNTIME_ENV"
fi

install -d -m 0755 "$(dirname "$RUNTIME_SERVICE")"

install -m 0644 \
    "$CLIENT_DATA_SOURCE/systemd/neebles-stage0.service" \
    "$STAGE0_SERVICE"

install -m 0644 \
    "$CLIENT_DATA_SOURCE/systemd/neebles-runtime.service" \
    "$RUNTIME_SERVICE"

install -d -m 0755 "$RUNTIME_WANTS"

ln -sfnT \
    /usr/lib/systemd/system/neebles-stage0.service \
    "$RUNTIME_WANTS/neebles-stage0.service"

ln -sfnT \
    /usr/lib/systemd/system/neebles-runtime.service \
    "$RUNTIME_WANTS/neebles-runtime.service"

install -d -m 0755 "$(dirname "$SYSTEMD_SERVICE")"

install -m 0644 \
    "$CLIENT_DATA_SOURCE/systemd/neebles-tray-manager.service" \
    "$SYSTEMD_SERVICE"

install -m 0644 \
    "$CLIENT_DATA_SOURCE/systemd/neebles-tray-host.service" \
    "$TRAY_HOST_SERVICE"

install -d -m 0755 "$SYSTEMD_WANTS"

ln -sfnT \
    /usr/lib/systemd/user/neebles-tray-manager.service \
    "$SYSTEMD_WANTS/neebles-tray-manager.service"

ln -sfnT \
    /usr/lib/systemd/user/neebles-tray-host.service \
    "$SYSTEMD_WANTS/neebles-tray-host.service"


progress 88
status_key "installer.progress.installing_launcher"

rm -rf "$PLASMA_LAUNCHER"
install -d -m 0755 "$PLASMA_LAUNCHER"
cp -R --no-preserve=ownership,mode,timestamps \
    "$LAUNCHER_DIR/." \
    "$PLASMA_LAUNCHER/"

find "$PLASMA_LAUNCHER" -type d -exec chmod 0755 {} +
find "$PLASMA_LAUNCHER" -type f -exec chmod 0644 {} +

status_key "installer.progress.installing_spacer"

rm -rf "$PLASMA_SPACER"
install -d -m 0755 "$PLASMA_SPACER"
cp -R --no-preserve=ownership,mode,timestamps \
    "$SPACER_DIR/." \
    "$PLASMA_SPACER/"

find "$PLASMA_SPACER" -type d -exec chmod 0755 {} +
find "$PLASMA_SPACER" -type f -exec chmod 0644 {} +

if [[ -n "${NEEBLES_INSTALL_TEST_FAIL_AFTER_GLOBALS:-}" ]]; then
    if [[ -z "$DESTDIR" ]]; then
        echo "NEEBLES_INSTALL_TEST_FAIL_AFTER_GLOBALS is only permitted with DESTDIR." >&2
        exit 1
    fi

    echo "N.E.E.B.L.E.S.: forced packaging rollback test failure." >&2
    exit 97
fi

progress 94
status_key "installer.progress.verifying_backend"

"$BIN_DIR/neebles" --version

[[ "$(readlink "$BIN_DIR/neebles")" == "../backend/neebles-backend" ]] || {
    echo "Installed Boss entrypoint symlink is invalid." >&2
    exit 1
}

[[ "$(readlink "$GLOBAL_BIN")" == "$NEEBLES_ROOT/client/bin/neebles" ]] || {
    echo "Global Boss command symlink is invalid." >&2
    exit 1
}

cmp -s \
    "$CLIENT_DATA_SOURCE/systemd/neebles-tray-manager.service" \
    "$SYSTEMD_SERVICE" \
    || {
        echo "Installed Tray Manager service does not match payload." >&2
        exit 1
    }

cmp -s \
    "$CLIENT_DATA_SOURCE/systemd/neebles-tray-host.service" \
    "$TRAY_HOST_SERVICE" \
    || {
        echo "Installed Tray Host service does not match payload." >&2
        exit 1
    }

cmp -s \
    "$CLIENT_DATA_SOURCE/systemd/neebles-stage0.service" \
    "$STAGE0_SERVICE" \
    || {
        echo "Installed Stage0 service does not match payload." >&2
        exit 1
    }

cmp -s \
    "$CLIENT_DATA_SOURCE/systemd/neebles-runtime.service" \
    "$RUNTIME_SERVICE" \
    || {
        echo "Installed Boss Runtime service does not match payload." >&2
        exit 1
    }

if [[ -z "$DESTDIR" ]]; then
    systemctl daemon-reload

    systemctl enable neebles-stage0.service
    systemctl enable neebles-runtime.service

    systemctl restart neebles-stage0.service
    systemctl restart neebles-runtime.service

    systemctl is-active --quiet neebles-runtime.service || {
        echo "N.E.E.B.L.E.S. Boss Runtime did not start correctly." >&2
        exit 1
    }
fi


CLIENT_SWAPPED=0
GLOBAL_PATHS=()
GLOBAL_BACKUPS=()
GLOBAL_EXISTED=()

if [[ -n "$GLOBAL_BACKUP_ROOT" && -d "$GLOBAL_BACKUP_ROOT" ]]; then
    rm -rf "$GLOBAL_BACKUP_ROOT"
    GLOBAL_BACKUP_ROOT=""
fi

if [[ -n "$CLIENT_BACKUP" && -e "$CLIENT_BACKUP" ]]; then
    rm -rf "$CLIENT_BACKUP"
    CLIENT_BACKUP=""
fi

progress 100
status_key "installer.progress.completed"

echo "Installed backend: $BACKEND_DIR/neebles-backend"
echo "Installed UI: $UI_DIR/neebles-ui"
echo "OS entrypoint: $BIN_DIR/neebles"
echo "Global command: $GLOBAL_BIN"

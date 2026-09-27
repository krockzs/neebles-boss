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
TRAY_HOST_DIR="$CLIENT_ROOT/tray-host"
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
GLOBAL_TRAY_ICON="${DESTDIR}/usr/share/icons/hicolor/256x256/apps/neebles-boss-tray-icon.png"
GLOBAL_INSTALLER_ICON="${DESTDIR}/usr/share/icons/hicolor/256x256/apps/neebles-installer-icon.png"
GLOBAL_DESKTOP="${DESTDIR}/usr/share/applications/org.neebles.Boss.desktop"
GLOBAL_INSTALLER_DESKTOP="${DESTDIR}/usr/share/applications/org.neebles.Installer.desktop"
SYSTEMD_SERVICE="${DESTDIR}/usr/lib/systemd/user/neebles-tray-manager.service"
TRAY_HOST_SERVICE="${DESTDIR}/usr/lib/systemd/user/neebles-tray-host.service"
TRAY_SNI_HOST_SERVICE="${DESTDIR}/usr/lib/systemd/user/neebles-tray-sni-host.service"
RUNTIME_SERVICE="${DESTDIR}/usr/lib/systemd/system/neebles-runtime.service"
EXTERNAL_SOCKET="${DESTDIR}/usr/lib/systemd/system/neebles-external.socket"
EXTERNAL_SERVICE="${DESTDIR}/usr/lib/systemd/system/neebles-external.service"
RUNTIME_WANTS="${DESTDIR}/etc/systemd/system/multi-user.target.wants"
SOCKET_WANTS="${DESTDIR}/etc/systemd/system/sockets.target.wants"
RUNTIME_ENV="${DESTDIR}/etc/neebles/runtime.env"
EXTERNAL_SOCKET_DROPIN_DIR="${DESTDIR}/etc/systemd/system/neebles-external.socket.d"
EXTERNAL_SOCKET_DROPIN="$EXTERNAL_SOCKET_DROPIN_DIR/owner.conf"
PLASMA_LAUNCHER="${DESTDIR}/usr/share/plasma/plasmoids/org.neebles.launcher"
PLASMA_SPACER="${DESTDIR}/usr/share/plasma/plasmoids/org.neebles.spacer"
BOSS_EVENTS_QML="${DESTDIR}/usr/lib/x86_64-linux-gnu/qt6/qml/NEEBLES/BossEvents"

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
DESKTOP_USER=""
DESKTOP_GROUP=""

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

    DESKTOP_USER="$(
        getent passwd "$DESKTOP_UID" |
            awk -F: 'NR == 1 { print $1 }'
    )"

    DESKTOP_GROUP="$(
        getent group "$DESKTOP_GID" |
            awk -F: 'NR == 1 { print $1 }'
    )"

    [[ -n "$DESKTOP_USER" ]] || {
        echo "Could not resolve desktop user for UID $DESKTOP_UID." >&2
        exit 1
    }

    [[ -n "$DESKTOP_GROUP" ]] || {
        echo "Could not resolve desktop group for GID $DESKTOP_GID." >&2
        exit 1
    }
}

resolve_desktop_identity

if [[ -z "$DESTDIR" && ${EUID} -ne 0 ]]; then
    echo "This installer must run as root." >&2
    exit 1
fi

if [[ $# -ne 6 && $# -ne 7 ]]; then
    echo "Usage: $0 <neebles-backend-binary> <neebles-ui-binary> <client-data.tar.gz|client-data-directory> <neebles-auth-agent-binary> <runtime-resolver> <runtime-manifest> [authority-supply]" >&2
    exit 1
fi

BACKEND_SOURCE="$1"
UI_SOURCE="$2"
CLIENT_DATA_ARCHIVE="$3"
AUTH_AGENT_SOURCE="$4"
RUNTIME_RESOLVER_SOURCE="$5"
RUNTIME_MANIFEST_SOURCE="$6"
AUTHORITY_SUPPLY_SOURCE=""
CLIENT_DATA_SOURCE=""

if [[ $# -eq 7 ]]; then
    AUTHORITY_SUPPLY_SOURCE="$7"

    [[ "$AUTHORITY_SUPPLY_SOURCE" == /* ]] || {
        echo "AuthoritySupply path must be absolute." >&2
        exit 1
    }

    [[ "$AUTHORITY_SUPPLY_SOURCE" =~ ^/[A-Za-z0-9._/+:-]+$ ]] || {
        echo "AuthoritySupply path contains unsupported characters." >&2
        exit 1
    }

    [[ -f "$AUTHORITY_SUPPLY_SOURCE" && -r "$AUTHORITY_SUPPLY_SOURCE" ]] || {
        echo "AuthoritySupply path is missing or unreadable: $AUTHORITY_SUPPLY_SOURCE" >&2
        exit 1
    }
fi

progress 5
status_key "installer.progress.authorization_accepted"

progress 10
status_key "installer.progress.validating_payload"

for item in \
    "$BACKEND_SOURCE" \
    "$UI_SOURCE" \
    "$AUTH_AGENT_SOURCE" \
    "$RUNTIME_RESOLVER_SOURCE"
do
    [[ -f "$item" && -r "$item" ]] || {
        echo "Required binary payload is missing or unreadable: $item" >&2
        exit 1
    }
done

[[ -x "$RUNTIME_RESOLVER_SOURCE" ]] || {
    echo "Runtime resolver is not executable: $RUNTIME_RESOLVER_SOURCE" >&2
    exit 1
}

[[ -f "$RUNTIME_MANIFEST_SOURCE" && -r "$RUNTIME_MANIFEST_SOURCE" ]] || {
    echo "Runtime manifest is missing or unreadable: $RUNTIME_MANIFEST_SOURCE" >&2
    exit 1
}

RUNTIME_ROOT_RELATIVE="$(
    python3 - "$RUNTIME_MANIFEST_SOURCE" <<'PY_RUNTIME_ROOT'
from pathlib import Path, PurePosixPath
import json
import sys

manifest = Path(
    sys.argv[1]
)

try:
    data = json.loads(
        manifest.read_text(
            encoding="utf-8"
        )
    )
except Exception as error:
    raise SystemExit(
        "invalid runtime manifest JSON: "
        + str(error)
    )

root_value = data.get(
    "root"
)

if not isinstance(
    root_value,
    str,
):
    raise SystemExit(
        "runtime manifest root is not a string"
    )

root_value = root_value.strip()

if not root_value:
    raise SystemExit(
        "runtime manifest root is empty"
    )

root_reference = PurePosixPath(
    root_value
)

if root_reference.is_absolute():
    raise SystemExit(
        "runtime manifest root must be relative"
    )

if any(
    part in {
        "",
        ".",
        "..",
    }
    for part in root_reference.parts
):
    raise SystemExit(
        "runtime manifest root contains unsafe path components"
    )

authority = manifest.parent.resolve(
    strict=True
)

candidate = (
    manifest.parent
    / Path(
        *root_reference.parts
    )
)

try:
    resolved = candidate.resolve(
        strict=True
    )

    resolved.relative_to(
        authority
    )
except Exception as error:
    raise SystemExit(
        "runtime manifest root escapes authority: "
        + str(error)
    )

if not resolved.is_dir():
    raise SystemExit(
        "runtime manifest root is not a directory"
    )

print(
    root_reference.as_posix()
)
PY_RUNTIME_ROOT
)" || {
    echo "Runtime manifest does not describe a valid local authority root." >&2
    exit 1
}

[[ -n "$RUNTIME_ROOT_RELATIVE" ]] || {
    echo "Runtime manifest resolved an empty authority root." >&2
    exit 1
}

RUNTIME_AUTHORITY_SOURCE="${RUNTIME_MANIFEST_SOURCE%/*}"
RUNTIME_ROOTFS_SOURCE="$RUNTIME_AUTHORITY_SOURCE/$RUNTIME_ROOT_RELATIVE"

[[ -d "$RUNTIME_ROOTFS_SOURCE" ]] || {
    echo "Runtime authority root is missing: $RUNTIME_ROOTFS_SOURCE" >&2
    exit 1
}

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
    applications
    systemd
    runtime
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
    systemd/neebles-runtime.service
    runtime/tray-host/neebles-tray-host
    runtime/qml/NEEBLES/BossEvents/libneebles-launcher-events.so
    runtime/qml/NEEBLES/BossEvents/libneebles-launcher-eventsplugin.so
    runtime/qml/NEEBLES/BossEvents/neebles-launcher-events.qmltypes
    runtime/qml/NEEBLES/BossEvents/qmldir
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


progress 25
status_key "installer.progress.creating_structure"

install -d -m 0755 \
    "$INSTALL_ROOT" \
    "$MODULES_DIR" \
    "$SHARED_DIR" \
    "$TMP_DIR"

install -d -m 0700 "$SETTINGS_DIR"

if [[ -z "$DESTDIR" ]]; then
    chown -R "$DESKTOP_UID:$DESKTOP_GID" "$SETTINGS_DIR"

    find "$SETTINGS_DIR" -type d -exec chmod 0700 {} +
    find "$SETTINGS_DIR" -type f -exec chmod 0600 {} +
fi

DEACTIVATE_FILE="$SHARED_DIR/deactivate.json"

if [[ -L "$DEACTIVATE_FILE" ]]; then
    echo "Refusing symlinked deactivate registry: $DEACTIVATE_FILE" >&2
    exit 1
fi

if [[ ! -e "$DEACTIVATE_FILE" ]]; then
    printf '{}\n' > "$DEACTIVATE_FILE"
elif [[ ! -f "$DEACTIVATE_FILE" ]]; then
    echo "Deactivate registry is not a regular file: $DEACTIVATE_FILE" >&2
    exit 1
fi

chmod 0600 "$DEACTIVATE_FILE"

CLIENT_STAGE="$(mktemp -d "$TMP_DIR/client-install.XXXXXX")"
chmod 0755 "$CLIENT_STAGE"

install -d -m 0755 \
    "$CLIENT_STAGE/bin" \
    "$CLIENT_STAGE/backend" \
    "$CLIENT_STAGE/ui" \
    "$CLIENT_STAGE/auth" \
    "$CLIENT_STAGE/tray-host"

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

install -m 0755 \
    "$CLIENT_DATA_SOURCE/runtime/tray-host/neebles-tray-host" \
    "$CLIENT_STAGE/tray-host/neebles-tray-host"

install -d -m 0755 "$CLIENT_STAGE/runtime/boss"

install -m 0755 "$RUNTIME_RESOLVER_SOURCE" "$CLIENT_STAGE/runtime/neebles-runtime-resolve"

install -m 0644 "$RUNTIME_MANIFEST_SOURCE" "$CLIENT_STAGE/runtime/boss/domestic-runtime.json"

RUNTIME_ROOTFS_STAGE="$CLIENT_STAGE/runtime/boss/$RUNTIME_ROOT_RELATIVE"

install -d -m 0755 "$(dirname -- "$RUNTIME_ROOTFS_STAGE")"

cp -a --     "$RUNTIME_ROOTFS_SOURCE"     "$RUNTIME_ROOTFS_STAGE"

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

cmp -s "$RUNTIME_RESOLVER_SOURCE" "$CLIENT_STAGE/runtime/neebles-runtime-resolve" || {
    echo "Installed runtime resolver differs from bootstrap authority." >&2
    exit 1
}

cmp -s "$RUNTIME_MANIFEST_SOURCE" "$CLIENT_STAGE/runtime/boss/domestic-runtime.json" || {
    echo "Installed runtime manifest differs from bootstrap authority." >&2
    exit 1
}

[[ -d "$RUNTIME_ROOTFS_STAGE" ]] || {
    echo "Installed runtime authority root is missing." >&2
    exit 1
}

[[ -x "$CLIENT_STAGE/runtime/neebles-runtime-resolve" ]] || {
    echo "Installed runtime resolver lost executable permission." >&2
    exit 1
}

"$CLIENT_STAGE/runtime/neebles-runtime-resolve"     --manifest "$CLIENT_STAGE/runtime/boss/domestic-runtime.json"     --world boss.pkexec     --category executable     >/dev/null     || {
        echo "Installed runtime authority cannot resolve boss.pkexec." >&2
        exit 1
    }

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
backup_global_path "$GLOBAL_TRAY_ICON" "global-tray-icon"
backup_global_path "$GLOBAL_INSTALLER_ICON" "global-installer-icon"
backup_global_path "$GLOBAL_DESKTOP" "global-desktop"
backup_global_path "$GLOBAL_INSTALLER_DESKTOP" "global-installer-desktop"
backup_global_path "$SYSTEMD_SERVICE" "systemd-service"
backup_global_path "$TRAY_HOST_SERVICE" "tray-host-service"
backup_global_path "$TRAY_SNI_HOST_SERVICE" "tray-sni-host-service"
backup_global_path "$RUNTIME_SERVICE" "runtime-service"
backup_global_path "$EXTERNAL_SOCKET" "external-socket"
backup_global_path "$EXTERNAL_SERVICE" "external-service"
backup_global_path "$RUNTIME_WANTS/neebles-runtime.service" "runtime-wants"
backup_global_path "$SOCKET_WANTS/neebles-external.socket" "external-socket-wants"
backup_global_path "$EXTERNAL_SOCKET_DROPIN_DIR" "external-socket-dropin"
backup_global_path "$RUNTIME_ENV" "runtime-env"
backup_global_path "$PLASMA_LAUNCHER" "plasma-launcher"
backup_global_path "$PLASMA_SPACER" "plasma-spacer"
backup_global_path "$BOSS_EVENTS_QML" "boss-events-qml"

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

install -m 0644 \
    "$ASSETS_DIR/branding/neebles-boss-tray-icon.png" \
    "$GLOBAL_TRAY_ICON"

install -m 0644 \
    "$ASSETS_DIR/branding/neebles-installer-icon.png" \
    "$GLOBAL_INSTALLER_ICON"

install -d -m 0755 "$(dirname "$GLOBAL_DESKTOP")"

install -m 0644 \
    "$APPLICATIONS_DIR/org.neebles.Boss.desktop" \
    "$GLOBAL_DESKTOP"

install -m 0644 \
    "$APPLICATIONS_DIR/org.neebles.Installer.desktop" \
    "$GLOBAL_INSTALLER_DESKTOP"

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

install -d -m 0755 "$EXTERNAL_SOCKET_DROPIN_DIR"

cat > "$EXTERNAL_SOCKET_DROPIN" <<EOF
[Socket]
SocketUser=$DESKTOP_USER
SocketGroup=$DESKTOP_GROUP
EOF

chmod 0644 "$EXTERNAL_SOCKET_DROPIN"

if [[ -z "$DESTDIR" ]]; then
    chown root:root "$EXTERNAL_SOCKET_DROPIN"
fi

install -d -m 0755 "$(dirname "$RUNTIME_SERVICE")"

if [[ -z "$AUTHORITY_SUPPLY_SOURCE" ]]; then
    install -m 0644 \
        "$CLIENT_DATA_SOURCE/systemd/neebles-runtime.service" \
        "$RUNTIME_SERVICE"
else
    SOURCE_RUNTIME_EXEC="ExecStart=/opt/neebles/client/backend/neebles-backend socket serve"

    grep -Fxq \
        "$SOURCE_RUNTIME_EXEC" \
        "$CLIENT_DATA_SOURCE/systemd/neebles-runtime.service" \
        || {
            echo "Boss Runtime service source does not expose the expected ExecStart contract." >&2
            exit 1
        }

    python3 - \
        "$CLIENT_DATA_SOURCE/systemd/neebles-runtime.service" \
        "$RUNTIME_SERVICE" \
        "$AUTHORITY_SUPPLY_SOURCE" \
        <<'PY_SERVICE'
from pathlib import Path
import sys

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
supply = sys.argv[3]

old = (
    "ExecStart=/opt/neebles/client/backend/"
    "neebles-backend socket serve"
)

new = (
    "ExecStart=/opt/neebles/client/backend/"
    "neebles-backend --authority-supply "
    + supply
    + " socket serve"
)

data = source.read_text()

if data.count(old) != 1:
    raise SystemExit(
        "runtime service ExecStart contract mismatch"
    )

destination.write_text(
    data.replace(
        old,
        new,
    )
)
PY_SERVICE

    chmod 0644 "$RUNTIME_SERVICE"
fi

install -m 0644 \
    "$CLIENT_DATA_SOURCE/systemd/neebles-external.socket" \
    "$EXTERNAL_SOCKET"

install -m 0644 \
    "$CLIENT_DATA_SOURCE/systemd/neebles-external.service" \
    "$EXTERNAL_SERVICE"

install -d -m 0755 "$RUNTIME_WANTS"
install -d -m 0755 "$SOCKET_WANTS"


ln -sfnT \
    /usr/lib/systemd/system/neebles-runtime.service \
    "$RUNTIME_WANTS/neebles-runtime.service"

ln -sfnT \
    /usr/lib/systemd/system/neebles-external.socket \
    "$SOCKET_WANTS/neebles-external.socket"

install -d -m 0755 "$(dirname "$SYSTEMD_SERVICE")"

install -m 0644 \
    "$CLIENT_DATA_SOURCE/systemd/neebles-tray-manager.service" \
    "$SYSTEMD_SERVICE"

install -m 0644 \
    "$CLIENT_DATA_SOURCE/systemd/neebles-tray-host.service" \
    "$TRAY_HOST_SERVICE"

install -m 0644 \
    "$CLIENT_DATA_SOURCE/systemd/neebles-tray-sni-host.service" \
    "$TRAY_SNI_HOST_SERVICE"


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

rm -rf "$BOSS_EVENTS_QML"
install -d -m 0755 "$BOSS_EVENTS_QML"

install -m 0644     "$CLIENT_DATA_SOURCE/runtime/qml/NEEBLES/BossEvents/libneebles-launcher-events.so"     "$BOSS_EVENTS_QML/libneebles-launcher-events.so"

install -m 0644     "$CLIENT_DATA_SOURCE/runtime/qml/NEEBLES/BossEvents/libneebles-launcher-eventsplugin.so"     "$BOSS_EVENTS_QML/libneebles-launcher-eventsplugin.so"

install -m 0644     "$CLIENT_DATA_SOURCE/runtime/qml/NEEBLES/BossEvents/neebles-launcher-events.qmltypes"     "$BOSS_EVENTS_QML/neebles-launcher-events.qmltypes"

install -m 0644     "$CLIENT_DATA_SOURCE/runtime/qml/NEEBLES/BossEvents/qmldir"     "$BOSS_EVENTS_QML/qmldir"

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
    "$CLIENT_DATA_SOURCE/systemd/neebles-tray-sni-host.service" \
    "$TRAY_SNI_HOST_SERVICE" \
    || {
        echo "Installed Tray SNI Host service does not match payload." >&2
        exit 1
    }

cmp -s     "$CLIENT_DATA_SOURCE/runtime/tray-host/neebles-tray-host"     "$TRAY_HOST_DIR/neebles-tray-host"     || {
        echo "Installed Qt Tray Host does not match payload." >&2
        exit 1
    }

for item in     libneebles-launcher-events.so     libneebles-launcher-eventsplugin.so     neebles-launcher-events.qmltypes     qmldir
do
    cmp -s         "$CLIENT_DATA_SOURCE/runtime/qml/NEEBLES/BossEvents/$item"         "$BOSS_EVENTS_QML/$item"         || {
            echo "Installed BossEvents QML runtime differs from payload: $item" >&2
            exit 1
        }
done


if [[ -z "$AUTHORITY_SUPPLY_SOURCE" ]]; then
    cmp -s \
        "$CLIENT_DATA_SOURCE/systemd/neebles-runtime.service" \
        "$RUNTIME_SERVICE" \
        || {
            echo "Installed Boss Runtime service does not match standalone payload." >&2
            exit 1
        }
else
    grep -Fxq \
        "ExecStart=/opt/neebles/client/backend/neebles-backend --authority-supply $AUTHORITY_SUPPLY_SOURCE socket serve" \
        "$RUNTIME_SERVICE" \
        || {
            echo "Installed Boss Runtime service did not preserve AuthoritySupply." >&2
            exit 1
        }

    if grep -Fq "authority-supply" "$RUNTIME_ENV"; then
        echo "AuthoritySupply leaked into runtime.env." >&2
        exit 1
    fi
fi

cmp -s \
    "$CLIENT_DATA_SOURCE/systemd/neebles-external.socket" \
    "$EXTERNAL_SOCKET" \
    || {
        echo "Installed External socket does not match payload." >&2
        exit 1
    }

cmp -s \
    "$CLIENT_DATA_SOURCE/systemd/neebles-external.service" \
    "$EXTERNAL_SERVICE" \
    || {
        echo "Installed External service does not match payload." >&2
        exit 1
    }

grep -Fxq "SocketUser=$DESKTOP_USER" "$EXTERNAL_SOCKET_DROPIN" || {
    echo "External socket user override is invalid." >&2
    exit 1
}

grep -Fxq "SocketGroup=$DESKTOP_GROUP" "$EXTERNAL_SOCKET_DROPIN" || {
    echo "External socket group override is invalid." >&2
    exit 1
}

if [[ -z "$DESTDIR" ]]; then
    systemctl daemon-reload

    systemctl enable neebles-external.socket
    systemctl enable neebles-runtime.service

    systemctl restart neebles-external.socket
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

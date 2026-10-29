#!/usr/bin/env bash
set -Eeuo pipefail
# Privileged deployment deliberately separate from Boss installation until topology is verified.
BASE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ $(id -u) -eq 0 ]] || { echo 'Run as root'; exit 1; }
[[ -e /opt/neebles/client/backend/neebles-backend ]] || { echo 'Boss backend missing'; exit 1; }
[[ $(stat -c %u /opt/neebles/client/backend/neebles-backend) -eq 0 ]] || { echo 'Boss ELF must be root owned'; exit 1; }
install -d -o root -g root -m 0755 /usr/lib/neebles/platform-operations /etc/neebles/platform-operations
install -o root -g root -m 0644 "$BASE/platform_provider.py" /usr/lib/neebles/platform-operations/provider.py
install -o root -g root -m 0644 "$BASE/neebles-platform-operations.socket" /etc/systemd/system/
install -o root -g root -m 0644 "$BASE/neebles-platform-operations.service" /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now neebles-platform-operations.socket
echo 'Provider socket installed. Operations deny by default until root-owned policy exists.'

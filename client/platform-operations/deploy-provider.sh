#!/usr/bin/env bash
set -Eeuo pipefail
# Privileged deployment deliberately separate from Boss installation until topology is verified.
BASE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ $(id -u) -eq 0 ]] || { echo 'Run as root'; exit 1; }
[[ -e /opt/neebles/client/backend/neebles-backend ]] || { echo 'Boss backend missing'; exit 1; }
[[ $(stat -c %u /opt/neebles/client/backend/neebles-backend) -eq 0 ]] || { echo 'Boss ELF must be root owned'; exit 1; }
# Explicit OS administrator action only. Does not install module policies.
# It does not grant anything merely because a module binary exists.
[[ -f /usr/lib/neebles/platform/authority/authority-supply.json ]] || {
    echo 'Missing OS-owned AuthoritySupply; aborting before changing services.' >&2
    exit 1
}
install -d -o root -g root -m 0755 \
    /usr/lib/neebles/platform-operations \
    /usr/lib/neebles/module-operations \
    /etc/neebles/platform-operations \
    /etc/neebles/platform-operations/approvals \
    /var/lib/neebles/platform-operations/journal
install -o root -g root -m 0644 "$BASE/platform_provider.py" /usr/lib/neebles/platform-operations/provider.py
install -o root -g root -m 0644 "$BASE/platform-authority-os.py" /usr/lib/neebles/platform-operations/platform-authority-os.py
install -o root -g root -m 0644 "$BASE/global-platform-authority.json" /usr/lib/neebles/platform/authority/neebles-platform-operation.json
install -o root -g root -m 0644 "$BASE/neebles-platform-operations.socket" /etc/systemd/system/
install -o root -g root -m 0644 "$BASE/neebles-platform-operations.service" /etc/systemd/system/
# The OS-owned registration is separately authenticated and preserves all grants.
/usr/bin/python3 -I /usr/lib/neebles/platform-operations/platform-authority-os.py register-global
systemctl daemon-reload
systemctl enable --now neebles-platform-operations.socket
echo 'OS platform provider ready. Existing Boss runtime must be restarted explicitly.'
echo 'Module operations remain deny-by-default until OS-approved root policy exists.'

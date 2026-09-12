#!/bin/bash
set -euo pipefail

SOURCE_DIR="${1:-client/launcher}"
PACKAGE_ID="org.neebles.launcher"

if [[ ! -f "$SOURCE_DIR/metadata.json" ]]; then
    echo "N.E.E.B.L.E.S.: launcher package not found at $SOURCE_DIR" >&2
    exit 1
fi

if command -v kpackagetool6 >/dev/null 2>&1; then
    if kpackagetool6 --type Plasma/Applet --show "$PACKAGE_ID" >/dev/null 2>&1; then
        kpackagetool6 --type Plasma/Applet --upgrade "$SOURCE_DIR"
    else
        kpackagetool6 --type Plasma/Applet --install "$SOURCE_DIR"
    fi
else
    echo "N.E.E.B.L.E.S.: kpackagetool6 is required to install the Plasma launcher." >&2
    exit 1
fi

echo "N.E.E.B.L.E.S. launcher package installed. Add it next to Plasma's Application Launcher on the panel."

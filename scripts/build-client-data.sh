#!/bin/bash
set -euo pipefail

OUTPUT="${1:-}"
SOURCE="${2:-client}"

if [[ -z "$OUTPUT" ]]; then
    echo "Usage: $0 <output.tar.gz> [client-source-directory]" >&2
    exit 1
fi

[[ -d "$SOURCE" ]] || {
    echo "Client source directory not found: $SOURCE" >&2
    exit 1
}

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
    [[ -d "$SOURCE/$root" ]] || {
        echo "Client source is missing required root: $root" >&2
        exit 1
    }
done

OUTPUT="$(realpath -m "$OUTPUT")"
SOURCE="$(realpath "$SOURCE")"

install -d -m 0755 "$(dirname "$OUTPUT")"

TMP="${OUTPUT}.tmp.$$"

cleanup() {
    rm -f "$TMP"
}

trap cleanup EXIT

tar \
    --sort=name \
    --mtime='@0' \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    --mode='u+rwX,go+rX,go-w' \
    -C "$SOURCE" \
    -cf - \
    "${REQUIRED_ROOTS[@]}" \
    | gzip -n > "$TMP"

mv "$TMP" "$OUTPUT"
trap - EXIT

echo "Built reproducible client data: $OUTPUT"

#!/bin/bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT"
cargo build

cmake -S ui/client -B ui/client/build -DCMAKE_BUILD_TYPE=Debug
cmake --build ui/client/build --parallel

cmake -S client/tray -B client/tray/build -DCMAKE_BUILD_TYPE=Debug
cmake --build client/tray/build --parallel

echo
echo "Built:"
echo "  $ROOT/target/debug/neebles-backend"
echo "  $ROOT/ui/client/build/neebles-ui"
echo "  $ROOT/client/tray/build/neebles-tray"

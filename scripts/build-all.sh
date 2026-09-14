#!/bin/bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT"
cargo build

cmake -S ui/client -B ui/client/build -DCMAKE_BUILD_TYPE=Debug
cmake --build ui/client/build --parallel

cmake -S ui/auth-agent -B ui/auth-agent/build -DCMAKE_BUILD_TYPE=Debug
cmake --build ui/auth-agent/build --parallel

cmake -S client/tray-host -B client/tray-host/build -DCMAKE_BUILD_TYPE=Debug
cmake --build client/tray-host/build --parallel

echo
echo "Built:"
echo "  $ROOT/target/debug/neebles-backend"
echo "  $ROOT/ui/client/build/neebles-ui"
echo "  $ROOT/ui/auth-agent/build/neebles-auth-agent"
echo "  $ROOT/client/tray-host/build/neebles-tray-host"

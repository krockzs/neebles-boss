#!/bin/bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT"

echo "Validating Boss language contract..."
"$ROOT/scripts/validate-languages.py"

echo
cargo build

cmake -S ui/client -B ui/client/build -DCMAKE_BUILD_TYPE=Debug
cmake --build ui/client/build --parallel

cmake -S ui/installer -B ui/installer/build -DCMAKE_BUILD_TYPE=Debug
cmake --build ui/installer/build --parallel

cmake -S ui/auth-agent -B ui/auth-agent/build -DCMAKE_BUILD_TYPE=Debug
cmake --build ui/auth-agent/build --parallel

echo
echo "Built:"
echo "  $ROOT/target/debug/neebles-backend"
echo "  $ROOT/ui/client/build/neebles-ui"
echo "  $ROOT/ui/installer/build/neebles-installer"
echo "  $ROOT/ui/auth-agent/build/neebles-auth-agent"

#!/bin/bash
# Checks every example's release WASM against the Stylus size limit with cargo-stylus.
# Runs against RPC_URL (a devnet) when set, otherwise against the Arbitrum Sepolia RPC.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ENDPOINT="${RPC_URL:-https://sepolia-rollup.arbitrum.io/rpc}"

cargo build --release --target wasm32-unknown-unknown

FAILED=0
for manifest in examples/*/Cargo.toml examples/*/*/Cargo.toml; do
  [ -f "$manifest" ] || continue
  name=$(grep -m1 '^name = ' "$manifest" | grep -oE '"[^"]+"' | tr -d '"')
  wasm="target/wasm32-unknown-unknown/release/${name//-/_}.wasm"
  [ -f "$wasm" ] || continue
  echo "checking $name"
  if ! cargo stylus check -e "$ENDPOINT" --wasm-file "$wasm"; then
    FAILED=1
  fi
  sleep 2
done
exit $FAILED

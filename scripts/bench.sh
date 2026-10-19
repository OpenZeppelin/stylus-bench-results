#!/bin/bash
# Gas benchmarks. Builds the examples for wasm, optimizes them with wasm-opt, and runs
# the benches against a Stylus SDK devnet.
#
# Environment:
#   RPC_URL         a running devnet; a fresh one is started for the run when unset.
#   BENCH_WASM_DIR  directory with the <name>.wasm files to benchmark; defaults to the
#                   release output of this workspace. Point it at another build to
#                   compare two versions on the same devnet.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [ -z "${RPC_URL:-}" ]; then
  RPC_URL="$(scripts/devnet.sh start)"
  export RPC_URL
  trap 'scripts/devnet.sh stop' EXIT
fi

cargo build --release --target wasm32-unknown-unknown

WASM_DIR="${BENCH_WASM_DIR:-$ROOT/target/wasm32-unknown-unknown/release}"
export BENCH_WASM_DIR="$WASM_DIR"

# Optimize every example's WASM for gas usage (binaryen 116, as the gas workflow pins).
for manifest in examples/*/Cargo.toml examples/*/*/Cargo.toml; do
  [ -f "$manifest" ] || continue
  name=$(grep -m1 '^name = ' "$manifest" | grep -oE '"[^"]+"' | tr -d '"')
  bin="${name//-/_}.wasm"
  if [ -f "$WASM_DIR/$bin" ]; then
    wasm-opt --enable-bulk-memory -O3 -o "$WASM_DIR/${bin%.wasm}_opt.wasm" "$WASM_DIR/$bin"
  fi
done

export DEPLOYER_ADDRESS=0xcEcba2F1DC234f70Dd89F2041029807F8D03A990
cargo run -p benches

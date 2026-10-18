#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

source "$SCRIPT_DIR/stylus-tools.sh"
require_cargo_stylus
require_docker_daemon

# The benchmark harness deploys to a fresh stylus-tools devnet and builds
# Wasm through cargo-stylus using each contract's Stylus.toml.
cargo run --locked -p benches

echo
echo "Gas measurements omit the 21,000 EVM base transaction fee."
echo "Each benchmark run starts a fresh SDK-managed Nitro devnet."
echo "Reports include uncached and cached execution; Nitro currently rejects the Binaryen 116 output."
echo
echo "Finished running benches!"

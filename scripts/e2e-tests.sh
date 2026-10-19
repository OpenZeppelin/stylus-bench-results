#!/bin/bash
# Runs the end-to-end tests of the examples against a Stylus SDK devnet.
#
# One devnet serves the whole run: set RPC_URL to reuse a running node, otherwise a
# devnet is started here and stopped at the end. Extra arguments are passed to
# `cargo test`, for example `scripts/e2e-tests.sh -p erc20-example`.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [ -z "${RPC_URL:-}" ]; then
  RPC_URL="$(scripts/devnet.sh start)"
  export RPC_URL
  trap 'scripts/devnet.sh stop' EXIT
fi

# The tests deploy the release WASM of each example.
cargo build --release --target wasm32-unknown-unknown

if [ "$#" -eq 0 ]; then
  PACKAGES=$(cargo metadata --format-version=1 --no-deps \
    | jq -r '.packages[] | select(.features | has("e2e")) | .name' \
    | sed 's/^/-p /' | tr '\n' ' ')
  # shellcheck disable=SC2086
  cargo test $PACKAGES --features e2e --tests
else
  cargo test --features e2e "$@"
fi

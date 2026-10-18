#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

source "$SCRIPT_DIR/stylus-tools.sh"
require_cargo_stylus

# `cargo stylus check` builds each cdylib with the pinned SDK toolchain, checks
# the size limit, and estimates the activation data fee against the configured
# RPC. This needs outbound access
# to Arbitrum Sepolia unless RPC_URL is set to another compatible endpoint. The
# integration-test job separately deploys and activates examples on devnet.
RPC_URL="${RPC_URL:-https://sepolia-rollup.arbitrum.io/rpc}"
contract_configs=()
while IFS= read -r config; do
  contract_configs+=("$config")
done < <(find examples -type f -name Stylus.toml -print | sort)
while IFS= read -r config; do
  contract_configs+=("$config")
done < <(find templates -type f -name Stylus.toml -print | sort)
if [[ ${#contract_configs[@]} -eq 0 ]]; then
  echo "No example Stylus.toml files were found." >&2
  exit 1
fi

for config in "${contract_configs[@]}"; do
  contract_dir="$(dirname "$config")"
  echo
  echo "Checking $(basename "$contract_dir")"
  (
    cd "$contract_dir"
    cargo stylus check --endpoint "$RPC_URL"
  )
  # Keep requests below the public Arbitrum Sepolia RPC rate limit.
  sleep 2
done

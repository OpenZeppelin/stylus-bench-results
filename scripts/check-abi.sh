#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(git rev-parse --show-toplevel)
cd "$ROOT_DIR"

while IFS= read -r -d '' manifest; do
  contract_dir=$(dirname "$manifest")
  echo "Exporting ABI from $contract_dir"
  (cd "$contract_dir" && cargo stylus export-abi)
done < <(find examples templates -name Stylus.toml -print0 | sort -z)

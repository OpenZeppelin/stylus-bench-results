#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(git -C "$(dirname "$(realpath "$0")")" rev-parse --show-toplevel)
cd "$ROOT_DIR"

# Pass a contract directory to resume a long serial run after a failure.
start_at=${1:-}
started=0
found_start=0
if [[ -z "$start_at" ]]; then
  started=1
fi

# Each example is its own Cargo package. Run every package with an integration
# test so SDK devnet tests are not silently skipped by workspace feature selection.
while IFS= read -r -d '' stylus_config; do
  contract_dir=$(dirname "$stylus_config")
  if [[ -n "$start_at" && "$contract_dir" == "$start_at" ]]; then
    started=1
    found_start=1
  fi
  ((started)) || continue

  test_dir="$contract_dir/tests"
  [[ -d "$test_dir" ]] || continue

  test_targets=()
  while IFS= read -r -d '' test_file; do
    test_name=$(basename "$test_file" .rs)
    test_targets+=(--test "$test_name")
  done < <(find "$test_dir" -maxdepth 1 -type f -name '*.rs' -print0 | sort -z)
  ((${#test_targets[@]})) || continue

  echo "Running devnet tests in $contract_dir"
  (
    cd "$contract_dir"
    cargo test --manifest-path "$ROOT_DIR/$contract_dir/Cargo.toml" \
      --features devnet \
      "${test_targets[@]}" -- --test-threads=1
  )
done < <(find examples templates -name Stylus.toml -print0 | sort -z)

if [[ -n "$start_at" && "$found_start" -eq 0 ]]; then
  echo "No Stylus.toml found for $start_at" >&2
  exit 1
fi

#!/usr/bin/env bash
set -euo pipefail

MYDIR=$(realpath "$(dirname "$0")")
cd "$MYDIR"
cd ..
source scripts/devnet.sh
source scripts/wasm-opt-wrapper/env.sh

cleanup() {
  local status=$?
  trap - EXIT INT TERM
  if [[ ${status} -ne 0 && -f "${DEVNET_LOG_FILE}" ]]; then
    printf '\nNitro devnode log (%s):\n' "${DEVNET_LOG_FILE}" >&2
    cat "${DEVNET_LOG_FILE}" >&2 || true
  fi
  stop_devnet
  exit "${status}"
}
trap cleanup EXIT INT TERM

start_clean_devnet

# No need to compile the benchmark harness with `--release`.
cargo run --locked -p benches
echo "This benchmarks measure gas execution cost,
 the 21000 EVM base gas fee is omitted."
echo
echo "To measure uncached contract gas correctly, run this on a clean Nitro devnode."
echo
echo "Finished running benches!"

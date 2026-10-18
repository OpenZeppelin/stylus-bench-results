#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"
source "${ROOT_DIR}/scripts/devnet.sh"
source "${ROOT_DIR}/scripts/wasm-opt-wrapper/env.sh"

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

E2E_PACKAGES=()
while IFS= read -r package; do
  E2E_PACKAGES+=("${package}")
done < <(
  find examples -name Cargo.toml -print0 |
    while IFS= read -r -d '' manifest; do
      if grep -Eq '^e2e[[:space:]]*=[[:space:]]*\[\]' "${manifest}" &&
        [[ -d "$(dirname "${manifest}")/tests" ]] &&
        find "$(dirname "${manifest}")/tests" -maxdepth 1 \
          -type f -name '*.rs' -print -quit | grep -q .; then
        awk '
          /^\[package\]/ { package = 1; next }
          /^\[/ { package = 0 }
          package && /^name[[:space:]]*=/ {
            gsub(/"/, "", $3)
            print $3
            exit
          }
        ' "${manifest}"
      fi
    done | sort -u
)

if [[ ${#E2E_PACKAGES[@]} -eq 0 ]]; then
  printf 'No example packages define the e2e feature.\n' >&2
  exit 1
fi

if [[ $# -eq 0 ]]; then
  # Each example gets a fresh SDK-managed Nitro node. Long suites otherwise
  # inherit fee-pricing state from earlier packages, which can make Stylus
  # activation estimates nondeterministic for fragmented contracts.
  for package in "${E2E_PACKAGES[@]}"; do
    printf '\nRunning %s E2E tests on a fresh SDK devnet.\n' "${package}"
    start_clean_devnet
    CARGO_PROFILE_DEV_PANIC=unwind cargo test --locked --features e2e \
      --package "${package}" --test '*' -- --test-threads=1
    stop_devnet
  done
else
  start_clean_devnet
  CARGO_PROFILE_DEV_PANIC=unwind cargo test --locked --features e2e "$@"
fi

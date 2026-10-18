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

start_clean_devnet

E2E_PACKAGES=()
while IFS= read -r package; do
  E2E_PACKAGES+=("${package}")
done < <(
  find examples -mindepth 2 -maxdepth 2 -name Cargo.toml -print0 |
    while IFS= read -r -d '' manifest; do
      if grep -Eq '^e2e[[:space:]]*=[[:space:]]*\[\]' "${manifest}"; then
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

PACKAGE_ARGS=()
for package in "${E2E_PACKAGES[@]}"; do
  PACKAGE_ARGS+=(--package "${package}")
done

if [[ $# -eq 0 ]]; then
  CARGO_PROFILE_DEV_PANIC=unwind cargo test --locked --features e2e \
    "${PACKAGE_ARGS[@]}" --test '*' -- --test-threads=1
else
  CARGO_PROFILE_DEV_PANIC=unwind cargo test --locked --features e2e "$@"
fi

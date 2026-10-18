#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT_DIR}"
source "${ROOT_DIR}/scripts/wasm-opt-wrapper/env.sh"

endpoint=${RPC_URL:-http://localhost:8547}
contracts=()
while IFS=$'\t' read -r contract package_directory; do
  [[ -n "${contract}" ]] && contracts+=("${contract}|${package_directory}")
done < <(
  cargo metadata --no-deps --format-version 1 |
    python3 -c 'import json,sys; data=json.load(sys.stdin); print("\n".join(p["name"]+"\t"+p["manifest_path"].rsplit("/",1)[0] for p in data["packages"] if "/examples/" in p["manifest_path"] and any("cdylib" in t["kind"] for t in p["targets"])))'
)

if [[ ${#contracts[@]} -eq 0 ]]; then
  printf 'No example cdylib contracts found.\n' >&2
  exit 1
fi

for entry in "${contracts[@]}"; do
  contract=${entry%%|*}
  package_directory=${entry#*|}
  printf '\nChecking and activating %s on %s\n' "${contract}" "${endpoint}"
  (
    cd "${package_directory}"
    cargo stylus check --contract "${contract}" --endpoint "${endpoint}"
  )
done

# The crypto-ERC20 starter template is intentionally outside the main Cargo
# workspace, so check it from its own project directory.
printf '\nChecking and activating crypto-erc20 template on %s\n' "${endpoint}"
(
  cd templates/crypto-erc20
  cargo stylus check --endpoint "${endpoint}"
)

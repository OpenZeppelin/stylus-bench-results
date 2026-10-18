#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

source "$SCRIPT_DIR/stylus-tools.sh"
require_cargo_stylus
require_docker_daemon

# Select each workspace example package that owns feature-gated integration
# test targets. These tests use stylus-tools::devnet::Node, which starts and
# stops a pinned Nitro container through Testcontainers. Running test binaries
# and test cases serially keeps Docker and devnet load bounded on runners.
metadata="$(cargo metadata --locked --no-deps --format-version 1)"
test_packages="$(python3 -c '
import json
import sys

metadata = json.load(sys.stdin)
for package in metadata["packages"]:
    if "integration-tests" not in package["features"]:
        continue
    if any("test" in target["kind"] for target in package["targets"]):
        print(package["name"])
' <<<"$metadata")"

packages=()
while IFS= read -r package; do
  [[ -n "$package" ]] && packages+=("$package")
done <<<"$test_packages"
if [[ ${#packages[@]} -eq 0 || -z "${packages[0]}" ]]; then
  echo "No integration-test packages with the integration-tests feature were found." >&2
  exit 1
fi

DEVNET_IMAGE="offchainlabs/nitro-node:v3.11.1-8512b8c"
nodes_before=""

cleanup_new_nodes() {
  local current_nodes new_nodes
  current_nodes="$(docker ps -aq --filter "ancestor=$DEVNET_IMAGE")"
  new_nodes="$(python3 -c '
import sys

before = set(sys.argv[1].split())
current = sys.argv[2].split()
print(" ".join(container_id for container_id in current if container_id not in before))
' "$nodes_before" "$current_nodes")"
  if [[ -n "$new_nodes" ]]; then
    # The SDK Node stores the container for the process lifetime. Remove only
    # test containers created during this package run, even after a failed test.
    docker rm --force $new_nodes >/dev/null
  fi
  nodes_before=""
}

trap cleanup_new_nodes EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

start_index=0
if [[ -n "${E2E_START_FROM:-}" ]]; then
  start_index=-1
  for index in "${!packages[@]}"; do
    if [[ "${packages[$index]}" == "$E2E_START_FROM" ]]; then
      start_index="$index"
      break
    fi
  done
  if (( start_index < 0 )); then
    echo "Unknown E2E_START_FROM package: $E2E_START_FROM" >&2
    exit 2
  fi
fi

echo "Running SDK devnet integration tests for ${#packages[@]} example packages."
for ((index = start_index; index < ${#packages[@]}; index++)); do
  package="${packages[$index]}"
  printf '[%d/%d] %s\n' "$((index + 1))" "${#packages[@]}" "$package"
  nodes_before="$(docker ps -aq --filter "ancestor=$DEVNET_IMAGE")"
  set +e
  cargo test --jobs 1 --locked -p "$package" --features integration-tests \
    --tests "$@" -- --test-threads=1
  result=$?
  set -e
  cleanup_new_nodes
  if (( result != 0 )); then
    exit "$result"
  fi
done

echo "Running the crypto-erc20 template activation smoke test."
nodes_before="$(docker ps -aq --filter "ancestor=$DEVNET_IMAGE")"
set +e
cargo test --jobs 1 --manifest-path templates/crypto-erc20/Cargo.toml \
  --locked --features integration-tests --test devnet "$@" -- \
  --test-threads=1
result=$?
set -e
cleanup_new_nodes
exit "$result"

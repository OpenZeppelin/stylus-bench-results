#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

source "$SCRIPT_DIR/stylus-tools.sh"
require_cargo_stylus
require_wasm_opt_116
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

package_args=()
for package in "${packages[@]}"; do
  package_args+=(-p "$package")
done

echo "Running SDK devnet integration tests for ${#packages[@]} example packages."
cargo test --jobs 1 --locked "${package_args[@]}" --features integration-tests --tests "$@" -- --test-threads=1

#!/bin/bash
set -e

MYDIR=$(realpath "$(dirname "$0")")
cd "$MYDIR"
cd ..

WASM_OPT_VERSION=$(python3 -c 'import sys, tomllib; print(tomllib.load(open(sys.argv[1], "rb"))["wasm-opt"]["version"])' Stylus.toml)
WASM_OPT_ACTUAL_VERSION=$(wasm-opt --version)
if [[ "$WASM_OPT_ACTUAL_VERSION" != *"version $WASM_OPT_VERSION"* ]]; then
  echo "Expected Binaryen wasm-opt $WASM_OPT_VERSION, found: $WASM_OPT_ACTUAL_VERSION" >&2
  exit 1
fi

WASM_OPT_FLAGS=()
while IFS= read -r FLAG; do
  WASM_OPT_FLAGS+=("$FLAG")
done < <(python3 -c 'import sys, tomllib; print("\n".join(tomllib.load(open(sys.argv[1], "rb"))["wasm-opt"]["flags"]))' Stylus.toml)

# Optimize contract's wasm binary by crate name.
opt_wasm() {
  local CONTRACT_CRATE_NAME=$1
  local CONTRACT_BIN_NAME="${CONTRACT_CRATE_NAME//-/_}.wasm"
  local CONTRACT_OPT_BIN_NAME="${CONTRACT_CRATE_NAME//-/_}_opt.wasm"

  echo
  echo "Optimizing $CONTRACT_CRATE_NAME WASM binary"
  wasm-opt "${WASM_OPT_FLAGS[@]}" -o ./target/wasm32-unknown-unknown/release/"$CONTRACT_OPT_BIN_NAME" ./target/wasm32-unknown-unknown/release/"$CONTRACT_BIN_NAME"
}

# Retrieve the contract crate names configured for cargo-stylus.
get_example_crate_names() {
  while IFS= read -r manifest; do
    local contract_dir
    contract_dir=$(dirname "$manifest")
    sed -n '/^\[package\]/,/^\[/s/^name = "\(.*\)"/\1/p' "$contract_dir/Cargo.toml"
  done < <(find ./examples ./templates -type f -name Stylus.toml | sort)
}

# The test support, benchmark runner, and basic-script CLI are host tools.
# Build every Stylus contract package for WASM and skip those native packages.
cargo build --workspace \
  --exclude benches \
  --exclude stylus-test-support \
  --exclude basic-script-example \
  --release \
  --target wasm32-unknown-unknown
cargo build \
  --manifest-path templates/crypto-erc20/Cargo.toml \
  --release \
  --target wasm32-unknown-unknown \
  --target-dir target

# Optimize contract's wasm for gas usage.
for CRATE_NAME in $(get_example_crate_names); do
  opt_wasm "$CRATE_NAME"
done

# No need to compile benchmarks with `--release`
# since this only runs the benchmarking code and the contracts have already been compiled with `--release`.
# The benchmark helper starts its SDK-managed Nitro devnet automatically.
cargo run -p benches --bin benches
echo "This benchmarks measure gas execution cost,
 the 21000 EVM base gas fee is omitted."
echo
echo "To measure non cached contract's gas usage correctly,
 benchmarks should run on a clean instance of the nitro test node."
echo
echo "Finished running benches!"

#!/usr/bin/env bash
set -euo pipefail

MYDIR=$(realpath "$(dirname "$0")")
cd "$MYDIR"
cd ..

export RPC_URL=http://localhost:8547

# No need to compile benchmarks with `--release`
# `cargo stylus deploy` builds each contract with its pinned SDK optimizer config.
cargo run --locked -p benches
echo "This benchmarks measure gas execution cost,
 the 21000 EVM base gas fee is omitted."
echo
echo "To measure uncached contract gas correctly, run this on a clean Nitro devnode."
echo
echo "Finished running benches!"

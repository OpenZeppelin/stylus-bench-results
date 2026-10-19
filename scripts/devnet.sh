#!/bin/bash
# Starts or stops the Stylus SDK devnet (a pinned Nitro node in Docker) used by the
# end-to-end tests and the benches.
#
#   scripts/devnet.sh start   builds the devnet binary, starts a node in the background
#                             and prints its RPC URL once it is ready.
#   scripts/devnet.sh stop    stops the node that `start` launched.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/target/devnet"
mkdir -p "$STATE"

case "${1:-}" in
  start)
    cargo build -p devnet --bin devnet --quiet
    "$ROOT/target/debug/devnet" > "$STATE/devnet.log" 2>&1 &
    echo $! > "$STATE/devnet.pid"
    until grep -q 'RPC_URL=' "$STATE/devnet.log"; do
      if ! kill -0 "$(cat "$STATE/devnet.pid")" 2>/dev/null; then
        echo "the devnet exited, see $STATE/devnet.log" >&2
        exit 1
      fi
      sleep 2
    done
    grep -o 'RPC_URL=.*' "$STATE/devnet.log" | head -1 | cut -d= -f2-
    ;;
  stop)
    if [ -f "$STATE/devnet.pid" ]; then
      kill -INT "$(cat "$STATE/devnet.pid")" 2>/dev/null || true
      rm -f "$STATE/devnet.pid"
    fi
    ;;
  *)
    echo "usage: $0 start|stop" >&2
    exit 2
    ;;
esac

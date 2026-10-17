#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(git rev-parse --show-toplevel)
cd "$ROOT_DIR"

# The SDK integration-test harness starts a disposable Nitro devnet in Docker.
# Its deployment helper builds each contract, checks its size and activation
# cost, then deploys and activates it on that devnet.
exec ./scripts/e2e-tests.sh

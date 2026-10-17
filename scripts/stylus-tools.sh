#!/usr/bin/env bash

require_cargo_stylus() {
  if ! command -v cargo-stylus >/dev/null 2>&1; then
    echo "cargo-stylus 0.10.10 is required; install it with: cargo install --locked --force cargo-stylus@0.10.10" >&2
    return 1
  fi

  local version
  version="$(cargo stylus --version)"
  if [[ ! "$version" =~ (^|[[:space:]])0\.10\.10([[:space:]]|$) ]]; then
    echo "cargo-stylus 0.10.10 is required, found: $version" >&2
    return 1
  fi
}

require_wasm_opt_116() {
  if ! command -v wasm-opt >/dev/null 2>&1; then
    echo "Binaryen wasm-opt 116 is required; install it with: cargo install --locked --force wasm-opt@0.116.1" >&2
    return 1
  fi

  local version
  version="$(wasm-opt --version)"
  if ! grep -Eq 'version[[:space:]]116([^0-9]|$)' <<<"$version"; then
    echo "Binaryen wasm-opt 116 is required, found: $version" >&2
    return 1
  fi
}

require_docker_daemon() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "Docker is required by stylus-tools devnet integration tests and benchmarks." >&2
    return 1
  fi
  if ! docker info >/dev/null 2>&1; then
    echo "The Docker daemon is not reachable; stylus-tools starts Nitro with Testcontainers." >&2
    return 1
  fi
}

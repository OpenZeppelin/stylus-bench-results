#!/usr/bin/env bash

_OZ_ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
_OZ_REAL_WASM_OPT="$(command -v wasm-opt || true)"
_OZ_WASM_TOOLS="$(command -v wasm-tools || true)"

if [[ -z "${_OZ_REAL_WASM_OPT}" || -z "${_OZ_WASM_TOOLS}" ]]; then
  printf 'wasm-opt 116 and wasm-tools 1.262.0 are required.\n' >&2
  return 1
fi
if [[ "$("${_OZ_REAL_WASM_OPT}" --version)" != *'version 116 '* ]]; then
  printf 'Expected wasm-opt 116, found: %s\n' \
    "$("${_OZ_REAL_WASM_OPT}" --version)" >&2
  return 1
fi
if [[ "$("${_OZ_WASM_TOOLS}" --version)" != 'wasm-tools 1.262.0' ]]; then
  printf 'Expected wasm-tools 1.262.0, found: %s\n' \
    "$("${_OZ_WASM_TOOLS}" --version)" >&2
  return 1
fi
if [[ "$(cargo stylus --version)" != 'stylus 0.10.10' ]]; then
  printf 'Expected cargo-stylus 0.10.10, found: %s\n' \
    "$(cargo stylus --version)" >&2
  return 1
fi

export OZ_WASM_OPT_REAL="${_OZ_REAL_WASM_OPT}"
export OZ_WASM_TOOLS="${_OZ_WASM_TOOLS}"
export OZ_WASM_TOOLS_ROOT="${_OZ_ROOT_DIR}"
export PATH="${_OZ_ROOT_DIR}/scripts/wasm-opt-wrapper:${PATH}"

unset _OZ_ROOT_DIR _OZ_REAL_WASM_OPT _OZ_WASM_TOOLS

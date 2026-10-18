#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

if [[ $# -eq 0 ]]; then
  printf 'Usage: scripts/build-wasm-artifacts.sh PACKAGE...\n' >&2
  exit 2
fi

rustc_version="$(rustc --version)"
wasm_opt_version="$(wasm-opt --version)"
wasm_tools_version="$(wasm-tools --version)"
if [[ "${rustc_version}" != rustc\ 1.91.0\ * ]]; then
  printf 'Expected stable Rust 1.91.0, found: %s\n' "${rustc_version}" >&2
  exit 1
fi
if [[ "${wasm_opt_version}" != *'version 116 '* ]]; then
  printf 'Expected wasm-opt 116, found: %s\n' "${wasm_opt_version}" >&2
  exit 1
fi
if [[ "${wasm_tools_version}" != 'wasm-tools 1.262.0' ]]; then
  printf 'Expected wasm-tools 1.262.0, found: %s\n' "${wasm_tools_version}" >&2
  exit 1
fi
if ! rg -Fq 'version = "116"' Stylus.toml || \
  ! rg -Fq 'flags = ["--enable-bulk-memory", "-O3"]' Stylus.toml; then
  printf 'scripts/build-wasm-artifacts.sh must match Stylus.toml.\n' >&2
  exit 1
fi

target_directory="$(cargo metadata --no-deps --format-version 1 | \
  python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
package_args=()
for package in "$@"; do
  package_args+=(--package "${package}")
done

cargo build --locked --release --target wasm32-unknown-unknown \
  "${package_args[@]}"

artifact_directory="${target_directory}/stylus-artifacts"
mkdir -p "${artifact_directory}"
for package in "$@"; do
  crate_name="${package//-/_}"
  raw_wasm="${target_directory}/wasm32-unknown-unknown/release/${crate_name}.wasm"
  optimized_wasm="${artifact_directory}/${package}.binaryen.wasm"
  wat_file="${artifact_directory}/${package}.wat"
  final_wasm="${artifact_directory}/${package}.wasm"

  if [[ ! -f "${raw_wasm}" ]]; then
    printf 'Missing WebAssembly output for package %s: %s\n' \
      "${package}" "${raw_wasm}" >&2
    exit 1
  fi

  wasm-opt --enable-bulk-memory -O3 "${raw_wasm}" -o "${optimized_wasm}"
  wasm-tools validate "${optimized_wasm}"
  wasm-tools print "${optimized_wasm}" -o "${wat_file}"
  if rg -q '\(memory\.init|\(data\.drop' "${wat_file}"; then
    printf '%s contains data-index instructions; retaining DataCount is required.\n' \
      "${package}" >&2
    exit 1
  fi

  # Nitro's devnet activation parser rejects DataCount even though this module
  # has no memory.init or data.drop instructions, which are the only operators
  # that require it. Validate both sides of the section removal.
  python3 scripts/strip-wasm-datacount.py \
    "${optimized_wasm}" "${final_wasm}"
  wasm-tools validate "${final_wasm}"
  raw_bytes="$(wc -c <"${raw_wasm}" | tr -d ' ')"
  optimized_bytes="$(wc -c <"${optimized_wasm}" | tr -d ' ')"
  deployable_bytes="$(wc -c <"${final_wasm}" | tr -d ' ')"
  rm -f "${optimized_wasm}" "${wat_file}"
  printf '%s: raw=%s bytes, Binaryen=%s bytes, deployable=%s bytes\n' \
    "${package}" "${raw_bytes}" "${optimized_bytes}" "${deployable_bytes}"
done

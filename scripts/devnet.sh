#!/usr/bin/env bash

# Source this file to use start_devnet/stop_devnet from another runner, or use
# `scripts/devnet.sh run -- command ...` to share one SDK-managed Nitro node with
# that command. stylus-tools configures ArbOS and the Stylus system contracts.
DEVNET_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEVNET_ROOT_DIR="$(cd "${DEVNET_SCRIPT_DIR}/.." && pwd)"
DEVNET_EXPLICIT_RPC_URL="${RPC_URL:-}"
DEVNET_RPC_URL="${DEVNET_EXPLICIT_RPC_URL}"
DEVNET_STATE_DIR="${DEVNET_STATE_DIR:-/tmp/openzeppelin-stylus-devnet}"
DEVNET_PID_FILE="${DEVNET_STATE_DIR}/sdk-devnet.pid"
DEVNET_RPC_FILE="${DEVNET_STATE_DIR}/sdk-devnet.rpc"
DEVNET_LOG_FILE="${DEVNET_STATE_DIR}/sdk-devnet.log"
DEVNET_STARTED_BY_CALLER=0
export DEPLOYER_ADDRESS="${DEPLOYER_ADDRESS:-0xcEcba2F1DC234f70Dd89F2041029807F8D03A990}"

devnet_rpc_is_ready() {
  [[ -n "${DEVNET_RPC_URL}" ]] || return 1
  curl --silent --show-error --fail --max-time 2 \
    --header 'Content-Type: application/json' \
    --data '{"jsonrpc":"2.0","method":"net_version","params":[],"id":1}' \
    "${DEVNET_RPC_URL}" | grep -q '"result"'
}

devnet_wait_until_ready() {
  for ((attempt = 0; attempt < 600; attempt++)); do
    if [[ -n "${DEVNET_PID:-}" ]] && ! kill -0 "${DEVNET_PID}" 2>/dev/null; then
      return 1
    fi

    if [[ -z "${DEVNET_RPC_URL}" && -f "${DEVNET_RPC_FILE}" ]]; then
      DEVNET_RPC_URL="$(<"${DEVNET_RPC_FILE}")"
    fi
    if [[ -z "${DEVNET_RPC_URL}" && -f "${DEVNET_LOG_FILE}" ]]; then
      DEVNET_RPC_URL="$(awk -F= '/^RPC_URL=/ { print substr($0, 9); exit }' "${DEVNET_LOG_FILE}")"
    fi
    if [[ -n "${DEVNET_RPC_URL}" ]] && devnet_rpc_is_ready; then
      printf '%s\n' "${DEVNET_RPC_URL}" >"${DEVNET_RPC_FILE}"
      export RPC_URL="${DEVNET_RPC_URL}"
      return 0
    fi
    sleep 0.5
  done
  return 1
}

devnet_binary_path() {
  local target_directory
  target_directory="$(cargo metadata --locked --no-deps --format-version 1 \
    --manifest-path "${DEVNET_ROOT_DIR}/Cargo.toml" |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
  printf '%s/stylus-devnet-node\n' "${target_directory}/debug"
}

start_devnet() {
  if [[ -n "${DEVNET_EXPLICIT_RPC_URL}" ]]; then
    DEVNET_RPC_URL="${DEVNET_EXPLICIT_RPC_URL}"
    if ! devnet_rpc_is_ready; then
      printf 'RPC endpoint is unavailable: %s\n' "${DEVNET_RPC_URL}" >&2
      return 1
    fi
    export RPC_URL="${DEVNET_RPC_URL}"
    return 0
  fi

  mkdir -p "${DEVNET_STATE_DIR}"
  if [[ -f "${DEVNET_PID_FILE}" ]]; then
    local existing_pid
    existing_pid="$(<"${DEVNET_PID_FILE}")"
    if [[ "${existing_pid}" =~ ^[0-9]+$ ]] && kill -0 "${existing_pid}" 2>/dev/null; then
      local process_command
      process_command="$(ps -p "${existing_pid}" -o command= 2>/dev/null || true)"
      if [[ "${process_command}" == *"stylus-devnet-node"* ]]; then
        DEVNET_PID="${existing_pid}"
        DEVNET_RPC_URL=""
        if devnet_wait_until_ready; then
          unset DEVNET_PID
          return 0
        fi
        printf 'Existing SDK Nitro devnet did not become ready.\n' >&2
        cat "${DEVNET_LOG_FILE}" >&2 2>/dev/null || true
        return 1
      fi
    fi
    rm -f "${DEVNET_PID_FILE}" "${DEVNET_RPC_FILE}"
  fi

  command -v docker >/dev/null || {
    printf 'Docker is required to start the SDK Nitro devnet.\n' >&2
    return 1
  }

  cargo build --locked --manifest-path "${DEVNET_ROOT_DIR}/Cargo.toml" \
    -p stylus-devnet-node
  local binary
  binary="$(devnet_binary_path)"
  if [[ ! -x "${binary}" ]]; then
    printf 'SDK devnet helper was not built at %s\n' "${binary}" >&2
    return 1
  fi

  : >"${DEVNET_LOG_FILE}"
  nohup "${binary}" >"${DEVNET_LOG_FILE}" 2>&1 </dev/null &
  DEVNET_PID=$!
  DEVNET_STARTED_BY_CALLER=1
  printf '%s\n' "${DEVNET_PID}" >"${DEVNET_PID_FILE}.tmp"
  mv "${DEVNET_PID_FILE}.tmp" "${DEVNET_PID_FILE}"

  if ! devnet_wait_until_ready; then
    printf 'SDK Nitro devnet did not become ready.\n' >&2
    cat "${DEVNET_LOG_FILE}" >&2 || true
    stop_devnet
    return 1
  fi
}

start_clean_devnet() {
  if [[ -z "${DEVNET_EXPLICIT_RPC_URL}" ]]; then
    stop_devnet --force
  fi
  start_devnet
}

stop_devnet() {
  local force=0
  if [[ "${1:-}" == "--force" ]]; then
    force=1
  fi
  if [[ "${DEVNET_STARTED_BY_CALLER}" != 1 && "${force}" != 1 ]]; then
    return 0
  fi

  local pid="${DEVNET_PID:-}"
  if [[ -z "${pid}" && -f "${DEVNET_PID_FILE}" ]]; then
    pid="$(<"${DEVNET_PID_FILE}")"
  fi
  if [[ "${pid}" =~ ^[0-9]+$ ]] && kill -0 "${pid}" 2>/dev/null; then
    local process_command
    process_command="$(ps -p "${pid}" -o command= 2>/dev/null || true)"
    if [[ "${process_command}" == *"stylus-devnet-node"* ]]; then
      kill -TERM "${pid}" 2>/dev/null || true
      for ((attempt = 0; attempt < 100; attempt++)); do
        kill -0 "${pid}" 2>/dev/null || break
        sleep 0.1
      done
      if kill -0 "${pid}" 2>/dev/null; then
        kill -KILL "${pid}" 2>/dev/null || true
      fi
      if [[ -n "${DEVNET_PID:-}" ]]; then
        wait "${pid}" 2>/dev/null || true
      fi
    else
      pid=""
    fi
  fi
  if [[ -f "${DEVNET_PID_FILE}" && "$(<"${DEVNET_PID_FILE}")" == "${pid}" ]]; then
    rm -f "${DEVNET_PID_FILE}" "${DEVNET_RPC_FILE}"
  fi
  if [[ -z "${DEVNET_EXPLICIT_RPC_URL}" && "${RPC_URL:-}" == "${DEVNET_RPC_URL}" ]]; then
    unset RPC_URL
  fi
  unset DEVNET_PID
  DEVNET_STARTED_BY_CALLER=0
}

devnet_run() {
  if [[ "${1:-}" != -- ]]; then
    printf 'Usage: scripts/devnet.sh run -- command [args...]\n' >&2
    return 2
  fi
  shift
  if [[ $# -eq 0 ]]; then
    printf 'A command is required after --.\n' >&2
    return 2
  fi

  start_devnet || return
  local status=0
  "$@" || status=$?
  if [[ ${status} -ne 0 ]]; then
    printf '\nSDK Nitro devnet log (%s):\n' "${DEVNET_LOG_FILE}" >&2
    cat "${DEVNET_LOG_FILE}" >&2 || true
  fi
  stop_devnet
  return "${status}"
}

devnet_main() {
  case "${1:-}" in
    start)
      start_devnet
      printf 'SDK Nitro devnet RPC: %s\n' "${RPC_URL}"
      ;;
    stop)
      stop_devnet --force
      ;;
    run)
      shift
      devnet_run "$@"
      ;;
    *)
      printf 'Usage: scripts/devnet.sh {start|stop|run -- command [args...]}\n' >&2
      return 2
      ;;
  esac
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  set -euo pipefail
  devnet_main "$@"
fi

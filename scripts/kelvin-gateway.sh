#!/usr/bin/env bash
# kelvin-gateway — Service manager for the kelvin-gateway daemon.
#
# Usage: ./kelvin-gateway <start|stop|restart|status> [options]
#
# Environment variables:
#   KELVIN_MODEL_PROVIDER      Plugin id for the model provider (default: kelvin.echo)
#   KELVIN_PLUGIN_INDEX_URL    Plugin index URL (required if KELVIN_MODEL_PROVIDER != kelvin.echo)
#   KELVIN_HOME                State root directory (default: ~/.kelvinclaw)
#   KELVIN_PLUGIN_HOME         Override plugin install root
#   KELVIN_TRUST_POLICY_PATH   Override trust policy path
#   KELVIN_GATEWAY_TOKEN       Auth token for the gateway
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -x "${SCRIPT_DIR}/bin/kelvin-gateway" ]]; then
  ROOT_DIR="${SCRIPT_DIR}"
else
  ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
fi

# ── dotenv loader ─────────────────────────────────────────────────────────────
_kgw_trim()   { local v="$1"; v="${v#"${v%%[![:space:]]*}"}"; v="${v%"${v##*[![:space:]]}"}"; printf '%s' "${v}"; }
_kgw_unquote() {
  local v="$1"
  if [[ "${v}" == \"*\" ]] && [[ "${v}" == *\" ]]; then printf '%s' "${v:1:${#v}-2}"; return; fi
  if [[ "${v}" == \'*\' ]] && [[ "${v}" == *\' ]]; then printf '%s' "${v:1:${#v}-2}"; return; fi
  printf '%s' "${v}"
}
load_dotenv() {
  local f line stripped key value
  for f in "${PWD}/.env.local" "${PWD}/.env" "${HOME}/.kelvinclaw/.env.local" "${HOME}/.kelvinclaw/.env"; do
    [[ -f "${f}" ]] || continue
    while IFS= read -r line || [[ -n "${line}" ]]; do
      stripped="$(_kgw_trim "${line%%#*}")"
      [[ -z "${stripped}" ]] && continue
      [[ "${stripped}" =~ ^export[[:space:]]+ ]] && stripped="$(_kgw_trim "${stripped#export }")"
      if [[ "${stripped}" =~ ^([A-Za-z_][A-Za-z0-9_]*)[[:space:]]*=[[:space:]]*(.*)$ ]]; then
        key="${BASH_REMATCH[1]}"
        value="$(_kgw_unquote "$(_kgw_trim "${BASH_REMATCH[2]}")")"
        [[ -z "${!key+x}" ]] && export "${key}=${value}"
      fi
    done < "${f}"
  done
}
load_dotenv
# ──────────────────────────────────────────────────────────────────────────────

KELVIN_MODEL_PROVIDER="${KELVIN_MODEL_PROVIDER:-kelvin.echo}"
KELVIN_HOME="${KELVIN_HOME:-${HOME}/.kelvinclaw}"
KELVIN_HOME="${KELVIN_HOME/#\~/${HOME}}"
PLUGIN_HOME="${KELVIN_PLUGIN_HOME:-${KELVIN_HOME}/plugins}"
PLUGIN_HOME="${PLUGIN_HOME/#\~/${HOME}}"
TRUST_POLICY_PATH="${KELVIN_TRUST_POLICY_PATH:-${KELVIN_HOME}/trusted_publishers.json}"
TRUST_POLICY_PATH="${TRUST_POLICY_PATH/#\~/${HOME}}"
INDEX_URL="${KELVIN_PLUGIN_INDEX_URL:-}"
LOG_DIR="${KELVIN_HOME}/logs"
LOG_FILE="${LOG_DIR}/gateway.log"
PID_FILE="${KELVIN_HOME}/gateway.pid"
GATEWAY_BINARY="${ROOT_DIR}/bin/kelvin-gateway"

# ── helpers ───────────────────────────────────────────────────────────────────

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: missing required command: $1" >&2
    exit 1
  fi
}

is_running() {
  [[ -f "${PID_FILE}" ]] || return 1
  local pid
  pid="$(cat "${PID_FILE}")"
  kill -0 "${pid}" 2>/dev/null
}

read_pid() {
  cat "${PID_FILE}" 2>/dev/null || echo ""
}

# Returns elapsed seconds since a file was last modified.
file_age_seconds() {
  local f="$1"
  local now mtime
  now="$(date +%s)"
  if stat --version 2>/dev/null | grep -q GNU; then
    mtime="$(stat -c %Y "${f}")"
  else
    mtime="$(stat -f %m "${f}")"   # macOS/BSD
  fi
  echo $(( now - mtime ))
}

format_uptime() {
  local secs="$1"
  local d=$(( secs / 86400 ))
  local h=$(( (secs % 86400) / 3600 ))
  local m=$(( (secs % 3600) / 60 ))
  local s=$(( secs % 60 ))
  if [[ ${d} -gt 0 ]]; then printf '%dd %dh %dm' "${d}" "${h}" "${m}"; return; fi
  if [[ ${h} -gt 0 ]]; then printf '%dh %dm %ds' "${h}" "${m}" "${s}"; return; fi
  if [[ ${m} -gt 0 ]]; then printf '%dm %ds' "${m}" "${s}"; return; fi
  printf '%ds' "${s}"
}

ensure_plugin() {
  if [[ "${KELVIN_MODEL_PROVIDER}" == "kelvin.echo" ]]; then
    return 0
  fi
  local plugin_current="${PLUGIN_HOME}/${KELVIN_MODEL_PROVIDER}/current"
  if [[ -e "${plugin_current}" ]]; then
    return 0
  fi
  if [[ -z "${INDEX_URL}" ]]; then
    echo "error: KELVIN_PLUGIN_INDEX_URL must be set to install '${KELVIN_MODEL_PROVIDER}'" >&2
    exit 1
  fi
  echo "[kelvin-gateway] installing model provider: ${KELVIN_MODEL_PROVIDER}"
  "${ROOT_DIR}/share/scripts/plugin-index-install.sh" \
    --plugin "${KELVIN_MODEL_PROVIDER}" \
    --index-url "${INDEX_URL}"
}

ensure_trust_policy() {
  mkdir -p "${PLUGIN_HOME}" "$(dirname "${TRUST_POLICY_PATH}")"
  export KELVIN_PLUGIN_HOME="${PLUGIN_HOME}"
  export KELVIN_TRUST_POLICY_PATH="${TRUST_POLICY_PATH}"
  if [[ ! -f "${TRUST_POLICY_PATH}" ]]; then
    echo '{"require_signature":false,"publishers":[]}' > "${TRUST_POLICY_PATH}"
    echo "[kelvin-gateway] wrote permissive trust policy: ${TRUST_POLICY_PATH}"
  fi
}

# ── usage ─────────────────────────────────────────────────────────────────────

usage() {
  cat <<'USAGE'
Usage: ./kelvin-gateway <subcommand> [options]

Lifecycle manager for the kelvin-gateway daemon.

Subcommands:
  start [--foreground] [-- <gateway-args>]
                   Start the gateway.
                   Default: daemon mode (background, PID file, log file).
                   --foreground: run attached to the terminal (Ctrl+C to stop).
                   Pass gateway binary flags after --.
  stop             Stop the running gateway daemon.
  restart [-- <gateway-args>]
                   Stop (if running) and start the gateway.
  status           Show gateway status, PID, model provider, log path, uptime.
  -h, --help       Show this help.

State files:
  $KELVIN_HOME/gateway.pid        PID of the running daemon
  $KELVIN_HOME/logs/gateway.log   Daemon stdout + stderr (appended)

Environment:
  KELVIN_MODEL_PROVIDER      Model provider plugin id (default: kelvin.echo)
  KELVIN_PLUGIN_INDEX_URL    Plugin index URL (required for non-echo providers)
  KELVIN_GATEWAY_TOKEN       Auth token for the gateway
  KELVIN_HOME                State root (default: ~/.kelvinclaw)
  KELVIN_PLUGIN_HOME         Override plugin install root
  KELVIN_TRUST_POLICY_PATH   Override trust policy path

Examples:
  ./kelvin-gateway start
  ./kelvin-gateway start -- --bind 0.0.0.0:34617
  ./kelvin-gateway start --foreground -- --bind 127.0.0.1:34617
  ./kelvin-gateway status
  ./kelvin-gateway stop
  ./kelvin-gateway restart
USAGE
}

# ── subcommands ───────────────────────────────────────────────────────────────

cmd_start() {
  local foreground=0
  local -a gateway_args=()

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --foreground) foreground=1; shift ;;
      --)           shift; gateway_args=("$@"); break ;;
      *)            echo "error: unknown option: $1" >&2; exit 1 ;;
    esac
  done

  require_cmd curl
  require_cmd tar
  require_cmd jq

  ensure_trust_policy
  ensure_plugin

  if [[ "${foreground}" -eq 1 ]]; then
    exec "${GATEWAY_BINARY}" --model-provider "${KELVIN_MODEL_PROVIDER}" "${gateway_args[@]}"
  fi

  # Daemon mode
  if [[ -f "${PID_FILE}" ]]; then
    local existing_pid
    existing_pid="$(cat "${PID_FILE}")"
    if kill -0 "${existing_pid}" 2>/dev/null; then
      echo "error: gateway is already running (pid=${existing_pid})" >&2
      echo "       log: ${LOG_FILE}" >&2
      exit 1
    fi
    echo "[kelvin-gateway] removing stale PID file (pid=${existing_pid})"
    rm -f "${PID_FILE}"
  fi

  mkdir -p "${LOG_DIR}"
  nohup "${GATEWAY_BINARY}" --model-provider "${KELVIN_MODEL_PROVIDER}" "${gateway_args[@]}" \
    >> "${LOG_FILE}" 2>&1 &
  local pid=$!
  printf '%s' "${pid}" > "${PID_FILE}"
  echo "[kelvin-gateway] started (pid=${pid})"
  echo "[kelvin-gateway] log: ${LOG_FILE}"
  echo "[kelvin-gateway] pid: ${PID_FILE}"
}

cmd_stop() {
  if [[ ! -f "${PID_FILE}" ]]; then
    echo "error: gateway is not running (no PID file)" >&2
    exit 1
  fi

  local pid
  pid="$(cat "${PID_FILE}")"

  if ! kill -0 "${pid}" 2>/dev/null; then
    echo "[kelvin-gateway] not running (stale PID ${pid}); removing PID file"
    rm -f "${PID_FILE}"
    exit 0
  fi

  echo "[kelvin-gateway] stopping (pid=${pid})"
  kill "${pid}"

  local elapsed=0
  while kill -0 "${pid}" 2>/dev/null; do
    sleep 0.5
    elapsed=$(( elapsed + 1 ))
    if [[ ${elapsed} -ge 6 ]]; then   # 3 seconds
      echo "[kelvin-gateway] process did not stop; sending SIGKILL"
      kill -9 "${pid}" 2>/dev/null || true
      break
    fi
  done

  rm -f "${PID_FILE}"
  echo "[kelvin-gateway] stopped"
}

cmd_restart() {
  local -a passthrough=("$@")
  if is_running; then
    cmd_stop
  fi
  cmd_start "${passthrough[@]}"
}

cmd_status() {
  echo "KELVIN_HOME=${KELVIN_HOME}"
  echo "KELVIN_MODEL_PROVIDER=${KELVIN_MODEL_PROVIDER}"
  echo "KELVIN_PLUGIN_INDEX_URL=${INDEX_URL:-(not set)}"
  echo "log: ${LOG_FILE}"
  echo ""

  if ! [[ -f "${PID_FILE}" ]]; then
    echo "status: stopped"
    return 0
  fi

  local pid
  pid="$(cat "${PID_FILE}")"

  if ! kill -0 "${pid}" 2>/dev/null; then
    echo "status: stopped (stale PID file: ${pid})"
    return 0
  fi

  local uptime_str=""
  if [[ -f "${PID_FILE}" ]]; then
    local age
    age="$(file_age_seconds "${PID_FILE}")"
    uptime_str=" (up $(format_uptime "${age}"))"
  fi

  echo "status: running${uptime_str}"
  echo "pid:    ${pid}"
}

# ── dispatch ──────────────────────────────────────────────────────────────────

if [[ $# -eq 0 ]]; then
  usage
  exit 0
fi

SUBCOMMAND="$1"
shift

case "${SUBCOMMAND}" in
  start)   cmd_start "$@" ;;
  stop)    cmd_stop ;;
  restart) cmd_restart "$@" ;;
  status)  cmd_status ;;
  -h|--help) usage; exit 0 ;;
  *)
    echo "error: unknown subcommand: ${SUBCOMMAND}" >&2
    echo "" >&2
    usage >&2
    exit 1
    ;;
esac

#!/usr/bin/env bash
# start-gateway.sh — Release-bundle launcher for kelvin-gateway.
#
# Auto-installs the model provider plugin specified by KELVIN_MODEL_PROVIDER
# from the plugin index if not already installed, then execs kelvin-gateway.
#
# Environment variables:
#   KELVIN_MODEL_PROVIDER      Plugin id for the model provider (default: kelvin.echo)
#   KELVIN_PLUGIN_INDEX_URL    Plugin index URL (required if KELVIN_MODEL_PROVIDER != kelvin.echo)
#   KELVIN_HOME                State root directory (default: ~/.kelvinclaw)
#   KELVIN_PLUGIN_HOME         Override plugin install root
#   KELVIN_TRUST_POLICY_PATH   Override trust policy path
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -x "${SCRIPT_DIR}/bin/kelvin-gateway" ]]; then
  ROOT_DIR="${SCRIPT_DIR}"
else
  ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
fi

KELVIN_MODEL_PROVIDER="${KELVIN_MODEL_PROVIDER:-kelvin.echo}"
KELVIN_HOME="${KELVIN_HOME:-${HOME}/.kelvinclaw}"
KELVIN_HOME="${KELVIN_HOME/#\~/${HOME}}"
PLUGIN_HOME="${KELVIN_PLUGIN_HOME:-${KELVIN_HOME}/plugins}"
PLUGIN_HOME="${PLUGIN_HOME/#\~/${HOME}}"
TRUST_POLICY_PATH="${KELVIN_TRUST_POLICY_PATH:-${KELVIN_HOME}/trusted_publishers.json}"
TRUST_POLICY_PATH="${TRUST_POLICY_PATH/#\~/${HOME}}"
INDEX_URL="${KELVIN_PLUGIN_INDEX_URL:-}"

usage() {
  cat <<'USAGE'
Usage: ./start-gateway [kelvin-gateway args]

Release-bundle launcher for kelvin-gateway. Auto-installs the model provider
plugin specified by KELVIN_MODEL_PROVIDER before starting the gateway.

Environment:
  KELVIN_MODEL_PROVIDER      Plugin id to use as model provider (default: kelvin.echo)
  KELVIN_PLUGIN_INDEX_URL    Plugin index URL (required if KELVIN_MODEL_PROVIDER != kelvin.echo)
  KELVIN_HOME                Bundle-managed state root (default: ~/.kelvinclaw)
  KELVIN_PLUGIN_HOME         Override plugin install root
  KELVIN_TRUST_POLICY_PATH   Override trust policy path

Examples:
  KELVIN_MODEL_PROVIDER=kelvin.anthropic \
  KELVIN_PLUGIN_INDEX_URL=https://raw.githubusercontent.com/AgenticHighway/kelvinclaw/main/index.json \
  ANTHROPIC_API_KEY=sk-ant-... \
  ./start-gateway --bind 127.0.0.1:34617
USAGE
}

require_cmd() {
  local name="$1"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "Missing required command: ${name}" >&2
    exit 1
  fi
}

if [[ $# -gt 0 ]]; then
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
  esac
fi

require_cmd curl
require_cmd tar
require_cmd jq

mkdir -p "${PLUGIN_HOME}" "$(dirname "${TRUST_POLICY_PATH}")"
export KELVIN_PLUGIN_HOME="${PLUGIN_HOME}"
export KELVIN_TRUST_POLICY_PATH="${TRUST_POLICY_PATH}"

# Write a permissive trust policy if none exists.
# First-party plugins are unsigned_local; signature enforcement is disabled
# until a signed distribution flow is in place.
if [[ ! -f "${TRUST_POLICY_PATH}" ]]; then
  echo '{"require_signature":false,"publishers":[]}' > "${TRUST_POLICY_PATH}"
  echo "[start-gateway] wrote permissive trust policy: ${TRUST_POLICY_PATH}"
fi

# Auto-install the model provider plugin if it is not already present.
if [[ "${KELVIN_MODEL_PROVIDER}" != "kelvin.echo" ]]; then
  PLUGIN_CURRENT="${PLUGIN_HOME}/${KELVIN_MODEL_PROVIDER}/current"
  if [[ ! -e "${PLUGIN_CURRENT}" ]]; then
    if [[ -z "${INDEX_URL}" ]]; then
      echo "[start-gateway] error: KELVIN_PLUGIN_INDEX_URL must be set to install '${KELVIN_MODEL_PROVIDER}'" >&2
      exit 1
    fi
    echo "[start-gateway] installing model provider: ${KELVIN_MODEL_PROVIDER}"
    "${ROOT_DIR}/share/scripts/plugin-index-install.sh" \
      --plugin "${KELVIN_MODEL_PROVIDER}" \
      --index-url "${INDEX_URL}"
  fi
fi

exec "${ROOT_DIR}/bin/kelvin-gateway" --model-provider "${KELVIN_MODEL_PROVIDER}" "$@"

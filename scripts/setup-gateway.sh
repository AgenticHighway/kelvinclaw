#!/usr/bin/env bash
# setup-gateway.sh — Post-install configuration helper for kelvin-gateway release bundles.
#
# Provides subcommands for installing model provider plugins and inspecting
# the current plugin configuration without starting the gateway.
#
# Environment variables:
#   KELVIN_PLUGIN_INDEX_URL    Plugin index URL (required for install-provider)
#   KELVIN_MODEL_PROVIDER      Model provider plugin id (informational; used in status output)
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

KELVIN_HOME="${KELVIN_HOME:-${HOME}/.kelvinclaw}"
KELVIN_HOME="${KELVIN_HOME/#\~/${HOME}}"
PLUGIN_HOME="${KELVIN_PLUGIN_HOME:-${KELVIN_HOME}/plugins}"
PLUGIN_HOME="${PLUGIN_HOME/#\~/${HOME}}"
TRUST_POLICY_PATH="${KELVIN_TRUST_POLICY_PATH:-${KELVIN_HOME}/trusted_publishers.json}"
TRUST_POLICY_PATH="${TRUST_POLICY_PATH/#\~/${HOME}}"
INDEX_URL="${KELVIN_PLUGIN_INDEX_URL:-}"

usage() {
  cat <<'USAGE'
Usage: ./setup-gateway <subcommand> [options]

Configure KelvinClaw gateway plugins and settings.

Subcommands:
  install-provider [--provider <id>]   Install a model provider plugin from the index
  list                                 List installed plugins
  status                               Show current configuration

Options:
  -h, --help   Show this help

Environment:
  KELVIN_PLUGIN_INDEX_URL    Plugin index URL (required for install-provider)
  KELVIN_MODEL_PROVIDER      Model provider plugin id (informational in status output)
  KELVIN_HOME                State root (default: ~/.kelvinclaw)
  KELVIN_PLUGIN_HOME         Override plugin install root
  KELVIN_TRUST_POLICY_PATH   Override trust policy path
USAGE
}

require_cmd() {
  local name="$1"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "Missing required command: ${name}" >&2
    exit 1
  fi
}

cmd_list() {
  if [[ ! -d "${PLUGIN_HOME}" ]]; then
    echo "No plugins installed (KELVIN_PLUGIN_HOME=${PLUGIN_HOME})"
    return 0
  fi
  local found="0"
  for plugin_dir in "${PLUGIN_HOME}"/*/; do
    [[ -d "${plugin_dir}" ]] || continue
    local plugin_id; plugin_id="$(basename "${plugin_dir}")"
    local current_link="${plugin_dir}current"
    local version="(unknown)"
    if [[ -L "${current_link}" ]]; then
      version="$(basename "$(readlink "${current_link}")")"
    elif [[ -f "${current_link}/plugin.json" ]]; then
      version="$(jq -r '.version // "(unknown)"' "${current_link}/plugin.json" 2>/dev/null || echo "(unknown)")"
    fi
    echo "  ${plugin_id}@${version}"
    found="1"
  done
  if [[ "${found}" == "0" ]]; then
    echo "No plugins installed (KELVIN_PLUGIN_HOME=${PLUGIN_HOME})"
  fi
}

cmd_status() {
  echo "KELVIN_HOME=${KELVIN_HOME}"
  echo "KELVIN_PLUGIN_HOME=${PLUGIN_HOME}"
  echo "KELVIN_TRUST_POLICY_PATH=${TRUST_POLICY_PATH}"
  echo "KELVIN_MODEL_PROVIDER=${KELVIN_MODEL_PROVIDER:-kelvin.echo}"
  echo "KELVIN_PLUGIN_INDEX_URL=${INDEX_URL:-(not set)}"
  echo
  echo "Installed plugins:"
  cmd_list
}

cmd_install_provider() {
  local provider_id=""

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --provider)
        provider_id="${2:?missing value for --provider}"
        shift 2
        ;;
      *)
        echo "Unknown argument: $1" >&2
        exit 1
        ;;
    esac
  done

  require_cmd curl
  require_cmd tar
  require_cmd jq

  if [[ -z "${INDEX_URL}" ]]; then
    echo "error: KELVIN_PLUGIN_INDEX_URL is required" >&2
    exit 1
  fi

  # Interactive provider selection when --provider is not passed on a TTY.
  if [[ -z "${provider_id}" ]]; then
    if [[ ! -t 0 || ! -t 1 ]]; then
      echo "error: --provider is required in non-interactive mode" >&2
      exit 1
    fi
    echo "Fetching available plugins from index..."
    local index_json
    index_json="$(curl -fsSL --max-time 15 "${INDEX_URL}")"
    local entries
    entries="$(printf '%s' "${index_json}" | jq -r '.plugins[] | "\(.id)@\(.version)"' 2>/dev/null || true)"
    if [[ -z "${entries}" ]]; then
      echo "error: no plugins found in index: ${INDEX_URL}" >&2
      exit 1
    fi
    echo
    echo "Available plugins:"
    while IFS= read -r entry; do
      echo "  ${entry}"
    done <<< "${entries}"
    echo
    printf "Enter provider id (e.g. kelvin.anthropic): "
    IFS= read -r provider_id
    provider_id="${provider_id// /}"
  fi

  if [[ -z "${provider_id}" ]]; then
    echo "error: no provider specified" >&2
    exit 1
  fi

  mkdir -p "${PLUGIN_HOME}" "$(dirname "${TRUST_POLICY_PATH}")"
  export KELVIN_PLUGIN_HOME="${PLUGIN_HOME}"
  export KELVIN_TRUST_POLICY_PATH="${TRUST_POLICY_PATH}"

  echo "[setup-gateway] installing: ${provider_id}"
  "${ROOT_DIR}/share/scripts/plugin-index-install.sh" \
    --plugin "${provider_id}" \
    --index-url "${INDEX_URL}"

  echo
  echo "Plugin installed. To use it, start the gateway with:"
  echo "  KELVIN_MODEL_PROVIDER=${provider_id} ./start-gateway [args...]"
}

if [[ $# -eq 0 ]]; then
  usage
  exit 0
fi

SUBCOMMAND="$1"
shift

case "${SUBCOMMAND}" in
  install-provider)
    cmd_install_provider "$@"
    ;;
  list)
    require_cmd jq
    cmd_list
    ;;
  status)
    require_cmd jq
    cmd_status
    ;;
  -h|--help)
    usage
    exit 0
    ;;
  *)
    echo "Unknown subcommand: ${SUBCOMMAND}" >&2
    echo >&2
    usage >&2
    exit 1
    ;;
esac

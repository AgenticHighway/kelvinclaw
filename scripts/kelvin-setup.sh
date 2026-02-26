#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KELVIN_HOME="${KELVIN_HOME:-/kelvin}"
PLUGIN_HOME="${KELVIN_PLUGIN_HOME:-${KELVIN_HOME}/plugins}"
TRUST_POLICY_PATH="${KELVIN_TRUST_POLICY_PATH:-${KELVIN_HOME}/trusted_publishers.json}"
SETUP_MARKER="${KELVIN_HOME}/.setup_complete"

INTERACTIVE="1"
FORCE="0"
INDEX_URL="${KELVIN_PLUGIN_INDEX_URL:-}"

usage() {
  cat <<'USAGE'
Usage: scripts/kelvin-setup.sh [options]

Interactive first-run setup for Kelvin runtime containers.

Options:
  --index-url <url>   Plugin index URL (overrides $KELVIN_PLUGIN_INDEX_URL)
  --force             Re-run setup even if already completed
  --non-interactive   Fail if required setup inputs are missing
  -h, --help          Show help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --index-url)
      INDEX_URL="${2:?missing value for --index-url}"
      shift 2
      ;;
    --force)
      FORCE="1"
      shift
      ;;
    --non-interactive)
      INTERACTIVE="0"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

require_cmd() {
  local name="$1"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "Missing required command: ${name}" >&2
    exit 1
  fi
}

require_cmd jq
require_cmd curl
require_cmd tar

mkdir -p "${KELVIN_HOME}" "${PLUGIN_HOME}" "$(dirname "${TRUST_POLICY_PATH}")"
export KELVIN_PLUGIN_HOME="${PLUGIN_HOME}"
export KELVIN_TRUST_POLICY_PATH="${TRUST_POLICY_PATH}"

if [[ -f "${SETUP_MARKER}" && "${FORCE}" != "1" ]]; then
  echo "[kelvin-setup] already completed: ${SETUP_MARKER}"
  echo "[kelvin-setup] plugin_home=${KELVIN_PLUGIN_HOME}"
  echo "[kelvin-setup] trust_policy=${KELVIN_TRUST_POLICY_PATH}"
  exit 0
fi

echo "Welcome to KelvinClaw."
echo "This runtime is minimal and installs plugins separately."
echo
echo "Runtime paths:"
echo "  KELVIN_HOME=${KELVIN_HOME}"
echo "  KELVIN_PLUGIN_HOME=${KELVIN_PLUGIN_HOME}"
echo "  KELVIN_TRUST_POLICY_PATH=${KELVIN_TRUST_POLICY_PATH}"
echo

CLI_PLUGIN_DIR="${KELVIN_PLUGIN_HOME}/kelvin.cli/current"
if [[ -d "${CLI_PLUGIN_DIR}" ]]; then
  echo "[kelvin-setup] kelvin.cli already installed: ${CLI_PLUGIN_DIR}"
else
  if [[ -z "${INDEX_URL}" && "${INTERACTIVE}" == "1" ]]; then
    read -r -p "Enter plugin index URL for Kelvin plugins: " INDEX_URL
  fi

  if [[ -z "${INDEX_URL}" ]]; then
    echo "[kelvin-setup] missing plugin index URL; cannot install required plugin kelvin.cli" >&2
    echo "[kelvin-setup] rerun with --index-url <url> or set KELVIN_PLUGIN_INDEX_URL" >&2
    exit 1
  fi

  echo "[kelvin-setup] installing required plugin: kelvin.cli"
  "${ROOT_DIR}/scripts/plugin-index-install.sh" \
    --index-url "${INDEX_URL}" \
    --plugin "kelvin.cli" \
    --plugin-home "${KELVIN_PLUGIN_HOME}" \
    --trust-policy-path "${KELVIN_TRUST_POLICY_PATH}"
fi

cat > "${SETUP_MARKER}" <<EOF
setup_completed_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
plugin_home=${KELVIN_PLUGIN_HOME}
trust_policy_path=${KELVIN_TRUST_POLICY_PATH}
EOF

echo "[kelvin-setup] setup complete"
echo
echo "Next step example:"
echo "  kelvin-host --prompt \"What is KelvinClaw?\" --timeout-ms 3000"

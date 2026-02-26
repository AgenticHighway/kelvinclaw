#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

KELVIN_HOME="${KELVIN_HOME:-/kelvin}"
export KELVIN_HOME
export KELVIN_PLUGIN_HOME="${KELVIN_PLUGIN_HOME:-${KELVIN_HOME}/plugins}"
export KELVIN_TRUST_POLICY_PATH="${KELVIN_TRUST_POLICY_PATH:-${KELVIN_HOME}/trusted_publishers.json}"

mkdir -p "${KELVIN_HOME}" "${KELVIN_PLUGIN_HOME}" "$(dirname "${KELVIN_TRUST_POLICY_PATH}")"

if [[ $# -gt 0 ]]; then
  exec "$@"
fi

if [[ -t 1 ]]; then
  echo "======================================="
  echo " KelvinClaw Runtime (Container)"
  echo " Security-first, stable, modular host"
  echo "======================================="
  echo
  "${ROOT_DIR}/scripts/kelvin-setup.sh"
  echo
  echo "Interactive shell ready."
  echo "Try:"
  echo "  kelvin-host --prompt \"What is KelvinClaw?\" --timeout-ms 3000"
  exec bash
fi

echo "[runtime-entrypoint] non-interactive mode without command; exiting."
exit 1

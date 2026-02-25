#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="$(mktemp -d)"
PLUGIN_HOME="${WORK_DIR}/plugins"
TRUST_POLICY_PATH="${WORK_DIR}/trusted_publishers.json"
TARGET_DIR="${ROOT_DIR}/target/test-cli-plugin-dogfood"
LOG_PATH="${WORK_DIR}/cli.log"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

echo "[test-cli-plugin-dogfood] installing bundled kelvin_cli package"
"${ROOT_DIR}/scripts/install-kelvin-cli-plugin.sh" \
  --plugin-home "${PLUGIN_HOME}" \
  --trust-policy-path "${TRUST_POLICY_PATH}"

echo "[test-cli-plugin-dogfood] running kelvin-host via SDK"
KELVIN_PLUGIN_HOME="${PLUGIN_HOME}" \
KELVIN_TRUST_POLICY_PATH="${TRUST_POLICY_PATH}" \
CARGO_TARGET_DIR="${TARGET_DIR}" \
  cargo run -p kelvin-host -- \
    --prompt "dogfood sdk lane" \
    --timeout-ms 5000 > "${LOG_PATH}"

if ! grep -q "cli plugin preflight: kelvin_cli executed" "${LOG_PATH}"; then
  echo "[test-cli-plugin-dogfood] expected cli plugin preflight output not found" >&2
  cat "${LOG_PATH}" >&2
  exit 1
fi

echo "[test-cli-plugin-dogfood] success"

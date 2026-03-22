#!/usr/bin/env bash
# gateway-plugin-init.sh — Runs first-time setup for the gateway's shared plugin volume.
#
# Called by the kelvin-init service in docker-compose before kelvin-gateway starts.
# Installs kelvin.cli (required) and conditionally installs the model-provider plugin
# specified by KELVIN_MODEL_PROVIDER (default: kelvin.echo, no plugin needed).
#
# Environment variables:
#   KELVIN_HOME              — shared home volume root  (default: /kelvin)
#   KELVIN_PLUGIN_HOME       — plugin install path      (default: /kelvin/plugins)
#   KELVIN_TRUST_POLICY_PATH — trust policy file        (default: /kelvin/trusted_publishers.json)
#   KELVIN_MODEL_PROVIDER    — model-provider plugin id (default: kelvin.echo)
#   ANTHROPIC_API_KEY        — required when KELVIN_MODEL_PROVIDER=kelvin.anthropic
set -euo pipefail

SCRIPTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

KELVIN_MODEL_PROVIDER="${KELVIN_MODEL_PROVIDER:-kelvin.echo}"
KELVIN_PLUGIN_HOME="${KELVIN_PLUGIN_HOME:-${KELVIN_HOME:-/kelvin}/plugins}"
KELVIN_TRUST_POLICY_PATH="${KELVIN_TRUST_POLICY_PATH:-${KELVIN_HOME:-/kelvin}/trusted_publishers.json}"

export KELVIN_PLUGIN_HOME
export KELVIN_TRUST_POLICY_PATH

# Standard setup: creates dirs, installs kelvin.cli, writes setup marker.
"${SCRIPTS_DIR}/kelvin-setup.sh" --non-interactive

# Install the model-provider plugin if it requires a WASM plugin (not the built-in Echo).
case "${KELVIN_MODEL_PROVIDER}" in
  kelvin.echo)
    echo "[gateway-plugin-init] using built-in echo provider — no additional plugin required"
    ;;
  kelvin.anthropic)
    if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
      echo "[gateway-plugin-init] KELVIN_MODEL_PROVIDER=kelvin.anthropic but ANTHROPIC_API_KEY is not set" >&2
      exit 1
    fi
    ANTHROPIC_PLUGIN_DIR="${KELVIN_PLUGIN_HOME}/kelvin.anthropic"
    if [[ -d "${ANTHROPIC_PLUGIN_DIR}" ]]; then
      echo "[gateway-plugin-init] kelvin.anthropic already installed: ${ANTHROPIC_PLUGIN_DIR}"
    else
      echo "[gateway-plugin-init] installing kelvin.anthropic plugin"
      "${SCRIPTS_DIR}/plugin-index-install.sh" \
        --plugin "kelvin.anthropic" \
        --plugin-home "${KELVIN_PLUGIN_HOME}" \
        --trust-policy-path "${KELVIN_TRUST_POLICY_PATH}"
    fi
    ;;
  *)
    echo "[gateway-plugin-init] KELVIN_MODEL_PROVIDER=${KELVIN_MODEL_PROVIDER} — ensure plugin is pre-installed in the plugin volume"
    ;;
esac

echo "[gateway-plugin-init] init complete (model-provider=${KELVIN_MODEL_PROVIDER})"

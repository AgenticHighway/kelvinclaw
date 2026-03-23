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
BUILTIN_PLUGIN_DIR="${BUILTIN_PLUGIN_DIR:-/opt/kelvin/plugins-builtin}"

export KELVIN_PLUGIN_HOME
export KELVIN_TRUST_POLICY_PATH

# Install a first-party plugin from a source directory baked into the container image.
install_builtin_plugin() {
  local src_dir="$1"
  local plugin_id; plugin_id="$(jq -r '.id' "${src_dir}/plugin.json")"
  local plugin_version; plugin_version="$(jq -r '.version' "${src_dir}/plugin.json")"
  local install_dir="${KELVIN_PLUGIN_HOME}/${plugin_id}/${plugin_version}"
  if [[ -d "${install_dir}" ]]; then
    echo "[gateway-plugin-init] ${plugin_id}@${plugin_version} already installed"
    return 0
  fi
  mkdir -p "${install_dir}"
  cp "${src_dir}/plugin.json" "${install_dir}/plugin.json"
  cp -r "${src_dir}/payload" "${install_dir}/payload"
  ln -sfn "${plugin_version}" "${KELVIN_PLUGIN_HOME}/${plugin_id}/current"
  echo "[gateway-plugin-init] installed ${plugin_id}@${plugin_version} from built-in"
}

# Standard setup: creates dirs, installs kelvin.cli, writes setup marker.
"${SCRIPTS_DIR}/kelvin-setup.sh" --non-interactive

# Write a permissive trust policy if none exists. First-party plugins are unsigned_local;
# signature enforcement is disabled until a signed distribution flow is in place.
if [[ ! -f "${KELVIN_TRUST_POLICY_PATH}" ]]; then
  mkdir -p "$(dirname "${KELVIN_TRUST_POLICY_PATH}")"
  echo '{"require_signature":false,"publishers":[]}' > "${KELVIN_TRUST_POLICY_PATH}"
  echo "[gateway-plugin-init] wrote permissive trust policy: ${KELVIN_TRUST_POLICY_PATH}"
fi

# Install the model-provider plugin if it requires a WASM plugin (not the built-in Echo).
case "${KELVIN_MODEL_PROVIDER}" in
  kelvin.echo)
    install_builtin_plugin "${BUILTIN_PLUGIN_DIR}/kelvin-echo-plugin"
    ;;
  kelvin.anthropic)
    if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
      echo "[gateway-plugin-init] KELVIN_MODEL_PROVIDER=kelvin.anthropic but ANTHROPIC_API_KEY is not set" >&2
      exit 1
    fi
    install_builtin_plugin "${BUILTIN_PLUGIN_DIR}/kelvin-anthropic-plugin"
    ;;
  kelvin.openrouter)
    install_builtin_plugin "${BUILTIN_PLUGIN_DIR}/kelvin-openrouter-plugin"
    ;;
  *)
    echo "[gateway-plugin-init] KELVIN_MODEL_PROVIDER=${KELVIN_MODEL_PROVIDER} — ensure plugin is pre-installed in the plugin volume"
    ;;
esac

echo "[gateway-plugin-init] init complete (model-provider=${KELVIN_MODEL_PROVIDER})"

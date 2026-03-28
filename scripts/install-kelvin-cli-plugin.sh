#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_INDEX_URL="https://raw.githubusercontent.com/AgenticHighway/kelvinclaw-plugins/main/index.json"

INDEX_URL="${KELVIN_PLUGIN_INDEX_URL:-${DEFAULT_INDEX_URL}}"
PLUGIN_HOME="${KELVIN_PLUGIN_HOME:-${ROOT_DIR}/.kelvin/plugins}"
PLUGIN_HOME="${PLUGIN_HOME/#\~/${HOME}}"
TRUST_POLICY_PATH="${KELVIN_TRUST_POLICY_PATH:-${ROOT_DIR}/.kelvin/trusted_publishers.json}"
TRUST_POLICY_PATH="${TRUST_POLICY_PATH/#\~/${HOME}}"
PLUGIN_VERSION=""
FORCE="0"

usage() {
  cat <<'USAGE'
Usage: scripts/install-kelvin-cli-plugin.sh [options]

Install Kelvin's first-party CLI WASM plugin from the plugin index.

Options:
  --index-url <url>           Plugin index URL
                              (default: $KELVIN_PLUGIN_INDEX_URL or set $KELVIN_PLUGIN_INDEX_URL for a community index)
  --version <version>         Specific plugin version (default: latest from index)
  --plugin-home <dir>         Plugin install root (default: $KELVIN_PLUGIN_HOME or ./.kelvin/plugins)
  --trust-policy-path <path>  Trust policy file path (default: $KELVIN_TRUST_POLICY_PATH or ./.kelvin/trusted_publishers.json)
  --force                     Reinstall plugin version if it already exists
  -h, --help                  Show help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --index-url)
      INDEX_URL="${2:?missing value for --index-url}"
      shift 2
      ;;
    --version)
      PLUGIN_VERSION="${2:?missing value for --version}"
      shift 2
      ;;
    --plugin-home)
      PLUGIN_HOME="${2:?missing value for --plugin-home}"
      shift 2
      ;;
    --trust-policy-path)
      TRUST_POLICY_PATH="${2:?missing value for --trust-policy-path}"
      shift 2
      ;;
    --force)
      FORCE="1"
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

# Prefer vendored tarball when no index URL is configured (offline / fresh clone).
VENDOR_DIR="${ROOT_DIR}/release/vendor"
VENDORED_TARBALL="$(ls -1 "${VENDOR_DIR}"/kelvin.cli-*.tar.gz 2>/dev/null | sort -V | tail -1 || true)"

if [[ -n "${VENDORED_TARBALL}" && -z "${INDEX_URL}" ]]; then
  echo "[install-kelvin-cli-plugin] installing from vendored tarball: ${VENDORED_TARBALL}"
  INSTALL_ARGS=(--package "${VENDORED_TARBALL}" --plugin-home "${PLUGIN_HOME}")
  if [[ "${FORCE}" == "1" ]]; then
    INSTALL_ARGS+=(--force)
  fi
  "${ROOT_DIR}/scripts/plugin-install.sh" "${INSTALL_ARGS[@]}"

  # Write a default trust policy if none exists (vendored path skips the index
  # installer which normally handles trust policy merging).
  if [[ ! -f "${TRUST_POLICY_PATH}" ]]; then
    mkdir -p "$(dirname "${TRUST_POLICY_PATH}")"
    echo '{"require_signature":false,"publishers":[]}' > "${TRUST_POLICY_PATH}"
    echo "[install-kelvin-cli-plugin] wrote default trust policy: ${TRUST_POLICY_PATH}"
  fi
  exit 0
fi

if [[ -z "${INDEX_URL}" ]]; then
  echo "No plugin index URL configured and no vendored tarball found." >&2
  echo "Set KELVIN_PLUGIN_INDEX_URL or pass --index-url <url>." >&2
  exit 1
fi

INSTALL_ARGS=(
  --index-url "${INDEX_URL}"
  --plugin "kelvin.cli"
  --plugin-home "${PLUGIN_HOME}"
  --trust-policy-path "${TRUST_POLICY_PATH}"
)
if [[ -n "${PLUGIN_VERSION}" ]]; then
  INSTALL_ARGS+=(--version "${PLUGIN_VERSION}")
fi
if [[ "${FORCE}" == "1" ]]; then
  INSTALL_ARGS+=(--force)
fi

"${ROOT_DIR}/scripts/plugin-index-install.sh" "${INSTALL_ARGS[@]}"

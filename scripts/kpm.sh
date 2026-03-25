#!/usr/bin/env bash
# kpm — Kelvin Plugin Manager
#
# Manage KelvinClaw plugins: install, uninstall, search, update, list, info, status.
#
# Environment variables:
#   KELVIN_PLUGIN_INDEX_URL    Plugin index URL (required for: install, search, info, update)
#   KELVIN_MODEL_PROVIDER      Active model provider plugin id (informational in status output)
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

# ── helpers ──────────────────────────────────────────────────────────────────

require_cmd() {
  local name="$1"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "error: missing required command: ${name}" >&2
    exit 1
  fi
}

require_index_url() {
  if [[ -z "${INDEX_URL}" ]]; then
    echo "error: KELVIN_PLUGIN_INDEX_URL is required for this command" >&2
    exit 1
  fi
}

fetch_index() {
  curl -fsSL --max-time 15 "${INDEX_URL}"
}

plugin_current_version() {
  local plugin_id="$1"
  local current_link="${PLUGIN_HOME}/${plugin_id}/current"
  if [[ -L "${current_link}" ]]; then
    basename "$(readlink "${current_link}")"
    return 0
  fi
  if [[ -f "${current_link}/plugin.json" ]]; then
    jq -r '.version // empty' "${current_link}/plugin.json" 2>/dev/null
    return 0
  fi
  return 1
}

# ── usage ─────────────────────────────────────────────────────────────────────

usage() {
  cat <<'USAGE'
Usage: kpm <subcommand> [options]

Kelvin Plugin Manager — install and manage KelvinClaw plugins.

Subcommands:
  install <plugin-id> [--version <ver>] [--force]
                         Install a plugin from the index
  uninstall <plugin-id> [--yes]
                         Remove an installed plugin
  update [<plugin-id>] [--dry-run]
                         Update installed plugins to the latest index version
  search [<query>]       List available plugins from the index
  info <plugin-id>       Show detailed metadata for a plugin from the index
  list                   List installed plugins
  status                 Show current configuration and installed plugins

Options:
  -h, --help   Show this help

Environment:
  KELVIN_PLUGIN_INDEX_URL    Plugin index URL (required for install, search, info, update)
  KELVIN_MODEL_PROVIDER      Active model provider (informational in status output)
  KELVIN_HOME                State root (default: ~/.kelvinclaw)
  KELVIN_PLUGIN_HOME         Override plugin install root
  KELVIN_TRUST_POLICY_PATH   Override trust policy path
USAGE
}

# ── subcommands ───────────────────────────────────────────────────────────────

cmd_install() {
  local plugin_id=""
  local plugin_version=""
  local force="0"

  # First positional arg (if not a flag) is the plugin id.
  if [[ $# -gt 0 && "${1:0:1}" != "-" ]]; then
    plugin_id="$1"
    shift
  fi

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --version)
        plugin_version="${2:?missing value for --version}"
        shift 2
        ;;
      --force)
        force="1"
        shift
        ;;
      *)
        echo "error: unknown argument: $1" >&2
        exit 1
        ;;
    esac
  done

  require_cmd curl
  require_cmd tar
  require_cmd jq
  require_index_url

  # Interactive selection when no plugin id is provided on a TTY.
  if [[ -z "${plugin_id}" ]]; then
    if [[ ! -t 0 || ! -t 1 ]]; then
      echo "error: plugin id is required in non-interactive mode" >&2
      echo "  Usage: kpm install <plugin-id>" >&2
      exit 1
    fi
    local index_json entries
    echo "Fetching available plugins from index..."
    index_json="$(fetch_index)"
    entries="$(printf '%s' "${index_json}" | jq -r '.plugins[] | "\(.id)  \(.version)  \(.description // "")"' 2>/dev/null || true)"
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
    printf "Enter plugin id: "
    IFS= read -r plugin_id
    plugin_id="${plugin_id// /}"
  fi

  if [[ -z "${plugin_id}" ]]; then
    echo "error: no plugin id specified" >&2
    exit 1
  fi

  mkdir -p "${PLUGIN_HOME}" "$(dirname "${TRUST_POLICY_PATH}")"
  export KELVIN_PLUGIN_HOME="${PLUGIN_HOME}"
  export KELVIN_TRUST_POLICY_PATH="${TRUST_POLICY_PATH}"

  local install_args=(
    --plugin "${plugin_id}"
    --index-url "${INDEX_URL}"
  )
  if [[ -n "${plugin_version}" ]]; then
    install_args+=(--version "${plugin_version}")
  fi
  if [[ "${force}" == "1" ]]; then
    install_args+=(--force)
  fi

  "${ROOT_DIR}/share/scripts/plugin-index-install.sh" "${install_args[@]}"
}

cmd_uninstall() {
  local plugin_id=""
  local yes="0"

  if [[ $# -gt 0 && "${1:0:1}" != "-" ]]; then
    plugin_id="$1"
    shift
  fi

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --yes|-y)
        yes="1"
        shift
        ;;
      *)
        echo "error: unknown argument: $1" >&2
        exit 1
        ;;
    esac
  done

  if [[ -z "${plugin_id}" ]]; then
    echo "error: plugin id is required" >&2
    echo "  Usage: kpm uninstall <plugin-id>" >&2
    exit 1
  fi

  local plugin_dir="${PLUGIN_HOME}/${plugin_id}"
  if [[ ! -d "${plugin_dir}" ]]; then
    echo "error: plugin not installed: ${plugin_id}" >&2
    exit 1
  fi

  if [[ "${yes}" == "0" && -t 0 && -t 1 ]]; then
    printf "Remove %s from %s? [y/N] " "${plugin_id}" "${plugin_dir}"
    local answer
    IFS= read -r answer
    if [[ "${answer}" != "y" && "${answer}" != "Y" ]]; then
      echo "Aborted."
      exit 0
    fi
  fi

  rm -rf "${plugin_dir}"
  echo "Removed ${plugin_id}"
}

cmd_update() {
  local plugin_id=""
  local dry_run="0"

  if [[ $# -gt 0 && "${1:0:1}" != "-" ]]; then
    plugin_id="$1"
    shift
  fi

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --dry-run)
        dry_run="1"
        shift
        ;;
      *)
        echo "error: unknown argument: $1" >&2
        exit 1
        ;;
    esac
  done

  require_cmd curl
  require_cmd tar
  require_cmd jq
  require_index_url

  if [[ ! -d "${PLUGIN_HOME}" ]]; then
    echo "No plugins installed."
    return 0
  fi

  local index_json
  index_json="$(fetch_index)"

  local updated="0"
  local checked="0"

  check_and_update_plugin() {
    local id="$1"
    local installed_version
    installed_version="$(plugin_current_version "${id}" || true)"
    if [[ -z "${installed_version}" ]]; then
      return 0
    fi

    local index_version
    index_version="$(printf '%s' "${index_json}" | jq -r --arg id "${id}" '.plugins[] | select(.id == $id) | .version' 2>/dev/null || true)"
    if [[ -z "${index_version}" ]]; then
      echo "  ${id}: not found in index (skipping)"
      return 0
    fi

    checked=$((checked + 1))
    if [[ "${installed_version}" == "${index_version}" ]]; then
      echo "  ${id}: up to date (${installed_version})"
      return 0
    fi

    echo "  ${id}: ${installed_version} → ${index_version}"
    if [[ "${dry_run}" == "1" ]]; then
      return 0
    fi

    mkdir -p "${PLUGIN_HOME}" "$(dirname "${TRUST_POLICY_PATH}")"
    export KELVIN_PLUGIN_HOME="${PLUGIN_HOME}"
    export KELVIN_TRUST_POLICY_PATH="${TRUST_POLICY_PATH}"

    "${ROOT_DIR}/share/scripts/plugin-index-install.sh" \
      --plugin "${id}" \
      --index-url "${INDEX_URL}" \
      --force
    updated=$((updated + 1))
  }

  if [[ -n "${plugin_id}" ]]; then
    check_and_update_plugin "${plugin_id}"
  else
    for plugin_dir in "${PLUGIN_HOME}"/*/; do
      [[ -d "${plugin_dir}" ]] || continue
      check_and_update_plugin "$(basename "${plugin_dir}")"
    done
  fi

  if [[ "${dry_run}" == "1" ]]; then
    echo "(dry run — no changes made)"
  elif [[ "${updated}" -gt 0 ]]; then
    echo "${updated} plugin(s) updated."
  else
    echo "All plugins up to date."
  fi
}

cmd_search() {
  local query="${1:-}"

  require_cmd curl
  require_cmd jq
  require_index_url

  local index_json
  index_json="$(fetch_index)"

  local jq_filter
  if [[ -n "${query}" ]]; then
    jq_filter='[.plugins[] | select((.id | test($q;"i")) or (.name // "" | test($q;"i")))]'
  else
    jq_filter='.plugins'
  fi

  local results
  results="$(printf '%s' "${index_json}" | jq -r --arg q "${query}" "${jq_filter}"' | .[] | [.id, .version, (.description // "(no description)")] | @tsv' 2>/dev/null || true)"

  if [[ -z "${results}" ]]; then
    if [[ -n "${query}" ]]; then
      echo "No plugins matching: ${query}"
    else
      echo "No plugins found in index."
    fi
    return 0
  fi

  # Print with column formatting.
  printf '%-30s  %-10s  %s\n' "ID" "VERSION" "DESCRIPTION"
  printf '%-30s  %-10s  %s\n' "──────────────────────────────" "──────────" "───────────────────────────────────────"
  while IFS=$'\t' read -r id version description; do
    # Truncate description to 60 chars.
    if [[ ${#description} -gt 60 ]]; then
      description="${description:0:57}..."
    fi
    printf '%-30s  %-10s  %s\n' "${id}" "${version}" "${description}"
  done <<< "${results}"
}

cmd_info() {
  local plugin_id="${1:-}"

  if [[ -z "${plugin_id}" ]]; then
    echo "error: plugin id is required" >&2
    echo "  Usage: kpm info <plugin-id>" >&2
    exit 1
  fi

  require_cmd curl
  require_cmd jq
  require_index_url

  local index_json plugin_json
  index_json="$(fetch_index)"
  plugin_json="$(printf '%s' "${index_json}" | jq --arg id "${plugin_id}" '.plugins[] | select(.id == $id)' 2>/dev/null || true)"

  if [[ -z "${plugin_json}" || "${plugin_json}" == "null" ]]; then
    echo "error: plugin not found in index: ${plugin_id}" >&2
    exit 1
  fi

  local installed_version
  installed_version="$(plugin_current_version "${plugin_id}" || true)"

  echo "id:           $(printf '%s' "${plugin_json}" | jq -r '.id')"
  echo "name:         $(printf '%s' "${plugin_json}" | jq -r '.name // "(none)"')"
  echo "version:      $(printf '%s' "${plugin_json}" | jq -r '.version')"
  if [[ -n "${installed_version}" ]]; then
    echo "installed:    ${installed_version}"
  else
    echo "installed:    (not installed)"
  fi
  echo "description:  $(printf '%s' "${plugin_json}" | jq -r '.description // "(none)"')"
  echo "homepage:     $(printf '%s' "${plugin_json}" | jq -r '.homepage // "(none)"')"
  echo "capabilities: $(printf '%s' "${plugin_json}" | jq -r '.capabilities | join(", ")')"
  echo "runtime:      $(printf '%s' "${plugin_json}" | jq -r '.runtime // "(none)"')"
  echo "quality_tier: $(printf '%s' "${plugin_json}" | jq -r '.quality_tier // "(none)"')"
  echo "sha256:       $(printf '%s' "${plugin_json}" | jq -r '.sha256 // "(none)"')"
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
    local version
    version="$(plugin_current_version "${plugin_id}" || echo "(unknown)")"
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

# ── dispatch ──────────────────────────────────────────────────────────────────

if [[ $# -eq 0 ]]; then
  usage
  exit 0
fi

SUBCOMMAND="$1"
shift

case "${SUBCOMMAND}" in
  install)
    cmd_install "$@"
    ;;
  uninstall)
    cmd_uninstall "$@"
    ;;
  update)
    require_cmd jq
    cmd_update "$@"
    ;;
  search)
    cmd_search "${1:-}"
    ;;
  info)
    cmd_info "${1:-}"
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
    echo "error: unknown subcommand: ${SUBCOMMAND}" >&2
    echo >&2
    usage >&2
    exit 1
    ;;
esac

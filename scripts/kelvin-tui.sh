#!/usr/bin/env bash
# kelvin-tui.sh — Release-bundle launcher for kelvin-tui.
#
# Loads .env files automatically, then execs kelvin-tui.
#
# Environment variables:
#   KELVIN_GATEWAY_TOKEN   Auth token for the gateway (read by kelvin-tui directly)
#   KELVIN_GATEWAY_URL     Override gateway WebSocket URL (default: ws://127.0.0.1:34617)
#   KELVIN_HOME            State root directory (default: ~/.kelvinclaw)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -x "${SCRIPT_DIR}/bin/kelvin-tui" ]]; then
  ROOT_DIR="${SCRIPT_DIR}"
else
  ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
fi
DEFAULT_KELVIN_HOME="${KELVIN_HOME:-${HOME}/.kelvinclaw}"
DEFAULT_KELVIN_HOME="${DEFAULT_KELVIN_HOME/#\~/${HOME}}"

# ── dotenv loader ─────────────────────────────────────────────────────────────
_ktui_trim()   { local v="$1"; v="${v#"${v%%[![:space:]]*}"}"; v="${v%"${v##*[![:space:]]}"}"; printf '%s' "${v}"; }
_ktui_unquote() {
  local v="$1"
  if [[ "${v}" == \"*\" ]] && [[ "${v}" == *\" ]]; then printf '%s' "${v:1:${#v}-2}"; return; fi
  if [[ "${v}" == \'*\' ]] && [[ "${v}" == *\' ]]; then printf '%s' "${v:1:${#v}-2}"; return; fi
  printf '%s' "${v}"
}
load_dotenv() {
  local f line stripped key value
  for f in "${DEFAULT_KELVIN_HOME}/.env.local" "${DEFAULT_KELVIN_HOME}/.env" "${PWD}/.env.local" "${PWD}/.env"; do
    [[ -f "${f}" ]] || continue
    while IFS= read -r line || [[ -n "${line}" ]]; do
      stripped="$(_ktui_trim "${line%%#*}")"
      [[ -z "${stripped}" ]] && continue
      [[ "${stripped}" =~ ^export[[:space:]]+ ]] && stripped="$(_ktui_trim "${stripped#export }")"
      if [[ "${stripped}" =~ ^([A-Za-z_][A-Za-z0-9_]*)[[:space:]]*=[[:space:]]*(.*)$ ]]; then
        key="${BASH_REMATCH[1]}"
        value="$(_ktui_unquote "$(_ktui_trim "${BASH_REMATCH[2]}")")"
        [[ -z "${!key+x}" ]] && export "${key}=${value}"
      fi
    done < "${f}"
  done
  return 0
}
load_dotenv
# ──────────────────────────────────────────────────────────────────────────────

usage() {
  cat <<'USAGE'
Usage: ./kelvin-tui [kelvin-tui args]

Release-bundle launcher for kelvin-tui. Loads .env files automatically.

Environment:
  KELVIN_GATEWAY_TOKEN   Auth token for the gateway (required)
  KELVIN_HOME            State root (default: ~/.kelvinclaw)

The launcher reads KELVIN_GATEWAY_TOKEN from:
  - ~/.kelvinclaw/.env.local / ~/.kelvinclaw/.env
  - ./.env.local / ./.env

Pass --help to kelvin-tui for its full option list.
USAGE
}

if [[ $# -gt 0 ]]; then
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
  esac
fi

exec "${ROOT_DIR}/bin/kelvin-tui" "$@"

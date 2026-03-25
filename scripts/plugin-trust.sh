#!/usr/bin/env bash
set -euo pipefail

TRUST_POLICY_DEFAULT="${HOME}/.kelvinclaw/trusted_publishers.json"
TRUST_POLICY_PATH="${KELVIN_TRUST_POLICY_PATH:-${TRUST_POLICY_DEFAULT}}"
TRUST_POLICY_PATH="${TRUST_POLICY_PATH/#\~/${HOME}}"

usage() {
  cat <<'USAGE'
Usage: scripts/plugin-trust.sh <command> [options]

Commands:
  show
    Print trust policy JSON.

  rotate-key --publisher <id> --public-key <base64>
    Upsert publisher public key for key rotation.

  revoke --publisher <id>
    Revoke publisher id (loader rejects plugins from it).

  unrevoke --publisher <id>
    Remove publisher id from revocation list.

  pin --plugin <plugin-id> --publisher <id>
    Pin plugin id to a publisher id.

  unpin --plugin <plugin-id>
    Remove pin for plugin id.

Global options:
  --trust-policy <path>   Path to trust policy file
USAGE
}

require_cmd() {
  local name="$1"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "Missing required command: ${name}" >&2
    exit 1
  fi
}

init_policy_if_missing() {
  if [[ -f "${TRUST_POLICY_PATH}" ]]; then
    return 0
  fi
  mkdir -p "$(dirname "${TRUST_POLICY_PATH}")"
  jq -n '{
    require_signature: true,
    publishers: [],
    revoked_publishers: [],
    pinned_plugin_publishers: {}
  }' > "${TRUST_POLICY_PATH}"
}

cmd_show() {
  init_policy_if_missing
  jq '.' "${TRUST_POLICY_PATH}"
}

cmd_rotate_key() {
  local publisher="" public_key=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --publisher) publisher="${2:?missing value for --publisher}"; shift 2 ;;
      --public-key) public_key="${2:?missing value for --public-key}"; shift 2 ;;
      *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
  done
  [[ -n "${publisher}" && -n "${public_key}" ]] || {
    echo "rotate-key requires --publisher and --public-key" >&2
    exit 1
  }
  init_policy_if_missing
  local tmp="${TRUST_POLICY_PATH}.tmp"
  jq \
    --arg publisher "${publisher}" \
    --arg public_key "${public_key}" \
    '
      .publishers = (
        (.publishers // [])
        | map(select(.id != $publisher))
        + [{id:$publisher, ed25519_public_key:$public_key}]
      )
    ' "${TRUST_POLICY_PATH}" > "${tmp}"
  mv "${tmp}" "${TRUST_POLICY_PATH}"
  echo "Updated key for publisher '${publisher}' in ${TRUST_POLICY_PATH}"
}

cmd_revoke() {
  local publisher=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --publisher) publisher="${2:?missing value for --publisher}"; shift 2 ;;
      *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
  done
  [[ -n "${publisher}" ]] || {
    echo "revoke requires --publisher" >&2
    exit 1
  }
  init_policy_if_missing
  local tmp="${TRUST_POLICY_PATH}.tmp"
  jq \
    --arg publisher "${publisher}" \
    '
      .revoked_publishers = (
        ((.revoked_publishers // []) + [$publisher])
        | unique
      )
    ' "${TRUST_POLICY_PATH}" > "${tmp}"
  mv "${tmp}" "${TRUST_POLICY_PATH}"
  echo "Revoked publisher '${publisher}' in ${TRUST_POLICY_PATH}"
}

cmd_unrevoke() {
  local publisher=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --publisher) publisher="${2:?missing value for --publisher}"; shift 2 ;;
      *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
  done
  [[ -n "${publisher}" ]] || {
    echo "unrevoke requires --publisher" >&2
    exit 1
  }
  init_policy_if_missing
  local tmp="${TRUST_POLICY_PATH}.tmp"
  jq \
    --arg publisher "${publisher}" \
    '.revoked_publishers = ((.revoked_publishers // []) | map(select(. != $publisher)))' \
    "${TRUST_POLICY_PATH}" > "${tmp}"
  mv "${tmp}" "${TRUST_POLICY_PATH}"
  echo "Removed revocation for publisher '${publisher}' in ${TRUST_POLICY_PATH}"
}

cmd_pin() {
  local plugin_id="" publisher=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --plugin) plugin_id="${2:?missing value for --plugin}"; shift 2 ;;
      --publisher) publisher="${2:?missing value for --publisher}"; shift 2 ;;
      *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
  done
  [[ -n "${plugin_id}" && -n "${publisher}" ]] || {
    echo "pin requires --plugin and --publisher" >&2
    exit 1
  }
  init_policy_if_missing
  local tmp="${TRUST_POLICY_PATH}.tmp"
  jq \
    --arg plugin_id "${plugin_id}" \
    --arg publisher "${publisher}" \
    '.pinned_plugin_publishers = (.pinned_plugin_publishers // {}) | .pinned_plugin_publishers[$plugin_id] = $publisher' \
    "${TRUST_POLICY_PATH}" > "${tmp}"
  mv "${tmp}" "${TRUST_POLICY_PATH}"
  echo "Pinned plugin '${plugin_id}' to publisher '${publisher}' in ${TRUST_POLICY_PATH}"
}

cmd_unpin() {
  local plugin_id=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --plugin) plugin_id="${2:?missing value for --plugin}"; shift 2 ;;
      *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
  done
  [[ -n "${plugin_id}" ]] || {
    echo "unpin requires --plugin" >&2
    exit 1
  }
  init_policy_if_missing
  local tmp="${TRUST_POLICY_PATH}.tmp"
  jq \
    --arg plugin_id "${plugin_id}" \
    'del(.pinned_plugin_publishers[$plugin_id])' \
    "${TRUST_POLICY_PATH}" > "${tmp}"
  mv "${tmp}" "${TRUST_POLICY_PATH}"
  echo "Removed pin for plugin '${plugin_id}' in ${TRUST_POLICY_PATH}"
}

main() {
  require_cmd jq
  if [[ $# -lt 1 ]]; then
    usage
    exit 1
  fi
  local command="$1"
  shift || true

  local forwarded=()
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --trust-policy)
        TRUST_POLICY_PATH="${2:?missing value for --trust-policy}"
        shift 2
        ;;
      *)
        forwarded+=("$1")
        shift
        ;;
    esac
  done
  set -- "${forwarded[@]}"

  case "${command}" in
    show) cmd_show "$@" ;;
    rotate-key) cmd_rotate_key "$@" ;;
    revoke) cmd_revoke "$@" ;;
    unrevoke) cmd_unrevoke "$@" ;;
    pin) cmd_pin "$@" ;;
    unpin) cmd_unpin "$@" ;;
    -h|--help) usage ;;
    *) echo "Unknown command: ${command}" >&2; usage; exit 1 ;;
  esac
}

main "$@"

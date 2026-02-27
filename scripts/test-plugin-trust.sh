#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TRUST_CLI="${ROOT_DIR}/scripts/plugin-trust.sh"

require_cmd() {
  local name="$1"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "Missing required command: ${name}" >&2
    exit 1
  fi
}

require_cmd jq

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "${WORK_DIR}"' EXIT
POLICY="${WORK_DIR}/trusted_publishers.json"

"${TRUST_CLI}" rotate-key --trust-policy "${POLICY}" --publisher acme --public-key AAAABBBBCCCC
"${TRUST_CLI}" pin --trust-policy "${POLICY}" --plugin acme.echo --publisher acme
"${TRUST_CLI}" revoke --trust-policy "${POLICY}" --publisher bad.publisher

jq -e '.publishers | any(.id == "acme")' "${POLICY}" >/dev/null
jq -e '.pinned_plugin_publishers["acme.echo"] == "acme"' "${POLICY}" >/dev/null
jq -e '.revoked_publishers | index("bad.publisher") != null' "${POLICY}" >/dev/null

"${TRUST_CLI}" unrevoke --trust-policy "${POLICY}" --publisher bad.publisher
"${TRUST_CLI}" unpin --trust-policy "${POLICY}" --plugin acme.echo

jq -e '(.revoked_publishers | index("bad.publisher")) == null' "${POLICY}" >/dev/null
jq -e '(.pinned_plugin_publishers["acme.echo"]) == null' "${POLICY}" >/dev/null

echo "[test-plugin-trust] success"

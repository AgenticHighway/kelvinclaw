#!/usr/bin/env bash
set -euo pipefail

MANIFEST_PATH=""
PRIVATE_KEY_PATH=""
SIGNATURE_PATH=""
PUBLISHER_ID=""
TRUST_POLICY_OUT=""

usage() {
  cat <<'USAGE'
Usage: scripts/plugin-sign.sh --manifest <plugin.json> --private-key <ed25519-private-key.pem> [options]

Signs a plugin manifest and writes plugin.sig (base64 signature) for Kelvin installed-plugin verification.

Required:
  --manifest <path>        Path to plugin.json to sign
  --private-key <path>     Ed25519 private key in PEM format

Optional:
  --output <path>          Signature output path (default: <manifest_dir>/plugin.sig)
  --publisher-id <id>      Publisher id for trust policy snippet output
  --trust-policy-out <path>Write trusted_publishers.json snippet with derived public key
  -h, --help               Show this help

Example:
  scripts/plugin-sign.sh \
    --manifest ~/.kelvinclaw/plugins/acme.echo/1.0.0/plugin.json \
    --private-key ~/.kelvinclaw/keys/acme-ed25519-private.pem \
    --publisher-id acme \
    --trust-policy-out ./trusted_publishers.acme.json
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --manifest)
      MANIFEST_PATH="${2:?missing value for --manifest}"
      shift 2
      ;;
    --private-key)
      PRIVATE_KEY_PATH="${2:?missing value for --private-key}"
      shift 2
      ;;
    --output)
      SIGNATURE_PATH="${2:?missing value for --output}"
      shift 2
      ;;
    --publisher-id)
      PUBLISHER_ID="${2:?missing value for --publisher-id}"
      shift 2
      ;;
    --trust-policy-out)
      TRUST_POLICY_OUT="${2:?missing value for --trust-policy-out}"
      shift 2
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

require_cmd openssl
require_cmd awk
require_cmd xxd
require_cmd jq

if [[ -z "${MANIFEST_PATH}" || -z "${PRIVATE_KEY_PATH}" ]]; then
  echo "Missing required arguments." >&2
  usage
  exit 1
fi

if [[ ! -f "${MANIFEST_PATH}" ]]; then
  echo "Manifest not found: ${MANIFEST_PATH}" >&2
  exit 1
fi
if [[ ! -f "${PRIVATE_KEY_PATH}" ]]; then
  echo "Private key not found: ${PRIVATE_KEY_PATH}" >&2
  exit 1
fi

if [[ -z "${SIGNATURE_PATH}" ]]; then
  SIGNATURE_PATH="$(cd "$(dirname "${MANIFEST_PATH}")" && pwd)/plugin.sig"
fi

WORK_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

SIG_BIN_PATH="${WORK_DIR}/plugin.sig.bin"

# Ed25519 signs raw message bytes; plugin runtime verifies plugin.sig over plugin.json bytes.
openssl pkeyutl -sign -inkey "${PRIVATE_KEY_PATH}" -rawin -in "${MANIFEST_PATH}" -out "${SIG_BIN_PATH}"
openssl base64 -A -in "${SIG_BIN_PATH}" > "${SIGNATURE_PATH}"

# Verify signature immediately before returning success.
PUB_PEM_PATH="${WORK_DIR}/public.pem"
openssl pkey -in "${PRIVATE_KEY_PATH}" -pubout -out "${PUB_PEM_PATH}" >/dev/null 2>&1
if ! openssl pkeyutl -verify -pubin -inkey "${PUB_PEM_PATH}" -rawin -in "${MANIFEST_PATH}" -sigfile "${SIG_BIN_PATH}" >/dev/null 2>&1; then
  echo "Signature verification failed after signing; refusing to continue." >&2
  exit 1
fi

echo "Wrote signature: ${SIGNATURE_PATH}"

# Derive raw 32-byte Ed25519 public key and emit base64 for Kelvin trust policy.
PUB_HEX="$(
  openssl pkey -in "${PRIVATE_KEY_PATH}" -pubout -text -noout 2>/dev/null \
    | awk '
      /^pub:/ {capture=1; next}
      capture && /^[[:space:]]*$/ {capture=0; next}
      capture {gsub(/[ :]/, "", $0); printf "%s", $0}
    '
)"

if [[ ${#PUB_HEX} -ne 64 ]]; then
  echo "Failed to derive a raw 32-byte Ed25519 public key from private key." >&2
  exit 1
fi

PUB_B64="$(printf '%s' "${PUB_HEX}" | xxd -r -p | openssl base64 -A)"
echo "Derived publisher public key (base64): ${PUB_B64}"

if [[ -n "${TRUST_POLICY_OUT}" ]]; then
  if [[ -z "${PUBLISHER_ID}" ]]; then
    echo "--publisher-id is required when --trust-policy-out is used." >&2
    exit 1
  fi
  jq -n \
    --arg publisher_id "${PUBLISHER_ID}" \
    --arg public_key "${PUB_B64}" \
    '{
      require_signature: true,
      publishers: [
        {
          id: $publisher_id,
          ed25519_public_key: $public_key
        }
      ]
    }' > "${TRUST_POLICY_OUT}"
  echo "Wrote trust policy snippet: ${TRUST_POLICY_OUT}"
fi

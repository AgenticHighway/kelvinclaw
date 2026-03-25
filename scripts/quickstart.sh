#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/lib/rust-toolchain-path.sh"

MODE="local" # local | docker
PROMPT="${KELVIN_QUICKSTART_PROMPT:-What is KelvinClaw?}"

usage() {
  cat <<'USAGE'
Usage: scripts/quickstart.sh [options]

Canonical quick start for KelvinClaw Daily Driver MVP.

Options:
  --mode <local|docker>  Run local profile or runtime container flow (default: local)
  --prompt <text>        Prompt used for local smoke run
  -h, --help             Show help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      MODE="${2:?missing value for --mode}"
      shift 2
      ;;
    --prompt)
      PROMPT="${2:?missing value for --prompt}"
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

if [[ "${MODE}" == "docker" ]]; then
  echo "[quickstart] mode=docker"
  exec "${ROOT_DIR}/scripts/run-runtime-container.sh"
fi

if [[ "${MODE}" != "local" ]]; then
  echo "Invalid mode: ${MODE} (expected local or docker)" >&2
  exit 1
fi

echo "[quickstart] mode=local"

# If no model API key is set and we're in an interactive TTY, offer a choice.
if [[ -z "${OPENAI_API_KEY:-}" && -z "${ANTHROPIC_API_KEY:-}" && -z "${OPENROUTER_API_KEY:-}" ]]; then
  if [[ -t 0 && -t 1 ]]; then
    echo ""
    echo "No LLM API key detected. Pick a model provider for this run:"
    echo ""
    echo "  1) OpenAI       (needs OPENAI_API_KEY)"
    echo "  2) Anthropic    (needs ANTHROPIC_API_KEY)"
    echo "  3) OpenRouter   (needs OPENROUTER_API_KEY)"
    echo "  4) Echo mode    (no key needed — responses are echoed back)"
    echo ""
    printf "Choice [1-4, default 4]: "
    read -r choice
    case "${choice}" in
      1)
        printf "Paste your OpenAI API key: "
        IFS= read -r -s api_key; printf '\n'
        if [[ -n "${api_key}" ]]; then export OPENAI_API_KEY="${api_key}"; fi
        ;;
      2)
        printf "Paste your Anthropic API key: "
        IFS= read -r -s api_key; printf '\n'
        if [[ -n "${api_key}" ]]; then export ANTHROPIC_API_KEY="${api_key}"; fi
        ;;
      3)
        printf "Paste your OpenRouter API key: "
        IFS= read -r -s api_key; printf '\n'
        if [[ -n "${api_key}" ]]; then export OPENROUTER_API_KEY="${api_key}"; fi
        ;;
      4|"")
        echo "[quickstart] continuing with echo mode"
        ;;
      *)
        echo "[quickstart] invalid choice, continuing with echo mode"
        ;;
    esac
  fi
fi

"${ROOT_DIR}/scripts/kelvin-local-profile.sh" start

ensure_rust_toolchain_path || {
  echo "[quickstart] cargo/rustup required for local host run" >&2
  exit 1
}

PLUGIN_HOME="${KELVIN_PLUGIN_HOME:-${ROOT_DIR}/.kelvin/plugins}"
TRUST_POLICY_PATH="${KELVIN_TRUST_POLICY_PATH:-${ROOT_DIR}/.kelvin/trusted_publishers.json}"
STATE_DIR="${KELVIN_STATE_DIR:-${ROOT_DIR}/.kelvin/state}"

MODEL_PROVIDER=""
if [[ -n "${OPENAI_API_KEY:-}" ]]; then
  MODEL_PROVIDER="kelvin.openai"
elif [[ -n "${ANTHROPIC_API_KEY:-}" ]]; then
  MODEL_PROVIDER="kelvin.anthropic"
elif [[ -n "${OPENROUTER_API_KEY:-}" ]]; then
  MODEL_PROVIDER="kelvin.openrouter"
fi

HOST_ARGS=(
  --prompt "${PROMPT}"
  --workspace "${ROOT_DIR}"
  --state-dir "${STATE_DIR}"
)
if [[ -n "${MODEL_PROVIDER}" ]]; then
  HOST_ARGS+=(--model-provider "${MODEL_PROVIDER}")
fi

KELVIN_PLUGIN_HOME="${PLUGIN_HOME}" \
KELVIN_TRUST_POLICY_PATH="${TRUST_POLICY_PATH}" \
  cargo run -p kelvin-host -- "${HOST_ARGS[@]}"

echo "[quickstart] success"

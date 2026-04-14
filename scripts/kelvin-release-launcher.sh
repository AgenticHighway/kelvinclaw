#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -x "${SCRIPT_DIR}/bin/kelvin-host" ]]; then
  ROOT_DIR="${SCRIPT_DIR}"
else
  ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
fi
KELVIN_HOME_DEFAULT="${HOME}/.kelvinclaw"
KELVIN_HOME="${KELVIN_HOME:-${KELVIN_HOME_DEFAULT}}"
KELVIN_HOME="${KELVIN_HOME/#\~/${HOME}}"
CONFIG_ENV_PATH="${KELVIN_HOME}/.env"
PLUGIN_HOME="${KELVIN_PLUGIN_HOME:-${KELVIN_HOME}/plugins}"
PLUGIN_HOME="${PLUGIN_HOME/#\~/${HOME}}"
TRUST_POLICY_PATH="${KELVIN_TRUST_POLICY_PATH:-${KELVIN_HOME}/trusted_publishers.json}"
TRUST_POLICY_PATH="${TRUST_POLICY_PATH/#\~/${HOME}}"
STATE_DIR="${KELVIN_STATE_DIR:-${KELVIN_HOME}/state}"
STATE_DIR="${STATE_DIR/#\~/${HOME}}"
DEFAULT_PROMPT="${KELVIN_DEFAULT_PROMPT:-What is KelvinClaw?}"
DEFAULT_PLUGIN_INDEX_URL="https://raw.githubusercontent.com/AgenticHighway/kelvinclaw-plugins/main/index.json"
DEFAULT_OLLAMA_BASE_URL="http://localhost:11434"
PLUGIN_MANIFEST_PATH="${ROOT_DIR}/share/official-first-party-plugins.env"
ENV_SEARCH_PATHS=(
  "${PWD}/.env.local"
  "${PWD}/.env"
  "${KELVIN_HOME}/.env.local"
  "${KELVIN_HOME}/.env"
)

usage() {
  cat <<'USAGE'
Usage: ./kelvin [init [options] | kelvin-host args]

Release-bundle launcher for KelvinClaw.

Behavior:
  - `kelvin init` writes ~/.kelvinclaw/.env for first-run setup
  - with no args, installs required official plugins on first run
  - starts interactive mode on a TTY
  - falls back to a default prompt when stdin/stdout are not TTYs

Environment:
  KELVIN_HOME                Override bundle-managed state root (default: ~/.kelvinclaw)
  KELVIN_PLUGIN_HOME         Override plugin install root
  KELVIN_TRUST_POLICY_PATH   Override trust policy path
  KELVIN_STATE_DIR           Override host state dir
  KELVIN_DEFAULT_PROMPT      Prompt used for non-interactive no-arg runs
  OPENAI_API_KEY             If set, installs and selects kelvin.openai on first run

The launcher also reads OPENAI_API_KEY from:
  - ./.env.local
  - ./.env
  - ~/.kelvinclaw/.env.local
  - ~/.kelvinclaw/.env
USAGE
}

show_init_usage() {
  cat <<'USAGE'
Usage: ./kelvin init [--provider <echo|openai|anthropic|openrouter|ollama>] [--force]

Initialize KelvinClaw's user config in ~/.kelvinclaw/.env.

Options:
  --provider <name>  Preselect a provider instead of prompting.
  --force            Overwrite an existing ~/.kelvinclaw/.env.
  -h, --help         Show this help.
USAGE
}

require_cmd() {
  local name="$1"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "Missing required command: ${name}" >&2
    exit 1
  fi
}

sha256_file() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${file}" | awk '{print $1}'
    return 0
  fi
  shasum -a 256 "${file}" | awk '{print $1}'
}

trim_whitespace() {
  local value="$1"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "${value}"
}

strip_wrapping_quotes() {
  local value="$1"
  if [[ "${value}" == \"*\" && "${value}" == *\" ]]; then
    printf '%s' "${value:1:${#value}-2}"
    return
  fi
  if [[ "${value}" == \'*\' && "${value}" == *\' ]]; then
    printf '%s' "${value:1:${#value}-2}"
    return
  fi
  printf '%s' "${value}"
}

read_secret_value() {
  local prompt="$1"
  local value=""
  printf '%s' "${prompt}" >&2
  IFS= read -r -s value
  printf '\n' >&2
  printf '%s' "${value}"
}

read_value_with_default() {
  local prompt="$1"
  local default_value="$2"
  local value=""
  printf '%s [%s]: ' "${prompt}" "${default_value}" >&2
  IFS= read -r value
  value="$(trim_whitespace "${value}")"
  if [[ -z "${value}" ]]; then
    printf '%s' "${default_value}"
    return 0
  fi
  printf '%s' "${value}"
}

config_template_path() {
  if [[ -f "${ROOT_DIR}/release/env.example" ]]; then
    printf '%s\n' "${ROOT_DIR}/release/env.example"
    return 0
  fi
  if [[ -f "${ROOT_DIR}/.env.example" ]]; then
    printf '%s\n' "${ROOT_DIR}/.env.example"
    return 0
  fi
  echo "Missing KelvinClaw config template (.env.example)" >&2
  exit 1
}

generate_gateway_token() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -hex 32
    return 0
  fi
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import secrets; print(secrets.token_hex(32))'
    return 0
  fi
  echo "Missing required command: openssl or python3" >&2
  exit 1
}

replace_or_append_env_line() {
  local file="$1"
  local key="$2"
  local value="$3"
  local temp_file=""
  temp_file="$(mktemp)"
  awk -v key="${key}" -v value="${value}" '
    BEGIN { replaced = 0 }
    $0 ~ "^[[:space:]]*" key "[[:space:]]*=" {
      print key "=" value
      replaced = 1
      next
    }
    { print }
    END {
      if (!replaced) {
        print key "=" value
      }
    }
  ' "${file}" > "${temp_file}"
  mv "${temp_file}" "${file}"
}

normalize_init_provider() {
  case "$1" in
    echo|kelvin.echo) printf '%s\n' 'echo' ;;
    openai|kelvin.openai) printf '%s\n' 'openai' ;;
    anthropic|kelvin.anthropic) printf '%s\n' 'anthropic' ;;
    openrouter|kelvin.openrouter) printf '%s\n' 'openrouter' ;;
    ollama|kelvin.ollama) printf '%s\n' 'ollama' ;;
    *)
      echo "Unsupported provider: $1" >&2
      echo "Expected one of: echo, openai, anthropic, openrouter, ollama" >&2
      exit 1
      ;;
  esac
}

prompt_for_init_provider() {
  local selection=""
  cat <<'PROMPT' >&2
[kelvin init] Choose a provider:
  1) kelvin.echo (Recommended)
  2) kelvin.openai
  3) kelvin.anthropic
  4) kelvin.openrouter
  5) kelvin.ollama
PROMPT
  printf '[kelvin init] Provider [1]: ' >&2
  IFS= read -r selection
  selection="$(trim_whitespace "${selection}")"
  case "${selection}" in
    ""|1) printf '%s\n' 'echo' ;;
    2) printf '%s\n' 'openai' ;;
    3) printf '%s\n' 'anthropic' ;;
    4) printf '%s\n' 'openrouter' ;;
    5) printf '%s\n' 'ollama' ;;
    *)
      normalize_init_provider "${selection}"
      ;;
  esac
}

run_init() {
  local force="0"
  local provider=""
  local provider_id=""
  local config_template=""
  local gateway_token=""
  local plugin_index_url="${KELVIN_PLUGIN_INDEX_URL:-${DEFAULT_PLUGIN_INDEX_URL}}"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --force)
        force="1"
        shift
        ;;
      --provider)
        provider="$(normalize_init_provider "${2:?missing value for --provider}")"
        shift 2
        ;;
      -h|--help)
        show_init_usage
        exit 0
        ;;
      *)
        echo "error: unknown init argument: $1" >&2
        show_init_usage >&2
        exit 1
        ;;
    esac
  done

  if [[ -z "${provider}" ]]; then
    if [[ -t 0 && -t 1 ]]; then
      provider="$(prompt_for_init_provider)"
    else
      provider="echo"
    fi
  fi

  mkdir -p "${KELVIN_HOME}"
  if [[ -f "${CONFIG_ENV_PATH}" && "${force}" != "1" ]]; then
    echo "error: ${CONFIG_ENV_PATH} already exists" >&2
    echo "Run 'kelvin init --force' to overwrite it." >&2
    exit 1
  fi

  config_template="$(config_template_path)"
  cp "${config_template}" "${CONFIG_ENV_PATH}"

  gateway_token="$(generate_gateway_token)"
  replace_or_append_env_line "${CONFIG_ENV_PATH}" "KELVIN_GATEWAY_TOKEN" "${gateway_token}"
  replace_or_append_env_line "${CONFIG_ENV_PATH}" "KELVIN_PLUGIN_INDEX_URL" "${plugin_index_url}"

  case "${provider}" in
    echo)
      provider_id="kelvin.echo"
      ;;
    openai)
      local openai_api_key="${OPENAI_API_KEY:-}"
      if [[ -z "${openai_api_key}" ]]; then
        if [[ ! -t 0 || ! -t 1 ]]; then
          echo "error: OPENAI_API_KEY must be set for non-interactive openai init" >&2
          exit 1
        fi
        openai_api_key="$(read_secret_value "[kelvin init] OpenAI API key: ")"
      fi
      openai_api_key="$(trim_whitespace "${openai_api_key}")"
      if [[ -z "${openai_api_key}" ]]; then
        echo "error: OPENAI_API_KEY cannot be empty" >&2
        exit 1
      fi
      replace_or_append_env_line "${CONFIG_ENV_PATH}" "OPENAI_API_KEY" "${openai_api_key}"
      provider_id="kelvin.openai"
      ;;
    anthropic)
      local anthropic_api_key="${ANTHROPIC_API_KEY:-}"
      if [[ -z "${anthropic_api_key}" ]]; then
        if [[ ! -t 0 || ! -t 1 ]]; then
          echo "error: ANTHROPIC_API_KEY must be set for non-interactive anthropic init" >&2
          exit 1
        fi
        anthropic_api_key="$(read_secret_value "[kelvin init] Anthropic API key: ")"
      fi
      anthropic_api_key="$(trim_whitespace "${anthropic_api_key}")"
      if [[ -z "${anthropic_api_key}" ]]; then
        echo "error: ANTHROPIC_API_KEY cannot be empty" >&2
        exit 1
      fi
      replace_or_append_env_line "${CONFIG_ENV_PATH}" "ANTHROPIC_API_KEY" "${anthropic_api_key}"
      provider_id="kelvin.anthropic"
      ;;
    openrouter)
      local openrouter_api_key="${OPENROUTER_API_KEY:-}"
      if [[ -z "${openrouter_api_key}" ]]; then
        if [[ ! -t 0 || ! -t 1 ]]; then
          echo "error: OPENROUTER_API_KEY must be set for non-interactive openrouter init" >&2
          exit 1
        fi
        openrouter_api_key="$(read_secret_value "[kelvin init] OpenRouter API key: ")"
      fi
      openrouter_api_key="$(trim_whitespace "${openrouter_api_key}")"
      if [[ -z "${openrouter_api_key}" ]]; then
        echo "error: OPENROUTER_API_KEY cannot be empty" >&2
        exit 1
      fi
      replace_or_append_env_line "${CONFIG_ENV_PATH}" "OPENROUTER_API_KEY" "${openrouter_api_key}"
      provider_id="kelvin.openrouter"
      ;;
    ollama)
      local ollama_base_url="${OLLAMA_BASE_URL:-${DEFAULT_OLLAMA_BASE_URL}}"
      if [[ -t 0 && -t 1 ]]; then
        ollama_base_url="$(read_value_with_default "[kelvin init] Ollama base URL" "${ollama_base_url}")"
      fi
      ollama_base_url="$(trim_whitespace "${ollama_base_url}")"
      if [[ -z "${ollama_base_url}" ]]; then
        echo "error: OLLAMA_BASE_URL cannot be empty" >&2
        exit 1
      fi
      replace_or_append_env_line "${CONFIG_ENV_PATH}" "OLLAMA_BASE_URL" "${ollama_base_url}"
      provider_id="kelvin.ollama"
      ;;
  esac

  replace_or_append_env_line "${CONFIG_ENV_PATH}" "KELVIN_MODEL_PROVIDER" "${provider_id}"

  echo "[kelvin init] Wrote ${CONFIG_ENV_PATH}"
  echo "[kelvin init] Provider: ${provider_id}"
  echo "[kelvin init] Next step: kelvin"
}

load_env_var_from_file() {
  local key="$1"
  local file="$2"
  local line=""
  local stripped=""
  local value=""
  [[ -f "${file}" ]] || return 1

  while IFS= read -r line || [[ -n "${line}" ]]; do
    stripped="$(trim_whitespace "${line%%#*}")"
    [[ -z "${stripped}" ]] && continue
    if [[ "${stripped}" =~ ^export[[:space:]]+ ]]; then
      stripped="$(trim_whitespace "${stripped#export }")"
    fi
    if [[ "${stripped}" =~ ^${key}[[:space:]]*=[[:space:]]*(.*)$ ]]; then
      value="$(trim_whitespace "${BASH_REMATCH[1]}")"
      strip_wrapping_quotes "${value}"
      return 0
    fi
  done < "${file}"

  return 1
}

load_dotenv() {
  local env_file line stripped key value
  for env_file in "${ENV_SEARCH_PATHS[@]}"; do
    [[ -f "${env_file}" ]] || continue
    while IFS= read -r line || [[ -n "${line}" ]]; do
      stripped="$(trim_whitespace "${line%%#*}")"
      [[ -z "${stripped}" ]] && continue
      [[ "${stripped}" =~ ^export[[:space:]]+ ]] && stripped="$(trim_whitespace "${stripped#export }")"
      if [[ "${stripped}" =~ ^([A-Za-z_][A-Za-z0-9_]*)[[:space:]]*=[[:space:]]*(.*)$ ]]; then
        key="${BASH_REMATCH[1]}"
        value="$(strip_wrapping_quotes "$(trim_whitespace "${BASH_REMATCH[2]}")")"
        [[ -z "${!key+x}" ]] && export "${key}=${value}"
      fi
    done < "${env_file}"
  done
}

prompt_for_openai_api_key() {
  local value=""
  [[ -n "${OPENAI_API_KEY:-}" ]] && return 0
  [[ $# -eq 0 ]] || return 0
  [[ -t 0 && -t 1 ]] || return 0

  echo "[kelvin] OPENAI_API_KEY not found in the environment or .env files."
  printf '[kelvin] Paste your OpenAI API key for this run, or press Enter to continue with echo mode: ' >&2
  IFS= read -r -s value
  printf '\n' >&2

  value="$(trim_whitespace "${value}")"
  if [[ -n "${value}" ]]; then
    export OPENAI_API_KEY="${value}"
  fi
}

plugin_current_version() {
  local plugin_id="$1"
  local current_link="${PLUGIN_HOME}/${plugin_id}/current"

  if [[ -L "${current_link}" ]]; then
    basename "$(readlink "${current_link}")"
    return 0
  fi
  if [[ -f "${current_link}/plugin.json" ]]; then
    awk -F'"' '/"version"[[:space:]]*:/ {print $4; exit}' "${current_link}/plugin.json"
    return 0
  fi
  return 1
}

ensure_trust_policy() {
  if [[ -f "${TRUST_POLICY_PATH}" ]]; then
    return 0
  fi
  mkdir -p "$(dirname "${TRUST_POLICY_PATH}")"
  echo "[kelvin] fetching official trust policy"
  curl -fsSL "${OFFICIAL_TRUST_POLICY_URL}" -o "${TRUST_POLICY_PATH}"
}

extract_package_cleanly() {
  local tarball_path="$1"
  local extract_dir="$2"
  local stderr_path="${extract_dir}/tar.stderr"

  mkdir -p "${extract_dir}"
  if ! tar -xzf "${tarball_path}" -C "${extract_dir}" 2>"${stderr_path}"; then
    cat "${stderr_path}" >&2 || true
    return 1
  fi

  if [[ -s "${stderr_path}" ]]; then
    if grep -Fv "Ignoring unknown extended header keyword 'LIBARCHIVE.xattr.com.apple.provenance'" "${stderr_path}" | grep -q .; then
      cat "${stderr_path}" >&2
      return 1
    fi
  fi

  find "${extract_dir}" -name '._*' -delete
  rm -f "${stderr_path}"
}

install_official_plugin() {
  local plugin_id="$1"
  local version="$2"
  local package_url="$3"
  local expected_sha="$4"
  local current_version=""
  local work_dir=""
  local package_path=""
  local install_dir=""
  local current_link=""

  current_version="$(plugin_current_version "${plugin_id}" || true)"
  if [[ "${current_version}" == "${version}" && -f "${PLUGIN_HOME}/${plugin_id}/${version}/plugin.json" ]]; then
    return 0
  fi

  echo "[kelvin] installing official plugin: ${plugin_id}@${version}"
  ensure_trust_policy
  mkdir -p "${PLUGIN_HOME}/${plugin_id}"

  work_dir="$(mktemp -d)"
  package_path="${work_dir}/plugin.tar.gz"
  curl -fsSL "${package_url}" -o "${package_path}"

  if [[ "$(sha256_file "${package_path}")" != "${expected_sha}" ]]; then
    echo "Checksum mismatch for ${plugin_id}@${version}" >&2
    rm -rf "${work_dir}"
    exit 1
  fi

  extract_package_cleanly "${package_path}" "${work_dir}/extract"
  install_dir="${PLUGIN_HOME}/${plugin_id}/${version}"
  current_link="${PLUGIN_HOME}/${plugin_id}/current"

  rm -rf "${install_dir}"
  mkdir -p "${install_dir}"
  cp -R "${work_dir}/extract/." "${install_dir}/"
  ln -sfn "${version}" "${current_link}"
  rm -rf "${work_dir}"
}

bootstrap_official_plugins() {
  require_cmd curl
  require_cmd tar
  require_cmd awk

  if [[ ! -f "${PLUGIN_MANIFEST_PATH}" ]]; then
    echo "Release bundle is missing ${PLUGIN_MANIFEST_PATH}" >&2
    exit 1
  fi
  # shellcheck disable=SC1090
  source "${PLUGIN_MANIFEST_PATH}"

  if [[ -n "${KELVIN_CLI_VERSION:-}" ]]; then
    install_official_plugin "kelvin.cli" "${KELVIN_CLI_VERSION}" "${KELVIN_CLI_PACKAGE_URL}" "${KELVIN_CLI_SHA256}"
  fi

  if [[ -n "${OPENAI_API_KEY:-}" && -n "${KELVIN_OPENAI_VERSION:-}" ]]; then
    install_official_plugin "kelvin.openai" "${KELVIN_OPENAI_VERSION}" "${KELVIN_OPENAI_PACKAGE_URL}" "${KELVIN_OPENAI_SHA256}"
  fi
}

if [[ $# -gt 0 ]]; then
  case "$1" in
    init)
      shift
      run_init "$@"
      exit 0
      ;;
    -h|--help)
      usage
      exit 0
      ;;
  esac
fi

load_dotenv
prompt_for_openai_api_key "$@"

bootstrap_official_plugins

mkdir -p "${STATE_DIR}"
export KELVIN_PLUGIN_HOME="${PLUGIN_HOME}"
export KELVIN_TRUST_POLICY_PATH="${TRUST_POLICY_PATH}"

DEFAULT_HOST_ARGS=()
if [[ -n "${OPENAI_API_KEY:-}" ]]; then
  DEFAULT_HOST_ARGS+=(--model-provider kelvin.openai)
fi

if [[ $# -eq 0 ]]; then
  if [[ -t 0 && -t 1 ]]; then
    exec "${ROOT_DIR}/bin/kelvin-host" \
      "${DEFAULT_HOST_ARGS[@]}" \
      --interactive \
      --workspace "$(pwd)" \
      --state-dir "${STATE_DIR}"
  fi

  exec "${ROOT_DIR}/bin/kelvin-host" \
    "${DEFAULT_HOST_ARGS[@]}" \
    --prompt "${DEFAULT_PROMPT}" \
    --workspace "$(pwd)" \
    --state-dir "${STATE_DIR}"
fi

exec "${ROOT_DIR}/bin/kelvin-host" "$@"

#!/usr/bin/env bash
# kelvin-medkit.sh — KelvinClaw diagnostic health check
# Usage: kelvin medkit
#        scripts/kelvin-medkit.sh [--json] [--fix]
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KELVIN_HOME="${KELVIN_HOME:-${HOME}/.kelvinclaw}"
PLUGIN_HOME="${KELVIN_PLUGIN_HOME:-${KELVIN_HOME}/plugins}"
TRUST_POLICY_PATH="${KELVIN_TRUST_POLICY_PATH:-${KELVIN_HOME}/trusted_publishers.json}"
STATE_DIR="${KELVIN_STATE_DIR:-${KELVIN_HOME}/state}"
OUTPUT_JSON="0"
AUTO_FIX="0"

PASS_COUNT=0
WARN_COUNT=0
FAIL_COUNT=0
TOTAL_COUNT=0

# ── colors ────────────────────────────────────────────────────────
if [[ -t 1 ]]; then
  C_RESET='\033[0m'
  C_GREEN='\033[1;32m'
  C_YELLOW='\033[1;33m'
  C_RED='\033[1;31m'
  C_CYAN='\033[1;36m'
  C_BOLD='\033[1m'
  C_DIM='\033[2m'
else
  C_RESET='' C_GREEN='' C_YELLOW='' C_RED='' C_CYAN='' C_BOLD='' C_DIM=''
fi

# ── argument parsing ──────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --json) OUTPUT_JSON="1"; shift ;;
    --fix)  AUTO_FIX="1"; shift ;;
    -h|--help)
      cat <<'EOF'
Usage: kelvin medkit [--json] [--fix]

Run diagnostic health checks on your KelvinClaw installation.

Options:
  --json    Emit JSON report instead of human-readable output
  --fix     Attempt to auto-fix problems where possible
  -h        Show this help

Checks:
  - System prerequisites (cargo, jq, curl, tar, openssl)
  - KelvinClaw home directory and directory structure
  - Environment configuration (.env files, API keys)
  - Trust policy existence and validity
  - Installed plugins and their integrity
  - Plugin index connectivity
  - Gateway / memory controller process state
  - Version information
EOF
      exit 0
      ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

# ── check helpers ─────────────────────────────────────────────────
section() {
  printf '\n%b── %s ──%b\n' "${C_CYAN}" "$1" "${C_RESET}"
}

check_pass() {
  TOTAL_COUNT=$((TOTAL_COUNT + 1))
  PASS_COUNT=$((PASS_COUNT + 1))
  printf '  %b✔%b  %s\n' "${C_GREEN}" "${C_RESET}" "$1"
}

check_warn() {
  TOTAL_COUNT=$((TOTAL_COUNT + 1))
  WARN_COUNT=$((WARN_COUNT + 1))
  printf '  %b⚠%b  %s\n' "${C_YELLOW}" "${C_RESET}" "$1"
  if [[ -n "${2:-}" ]]; then
    printf '     %b↳ %s%b\n' "${C_DIM}" "$2" "${C_RESET}"
  fi
}

check_fail() {
  TOTAL_COUNT=$((TOTAL_COUNT + 1))
  FAIL_COUNT=$((FAIL_COUNT + 1))
  printf '  %b✘%b  %s\n' "${C_RED}" "${C_RESET}" "$1"
  if [[ -n "${2:-}" ]]; then
    printf '     %b↳ %s%b\n' "${C_DIM}" "$2" "${C_RESET}"
  fi
}

# ── version banner ────────────────────────────────────────────────
KELVIN_VERSION=""
if [[ -f "${ROOT_DIR}/Cargo.toml" ]]; then
  KELVIN_VERSION="$(awk -F'"' '/^\[workspace\.package\]/{found=1} found && /^version[[:space:]]*=/{print $2; exit}' "${ROOT_DIR}/Cargo.toml" 2>/dev/null || true)"
fi
if [[ -f "${ROOT_DIR}/BUILD_INFO.txt" ]] && [[ -z "${KELVIN_VERSION}" ]]; then
  KELVIN_VERSION="$(awk -F= '/^version=/{print $2}' "${ROOT_DIR}/BUILD_INFO.txt" 2>/dev/null || true)"
fi

printf '\n%b🩺 KelvinClaw Medkit%b' "${C_BOLD}" "${C_RESET}"
if [[ -n "${KELVIN_VERSION}" ]]; then
  printf '  %bv%s%b' "${C_DIM}" "${KELVIN_VERSION}" "${C_RESET}"
fi
printf '\n'

# ── 1. System prerequisites ──────────────────────────────────────
section "System Prerequisites"

check_command() {
  local cmd="$1"
  local hint="${2:-}"
  if command -v "${cmd}" >/dev/null 2>&1; then
    local ver=""
    case "${cmd}" in
      cargo)   ver="$(cargo --version 2>/dev/null | awk '{print $2}')" ;;
      rustc)   ver="$(rustc --version 2>/dev/null | awk '{print $2}')" ;;
      jq)      ver="$(jq --version 2>/dev/null)" ;;
      curl)    ver="$(curl --version 2>/dev/null | head -1 | awk '{print $2}')" ;;
      openssl) ver="$(openssl version 2>/dev/null | awk '{print $2}')" ;;
      docker)  ver="$(docker --version 2>/dev/null | awk -F'[ ,]' '{print $3}')" ;;
    esac
    if [[ -n "${ver}" ]]; then
      check_pass "${cmd} (${ver})"
    else
      check_pass "${cmd}"
    fi
  else
    if [[ -n "${hint}" ]]; then
      check_fail "${cmd} not found" "${hint}"
    else
      check_fail "${cmd} not found"
    fi
  fi
}

check_command cargo "Install from https://rustup.rs"
check_command rustc "Install from https://rustup.rs"
check_command jq "brew install jq / apt install jq"
check_command curl
check_command tar
check_command openssl

# Check ed25519 support (macOS LibreSSL sometimes lacks it)
if command -v openssl >/dev/null 2>&1; then
  if openssl genpkey -algorithm ed25519 -out /dev/null 2>/dev/null; then
    check_pass "openssl ed25519 support"
  else
    check_warn "openssl lacks ed25519 support" "Memory controller dev keys require ed25519. Install openssl@3: brew install openssl@3"
  fi
fi

# Optional tools
if command -v docker >/dev/null 2>&1; then
  check_pass "docker (optional, $(docker --version 2>/dev/null | awk -F'[ ,]' '{print $3}'))"
else
  check_warn "docker not found (optional)" "Required for Docker deployment mode"
fi

# ── 2. Directory structure ────────────────────────────────────────
section "Directory Structure"

check_dir() {
  local dir="$1"
  local label="${2:-${dir}}"
  if [[ -d "${dir}" ]]; then
    check_pass "${label}"
  else
    if [[ "${AUTO_FIX}" == "1" ]]; then
      mkdir -p "${dir}"
      check_pass "${label} (created)"
    else
      check_fail "${label} missing" "Run: mkdir -p ${dir}"
    fi
  fi
}

check_dir "${KELVIN_HOME}" "KELVIN_HOME (${KELVIN_HOME})"
check_dir "${PLUGIN_HOME}" "Plugin home (${PLUGIN_HOME})"
check_dir "${STATE_DIR}" "State directory (${STATE_DIR})"

# ── 3. Environment / API keys ────────────────────────────────────
section "Configuration"

ENV_FILES_FOUND=0
for env_path in \
  "${PWD}/.env.local" \
  "${PWD}/.env" \
  "${KELVIN_HOME}/.env.local" \
  "${KELVIN_HOME}/.env"; do
  if [[ -f "${env_path}" ]]; then
    check_pass ".env found: ${env_path}"
    ENV_FILES_FOUND=$((ENV_FILES_FOUND + 1))
  fi
done
if [[ "${ENV_FILES_FOUND}" -eq 0 ]]; then
  check_warn "No .env file found" "Create ${KELVIN_HOME}/.env with your API keys"
fi

# Check for API keys
PROVIDER_COUNT=0
if [[ -n "${OPENAI_API_KEY:-}" ]]; then
  # Validate format (starts with sk-)
  if [[ "${OPENAI_API_KEY}" == sk-* ]]; then
    check_pass "OPENAI_API_KEY set (sk-...)"
  else
    check_warn "OPENAI_API_KEY set but unusual format" "Expected prefix: sk-"
  fi
  PROVIDER_COUNT=$((PROVIDER_COUNT + 1))
fi
if [[ -n "${ANTHROPIC_API_KEY:-}" ]]; then
  if [[ "${ANTHROPIC_API_KEY}" == sk-ant-* ]]; then
    check_pass "ANTHROPIC_API_KEY set (sk-ant-...)"
  else
    check_warn "ANTHROPIC_API_KEY set but unusual format" "Expected prefix: sk-ant-"
  fi
  PROVIDER_COUNT=$((PROVIDER_COUNT + 1))
fi
if [[ -n "${OPENROUTER_API_KEY:-}" ]]; then
  if [[ "${OPENROUTER_API_KEY}" == sk-or-* ]]; then
    check_pass "OPENROUTER_API_KEY set (sk-or-...)"
  else
    check_warn "OPENROUTER_API_KEY set but unusual format" "Expected prefix: sk-or-"
  fi
  PROVIDER_COUNT=$((PROVIDER_COUNT + 1))
fi
if [[ "${PROVIDER_COUNT}" -eq 0 ]]; then
  check_warn "No model provider API keys detected" "Set OPENAI_API_KEY, ANTHROPIC_API_KEY, or OPENROUTER_API_KEY for a real model. Echo mode works without keys."
fi

# Check KELVIN_MODEL_PROVIDER
if [[ -n "${KELVIN_MODEL_PROVIDER:-}" ]]; then
  check_pass "KELVIN_MODEL_PROVIDER=${KELVIN_MODEL_PROVIDER}"
else
  check_warn "KELVIN_MODEL_PROVIDER not set" "Will auto-detect from API keys or default to echo"
fi

# ── 4. Trust policy ──────────────────────────────────────────────
section "Trust Policy"

if [[ -f "${TRUST_POLICY_PATH}" ]]; then
  # Validate it's valid JSON
  if jq empty "${TRUST_POLICY_PATH}" 2>/dev/null; then
    REQ_SIG="$(jq -r '.require_signature // "unset"' "${TRUST_POLICY_PATH}")"
    PUB_COUNT="$(jq '.publishers | length // 0' "${TRUST_POLICY_PATH}" 2>/dev/null || echo 0)"
    check_pass "Trust policy: ${TRUST_POLICY_PATH} (require_signature=${REQ_SIG}, ${PUB_COUNT} publishers)"
  else
    check_fail "Trust policy is invalid JSON" "Delete and re-create: rm ${TRUST_POLICY_PATH}"
  fi
else
  if [[ "${AUTO_FIX}" == "1" ]]; then
    mkdir -p "$(dirname "${TRUST_POLICY_PATH}")"
    echo '{"require_signature":false,"publishers":[]}' > "${TRUST_POLICY_PATH}"
    check_pass "Trust policy created: ${TRUST_POLICY_PATH}"
  else
    check_fail "Trust policy missing: ${TRUST_POLICY_PATH}" "Run: kelvin medkit --fix"
  fi
fi

# ── 5. Installed plugins ─────────────────────────────────────────
section "Installed Plugins"

INSTALLED_PLUGINS=0
if [[ -d "${PLUGIN_HOME}" ]]; then
  for plugin_dir in "${PLUGIN_HOME}"/*/; do
    [[ -d "${plugin_dir}" ]] || continue
    plugin_id="$(basename "${plugin_dir}")"

    # Resolve current version
    current_link="${plugin_dir}current"
    if [[ -L "${current_link}" ]]; then
      version="$(basename "$(readlink "${current_link}")")"
    elif [[ -d "${current_link}" ]] && [[ -f "${current_link}/plugin.json" ]]; then
      version="$(jq -r '.version // "unknown"' "${current_link}/plugin.json" 2>/dev/null || echo "unknown")"
    else
      check_warn "Plugin ${plugin_id}: no current version" "Reinstall: kelvin plugin index-install --plugin ${plugin_id} --force"
      continue
    fi

    # Check plugin.json integrity
    manifest="${plugin_dir}${version}/plugin.json"
    if [[ ! -f "${manifest}" ]]; then
      manifest="${current_link}/plugin.json"
    fi

    if [[ -f "${manifest}" ]]; then
      if jq empty "${manifest}" 2>/dev/null; then
        p_name="$(jq -r '.name // .id' "${manifest}")"
        p_runtime="$(jq -r '.runtime // "unknown"' "${manifest}")"
        check_pass "${plugin_id}@${version} (${p_runtime})"

        # Check WASM entrypoint
        entrypoint="$(jq -r '.entrypoint // ""' "${manifest}")"
        if [[ -n "${entrypoint}" ]]; then
          wasm_path="${plugin_dir}${version}/payload/${entrypoint}"
          if [[ ! -f "${wasm_path}" ]]; then
            wasm_path="${current_link}/payload/${entrypoint}"
          fi
          if [[ ! -f "${wasm_path}" ]]; then
            check_fail "  ${plugin_id}: missing WASM payload (${entrypoint})" "Reinstall plugin"
          fi
        fi
      else
        check_fail "${plugin_id}@${version}: corrupt plugin.json" "Reinstall: kelvin plugin index-install --plugin ${plugin_id} --force"
      fi
    else
      check_warn "${plugin_id}@${version}: no plugin.json manifest"
    fi
    INSTALLED_PLUGINS=$((INSTALLED_PLUGINS + 1))
  done
fi

if [[ "${INSTALLED_PLUGINS}" -eq 0 ]]; then
  check_warn "No plugins installed" "Run: kelvin plugin index-install --plugin kelvin.cli"
fi

# Check required plugin: kelvin.cli
if [[ -d "${PLUGIN_HOME}/kelvin.cli/current" ]]; then
  check_pass "Required plugin kelvin.cli: installed"
else
  check_fail "Required plugin kelvin.cli: missing" "Run: kelvin plugin index-install --plugin kelvin.cli"
fi

# ── 6. Plugin index connectivity ─────────────────────────────────
section "Plugin Index"

DEFAULT_INDEX_URL="https://raw.githubusercontent.com/AgenticHighway/kelvinclaw/main/index.json"
INDEX_URL="${KELVIN_PLUGIN_INDEX_URL:-${DEFAULT_INDEX_URL}}"

if curl -fsSL --max-time 10 "${INDEX_URL}" -o /dev/null 2>/dev/null; then
  REMOTE_COUNT="$(curl -fsSL --max-time 10 "${INDEX_URL}" 2>/dev/null | jq '.plugins | length' 2>/dev/null || echo 0)"
  check_pass "Plugin index reachable (${INDEX_URL}, ${REMOTE_COUNT} plugins)"
else
  check_warn "Plugin index unreachable" "Check network or KELVIN_PLUGIN_INDEX_URL"
fi

# ── 7. Process state ─────────────────────────────────────────────
section "Running Services"

PROFILE_DIR="${KELVIN_LOCAL_PROFILE_DIR:-${KELVIN_HOME}/local-profile}"
check_pid_file() {
  local name="$1"
  local pid_file="$2"
  if [[ -f "${pid_file}" ]]; then
    local pid
    pid="$(cat "${pid_file}")"
    if kill -0 "${pid}" 2>/dev/null; then
      check_pass "${name}: running (pid ${pid})"
    else
      check_warn "${name}: stale PID file (pid ${pid} not running)" "Remove: rm ${pid_file}"
    fi
  else
    check_warn "${name}: not running" "Start with: scripts/kelvin-local-profile.sh start"
  fi
}

check_pid_file "Memory controller" "${PROFILE_DIR}/memory-controller.pid"
check_pid_file "Gateway" "${PROFILE_DIR}/gateway.pid"

# ── 8. Build state (dev mode only) ───────────────────────────────
if [[ -f "${ROOT_DIR}/Cargo.toml" ]]; then
  section "Build State"

  if [[ -x "${ROOT_DIR}/target/debug/kelvin-gateway" ]]; then
    check_pass "kelvin-gateway binary: built"
  else
    check_warn "kelvin-gateway binary: not built" "Run: cargo build -p kelvin-gateway --features memory_rpc"
  fi

  if [[ -x "${ROOT_DIR}/target/debug/kelvin-host" ]]; then
    check_pass "kelvin-host binary: built"
  else
    check_warn "kelvin-host binary: not built" "Run: cargo build -p kelvin-host"
  fi

  if [[ -x "${ROOT_DIR}/target/debug/kelvin-memory-controller" ]]; then
    check_pass "kelvin-memory-controller binary: built"
  else
    check_warn "kelvin-memory-controller binary: not built" "Run: cargo build -p kelvin-memory-controller"
  fi
fi

# ── 9. Security checks ───────────────────────────────────────────
section "Security"

# Check for insecure public bind
if [[ "${KELVIN_GATEWAY_ALLOW_INSECURE_PUBLIC_BIND:-}" == "1" || "${KELVIN_GATEWAY_ALLOW_INSECURE_PUBLIC_BIND:-}" == "true" ]]; then
  check_warn "KELVIN_GATEWAY_ALLOW_INSECURE_PUBLIC_BIND is enabled" "Disable for production use"
fi

# Check gateway token
GATEWAY_BIND="${KELVIN_GATEWAY_BIND:-127.0.0.1:34617}"
if [[ "${GATEWAY_BIND}" != 127.0.0.1:* && "${GATEWAY_BIND}" != localhost:* ]]; then
  if [[ -z "${KELVIN_GATEWAY_TOKEN:-}" ]]; then
    check_fail "Gateway bound to non-loopback address without token" "Set KELVIN_GATEWAY_TOKEN for non-local binds"
  else
    if [[ "${KELVIN_GATEWAY_TOKEN}" == "change-me" || "${#KELVIN_GATEWAY_TOKEN}" -lt 16 ]]; then
      check_fail "Gateway token is weak or default" "Use a strong random token (32+ characters)"
    else
      check_pass "Gateway token set for non-local bind"
    fi
  fi
else
  check_pass "Gateway bound to loopback (${GATEWAY_BIND})"
fi

# Check .env not in git
if [[ -f "${ROOT_DIR}/.gitignore" ]]; then
  if grep -qF '.env' "${ROOT_DIR}/.gitignore" 2>/dev/null; then
    check_pass ".env is in .gitignore"
  else
    check_warn ".env not in .gitignore" "Add .env to .gitignore to prevent secret leaks"
  fi
fi

# ── Summary ───────────────────────────────────────────────────────
printf '\n%b── Summary ──%b\n' "${C_CYAN}" "${C_RESET}"
printf '  %b%d passed%b  ' "${C_GREEN}" "${PASS_COUNT}" "${C_RESET}"
printf '%b%d warnings%b  ' "${C_YELLOW}" "${WARN_COUNT}" "${C_RESET}"
printf '%b%d failed%b  ' "${C_RED}" "${FAIL_COUNT}" "${C_RESET}"
printf '%b(%d total checks)%b\n' "${C_DIM}" "${TOTAL_COUNT}" "${C_RESET}"

if [[ "${FAIL_COUNT}" -gt 0 ]]; then
  printf '\n  %bSome checks failed.%b Run %bkelvin medkit --fix%b to auto-fix where possible.\n' "${C_RED}" "${C_RESET}" "${C_BOLD}" "${C_RESET}"
  exit 1
elif [[ "${WARN_COUNT}" -gt 0 ]]; then
  printf '\n  %bAll critical checks passed%b with %d warnings.\n' "${C_GREEN}" "${C_RESET}" "${WARN_COUNT}"
  exit 0
else
  printf '\n  %bAll checks passed. KelvinClaw is healthy!%b 🦀\n' "${C_GREEN}" "${C_RESET}"
  exit 0
fi

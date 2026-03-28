#!/usr/bin/env bash
# test-bash32-compat.sh — smoke-test user-facing scripts under bash 3.2
# Usage: bash scripts/test-bash32-compat.sh
# Requires: Docker
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="kelvinclaw-bash32-test"
PASS=0
FAIL=0

run_test() {
  local name="$1"; shift
  local result exit_code=0
  result="$(docker run --rm -v "${ROOT_DIR}:/repo" -w /repo "${IMAGE}" "$@" 2>&1)" || exit_code=$?
  if [[ ${exit_code} -eq 0 ]]; then
    echo "  [pass] ${name}"
    PASS=$(( PASS + 1 ))
  else
    echo "  [FAIL] ${name}"
    echo "${result}" | sed 's/^/         /'
    FAIL=$(( FAIL + 1 ))
  fi
}

echo "=== bash 3.2 compatibility smoke tests ==="
echo "Image: ${IMAGE}"
echo ""

# ── syntax check (bash -n) ────────────────────────────────────────────────────
echo "--- syntax check (bash -n) ---"
for script in \
  scripts/kelvin-gateway.sh \
  scripts/kelvin-gateway-daemon.sh \
  scripts/kpm.sh \
  scripts/kelvin-release-launcher.sh \
  scripts/kelvin-tui.sh \
  scripts/plugin-index-install.sh \
  scripts/plugin-install.sh \
  scripts/plugin-trust.sh \
  scripts/plugin-uninstall.sh \
  scripts/plugin-update-check.sh \
  scripts/gateway-plugin-init.sh \
; do
  run_test "syntax: ${script}" bash -n "${script}"
done

echo ""

# ── --help / usage paths ──────────────────────────────────────────────────────
echo "--- --help / usage ---"
run_test "kelvin-gateway.sh --help"       bash scripts/kelvin-gateway.sh --help
run_test "kpm.sh --help"                  bash scripts/kpm.sh --help
run_test "plugin-trust.sh --help"         bash scripts/plugin-trust.sh --help
run_test "plugin-uninstall.sh --help"     bash scripts/plugin-uninstall.sh --help
run_test "plugin-index-install.sh --help" bash scripts/plugin-index-install.sh --help
run_test "plugin-install.sh --help"       bash scripts/plugin-install.sh --help

echo ""

# ── empty array expansion under set -u ───────────────────────────────────────
echo "--- empty array expansion under set -u ---"

# Verify the broken form actually fails (documents why the fix is needed)
run_test "broken form fails on empty array" bash -c '
  result=$(bash -euo pipefail -c '"'"'a=(); echo "${a[@]}"'"'"' 2>&1) && exit 1 || true
  echo "${result}" | grep -q "unbound variable"
'

run_test "empty array: gateway_args foreground" bash -euo pipefail -c '
  gateway_args=()
  exec echo --model-provider kelvin.echo ${gateway_args[@]+"${gateway_args[@]}"}
'
run_test "empty array: gateway_args daemon" bash -euo pipefail -c '
  gateway_args=()
  echo nohup echo --model-provider kelvin.echo ${gateway_args[@]+"${gateway_args[@]}"}
'
run_test "empty array: passthrough restart" bash -euo pipefail -c '
  passthrough=()
  set -- ${passthrough[@]+"${passthrough[@]}"}
  [[ $# -eq 0 ]]
'
run_test "empty array: token_arg" bash -euo pipefail -c '
  token_arg=()
  result=$(echo ${token_arg[@]+"${token_arg[@]}"})
  [[ -z "${result}" ]]
'
run_test "non-empty array still passes through correctly" bash -euo pipefail -c '
  args=(--foo bar --baz)
  result=$(echo ${args[@]+"${args[@]}"})
  [[ "${result}" == "--foo bar --baz" ]]
'
run_test "array with one element" bash -euo pipefail -c '
  args=(--single)
  result=$(echo ${args[@]+"${args[@]}"})
  [[ "${result}" == "--single" ]]
'

echo ""

# ── bash 3.2 specific language checks ────────────────────────────────────────
echo "--- bash 3.2 language feature checks ---"

# printf -v not available in bash 3.2 — check we don't use it
run_test "no printf -v usage in release scripts" bash -c '
  ! grep -rn "printf -v" scripts/kelvin-gateway.sh scripts/kpm.sh scripts/kelvin-tui.sh 2>/dev/null
'

# &> redirect — bash 3.2 supports it but check anyway
run_test "kelvin-gateway.sh runs under strict bash 3.2" bash -euo pipefail -c '
  bash --version | head -1 | grep -q "version 3.2"
  bash -n scripts/kelvin-gateway.sh
'

# declare -A (associative arrays) not supported in bash 3.2
run_test "no associative arrays in release scripts" bash -c '
  ! grep -rn "declare -A\|typeset -A" \
    scripts/kelvin-gateway.sh scripts/kpm.sh scripts/kelvin-tui.sh 2>/dev/null
'

# process substitution <() — supported in bash 3.2
run_test "process substitution works in bash 3.2" bash -euo pipefail -c '
  result=$(cat <(echo hello))
  [[ "${result}" == "hello" ]]
'

echo ""

# ── kpm subcommands ───────────────────────────────────────────────────────────
echo "--- kpm subcommands ---"
run_test "kpm list (no plugins installed)" bash -c '
  KELVIN_HOME=/tmp/kgw-test-$$ KELVIN_PLUGIN_HOME=/tmp/kgw-test-$$/plugins \
  bash scripts/kpm.sh list
'
run_test "kpm --help shows subcommands" bash -c '
  bash scripts/kpm.sh --help | grep -q install
'
run_test "kpm install requires index url" bash -c '
  out=$(KELVIN_HOME=/tmp/kgw-test-$$ bash scripts/kpm.sh install kelvin.echo 2>&1) || true
  echo "${out}" | grep -qi "index\|url\|KELVIN_PLUGIN_INDEX_URL"
'

echo ""

# ── kelvin-gateway subcommands ────────────────────────────────────────────────
echo "--- kelvin-gateway subcommands ---"
run_test "kelvin-gateway status (not running)" bash -c '
  KELVIN_HOME=/tmp/kgw-test-$$ bash scripts/kelvin-gateway.sh status
'
run_test "kelvin-gateway stop (not running)" bash -c '
  out=$(KELVIN_HOME=/tmp/kgw-test-$$ bash scripts/kelvin-gateway.sh stop 2>&1) || true
  echo "${out}" | grep -qi "not running\|no PID"
'
run_test "kelvin-gateway start requires binary" bash -c '
  out=$(KELVIN_HOME=/tmp/kgw-test-$$ KELVIN_PLUGIN_INDEX_URL=x bash scripts/kelvin-gateway.sh start 2>&1) || true
  echo "${out}" | grep -qi "curl\|jq\|tar\|binary\|not found\|index\|fetching"
'

echo ""

# ── plugin-trust subcommands ──────────────────────────────────────────────────
echo "--- plugin-trust subcommands ---"
run_test "plugin-trust show (no file)" bash -c '
  KELVIN_TRUST_POLICY_PATH=/tmp/trust-test-$$.json \
  bash scripts/plugin-trust.sh show | jq -e .require_signature >/dev/null
'
run_test "plugin-trust rotate-key" bash -c '
  P=/tmp/trust-test-$$.json
  KELVIN_TRUST_POLICY_PATH="${P}" \
  bash scripts/plugin-trust.sh rotate-key --publisher testpub --public-key AAAA
  jq -e '"'"'.publishers[] | select(.id=="testpub")'"'"' "${P}" >/dev/null
  rm -f "${P}"
'
run_test "plugin-trust revoke + unrevoke" bash -c '
  P=/tmp/trust-test-$$.json
  KELVIN_TRUST_POLICY_PATH="${P}" bash scripts/plugin-trust.sh rotate-key --publisher x --public-key A
  KELVIN_TRUST_POLICY_PATH="${P}" bash scripts/plugin-trust.sh revoke --publisher x
  jq -e '"'"'.revoked_publishers | index("x") != null'"'"' "${P}" >/dev/null
  KELVIN_TRUST_POLICY_PATH="${P}" bash scripts/plugin-trust.sh unrevoke --publisher x
  jq -e '"'"'(.revoked_publishers | index("x")) == null'"'"' "${P}" >/dev/null
  rm -f "${P}"
'

echo ""
echo "=== results: ${PASS} passed, ${FAIL} failed ==="
[[ ${FAIL} -eq 0 ]]

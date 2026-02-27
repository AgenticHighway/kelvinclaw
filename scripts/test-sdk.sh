#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/lib/rust-toolchain-path.sh"

if ! ensure_rust_toolchain_path; then
  echo "[test-sdk] missing required commands: cargo/rustup" >&2
  exit 1
fi

cd "${ROOT_DIR}"
echo "[test-sdk] running Kelvin Core SDK tests"
cargo test -p kelvin-core --test sdk_security_stability
cargo test -p kelvin-core --test sdk_owasp_top10_ai_2025
cargo test -p kelvin-core --test sdk_nist_ai_rmf_1_0
cargo test -p kelvin-core sdk::tests
echo "[test-sdk] running SDK model-provider lane tests"
cargo test -p kelvin-wasm model_host::tests
cargo test -p kelvin-brain installed_plugins::tests
cargo test -p kelvin-sdk --lib
echo "[test-sdk] running SDK tool-sandbox OWASP/NIST suites"
cargo test -p kelvin-sdk --test tool_sandbox_owasp_top10_ai_2025
cargo test -p kelvin-sdk --test tool_sandbox_nist_ai_rmf_1_0
echo "[test-sdk] running plugin author kit lifecycle test"
"${ROOT_DIR}/scripts/test-plugin-author-kit.sh"
echo "[test-sdk] running trust-policy operations test"
"${ROOT_DIR}/scripts/test-plugin-trust.sh"
echo "[test-sdk] success"

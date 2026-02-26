#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/lib/rust-toolchain-path.sh"

if ! ensure_rust_toolchain_path; then
  echo "error: cargo/rustup are required" >&2
  exit 1
fi

echo "[1/4] workspace tests"
cargo test --workspace -j1

echo "[2/4] kelvin-host legacy path"
cargo check -p kelvin-host

echo "[3/4] kelvin-host rpc path"
cargo check -p kelvin-host --features memory_rpc

echo "[4/4] kelvin-host rpc + legacy fallback"
cargo check -p kelvin-host --features memory_rpc,memory_legacy_fallback

echo "memory rollout checks passed"

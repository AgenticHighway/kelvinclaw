#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required" >&2
  exit 1
fi

echo "[1/4] workspace tests"
cargo test --workspace -j1

echo "[2/4] archive cli legacy path"
cargo check --manifest-path archive/kelvin-cli/Cargo.toml

echo "[3/4] archive cli rpc path"
cargo check --manifest-path archive/kelvin-cli/Cargo.toml --features memory_rpc

echo "[4/4] archive cli rpc + legacy fallback"
cargo check --manifest-path archive/kelvin-cli/Cargo.toml --features memory_rpc,memory_legacy_fallback

echo "memory rollout checks passed"

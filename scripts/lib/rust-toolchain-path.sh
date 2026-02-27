#!/usr/bin/env bash
set -euo pipefail

ensure_rust_toolchain_path() {
  if command -v cargo >/dev/null 2>&1 && command -v rustup >/dev/null 2>&1; then
    return 0
  fi

  if [[ -n "${HOME:-}" && -d "${HOME}/.cargo/bin" ]]; then
    export PATH="${HOME}/.cargo/bin:${PATH}"
  fi
  if command -v cargo >/dev/null 2>&1 && command -v rustup >/dev/null 2>&1; then
    return 0
  fi

  if [[ -d "/usr/local/cargo/bin" ]]; then
    export PATH="/usr/local/cargo/bin:${PATH}"
  fi
  if command -v cargo >/dev/null 2>&1 && command -v rustup >/dev/null 2>&1; then
    return 0
  fi

  # Homebrew rustup installs (macOS arm64/intel)
  if [[ -d "/opt/homebrew/opt/rustup/bin" ]]; then
    export PATH="/opt/homebrew/opt/rustup/bin:${PATH}"
  fi
  if command -v cargo >/dev/null 2>&1 && command -v rustup >/dev/null 2>&1; then
    return 0
  fi
  if [[ -d "/usr/local/opt/rustup/bin" ]]; then
    export PATH="/usr/local/opt/rustup/bin:${PATH}"
  fi
  if command -v cargo >/dev/null 2>&1 && command -v rustup >/dev/null 2>&1; then
    return 0
  fi

  return 1
}

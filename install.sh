#!/usr/bin/env bash
# KelvinClaw Installer
# Usage: curl -sSL https://raw.githubusercontent.com/AgenticHighway/kelvinclaw/main/install.sh | bash
#   or:  curl -sSL https://raw.githubusercontent.com/AgenticHighway/kelvinclaw/main/install.sh | bash -s -- --version 0.1.8
set -euo pipefail

REPO="AgenticHighway/kelvinclaw"
INSTALL_DIR="${KELVIN_INSTALL_DIR:-${HOME}/.kelvinclaw}"
BIN_DIR="${INSTALL_DIR}/bin"
VERSION=""

# ── helpers ────────────────────────────────────────────────────────
info()  { printf '\033[1;34m[kelvinclaw]\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m[kelvinclaw]\033[0m %s\n' "$*" >&2; }
fail()  { printf '\033[1;31m[kelvinclaw]\033[0m %s\n' "$*" >&2; exit 1; }

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "Missing required command: $1"
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    fail "Missing sha256sum or shasum"
  fi
}

# ── argument parsing ──────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)  VERSION="${2:?missing value for --version}"; shift 2 ;;
    --install-dir) INSTALL_DIR="${2:?missing value for --install-dir}"; BIN_DIR="${INSTALL_DIR}/bin"; shift 2 ;;
    -h|--help)
      cat <<'EOF'
KelvinClaw Installer

Usage:
  curl -sSL https://raw.githubusercontent.com/AgenticHighway/kelvinclaw/main/install.sh | bash
  curl -sSL .../install.sh | bash -s -- --version 0.1.8

Options:
  --version <semver>      Install a specific version (default: latest)
  --install-dir <path>    Override install directory (default: ~/.kelvinclaw)
  -h, --help              Show this help
EOF
      exit 0
      ;;
    *) fail "Unknown argument: $1" ;;
  esac
done

require_cmd curl
require_cmd tar
require_cmd awk

# ── detect platform ───────────────────────────────────────────────
detect_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}" in
    Linux)  os="linux" ;;
    Darwin) os="macos" ;;
    *)      fail "Unsupported OS: ${os}. KelvinClaw supports Linux and macOS." ;;
  esac

  case "${arch}" in
    x86_64|amd64)       arch="x86_64" ;;
    aarch64|arm64)      arch="arm64" ;;
    *)                  fail "Unsupported architecture: ${arch}" ;;
  esac

  printf '%s-%s' "${os}" "${arch}"
}

PLATFORM="$(detect_platform)"
info "Detected platform: ${PLATFORM}"

# ── resolve version ───────────────────────────────────────────────
if [[ -z "${VERSION}" ]]; then
  info "Resolving latest version..."
  VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | awk -F'"' '/"tag_name"/{print $4; exit}')"
  VERSION="${VERSION#v}"
  if [[ -z "${VERSION}" ]]; then
    fail "Could not determine latest release version"
  fi
fi
info "Installing KelvinClaw v${VERSION}"

# ── download + verify ─────────────────────────────────────────────
ARCHIVE_NAME="kelvinclaw-${VERSION}-${PLATFORM}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/v${VERSION}/${ARCHIVE_NAME}"
CHECKSUM_URL="${DOWNLOAD_URL}.sha256"

WORK_DIR="$(mktemp -d)"
cleanup() { rm -rf "${WORK_DIR}"; }
trap cleanup EXIT

info "Downloading ${ARCHIVE_NAME}..."
curl -fSL --progress-bar "${DOWNLOAD_URL}" -o "${WORK_DIR}/${ARCHIVE_NAME}" || \
  fail "Download failed. Check that v${VERSION} has a release for ${PLATFORM}."

info "Verifying checksum..."
curl -fsSL "${CHECKSUM_URL}" -o "${WORK_DIR}/expected.sha256" || \
  warn "Could not download checksum file — skipping verification"

if [[ -f "${WORK_DIR}/expected.sha256" ]]; then
  EXPECTED="$(awk '{print $1}' "${WORK_DIR}/expected.sha256")"
  ACTUAL="$(sha256_file "${WORK_DIR}/${ARCHIVE_NAME}")"
  if [[ "${ACTUAL}" != "${EXPECTED}" ]]; then
    fail "Checksum mismatch!\n  expected: ${EXPECTED}\n  actual:   ${ACTUAL}"
  fi
  info "Checksum verified"
fi

# ── extract ───────────────────────────────────────────────────────
info "Extracting to ${INSTALL_DIR}..."
mkdir -p "${INSTALL_DIR}"

tar -xzf "${WORK_DIR}/${ARCHIVE_NAME}" -C "${WORK_DIR}" 2>/dev/null || \
  tar -xzf "${WORK_DIR}/${ARCHIVE_NAME}" -C "${WORK_DIR}"

# The tarball extracts to kelvinclaw-<version>-<platform>/
EXTRACT_DIR="${WORK_DIR}/kelvinclaw-${VERSION}-${PLATFORM}"
if [[ ! -d "${EXTRACT_DIR}" ]]; then
  # Try alternate naming: some builds use the target triple
  EXTRACT_DIR="$(find "${WORK_DIR}" -maxdepth 1 -type d -name 'kelvinclaw-*' | head -1)"
  if [[ -z "${EXTRACT_DIR}" || ! -d "${EXTRACT_DIR}" ]]; then
    fail "Could not find extracted archive directory"
  fi
fi

# Copy binaries
mkdir -p "${BIN_DIR}"
if [[ -d "${EXTRACT_DIR}/bin" ]]; then
  cp -f "${EXTRACT_DIR}/bin/"* "${BIN_DIR}/"
  chmod +x "${BIN_DIR}/"*
fi

# Copy launcher script
if [[ -f "${EXTRACT_DIR}/kelvin" ]]; then
  cp -f "${EXTRACT_DIR}/kelvin" "${INSTALL_DIR}/kelvin"
  chmod +x "${INSTALL_DIR}/kelvin"
fi

# Copy share/ (plugin manifest, etc.)
if [[ -d "${EXTRACT_DIR}/share" ]]; then
  mkdir -p "${INSTALL_DIR}/share"
  cp -Rf "${EXTRACT_DIR}/share/"* "${INSTALL_DIR}/share/"
fi

# Copy LICENSE + README
for f in LICENSE README.md BUILD_INFO.txt; do
  if [[ -f "${EXTRACT_DIR}/${f}" ]]; then
    cp -f "${EXTRACT_DIR}/${f}" "${INSTALL_DIR}/${f}"
  fi
done

# ── PATH setup ────────────────────────────────────────────────────
SHELL_NAME="$(basename "${SHELL:-bash}")"
PROFILE_FILE=""
case "${SHELL_NAME}" in
  zsh)  PROFILE_FILE="${HOME}/.zshrc" ;;
  bash)
    if [[ -f "${HOME}/.bash_profile" ]]; then
      PROFILE_FILE="${HOME}/.bash_profile"
    else
      PROFILE_FILE="${HOME}/.bashrc"
    fi
    ;;
  fish) PROFILE_FILE="${HOME}/.config/fish/config.fish" ;;
  *)    PROFILE_FILE="${HOME}/.profile" ;;
esac

PATH_LINE="export PATH=\"${INSTALL_DIR}:\${PATH}\""
if [[ "${SHELL_NAME}" == "fish" ]]; then
  PATH_LINE="set -gx PATH ${INSTALL_DIR} \$PATH"
fi

PATH_ADDED=0
if [[ -n "${PROFILE_FILE}" ]] && ! grep -qF "${INSTALL_DIR}" "${PROFILE_FILE}" 2>/dev/null; then
  printf '\n# KelvinClaw\n%s\n' "${PATH_LINE}" >> "${PROFILE_FILE}"
  PATH_ADDED=1
fi

# ── done ──────────────────────────────────────────────────────────
info ""
info "KelvinClaw v${VERSION} installed to ${INSTALL_DIR}"
info ""
info "Binaries:"
ls -1 "${BIN_DIR}/" 2>/dev/null | while read -r f; do
  info "  ${BIN_DIR}/${f}"
done
if [[ -f "${INSTALL_DIR}/kelvin" ]]; then
  info "  ${INSTALL_DIR}/kelvin  (launcher)"
fi
info ""

if [[ "${PATH_ADDED}" == "1" ]]; then
  info "Added ${INSTALL_DIR} to PATH in ${PROFILE_FILE}"
  info "Run: source ${PROFILE_FILE}  (or open a new terminal)"
elif echo "${PATH}" | tr ':' '\n' | grep -qF "${INSTALL_DIR}"; then
  info "${INSTALL_DIR} is already in your PATH"
else
  info "Add to your PATH:"
  info "  ${PATH_LINE}"
fi

info ""
info "Get started:"
info "  kelvin --help"
info "  kelvin medkit"
info ""

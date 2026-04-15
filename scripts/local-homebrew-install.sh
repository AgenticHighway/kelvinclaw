#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BREW_FORMULA="agentichighway/tap/kelvinclaw"
TARGET=""
OUTPUT_DIR="${ROOT_DIR}/dist/releases"
TARGET_DIR="${ROOT_DIR}/target/releases"
VERSION=""
FORMULA_SOURCE="${ROOT_DIR}/../homebrew-tap/Formula/kelvinclaw.rb"
LOCAL_TAP_FORMULA=""
SKIP_PACKAGE="false"
RESTORE_SOURCE=""
BACKUP_FORMULA=""
TEMP_FORMULA=""

usage() {
  cat <<'USAGE'
Usage: scripts/local-homebrew-install.sh [options]

Build the current KelvinClaw release bundle for the host platform, temporarily
point the local Homebrew tap formula at that artifact, install/reinstall via
Homebrew, then restore the tap formula to its normal source.

Options:
  --target <triple>         Rust target triple to package (default: host target)
  --output-dir <path>       Release artifact directory (default: ./dist/releases)
  --target-dir <path>       Cargo target directory (default: ./target/releases)
  --version <semver>        Override workspace version
  --formula-source <path>   Formula file to restore after install
  --skip-package            Reuse an existing archive instead of rebuilding it
  -h, --help                Show help
USAGE
}

require_cmd() {
  local name="$1"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "Missing required command: ${name}" >&2
    exit 1
  fi
}

host_target() {
  case "$(uname -s):$(uname -m)" in
    Darwin:arm64|Darwin:aarch64) printf '%s\n' 'aarch64-apple-darwin' ;;
    Darwin:x86_64) printf '%s\n' 'x86_64-apple-darwin' ;;
    Linux:arm64|Linux:aarch64) printf '%s\n' 'aarch64-unknown-linux-gnu' ;;
    Linux:x86_64) printf '%s\n' 'x86_64-unknown-linux-gnu' ;;
    *)
      echo "Unsupported host platform: $(uname -s) $(uname -m)" >&2
      exit 1
      ;;
  esac
}

platform_label() {
  case "$1" in
    x86_64-unknown-linux-gnu) printf '%s\n' 'linux-x86_64' ;;
    aarch64-unknown-linux-gnu) printf '%s\n' 'linux-arm64' ;;
    x86_64-apple-darwin) printf '%s\n' 'macos-x86_64' ;;
    aarch64-apple-darwin) printf '%s\n' 'macos-arm64' ;;
    *)
      echo "Unsupported target triple: $1" >&2
      exit 1
      ;;
  esac
}

workspace_version() {
  cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.name == "kelvin-host") | .version'
}

ensure_homebrew_tap() {
  if ! brew --repo agentichighway/tap >/dev/null 2>&1; then
    brew tap AgenticHighway/tap >/dev/null
  fi
  LOCAL_TAP_FORMULA="$(brew --repo agentichighway/tap)/Formula/kelvinclaw.rb"
}

restore_formula() {
  local restore_path=""
  if [[ -n "${RESTORE_SOURCE}" && -f "${RESTORE_SOURCE}" ]]; then
    restore_path="${RESTORE_SOURCE}"
  elif [[ -n "${BACKUP_FORMULA}" && -f "${BACKUP_FORMULA}" ]]; then
    restore_path="${BACKUP_FORMULA}"
  fi

  if [[ -n "${restore_path}" && -n "${LOCAL_TAP_FORMULA}" ]]; then
    cp "${restore_path}" "${LOCAL_TAP_FORMULA}"
  fi
}

cleanup() {
  restore_formula
  [[ -n "${BACKUP_FORMULA}" ]] && rm -f "${BACKUP_FORMULA}"
  [[ -n "${TEMP_FORMULA}" ]] && rm -f "${TEMP_FORMULA}"
}

rewrite_formula_for_local_archive() {
  local archive_path="$1"
  local archive_sha="$2"
  local release_version="$3"
  local host_platform="$4"

  TEMP_FORMULA="$(mktemp)"
  FORMULA_SOURCE="${RESTORE_SOURCE}" \
  RELEASE_VERSION="${release_version}" \
  LOCAL_ARCHIVE_URL="file://${archive_path}" \
  LOCAL_ARCHIVE_SHA="${archive_sha}" \
  HOST_PLATFORM="${host_platform}" \
  ruby <<'RUBY' > "${TEMP_FORMULA}"
source_path = ENV.fetch("FORMULA_SOURCE")
text = File.read(source_path)

release_version = ENV.fetch("RELEASE_VERSION")
local_archive_url = ENV.fetch("LOCAL_ARCHIVE_URL")
local_archive_sha = ENV.fetch("LOCAL_ARCHIVE_SHA")
host_platform = ENV.fetch("HOST_PLATFORM")

version_pattern = /version "\d+\.\d+\.\d+"/
updated = text.sub(version_pattern, %(version "#{release_version}"))
raise "failed to update formula version" if updated == text
text = updated

asset_name =
  case host_platform
  when "macos-arm64" then "macos-arm64"
  when "macos-x86_64" then "macos-x86_64"
  when "linux-arm64" then "linux-arm64"
  when "linux-x86_64" then "linux-x86_64"
  else
    raise "unsupported host platform #{host_platform.inspect}"
  end

asset_block_pattern = %r{url ".*kelvinclaw-\#\{version\}-#{Regexp.escape(asset_name)}\.tar\.gz"\n\s+sha256 "[0-9a-f]+"}
updated = text.sub(
  asset_block_pattern,
  %(url "#{local_archive_url}"\n      sha256 "#{local_archive_sha}")
)
raise "failed to update formula archive block for #{asset_name}" if updated == text
text = updated

install_block = <<~'RUBYBLOCK'.rstrip
  def install
    bundle_root =
      if (buildpath/"bin/kelvin").exist?
        buildpath
      else
        buildpath.glob("kelvinclaw-*").find { |dir| (dir/"bin/kelvin").exist? }
      end

    raise "Expected KelvinClaw release bundle" unless bundle_root

    bundle_path = libexec/"kelvinclaw"
    ignored_entries = %w[. ..].freeze
    bundle_contents = Dir["#{bundle_root}/*", "#{bundle_root}/.*"].reject do |path|
      ignored_entries.include?(File.basename(path))
    end
    bundle_path.install bundle_contents

    {
      "kelvin" => "#{bundle_path}/bin/kelvin",
      "kelvin-gateway" => "#{bundle_path}/bin/kelvin-gateway",
      "kelvin-tui" => "#{bundle_path}/bin/kelvin-tui",
    }.each do |command, target|
      (bin/command).write <<~SH
        #!/bin/bash
        exec "#{target}" "$@"
      SH
      chmod 0555, bin/command
    end

    (bin/"kpm").write <<~SH
      #!/bin/bash
      exec "#{bundle_path}/bin/kelvin" kpm "$@"
    SH
    chmod 0555, bin/"kpm"

    {
      "kelvin-host" => "#{bundle_path}/bin/kelvin-host",
      "kelvin-memory-controller" => "#{bundle_path}/bin/kelvin-memory-controller",
      "kelvin-registry" => "#{bundle_path}/bin/kelvin-registry",
    }.each do |command, target|
      (bin/command).write <<~SH
        #!/bin/bash
        exec "#{target}" "$@"
      SH
      chmod 0555, bin/command
    end
  end
RUBYBLOCK

install_pattern = /  def install\n.*?\n  end\n\n  def caveats/m
updated = text.sub(install_pattern, "#{install_block}\n\n  def caveats")
raise "failed to update formula install block" if updated == text
text = updated

puts text
RUBY

  cp "${TEMP_FORMULA}" "${LOCAL_TAP_FORMULA}"
  ruby -c "${LOCAL_TAP_FORMULA}" >/dev/null
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target)
      TARGET="${2:?missing value for --target}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:?missing value for --output-dir}"
      shift 2
      ;;
    --target-dir)
      TARGET_DIR="${2:?missing value for --target-dir}"
      shift 2
      ;;
    --version)
      VERSION="${2:?missing value for --version}"
      shift 2
      ;;
    --formula-source)
      FORMULA_SOURCE="${2:?missing value for --formula-source}"
      shift 2
      ;;
    --skip-package)
      SKIP_PACKAGE="true"
      shift
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

require_cmd brew
require_cmd ruby
require_cmd cargo
require_cmd jq

if [[ -z "${TARGET}" ]]; then
  TARGET="$(host_target)"
fi

if [[ -z "${VERSION}" ]]; then
  VERSION="$(workspace_version)"
fi

HOST_PLATFORM="$(platform_label "${TARGET}")"
ARCHIVE_PATH="${OUTPUT_DIR}/kelvinclaw-${VERSION}-${HOST_PLATFORM}.tar.gz"
CHECKSUM_PATH="${ARCHIVE_PATH}.sha256"

ensure_homebrew_tap

if [[ -f "${FORMULA_SOURCE}" ]]; then
  RESTORE_SOURCE="${FORMULA_SOURCE}"
else
  echo "warning: formula source not found at ${FORMULA_SOURCE}; restoring local tap backup instead" >&2
fi

BACKUP_FORMULA="$(mktemp)"
if [[ -f "${LOCAL_TAP_FORMULA}" ]]; then
  cp "${LOCAL_TAP_FORMULA}" "${BACKUP_FORMULA}"
fi

trap cleanup EXIT

if [[ "${SKIP_PACKAGE}" != "true" ]]; then
  "${ROOT_DIR}/scripts/package-unix-release.sh" \
    --target "${TARGET}" \
    --output-dir "${OUTPUT_DIR}" \
    --target-dir "${TARGET_DIR}"
fi

if [[ ! -f "${ARCHIVE_PATH}" ]]; then
  echo "Expected archive not found: ${ARCHIVE_PATH}" >&2
  exit 1
fi

if [[ ! -f "${CHECKSUM_PATH}" ]]; then
  echo "Expected checksum not found: ${CHECKSUM_PATH}" >&2
  exit 1
fi

ARCHIVE_SHA="$(awk '{print $1}' "${CHECKSUM_PATH}")"

rewrite_formula_for_local_archive "${ARCHIVE_PATH}" "${ARCHIVE_SHA}" "${VERSION}" "${HOST_PLATFORM}"

if brew list --versions kelvinclaw >/dev/null 2>&1; then
  brew reinstall "${BREW_FORMULA}"
else
  brew install "${BREW_FORMULA}"
fi

echo
echo "Installed local artifact via Homebrew:"
echo "  ${ARCHIVE_PATH}"
echo
echo "The tap formula will be restored to:"
echo "  ${RESTORE_SOURCE:-${LOCAL_TAP_FORMULA}}"

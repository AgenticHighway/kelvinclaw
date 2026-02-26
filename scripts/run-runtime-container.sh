#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${KELVIN_RUNTIME_IMAGE:-kelvin-runtime:dev}"
INDEX_URL="${KELVIN_PLUGIN_INDEX_URL:-}"

usage() {
  cat <<'USAGE'
Usage: scripts/run-runtime-container.sh [options]

Build and run the minimal Kelvin runtime container with interactive setup.

Options:
  --image <name>       Image tag to use/build (default: kelvin-runtime:dev)
  --index-url <url>    Plugin index URL exposed as KELVIN_PLUGIN_INDEX_URL in container
  --no-build           Skip docker build step
  -h, --help           Show help
USAGE
}

DO_BUILD="1"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --image)
      IMAGE="${2:?missing value for --image}"
      shift 2
      ;;
    --index-url)
      INDEX_URL="${2:?missing value for --index-url}"
      shift 2
      ;;
    --no-build)
      DO_BUILD="0"
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

if [[ "${DO_BUILD}" == "1" ]]; then
  docker build \
    -f "${ROOT_DIR}/docker/Dockerfile.runtime" \
    -t "${IMAGE}" \
    "${ROOT_DIR}"
fi

docker_args=(
  run --rm -it
  -v "${ROOT_DIR}/.kelvin:/kelvin"
  -v "${ROOT_DIR}:/workspace"
  -w /workspace
)

if [[ -n "${INDEX_URL}" ]]; then
  docker_args+=(-e "KELVIN_PLUGIN_INDEX_URL=${INDEX_URL}")
fi

docker_args+=("${IMAGE}")

exec docker "${docker_args[@]}"

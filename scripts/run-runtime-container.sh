#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/lib/docker-cache.sh"
IMAGE="${KELVIN_RUNTIME_IMAGE:-kelvin-runtime:dev}"
DEFAULT_INDEX_URL="https://raw.githubusercontent.com/AgenticHighway/kelvinclaw-plugins/main/index.json"
INDEX_URL="${KELVIN_PLUGIN_INDEX_URL:-${DEFAULT_INDEX_URL}}"
BUILDER_NAME="${KELVIN_DOCKER_BUILDER:-kelvinclaw-builder}"
CACHE_DIR="${KELVIN_RUNTIME_DOCKER_CACHE_DIR:-$(kelvin_docker_buildx_cache_dir "${ROOT_DIR}" "runtime")}"

usage() {
  cat <<'USAGE'
Usage: scripts/run-runtime-container.sh [options]

Build and run the minimal Kelvin runtime container with interactive setup.

Options:
  --image <name>       Image tag to use/build (default: kelvin-runtime:dev)
  --index-url <url>    Plugin index URL exposed as KELVIN_PLUGIN_INDEX_URL in container
                       (required for community plugin installation; first-party plugins are baked in)
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
  if ! docker buildx inspect "${BUILDER_NAME}" >/dev/null 2>&1; then
    docker buildx create --name "${BUILDER_NAME}" --use >/dev/null
  else
    docker buildx use "${BUILDER_NAME}" >/dev/null
  fi

  mkdir -p "${CACHE_DIR}"
  CACHE_TMP="${CACHE_DIR}-new"
  rm -rf "${CACHE_TMP}"

  build_cmd=(
    docker buildx build
    --builder "${BUILDER_NAME}"
    --file "${ROOT_DIR}/docker/Dockerfile.runtime"
    --progress plain
    --load
    --tag "${IMAGE}"
    --cache-to "type=local,dest=${CACHE_TMP},mode=max"
  )

  if [[ -f "${CACHE_DIR}/index.json" ]]; then
    build_cmd+=(--cache-from "type=local,src=${CACHE_DIR}")
  fi

  build_cmd+=("${ROOT_DIR}")
  "${build_cmd[@]}"

  rm -rf "${CACHE_DIR}"
  mv "${CACHE_TMP}" "${CACHE_DIR}"
fi

docker_args=(
  run --rm -it
  -v "${ROOT_DIR}/.kelvin:/kelvin"
  -v "${ROOT_DIR}:/workspace"
  -w /workspace
)

[[ -n "${INDEX_URL}" ]] && docker_args+=(-e "KELVIN_PLUGIN_INDEX_URL=${INDEX_URL}")

docker_args+=("${IMAGE}")

exec docker "${docker_args[@]}"

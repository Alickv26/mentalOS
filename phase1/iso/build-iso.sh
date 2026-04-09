#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PROTO_DIR="${REPO_ROOT}/mentalOS-prototype"
PROFILE_DIR="${SCRIPT_DIR}"
OUT_DIR="${PROFILE_DIR}/out"
WORK_DIR="${WORK_DIR:-/tmp/archiso-tmp}"
BINARY_PATH="${PROTO_DIR}/target/release/mentalOS"

log() { printf '[build-iso] %s\n' "$*"; }

require_tools() {
  local tools=(mkarchiso rsync)
  local missing=()
  for t in "${tools[@]}"; do
    command -v "$t" >/dev/null 2>&1 || missing+=("$t")
  done
  if ((${#missing[@]})); then
    echo "Missing tools: ${missing[*]}" >&2
    echo "Install archiso package first: sudo pacman -S --needed archiso" >&2
    exit 1
  fi
}

build_binary() {
  log "Building mentalOS release binary"
  cargo build --manifest-path "${PROTO_DIR}/Cargo.toml" --release
  if [[ ! -x "${BINARY_PATH}" ]]; then
    echo "Expected binary not found: ${BINARY_PATH}" >&2
    exit 1
  fi
}

stage_binary() {
  log "Staging mentalOS binary into airootfs"
  install -Dm0755 "${BINARY_PATH}" "${PROFILE_DIR}/airootfs/usr/local/bin/mentalOS"
}

build_iso() {
  mkdir -p "${OUT_DIR}"
  log "Running mkarchiso"
  sudo mkarchiso -v -w "${WORK_DIR}" -o "${OUT_DIR}" "${PROFILE_DIR}"
}

main() {
  require_tools
  build_binary
  stage_binary
  build_iso
  log "ISO build complete. Output files:"
  ls -lh "${OUT_DIR}"/*.iso
}

main "$@"

#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PROTO_DIR="${REPO_ROOT}/mentalOS-prototype"
PROFILE_DIR="${SCRIPT_DIR}"
OUT_DIR="${PROFILE_DIR}/out"
WORK_DIR="${WORK_DIR:-${PROFILE_DIR}/work}"
DEFAULT_BINARY_PATH="${PROTO_DIR}/target/release/mentalOS"
BINARY_PATH="${MENTALOS_BINARY:-${DEFAULT_BINARY_PATH}}"
USE_EXISTING_BINARY="${USE_EXISTING_BINARY:-1}"
FORCE_REBUILD="${FORCE_REBUILD:-0}"
AUTO_INSTALL_TOOLS="${AUTO_INSTALL_TOOLS:-1}"
CLEAN_WORK="${CLEAN_WORK:-1}"
INSTALLER_SOURCE_DIR="${REPO_ROOT}/phase1/arch-base"
INSTALLER_STAGE_DIR="${PROFILE_DIR}/airootfs/opt/mentalos/arch-base"
KEEP_STAGE="${KEEP_STAGE:-0}"

log() { printf '[build-iso] %s\n' "$*"; }

install_tool_package() {
  local package="$1"
  if [[ "${AUTO_INSTALL_TOOLS}" != "1" ]]; then
    return 1
  fi
  if ! command -v pacman >/dev/null 2>&1; then
    return 1
  fi
  if ! command -v sudo >/dev/null 2>&1; then
    return 1
  fi

  log "Auto-installing missing package: ${package}"
  sudo pacman -S --needed --noconfirm "${package}"
}

ensure_tool() {
  local cmd="$1"
  local package="$2"
  if command -v "${cmd}" >/dev/null 2>&1; then
    return 0
  fi
  if install_tool_package "${package}" && command -v "${cmd}" >/dev/null 2>&1; then
    log "Installed ${package} for ${cmd}"
    return 0
  fi
  echo "Missing required tool '${cmd}' (package: ${package})." >&2
  echo "Install manually: sudo pacman -S --needed ${package}" >&2
  exit 1
}

require_tools() {
  ensure_tool mkarchiso archiso
  ensure_tool rsync rsync
  ensure_tool sudo sudo
}

build_binary_if_needed() {
  if [[ "${FORCE_REBUILD}" == "1" ]]; then
    log "FORCE_REBUILD=1, rebuilding mentalOS release binary"
    cargo build --manifest-path "${PROTO_DIR}/Cargo.toml" --release
    return
  fi

  if [[ "${USE_EXISTING_BINARY}" == "1" && -x "${BINARY_PATH}" ]]; then
    log "Using existing binary: ${BINARY_PATH}"
    return
  fi

  log "Building mentalOS release binary"
  cargo build --manifest-path "${PROTO_DIR}/Cargo.toml" --release
}

stage_binary() {
  if [[ ! -x "${BINARY_PATH}" ]]; then
    echo "Expected binary not found or not executable: ${BINARY_PATH}" >&2
    exit 1
  fi
  log "Staging mentalOS binary into airootfs"
  install -Dm0755 "${BINARY_PATH}" "${PROFILE_DIR}/airootfs/usr/local/bin/mentalOS"
}

stage_installer_assets() {
  if [[ ! -d "${INSTALLER_SOURCE_DIR}" ]]; then
    echo "Installer source directory missing: ${INSTALLER_SOURCE_DIR}" >&2
    exit 1
  fi
  log "Staging installer assets from ${INSTALLER_SOURCE_DIR}"
  mkdir -p "${INSTALLER_STAGE_DIR}"
  rsync -a --delete --exclude 'config/home/*' "${INSTALLER_SOURCE_DIR}/" "${INSTALLER_STAGE_DIR}/"
}

cleanup_stage() {
  if [[ "${KEEP_STAGE}" == "1" ]]; then
    log "KEEP_STAGE=1 set; leaving staged assets in profile tree"
    return
  fi
  rm -f "${PROFILE_DIR}/airootfs/usr/local/bin/mentalOS"
  rm -rf "${INSTALLER_STAGE_DIR}"
}

build_iso() {
  mkdir -p "${OUT_DIR}"
  if [[ "${CLEAN_WORK}" == "1" ]]; then
    log "Cleaning work directory: ${WORK_DIR}"
    sudo rm -rf "${WORK_DIR}"
  fi
  log "Using work directory: ${WORK_DIR}"
  log "Running mkarchiso"
  sudo mkarchiso -v -w "${WORK_DIR}" -o "${OUT_DIR}" "${PROFILE_DIR}"
}

main() {
  trap cleanup_stage EXIT
  require_tools
  build_binary_if_needed
  stage_binary
  stage_installer_assets
  build_iso
  log "ISO build complete. Output files:"
  ls -lh "${OUT_DIR}"/*.iso
}

main "$@"

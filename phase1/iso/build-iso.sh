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
DEFAULT_WORKSPACE_MONITOR_PATH="${PROTO_DIR}/target/release/mentalos-workspace-monitor"
WORKSPACE_MONITOR_BINARY_PATH="${MENTALOS_WORKSPACE_MONITOR_BINARY:-${DEFAULT_WORKSPACE_MONITOR_PATH}}"
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
    log "FORCE_REBUILD=1, rebuilding mentalOS release binaries"
    cargo build --manifest-path "${PROTO_DIR}/Cargo.toml" --release --bin mentalOS --bin mentalos-workspace-monitor
    return
  fi

  if [[ "${USE_EXISTING_BINARY}" == "1" && -x "${BINARY_PATH}" && -x "${WORKSPACE_MONITOR_BINARY_PATH}" ]]; then
    log "Using existing binaries: ${BINARY_PATH}, ${WORKSPACE_MONITOR_BINARY_PATH}"
    return
  fi

  log "Building mentalOS release binaries (mentalOS + mentalos-workspace-monitor)"
  cargo build --manifest-path "${PROTO_DIR}/Cargo.toml" --release --bin mentalOS --bin mentalos-workspace-monitor
}

stage_binary() {
  if [[ ! -x "${BINARY_PATH}" ]]; then
    echo "Expected binary not found or not executable: ${BINARY_PATH}" >&2
    exit 1
  fi
  log "Staging mentalOS binary into airootfs"
  install -Dm0755 "${BINARY_PATH}" "${PROFILE_DIR}/airootfs/usr/local/bin/mentalOS"

  if [[ ! -x "${WORKSPACE_MONITOR_BINARY_PATH}" ]]; then
    echo "Expected binary not found or not executable: ${WORKSPACE_MONITOR_BINARY_PATH}" >&2
    exit 1
  fi
  log "Staging mentalos-workspace-monitor binary into airootfs"
  install -Dm0755 "${WORKSPACE_MONITOR_BINARY_PATH}" "${PROFILE_DIR}/airootfs/usr/local/bin/mentalos-workspace-monitor"
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
  rm -f "${PROFILE_DIR}/airootfs/usr/local/bin/mentalos-workspace-monitor"
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

# Post-build verification: check that the squashfs contains the expected
# files. This catches cases where the airootfs overlay or customize script
# failed silently.
verify_build() {
  local iso_file
  # shellcheck disable=SC2012  # ls -1t is intentional — we want the most recently-built ISO
  iso_file="$(ls -1t "${OUT_DIR}"/*.iso 2>/dev/null | head -n 1 || true)"
  if [[ -z "${iso_file}" ]]; then
    echo "[build-iso] ERROR: No ISO file found in ${OUT_DIR}" >&2
    exit 1
  fi

  log "Verifying ISO: ${iso_file}"

  # Check that the squashfs in the ISO contains our key files.
  # We use unsquashfs to list the contents and grep for expected paths.
  local squashfs_path="/arch/x86_64/airootfs.sfs"

  # Mount the ISO to extract the squashfs
  local mount_dir
  mount_dir="$(mktemp -d)"
  if ! sudo mount -o loop "${iso_file}" "${mount_dir}" 2>/dev/null; then
    echo "[build-iso] WARN: Could not mount ISO for verification (loop device issue?)"
    echo "[build-iso] Skipping detailed verification."
    return 0
  fi

  local squashfs_file="${mount_dir}${squashfs_path}"
  if [[ ! -f "${squashfs_file}" ]]; then
    echo "[build-iso] WARN: squashfs not found at ${squashfs_path} in ISO"
    sudo umount "${mount_dir}" 2>/dev/null || true
    rmdir "${mount_dir}" 2>/dev/null || true
    return 0
  fi

  local errors=0

  # List squashfs contents and check for required files
  local listing
  listing="$(sudo unsquashfs -l "${squashfs_file}" 2>/dev/null || echo "")"

  check_in_squashfs() {
    local path="$1"
    if echo "${listing}" | grep -q "${path}"; then
      log "  OK: ${path} found in squashfs"
    else
      echo "[build-iso] ERROR: ${path} NOT found in squashfs!" >&2
      errors=$((errors + 1))
    fi
  }

  check_in_squashfs "etc/systemd/system/mentalOS.service"
  check_in_squashfs "etc/systemd/system/openclaw.service"
  check_in_squashfs "etc/systemd/system/ollama.service"
  check_in_squashfs "etc/systemd/system/workspace-monitor.service"
  check_in_squashfs "usr/local/bin/mentalOS"
  check_in_squashfs "usr/local/bin/mentalos-workspace-monitor"
  check_in_squashfs "etc/systemd/system/getty@tty1.service.d/override.conf"
  check_in_squashfs "etc/mentalos-build-info"

  sudo umount "${mount_dir}" 2>/dev/null || true
  rmdir "${mount_dir}" 2>/dev/null || true

  if [[ ${errors} -gt 0 ]]; then
    echo "" >&2
    echo "[build-iso] VERIFICATION FAILED: ${errors} expected files missing from the ISO." >&2
    echo "[build-iso] The ISO was built but is missing critical components." >&2
    echo "[build-iso] Likely causes:" >&2
    echo "[build-iso]   1. The airootfs overlay didn't get applied" >&2
    echo "[build-iso]   2. customize_airootfs.sh failed during build" >&2
    echo "[build-iso]   3. You're building from an older commit" >&2
    echo "[build-iso] Check: git log --oneline -3 (should show 84ece0c or later)" >&2
    exit 1
  else
    log "Verification passed: all expected files present in squashfs"
  fi
}

main() {
  trap cleanup_stage EXIT
  require_tools
  build_binary_if_needed
  stage_binary
  stage_installer_assets
  build_iso
  verify_build
  log "ISO build complete. Output files:"
  ls -lh "${OUT_DIR}"/*.iso
}

main "$@"

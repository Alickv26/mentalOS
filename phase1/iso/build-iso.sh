#!/usr/bin/env bash
# mentalOS Phase 1 ISO builder — hardened version.
#
# Changes vs. previous version:
#   - Full mkarchiso output is captured to out/build.log for post-mortems.
#   - PREFLIGHT checks run BEFORE mkarchiso: if any staged/overlay file is
#     missing the build stops immediately (no 10-minute wait to find out).
#   - unsquashfs is a required tool (the old verifier silently produced an
#     empty listing when it was missing, which looked like "files missing").
#   - Post-build verification sanity-checks the squashfs listing itself and
#     verifies the work tree (overlay landing + customize_airootfs execution).
#   - Optional openclaw binary staging (openclaw.service skips via
#     ConditionPathExists when the binary is not staged).
#
# Environment overrides (see README.md):
#   MENTALOS_BINARY, MENTALOS_WORKSPACE_MONITOR_BINARY, MENTALOS_OPENCLAW_BINARY,
#   USE_EXISTING_BINARY, FORCE_REBUILD, AUTO_INSTALL_TOOLS, CLEAN_WORK,
#   WORK_DIR, KEEP_STAGE
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PROTO_DIR="${REPO_ROOT}/mentalOS-prototype"
PROFILE_DIR="${SCRIPT_DIR}"
OUT_DIR="${PROFILE_DIR}/out"
WORK_DIR="${WORK_DIR:-${PROFILE_DIR}/work}"
BUILD_LOG="${OUT_DIR}/build.log"
LISTING_FILE="${OUT_DIR}/squashfs-listing.txt"
DEFAULT_BINARY_PATH="${PROTO_DIR}/target/release/mentalOS"
BINARY_PATH="${MENTALOS_BINARY:-${DEFAULT_BINARY_PATH}}"
DEFAULT_WORKSPACE_MONITOR_PATH="${PROTO_DIR}/target/release/mentalos-workspace-monitor"
WORKSPACE_MONITOR_BINARY_PATH="${MENTALOS_WORKSPACE_MONITOR_BINARY:-${DEFAULT_WORKSPACE_MONITOR_PATH}}"
DEFAULT_OPENCLAW_PATH="${REPO_ROOT}/openclaw/target/release/openclaw"
OPENCLAW_BINARY_PATH="${MENTALOS_OPENCLAW_BINARY:-${DEFAULT_OPENCLAW_PATH}}"
USE_EXISTING_BINARY="${USE_EXISTING_BINARY:-1}"
FORCE_REBUILD="${FORCE_REBUILD:-0}"
AUTO_INSTALL_TOOLS="${AUTO_INSTALL_TOOLS:-1}"
CLEAN_WORK="${CLEAN_WORK:-1}"
INSTALLER_SOURCE_DIR="${REPO_ROOT}/phase1/arch-base"
INSTALLER_STAGE_DIR="${PROFILE_DIR}/airootfs/opt/mentalos/arch-base"
KEEP_STAGE="${KEEP_STAGE:-0}"

log() { printf '[build-iso] %s\n' "$*"; }
warn() { printf '[build-iso] WARN: %s\n' "$*" >&2; }
die() {
  printf '[build-iso] ERROR: %s\n' "$*" >&2
  if [[ -f "${BUILD_LOG}" ]]; then
    printf '[build-iso] See the full build log: %s\n' "${BUILD_LOG}" >&2
    printf '    Last 25 lines of the log:\n' >&2
    tail -n 25 "${BUILD_LOG}" | sed 's/^/    /' >&2 || true
  fi
  exit 1
}

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
  die "Missing required tool '${cmd}' (package: ${package}). Install it with: sudo pacman -S --needed ${package}"
}

require_tools() {
  ensure_tool mkarchiso archiso
  ensure_tool rsync rsync
  ensure_tool sudo sudo
  # unsquashfs is required by the verification step. archiso depends on
  # squashfs-tools, but verify explicitly: the old build script produced a
  # misleading "all files missing" failure when unsquashfs was absent.
  ensure_tool unsquashfs squashfs-tools
}

record_build_environment() {
  mkdir -p "${OUT_DIR}"
  {
    echo "=== mentalOS ISO build $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
    echo "Profile:      ${PROFILE_DIR}"
    echo "Repo commit:  $(git -C "${REPO_ROOT}" rev-parse HEAD 2>/dev/null || echo 'unknown')"
    echo "Repo dirty:   $(git -C "${REPO_ROOT}" status --short 2>/dev/null | wc -l) uncommitted entries"
    echo "Build host:   $(uname -sr)"
    echo "Host OS:      $(grep PRETTY_NAME /etc/os-release 2>/dev/null || echo 'unknown')"
    echo "archiso:      $(pacman -Q archiso 2>/dev/null || echo 'version unknown (not installed via pacman)')"
    echo "squashfs:     $(unsquashfs -version 2>/dev/null | head -n 1 || echo 'unknown')"
    echo ""
  } >"${BUILD_LOG}"
  log "Build host: $(uname -sr) — $(grep PRETTY_NAME /etc/os-release 2>/dev/null | cut -d= -f2- || echo '?')"
  log "NOTE: packages come from the repos in this host's mirrorlist. Building on"
  log "      Manjaro produces a Manjaro-based airootfs (cosmetic; branding is"
  log "      overridden at build time). Build in an Arch container for pure Arch."
}

build_binary_if_needed() {
  if [[ "${FORCE_REBUILD}" == "1" ]]; then
    log "FORCE_REBUILD=1, rebuilding mentalOS release binaries"
  elif [[ "${USE_EXISTING_BINARY}" == "1" && -x "${BINARY_PATH}" && -x "${WORKSPACE_MONITOR_BINARY_PATH}" ]]; then
    log "Using existing binaries: ${BINARY_PATH}, ${WORKSPACE_MONITOR_BINARY_PATH}"
    return
  else
    log "Building mentalOS release binaries (mentalOS + mentalos-workspace-monitor)"
  fi

  if ! cargo build --manifest-path "${PROTO_DIR}/Cargo.toml" --release --bin mentalOS --bin mentalos-workspace-monitor; then
    die "cargo build failed. The GTK4 build needs host packages:
    sudo pacman -S --needed base-devel rust pkgconf gtk4"
  fi
}

stage_binary() {
  if [[ ! -x "${BINARY_PATH}" ]]; then
    die "Expected binary not found or not executable: ${BINARY_PATH}"
  fi
  log "Staging mentalOS binary into airootfs"
  install -Dm0755 "${BINARY_PATH}" "${PROFILE_DIR}/airootfs/usr/local/bin/mentalOS"

  if [[ ! -x "${WORKSPACE_MONITOR_BINARY_PATH}" ]]; then
    die "Expected binary not found or not executable: ${WORKSPACE_MONITOR_BINARY_PATH}"
  fi
  log "Staging mentalos-workspace-monitor binary into airootfs"
  install -Dm0755 "${WORKSPACE_MONITOR_BINARY_PATH}" "${PROFILE_DIR}/airootfs/usr/local/bin/mentalos-workspace-monitor"
}

stage_openclaw() {
  # openclaw is optional: openclaw.service ships with
  # ConditionPathExists=/usr/local/bin/openclaw and is cleanly skipped at
  # boot when the binary was not staged.
  if [[ -x "${OPENCLAW_BINARY_PATH}" ]]; then
    log "Staging openclaw gateway binary into airootfs (${OPENCLAW_BINARY_PATH})"
    install -Dm0755 "${OPENCLAW_BINARY_PATH}" "${PROFILE_DIR}/airootfs/usr/local/bin/openclaw"
  else
    warn "openclaw binary not found at ${OPENCLAW_BINARY_PATH}"
    warn "openclaw.service will be skipped at boot (ConditionPathExists)."
    warn "Set MENTALOS_OPENCLAW_BINARY=/path/to/openclaw to stage it."
  fi
}

stage_installer_assets() {
  if [[ ! -d "${INSTALLER_SOURCE_DIR}" ]]; then
    die "Installer source directory missing: ${INSTALLER_SOURCE_DIR}"
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
  rm -f "${PROFILE_DIR}/airootfs/usr/local/bin/openclaw"
  rm -rf "${INSTALLER_STAGE_DIR}"
}

# ─── Preflight: everything mkarchiso needs must exist BEFORE we spend 10+ ────
# ─── minutes running it. This catches staging failures cheaply.            ────
preflight_checks() {
  local missing=0
  local f

  local expected=(
    "profiledef.sh"
    "pacman.conf"
    "packages.x86_64"
    "airootfs/root/customize_airootfs.sh"
    "airootfs/etc/mkinitcpio.conf.d/archiso.conf"
    "airootfs/etc/mkinitcpio.d/linux.preset"
    "airootfs/etc/skel/.config/sway/config"
    "airootfs/etc/systemd/system/mentalOS.service"
    "airootfs/etc/systemd/system/openclaw.service"
    "airootfs/etc/systemd/system/ollama.service"
    "airootfs/etc/systemd/system/workspace-monitor.service"
    "airootfs/etc/systemd/system/getty@tty1.service.d/override.conf"
    "airootfs/etc/systemd/system/graphical.target.wants/mentalOS.service"
    "airootfs/etc/systemd/system/multi-user.target.wants/openclaw.service"
    "airootfs/etc/systemd/system/multi-user.target.wants/ollama.service"
    "airootfs/etc/systemd/system/multi-user.target.wants/workspace-monitor.service"
    "airootfs/usr/lib/sysusers.d/mentalos.conf"
    "airootfs/usr/lib/tmpfiles.d/mentalos.conf"
    "airootfs/usr/local/bin/mentalos-launch"
    "airootfs/usr/local/bin/mentalos-install"
    "airootfs/usr/local/bin/mentalos-diagnose"
    # Staged at build time by this script:
    "airootfs/usr/local/bin/mentalOS"
    "airootfs/usr/local/bin/mentalos-workspace-monitor"
  )

  for f in "${expected[@]}"; do
    if [[ -e "${PROFILE_DIR}/${f}" || -L "${PROFILE_DIR}/${f}" ]]; then
      :
    else
      echo "[build-iso] PREFLIGHT FAIL: ${f} missing from profile tree" >&2
      missing=$((missing + 1))
    fi
  done

  # The .wants entries are symlinks — verify they resolve to real files.
  local link target
  for link in \
    "airootfs/etc/systemd/system/graphical.target.wants/mentalOS.service" \
    "airootfs/etc/systemd/system/multi-user.target.wants/openclaw.service" \
    "airootfs/etc/systemd/system/multi-user.target.wants/ollama.service" \
    "airootfs/etc/systemd/system/multi-user.target.wants/workspace-monitor.service"
  do
    target="$(readlink -f "${PROFILE_DIR}/${link}" 2>/dev/null || true)"
    if [[ -z "${target}" || ! -e "${target}" ]]; then
      echo "[build-iso] PREFLIGHT FAIL: symlink ${link} does not resolve" >&2
      missing=$((missing + 1))
    fi
  done

  if [[ "${missing}" -gt 0 ]]; then
    die "Preflight failed: ${missing} file(s) missing from the profile tree. Fix the tree before running mkarchiso."
  fi
  log "Preflight passed: ${#expected[@]} expected files present in profile tree"
}

build_iso() {
  mkdir -p "${OUT_DIR}"
  if [[ "${CLEAN_WORK}" == "1" ]]; then
    log "Cleaning work directory: ${WORK_DIR}"
    sudo rm -rf "${WORK_DIR}"
  fi
  log "Using work directory: ${WORK_DIR}"
  log "Running mkarchiso (full output logged to ${BUILD_LOG})"
  # shellcheck disable=SC2024  # stdin stays on the tty for sudo prompts
  if ! sudo mkarchiso -v -w "${WORK_DIR}" -o "${OUT_DIR}" "${PROFILE_DIR}" 2>&1 | tee -a "${BUILD_LOG}"; then
    die "mkarchiso failed. The full log is at ${BUILD_LOG}"
  fi
}

# ─── Work tree inspection: direct evidence of what mkarchiso actually did. ───
inspect_work_tree() {
  local airootfs="${WORK_DIR}/x86_64/airootfs"
  local markers marker_list

  log "Inspecting work tree: ${airootfs}"

  if [[ ! -d "${airootfs}" ]]; then
    warn "Work airootfs tree does not exist: ${airootfs}"
    warn "mkarchiso never created the pacstrap directory — check ${BUILD_LOG}"
    return
  fi

  marker_list="$(ls "${WORK_DIR}"/base._* 2>/dev/null | xargs -n1 basename 2>/dev/null || true)"
  log "mkarchiso step markers present: ${marker_list:-none}"

  local probes=(
    "etc/systemd/system/mentalOS.service"
    "etc/systemd/system/ollama.service"
    "usr/local/bin/mentalOS"
    "usr/local/bin/mentalos-workspace-monitor"
    "etc/systemd/system/getty@tty1.service.d/override.conf"
  )
  local p landed=0
  for p in "${probes[@]}"; do
    if [[ -e "${airootfs}/${p}" ]]; then
      landed=$((landed + 1))
    else
      warn "NOT in work airootfs: ${p} (overlay copy or staging failed)"
    fi
  done
  log "Work tree probes: ${landed}/${#probes[@]} files landed"

  # mkarchiso deletes customize_airootfs.sh from the work tree after running
  # it. Its absence is therefore PROOF that the script executed.
  if [[ -e "${airootfs}/root/customize_airootfs.sh" ]]; then
    warn "customize_airootfs.sh still present in work tree — it was NEVER RUN"
    warn "  (mkarchiso deletes it after execution; check ${BUILD_LOG} for"
    warn "   'Running customize_airootfs.sh')"
  else
    log "customize_airootfs.sh was executed (removed from work tree by mkarchiso)"
  fi

  if ! grep -q "Copying custom airootfs files" "${BUILD_LOG}" 2>/dev/null; then
    warn "Log does not contain 'Copying custom airootfs files'"
    warn "  → mkarchiso skipped the overlay copy. If ${PROFILE_DIR}/airootfs"
    warn "    exists in your checkout this indicates an archiso version issue."
  else
    log "Log confirms: 'Copying custom airootfs files' step ran"
  fi
}

# ─── Post-build verification of the produced ISO ─────────────────────────────
verify_build() {
  local iso_file
  # shellcheck disable=SC2012  # ls -1t is intentional — we want the most recently-built ISO
  iso_file="$(ls -1t "${OUT_DIR}"/*.iso 2>/dev/null | head -n 1 || true)"
  if [[ -z "${iso_file}" ]]; then
    die "No ISO file found in ${OUT_DIR}"
  fi

  log "Verifying ISO: ${iso_file}"

  local squashfs_rel_path="/arch/x86_64/airootfs.sfs" # matches install_dir=arch, arch=x86_64
  local squashfs_file="" mount_dir="" tmp_sfs=""

  # Method 1: loop-mount the ISO (fast, no big temp file).
  mount_dir="$(mktemp -d)"
  if sudo mount -o loop "${iso_file}" "${mount_dir}" 2>/dev/null; then
    if [[ -f "${mount_dir}${squashfs_rel_path}" ]]; then
      squashfs_file="${mount_dir}${squashfs_rel_path}"
    else
      warn "squashfs not at ${squashfs_rel_path} — ISO layout differs. Contents:"
      ls -R "${mount_dir}" 2>/dev/null | head -n 30 | sed 's/^/    /' >&2 || true
    fi
  else
    warn "Could not loop-mount the ISO for verification; trying bsdtar fallback"
  fi

  # Method 2: bsdtar extraction (works without loop devices).
  if [[ -z "${squashfs_file}" ]] && command -v bsdtar >/dev/null 2>&1; then
    tmp_sfs="${OUT_DIR}/.verify-airootfs.sfs"
    log "Extracting squashfs with bsdtar (needs ~1.5 GB temp space)"
    if bsdtar -x -f "${iso_file}" -O "arch/x86_64/airootfs.sfs" >"${tmp_sfs}" 2>/dev/null && [[ -s "${tmp_sfs}" ]]; then
      squashfs_file="${tmp_sfs}"
    else
      rm -f "${tmp_sfs}" 2>/dev/null || true
    fi
  fi

  if [[ -z "${squashfs_file}" ]]; then
    if [[ -n "${mount_dir}" ]]; then
      sudo umount "${mount_dir}" 2>/dev/null || true
      rmdir "${mount_dir}" 2>/dev/null || true
    fi
    die "Verification INCONCLUSIVE: could not access the squashfs inside the ISO.
    Neither loop-mount nor bsdtar extraction worked. Check the ISO is complete
    and that loop devices / libarchive (bsdtar) are available."
  fi

  # Generate the listing, capturing stderr so tool failures are visible.
  local unsquashfs_stderr
  unsquashfs_stderr="$(mktemp)"
  local listing=""
  listing="$(sudo unsquashfs -l "${squashfs_file}" 2>"${unsquashfs_stderr}")" || true
  printf '%s\n' "${listing}" >"${LISTING_FILE}"

  # ─── Sanity check the listing itself ───
  local line_count
  line_count="$(printf '%s\n' "${listing}" | grep -c . || true)"
  if [[ "${line_count}" -lt 100 ]]; then
    echo "[build-iso] unsquashfs stderr was:" >&2
    sed 's/^/    /' "${unsquashfs_stderr}" >&2 || true
    rm -f "${unsquashfs_stderr}"
    sudo umount "${mount_dir}" 2>/dev/null || true
    rmdir "${mount_dir}" 2>/dev/null || true
    rm -f "${tmp_sfs}" 2>/dev/null || true
    die "Verification INCONCLUSIVE: squashfs listing has only ${line_count} lines.
    unsquashfs could not read the image (missing tool, unsupported compression,
    or a truncated ISO). The ISO may or may not be good — do not trust the
    checks below until this is resolved. Listing saved to ${LISTING_FILE}"
  fi
  rm -f "${unsquashfs_stderr}"
  log "Squashfs listing: ${line_count} entries (saved to ${LISTING_FILE})"

  # Reference files that the base package set always provides. If these are
  # "missing" the listing itself is broken, not the ISO.
  local ref
  for ref in "usr/bin/bash" "usr/lib/systemd/system/multi-user.target"; do
    if ! printf '%s\n' "${listing}" | grep -qF -- "${ref}"; then
      sudo umount "${mount_dir}" 2>/dev/null || true
      rmdir "${mount_dir}" 2>/dev/null || true
      rm -f "${tmp_sfs}" 2>/dev/null || true
      die "Verification INCONCLUSIVE: reference file '${ref}' is not in the listing.
    The base system must contain it, so the listing itself is broken (tool or
    compression problem), not necessarily the ISO."
    fi
  done

  # ─── The actual mentalOS content checks ───
  local errors=0
  check_in_squashfs() {
    local path="$1"
    if printf '%s\n' "${listing}" | grep -qF -- "${path}"; then
      log "  OK: ${path} found in squashfs"
    else
      echo "[build-iso] ERROR: ${path} NOT found in squashfs!" >&2
      errors=$((errors + 1))
    fi
  }

  # From the committed airootfs overlay (copied by mkarchiso):
  check_in_squashfs "etc/systemd/system/mentalOS.service"
  check_in_squashfs "etc/systemd/system/openclaw.service"
  check_in_squashfs "etc/systemd/system/ollama.service"
  check_in_squashfs "etc/systemd/system/workspace-monitor.service"
  check_in_squashfs "etc/systemd/system/getty@tty1.service.d/override.conf"
  check_in_squashfs "etc/systemd/system/multi-user.target.wants/ollama.service"
  check_in_squashfs "etc/systemd/system/multi-user.target.wants/openclaw.service"
  check_in_squashfs "etc/systemd/system/multi-user.target.wants/workspace-monitor.service"
  check_in_squashfs "etc/systemd/system/graphical.target.wants/mentalOS.service"
  check_in_squashfs "usr/lib/sysusers.d/mentalos.conf"
  check_in_squashfs "usr/lib/tmpfiles.d/mentalos.conf"
  # Staged binaries:
  check_in_squashfs "usr/local/bin/mentalOS"
  check_in_squashfs "usr/local/bin/mentalos-workspace-monitor"
  # From packages.x86_64:
  check_in_squashfs "usr/bin/ollama"
  # Written by customize_airootfs.sh:
  check_in_squashfs "etc/mentalos-build-info"
  check_in_squashfs "etc/hostname"

  sudo umount "${mount_dir}" 2>/dev/null || true
  rmdir "${mount_dir}" 2>/dev/null || true
  rm -f "${tmp_sfs}" 2>/dev/null || true

  if [[ ${errors} -gt 0 ]]; then
    echo "" >&2
    echo "[build-iso] VERIFICATION FAILED: ${errors} expected file(s) missing from the ISO." >&2
    echo "[build-iso] Diagnostic evidence collected:" >&2
    echo "[build-iso]   - mkarchiso log:        ${BUILD_LOG}" >&2
    echo "[build-iso]   - squashfs listing:     ${LISTING_FILE}" >&2
    echo "[build-iso]   - work tree (persists): ${WORK_DIR}/x86_64/airootfs" >&2
    echo "[build-iso] Check the log for: 'Copying custom airootfs files'," >&2
    echo "[build-iso] 'Running customize_airootfs.sh' and 'Creating SquashFS image'." >&2
    exit 1
  else
    log "Verification passed: ${line_count} squashfs entries scanned, all expected files present"
    log "(reference: full listing at ${LISTING_FILE})"
  fi
}

main() {
  trap cleanup_stage EXIT
  require_tools
  record_build_environment
  build_binary_if_needed
  stage_binary
  stage_openclaw
  stage_installer_assets
  preflight_checks
  build_iso
  inspect_work_tree
  verify_build
  log "ISO build complete. Output files:"
  ls -lh "${OUT_DIR}"/*.iso
  log "Next steps:"
  log "  1. Boot test:    ./test-qemu.sh"
  log "  2. In the VM:    systemctl status mentalOS openclaw ollama workspace-monitor"
  log "  3. Diagnostics:  mentalos-diagnose"
}

main "$@"

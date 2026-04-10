#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ISO_PATH="${1:-}"
AUTO_INSTALL_TOOLS="${AUTO_INSTALL_TOOLS:-1}"

if [[ -z "${ISO_PATH}" ]]; then
  ISO_PATH="$(ls -1t "${SCRIPT_DIR}/out"/*.iso 2>/dev/null | head -n 1 || true)"
fi

if [[ -z "${ISO_PATH}" || ! -f "${ISO_PATH}" ]]; then
  echo "Usage: $0 /path/to/mentalos.iso" >&2
  echo "No ISO found in ${SCRIPT_DIR}/out" >&2
  exit 1
fi

if ! command -v qemu-system-x86_64 >/dev/null 2>&1; then
  if [[ "${AUTO_INSTALL_TOOLS}" == "1" ]] && command -v sudo >/dev/null 2>&1 && command -v pacman >/dev/null 2>&1; then
    echo "[test-qemu] qemu-system-x86_64 missing; installing qemu-desktop..."
    sudo pacman -S --needed --noconfirm qemu-desktop
  else
    echo "qemu-system-x86_64 is required" >&2
    exit 1
  fi
fi

find_ovmf_paths() {
  local code_candidates=(
    "/usr/share/OVMF/OVMF_CODE.fd"
    "/usr/share/edk2/x64/OVMF_CODE.fd"
    "/usr/share/edk2/x64/OVMF_CODE.4m.fd"
  )
  local vars_candidates=(
    "/usr/share/OVMF/OVMF_VARS.fd"
    "/usr/share/edk2/x64/OVMF_VARS.fd"
    "/usr/share/edk2/x64/OVMF_VARS.4m.fd"
  )
  local code vars

  for code in "${code_candidates[@]}"; do
    for vars in "${vars_candidates[@]}"; do
      if [[ -f "${code}" && -f "${vars}" ]]; then
        printf '%s\n%s\n' "${code}" "${vars}"
        return 0
      fi
    done
  done
  return 1
}

OVMF_PATHS="$(find_ovmf_paths || true)"
if [[ -z "${OVMF_PATHS}" && "${AUTO_INSTALL_TOOLS}" == "1" ]] && command -v sudo >/dev/null 2>&1 && command -v pacman >/dev/null 2>&1; then
  echo "[test-qemu] OVMF firmware not found; installing edk2-ovmf..."
  sudo pacman -S --needed --noconfirm edk2-ovmf
  OVMF_PATHS="$(find_ovmf_paths || true)"
fi

if [[ -z "${OVMF_PATHS}" ]]; then
  echo "OVMF firmware files not found. Install edk2-ovmf." >&2
  exit 1
fi

OVMF_CODE="$(printf '%s\n' "${OVMF_PATHS}" | sed -n '1p')"
OVMF_VARS_TEMPLATE="$(printf '%s\n' "${OVMF_PATHS}" | sed -n '2p')"
OVMF_VARS_RUNTIME="/tmp/OVMF_VARS_mentalos.fd"

if [[ ! -f "${OVMF_VARS_RUNTIME}" ]]; then
  cp "${OVMF_VARS_TEMPLATE}" "${OVMF_VARS_RUNTIME}"
fi

echo "Booting ${ISO_PATH} in QEMU..."
qemu-system-x86_64 \
  -enable-kvm \
  -m 4096 \
  -smp 4 \
  -cpu host \
  -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
  -drive if=pflash,format=raw,file="${OVMF_VARS_RUNTIME}" \
  -cdrom "${ISO_PATH}" \
  -boot d \
  -netdev user,id=net0 \
  -device virtio-net-pci,netdev=net0 \
  -display gtk

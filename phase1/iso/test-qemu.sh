#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ISO_PATH="${1:-}"

if [[ -z "${ISO_PATH}" ]]; then
  ISO_PATH="$(ls -1t "${SCRIPT_DIR}/out"/*.iso 2>/dev/null | head -n 1 || true)"
fi

if [[ -z "${ISO_PATH}" || ! -f "${ISO_PATH}" ]]; then
  echo "Usage: $0 /path/to/mentalos.iso" >&2
  echo "No ISO found in ${SCRIPT_DIR}/out" >&2
  exit 1
fi

if ! command -v qemu-system-x86_64 >/dev/null 2>&1; then
  echo "qemu-system-x86_64 is required" >&2
  exit 1
fi

echo "Booting ${ISO_PATH} in QEMU..."
qemu-system-x86_64 \
  -enable-kvm \
  -m 4096 \
  -smp 4 \
  -cpu host \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE.fd \
  -drive if=pflash,format=raw,file=/tmp/OVMF_VARS_mentalos.fd \
  -cdrom "${ISO_PATH}" \
  -boot d \
  -netdev user,id=net0 \
  -device virtio-net-pci,netdev=net0 \
  -display gtk

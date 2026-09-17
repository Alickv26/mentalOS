#!/usr/bin/env bash
# Arch Linux base installation for mentalOS (Phase 1.1)
# Run from Arch ISO after confirming target disk and network.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGES_FILE="${SCRIPT_DIR}/packages.txt"
TEMPLATE_ROOT="${SCRIPT_DIR}/config"
MOUNT_ROOT="/mnt"

INSTALL_DEV="${INSTALL_DEV:-/dev/sda}"
HOSTNAME="${HOSTNAME:-mentalOS}"
USERNAME="${USERNAME:-user}"
TIMEZONE="${TIMEZONE:-UTC}"
LOCALE="${LOCALE:-en_US.UTF-8}"
DRY_RUN="${DRY_RUN:-0}"
FORCE="${FORCE:-0}"

EFI_PART="${INSTALL_DEV}1"
ROOT_PART="${INSTALL_DEV}2"
HOME_PART="${INSTALL_DEV}3"

log() { printf '[install-base] %s\n' "$*"; }
run() {
  if [[ "$DRY_RUN" == "1" ]]; then
    printf '[dry-run] %s\n' "$*"
  else
    # shellcheck disable=SC2294  # eval is intentional — run() takes a shell command string for dry-run echo
    eval "$@"
  fi
}

require_root() {
  if [[ "${EUID}" -ne 0 ]]; then
    echo "This script must run as root." >&2
    exit 1
  fi
}

require_tools() {
  local tools=(parted mkfs.fat mkfs.ext4 pacstrap genfstab arch-chroot bootctl sed awk)
  local missing=()
  for t in "${tools[@]}"; do
    command -v "$t" >/dev/null 2>&1 || missing+=("$t")
  done
  if ((${#missing[@]})); then
    echo "Missing required tools: ${missing[*]}" >&2
    exit 1
  fi
}

confirm_target() {
  lsblk -o NAME,SIZE,TYPE,MOUNTPOINT "$INSTALL_DEV" || true
  if [[ "$FORCE" != "1" ]]; then
    echo "About to wipe and repartition ${INSTALL_DEV}."
    read -r -p "Type YES to continue: " confirm
    [[ "$confirm" == "YES" ]] || { echo "Aborted."; exit 1; }
  fi
}

partition_disk() {
  log "Partitioning ${INSTALL_DEV}"
  run "parted -s '${INSTALL_DEV}' mklabel gpt"
  run "parted -s '${INSTALL_DEV}' mkpart ESP fat32 1MiB 513MiB"
  run "parted -s '${INSTALL_DEV}' set 1 esp on"
  run "parted -s '${INSTALL_DEV}' mkpart root ext4 513MiB 30.5GiB"
  run "parted -s '${INSTALL_DEV}' mkpart home ext4 30.5GiB 100%"
}

format_partitions() {
  log "Formatting partitions"
  run "mkfs.fat -F32 '${EFI_PART}'"
  run "mkfs.ext4 -F '${ROOT_PART}'"
  run "mkfs.ext4 -F '${HOME_PART}'"
}

mount_partitions() {
  log "Mounting partitions"
  run "mount '${ROOT_PART}' '${MOUNT_ROOT}'"
  run "mkdir -p '${MOUNT_ROOT}/boot' '${MOUNT_ROOT}/home'"
  run "mount '${EFI_PART}' '${MOUNT_ROOT}/boot'"
  run "mount '${HOME_PART}' '${MOUNT_ROOT}/home'"
}

package_list() {
  awk 'NF && $1 !~ /^#/{print $1}' "${PACKAGES_FILE}"
}

install_base() {
  log "Installing base packages"
  local pkgs
  pkgs="$(package_list | tr '\n' ' ')"
  run "pacstrap -K '${MOUNT_ROOT}' ${pkgs}"
  run "genfstab -U '${MOUNT_ROOT}' >> '${MOUNT_ROOT}/etc/fstab'"
}

copy_templates() {
  log "Copying configuration templates"
  run "cp -r '${TEMPLATE_ROOT}/etc/.' '${MOUNT_ROOT}/etc/'"
  run "cp -r '${TEMPLATE_ROOT}/boot/.' '${MOUNT_ROOT}/boot/'"
  run "mkdir -p '${MOUNT_ROOT}/home/${USERNAME}'"
  run "cp -r '${TEMPLATE_ROOT}/home/user/.' '${MOUNT_ROOT}/home/${USERNAME}/'"
  run "sed -i 's/mentalOS/${HOSTNAME}/g' '${MOUNT_ROOT}/etc/hostname'"
  run "sed -i 's/mentalOS.localdomain mentalOS/${HOSTNAME}.localdomain ${HOSTNAME}/g' '${MOUNT_ROOT}/etc/hosts'"
}

chroot_configure() {
  log "Configuring system in chroot"

  local root_uuid
  root_uuid="$(blkid -s UUID -o value "${ROOT_PART}")"
  if [[ -z "$root_uuid" ]]; then
    echo "Could not determine root UUID" >&2
    exit 1
  fi
  run "sed -i 's/__ROOT_UUID__/${root_uuid}/g' '${MOUNT_ROOT}/boot/loader/entries/mentalos.conf'"

  local chroot_script
  chroot_script="$(mktemp)"
  cat >"${chroot_script}" <<CHROOT
set -euo pipefail
ln -sf /usr/share/zoneinfo/${TIMEZONE} /etc/localtime
hwclock --systohc
sed -i 's/^#${LOCALE}/${LOCALE}/' /etc/locale.gen
locale-gen
echo 'LANG=${LOCALE}' > /etc/locale.conf

systemctl enable NetworkManager
systemctl enable iwd

useradd -m -G wheel -s /bin/bash '${USERNAME}' || true
mkdir -p /etc/sudoers.d
echo '%wheel ALL=(ALL:ALL) ALL' > /etc/sudoers.d/10-wheel
chmod 0440 /etc/sudoers.d/10-wheel

bootctl install
CHROOT
  if [[ "$DRY_RUN" == "1" ]]; then
    sed 's/^/[dry-run chroot] /' "${chroot_script}"
  else
    arch-chroot "${MOUNT_ROOT}" /bin/bash < "${chroot_script}"
  fi
  rm -f "${chroot_script}"

  run "chown -R 1000:1000 '${MOUNT_ROOT}/home/${USERNAME}'"
}

summary() {
  cat <<MSG

Installation steps complete.

Next steps:
1. Set user/root passwords:
   arch-chroot ${MOUNT_ROOT} passwd ${USERNAME}
   arch-chroot ${MOUNT_ROOT} passwd
2. Install mentalOS binary to /usr/local/bin/mentalOS.
3. Reboot and verify autologin -> sway -> mentalOS startup.

MSG
}

main() {
  require_root
  require_tools
  confirm_target
  partition_disk
  format_partitions
  mount_partitions
  install_base
  copy_templates
  chroot_configure
  summary
}

main "$@"

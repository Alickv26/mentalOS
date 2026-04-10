#!/usr/bin/env bash
set -euo pipefail

# Some Arch derivatives ship kernel/initramfs with versioned names (e.g. vmlinuz-5.10-x86_64).
# Archiso boot entries expect canonical names and mkarchiso copies all vmlinuz-*/initramfs-*.img
# matches into the EFI FAT image. Use rename (not symlink) to avoid duplicate kernel payloads.
if [[ ! -e /boot/vmlinuz-linux ]]; then
  first_kernel="$(find /boot -maxdepth 1 -type f -name 'vmlinuz-*' | head -n 1 || true)"
  if [[ -n "${first_kernel}" ]]; then
    mv "${first_kernel}" /boot/vmlinuz-linux
  fi
fi

if [[ ! -e /boot/initramfs-linux.img ]]; then
  first_initramfs="$(
    find /boot -maxdepth 1 -type f -name 'initramfs-*.img' ! -name '*fallback*' | head -n 1 || true
  )"
  if [[ -n "${first_initramfs}" ]]; then
    mv "${first_initramfs}" /boot/initramfs-linux.img
  fi
fi

useradd -m -G wheel -s /bin/bash user || true
mkdir -p /etc/sudoers.d
printf '%%wheel ALL=(ALL:ALL) ALL\n' > /etc/sudoers.d/10-wheel
chmod 0440 /etc/sudoers.d/10-wheel

systemctl enable NetworkManager.service
systemctl enable iwd.service

mkdir -p /etc/systemd/system/getty@tty1.service.d
cat > /etc/systemd/system/getty@tty1.service.d/override.conf <<OVERRIDE
[Service]
ExecStart=
ExecStart=-/usr/bin/agetty --autologin user --noclear %I $TERM
OVERRIDE

mkdir -p /home/user/.config/sway
cp /etc/skel/.config/sway/config /home/user/.config/sway/config
chown -R user:user /home/user/.config

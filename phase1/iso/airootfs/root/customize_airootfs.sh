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

# Keep only the primary initramfs image. Fallback/extra images can overflow
# archiso's EFI FAT image and cause "Disk full" during mkarchiso.
if [[ -e /boot/initramfs-linux.img ]]; then
  find /boot -maxdepth 1 -type f -name 'initramfs-*.img' ! -name 'initramfs-linux.img' -delete
fi

# Unlock root with a known password for emergency shell access
printf 'root:mentalos\n' | chpasswd

useradd -m -G wheel -s /bin/bash user || true
mkdir -p /etc/sudoers.d
printf '%%wheel ALL=(ALL:ALL) ALL\n' > /etc/sudoers.d/10-wheel
chmod 0440 /etc/sudoers.d/10-wheel

systemctl enable NetworkManager.service
systemctl enable iwd.service

# ── mentalOS system services ────────────────────────────────
# mentalOS.service        — the main GTK4 app (WantedBy graphical.target)
# openclaw.service        — OpenClaw gateway HTTP backend
# ollama.service          — local Ollama LLM server (fallback provider)
# workspace-monitor.service — file watcher for ~/workspaces
#
# The service unit files are staged into /etc/systemd/system/ by mkarchiso
# from the airootfs/etc/systemd/system/ tree. Enable them so they auto-start
# at boot.

systemctl enable openclaw.service
systemctl enable ollama.service
systemctl enable workspace-monitor.service
# mentalOS.service is WantedBy graphical.target so it auto-starts when
# the display comes up. Enable it explicitly so the user can manage it
# via `systemctl start/stop mentalOS`.
systemctl enable mentalOS.service

# Create the ollama system user and data directory.
# ollama.service runs as User=ollama and writes to /var/lib/ollama.
useradd -r -s /usr/sbin/nologin -d /var/lib/ollama ollama 2>/dev/null || true
install -d -o ollama -g ollama -m 0750 /var/lib/ollama
install -d -o ollama -g ollama -m 0755 /var/lib/ollama/models

# Pre-create the mentalOS state directory that mentalOS.service writes to
# (ReadWritePaths=/var/lib/mentalOS).
install -d -o user -g user -m 0755 /var/lib/mentalOS

# Pre-create the workspace monitor state directory under the user's home.
install -d -o user -g user -m 0755 /home/user/workspaces/.mentalOS

mkdir -p /etc/systemd/system/getty@tty1.service.d
cat > /etc/systemd/system/getty@tty1.service.d/override.conf <<OVERRIDE
[Service]
ExecStart=
ExecStart=-/usr/bin/agetty --autologin user --noclear %I $TERM
OVERRIDE

mkdir -p /home/user/.config/sway
cp /etc/skel/.config/sway/config /home/user/.config/sway/config
chown -R user:user /home/user/.config

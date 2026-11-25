#!/usr/bin/env bash
set -uo pipefail  # NOT -e: one failing systemctl enable shouldn't abort the whole script

# mentalOS live ISO rootfs customization script.
#
# This script runs INSIDE the chroot (via arch-chroot) during mkarchiso
# build. It customizes the live rootfs before the squashfs image is
# created.
#
# Robustness rules:
#   1. Critical user-facing features (autologin, sway config) come FIRST
#      so they survive even if later steps fail.
#   2. Every non-critical step uses `|| true` or explicit error handling.
#   3. We write a marker file + log so the booted system can be diagnosed.

MARKER=/etc/mentalos-build-info
LOG=/var/log/mentalos-customize.log

log() {
    printf '[customize_airootfs] %s\n' "$*" | tee -a "${LOG}" 2>/dev/null || true
}

log "=== mentalOS customize_airootfs starting at $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="

# Record what version of the script ran (for diagnostics)
cat > "${MARKER}" <<MARKER
customize_version=2
built_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
MARKER
chmod 0644 "${MARKER}"

# ══════════════════════════════════════════════════════════════════
# SECTION 1: CRITICAL user-facing features (must succeed)
# These come FIRST so they're applied even if later sections fail.
# ══════════════════════════════════════════════════════════════════

log "Section 1: Kernel/initramfs normalization"

# Some Arch derivatives ship kernel/initramfs with versioned names (e.g. vmlinuz-6.6-x86_64).
# Archiso boot entries expect canonical names and mkarchiso copies all vmlinuz-*/initramfs-*.img
# matches into the EFI FAT image. Use rename (not symlink) to avoid duplicate kernel payloads.
if [[ ! -e /boot/vmlinuz-linux ]]; then
  first_kernel="$(find /boot -maxdepth 1 -type f -name 'vmlinuz-*' | head -n 1 || true)"
  if [[ -n "${first_kernel}" ]]; then
    mv "${first_kernel}" /boot/vmlinuz-linux
    log "  Renamed kernel: ${first_kernel} -> /boot/vmlinuz-linux"
  fi
fi

if [[ ! -e /boot/initramfs-linux.img ]]; then
  first_initramfs="$(
    find /boot -maxdepth 1 -type f -name 'initramfs-*.img' ! -name '*fallback*' | head -n 1 || true
  )"
  if [[ -n "${first_initramfs}" ]]; then
    mv "${first_initramfs}" /boot/initramfs-linux.img
    log "  Renamed initramfs: ${first_initramfs} -> /boot/initramfs-linux.img"
  fi
fi

# Keep only the primary initramfs image. Fallback/extra images can overflow
# archiso's EFI FAT image and cause "Disk full" during mkarchiso.
if [[ -e /boot/initramfs-linux.img ]]; then
  find /boot -maxdepth 1 -type f -name 'initramfs-*.img' ! -name 'initramfs-linux.img' -delete
fi

log "Section 1: User + password setup"

# Unlock root with a known password for emergency shell access
printf 'root:mentalos\n' | chpasswd || log "  WARN: chpasswd failed"

useradd -m -G wheel -s /bin/bash user 2>/dev/null || log "  user 'user' already exists or creation failed"
mkdir -p /etc/sudoers.d
printf '%%wheel ALL=(ALL:ALL) ALL\n' > /etc/sudoers.d/10-wheel
chmod 0440 /etc/sudoers.d/10-wheel

log "Section 1: Getty autologin override (CRITICAL — user must reach shell)"

mkdir -p /etc/systemd/system/getty@tty1.service.d
cat > /etc/systemd/system/getty@tty1.service.d/override.conf <<OVERRIDE
[Service]
ExecStart=
ExecStart=-/usr/bin/agetty --autologin user --noclear %I $TERM
OVERRIDE
log "  Getty override written"

log "Section 1: Sway config for user"

mkdir -p /home/user/.config/sway
if [[ -f /etc/skel/.config/sway/config ]]; then
  cp /etc/skel/.config/sway/config /home/user/.config/sway/config
else
  # The skel config might not exist if the sway package doesn't ship one.
  # Write a minimal one that launches mentalOS.
  cat > /home/user/.config/sway/config <<'SWAYCFG'
# Minimal sway config for mentalOS kiosk-like startup
set $mod Mod4

output * bg #111111 solid_color

default_border pixel 1
focus_follows_mouse no

# Terminal fallback
bindsym $mod+Return exec foot
bindsym $mod+Shift+q kill
bindsym $mod+Shift+e exec swaynag -t warning -m 'Exit sway?' -b 'Exit' 'swaymsg exit'

# Start mentalOS immediately on session start
exec --no-startup-id mentalOS
SWAYCFG
fi
chown -R user:user /home/user/.config
log "  Sway config written"

# ══════════════════════════════════════════════════════════════════
# SECTION 2: System services (non-critical — each with || true fallback)
# ══════════════════════════════════════════════════════════════════

log "Section 2: Basic system services"

systemctl enable NetworkManager.service 2>&1 | tee -a "${LOG}" 2>/dev/null || log "  WARN: NetworkManager enable failed"
systemctl enable iwd.service 2>&1 | tee -a "${LOG}" 2>/dev/null || log "  WARN: iwd enable failed"

log "Section 2: mentalOS system services"

# mentalOS.service        — the main GTK4 app (WantedBy graphical.target)
# openclaw.service        — OpenClaw gateway HTTP backend
# ollama.service          — local Ollama LLM server (fallback provider)
# workspace-monitor.service — file watcher for ~/workspaces
#
# The service unit files are staged into /etc/systemd/system/ by mkarchiso
# from the airootfs/etc/systemd/system/ tree. Enable them so they auto-start
# at boot.

enable_service() {
  local svc="$1"
  local target="$2"  # e.g. multi-user.target or graphical.target

  # Method 1: try systemctl enable (works on most systems in a chroot)
  if systemctl enable "${svc}" 2>>"${LOG}"; then
    log "  Enabled ${svc} via systemctl"
    return 0
  fi

  # Method 2: create the symlink directly (always works)
  log "  systemctl enable failed for ${svc}, creating symlink manually"
  local wants_dir="/etc/systemd/system/${target}.wants"
  mkdir -p "${wants_dir}"
  if [[ -f "/etc/systemd/system/${svc}" ]]; then
    ln -sf "../${svc}" "${wants_dir}/${svc}"
    log "  Created symlink: ${wants_dir}/${svc} -> /etc/systemd/system/${svc}"
  else
    log "  ERROR: service file /etc/systemd/system/${svc} does not exist!"
    return 1
  fi
}

enable_service "openclaw.service" "multi-user.target" || true
enable_service "ollama.service" "multi-user.target" || true
enable_service "workspace-monitor.service" "multi-user.target" || true
enable_service "mentalOS.service" "graphical.target" || true

# ══════════════════════════════════════════════════════════════════
# SECTION 3: Users and directories for services
# ══════════════════════════════════════════════════════════════════

log "Section 3: Service users and directories"

# Create the ollama system user and data directory.
# ollama.service runs as User=ollama and writes to /var/lib/ollama.
if ! id -u ollama >/dev/null 2>&1; then
  useradd -r -s /usr/sbin/nologin -d /var/lib/ollama ollama || log "  WARN: ollama useradd failed"
fi
install -d -o ollama -g ollama -m 0750 /var/lib/ollama 2>/dev/null || {
  log "  WARN: /var/lib/ollama creation failed, trying without ownership"
  mkdir -p /var/lib/ollama
}
install -d -o ollama -g ollama -m 0755 /var/lib/ollama/models 2>/dev/null || {
  mkdir -p /var/lib/ollama/models
}

# Pre-create the mentalOS state directory that mentalOS.service writes to
# (ReadWritePaths=/var/lib/mentalOS).
install -d -o user -g user -m 0755 /var/lib/mentalOS 2>/dev/null || {
  log "  WARN: /var/lib/mentalOS creation failed"
  mkdir -p /var/lib/mentalOS
}

# Pre-create the workspace monitor state directory under the user's home.
install -d -o user -g user -m 0755 /home/user/workspaces/.mentalOS 2>/dev/null || {
  log "  WARN: ~/workspaces/.mentalOS creation failed"
  mkdir -p /home/user/workspaces/.mentalOS
}

# ══════════════════════════════════════════════════════════════════
# SECTION 4: Post-build diagnostics
# ══════════════════════════════════════════════════════════════════

log "Section 4: Post-build diagnostics"

# Record what we actually accomplished
{
  echo "--- Service files present ---"
  ls -la /etc/systemd/system/*.service 2>/dev/null || echo "  (none)"
  echo "--- Service symlinks (multi-user.target.wants) ---"
  ls -la /etc/systemd/system/multi-user.target.wants/ 2>/dev/null || echo "  (none)"
  echo "--- Service symlinks (graphical.target.wants) ---"
  ls -la /etc/systemd/system/graphical.target.wants/ 2>/dev/null || echo "  (none)"
  echo "--- /usr/local/bin contents ---"
  ls -la /usr/local/bin/ 2>/dev/null || echo "  (empty or missing)"
  echo "--- Users ---"
  cut -d: -f1 /etc/passwd | grep -E "^(user|ollama)$" || echo "  (user/ollama not found)"
} >> "${LOG}" 2>/dev/null || true

log "=== mentalOS customize_airootfs completed ==="

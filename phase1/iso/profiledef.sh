#!/usr/bin/env bash
# shellcheck disable=SC2034  # variables below are read by mkarchiso when it sources this file
# shellcheck disable=SC2154  # arrays below are expanded by mkarchiso, not by this script

iso_name="mentalos"
iso_label="MENTALOS_$(date +%Y%m)"
iso_publisher="mentalOS Project <https://github.com/Alickv26/mentalOS>"
iso_application="mentalOS Arch ISO"
iso_version="0.1.0"
install_dir="arch"
buildmodes=('iso')
bootmodes=(
  'bios.syslinux'
  'uefi.systemd-boot'
)
arch="x86_64"
pacman_conf="pacman.conf"
airootfs_image_type="squashfs"
# NOTE: 'compression' is NOT an archiso variable — it was silently ignored.
# airootfs_image_tool_options is the real setting (see archiso docs).
airootfs_image_tool_options=('-comp' 'zstd' '-Xcompression-level' '15' '-b' '1M')

file_permissions=(
  ["/etc/shadow"]="0:0:400"
  ["/root"]="0:0:750"
  ["/etc/systemd/system/mentalOS.service"]="0:0:644"
  ["/etc/systemd/system/openclaw.service"]="0:0:644"
  ["/etc/systemd/system/ollama.service"]="0:0:644"
  ["/etc/systemd/system/workspace-monitor.service"]="0:0:644"
  ["/usr/local/bin/mentalOS"]="0:0:755"
  ["/usr/local/bin/mentalos-workspace-monitor"]="0:0:755"
)

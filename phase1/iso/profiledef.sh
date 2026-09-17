#!/usr/bin/env bash
# shellcheck disable=SC2034  # variables below are read by mkarchiso when it sources this file
# shellcheck disable=SC2154  # arrays below are expanded by mkarchiso, not by this script

iso_name="mentalos"
iso_label="MENTALOS_$(date +%Y%m)"
iso_publisher="mentalOS <https://example.invalid>"
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
compression="zstd"

file_permissions=(
  ["/etc/shadow"]="0:0:400"
  ["/root"]="0:0:750"
  ["/etc/systemd/system/mentalOS.service"]="0:0:644"
  ["/etc/systemd/system/openclaw.service"]="0:0:644"
  ["/etc/systemd/system/ollama.service"]="0:0:644"
  ["/etc/systemd/system/workspace-monitor.service"]="0:0:644"
  ["/usr/local/bin/mentalos-workspace-monitor"]="0:0:755"
)

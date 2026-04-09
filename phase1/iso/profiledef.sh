#!/usr/bin/env bash

iso_name="mentalos"
iso_label="MENTALOS_$(date +%Y%m)"
iso_publisher="mentalOS <https://example.invalid>"
iso_application="mentalOS Arch ISO"
iso_version="0.1.0"
install_dir="arch"
buildmodes=('iso')
bootmodes=(
  'bios.syslinux.mbr'
  'bios.syslinux.eltorito'
  'uefi-x64.systemd-boot.esp'
  'uefi-x64.systemd-boot.eltorito'
)
arch="x86_64"
pacman_conf="pacman.conf"
airootfs_image_type="squashfs"
compression="zstd"

file_permissions=(
  ["/etc/shadow"]="0:0:400"
  ["/root"]="0:0:750"
)

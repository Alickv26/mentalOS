#!/usr/bin/env bash
set -euo pipefail

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

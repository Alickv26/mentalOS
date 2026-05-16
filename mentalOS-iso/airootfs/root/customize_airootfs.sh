#!/bin/bash
#
# mentalOS airootfs customization script
# Runs during ISO build to configure the system
#

set -e

echo "Running mentalOS post-install customization..."

# Set hostname
echo "mentalOS" > /etc/hostname

# Configure locale
sed -i 's/#en_US.UTF-8/en_US.UTF-8/' /etc/locale.gen
locale-gen
echo "LANG=en_US.UTF-8" > /etc/locale.conf

# Set timezone (default UTC)
ln -sf /usr/share/zoneinfo/UTC /etc/localtime

# Configure vconsole
cat > /etc/vconsole.conf << EOF
KEYMAP=us
FONT=
EOF

# Configure hosts
cat > /etc/hosts << EOF
127.0.0.1   localhost
::1         localhost
127.0.1.1   mentalOS.localdomain mentalOS
EOF

# Enable NetworkManager
systemctl enable NetworkManager

# Enable firejail
systemctl enable firejail

# Create default user
useradd -m -G wheel,storage,power -s /bin/bash alick
echo "alick:mentalOS" | chpasswd

# Enable sudo for wheel group
sed -i 's/# %wheel ALL=(ALL) ALL/%wheel ALL=(ALL) ALL/' /etc/sudoers

# Configure auto-login
mkdir -p /etc/systemd/system/getty@tty1.service.d
cat > /etc/systemd/system/getty@tty1.service.d/override.conf << EOF
[Service]
ExecStart=
ExecStart=-/usr/bin/agetty --autologin alick --noclear %I \$TERM
EOF

# Create mentalOS directories
mkdir -p /home/alick/workspaces
mkdir -p /home/alick/.config/mentalOS
mkdir -p /home/alick/.openclaw

# Set ownership
chown -R alick:alick /home/alick

# Configure Sway
mkdir -p /home/alick/.config/sway
cat > /home/alick/.config/sway/config << 'EOF'
set $mod Mod4
set $term alacritty
set $menu wofi --show drun

default_border pixel 3
hide_edge_borders smart

exec_always mentalOS

bar {
    position top
    status_command while date +'%Y-%m-%d %H:%M'; do sleep 1; done
}
EOF

chown -R alick:alick /home/alick/.config

# Configure firejail profiles
mkdir -p /etc/firejail

# Copy config templates
cat > /home/alick/.config/mentalOS/config.toml << 'EOF'
[general]
hostname = "mentalOS"
workspace_dir = "/home/alick/workspaces"

[ai]
default_model = "llama3.1"
fallback_to_local = true

[openclaw]
host = "localhost"
port = 3000

[ollama]
host = "localhost"
port = 11434
model = "llama3.1"

[security]
require_approval = true
log_commands = true
EOF

chown alick:alick /home/alick/.config/mentalOS/config.toml

# Create whitelist
cat > /home/alick/.config/mentalOS/whitelist.json << 'EOF'
{
  "commands": ["git *", "cargo *", "npm *", "python *", "ls", "cat", "head", "tail"],
  "temporary": {}
}
EOF

chown alick:alick /home/alick/.config/mentalOS/whitelist.json

# Clean up pacman cache
pacman -Scc --noconfirm

echo "mentalOS customization complete!"
#!/bin/bash
#
# mentalOS Installation Script
# Phase 1.1 - Minimal Arch Linux Base
#
# Target: Second SSD (~35GB)
# Username: Alick

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
HOSTNAME="mentalOS"
USERNAME="alick"
TARGET_DISK=""
ROOT_SIZE="30G"
SWAP_SIZE="4G"
EFI_SIZE="512M"
FLASH_DRIVE_MODE=0
DRY_RUN=0
AUTO_YES=0

# Partition defaults for SSD mode
ROOT_SIZE="30G"
SWAP_SIZE="4G"
EFI_SIZE="512M"

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Run command or print dry-run message
run_or_dry() {
    if [[ $DRY_RUN -eq 1 ]]; then
        echo -e "${YELLOW}[DRY RUN]${NC} Would execute: $*"
    else
        eval "$@"
    fi
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --dry-run)
                DRY_RUN=1
                shift
                ;;
            --real)
                DRY_RUN=0
                shift
                ;;
            --disk=*)
                TARGET_DISK="${1#*=}"
                shift
                ;;
            --root-size=*)
                ROOT_SIZE="${1#*=}"
                shift
                ;;
            --swap-size=*)
                SWAP_SIZE="${1#*=}"
                shift
                ;;
            --efi-size=*)
                EFI_SIZE="${1#*=}"
                shift
                ;;
            --mode=*)
                INSTALL_MODE="${1#*=}"
                shift
                ;;
            --yes)
                AUTO_YES=1
                shift
                ;;
            -h|--help)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --dry-run       Show what would happen without making changes"
                echo "  --real          Run actual installation (default)"
                echo "  --disk=/dev/sdX  Specify target disk (skip prompt)"
                echo "  --root-size=NG (e.g. 8G)"
                echo "  --swap-size=NG (e.g. 2G)"
                echo "  --mode=ssd|usb Installation mode"
                echo "  --yes         Auto-confirm partitioning"
                echo "  -h, --help    Show this help message"
                echo ""
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                ;;
        esac
    done
}

# Check if running as root (skip for dry-run)
check_root() {
    if [[ $DRY_RUN -eq 1 ]]; then
        log_info "Skipping root check for dry-run mode"
        return
    fi
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root (use sudo)"
    fi
}

# Detect hardware
detect_hardware() {
    log_info "Detecting hardware..."
    
    # Detect GPU
    GPU=""
    if lspci | grep -i nvidia > /dev/null; then
        GPU="nvidia"
        log_info "Detected NVIDIA GPU"
    elif lspci | grep -i intel > /dev/null; then
        GPU="intel"
        log_info "Detected Intel GPU"
    elif lspci | grep -i amd > /dev/null; then
        GPU="amd"
        log_info "Detected AMD GPU"
    else
        GPU="basic"
        log_info "Using basic Mesa drivers"
    fi
    
    # Detect audio (laptop = likely need audio)
    HAS_AUDIO=0
    if lspci | grep -i "audio\|sound" > /dev/null; then
        HAS_AUDIO=1
        log_info "Audio hardware detected"
    else
        log_warn "No audio hardware detected"
    fi
    
    # Detect wireless
    HAS_WIFI=0
    if lspci | grep -i wireless > /dev/null || ip link show | grep -i wlan > /dev/null; then
        HAS_WIFI=1
        log_info "Wireless hardware detected"
    else
        log_warn "No wireless hardware detected"
    fi
    
    export GPU HAS_AUDIO HAS_WIFI
}

# Prompt for installation mode
prompt_install_mode() {
    # If sizes provided via CLI, skip mode prompt
    if [[ "$ROOT_SIZE" != "30G" || "$SWAP_SIZE" != "4G" || "$INSTALL_MODE" == "usb" ]]; then
        FLASH_DRIVE_MODE=1
        log_info "Custom partition sizes enabled"
        return
    fi
    
    # If specified via command line, skip
    if [[ -z "$INSTALL_MODE" ]]; then
        echo ""
        echo "Installation mode:"
        echo "  1) Internal SSD (default)"
        echo "  2) USB Flash Drive"
        echo ""
        read -p "Select mode [1-2]: " mode_choice
        
        case $mode_choice in
            2)
                FLASH_DRIVE_MODE=1
                ROOT_SIZE="15G"
                SWAP_SIZE="2G"
                EFI_SIZE="256M"
                log_info "Flash drive mode enabled"
                ;;
            *)
                FLASH_DRIVE_MODE=0
                log_info "SSD mode selected"
                ;;
        esac
    elif [[ "$INSTALL_MODE" == "usb" ]]; then
        FLASH_DRIVE_MODE=1
        ROOT_SIZE="15G"
        SWAP_SIZE="2G"
        EFI_SIZE="256M"
        log_info "Flash drive mode enabled"
    fi
    
    if [[ $FLASH_DRIVE_MODE -eq 1 ]]; then
        log_info "Optimizing for USB: smaller partitions"
    fi
}

# Select target disk
select_disk() {
    # If disk was passed via --disk, skip prompt
    if [[ -n "$TARGET_DISK" ]]; then
        if [[ ! -b "$TARGET_DISK" ]]; then
            log_error "Invalid disk: $TARGET_DISK"
        fi
        log_info "Using specified disk: $TARGET_DISK"
        return
    fi
    
    log_info "Available disks:"
    lsblk -o NAME,SIZE,TYPE,MOUNTPOINT
    
    echo ""
    read -p "Enter target disk (e.g., /dev/sdb): " TARGET_DISK
    
    if [[ ! -b "$TARGET_DISK" ]]; then
        log_error "Invalid disk: $TARGET_DISK"
    fi
    
    log_info "Selected: $TARGET_DISK"
}

# Show partition plan
show_partition_plan() {
    echo ""
    log_info "Partition plan for $TARGET_DISK:"
    echo "  EFI:     ${EFI_SIZE}  (FAT32)"
    echo "  Root:    ${ROOT_SIZE} (ext4)"
    echo "  Swap:    ${SWAP_SIZE} (swap)"
    echo "  Home:    Remaining"
    echo ""
    
    if [[ $AUTO_YES -eq 1 ]]; then
        log_info "Auto-confirming (--yes flag)"
        return
    fi
    
    read -p "Proceed with partitioning? (y/N): " confirm
    if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
        log_error "Installation cancelled"
    fi
}

# Partition the disk
partition_disk() {
    log_info "Creating partitions..."
    
    # Convert size strings to numbers only
    local efi_mb root_mb swap_mb
    
    efi_mb=$(echo "$EFI_SIZE" | tr -d '[:space:]' | sed 's/[^0-9]//g')
    
    if echo "$ROOT_SIZE" | grep -q 'G'; then
        root_mb=$(echo "$ROOT_SIZE" | sed 's/G//' | awk '{print $1 * 1024}')
    else
        root_mb=$(echo "$ROOT_SIZE" | sed 's/M//' | sed 's/[^0-9]//g')
    fi
    
    if echo "$SWAP_SIZE" | grep -q 'G'; then
        swap_mb=$(echo "$SWAP_SIZE" | sed 's/G//' | awk '{print $1 * 1024}')
    else
        swap_mb=$(echo "$SWAP_SIZE" | sed 's/M//' | sed 's/[^0-9]//g')
    fi
    
    log_info "Partition sizes: EFI=${efi_mb}MB Root=${root_mb}MB Swap=${swap_mb}MB"
    
    # Unmount any mounted partitions
    umount ${TARGET_DISK}* 2>/dev/null || true
    swapoff ${TARGET_DISK}* 2>/dev/null || true
    
    # Get actual disk size in MiB
    local disk_mb=$(sudo blockdev --getsize64 $TARGET_DISK | awk '{print int($1/1024/1024)}')
    log_info "Disk size: ${disk_mb}MB"
    
    # Calculate safe sizes if needed
    local total_needed=$((efi_mb + root_mb + swap_mb))
    local home_mb=$((disk_mb - total_needed))
    
    if [[ $home_mb -lt 100 ]]; then
        # Not enough for home, adjust root to leave 500MB for home
        home_mb=500
        root_mb=$((disk_mb - efi_mb - swap_mb - home_mb))
        log_warn "Adjusted root to ${root_mb}MB to fit disk"
    fi
    
    log_info "Creating partitions with parted..."
    
    # Use parted directly - create partition table first
    sudo parted -s $TARGET_DISK mklabel msdos
    
    # EFI partition (256MB)
    sudo parted -s $TARGET_DISK mkpart primary fat32 1MiB 257MiB
    sudo parted -s $TARGET_DISK set 1 boot on
    
    # Root partition
    local root_end=$((257 + root_mb))
    sudo parted -s $TARGET_DISK mkpart primary ext4 257MiB ${root_end}MiB
    
    # Swap partition 
    local swap_start=$root_end
    local swap_end=$((swap_start + swap_mb))
    sudo parted -s $TARGET_DISK mkpart primary linux-swap ${swap_start}MiB ${swap_end}MiB
    
    # Home partition (remaining)
    sudo parted -s $TARGET_DISK mkpart primary ext4 ${swap_end}MiB 100%
    
    log_success "Partitions created"
    sudo parted -s $TARGET_DISK print
}

# Format partitions
format_partitions() {
    log_info "Formatting partitions..."
    
    # EFI (FAT32)
    sudo mkfs.fat -F32 ${TARGET_DISK}p1
    
    # Root (ext4)
    sudo mkfs.ext4 -F ${TARGET_DISK}p2
    
    # Swap
    sudo mkswap ${TARGET_DISK}p3
    
    # Home (ext4)
    sudo mkfs.ext4 -F ${TARGET_DISK}p4
    
    # Enable swap
    sudo swapon ${TARGET_DISK}p3
    
    log_success "Partitions formatted"
}

# Mount partitions
mount_partitions() {
    log_info "Mounting partitions..."
    
    MOUNT_ROOT="/mnt/mentalOS"
    MOUNT_HOME="$MOUNT_ROOT/home/$USERNAME"
    MOUNT_EFI="$MOUNT_ROOT/boot/efi"
    
    mkdir -p $MOUNT_ROOT
    sudo mount ${TARGET_DISK}p2 $MOUNT_ROOT
    
    sudo mkdir -p $MOUNT_EFI
    sudo mount ${TARGET_DISK}p1 $MOUNT_EFI
    
    sudo mkdir -p $MOUNT_HOME
    sudo mount ${TARGET_DISK}p4 $MOUNT_HOME
    
    log_success "Partitions mounted at $MOUNT_ROOT"
}

# Install base packages - essential only for VM test
install_base_packages() {
    log_info "Installing base packages..."
    
    # Minimal essential packages only
    PACKAGES=(
        base
        base-devel
        linux
        linux-firmware
        systemd
        systemd-sysvcompat
        networkmanager
        iwd
        openssl
        ca-certificates
        efibootmgr
        wayland
        xorg-xwayland
        mesa
        sway
        wl-clipboard
        grim
        slurp
        wofi
        git
        curl
        wget
        vim
        nano
        man-db
        which
        findutils
        gawk
        sed
        grep
        coreutils
        util-linux
        procps-ng
        pciutils
        usbutils
        tar
        gzip
        xz
        zip
        unzip
        htop
        tree
        bash
        sudo
        polkit
        ttf-dejavu
        noto-fonts
        fontconfig
        gtk4
        adwaita-icon-theme
        xdg-user-dirs
        xdg-utils
        alacritty
    )
    
    # Install packages
    pacstrap $MOUNT_ROOT "${PACKAGES[@]}"
    
    log_success "Base packages installed"
}

# Generate fstab
generate_fstab() {
    log_info "Generating fstab..."
    
    genfstab -U -p $MOUNT_ROOT >> $MOUNT_ROOT/etc/fstab
    
    # Add swap (GPT uses p3)
    echo "${TARGET_DISK}p3 none swap sw 0 0" >> $MOUNT_ROOT/etc/fstab
    
    # Add USB optimizations if flash drive mode
    if [[ $FLASH_DRIVE_MODE -eq 1 ]]; then
        sed -i 's|/dev/loop.*p2  /           ext4    rw,relatime|/dev/loop*p2  /           ext4    rw,noatime,nodiratime|' $MOUNT_ROOT/etc/fstab
        sed -i 's|/dev/loop.*p4  /home       ext4    rw,relatime|/dev/loop*p4  /home       ext4    rw,noatime,nodiratime|' $MOUNT_ROOT/etc/fstab
    fi
    
    log_success "fstab generated"
}

# Configure system
configure_system() {
    log_info "Configuring system..."
    
    MOUNT_ROOT="/mnt/mentalOS"
    
    # Configure pacman for non-interactive
    sed -i 's/#NoProgressBar/NoProgressBar/' $MOUNT_ROOT/etc/pacman.conf
    sed -i 's/#Color/Color/' $MOUNT_ROOT/etc/pacman.conf
    
    # Hostname
    echo "$HOSTNAME" > $MOUNT_ROOT/etc/hostname
    
    # Hosts file
    cat > $MOUNT_ROOT/etc/hosts << EOF
127.0.0.1   localhost
::1         localhost
127.0.1.1   ${HOSTNAME}.localdomain ${HOSTNAME}
EOF
    
    # Locale
    sed -i 's/#en_US.UTF-8 UTF-8/en_US.UTF-8 UTF-8/' $MOUNT_ROOT/etc/locale.gen
    arch-chroot $MOUNT_ROOT locale-gen
    
    # Vconsole
    cat > $MOUNT_ROOT/etc/vconsole.conf << EOF
KEYMAP=us
FONT=
EOF
    
    # Timezone (default UTC, can be changed later)
    ln -sf /usr/share/zoneinfo/UTC $MOUNT_ROOT/etc/localtime
    
log_success "System configured"
}

# Configure Sway
configure_sway() {
    log_info "Configuring Sway..."
    
    MOUNT_ROOT="/mnt/mentalOS"
    USER_HOME="$MOUNT_ROOT/home/$USERNAME"
    
    mkdir -p $USER_HOME/.config/sway
    mkdir -p $USER_HOME/.config/mentalOS
    
    # Sway config
    cat > $USER_HOME/.config/sway/config << 'EOF'
# mentalOS Sway Configuration

# Variables
set $mod Mod4
set $term alacritty
set $menu wofi --show drun

# Default layout
default_border pixel 3
default_floating_border normal
hide_edge_borders smart

# Colors (mentalOS dark theme)
set $bg #1e1e2e
set $fg #cdd6f4
set $accent #89b4fa
set $urgent #f38ba8

# Window colors
client.background $bg
client.focused $accent $bg $fg $accent
client.unfocused $bg $bg $fg $bg
client.urgent $urgent $urgent $fg $urgent

# Input
input * {
    xkb_layout us
    xkb_options caps:escape
}

# Output
output * bg #1e1e2e solid_color

# Key bindings
bindsym $mod+Return exec $term
bindsym $mod+Shift+Return exec $term
bindsym $mod+d exec $menu
bindsym $mod+Shift+e exit
bindsym $mod+l exec swaylock -f

# Launch mentalOS on login
exec_always mentalOS

# Status bar
bar {
    position top
    status_command while date +'%Y-%m-%d %H:%M'; do sleep 1; done
    colors {
        background $bg
        statusline $fg
        focused_workspace $accent $bg $fg
        inactive_workspace $bg $bg $fg
        urgent_workspace $urgent $urgent $fg
    }
}
EOF
    
    log_success "Sway configured"
}

# Configure systemd-boot
configure_boot() {
    log_info "Configuring systemd-boot..."
    
    MOUNT_ROOT="/mnt/mentalOS"
    
    # Boot entry
    mkdir -p $MOUNT_ROOT/boot/loader/entries
    
    cat > $MOUNT_ROOT/boot/loader/entries/mentalOS.conf << EOF
title   mentalOS
linux   /vmlinuz-linux
initrd  /initramfs-linux.img
options root=${TARGET_DISK}2 rw quiet splash
EOF
    
    # Loader config
    cat > $MOUNT_ROOT/boot/loader/loader.conf << EOF
default mentalOS
timeout 5
editor  no
EOF
    
    log_success "Boot configured"
}

# Configure auto-login
configure_autologin() {
    log_info "Configuring auto-login..."
    
    MOUNT_ROOT="/mnt/mentalOS"
    
    mkdir -p $MOUNT_ROOT/etc/systemd/system/getty@tty1.service.d
    
    cat > $MOUNT_ROOT/etc/systemd/system/getty@tty1.service.d/override.conf << EOF
[Service]
ExecStart=
ExecStart=-/usr/bin/agetty --autologin $USERNAME --noclear %I $TERM
EOF
    
    log_success "Auto-login configured"
}

# Configure NetworkManager
configure_network() {
    log_info "Configuring NetworkManager..."
    
    MOUNT_ROOT="/mnt/mentalOS"
    
    # Enable NetworkManager
    arch-chroot $MOUNT_ROOT systemctl enable NetworkManager
    
    # Create wpa_supplicant config if needed
    if [[ $HAS_WIFI -eq 1 ]]; then
        mkdir -p $MOUNT_ROOT/etc/wpa_supplicant
        touch $MOUNT_ROOT/etc/wpa_supplicant/wpa_supplicant.conf
    fi
    
    log_success "Network configured"
}

# Create mentalOS directories
create_directories() {
    log_info "Creating mentalOS directories..."
    
    MOUNT_ROOT="/mnt/mentalOS"
    USER_HOME="$MOUNT_ROOT/home/$USERNAME"
    
    mkdir -p $USER_HOME/workspaces
    mkdir -p $USER_HOME/.config/mentalOS
    mkdir -p $USER_HOME/.openclaw
    mkdir -p $USER_HOME/.local/share
    
    # Set ownership
    chown -R $USERNAME:$USERNAME $USER_HOME
    
    log_success "Directories created"
}

# Copy config templates
copy_config_templates() {
    log_info "Copying config templates..."
    
    MOUNT_ROOT="/mnt/mentalOS"
    USER_HOME="$MOUNT_ROOT/home/$USERNAME"
    
    # Copy mentalOS config template
    if [[ -f "$(dirname "$0")/../mentalOS-prototype/config.example.toml" ]]; then
        cp $(dirname "$0")/../mentalOS-prototype/config.example.toml $USER_HOME/.config/mentalOS/config.toml
        chown $USERNAME:$USERNAME $USER_HOME/.config/mentalOS/config.toml
    fi
    
    # Copy whitelist template
    cat > $USER_HOME/.config/mentalOS/whitelist.json << 'EOF'
{
  "commands": [
    "git *",
    "cargo *",
    "npm *",
    "python *",
    "make *",
    "ls",
    "cat",
    "head",
    "tail",
    "grep"
  ],
  "temporary": {}
}
EOF
    chown $USERNAME:$USERNAME $USER_HOME/.config/mentalOS/whitelist.json
    
    # Copy firejail profile
    if [[ -f "$(dirname "$0")/../mentalOS-prototype/firejail/openclaw.profile" ]]; then
        mkdir -p $MOUNT_ROOT/etc/firejail
        cp $(dirname "$0")/../mentalOS-prototype/firejail/openclaw.profile $MOUNT_ROOT/etc/firejail/
    fi
    
    log_success "Config templates copied"
}

# Install OpenClaw (placeholder - will be done in Phase 1.3)
install_openclaw() {
    log_info "Installing OpenClaw..."
    log_warn "OpenClaw installation will be completed in Phase 1.3"
    
    # Create placeholder for now
    MOUNT_ROOT="/mnt/mentalOS"
    USER_HOME="$MOUNT_ROOT/home/$USERNAME"
    
    mkdir -p $USER_HOME/.openclaw
    echo "# OpenClaw will be installed here" > $USER_HOME/.openclaw/README
    
    log_success "OpenClaw placeholder created"
}

# Final setup
finalize() {
    log_info "Finalizing installation..."
    
    MOUNT_ROOT="/mnt/mentalOS"
    
    # Generate initramfs
    arch-chroot $MOUNT_ROOT mkinitcpio -P
    
    # Install boot loader
    arch-chroot $MOUNT_ROOT bootctl install
    
    log_success "Boot loader installed"
}

# Print summary
print_summary() {
    MODE_NAME=$([ $FLASH_DRIVE_MODE -eq 1 ] && echo "USB Flash Drive" || echo "Internal SSD")
    
    echo ""
    echo "=========================================="
    log_success "mentalOS installation complete!"
    echo "=========================================="
    echo ""
    echo "Installation summary:"
    echo "  Hostname:     $HOSTNAME"
    echo "  Username:     $USERNAME"
    echo "  Target disk:  $TARGET_DISK"
    echo "  Mode:         $MODE_NAME"
    echo "  GPU:          $GPU"
    echo "  Audio:        $([ $HAS_AUDIO -eq 1 ] && echo 'Yes' || echo 'No')"
    echo "  WiFi:         $([ $HAS_WIFI -eq 1 ] && echo 'Yes' || echo 'No')"
    echo ""
    echo "Partitions:"
    echo "  ${TARGET_DISK}1 - EFI     (${EFI_SIZE})"
    echo "  ${TARGET_DISK}2 - Root    (${ROOT_SIZE})"
    echo "  ${TARGET_DISK}3 - Swap    (${SWAP_SIZE})"
    echo "  ${TARGET_DISK}4 - Home    (remaining)"
    echo ""
    
    if [[ $FLASH_DRIVE_MODE -eq 1 ]]; then
        echo "USB-specific optimizations:"
        echo "  - noatime,nodiratime for flash longevity"
        echo "  - Smaller partition sizes"
        echo "  - Boot from USB in BIOS"
        echo ""
    fi
    
    echo "Next steps:"
    echo "  1. Reboot and select $TARGET_DISK from boot menu"
    echo "  2. Log in as $USERNAME"
    echo "  3. Run 'mentalOS' to start the application"
    echo ""
    log_warn "Ensure Secure Boot is disabled in BIOS"
}

# Main execution
main() {
    parse_args "$@"
    
    echo "=========================================="
    echo "  mentalOS Installation Script"
    echo "  Phase 1.1 - Minimal Arch Base"
    echo "=========================================="
    
    if [[ $DRY_RUN -eq 1 ]]; then
        echo ""
        echo -e "${YELLOW}[DRY RUN]${NC} No changes will be made"
        echo ""
    else
        echo ""
    fi
    
    check_root
    detect_hardware
    prompt_install_mode
    select_disk
    show_partition_plan
    
    if [[ $DRY_RUN -eq 1 ]]; then
        dry_run_show_plan
        exit 0
    fi
    
    partition_disk
    format_partitions
    mount_partitions
    install_base_packages
    generate_fstab
    configure_system
    configure_sway
    configure_boot
    configure_autologin
    configure_network
    create_directories
    copy_config_templates
    install_openclaw
    finalize
    print_summary
}

# Dry-run: show what would happen
dry_run_show_plan() {
    echo ""
    echo "=========================================="
    echo "           DRY RUN PLAN"
    echo "=========================================="
    echo ""
    echo "Hardware detected:"
    echo "  - GPU: $GPU"
    echo "  - Audio: $([ $HAS_AUDIO -eq 1 ] && echo 'Yes' || echo 'No')"
    echo "  - WiFi: $([ $HAS_WIFI -eq 1 ] && echo 'Yes' || echo 'No')"
    echo ""
    echo "Installation mode: $MODE_NAME"
    echo ""
    echo "Partition plan for $TARGET_DISK:"
    echo "  ${TARGET_DISK}1 - EFI     (${EFI_SIZE})"
    echo "  ${TARGET_DISK}2 - Root    (${ROOT_SIZE})"
    echo "  ${TARGET_DISK}3 - Swap    (${SWAP_SIZE})"
    echo "  ${TARGET_DISK}4 - Home    (remaining)"
    echo ""
    echo "Would execute:"
    echo "  1. Create GPT partition table"
    echo "  2. Format partitions (FAT32, ext4, swap)"
    echo "  3. Mount at /mnt/mentalOS"
    echo "  4. Install ~70 packages via pacstrap"
    echo "  5. Generate fstab"
    echo "  6. Configure hostname, locale, user"
    echo "  7. Install systemd-boot"
    echo "  8. Configure auto-login"
    echo "  9. Enable NetworkManager"
    echo "  10. Create mentalOS directories"
    echo ""
    echo "Would create config files:"
    echo "  - /etc/hostname"
    echo "  - /etc/hosts"
    echo "  - /etc/fstab"
    echo "  - /etc/vconsole.conf"
    echo "  - /etc/locale.gen"
    echo "  - /etc/systemd/system/getty@tty1.service.d/override.conf"
    echo "  - /boot/loader/entries/mentalOS.conf"
    echo "  - /home/alick/.config/sway/config"
    echo "  - /home/alick/.config/mentalOS/config.toml"
    echo "  - /home/alick/.config/mentalOS/whitelist.json"
    echo ""
    echo "Would enable services:"
    echo "  - NetworkManager"
    echo ""
    echo -e "${RED}[WARNING]${NC} This will DESTROY all data on $TARGET_DISK"
    echo ""
    echo "To run actual installation:"
    echo "  sudo ./install-base.sh --real"
    echo ""
    log_warn "Dry run complete - no changes made"
}

main "$@"
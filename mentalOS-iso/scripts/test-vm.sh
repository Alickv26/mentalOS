#!/bin/bash
#
# mentalOS VM Test Script
# Creates a virtual disk, runs install, boots in QEMU
#

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

DISK_SIZE=8
DISK_FILE="mentalOS-test.img"
LOOP_DEV=""
VM_RAM=4G
SPARSE=0

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ISO_DIR="$(dirname "$SCRIPT_DIR")"

cleanup() {
    if [[ -n "$LOOP_DEV" ]]; then
        log_info "Detaching loop device..."
        sudo losetup -d "$LOOP_DEV" 2>/dev/null || true
        LOOP_DEV=""
    fi
}

trap cleanup EXIT

show_help() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --create-only    Create disk and install, don't boot VM"
    echo "  --disk-size=N    Set disk size in GB (default: 20)"
    echo "  --ram=N          Set VM RAM (default: 4G)"
    echo "  --sparse         Use sparse file (faster, uses only needed space)"
    echo "  -h, --help       Show this help"
    echo ""
    echo "Examples:"
    echo "  $0                    # Full test (install + boot VM)"
    echo "  $0 --create-only      # Just install, no VM boot"
    echo "  $0 --disk-size=30     # 30GB disk"
    echo "  $0 --sparse           # Faster creation, less disk usage initially"
}

check_dependencies() {
    log_info "Checking dependencies..."
    
    # Check qemu
    if ! command -v qemu-system-x86_64 &> /dev/null; then
        log_info "Installing QEMU..."
        sudo pacman -S --noconfirm qemu-base ovmf
    fi
    
    # Check needed tools
    for cmd in dd losetup partprobe; do
        if ! command -v $cmd &> /dev/null; then
            log_error "$cmd not found"
        fi
    done
    
    log_success "Dependencies OK"
}

check_disk_space() {
    local required_gb=$1
    local available_kb=$(df -k . | tail -1 | awk '{print $4}')
    local available_gb=$((available_kb / 1024 / 1024))
    
    log_info "Available space: ${available_gb}GB"
    log_info "Required space:  ${required_gb}GB + 1GB buffer"
    
    # Use smaller buffer (1GB instead of 2GB)
    if [[ $available_gb -lt $((required_gb + 1)) ]]; then
        log_error "Not enough disk space. Need at least $((required_gb + 1))GB free."
    fi
    
    log_success "Space check passed"
}

create_disk() {
    log_info "Creating ${DISK_SIZE}GB disk image..."
    
    if [[ -f "$DISK_FILE" ]]; then
        log_warn "Disk file exists, removing..."
        rm -f "$DISK_FILE"
    fi
    
    # Use sparse file for faster creation (doesn't allocate blocks until used)
    # Use dd with status=progress for non-sparse with progress
    if [[ "$SPARSE" == "1" ]]; then
        truncate -s ${DISK_SIZE}G "$DISK_FILE"
    else
        dd if=/dev/zero of="$DISK_FILE" bs=1G count=$DISK_SIZE status=progress
    fi
    
    log_success "Disk image created: $DISK_FILE ($(du -h $DISK_FILE | cut -f1))"
}

setup_loop() {
    log_info "Setting up loop device..."
    
    LOOP_DEV=$(sudo losetup -f --show "$DISK_FILE")
    if [[ -z "$LOOP_DEV" ]]; then
        log_error "Failed to create loop device"
    fi
    
    log_success "Loop device: $LOOP_DEV"
    
    # Trigger partition scan
    sudo partprobe "$LOOP_DEV" 2>/dev/null || true
    sleep 1
}

run_install() {
    log_info "Running installation to $LOOP_DEV..."
    log_info "Mode: USB (for VM)"
    
    cd "$ISO_DIR"
    
    # Calculate partition sizes based on disk size
    # Leave 1GB for home, rest for root + swap
    local root_gb=$((DISK_SIZE - 2))  # root gets most
    local swap_gb=2
    
    if [[ $DISK_SIZE -lt 8 ]]; then
        root_gb=$((DISK_SIZE - 1))
        swap_gb=1
    fi
    
    log_info "Partition sizes: Root=${root_gb}GB Swap=${swap_gb}GB"
    
    sudo ./scripts/install-base.sh \
        --disk="$LOOP_DEV" \
        --root-size="${root_gb}G" \
        --swap-size="${swap_gb}G" \
        --efi-size="256M" \
        --mode=usb \
        --yes
    
    log_success "Installation complete"
}

show_install_results() {
    log_info "Installation summary:"
    echo ""
    echo "Mounted filesystems:"
    sudo mount | grep -E "mnt|mentalOS" || echo "  (none found - normal for loop device)"
    echo ""
    echo "Disk partition layout:"
    sudo fdisk -l "$LOOP_DEV" 2>/dev/null || lsblk "$LOOP_DEV"
}

find_ovmf() {
    local paths=(
        "/usr/share/edk2/ovmf/x64/OVMF.4m.fd"
        "/usr/share/ovmf/x64/OVMF.fd"
        "/usr/share/edk2/ovmf/x64/OVMF.fd"
        "/usr/share/qemu/ovmf-x86_64.bin"
    )
    
    for path in "${paths[@]}"; do
        if [[ -f "$path" ]]; then
            echo "$path"
            return 0
        fi
    done
    
    return 1
}

boot_vm() {
    log_info "Setting up UEFI boot..."
    
    local ovmf_path
    ovmf_path=$(find_ovmf)
    
    if [[ -z "$ovmf_path" ]]; then
        log_info "Installing edk2-ovmf..."
        sudo pacman -S --noconfirm edk2-ovmf
        ovmf_path=$(find_ovmf)
    fi
    
    if [[ -z "$ovmf_path" ]]; then
        # Try common paths
        for path in /usr/share/edk2/ovmf/x64/*.fd; do
            if [[ -f "$path" ]]; then
                ovmf_path="$path"
                break
            fi
        done
    fi
    
    if [[ -z "$ovmf_path" ]] || [[ ! -f "$ovmf_path" ]]; then
        log_error "OVMF not found. Install edk2-ovmf package."
    fi
    
    log_success "UEFI firmware: $ovmf_path"
    echo ""
    echo "=========================================="
    log_info "Starting VM..."
    echo "=========================================="
    echo ""
    log_warn "To exit: Close the window or press Ctrl+Alt+G"
    echo ""
    
    # Run QEMU with UEFI, KVM, GTK display
    sudo qemu-system-x86_64 \
        -m "$VM_RAM" \
        -smp 2 \
        -cpu host \
        -enable-kvm \
        -boot menu=on \
        -drive file="$DISK_FILE",format=raw,if=ide \
        -net nic,model=virtio \
        -net user \
        -display gtk \
        -bios "$ovmf_path" \
        -serial stdio
    
    log_success "VM session ended"
}

main() {
    CREATE_ONLY=0
    SPARSE=0
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --create-only)
                CREATE_ONLY=1
                shift
                ;;
            --disk-size=*)
                DISK_SIZE="${1#*=}"
                shift
                ;;
            --ram=*)
                VM_RAM="${1#*=}"
                shift
                ;;
            --sparse)
                SPARSE=1
                shift
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                ;;
        esac
    done
    
    echo "=========================================="
    echo "  mentalOS VM Test"
    echo "  Disk: ${DISK_SIZE}GB | RAM: $VM_RAM"
    echo "=========================================="
    echo ""
    
    check_dependencies
    check_disk_space $DISK_SIZE
    create_disk
    setup_loop
    run_install
    show_install_results
    
    if [[ $CREATE_ONLY -eq 1 ]]; then
        echo ""
        echo "=========================================="
        log_success "Installation complete!"
        echo "=========================================="
        echo ""
        echo "Disk image: $DISK_FILE"
        echo "Loop device: $LOOP_DEV"
        echo ""
        echo "To boot in VM later:"
        echo "  cd $ISO_DIR/mentalOS-iso/scripts"
        echo "  sudo ./test-vm.sh --disk-size=$DISK_SIZE"
        echo ""
        log_info "Loop device is still attached. Run 'sudo losetup -d $LOOP_DEV' to detach."
    else
        boot_vm
    fi
    
    log_success "Done!"
}

main "$@"
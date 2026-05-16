# mentalOS Partition Scheme

## Overview
This document describes the partition scheme for mentalOS installation on a secondary SSD (~35GB).

## Default Partition Layout

| Partition | Device     | Size    | File System | Mount Point | Purpose                          |
|-----------|------------|---------|-------------|-------------|----------------------------------|
| EFI       | /dev/sdX1  | 512MB   | FAT32       | /boot/efi   | UEFI boot loader                 |
| Root      | /dev/sdX2  | 30GB    | ext4        | /           | OS and applications              |
| Swap      | /dev/sdX3  | 4GB     | swap        | -           | Swap space                       |
| Home      | /dev/sdX4  | ~30GB   | ext4        | /home       | User data and workspaces         |

## Partition Details

### EFI System Partition (ESP)
- **Size**: 512MB (minimum required is 256MB, 512MB provides headroom)
- **Type**: FAT32
- **Flags**: `esp`, `boot`
- **Contents**:
  - systemd-boot loader
  - Linux kernel (vmlinuz-linux)
  - Initial ramdisk (initramfs-linux.img)

### Root Partition
- **Size**: 30GB
- **Type**: ext4
- **Contents**:
  - `/bin`, `/sbin`, `/lib` - Core binaries and libraries
  - `/usr` - Applications and libraries
  - `/etc` - System configuration
  - `/var` - Variable data (logs, cache)
  - `/opt` - Additional software
  - `/boot` - Boot files

### Swap Partition
- **Size**: 4GB (matches RAM for hibernate support)
- **Type**: linux-swap
- **Usage**:
  - Memory overflow
  - Hibernate support
  - Memory compression (zram)

### Home Partition
- **Size**: Remaining space (~30GB)
- **Type**: ext4
- **Contents**:
  - `/home/alick` - User home directory
    - `workspaces/` - AI-managed projects
    - `.config/mentalOS/` - mentalOS configuration
    - `.openclaw/` - OpenClaw data

## Customization

### Alternative Sizes
For different disk sizes, adjust proportionally:

**Smaller disk (20GB)**
- EFI: 256MB
- Root: 15GB
- Swap: 2GB
- Home: Remaining

**Larger disk (100GB+)**
- EFI: 512MB
- Root: 40GB
- Swap: 8GB
- Home: Remaining

### Alternative File Systems
- **btrfs**: Use for COW snapshots and subvolumes
- **xfs**: Use for large files and high-performance workloads
- **f2fs**: Use for SSDs with high write amplification

### Encrypted Partitions (Future)
Consider LUKS encryption for:
- Root partition (full disk encryption)
- Home partition (home encryption)
- Swap (encrypted swap)

## Partition Commands

### Using parted
```bash
parted /dev/sdb

# Create GPT partition table
mklabel gpt

# EFI partition
mkpart primary fat32 1MiB 513MiB
set 1 esp on

# Root partition
mkpart primary ext4 513MiB 30.5GiB

# Swap partition
mkpart primary linux-swap 30.5GiB 34.5GiB

# Home partition
mkpart primary ext4 34.5GiB 100%
```

### Using fdisk
```bash
fdisk /dev/sdb

# Create partitions using:
# n - new partition
# t - set type (1=EFI, 20=Linux swap, 23=Linux)
# w - write and quit
```

## Boot Process

1. UEFI firmware loads systemd-boot from EFI partition
2. systemd-boot reads `/boot/loader/entries/mentalOS.conf`
3. Kernel loads with root= parameter pointing to root partition
4. initramfs mounts home partition
5. Systemd starts, launches Sway and mentalOS

## Recovery Options

### Boot into Arch ISO
1. Boot from Arch Linux live USB
2. Mount partitions:
   ```bash
   mount /dev/sdb2 /mnt
   mount /dev/sdb1 /mnt/boot/efi
   mount /dev/sdb4 /mnt/home
   ```
3. Chroot: `arch-chroot /mnt`
4. Fix issues and run: `mkinitcpio -P`

### Backup and Restore
```bash
# Backup partitions
fsarchiver savear /dev/sdb2 /backup/root.fsa
fsarchiver savear /dev/sdb4 /backup/home.fsa

# Restore
fsarchiver restar /backup/root.fsa /dev/sdb2
fsarchiver restar /backup/home.fsa /dev/sdb4
```
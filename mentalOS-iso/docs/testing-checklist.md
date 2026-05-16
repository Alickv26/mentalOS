# mentalOS 1.1 Testing Checklist

## Pre-Installation Checks
- [ ] Backup existing data on target disk
- [ ] Verify 35GB+ free space on target SSD
- [ ] Create Arch Linux live USB
- [ ] Disable Secure Boot in BIOS
- [ ] Note current disk identifiers (lsblk)

## Installation Verification

### Partitioning
- [ ] Correct partition table created (GPT)
- [ ] EFI partition: 512MB, FAT32, esp flag set
- [ ] Root partition: 30GB, ext4
- [ ] Swap partition: 4GB, linux-swap type
- [ ] Home partition: ~30GB, ext4

### Base System
- [ ] System boots to terminal
- [ ] Root login works
- [ ] User 'alick' login works
- [ ] sudo works for wheel group

### Networking
- [ ] Ethernet connectivity (if available)
- [ ] WiFi scanning works (`nmcli device wifi list`)
- [ ] Can connect to WiFi network
- [ ] NetworkManager service running
- [ ] Internet access confirmed (`ping -c 3 archlinux.org`)

### Display
- [ ] Xorg/Wayland installed
- [ ] Sway starts (`sway`)
- [ ] Monitor detection works
- [ ] Correct resolution set
- [ ] Multi-monitor support (if applicable)

### GPU Acceleration
- [ ] Correct GPU driver loaded
  - Intel: `lsmod | grep i915`
  - AMD: `lsmod | grep amdgpu`
  - NVIDIA: `lsmod | grep nvidia`
- [ ] Hardware acceleration works (`glxinfo | grep direct`)
- [ ] No graphics glitches

### Audio
- [ ] PipeWire/PulseAudio installed
- [ ] Audio devices detected (`pactl list short sinks`)
- [ ] Volume controls work
- [ ] Can play audio

### Window Manager
- [ ] Sway loads without errors
- [ ] Key bindings work (Mod+Return for terminal)
- [ ] Window tiling works
- [ ] Status bar displays
- [ ] Can switch workspaces

### mentalOS Application
- [ ] mentalOS binary exists and runs
- [ ] GTK4 UI launches
- [ ] Can type in input field
- [ ] Chat history displays

### File System
- [ ] /home mounted correctly
- [ ] Workspaces directory exists
- [ ] Config directory exists
- [ ] Permissions correct

### Boot Configuration
- [ ] systemd-boot entry exists
- [ ] Can select mentalOS from boot menu
- [ ] Boot to desktop < 30 seconds

## Post-Installation Tests

### Daily Use
- [ ] Browser works (Firefox/Chromium)
- [ ] Terminal works (Alacritty)
- [ ] Can clone git repo
- [ ] Can build Rust project
- [ ] WiFi reconnects after reboot
- [ ] Audio persists after reboot

### Security
- [ ] Firejail profile loads
- [ ] Sandbox restricts access
- [ ] sudo requires password

### Performance
- [ ] Boot time < 30 seconds
- [ ] Memory usage < 2GB idle
- [ ] No obvious lag

## Known Issues & Solutions

| Issue | Solution |
|-------|----------|
| Black screen after boot | Check EFI partition mount, reinstall systemd-boot |
| WiFi not working | Enable iwd service: `systemctl enable iwd` |
| Sway won't start | Check logs: `journalctl -xe` |
| No audio | Run `pavucontrol` to check devices |
| Slow boot | Disable unnecessary services |

## Rollback Plan
1. Boot from Arch live USB
2. Mount root partition: `mount /dev/sdb2 /mnt`
3. Remove boot entry: `bootctl --path=/mnt/boot remove`
4. Wipe partitions with gparted
5. Reclaim space for original use

## Sign-Off
- [ ] All tests passed
- [ ] User can perform basic tasks
- [ ] No critical errors
- [ ] Ready for Phase 1.2

**Tested by**: 
**Date**: 
**Notes**:
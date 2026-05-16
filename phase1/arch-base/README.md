n# Phase 1.1 - Minimal Arch Base

This directory bootstraps a minimal Arch Linux install that auto-logs into sway and launches `mentalOS`.

## Deliverables

- `packages.txt`: categorized package manifest with justifications.
- `install-base.sh`: automated installation script for Arch ISO environment.
- `config/`: template configuration files copied into target rootfs.

## Partition Scheme

Target disk layout:

1. EFI system partition: `512MB` (`FAT32`)
2. Root partition: `30GB` (`ext4`)
3. Home partition: remaining space (`ext4`)
4. Swap: optional (prefer swapfile after install if needed)

## Key Config Files

Installed templates include:

- `/etc/hostname`
- `/etc/hosts`
- `/etc/locale.gen`
- `/etc/vconsole.conf`
- `/etc/systemd/system/getty@tty1.service.d/override.conf`
- `/boot/loader/loader.conf`
- `/boot/loader/entries/mentalos.conf`
- `/home/user/.config/sway/config`
- `/home/user/.bash_profile`

## Usage

Run from live Arch ISO after networking is up:

```bash
cd phase1/arch-base
sudo INSTALL_DEV=/dev/nvme0n1 HOSTNAME=mentalOS USERNAME=user ./install-base.sh
```

Dry-run mode (safe preview):

```bash
sudo DRY_RUN=1 FORCE=1 INSTALL_DEV=/dev/sda ./install-base.sh
```

## Testing Checklist

After rebooting into the installed system:

1. Confirm auto-login on `tty1` for the configured user.
2. Confirm sway starts automatically.
3. Confirm `mentalOS` starts on sway session launch.
4. Validate network connectivity (`nmcli device status`, `ping archlinux.org`).
5. Validate services are enabled:
   - `systemctl is-enabled NetworkManager`
   - `systemctl is-enabled iwd`
6. Verify boot loader entry exists:
   - `bootctl list`
7. Launch terminal and run:
   - `firejail --version`
   - `rustc --version`
   - `node --version`
   - `python --version`
8. Confirm GTK runtime assets are present by launching `mentalOS` manually from terminal.

## Notes

- `systemd-boot` is provided by `systemd` via `bootctl`.
- Keep either `intel-ucode` or `amd-ucode` in `packages.txt` for production images based on target CPU fleet.

# Phase 1.2 - Custom ISO Builder (Kickoff)

This profile is an initial `archiso` skeleton for generating a bootable mentalOS image.

## Contents

- `profiledef.sh`: archiso profile metadata/boot modes.
- `packages.x86_64`: runtime package set for live environment.
- `pacman.conf`: package repos/options for build.
- `airootfs/root/customize_airootfs.sh`: live rootfs customization script.
- `build-iso.sh`: one-command build wrapper.
- `test-qemu.sh`: quick local VM boot launcher.
- `MANUAL_BOOT_TESTS.md`: structured post-build validation checklist.
- `VIRTUALBOX_CHECKLIST.md`: secondary VirtualBox validation checklist.

## Build

From repository root:

```bash
cd phase1/iso
./build-iso.sh
```

What it does:

1. Uses existing binary by default (`mentalOS-prototype/target/release/mentalOS`).
2. Copies binary into `airootfs/usr/local/bin/mentalOS`.
3. Stages installer assets from `phase1/arch-base` into the live ISO.
4. Runs `mkarchiso` to produce output under `phase1/iso/out/`.

Live image includes installer assets at `/opt/mentalos/arch-base` and helper command:

- `mentalos-install` (dry-run preview by default)
- `mentalos-install --run` (executes full install script)

Build options:

- `MENTALOS_BINARY=/path/to/mentalOS ./build-iso.sh` to use a custom prebuilt binary.
- `FORCE_REBUILD=1 ./build-iso.sh` to rebuild binary even when one exists.
- `AUTO_INSTALL_TOOLS=0 ./build-iso.sh` to disable auto-install of missing host tools.
- `KEEP_STAGE=1 ./build-iso.sh` to keep staged build artifacts in `airootfs/` after the run.

## Test Checklist (VM)

1. Boot ISO in QEMU (baseline):
   - `./test-qemu.sh`
   - Script auto-installs missing `qemu-desktop` and `edk2-ovmf` on Arch by default.
   - Set `AUTO_INSTALL_TOOLS=0 ./test-qemu.sh` to disable auto-install.
   - Script resets OVMF vars for each run to avoid stale boot-order loops.
2. Verify auto-login user session on tty1.
3. Verify sway starts.
4. Verify `mentalOS` launches automatically.
5. Verify network stack (`nmcli`, `ping`).
6. Verify emergency stop + shortcuts in UI.
7. Optionally validate VirtualBox flow with `VIRTUALBOX_CHECKLIST.md`.

## Notes

- This is a kickoff profile; bootloader themes/syslinux customization can be expanded next.
- `build-iso.sh` auto-installs missing `archiso`/`rsync` packages when possible.

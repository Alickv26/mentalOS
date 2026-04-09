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

## Build

From repository root:

```bash
cd phase1/iso
./build-iso.sh
```

What it does:

1. Builds `mentalOS-prototype` release binary.
2. Copies binary into `airootfs/usr/local/bin/mentalOS`.
3. Runs `mkarchiso` to produce output under `phase1/iso/out/`.

## Test Checklist (VM)

1. Boot ISO in QEMU/VirtualBox.
2. Verify auto-login user session on tty1.
3. Verify sway starts.
4. Verify `mentalOS` launches automatically.
5. Verify network stack (`nmcli`, `ping`).
6. Verify emergency stop + shortcuts in UI.

## Notes

- This is a kickoff profile; bootloader themes/syslinux customization can be expanded next.
- `build-iso.sh` expects `archiso` and `sudo` availability on host.

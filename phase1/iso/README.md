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
- `CLEAN_WORK=0 ./build-iso.sh` to reuse existing mkarchiso work dir (default is clean build each run).
- `WORK_DIR=/custom/path ./build-iso.sh` to override the mkarchiso work directory.

## Test Checklist (VM)

1. Boot ISO in QEMU (baseline):
   - `./test-qemu.sh`
   - Script auto-installs missing `qemu-desktop` and `edk2-ovmf` on Arch by default.
   - Set `AUTO_INSTALL_TOOLS=0 ./test-qemu.sh` to disable auto-install.
   - Script prefers unified `OVMF.fd` when present, otherwise uses split code/vars and resets vars each run.
2. Verify auto-login user session on tty1.
3. Verify sway starts.
4. Verify `mentalOS` launches automatically.
5. Verify network stack (`nmcli`, `ping`).
6. Verify emergency stop + shortcuts in UI.
7. Optionally validate VirtualBox flow with `VIRTUALBOX_CHECKLIST.md`.

## Notes

- This is a kickoff profile; bootloader themes/syslinux customization can be expanded next.
- `build-iso.sh` auto-installs missing `archiso`/`rsync`/`squashfs-tools` packages when possible.

## Build prerequisites

Host packages needed to build:

```bash
sudo pacman -S --needed archiso rsync squashfs-tools base-devel rust pkgconf gtk4
```

- `squashfs-tools` provides `unsquashfs`, which the post-build verification requires.
- `rust`, `pkgconf`, `gtk4` (headers included in the Arch/Manjaro package) are needed
  to compile the mentalOS GTK4 binaries.

## Build-time diagnostics (new)

Every build now leaves evidence behind:

- `out/build.log` — full `mkarchiso` output plus build environment (host OS,
  archiso version, repo commit). Read this first when a build fails or
  misbehaves.
- `out/squashfs-listing.txt` — complete listing of the squashfs contents.
- `work/x86_64/airootfs/` — the work tree mkarchiso actually built from. It
  persists after the build, so you can inspect what really landed.
- Preflight checks run **before** mkarchiso: if any staged or overlay file is
  missing, the build stops immediately with a list of what is absent.

### Verification result meanings

- **`VERIFICATION FAILED: N expected file(s) missing`** — the squashfs listing
  was healthy (reference files like `usr/bin/bash` were present) but mentalOS
  content is genuinely absent. Check `out/build.log` for the lines
  `Copying custom airootfs files`, `Running customize_airootfs.sh` and
  `Creating SquashFS image` — whichever is missing is the step that failed.
- **`VERIFICATION INCONCLUSIVE`** — the verification itself could not read the
  squashfs (missing `unsquashfs`, failed loop mount, unsupported compression,
  or truncated ISO). The ISO may still be fine; fix the tool problem and
  re-check, or boot-test it.

## Manjaro build hosts (branding)

The profile's `pacman.conf` uses the host's mirrorlist. Building on Manjaro
therefore produces a Manjaro-based airootfs: the login banner previously said
"Manjaro Linux". `customize_airootfs.sh` now overrides `/etc/os-release`,
`/etc/issue`, `/etc/hostname` and `/etc/motd` at build time, so the image
identifies as mentalOS regardless of the host. `uname -r` still reports the
Manjaro-built kernel string — that is cosmetic. For a pure-Arch image, build
inside an `archlinux` container.

## Service enablement (robustness)

The four systemd services are enabled through **committed**
`airootfs/etc/systemd/system/*.target.wants/` symlinks (copied into the image
by mkarchiso unconditionally), with `sysusers.d`/`tmpfiles.d` fragments as
boot-time fallbacks for the runtime users and directories. The deprecated
`customize_airootfs.sh` still runs and re-applies the same setup, but the ISO
no longer depends on it succeeding.

`ollama` is installed from the package list (`/usr/bin/ollama`).
`openclaw` is staged only when a built binary is available
(`MENTALOS_OPENCLAW_BINARY=/path/to/openclaw`, or
`openclaw/target/release/openclaw` in the repo root); when absent,
`openclaw.service` is cleanly skipped via `ConditionPathExists`.


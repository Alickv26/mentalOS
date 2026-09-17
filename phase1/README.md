# Phase 1 Workspace

Phase 1 tracks the transition from prototype app to bootable mentalOS system artifacts.

## Milestone 1.1

- Arch base bootstrap: `arch-base/`

## Milestone 1.2 (kickoff)

- Archiso profile skeleton: `iso/`
- Build wrapper: `iso/build-iso.sh`
- VM test runner: `iso/test-qemu.sh`

## Milestone 1.3 (CI + validation)

- Profile structure validator: `iso/validate-profile.sh` (51 checks across profile, packages, bootloaders, shell scripts)
- CI workflow: `.github/workflows/phase1-validate.yml` (shellcheck + profile validation + package audit + config template audit)

## Milestone 1.4 (boot polish)

- Polished systemd-boot loader entry with version + quiet boot options
- Polished syslinux menu with colour theme, layout, and matching kernel options
- Documented bootsplash background PNG hook (drop file + uncomment one line)

## Suggested Next Steps

1. **Add bootsplash PNG asset** — drop a 640x480 16-colour indexed PNG at
   `iso/syslinux/mentalos-bg.png` and uncomment `MENU BACKGROUND` in
   `syslinux/syslinux.cfg`.
2. **Build the ISO on an Arch Linux machine** (the workspace runs Debian and
   can't run `mkarchiso` directly — the `phase1-validate.yml` CI catches
   structure errors, but only `build-iso.sh` on Arch can produce the image).
3. **Run `test-qemu.sh`** on a KVM-capable host to validate boot flow.
4. **Start Milestone 1.5** — persistent overlay + system services (mentalOS.service,
   openclaw.service, ollama.service, workspace-monitor.service).

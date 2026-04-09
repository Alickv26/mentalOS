# Phase 1 Workspace

Phase 1 tracks the transition from prototype app to bootable mentalOS system artifacts.

## Milestone 1.1

- Arch base bootstrap: `arch-base/`

## Milestone 1.2 (kickoff)

- Archiso profile skeleton: `iso/`
- Build wrapper: `iso/build-iso.sh`

## Suggested Next Steps

1. Add `efiboot` and `syslinux` boot assets for polished ISO branding.
2. Add CI job to lint shell scripts and validate archiso profile structure.
3. Add VM smoke boot automation (QEMU) for ISO startup checks.

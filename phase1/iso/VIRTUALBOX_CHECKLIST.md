# VirtualBox Checklist (Secondary Path)

Use this after generating an ISO with `./build-iso.sh`.

## VM Setup

1. Create new Linux VM (Arch/Other Linux 64-bit).
2. Assign at least:
   - 4 vCPU
   - 4GB RAM
   - 32GB disk
3. Enable EFI in VM settings.
4. Mount ISO from `phase1/iso/out/*.iso`.

## Boot Validation

1. Boot VM and wait for live environment startup.
2. Confirm autologin user reaches sway.
3. Confirm `mentalOS` auto-launches.

## Functional Smoke

1. Open terminal (`Mod+Return`) and run:
   - `nmcli device status`
   - `ip a`
2. In mentalOS test:
   - basic prompt
   - memory browser (`Ctrl+Shift+M`)
   - tasks browser (`Ctrl+Shift+J`)
   - sync selection (`Ctrl+Shift+S`)
3. Run installer preview:
   - `mentalos-install`

## Exit Criteria

- No crash at boot or sway startup.
- mentalOS launches and handles prompt flow.
- Installer preview runs and prints dry-run actions.

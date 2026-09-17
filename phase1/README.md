# Phase 1 Workspace

Phase 1 tracks the transition from prototype app to bootable mentalOS system artifacts.

## Milestone 1.1

- Arch base bootstrap: `arch-base/`

## Milestone 1.2 (kickoff)

- Archiso profile skeleton: `iso/`
- Build wrapper: `iso/build-iso.sh`
- VM test runner: `iso/test-qemu.sh`

## Milestone 1.3 (CI + validation)

- Profile structure validator: `iso/validate-profile.sh` (92 checks across profile, packages, bootloaders, shell scripts, and systemd service units)
- CI workflow: `.github/workflows/phase1-validate.yml` (shellcheck + profile validation + package audit + config template audit + service unit audit)

## Milestone 1.4 (boot polish)

- Polished systemd-boot loader entry with version + quiet boot options
- Polished syslinux menu with colour theme, layout, and matching kernel options
- Documented bootsplash background PNG hook (drop file + uncomment one line)

## Milestone 1.5 (system services)

Four systemd service units now live under `iso/airootfs/etc/systemd/system/`
and are enabled by `customize_airootfs.sh` at ISO build time:

| Service | Type | User | Purpose |
|---------|------|------|---------|
| `mentalOS.service` | system | `user` | Main GTK4 app — starts after `graphical.target`, Wants openclaw + ollama |
| `openclaw.service` | system | `user` | OpenClaw gateway HTTP backend (port 18789) |
| `ollama.service` | system | `ollama` | Local Ollama LLM server (port 11434, models in `/var/lib/ollama/models`) |
| `workspace-monitor.service` | system | `user` | File watcher for `~/workspaces` (placeholder bash script; uses `inotifywait` if available, falls back to polling) |

All four services:
- Auto-restart on failure (`Restart=on-failure`, `StartLimitBurst=5`)
- Run with security hardening (NoNewPrivileges, ProtectSystem, ProtectKernelTunables, etc.)
- Have file permissions set in `profiledef.sh`

`customize_airootfs.sh` also pre-creates the `ollama` system user, the `/var/lib/ollama` data directory, and the `~/workspaces/.mentalOS` state directory.

`validate-profile.sh` now runs 92 checks (up from 51) — added a new "Systemd service units" section that verifies each service has `[Unit]`, `[Service]`, `[Install]`, Description, ExecStart, Restart, WantedBy, and that mentalOS.service has the correct dependency chain (`Wants=openclaw.service ollama.service`, `After=graphical.target`, `WantedBy=graphical.target`).

## Suggested Next Steps

1. **Add bootsplash PNG asset** — drop a 640x480 16-colour indexed PNG at
   `iso/syslinux/mentalos-bg.png` and uncomment `MENU BACKGROUND` in
   `syslinux/syslinux.cfg`.
2. **Build the ISO on an Arch Linux machine** (the workspace runs Debian and
   can't run `mkarchiso` directly — the `phase1-validate.yml` CI catches
   structure errors, but only `build-iso.sh` on Arch can produce the image).
3. **Run `test-qemu.sh`** on a KVM-capable host to validate boot flow.
4. **Replace `mentalos-workspace-monitor` with a real Rust binary** that
   forwards inotify events to mentalOS via the backend channel — currently
   it just logs events to `~/workspaces/.mentalOS/workspace-monitor.log`.
5. **Start Milestone 1.6** — persistent overlay (live USB with `cow` or
   `overlay` mode) so changes survive reboot on a live stick.


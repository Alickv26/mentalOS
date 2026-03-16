# Troubleshooting

## App closes when switching agents

Symptoms:

- app exits on agent switch
- logs mention GLib source removal panic

What to check:

- run latest code and rebuild: `cargo run`
- verify you are not on an older binary

If issue persists, include logs with:

- `RUST_BACKTRACE=1 cargo run`

## `json error: missing field session_id`

Cause:

- legacy memory JSON files without `session_id`

Current behavior:

- migration logic is expected to backfill legacy files

If still failing:

1. Back up `~/workspaces/.memory/`
2. Share one failing JSON file in bug report (redact sensitive content)

## `Ctrl+/` or `Ctrl+?` does not open shortcuts help

Supported help triggers:

- `Ctrl+/`
- `Ctrl+?`
- numpad divide
- `F1`

If none work:

1. Open Manage Shortcuts (`Ctrl+,`)
2. Verify `show_help` binding
3. Save and retry

## Manage Shortcuts does not save

Expected behavior:

- status line shows `Saved shortcuts to ...`
- file updates at `~/.config/mentalOS/shortcuts.toml`

If save fails:

1. Check status text in dialog for validation errors
2. Confirm config directory is writable:
   - `ls -ld ~/.config/mentalOS`
3. Share exact status message in bug report

## Terminal button does nothing

The launcher tries common terminal binaries. If all are missing, you get a
system message in chat.

Verify:

- one of `kitty`, `alacritty`, `foot`, `gnome-terminal`, `konsole`, `xterm`
  is installed and in PATH

## OpenClaw unavailable

Check:

- OpenClaw binary exists and is executable
- endpoint/transport settings in `~/.config/mentalOS/config.toml`

Recommended quick test:

- switch provider to `ollama` and confirm local path works

## Validate sandbox behavior

Run:

```bash
./scripts/sandbox_smoke.sh
```

This verifies:

- sandbox environment variable injection (`MENTALOS_SANDBOX=1`)
- `/tmp` host isolation when `private-tmp` is active
- command timeout enforcement

If your environment blocks Firejail namespace creation, tests auto-skip.

## Collecting logs for bug reports

Recommended command:

```bash
MENTALOS_LOG_FORMAT=json RUST_BACKTRACE=1 cargo run 2>&1 | tee mentalos-debug.log
```

Default file logs are also written to:

- `~/.local/state/mentalOS/logs/mentalOS.log`
- rotated backup: `~/.local/state/mentalOS/logs/mentalOS.log.1`

Attach:

- `mentalos-debug.log`
- `~/.config/mentalOS/config.toml` (redacted keys)
- `~/.config/mentalOS/shortcuts.toml` (if shortcut issue)

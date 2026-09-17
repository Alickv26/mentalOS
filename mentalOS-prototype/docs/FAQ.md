# FAQ

## Do I need OpenClaw to use the app?

No. You can run with Ollama-only by setting provider to `ollama` in
`~/.config/mentalOS/config.toml`.

## Which model should I use?

Default is `phi3:mini`. It is lightweight and aligns with current project
defaults.

## Where are conversations stored?

Under `~/workspaces/.memory/`.

## Can I customize keyboard shortcuts?

Yes. Open Manage Shortcuts with `Ctrl+,` and save your bindings.

## Can I reopen onboarding later?

Yes. Use `Ctrl+Shift+T`.

## Why does help shortcut not match my keyboard layout?

Different layouts emit different key symbols. Help accepts multiple triggers:

- `Ctrl+/`
- `Ctrl+?`
- numpad divide
- `F1`

## How do I stop running actions immediately?

Use emergency stop:

- app bar `STOP` button
- `Ctrl+Shift+Q`

## Where are shortcut settings saved?

`~/.config/mentalOS/shortcuts.toml`

## How do I produce logs for debugging?

Run:

```bash
MENTALOS_LOG_FORMAT=json RUST_BACKTRACE=1 cargo run
```

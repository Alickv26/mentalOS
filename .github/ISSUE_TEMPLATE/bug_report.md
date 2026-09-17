---
name: Bug report
about: Report a reproducible bug in mentalOS prototype
title: "[bug] "
labels: bug
assignees: ""
---

## Summary

Describe the bug in one or two sentences.

## Environment

- OS:
- Rust version (`rustc --version`):
- Branch/commit:
- Run mode:
  - [ ] `cargo run`
  - [ ] `MENTALOS_LOG_FORMAT=json cargo run`
- Active provider:
  - [ ] openclaw
  - [ ] ollama

## Steps To Reproduce

1.
2.
3.

## Expected Behavior

What did you expect to happen?

## Actual Behavior

What happened instead?

## Logs

Attach relevant logs. For best results run:

```bash
MENTALOS_LOG_FORMAT=json RUST_BACKTRACE=1 cargo run 2>&1 | tee mentalos-debug.log
```

You can also attach:

- `~/.local/state/mentalOS/logs/mentalOS.log`
- `~/.local/state/mentalOS/logs/mentalOS.log.1` (if present)
- generated bundle: `./scripts/export_logs.sh`

Paste key excerpts:

```text
# paste here
```

## Config Snippets (redact secrets)

- `~/.config/mentalOS/config.toml` relevant sections
- `~/.config/mentalOS/shortcuts.toml` (for shortcut bugs)

## Reproducibility

- [ ] happens every time
- [ ] intermittent
- [ ] happened once

## Extra Context

Screenshots, videos, or notes.

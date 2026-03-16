# mentalOS Prototype (Phase 0)

mentalOS is an AI-first desktop assistant for Linux with a GTK4 front-end and
a Rust backend. This prototype focuses on natural-language workflows, command
safety, memory persistence, and swappable agents (OpenClaw and Ollama).

## Current Status

Implemented in this prototype:

- Routing + execution with whitelist approvals
- OpenClaw and Ollama client support
- Workspace metadata and task tracking
- Conversation memory with migration support
- GTK4 overlay UI with:
  - app bar status and progress bar
  - chat stream and command output rendering
  - transient notifications
  - first-run config wizard
  - keyboard shortcuts help dialog
  - runtime accessibility controls (high-contrast, font size, screen-reader labels)
  - editable shortcut bindings

## Quick Start

1. Install dependencies on Arch (or compatible) with:
   - `./setup.sh`
2. Build and run:
   - `cargo run`
3. Optional JSON logs:
   - `MENTALOS_LOG_FORMAT=json cargo run`

## Logging

- Console format:
  - default text
  - JSON with `MENTALOS_LOG_FORMAT=json`
- File logging:
  - enabled by default
  - default file: `~/.local/state/mentalOS/logs/mentalOS.log`
  - disable with `MENTALOS_LOG_TO_FILE=0`
  - custom path with `MENTALOS_LOG_PATH=/path/to/file.log`
  - max size before rotation (single backup `.1`): `MENTALOS_LOG_MAX_BYTES`
- Debug verbosity:
  - `MENTALOS_DEBUG=1 cargo run`

## Configuration

- Main config:
  - `~/.config/mentalOS/config.toml`
- Example template:
  - `config.example.toml`
- Editable keyboard shortcuts:
  - `~/.config/mentalOS/shortcuts.toml`

On first launch, if config is missing, the app opens a setup wizard before
starting backend message flow. After setup, an onboarding tutorial is shown.

## Keyboard Shortcuts

Defaults (user-editable in Manage Shortcuts):

- `Ctrl+L` focus input
- `Ctrl+K` open app launcher
- `Ctrl+Shift+M` open memory browser
- `Ctrl+Shift+H` toggle high contrast
- `Ctrl+Plus` increase font size
- `Ctrl+Minus` decrease font size
- `Ctrl+0` reset font size
- `Ctrl+Slash` shortcuts help (also accepts `Ctrl+?` / numpad divide / `F1`)
- `Ctrl+Comma` manage shortcuts
- `Ctrl+Shift+T` show onboarding tutorial
- `Ctrl+Shift+Q` emergency stop

## Testing

- Full test suite:
  - `cargo test`
- Fast suite:
  - `cargo test -q`
- Live Ollama integration test:
  - `MENTALOS_LIVE_OLLAMA=1 cargo test live_ollama`
- Live OpenClaw integration test:
  - `MENTALOS_LIVE_OPENCLAW=1 cargo test live_openclaw`
- Sandbox security smoke tests (Firejail):
  - `./scripts/sandbox_smoke.sh`
- GTK accessibility smoke tests:
  - `cargo test -q --test ui_accessibility`
- Coverage report (target: >=80% lines):
  - `./scripts/coverage.sh`
  - `cargo llvm-cov --workspace --all-features --summary-only --fail-under-lines 80`
- Manual performance benchmarks:
  - `./scripts/perf_bench.sh`
  - GitHub Actions (on demand): `../.github/workflows/performance-benchmarks.yml`
- Continuous integration (fmt + tests):
  - `../.github/workflows/ci.yml`

## Documentation

- User guide: `docs/USER_GUIDE.md`
- Troubleshooting: `docs/TROUBLESHOOTING.md`
- FAQ: `docs/FAQ.md`
- Architecture: `docs/ARCHITECTURE.md`
- Performance: `docs/PERFORMANCE.md`

## Reporting Issues

Use `../.github/ISSUE_TEMPLATE/bug_report.md` when filing bugs. It includes
the required runtime/logging details to reproduce UI and agent problems
quickly.

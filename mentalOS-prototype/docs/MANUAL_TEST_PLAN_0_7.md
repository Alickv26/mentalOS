# Manual Test Plan (Phase 0.7)

This checklist is designed for full-feature validation before pushing the next release.
Run the app with `cargo run` unless noted otherwise.

## 1) First-Run + Setup Flow

1. Move existing config away (optional):
   - `mv ~/.config/mentalOS/config.toml ~/.config/mentalOS/config.toml.bak`
2. Launch app.
3. Complete setup wizard with:
   - OpenClaw or Ollama endpoint/model
   - workspace directory
4. Verify onboarding tutorial appears after wizard completion.

Expected:
- No crash or panic.
- UI handshake succeeds and chat is usable.
- `~/.config/mentalOS/config.toml` is created.

## 2) Agent Switching Stability

1. Start with OpenClaw selected.
2. Switch to Ollama.
3. Switch back and forth 10+ times quickly.
4. Send messages on each agent after switching.

Expected:
- No app shutdown/panic.
- Status changes in app bar remain correct.
- Responses continue after each switch.

## 3) New Chat + Session Resume

1. Send 3 messages in current chat.
2. Press `Ctrl+N` (new chat).
3. Send another message.
4. Open memory browser (`Ctrl+Shift+M`) and resume both sessions one by one.

Expected:
- New chat creates a separate session.
- Old chat content is preserved.
- Resume loads selected conversation correctly.

## 4) Memory Browser Search + Deletion

1. Open memory browser.
2. Search by:
   - title fragment
   - category fragment
   - known text from message content
3. Delete one non-active session.
4. Delete active session.

Expected:
- Search filters in real time.
- Non-active deletion does not break active marker.
- Deleting active session clears active marker safely.

## 5) Tasks Workflow

1. Send chat prompts including:
   - `TODO: fix auth bug`
   - `I need to verify deploy by next week priority high`
2. Open tasks browser (`Ctrl+Shift+J`).
3. Mark one task completed, reopen it, then delete it.
4. Restart app and reopen tasks browser.

Expected:
- Tasks are extracted and persisted.
- Status updates persist across restart.
- Duplicate TODOs in one message are deduped.

## 6) Sync Selection + Export

1. Open memory browser -> Sync selection.
2. Apply date filters with exact boundary date (same day from/to).
3. Toggle one session as local-only and another as sync-enabled.
4. Click Apply Selection.
5. Click Export JSON.

Expected:
- Boundary-date sessions are included.
- Local-only sessions are excluded from payload.
- Export file appears under `~/workspaces/<workspace>/.memory/sync-exports/`.
- Exported JSON redacts sensitive values (tokens/API keys).

## 7) Shortcuts + Accessibility Controls

1. Press `Ctrl+/` and `Ctrl+?` and `F1` for help dialog.
2. Press `Ctrl+,` and change at least one shortcut; save.
3. Press `Ctrl+Shift+H` (high contrast).
4. Press `Ctrl+Plus`, `Ctrl+Minus`, `Ctrl+0`.
5. Use new tasks shortcut `Ctrl+Shift+J`.

Expected:
- Help dialog opens for fallback variants.
- Shortcut save persists to `~/.config/mentalOS/shortcuts.toml`.
- High contrast and font size changes apply immediately.

## 8) Project Linking + Related Conversation

1. Ask AI to create a project (or use project creation flow).
2. Confirm project is created under workspace root.
3. Verify system message references related conversation linkage.
4. Reopen app and run project command intents like:
   - `run this project`
   - `test the project`

Expected:
- Project path is linked to conversation metadata/actions.
- Related conversation lookup continues to work after restart.

## 9) Terminal + Launcher

1. Open launcher (`Ctrl+K`) and run an app.
2. Use terminal button from OmniPill.
3. If `TERMINAL` env var is set, verify that terminal is preferred.

Expected:
- Launcher opens and filters properly.
- Terminal command opens a terminal emulator without freezing app.

## 10) Logging + Bug Bundle

1. Run app with JSON logs:
   - `MENTALOS_LOG_FORMAT=json cargo run`
2. Trigger a few actions (agent switch, task extraction, memory open).
3. Export logs bundle:
   - `./scripts/export_logs.sh`

Expected:
- Log file exists at `~/.local/state/mentalOS/logs/mentalOS.log`.
- Export tarball is created and contains logs/config files.

## 11) Performance Smoke

1. Run `./scripts/perf_bench.sh`.
2. Optionally run `cargo test --release -- --ignored`.

Expected:
- Bench tests complete without failures.
- No severe regressions from previous baseline.

## 12) Full Regression Suite

1. Run:
   - `cargo fmt`
   - `cargo clippy --all-targets`
   - `cargo test`

Expected:
- No fmt diffs.
- No clippy errors.
- All tests pass.

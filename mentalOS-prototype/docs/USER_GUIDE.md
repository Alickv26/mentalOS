# mentalOS User Guide

## Start the App

1. Open a terminal in `mentalOS-prototype`.
2. Run `cargo run`.
3. Type into the prompt and press Enter.

Optional structured logs:

- `MENTALOS_LOG_FORMAT=json cargo run`

Default file logs:

- `~/.local/state/mentalOS/logs/mentalOS.log`

## First-Run Wizard

If `~/.config/mentalOS/config.toml` does not exist, a setup wizard opens.

Wizard fields:

- provider (`openclaw` or `ollama`)
- model (default `phi3:mini`)
- workspace directory
- OpenClaw token (optional)
- fallback and privacy toggles

## Daily Workflow

- Ask requests in natural language in the input field.
- The app shows status/progress while backend work is in-flight.
- AI and system messages appear in chat history.
- Chat message headers include role and local time (`Role · HH:MM`).
- Command outputs are rendered in chat.

## Safety and Approvals

- Non-whitelisted commands trigger approval dialogs.
- You can approve once or deny.
- Emergency stop is always available from the app bar and shortcut.

## Agent Switching

- Use the top input-row agent selector to switch between available agents.
- The current active agent is synced from backend state.

## Memory Browser

- Open with `Ctrl+Shift+M`.
- Search matches session title, category, and session id.
- Summary shows filtered count.

## Launcher Search

- Open launcher with `Ctrl+K`.
- Search matches app name, description, and exec command.
- If no match is found, the empty state includes your search text.

## Keyboard Shortcuts

Default bindings (all editable):

- `Ctrl+L`: focus input
- `Ctrl+K`: open app launcher
- `Ctrl+Shift+M`: open memory browser
- `Ctrl+Shift+H`: toggle high contrast
- `Ctrl+Plus`: increase font size
- `Ctrl+Minus`: decrease font size
- `Ctrl+0`: reset font size
- `Ctrl+Slash`: show shortcuts help
- `Ctrl+Comma`: manage shortcuts
- `Ctrl+Shift+T`: show onboarding tutorial
- `Ctrl+Shift+Q`: emergency stop

Help fallback keys for layout differences:

- `Ctrl+?`
- numpad divide
- `F1`

## Onboarding Tutorial

- Automatically shown for first-time setup.
- Reopen anytime with `Ctrl+Shift+T`.
- Step header includes completion percentage.
- Progress is stored in `~/.config/mentalOS/onboarding.toml`.

## Manage Shortcuts

1. Press `Ctrl+Comma`.
2. Edit shortcut strings (for example `Ctrl+Shift+H`).
3. Click Save.
4. Confirm the "Saved shortcuts to ..." message.

Shortcuts are persisted to:

- `~/.config/mentalOS/shortcuts.toml`

## Accessibility

- High contrast mode: `Ctrl+Shift+H`
- Font scaling:
  - increase: `Ctrl+Plus`
  - decrease: `Ctrl+Minus`
  - reset: `Ctrl+0`
- Screen reader metadata:
  - prompt input, agent selector, launcher/terminal/stop buttons
  - chat history region and message log
  - status/progress elements in the app bar

## Data Locations

- config: `~/.config/mentalOS/config.toml`
- shortcut bindings: `~/.config/mentalOS/shortcuts.toml`
- memory store: `~/workspaces/.memory/`
- tasks: `~/workspaces/.memory/tasks.json`

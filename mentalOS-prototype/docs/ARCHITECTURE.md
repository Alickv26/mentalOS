# Architecture Overview

## Runtime Shape

```text
GTK Main Thread
  |
  |-- MainWindow
  |     |-- AppBar / Notifications / ChatView / OmniPill
  |     |-- Dialogs (Approval, Wizard, Memory, Shortcuts)
  |
  |-- glib channel (BackendResponse)
        ^
        |
Backend Thread (Tokio Runtime)
  |
  |-- CommandRouter
  |     |-- OpenClawClient / Ollama path
  |     |-- WhitelistManager
  |     |-- FirejailExecutor
  |     |-- MemoryManager
  |     |-- ProjectHandler
  |     |-- TaskTracker
  |     |-- WorkspaceManager
  |
  `-- OpenClawLauncher (health/start/stop)
```

## Main Message Flow

1. User submits input in `OmniPill`.
2. UI sends `BackendRequest::Input`.
3. Backend routes request, performs command policy checks, and executes actions.
4. Backend sends one or more `BackendResponse` values.
5. UI updates status/progress, chat timeline, approvals, and notifications.

## Safety Model (Current Prototype)

- Commands are parsed and checked through `WhitelistManager`.
- Commands requiring approval are surfaced in modal dialog.
- Emergency stop calls router stop logic and launcher stop logic.

## Persistence

- Config: `~/.config/mentalOS/config.toml`
- Shortcuts: `~/.config/mentalOS/shortcuts.toml`
- Memory: `~/workspaces/.memory/*`
- Tasks: `~/workspaces/.memory/tasks.json`

## Logging

- default: human-readable env_logger
- JSON mode: `MENTALOS_LOG_FORMAT=json`

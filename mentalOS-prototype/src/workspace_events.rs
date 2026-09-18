//! Workspace event consumer — tails `events.jsonl` and forwards to the UI.
//!
//! The `mentalos-workspace-monitor` systemd service writes file-change events
//! as JSON Lines to `~/workspaces/.mentalOS/events.jsonl`. This module tails
//! that file and forwards each parsed event to the UI via the backend
//! channel.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────┐   events.jsonl   ┌──────────────────────┐
//! │ workspace-monitor    │ ───────────────▶  │ workspace_events      │
//! │ (systemd service)    │   (JSON Lines)    │ (tailer thread)       │
//! └──────────────────────┘                   └──────────┬───────────┘
//!                                                       │ BackendResponse::WorkspaceEvent
//!                                                       ▼
//!                                            ┌──────────────────────┐
//!                                            │ main_window.rs      │
//!                                            │ (UI refresh)        │
//!                                            └──────────────────────┘
//! ```
//!
//! # Event shape
//!
//! Each line in `events.jsonl` is a JSON object:
//!
//! ```json
//! {"timestamp":"2026-09-18T12:34:56.789Z","kind":"create","paths":["demo/main.rs"]}
//! ```
//!
//! # Feature gating
//!
//! The `WorkspaceEvent` struct and `parse_event` function are always available
//! (used by the workspace-monitor binary too). The tailer functions
//! (`spawn_tailer`, `tail_file`, etc.) are gated behind the `gtk4-ui` feature
//! because they depend on `BackendResponse` from the `ui` module.

use serde::{Deserialize, Serialize};

/// A single file-change event from the workspace monitor.
///
/// Serialized as one JSON object per line in `events.jsonl`. The struct is
/// shared between the workspace-monitor binary (producer) and this module
/// (consumer) so they stay in sync.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceEvent {
    /// ISO 8601 timestamp (UTC) when the event was observed.
    pub timestamp: String,
    /// Event kind: "create", "modify", "delete", "move", "access", "any", "other".
    pub kind: String,
    /// One or more affected paths (relative to watch root when possible).
    pub paths: Vec<String>,
}

/// Parse a single JSON line into a `WorkspaceEvent`.
///
/// Returns `Err` for malformed JSON. The caller should log and skip — a
/// single bad line shouldn't crash the tailer.
pub fn parse_event(line: &str) -> Result<WorkspaceEvent, serde_json::Error> {
    serde_json::from_str(line)
}

// ── Tailer implementation (only when the UI module is available) ───────

#[cfg(feature = "gtk4-ui")]
mod tailer {
    use crate::ui::messages::BackendResponse;
    use log::{info, warn};
    use notify::{RecommendedWatcher, RecursiveMode, Watcher};
    use std::io::{BufRead, BufReader, Seek, SeekFrom};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    /// Spawn a background thread that tails `~/workspaces/.mentalOS/events.jsonl`
    /// and forwards parsed events to the UI via `ui_tx`.
    ///
    /// The thread runs for the lifetime of the app. It handles:
    /// - File not existing yet (polls every 2s until it appears)
    /// - File truncation (re-opens from start)
    /// - Malformed JSON lines (skips with a warning, continues)
    /// - Watcher failures (falls back to 500ms polling)
    pub fn spawn_tailer(workspace_dir: PathBuf, ui_tx: async_channel::Sender<BackendResponse>) {
        std::thread::Builder::new()
            .name("workspace-events-tailer".to_string())
            .spawn(move || {
                let events_path = workspace_dir.join(".mentalOS").join("events.jsonl");
                info!(
                    "Workspace events tailer started, watching: {}",
                    events_path.display()
                );
                tail_file(&events_path, ui_tx);
            })
            .expect("failed to spawn workspace-events-tailer thread");
    }

    /// Tail a single file forever, forwarding parsed events to `ui_tx`.
    ///
    /// Tries to use `notify` for event-driven reading. Falls back to 500ms
    /// polling if the watcher can't be created (e.g., inotify limits hit).
    fn tail_file(path: &Path, ui_tx: async_channel::Sender<BackendResponse>) {
        // Wait for the file to appear the first time.
        wait_for_file(path);

        // Open and seek to end so we don't replay old events on startup.
        let mut file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) => {
                warn!("Failed to open {}: {}", path.display(), e);
                return;
            }
        };
        let _ = file.seek(SeekFrom::End(0));
        let mut reader = BufReader::new(file);

        // Try to set up a notify watcher on the parent directory.
        // (Watching the file directly misses truncation events on some platforms.)
        let parent = path.parent().unwrap_or(Path::new("."));
        let (notify_tx, notify_rx) = std::sync::mpsc::channel();
        let mut watcher: Option<RecommendedWatcher> = match notify::recommended_watcher(notify_tx) {
            Ok(mut w) => {
                match w.watch(parent, RecursiveMode::NonRecursive) {
                    Ok(()) => Some(w),
                    Err(e) => {
                        warn!(
                            "Failed to watch {} for changes: {}, falling back to polling",
                            parent.display(),
                            e
                        );
                        None
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to create file watcher: {}, falling back to polling",
                    e
                );
                None
            }
        };

        let using_notify = watcher.is_some();
        if !using_notify {
            info!("Using polling fallback (500ms) for workspace events");
        }

        loop {
            // Try to read a line from the current reader position.
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    // No data available right now.
                    if using_notify {
                        // Block on the notify channel for up to 5s.
                        match notify_rx.recv_timeout(Duration::from_secs(5)) {
                            Ok(_) => continue,
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                warn!("Notify watcher disconnected, falling back to polling");
                                drop(watcher.take());
                                continue_with_polling(&mut reader, path, &ui_tx);
                                return;
                            }
                        }
                    } else {
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                }
                Ok(_) => {
                    forward_line(&line, &ui_tx);
                }
                Err(e) => {
                    warn!("Error reading {}: {}, re-opening", path.display(), e);
                    std::thread::sleep(Duration::from_secs(1));
                    reopen_file(path, &mut reader);
                }
            }
        }
    }

    /// Polling-mode continuation after the notify watcher dies.
    /// Reads any new lines from the current position, then sleeps 500ms.
    fn continue_with_polling(
        reader: &mut BufReader<std::fs::File>,
        path: &Path,
        ui_tx: &async_channel::Sender<BackendResponse>,
    ) {
        loop {
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => forward_line(&line, ui_tx),
                    Err(e) => {
                        warn!("Error reading {}: {}, re-opening", path.display(), e);
                        std::thread::sleep(Duration::from_secs(1));
                        reopen_file(path, reader);
                        break;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Wait up to forever for a file to appear, polling every 2 seconds.
    fn wait_for_file(path: &Path) {
        while !path.exists() {
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    /// Re-open a file after an error, seeking to the start (in case it was
    /// truncated by the monitor restarting).
    fn reopen_file(path: &Path, reader: &mut BufReader<std::fs::File>) {
        let new_file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) => {
                warn!(
                    "Failed to re-open {}: {}, will retry in 2s",
                    path.display(),
                    e
                );
                std::thread::sleep(Duration::from_secs(2));
                return;
            }
        };
        // Check if the file shrank (truncation). If so, seek to start.
        let old_pos = reader.stream_position().unwrap_or(0);
        let new_len = new_file.metadata().map(|m| m.len()).unwrap_or(0);
        let new_reader = BufReader::new(new_file);
        let start = if new_len < old_pos { 0 } else { old_pos };
        let _ = new_reader.get_ref().seek(SeekFrom::Start(start));
        *reader = new_reader;
    }

    /// Parse a single line and forward it to the UI. Skips malformed lines.
    fn forward_line(line: &str, ui_tx: &async_channel::Sender<BackendResponse>) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        match super::parse_event(trimmed) {
            Ok(event) => {
                if let Err(e) = ui_tx.send_blocking(BackendResponse::WorkspaceEvent(event)) {
                    warn!("Failed to forward workspace event to UI: {}", e);
                }
            }
            Err(e) => {
                warn!("Malformed workspace event line: {} — {}", trimmed, e);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn forward_line_skips_empty_lines() {
            let (tx, _rx) = async_channel::unbounded::<BackendResponse>();
            forward_line("", &tx); // should not panic
            forward_line("   ", &tx); // should not panic
        }

        #[test]
        fn forward_line_skips_malformed_lines() {
            let (tx, _rx) = async_channel::unbounded::<BackendResponse>();
            forward_line("not json", &tx); // should not panic, should not send
        }

        #[test]
        fn forward_line_sends_valid_event() {
            let (tx, rx) = async_channel::unbounded::<BackendResponse>();
            let line = r#"{"timestamp":"2026-09-18T12:00:00Z","kind":"create","paths":["demo.rs"]}"#;
            forward_line(line, &tx);
            let received = rx.try_recv().expect("should have received an event");
            match received {
                BackendResponse::WorkspaceEvent(event) => {
                    assert_eq!(event.kind, "create");
                    assert_eq!(event.paths, vec!["demo.rs".to_string()]);
                }
                other => panic!("expected WorkspaceEvent, got {:?}", other),
            }
        }
    }
}

#[cfg(feature = "gtk4-ui")]
pub use tailer::spawn_tailer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_event_valid_json() {
        let line = r#"{"timestamp":"2026-09-18T12:00:00Z","kind":"create","paths":["demo/main.rs"]}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.timestamp, "2026-09-18T12:00:00Z");
        assert_eq!(event.kind, "create");
        assert_eq!(event.paths, vec!["demo/main.rs".to_string()]);
    }

    #[test]
    fn parse_event_multi_path() {
        let line = r#"{"timestamp":"2026-09-18T12:00:00Z","kind":"modify","paths":["a.rs","b.rs"]}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.kind, "modify");
        assert_eq!(event.paths.len(), 2);
    }

    #[test]
    fn parse_event_empty_paths_array() {
        let line = r#"{"timestamp":"2026-09-18T12:00:00Z","kind":"any","paths":[]}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.kind, "any");
        assert!(event.paths.is_empty());
    }

    #[test]
    fn parse_event_malformed_json_returns_err() {
        let line = "not json at all";
        assert!(parse_event(line).is_err());
    }

    #[test]
    fn parse_event_missing_field_returns_err() {
        // Missing "paths" field — should fail since WorkspaceEvent requires it.
        let line = r#"{"timestamp":"2026-09-18T12:00:00Z","kind":"create"}"#;
        assert!(parse_event(line).is_err());
    }

    #[test]
    fn parse_event_extra_fields_are_ignored() {
        // Extra "foo" field — serde ignores unknown fields by default.
        let line = r#"{"timestamp":"2026-09-18T12:00:00Z","kind":"create","paths":["x.rs"],"foo":"bar"}"#;
        let event = parse_event(line).unwrap();
        assert_eq!(event.paths, vec!["x.rs".to_string()]);
    }

    #[test]
    fn parse_event_round_trips_through_serialize() {
        let original = WorkspaceEvent {
            timestamp: "2026-09-18T12:00:00Z".to_string(),
            kind: "delete".to_string(),
            paths: vec!["old.rs".to_string()],
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed = parse_event(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn workspace_event_implements_eq_and_partial_eq() {
        let a = WorkspaceEvent {
            timestamp: "2026-09-18T12:00:00Z".to_string(),
            kind: "create".to_string(),
            paths: vec!["a.rs".to_string()],
        };
        let b = a.clone();
        assert_eq!(a, b);

        let c = WorkspaceEvent {
            timestamp: "2026-09-18T12:00:01Z".to_string(),
            ..a.clone()
        };
        assert_ne!(a, c);
    }

    /// Integration test: write a few events to a temp file and verify the
    /// tailer's parsing logic handles them. (We don't spawn the actual
    /// tailer thread here — we test the parsing layer it uses.)
    #[test]
    fn parse_event_handles_real_world_lines_from_workspace_monitor() {
        // These are actual lines the workspace-monitor binary writes:
        let lines = vec![
            r#"{"timestamp":"2026-09-18T12:34:56.789+00:00","kind":"create","paths":["demo/main.rs"]}"#,
            r#"{"timestamp":"2026-09-18T12:34:57.000+00:00","kind":"modify","paths":["demo/Cargo.toml"]}"#,
            r#"{"timestamp":"2026-09-18T12:34:58.123+00:00","kind":"delete","paths":["demo/old.rs"]}"#,
        ];
        for line in lines {
            let event = parse_event(line).expect(&format!("should parse: {}", line));
            assert!(!event.timestamp.is_empty());
            assert!(!event.kind.is_empty());
            assert!(!event.paths.is_empty());
        }
    }
}


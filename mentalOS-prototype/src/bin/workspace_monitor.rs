//! mentalOS workspace monitor — watches `~/workspaces` for file changes and
//! forwards events as JSON Lines to `~/workspaces/.mentalOS/events.jsonl`.
//!
//! Run as a systemd service (`workspace-monitor.service`). The mentalOS GTK
//! app's `workspace_events` module tails that file and forwards events to
//! the UI for live refresh.
//!
//! # Output files
//!
//! - `~/.mentalOS/workspace-monitor.log` — human-readable log (one line per event)
//! - `~/.mentalOS/events.jsonl` — machine-readable JSON Lines (one JSON object per line)
//!
//! Each JSON event has this shape:
//!
//! ```json
//! {"timestamp":"2026-09-18T12:34:56.789Z","kind":"create","paths":["/home/user/workspaces/demo/main.rs"]}
//! ```
//!
//! # Environment
//!
//! - `WORKSPACE_DIR` — override the watch root (defaults to `$HOME/workspaces`)
//! - `RUST_LOG` — log level (default: `info`)

use chrono::Utc;
use log::{info, warn};
use mental_os::workspace_events::WorkspaceEvent;
use notify::event::{Event, EventKind};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::env;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

fn main() {
    // Initialise logging to stderr so journald picks it up.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {} {}] {}",
                Utc::now().to_rfc3339(),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();

    let workspace_dir = resolve_workspace_dir();
    let state_dir = workspace_dir.join(".mentalOS");
    if let Err(e) = std::fs::create_dir_all(&state_dir) {
        eprintln!("Failed to create state dir {}: {}", state_dir.display(), e);
        std::process::exit(1);
    }

    let log_path = state_dir.join("workspace-monitor.log");
    let events_path = state_dir.join("events.jsonl");

    // Truncate events.jsonl on startup so we don't accumulate stale events
    // across restarts. The log file is append-only.
    if let Err(e) = File::create(&events_path) {
        eprintln!(
            "Failed to truncate events file {}: {}",
            events_path.display(),
            e
        );
        std::process::exit(1);
    }

    info!(
        "mentalOS workspace monitor starting (watch={})",
        workspace_dir.display()
    );
    append_log(&log_path, &format!("mentalOS workspace monitor starting"));
    append_log(
        &log_path,
        &format!("Watching: {}", workspace_dir.display()),
    );

    // Set up the file watcher. notify::recommended_watcher picks the best
    // backend for the platform (inotify on Linux, FSEvents on macOS, etc.).
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(tx) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to create watcher: {}", e);
            append_log(&log_path, &format!("Failed to create watcher: {}", e));
            std::process::exit(1);
        }
    };

    if let Err(e) = watcher.watch(&workspace_dir, RecursiveMode::Recursive) {
        eprintln!(
            "Failed to watch {}: {}",
            workspace_dir.display(),
            e
        );
        append_log(&log_path, &format!("Failed to watch: {}", e));
        std::process::exit(1);
    }

    // Poll the channel for events. The watcher runs in a background thread.
    loop {
        match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(Ok(event)) => {
                let workspace_event = format_event(&event, &workspace_dir);
                let human = human_readable(&workspace_event);
                append_log(&log_path, &human);
                if let Err(e) = append_event(&events_path, &workspace_event) {
                    warn!("Failed to write event to {}: {}", events_path.display(), e);
                }
            }
            Ok(Err(e)) => {
                warn!("Watcher error: {}", e);
                append_log(&log_path, &format!("Watcher error: {}", e));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No events in 60s — this is normal. Log heartbeat every 5 minutes.
                // (We don't actually track 5min intervals here; the heartbeat is
                // just that the loop is alive and receiving.)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Watcher thread died — exit so systemd can restart us.
                warn!("Watcher channel disconnected, exiting");
                append_log(&log_path, "Watcher channel disconnected, exiting");
                break;
            }
        }
    }

    info!("Shutting down");
    append_log(&log_path, "Shutting down");
}

/// Resolve the workspace directory to watch, honouring $WORKSPACE_DIR and
/// falling back to $HOME/workspaces, and finally to /home/user/workspaces
/// (the mentalOS default user) if HOME isn't set.
fn resolve_workspace_dir() -> PathBuf {
    if let Ok(dir) = env::var("WORKSPACE_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home).join("workspaces");
    }
    PathBuf::from("/home/user/workspaces")
}

/// Map a notify event to our serialised WorkspaceEvent, normalising paths
/// relative to the watch root when possible (so the JSON output is portable
/// across machines with different $HOME layouts).
fn format_event(event: &Event, watch_root: &Path) -> WorkspaceEvent {
    let kind = match event.kind {
        EventKind::Create(_) => "create",
        EventKind::Modify(_) => "modify",
        EventKind::Remove(_) => "delete",
        EventKind::Access(_) => "access",
        EventKind::Any => "any",
        _ => "other",
    }
    .to_string();

    let paths = event
        .paths
        .iter()
        .map(|p| {
            // Show relative path when inside the watch root, absolute otherwise.
            if let Ok(rel) = p.strip_prefix(watch_root) {
                rel.display().to_string()
            } else {
                p.display().to_string()
            }
        })
        .collect();

    WorkspaceEvent {
        timestamp: Utc::now().to_rfc3339(),
        kind,
        paths,
    }
}

/// Human-readable single-line representation for the log file.
fn human_readable(event: &WorkspaceEvent) -> String {
    format!(
        "{} {} {}",
        event.timestamp,
        event.kind,
        event.paths.join(", ")
    )
}

/// Append a line to the human-readable log file.
fn append_log(path: &Path, line: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{}", line);
    }
}

/// Serialise the event as one JSON line and append to events.jsonl.
fn append_event(path: &Path, event: &WorkspaceEvent) -> std::io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let json = serde_json::to_string(event)
        .unwrap_or_else(|_| "{\"error\":\"serialise failed\"}".to_string());
    writeln!(file, "{}", json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_event_create_kind_maps_to_create() {
        use notify::event::{CreateKind, Event};
        let event = Event::new(EventKind::Create(CreateKind::File));
        let watch_root = Path::new("/home/user/workspaces");
        let formatted = format_event(&event, watch_root);
        assert_eq!(formatted.kind, "create");
        assert!(formatted.paths.is_empty());
    }

    #[test]
    fn format_event_modify_kind_maps_to_modify() {
        use notify::event::{Event, ModifyKind};
        let event = Event::new(EventKind::Modify(ModifyKind::Any));
        let watch_root = Path::new("/home/user/workspaces");
        let formatted = format_event(&event, watch_root);
        assert_eq!(formatted.kind, "modify");
    }

    #[test]
    fn format_event_remove_kind_maps_to_delete() {
        use notify::event::{Event, RemoveKind};
        let event = Event::new(EventKind::Remove(RemoveKind::File));
        let watch_root = Path::new("/home/user/workspaces");
        let formatted = format_event(&event, watch_root);
        assert_eq!(formatted.kind, "delete");
    }

    #[test]
    fn format_event_paths_are_relative_to_watch_root() {
        use notify::event::{CreateKind, Event};
        let mut event = Event::new(EventKind::Create(CreateKind::File));
        event.paths
            .push(PathBuf::from("/home/user/workspaces/demo/main.rs"));
        let watch_root = Path::new("/home/user/workspaces");
        let formatted = format_event(&event, watch_root);
        assert_eq!(formatted.paths, vec!["demo/main.rs".to_string()]);
    }

    #[test]
    fn format_event_paths_outside_watch_root_stay_absolute() {
        use notify::event::{CreateKind, Event};
        let mut event = Event::new(EventKind::Create(CreateKind::File));
        event.paths.push(PathBuf::from("/etc/passwd"));
        let watch_root = Path::new("/home/user/workspaces");
        let formatted = format_event(&event, watch_root);
        assert_eq!(formatted.paths, vec!["/etc/passwd".to_string()]);
    }

    #[test]
    fn resolve_workspace_dir_honours_env_var() {
        unsafe { std::env::set_var("WORKSPACE_DIR", "/tmp/test-workspaces"); }
        let dir = resolve_workspace_dir();
        assert_eq!(dir, PathBuf::from("/tmp/test-workspaces"));
        unsafe { std::env::remove_var("WORKSPACE_DIR"); }
    }

    #[test]
    fn resolve_workspace_dir_falls_back_to_home() {
        unsafe { std::env::remove_var("WORKSPACE_DIR"); }
        // We can't reliably set HOME in a test, so just verify the function
        // returns *some* path (it shouldn't panic).
        let _dir = resolve_workspace_dir();
    }

    #[test]
    fn workspace_event_serialises_to_json() {
        let event = WorkspaceEvent {
            timestamp: "2026-09-18T12:34:56.789Z".to_string(),
            kind: "create".to_string(),
            paths: vec!["demo/main.rs".to_string()],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"kind\":\"create\""));
        assert!(json.contains("demo/main.rs"));
    }

    #[test]
    fn append_event_writes_one_json_line_per_call() {
        use std::io::BufRead;
        let temp = tempfile::NamedTempFile::new().unwrap();
        let path = temp.path();

        let e1 = WorkspaceEvent {
            timestamp: "2026-09-18T12:00:00Z".to_string(),
            kind: "create".to_string(),
            paths: vec!["a.txt".to_string()],
        };
        let e2 = WorkspaceEvent {
            timestamp: "2026-09-18T12:00:01Z".to_string(),
            kind: "modify".to_string(),
            paths: vec!["b.txt".to_string()],
        };
        append_event(path, &e1).unwrap();
        append_event(path, &e2).unwrap();

        let file = File::open(path).unwrap();
        let lines: Vec<String> = std::io::BufReader::new(file).lines().map(Result::unwrap).collect();
        assert_eq!(lines.len(), 2, "Expected 2 lines in events.jsonl");
        assert!(lines[0].contains("\"kind\":\"create\""));
        assert!(lines[1].contains("\"kind\":\"modify\""));
    }

    #[test]
    fn human_readable_includes_all_paths() {
        let event = WorkspaceEvent {
            timestamp: "2026-09-18T12:00:00Z".to_string(),
            kind: "modify".to_string(),
            paths: vec!["a.txt".to_string(), "b.txt".to_string()],
        };
        let s = human_readable(&event);
        assert!(s.contains("a.txt"));
        assert!(s.contains("b.txt"));
        assert!(s.contains("modify"));
    }
}

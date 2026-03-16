use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Box, Button, Entry, Label, Orientation, PolicyType, ScrolledWindow, Window};
use log::info;
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::rc::Rc;

/// Represents a parsed .desktop application entry.
#[derive(Debug, Clone)]
struct DesktopEntry {
    name: String,
    exec: String,
    comment: String,
}

/// A simple popup window listing installed desktop applications.
pub struct AppLauncher;

impl AppLauncher {
    /// Open the app launcher as a popup window.
    pub fn show(parent: &impl IsA<gtk4::Window>) {
        let window = Window::builder()
            .title("All Apps")
            .modal(true)
            .transient_for(parent)
            .default_width(420)
            .default_height(520)
            .build();
        window.add_css_class("launcher-window");

        let vbox = Box::new(Orientation::Vertical, 0);

        // ── Search entry ──
        let search = Entry::builder()
            .placeholder_text("Search applications...")
            .build();
        search.add_css_class("launcher-search");
        search.set_margin_top(8);
        search.set_margin_start(8);
        search.set_margin_end(8);
        search.set_margin_bottom(4);
        vbox.append(&search);

        // ── App list ──
        let list_box = Box::new(Orientation::Vertical, 0);
        list_box.add_css_class("launcher-list");

        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .child(&list_box)
            .build();
        vbox.append(&scroll);

        let hint = Label::new(Some(&launcher_hint_text()));
        hint.set_halign(gtk4::Align::Start);
        hint.add_css_class("app-bar-stats");
        hint.set_margin_start(8);
        hint.set_margin_end(8);
        hint.set_margin_top(6);
        hint.set_margin_bottom(8);
        vbox.append(&hint);

        window.set_child(Some(&vbox));

        // ── Load desktop entries ──
        let entries = load_desktop_entries();
        let entries = Rc::new(entries);

        // Populate initial list
        populate_list(&list_box, &entries, "");

        // ── Search filtering ──
        let entries_ref = entries.clone();
        let list_ref = list_box.clone();
        search.connect_changed(move |entry| {
            let query = entry.text().to_string();
            populate_list(&list_ref, &entries_ref, &query);
        });

        // Esc to close
        let key_ctrl = gtk4::EventControllerKey::new();
        let w = window.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                w.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(key_ctrl);

        window.present();
        search.grab_focus();
    }
}

pub fn launch_terminal() -> Result<(), String> {
    if let Ok(env_terminal) = std::env::var("TERMINAL") {
        if try_spawn_terminal_command(&env_terminal)? {
            return Ok(());
        }
    }

    let candidates: [(&str, &[&str]); 8] = [
        ("foot", &[]),
        ("alacritty", &[]),
        ("kitty", &[]),
        ("wezterm", &["start"]),
        ("gnome-terminal", &[]),
        ("konsole", &[]),
        ("xfce4-terminal", &[]),
        ("xterm", &[]),
    ];

    for (program, args) in candidates {
        match std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {
                info!("Launched terminal: {}", program);
                return Ok(());
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(format!("Failed launching terminal '{}' : {}", program, err));
            }
        }
    }

    Err("No terminal emulator found. Set $TERMINAL or install foot/alacritty/kitty.".to_string())
}

/// Populate (or re-populate) the list box with entries matching the query.
fn populate_list(list_box: &Box, entries: &[DesktopEntry], query: &str) {
    // Clear existing children
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let query_lower = query.to_lowercase();
    for entry in entries {
        if !query.is_empty() && !matches_entry_query(entry, &query_lower) {
            continue;
        }

        let btn = Button::new();
        btn.add_css_class("launcher-item");

        let row = Box::new(Orientation::Vertical, 2);
        let name_label = Label::new(Some(&entry.name));
        name_label.set_halign(gtk4::Align::Start);
        name_label.add_css_class("message-content");
        row.append(&name_label);

        if !entry.comment.is_empty() {
            let comment_label = Label::new(Some(&entry.comment));
            comment_label.set_halign(gtk4::Align::Start);
            comment_label.add_css_class("app-bar-stats");
            row.append(&comment_label);
        }

        btn.set_child(Some(&row));

        let exec = entry.exec.clone();
        btn.connect_clicked(move |_| {
            launch_app(&exec);
        });

        list_box.append(&btn);
    }

    if list_box.first_child().is_none() {
        let message = if query.trim().is_empty() {
            "No applications found.".to_string()
        } else {
            format!("No applications found for '{query}'.")
        };
        let empty = Label::new(Some(&message));
        empty.add_css_class("welcome-hint");
        empty.set_margin_top(20);
        list_box.append(&empty);
    }
}

/// Launch an application by its Exec string.
fn launch_app(exec: &str) {
    // Strip field codes (%f, %F, %u, %U, etc.)
    let clean = clean_exec_command(exec);

    info!("Launching app: {clean}");

    let parts: Vec<&str> = clean.split_whitespace().collect();
    if let Some((program, args)) = parts.split_first() {
        match std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => info!("Launched {program}"),
            Err(err) => log::warn!("Failed to launch {program}: {err}"),
        }
    }
}

fn clean_exec_command(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|token| !token.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ")
}

fn launcher_hint_text() -> String {
    "Enter to launch • Esc to close".to_string()
}

fn matches_entry_query(entry: &DesktopEntry, query_lower: &str) -> bool {
    entry.name.to_lowercase().contains(query_lower)
        || entry.comment.to_lowercase().contains(query_lower)
        || entry.exec.to_lowercase().contains(query_lower)
}

/// Read .desktop files from standard directories and return sorted entries.
fn load_desktop_entries() -> Vec<DesktopEntry> {
    let mut entries: BTreeMap<String, DesktopEntry> = BTreeMap::new();

    let dirs = ["/usr/share/applications", "/usr/local/share/applications"];

    for dir in &dirs {
        let path = Path::new(dir);
        if !path.is_dir() {
            continue;
        }

        if let Ok(read_dir) = fs::read_dir(path) {
            for entry in read_dir.flatten() {
                let file_path = entry.path();
                if file_path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }
                if let Some(desktop) = parse_desktop_file(&file_path) {
                    entries.entry(desktop.name.clone()).or_insert(desktop);
                }
            }
        }
    }

    entries.into_values().collect()
}

/// Minimal .desktop file parser — extracts Name, Exec, Comment, and skips
/// entries with NoDisplay=true.
fn parse_desktop_file(path: &Path) -> Option<DesktopEntry> {
    let content = fs::read_to_string(path).ok()?;

    let mut name = String::new();
    let mut exec = String::new();
    let mut comment = String::new();
    let mut no_display = false;
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }
        if line.starts_with('[') {
            // New section — stop parsing
            if in_desktop_entry {
                break;
            }
            continue;
        }
        if !in_desktop_entry {
            continue;
        }

        if let Some(val) = line.strip_prefix("Name=") {
            if name.is_empty() {
                name = val.to_string();
            }
        } else if let Some(val) = line.strip_prefix("Exec=") {
            exec = val.to_string();
        } else if let Some(val) = line.strip_prefix("Comment=") {
            if comment.is_empty() {
                comment = val.to_string();
            }
        } else if line.starts_with("NoDisplay=true") {
            no_display = true;
        }
    }

    if name.is_empty() || exec.is_empty() || no_display {
        return None;
    }

    Some(DesktopEntry {
        name,
        exec,
        comment,
    })
}

fn try_spawn_terminal_command(command: &str) -> Result<bool, String> {
    let parts = shell_words::split(command)
        .map_err(|err| format!("Invalid $TERMINAL value '{}': {}", command, err))?;
    let (program, args) = match parts.split_first() {
        Some((program, args)) => (program, args),
        None => return Ok(false),
    };

    match std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => {
            info!("Launched terminal from $TERMINAL: {}", command);
            Ok(true)
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(format!("Failed launching $TERMINAL '{}': {}", command, err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_exec_command_strips_desktop_field_tokens() {
        let cleaned = clean_exec_command("code %F --reuse-window %u");
        assert_eq!(cleaned, "code --reuse-window");
    }

    #[test]
    fn query_matches_name_comment_and_exec() {
        let entry = DesktopEntry {
            name: "Firefox".to_string(),
            exec: "firefox %u".to_string(),
            comment: "Web Browser".to_string(),
        };
        assert!(matches_entry_query(&entry, "fire"));
        assert!(matches_entry_query(&entry, "browser"));
        assert!(matches_entry_query(&entry, "firefox"));
        assert!(!matches_entry_query(&entry, "terminal"));
    }

    #[test]
    fn launcher_hint_is_stable() {
        assert_eq!(launcher_hint_text(), "Enter to launch • Esc to close");
    }
}

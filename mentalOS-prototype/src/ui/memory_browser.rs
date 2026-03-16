use crate::memory::MemoryManager;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, PolicyType, ScrolledWindow, Window};
use std::path::PathBuf;

pub struct MemoryBrowser;

impl MemoryBrowser {
    pub fn show(parent: &impl IsA<gtk4::Window>, workspace: &str) {
        let window = Window::builder()
            .title("Conversation Memory")
            .modal(true)
            .transient_for(parent)
            .default_width(560)
            .default_height(420)
            .build();
        window.add_css_class("launcher-window");

        let root = Box::new(Orientation::Vertical, 8);
        root.set_margin_top(10);
        root.set_margin_bottom(10);
        root.set_margin_start(10);
        root.set_margin_end(10);

        let title = Label::new(Some("Recent sessions"));
        title.set_halign(gtk4::Align::Start);
        title.add_css_class("app-bar-title");
        root.append(&title);

        let list_box = Box::new(Orientation::Vertical, 6);

        let manager = MemoryManager::new(default_workspace_root());
        match manager.list_sessions(workspace) {
            Ok(sessions) if !sessions.is_empty() => {
                for session in sessions.into_iter().take(30) {
                    let row = Box::new(Orientation::Vertical, 2);
                    row.add_css_class("message-row");
                    row.add_css_class("ai-message");

                    let title_text = session
                        .title
                        .unwrap_or_else(|| format!("Session {}", session.session_id));
                    let head = Label::new(Some(&title_text));
                    head.set_halign(gtk4::Align::Start);
                    head.set_wrap(true);
                    head.add_css_class("message-content");
                    row.append(&head);

                    let details = format!(
                        "category: {}   created: {}   last active: {}",
                        session.category,
                        session.created_at.format("%Y-%m-%d %H:%M"),
                        session.last_active.format("%Y-%m-%d %H:%M")
                    );
                    let meta = Label::new(Some(&details));
                    meta.set_halign(gtk4::Align::Start);
                    meta.add_css_class("app-bar-stats");
                    row.append(&meta);

                    list_box.append(&row);
                }
            }
            Ok(_) => {
                let empty = Label::new(Some("No conversations saved yet."));
                empty.add_css_class("welcome-hint");
                empty.set_halign(gtk4::Align::Start);
                list_box.append(&empty);
            }
            Err(err) => {
                let error = Label::new(Some(&format!("Failed to load memory: {}", err)));
                error.add_css_class("welcome-hint");
                error.set_halign(gtk4::Align::Start);
                list_box.append(&error);
            }
        }

        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .child(&list_box)
            .build();
        root.append(&scroll);

        window.set_child(Some(&root));
        window.present();
    }
}

fn default_workspace_root() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("workspaces")
}

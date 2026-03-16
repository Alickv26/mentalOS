use crate::memory::{Conversation, MemoryManager, SessionSummary, default_workspace_root};
use gtk4::prelude::*;
use gtk4::{Box, Button, Entry, Label, Orientation, PolicyType, ScrolledWindow, Window};
use std::rc::Rc;

pub struct MemoryBrowser;

impl MemoryBrowser {
    pub fn show<F>(parent: &impl IsA<gtk4::Window>, workspace: &str, on_select: F)
    where
        F: Fn(Conversation, SessionSummary) + 'static,
    {
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

        let search = Entry::builder()
            .placeholder_text("Search sessions...")
            .build();
        search.add_css_class("launcher-search");
        root.append(&search);

        let summary = Label::new(None);
        summary.set_halign(gtk4::Align::Start);
        summary.add_css_class("app-bar-stats");
        root.append(&summary);

        let list_box = Box::new(Orientation::Vertical, 6);

        let manager = MemoryManager::new(default_workspace_root());
        match manager.list_sessions(workspace) {
            Ok(sessions) if !sessions.is_empty() => {
                let sessions = Rc::new(sessions);
                let manager = Rc::new(manager);
                let on_select: Rc<dyn Fn(Conversation, SessionSummary)> = Rc::new(on_select);
                populate_sessions(
                    &list_box, &summary, &sessions, &manager, &on_select, &window, "",
                );

                let list_ref = list_box.clone();
                let summary_ref = summary.clone();
                let sessions_ref = sessions.clone();
                let manager_ref = manager.clone();
                let on_select_ref = on_select.clone();
                let window_ref = window.clone();
                search.connect_changed(move |entry| {
                    let query = entry.text().to_string();
                    populate_sessions(
                        &list_ref,
                        &summary_ref,
                        &sessions_ref,
                        &manager_ref,
                        &on_select_ref,
                        &window_ref,
                        &query,
                    );
                });
            }
            Ok(_) => {
                summary.set_text("No sessions found");
                let empty = Label::new(Some("No conversations saved yet."));
                empty.add_css_class("welcome-hint");
                empty.set_halign(gtk4::Align::Start);
                list_box.append(&empty);
            }
            Err(err) => {
                summary.set_text("Failed to load sessions");
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

fn format_session_summary(count: usize) -> String {
    if count == 1 {
        "Showing 1 session".to_string()
    } else {
        format!("Showing {count} sessions")
    }
}

fn format_filtered_summary(filtered: usize, total: usize) -> String {
    if total == 0 {
        "No sessions found".to_string()
    } else if filtered == total {
        format_session_summary(total)
    } else {
        format!("Showing {filtered} of {total} sessions")
    }
}

fn matches_session_query(session: &SessionSummary, query_lower: &str) -> bool {
    if query_lower.is_empty() {
        return true;
    }
    let title = session.title.as_deref().unwrap_or("");
    title.to_lowercase().contains(query_lower)
        || session.category.to_lowercase().contains(query_lower)
        || session.session_id.to_lowercase().contains(query_lower)
}

fn populate_sessions(
    list_box: &Box,
    summary: &Label,
    sessions: &[SessionSummary],
    manager: &Rc<MemoryManager>,
    on_select: &Rc<dyn Fn(Conversation, SessionSummary)>,
    window: &Window,
    query: &str,
) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let query_lower = query.to_lowercase();
    let mut shown = 0usize;

    for session in sessions.iter() {
        if !matches_session_query(session, &query_lower) {
            continue;
        }
        if shown >= 30 {
            break;
        }
        shown += 1;

        let btn = Button::new();
        btn.add_css_class("launcher-item");
        let row = Box::new(Orientation::Vertical, 2);

        let title_text = session
            .title
            .clone()
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

        btn.set_child(Some(&row));
        let session_clone = session.clone();
        let manager_ref = Rc::clone(manager);
        let window_ref = window.clone();
        let on_select_ref = Rc::clone(on_select);
        btn.connect_clicked(move |_| {
            let loaded = manager_ref.load_session(
                &session_clone.workspace,
                &session_clone.category,
                &session_clone.session_id,
            );
            match loaded {
                Ok(Some(conversation)) => {
                    let _ = manager_ref.set_active_session(
                        &session_clone.workspace,
                        &session_clone.category,
                        &session_clone.session_id,
                    );
                    on_select_ref(conversation, session_clone.clone());
                    window_ref.close();
                }
                Ok(None) => {
                    log::warn!("Session not found: {}", session_clone.session_id);
                }
                Err(err) => {
                    log::warn!(
                        "Failed to load session {}: {}",
                        session_clone.session_id,
                        err
                    );
                }
            }
        });

        list_box.append(&btn);
    }

    summary.set_text(&format_filtered_summary(shown, sessions.len()));

    if list_box.first_child().is_none() {
        let message = if query.trim().is_empty() {
            "No conversations saved yet.".to_string()
        } else {
            format!("No sessions found for '{query}'.")
        };
        let empty = Label::new(Some(&message));
        empty.add_css_class("welcome-hint");
        empty.set_halign(gtk4::Align::Start);
        list_box.append(&empty);
    }
}

#[cfg(test)]
mod tests {
    use super::{format_filtered_summary, format_session_summary, matches_session_query};
    use crate::memory::SessionSummary;
    use chrono::{TimeZone, Utc};

    #[test]
    fn summary_pluralization_is_correct() {
        assert_eq!(format_session_summary(1), "Showing 1 session");
        assert_eq!(format_session_summary(4), "Showing 4 sessions");
    }

    #[test]
    fn filtered_summary_handles_partial_results() {
        assert_eq!(format_filtered_summary(0, 0), "No sessions found");
        assert_eq!(format_filtered_summary(3, 10), "Showing 3 of 10 sessions");
        assert_eq!(format_filtered_summary(4, 4), "Showing 4 sessions");
    }

    #[test]
    fn session_query_matches_title_category_or_id() {
        let session = SessionSummary {
            workspace: "demo".to_string(),
            category: "general".to_string(),
            session_id: "abc123".to_string(),
            title: Some("Build CLI".to_string()),
            created_at: Utc.with_ymd_and_hms(2026, 3, 16, 10, 0, 0).unwrap(),
            last_active: Utc.with_ymd_and_hms(2026, 3, 16, 11, 0, 0).unwrap(),
        };
        assert!(matches_session_query(&session, "cli"));
        assert!(matches_session_query(&session, "general"));
        assert!(matches_session_query(&session, "abc"));
        assert!(!matches_session_query(&session, "missing"));
    }
}

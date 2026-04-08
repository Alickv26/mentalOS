use crate::memory::{Conversation, MemoryManager, SessionSummary, default_workspace_root};
use crate::ui::sync_selection::SyncSelectionDialog;
use crate::ui::tasks_browser::TasksBrowser;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Box, Button, Entry, Label, Orientation, PolicyType, ScrolledWindow, Window};
use std::cell::RefCell;
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

        let controls = Box::new(Orientation::Horizontal, 8);
        let resume_btn = Button::with_label("Resume current");
        resume_btn.add_css_class("icon-button");
        resume_btn.set_halign(gtk4::Align::Start);
        resume_btn.set_visible(false);
        controls.append(&resume_btn);

        let delete_btn = Button::with_label("Delete current");
        delete_btn.add_css_class("icon-button");
        delete_btn.set_halign(gtk4::Align::Start);
        delete_btn.set_visible(false);
        controls.append(&delete_btn);

        let sync_btn = Button::with_label("Sync selection");
        sync_btn.add_css_class("icon-button");
        sync_btn.set_halign(gtk4::Align::End);
        controls.append(&sync_btn);

        let tasks_btn = Button::with_label("Tasks");
        tasks_btn.add_css_class("icon-button");
        tasks_btn.set_halign(gtk4::Align::End);
        controls.append(&tasks_btn);
        root.append(&controls);

        let list_box = Box::new(Orientation::Vertical, 6);
        let workspace_for_sync = workspace.to_string();
        let window_for_sync = window.clone();
        sync_btn.connect_clicked(move |_| {
            SyncSelectionDialog::show(&window_for_sync, &workspace_for_sync);
        });

        let window_for_tasks = window.clone();
        tasks_btn.connect_clicked(move |_| {
            TasksBrowser::show(&window_for_tasks);
        });

        let manager = MemoryManager::new(default_workspace_root());
        match manager.list_sessions(workspace) {
            Ok(sessions) if !sessions.is_empty() => {
                let sessions = Rc::new(RefCell::new(sessions));
                let manager = Rc::new(manager);
                let on_select: Rc<dyn Fn(Conversation, SessionSummary)> = Rc::new(on_select);
                let workspace_name = workspace.to_string();
                let active_session = Rc::new(RefCell::new(None::<SessionSummary>));

                refresh_browser_state(
                    &list_box,
                    &summary,
                    &resume_btn,
                    &delete_btn,
                    &sessions,
                    &manager,
                    &on_select,
                    &window,
                    &workspace_name,
                    &active_session,
                    "",
                );

                let manager_ref = manager.clone();
                let on_select_ref = on_select.clone();
                let window_ref = window.clone();
                let active_ref = active_session.clone();
                resume_btn.connect_clicked(move |_| {
                    if let Some(session) = active_ref.borrow().clone() {
                        open_session(&manager_ref, on_select_ref.clone(), &window_ref, &session);
                    }
                });

                let list_ref = list_box.clone();
                let summary_ref = summary.clone();
                let resume_ref = resume_btn.clone();
                let delete_ref = delete_btn.clone();
                let sessions_ref = sessions.clone();
                let manager_ref = manager.clone();
                let on_select_ref = on_select.clone();
                let window_ref = window.clone();
                let workspace_ref = workspace_name.clone();
                let active_ref = active_session.clone();
                search.connect_changed(move |entry| {
                    let query = entry.text().to_string();
                    refresh_browser_state(
                        &list_ref,
                        &summary_ref,
                        &resume_ref,
                        &delete_ref,
                        &sessions_ref,
                        &manager_ref,
                        &on_select_ref,
                        &window_ref,
                        &workspace_ref,
                        &active_ref,
                        &query,
                    );
                });

                let manager_ref = manager.clone();
                let on_select_ref = on_select.clone();
                let window_ref = window.clone();
                let active_ref = active_session.clone();
                search.connect_activate(move |_| {
                    if let Some(session) = active_ref.borrow().clone() {
                        open_session(&manager_ref, on_select_ref.clone(), &window_ref, &session);
                    }
                });

                let search_ref = search.clone();
                let list_ref = list_box.clone();
                let summary_ref = summary.clone();
                let resume_ref = resume_btn.clone();
                let delete_ref = delete_btn.clone();
                let sessions_ref = sessions.clone();
                let manager_ref = manager.clone();
                let on_select_ref = on_select.clone();
                let window_ref = window.clone();
                let workspace_ref = workspace_name.clone();
                let active_ref = active_session.clone();
                glib::timeout_add_seconds_local(1, move || {
                    if !window_ref.is_visible() {
                        return glib::ControlFlow::Break;
                    }
                    refresh_browser_state(
                        &list_ref,
                        &summary_ref,
                        &resume_ref,
                        &delete_ref,
                        &sessions_ref,
                        &manager_ref,
                        &on_select_ref,
                        &window_ref,
                        &workspace_ref,
                        &active_ref,
                        &search_ref.text(),
                    );
                    glib::ControlFlow::Continue
                });

                let search_ref = search.clone();
                let list_ref = list_box.clone();
                let summary_ref = summary.clone();
                let resume_ref = resume_btn.clone();
                let delete_ref = delete_btn.clone();
                let sessions_ref = sessions.clone();
                let manager_ref = manager.clone();
                let on_select_ref = on_select.clone();
                let window_ref = window.clone();
                let workspace_ref = workspace_name.clone();
                let active_ref = active_session.clone();
                delete_btn.connect_clicked(move |_| {
                    let Some(session) = active_ref.borrow().clone() else {
                        return;
                    };
                    delete_session_and_refresh(
                        &session,
                        &list_ref,
                        &summary_ref,
                        &resume_ref,
                        &delete_ref,
                        &sessions_ref,
                        &manager_ref,
                        &on_select_ref,
                        &window_ref,
                        &workspace_ref,
                        &active_ref,
                        &search_ref.text(),
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

fn matches_session_query_with_content(
    manager: &MemoryManager,
    session: &SessionSummary,
    query_lower: &str,
) -> bool {
    if matches_session_query(session, query_lower) {
        return true;
    }
    if query_lower.is_empty() {
        return true;
    }

    match manager.load_session(&session.workspace, &session.category, &session.session_id) {
        Ok(Some(conversation)) => conversation
            .messages
            .iter()
            .any(|msg| msg.content.to_lowercase().contains(query_lower)),
        _ => false,
    }
}

fn detect_active_session_id(
    workspace: &str,
    sessions: &[SessionSummary],
    manager: &MemoryManager,
) -> Option<String> {
    for session in sessions {
        if let Ok(Some(active)) = manager.active_session_id(workspace, &session.category)
            && active != "NEW"
            && active == session.session_id
        {
            return Some(active);
        }
    }
    None
}

fn find_session(sessions: &[SessionSummary], session_id: &str) -> Option<SessionSummary> {
    sessions
        .iter()
        .find(|s| s.session_id == session_id)
        .cloned()
}

fn refresh_browser_state(
    list_box: &Box,
    summary: &Label,
    resume_btn: &Button,
    delete_btn: &Button,
    sessions: &Rc<RefCell<Vec<SessionSummary>>>,
    manager: &Rc<MemoryManager>,
    on_select: &Rc<dyn Fn(Conversation, SessionSummary)>,
    window: &Window,
    workspace: &str,
    active_session: &Rc<RefCell<Option<SessionSummary>>>,
    query: &str,
) {
    let sessions_snapshot = sessions.borrow().clone();
    let active_session_id = detect_active_session_id(workspace, &sessions_snapshot, manager);
    let active_summary = active_session_id
        .as_deref()
        .and_then(|session_id| find_session(&sessions_snapshot, session_id));

    if let Some(session) = active_summary.clone() {
        let label = session.title.as_deref().unwrap_or(&session.session_id);
        resume_btn.set_label(&format!("Resume current: {label}"));
        resume_btn.set_visible(true);
        delete_btn.set_label(&format!("Delete current: {}", session.session_id));
        delete_btn.set_visible(true);
        *active_session.borrow_mut() = Some(session);
    } else {
        resume_btn.set_visible(false);
        delete_btn.set_visible(false);
        *active_session.borrow_mut() = None;
    }

    populate_sessions(
        list_box,
        summary,
        &sessions_snapshot,
        sessions,
        manager,
        on_select,
        window,
        resume_btn,
        delete_btn,
        workspace,
        active_session,
        query,
        active_session_id.as_deref(),
    );
}

fn ordered_sessions<'a>(
    sessions: &'a [SessionSummary],
    active_session_id: Option<&str>,
) -> Vec<&'a SessionSummary> {
    let mut filtered: Vec<&SessionSummary> = sessions.iter().collect();

    if let Some(active) = active_session_id
        && let Some(index) = filtered
            .iter()
            .position(|session| session.session_id == active)
    {
        let active_session = filtered.remove(index);
        filtered.insert(0, active_session);
    }

    filtered
}

fn open_session(
    manager: &MemoryManager,
    on_select: Rc<dyn Fn(Conversation, SessionSummary)>,
    window: &Window,
    session: &SessionSummary,
) {
    let loaded = manager.load_session(&session.workspace, &session.category, &session.session_id);
    match loaded {
        Ok(Some(conversation)) => {
            let _ = manager.set_active_session(
                &session.workspace,
                &session.category,
                &session.session_id,
            );
            (on_select)(conversation, session.clone());
            window.close();
        }
        Ok(None) => {
            log::warn!("Session not found: {}", session.session_id);
        }
        Err(err) => {
            log::warn!("Failed to load session {}: {}", session.session_id, err);
        }
    }
}

fn populate_sessions(
    list_box: &Box,
    summary: &Label,
    sessions: &[SessionSummary],
    sessions_store: &Rc<RefCell<Vec<SessionSummary>>>,
    manager: &Rc<MemoryManager>,
    on_select: &Rc<dyn Fn(Conversation, SessionSummary)>,
    window: &Window,
    resume_btn: &Button,
    delete_btn: &Button,
    workspace: &str,
    active_session: &Rc<RefCell<Option<SessionSummary>>>,
    query: &str,
    active_session_id: Option<&str>,
) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let query_lower = query.to_lowercase();
    let filtered_sessions = ordered_sessions(sessions, active_session_id);
    let mut shown = 0usize;

    for session in filtered_sessions.iter() {
        if !matches_session_query_with_content(manager, session, &query_lower) {
            continue;
        }
        if shown >= 30 {
            break;
        }
        shown += 1;

        let btn = Button::new();
        btn.add_css_class("launcher-item");
        let row = Box::new(Orientation::Horizontal, 8);
        let content = Box::new(Orientation::Vertical, 2);

        if active_session_id == Some(session.session_id.as_str()) {
            btn.add_css_class("active-memory-session");
        }

        let title_text = session
            .title
            .clone()
            .unwrap_or_else(|| format!("Session {}", session.session_id));
        let head = Label::new(Some(&title_text));
        head.set_halign(gtk4::Align::Start);
        head.set_wrap(true);
        head.add_css_class("message-content");
        content.append(&head);

        let active_suffix = if active_session_id == Some(session.session_id.as_str()) {
            "   active"
        } else {
            ""
        };
        let details = format!(
            "category: {}   created: {}   last active: {}",
            session.category,
            session.created_at.format("%Y-%m-%d %H:%M"),
            session.last_active.format("%Y-%m-%d %H:%M")
        );
        let meta = Label::new(Some(&(details + active_suffix)));
        meta.set_halign(gtk4::Align::Start);
        meta.add_css_class("app-bar-stats");
        content.append(&meta);
        content.set_hexpand(true);

        let row_delete_btn = Button::with_label("Delete");
        row_delete_btn.add_css_class("icon-button");
        row_delete_btn.set_halign(gtk4::Align::End);
        row_delete_btn.set_valign(gtk4::Align::Start);

        row.append(&content);
        row.append(&row_delete_btn);

        btn.set_child(Some(&row));
        let session_clone = (*session).clone();
        let manager_ref = Rc::clone(manager);
        let window_ref = window.clone();
        let on_select_ref = Rc::clone(on_select);
        btn.connect_clicked(move |_| {
            open_session(
                &manager_ref,
                on_select_ref.clone(),
                &window_ref,
                &session_clone,
            );
        });

        let session_for_delete = (*session).clone();
        let list_ref = list_box.clone();
        let summary_ref = summary.clone();
        let resume_ref = resume_btn.clone();
        let delete_ref = delete_btn.clone();
        let sessions_ref = sessions_store.clone();
        let manager_ref = manager.clone();
        let on_select_ref = on_select.clone();
        let window_ref = window.clone();
        let workspace_ref = workspace.to_string();
        let active_ref = active_session.clone();
        let query_ref = query.to_string();
        row_delete_btn.connect_clicked(move |_| {
            delete_session_and_refresh(
                &session_for_delete,
                &list_ref,
                &summary_ref,
                &resume_ref,
                &delete_ref,
                &sessions_ref,
                &manager_ref,
                &on_select_ref,
                &window_ref,
                &workspace_ref,
                &active_ref,
                &query_ref,
            );
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

#[allow(clippy::too_many_arguments)]
fn delete_session_and_refresh(
    session: &SessionSummary,
    list_box: &Box,
    summary: &Label,
    resume_btn: &Button,
    delete_btn: &Button,
    sessions: &Rc<RefCell<Vec<SessionSummary>>>,
    manager: &Rc<MemoryManager>,
    on_select: &Rc<dyn Fn(Conversation, SessionSummary)>,
    window: &Window,
    workspace: &str,
    active_session: &Rc<RefCell<Option<SessionSummary>>>,
    query: &str,
) {
    match manager.delete_session(&session.workspace, &session.category, &session.session_id) {
        Ok(true) => {
            if let Ok(updated) = manager.list_sessions(workspace) {
                *sessions.borrow_mut() = updated;
            }
            refresh_browser_state(
                list_box,
                summary,
                resume_btn,
                delete_btn,
                sessions,
                manager,
                on_select,
                window,
                workspace,
                active_session,
                query,
            );
        }
        Ok(false) => {}
        Err(err) => {
            log::warn!("Failed to delete session {}: {}", session.session_id, err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_filtered_summary, format_session_summary, matches_session_query,
        matches_session_query_with_content, ordered_sessions,
    };
    use crate::memory::{MemoryManager, Role, SessionSummary};
    use chrono::{TimeZone, Utc};
    use tempfile::TempDir;

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
            local_only: false,
        };
        assert!(matches_session_query(&session, "cli"));
        assert!(matches_session_query(&session, "general"));
        assert!(matches_session_query(&session, "abc"));
        assert!(!matches_session_query(&session, "missing"));
    }

    #[test]
    fn active_session_is_pinned_first() {
        let s1 = SessionSummary {
            workspace: "demo".to_string(),
            category: "general".to_string(),
            session_id: "first".to_string(),
            title: Some("First".to_string()),
            created_at: Utc.with_ymd_and_hms(2026, 3, 16, 10, 0, 0).unwrap(),
            last_active: Utc.with_ymd_and_hms(2026, 3, 16, 11, 0, 0).unwrap(),
            local_only: false,
        };
        let s2 = SessionSummary {
            workspace: "demo".to_string(),
            category: "general".to_string(),
            session_id: "active".to_string(),
            title: Some("Active".to_string()),
            created_at: Utc.with_ymd_and_hms(2026, 3, 16, 10, 0, 0).unwrap(),
            last_active: Utc.with_ymd_and_hms(2026, 3, 16, 12, 0, 0).unwrap(),
            local_only: false,
        };
        let sessions = [s1, s2];
        let ordered = ordered_sessions(&sessions, Some("active"));
        assert_eq!(ordered[0].session_id, "active");
    }

    #[test]
    fn session_query_matches_message_content() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());
        manager
            .append_message(
                "default",
                "general",
                Role::User,
                "remember alpha deployment checklist",
            )
            .unwrap();
        let session = manager
            .list_sessions("default")
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        assert!(matches_session_query_with_content(
            &manager,
            &session,
            "deployment"
        ));
        assert!(!matches_session_query_with_content(
            &manager,
            &session,
            "nonexistent-token"
        ));
    }
}

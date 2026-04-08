use crate::memory::{MemoryManager, SessionSummary, default_workspace_root};
use chrono::NaiveDate;
use gtk4::prelude::*;
use gtk4::{
    Align, Box, Button, CheckButton, Entry, Label, Orientation, PolicyType, ScrolledWindow, Window,
};
use std::cell::RefCell;
use std::rc::Rc;

pub struct SyncSelectionDialog;

#[derive(Clone)]
struct SessionRowState {
    category: String,
    session_id: String,
    include_toggle: CheckButton,
    local_only_toggle: CheckButton,
}

impl SyncSelectionDialog {
    pub fn show(parent: &impl IsA<gtk4::Window>, workspace: &str) {
        let window = Window::builder()
            .title("Sync Conversations")
            .modal(true)
            .transient_for(parent)
            .default_width(620)
            .default_height(460)
            .build();
        window.add_css_class("launcher-window");

        let root = Box::new(Orientation::Vertical, 10);
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        root.set_margin_start(12);
        root.set_margin_end(12);

        let title = Label::new(Some("Select conversations to sync"));
        title.add_css_class("app-bar-title");
        title.set_halign(Align::Start);
        root.append(&title);

        let filters = Box::new(Orientation::Horizontal, 8);
        let from_entry = Entry::builder()
            .placeholder_text("From date (YYYY-MM-DD)")
            .build();
        from_entry.add_css_class("launcher-search");
        let to_entry = Entry::builder()
            .placeholder_text("To date (YYYY-MM-DD)")
            .build();
        to_entry.add_css_class("launcher-search");
        filters.append(&from_entry);
        filters.append(&to_entry);
        root.append(&filters);

        let hint = Label::new(Some(
            "Local-only conversations are excluded from sync even if selected.",
        ));
        hint.add_css_class("app-bar-stats");
        hint.set_halign(Align::Start);
        root.append(&hint);

        let list_box = Box::new(Orientation::Vertical, 6);
        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .child(&list_box)
            .build();
        root.append(&scroll);

        let status = Label::new(None);
        status.add_css_class("app-bar-stats");
        status.set_halign(Align::Start);
        root.append(&status);

        let actions = Box::new(Orientation::Horizontal, 8);
        actions.set_halign(Align::End);
        let close_btn = Button::with_label("Close");
        let apply_btn = Button::with_label("Apply Selection");
        let export_btn = Button::with_label("Export JSON");
        apply_btn.add_css_class("create-button");
        export_btn.add_css_class("create-button");
        actions.append(&close_btn);
        actions.append(&apply_btn);
        actions.append(&export_btn);
        root.append(&actions);

        window.set_child(Some(&root));

        let manager = Rc::new(MemoryManager::new(default_workspace_root()));
        let sessions = match manager.list_sessions(workspace) {
            Ok(v) => v,
            Err(err) => {
                status.set_text(&format!("Failed to load sessions: {err}"));
                window.present();
                return;
            }
        };
        let sessions = Rc::new(RefCell::new(sessions));
        let row_state = Rc::new(RefCell::new(Vec::<SessionRowState>::new()));
        let workspace_name = workspace.to_string();

        render_rows(
            &list_box,
            &status,
            &sessions.borrow(),
            &manager,
            &workspace_name,
            &row_state,
            &from_entry.text(),
            &to_entry.text(),
        );

        let list_ref = list_box.clone();
        let status_ref = status.clone();
        let sessions_ref = sessions.clone();
        let manager_ref = manager.clone();
        let workspace_ref = workspace_name.clone();
        let rows_ref = row_state.clone();
        let to_ref = to_entry.clone();
        from_entry.connect_changed(move |entry| {
            render_rows(
                &list_ref,
                &status_ref,
                &sessions_ref.borrow(),
                &manager_ref,
                &workspace_ref,
                &rows_ref,
                &entry.text(),
                &to_ref.text(),
            );
        });

        let list_ref = list_box.clone();
        let status_ref = status.clone();
        let sessions_ref = sessions.clone();
        let manager_ref = manager.clone();
        let workspace_ref = workspace_name.clone();
        let rows_ref = row_state.clone();
        let from_ref = from_entry.clone();
        to_entry.connect_changed(move |entry| {
            render_rows(
                &list_ref,
                &status_ref,
                &sessions_ref.borrow(),
                &manager_ref,
                &workspace_ref,
                &rows_ref,
                &from_ref.text(),
                &entry.text(),
            );
        });

        let rows_ref = row_state.clone();
        let status_ref = status.clone();
        let manager_ref = manager.clone();
        let workspace_ref = workspace_name.clone();
        apply_btn.connect_clicked(move |_| {
            let rows = rows_ref.borrow();
            let selected_pairs: Vec<(String, String)> = rows
                .iter()
                .filter(|r| r.include_toggle.is_active())
                .map(|r| (r.category.clone(), r.session_id.clone()))
                .collect();
            match manager_ref.build_sync_payload(&workspace_ref, &selected_pairs) {
                Ok(payload) => {
                    status_ref.set_text(&format!(
                        "Selected {}. Sync-eligible: {}. Local-only excluded: {}. Redactions applied: {}.",
                        selected_pairs.len(),
                        payload.conversations.len(),
                        payload.skipped_local_only,
                        payload.redactions
                    ));
                }
                Err(err) => {
                    status_ref.set_text(&format!("Failed to build sync payload: {err}"));
                }
            }
        });

        let rows_ref = row_state.clone();
        let status_ref = status.clone();
        let manager_ref = manager.clone();
        let workspace_ref = workspace_name.clone();
        export_btn.connect_clicked(move |_| {
            let rows = rows_ref.borrow();
            let selected_pairs: Vec<(String, String)> = rows
                .iter()
                .filter(|r| r.include_toggle.is_active())
                .map(|r| (r.category.clone(), r.session_id.clone()))
                .collect();
            match manager_ref.export_sync_payload(&workspace_ref, &selected_pairs) {
                Ok(result) => {
                    status_ref.set_text(&format!(
                        "Exported {} conversations to {} (local-only excluded: {}, redactions: {}).",
                        result.exported_count,
                        result.path.display(),
                        result.skipped_local_only,
                        result.redactions
                    ));
                }
                Err(err) => {
                    status_ref.set_text(&format!("Failed to export sync payload: {err}"));
                }
            }
        });

        let window_ref = window.clone();
        close_btn.connect_clicked(move |_| {
            window_ref.close();
        });

        window.present();
    }
}

fn render_rows(
    list_box: &Box,
    status: &Label,
    sessions: &[SessionSummary],
    manager: &Rc<MemoryManager>,
    workspace: &str,
    row_state: &Rc<RefCell<Vec<SessionRowState>>>,
    from_date: &str,
    to_date: &str,
) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }
    row_state.borrow_mut().clear();

    let from_filter = parse_date_filter(from_date);
    let to_filter = parse_date_filter(to_date);
    let mut rendered = 0usize;

    for session in sessions {
        if !session_matches_date_range(session, from_filter, to_filter) {
            continue;
        }
        rendered += 1;

        let row = Box::new(Orientation::Horizontal, 8);
        row.add_css_class("launcher-item");

        let detail = Box::new(Orientation::Vertical, 2);
        detail.set_hexpand(true);
        let title = session
            .title
            .as_deref()
            .unwrap_or(&format!("Session {}", session.session_id))
            .to_string();
        let title_lbl = Label::new(Some(&title));
        title_lbl.set_halign(Align::Start);
        title_lbl.set_wrap(true);
        title_lbl.add_css_class("message-content");
        detail.append(&title_lbl);

        let meta_text = format!(
            "{}  |  {}  |  {}",
            session.category,
            session.created_at.format("%Y-%m-%d %H:%M"),
            session.session_id
        );
        let meta = Label::new(Some(&meta_text));
        meta.set_halign(Align::Start);
        meta.add_css_class("app-bar-stats");
        detail.append(&meta);

        let include_toggle = CheckButton::with_label("Sync");
        include_toggle.set_active(!session.local_only);
        let local_only_toggle = CheckButton::with_label("Local only");
        local_only_toggle.set_active(session.local_only);

        let manager_ref = manager.clone();
        let workspace_ref = workspace.to_string();
        let category_ref = session.category.clone();
        let session_id_ref = session.session_id.clone();
        include_toggle.connect_toggled(move |toggle| {
            if toggle.is_active() {
                let _ = manager_ref.set_local_only(
                    &workspace_ref,
                    &category_ref,
                    &session_id_ref,
                    false,
                );
            }
        });

        let manager_ref = manager.clone();
        let workspace_ref = workspace.to_string();
        let category_ref = session.category.clone();
        let session_id_ref = session.session_id.clone();
        let include_ref = include_toggle.clone();
        local_only_toggle.connect_toggled(move |toggle| {
            let local_only = toggle.is_active();
            if local_only {
                include_ref.set_active(false);
            }
            let _ = manager_ref.set_local_only(
                &workspace_ref,
                &category_ref,
                &session_id_ref,
                local_only,
            );
        });

        row.append(&detail);
        row.append(&include_toggle);
        row.append(&local_only_toggle);
        list_box.append(&row);

        row_state.borrow_mut().push(SessionRowState {
            category: session.category.clone(),
            session_id: session.session_id.clone(),
            include_toggle,
            local_only_toggle,
        });
    }

    if rendered == 0 {
        let empty = Label::new(Some("No sessions match date filters."));
        empty.add_css_class("welcome-hint");
        empty.set_halign(Align::Start);
        list_box.append(&empty);
    }

    let local_only_count = row_state
        .borrow()
        .iter()
        .filter(|r| r.local_only_toggle.is_active())
        .count();
    status.set_text(&format!(
        "Showing {rendered} sessions. Local-only: {local_only_count}."
    ));
}

fn parse_date_filter(value: &str) -> Option<NaiveDate> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").ok()
}

fn session_matches_date_range(
    session: &SessionSummary,
    from_date: Option<NaiveDate>,
    to_date: Option<NaiveDate>,
) -> bool {
    let created = session.created_at.date_naive();
    if let Some(from) = from_date {
        if created < from {
            return false;
        }
    }
    if let Some(to) = to_date {
        if created > to {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{parse_date_filter, session_matches_date_range};
    use crate::memory::SessionSummary;
    use chrono::{TimeZone, Utc};

    #[test]
    fn parse_date_filter_accepts_iso_date() {
        assert!(parse_date_filter("2026-04-08").is_some());
        assert!(parse_date_filter("invalid").is_none());
    }

    #[test]
    fn session_matches_range_bounds() {
        let session = SessionSummary {
            workspace: "demo".to_string(),
            category: "general".to_string(),
            session_id: "abc".to_string(),
            title: Some("Test".to_string()),
            created_at: Utc.with_ymd_and_hms(2026, 4, 8, 10, 0, 0).unwrap(),
            last_active: Utc.with_ymd_and_hms(2026, 4, 8, 10, 5, 0).unwrap(),
            local_only: false,
        };
        let from = parse_date_filter("2026-04-01");
        let to = parse_date_filter("2026-04-30");
        assert!(session_matches_date_range(&session, from, to));
        let from = parse_date_filter("2026-05-01");
        assert!(!session_matches_date_range(&session, from, None));
    }
}

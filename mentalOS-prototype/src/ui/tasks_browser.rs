use crate::memory::default_workspace_root;
use crate::task_tracker::{Task, TaskStatus, TaskTracker};
use gtk4::prelude::*;
use gtk4::{Align, Box, Button, Label, Orientation, PolicyType, ScrolledWindow, Window};
use std::cell::RefCell;
use std::rc::Rc;

pub struct TasksBrowser;

impl TasksBrowser {
    pub fn show(parent: &impl IsA<gtk4::Window>) {
        let window = Window::builder()
            .title("Tasks")
            .modal(true)
            .transient_for(parent)
            .default_width(620)
            .default_height(420)
            .build();
        window.add_css_class("launcher-window");

        let root = Box::new(Orientation::Vertical, 8);
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        root.set_margin_start(12);
        root.set_margin_end(12);

        let title = Label::new(Some("Tracked tasks"));
        title.add_css_class("app-bar-title");
        title.set_halign(Align::Start);
        root.append(&title);

        let summary = Label::new(None);
        summary.add_css_class("app-bar-stats");
        summary.set_halign(Align::Start);
        root.append(&summary);

        let list_box = Box::new(Orientation::Vertical, 6);
        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .child(&list_box)
            .build();
        root.append(&scroll);

        let close_btn = Button::with_label("Close");
        close_btn.set_halign(Align::End);
        root.append(&close_btn);
        window.set_child(Some(&root));

        let tracker = Rc::new(RefCell::new(TaskTracker::new(default_workspace_root())));
        render_tasks(&list_box, &summary, &tracker);

        let window_ref = window.clone();
        close_btn.connect_clicked(move |_| {
            window_ref.close();
        });

        window.present();
    }
}

fn render_tasks(list_box: &Box, summary: &Label, tracker: &Rc<RefCell<TaskTracker>>) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let tasks = tracker.borrow().list_tasks(None);
    summary.set_text(&summarize_tasks(&tasks));

    if tasks.is_empty() {
        let empty = Label::new(Some("No tasks captured yet."));
        empty.add_css_class("welcome-hint");
        empty.set_halign(Align::Start);
        list_box.append(&empty);
        return;
    }

    for task in tasks {
        let row = Box::new(Orientation::Horizontal, 8);
        row.add_css_class("launcher-item");

        let detail = Box::new(Orientation::Vertical, 2);
        detail.set_hexpand(true);

        let title = Label::new(Some(&task.description));
        title.set_halign(Align::Start);
        title.set_wrap(true);
        title.add_css_class("message-content");
        detail.append(&title);

        let meta_text = format!(
            "{} | {} | {}",
            status_label(&task.status),
            priority_label(&task),
            task.created_at.format("%Y-%m-%d %H:%M")
        );
        let meta = Label::new(Some(&meta_text));
        meta.set_halign(Align::Start);
        meta.add_css_class("app-bar-stats");
        detail.append(&meta);

        let toggle_btn = Button::with_label(if task.status == TaskStatus::Pending {
            "Done"
        } else {
            "Reopen"
        });
        toggle_btn.add_css_class("icon-button");

        let delete_btn = Button::with_label("Delete");
        delete_btn.add_css_class("icon-button");

        let task_id = task.id.clone();
        let tracker_ref = tracker.clone();
        let list_ref = list_box.clone();
        let summary_ref = summary.clone();
        toggle_btn.connect_clicked(move |_| {
            let new_status = {
                let current = tracker_ref.borrow().get_task(&task_id);
                match current.map(|t| t.status) {
                    Some(TaskStatus::Pending) => TaskStatus::Completed,
                    Some(_) => TaskStatus::Pending,
                    None => return,
                }
            };
            let _ = tracker_ref.borrow_mut().update_status(&task_id, new_status);
            render_tasks(&list_ref, &summary_ref, &tracker_ref);
        });

        let task_id = task.id.clone();
        let tracker_ref = tracker.clone();
        let list_ref = list_box.clone();
        let summary_ref = summary.clone();
        delete_btn.connect_clicked(move |_| {
            let _ = tracker_ref.borrow_mut().delete_task(&task_id);
            render_tasks(&list_ref, &summary_ref, &tracker_ref);
        });

        row.append(&detail);
        row.append(&toggle_btn);
        row.append(&delete_btn);
        list_box.append(&row);
    }
}

fn status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Completed => "completed",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn priority_label(task: &Task) -> &'static str {
    match task.priority {
        crate::task_tracker::TaskPriority::Low => "low",
        crate::task_tracker::TaskPriority::Medium => "medium",
        crate::task_tracker::TaskPriority::High => "high",
        crate::task_tracker::TaskPriority::Critical => "critical",
    }
}

fn summarize_tasks(tasks: &[Task]) -> String {
    let total = tasks.len();
    let pending = tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Pending)
        .count();
    let done = tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Completed)
        .count();
    format!("Total: {total} | Pending: {pending} | Completed: {done}")
}

#[cfg(test)]
mod tests {
    use super::summarize_tasks;
    use crate::task_tracker::{Task, TaskPriority, TaskStatus};
    use chrono::Utc;

    #[test]
    fn summarize_tasks_counts_statuses() {
        let tasks = vec![
            Task {
                id: "a".to_string(),
                description: "one".to_string(),
                created_at: Utc::now(),
                status: TaskStatus::Pending,
                priority: TaskPriority::Medium,
                due_date: None,
                related_conversation: None,
                related_project: None,
            },
            Task {
                id: "b".to_string(),
                description: "two".to_string(),
                created_at: Utc::now(),
                status: TaskStatus::Completed,
                priority: TaskPriority::Medium,
                due_date: None,
                related_conversation: None,
                related_project: None,
            },
        ];
        let summary = summarize_tasks(&tasks);
        assert!(summary.contains("Total: 2"));
        assert!(summary.contains("Pending: 1"));
        assert!(summary.contains("Completed: 1"));
    }
}

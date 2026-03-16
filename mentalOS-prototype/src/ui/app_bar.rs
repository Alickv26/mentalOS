use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Box, Button, Label, Orientation, ProgressBar};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use sysinfo::System;

/// Top bar displaying the app name, AI status, system stats, and emergency stop.
pub struct AppBar {
    pub container: Box,
    status_label: Label,
    _cpu_label: Label,
    _mem_label: Label,
    new_chat_btn: Button,
    memory_btn: Button,
    stop_btn: Button,
    progress: ProgressBar,
    busy: Rc<Cell<bool>>,
}

fn memory_button_label() -> &'static str {
    "Memory"
}

fn new_chat_button_label() -> &'static str {
    "New chat"
}

impl AppBar {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Vertical, 0);
        container.add_css_class("app-bar");

        let row = Box::new(Orientation::Horizontal, 8);

        // ── Title ──
        let title = Label::new(Some("mentalOS"));
        title.add_css_class("app-bar-title");
        title.set_accessible_role(gtk4::AccessibleRole::Heading);
        title.update_property(&[
            gtk4::accessible::Property::Label("mentalOS"),
            gtk4::accessible::Property::Description("Application title."),
        ]);
        row.append(&title);

        // ── AI status ──
        let status_label = Label::new(Some("[AI: Idle]"));
        status_label.add_css_class("app-bar-status");
        status_label.set_hexpand(true);
        status_label.set_halign(gtk4::Align::Start);
        status_label.set_accessible_role(gtk4::AccessibleRole::Status);
        status_label.update_property(&[
            gtk4::accessible::Property::Label("AI status idle"),
            gtk4::accessible::Property::Description("Current AI runtime status."),
        ]);
        row.append(&status_label);

        // ── System stats ──
        let cpu_label = Label::new(Some("CPU: --%"));
        cpu_label.add_css_class("app-bar-stats");
        cpu_label.update_property(&[
            gtk4::accessible::Property::Label("CPU usage"),
            gtk4::accessible::Property::Description("Current CPU usage percentage."),
        ]);
        row.append(&cpu_label);

        let mem_label = Label::new(Some("MEM: --%"));
        mem_label.add_css_class("app-bar-stats");
        mem_label.update_property(&[
            gtk4::accessible::Property::Label("Memory usage"),
            gtk4::accessible::Property::Description("Current memory usage percentage."),
        ]);
        row.append(&mem_label);

        // ── New chat button ──
        let new_chat_btn = Button::with_label(new_chat_button_label());
        new_chat_btn.add_css_class("icon-button");
        new_chat_btn.update_property(&[
            gtk4::accessible::Property::Label("Start a new chat"),
            gtk4::accessible::Property::Description(
                "Clear the current conversation and start a new chat.",
            ),
        ]);
        row.append(&new_chat_btn);

        // ── Memory button ──
        let memory_btn = Button::with_label(memory_button_label());
        memory_btn.add_css_class("icon-button");
        memory_btn.update_property(&[
            gtk4::accessible::Property::Label("Open memory browser"),
            gtk4::accessible::Property::Description("Browse recent conversation sessions."),
            gtk4::accessible::Property::KeyShortcuts("Ctrl+Shift+M"),
        ]);
        row.append(&memory_btn);

        // ── Stop button ──
        let stop_btn = Button::with_label("🔴 STOP");
        stop_btn.add_css_class("stop-button");
        stop_btn.update_property(&[
            gtk4::accessible::Property::Label("Emergency stop"),
            gtk4::accessible::Property::Description("Immediately stop active AI operations."),
            gtk4::accessible::Property::KeyShortcuts("Ctrl+Shift+Q"),
        ]);
        row.append(&stop_btn);
        container.append(&row);

        let progress = ProgressBar::new();
        progress.set_show_text(false);
        progress.set_hexpand(true);
        progress.set_visible(false);
        progress.add_css_class("app-progress");
        progress.set_accessible_role(gtk4::AccessibleRole::ProgressBar);
        progress.update_property(&[
            gtk4::accessible::Property::Label("Operation progress"),
            gtk4::accessible::Property::Description("Indicates background request progress."),
        ]);
        container.append(&progress);

        let busy = Rc::new(Cell::new(false));
        let bar = Self {
            container,
            status_label,
            _cpu_label: cpu_label.clone(),
            _mem_label: mem_label.clone(),
            new_chat_btn: new_chat_btn.clone(),
            memory_btn: memory_btn.clone(),
            stop_btn: stop_btn.clone(),
            progress: progress.clone(),
            busy: busy.clone(),
        };

        // ── Periodic stats update ──
        let sys = Rc::new(RefCell::new(System::new()));
        let cpu_ref = cpu_label;
        let mem_ref = mem_label;
        glib::timeout_add_seconds_local(2, move || {
            let mut s = sys.borrow_mut();
            s.refresh_cpu_usage();
            s.refresh_memory();

            let cpu_usage: f32 = if s.cpus().is_empty() {
                0.0
            } else {
                s.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / s.cpus().len() as f32
            };
            let mem_total = s.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
            let mem_used = s.used_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
            let mem_pct = if mem_total > 0.0 {
                (mem_used / mem_total) * 100.0
            } else {
                0.0
            };

            cpu_ref.set_text(&format!("CPU: {cpu_usage:.0}%"));
            mem_ref.set_text(&format!("MEM: {mem_pct:.0}%"));
            glib::ControlFlow::Continue
        });

        let progress_ref = progress;
        glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
            if busy.get() {
                progress_ref.pulse();
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Continue
            }
        });

        bar
    }

    /// Update the AI status indicator.
    pub fn set_status(&self, status: &str) {
        let label = format!("[AI: {status}]");
        self.status_label.set_text(&label);
        self.status_label.update_property(&[
            gtk4::accessible::Property::Label(&format!("AI status {status}")),
            gtk4::accessible::Property::Description("Current AI runtime status."),
        ]);
    }

    pub fn connect_stop_clicked<F: Fn(&Button) + 'static>(&self, f: F) {
        self.stop_btn.connect_clicked(f);
    }

    pub fn connect_new_chat_clicked<F: Fn(&Button) + 'static>(&self, f: F) {
        self.new_chat_btn.connect_clicked(f);
    }

    pub fn connect_memory_clicked<F: Fn(&Button) + 'static>(&self, f: F) {
        self.memory_btn.connect_clicked(f);
    }

    pub fn set_busy(&self, busy: bool) {
        self.busy.set(busy);
        self.progress.set_visible(busy);
        if busy {
            self.progress.pulse();
        } else {
            self.progress.set_fraction(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{memory_button_label, new_chat_button_label};

    #[test]
    fn memory_button_label_is_stable() {
        assert_eq!(memory_button_label(), "Memory");
    }

    #[test]
    fn new_chat_button_label_is_stable() {
        assert_eq!(new_chat_button_label(), "New chat");
    }
}

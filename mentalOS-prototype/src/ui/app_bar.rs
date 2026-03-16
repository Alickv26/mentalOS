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
    stop_btn: Button,
    progress: ProgressBar,
    busy: Rc<Cell<bool>>,
}

impl AppBar {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Vertical, 0);
        container.add_css_class("app-bar");

        let row = Box::new(Orientation::Horizontal, 8);

        // ── Title ──
        let title = Label::new(Some("mentalOS"));
        title.add_css_class("app-bar-title");
        row.append(&title);

        // ── AI status ──
        let status_label = Label::new(Some("[AI: Idle]"));
        status_label.add_css_class("app-bar-status");
        status_label.set_hexpand(true);
        status_label.set_halign(gtk4::Align::Start);
        row.append(&status_label);

        // ── System stats ──
        let cpu_label = Label::new(Some("CPU: --%"));
        cpu_label.add_css_class("app-bar-stats");
        row.append(&cpu_label);

        let mem_label = Label::new(Some("MEM: --%"));
        mem_label.add_css_class("app-bar-stats");
        row.append(&mem_label);

        // ── Stop button ──
        let stop_btn = Button::with_label("🔴 STOP");
        stop_btn.add_css_class("stop-button");
        row.append(&stop_btn);
        container.append(&row);

        let progress = ProgressBar::new();
        progress.set_show_text(false);
        progress.set_hexpand(true);
        progress.set_visible(false);
        progress.add_css_class("app-progress");
        container.append(&progress);

        let busy = Rc::new(Cell::new(false));
        let bar = Self {
            container,
            status_label,
            _cpu_label: cpu_label.clone(),
            _mem_label: mem_label.clone(),
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
        self.status_label.set_text(&format!("[AI: {status}]"));
    }

    pub fn connect_stop_clicked<F: Fn(&Button) + 'static>(&self, f: F) {
        self.stop_btn.connect_clicked(f);
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

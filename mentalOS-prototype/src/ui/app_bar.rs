use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Box, Button, Label, Orientation};
use std::cell::RefCell;
use std::rc::Rc;
use sysinfo::System;

/// Top bar displaying the app name, AI status, system stats, and emergency stop.
pub struct AppBar {
    pub container: Box,
    status_label: Label,
    _cpu_label: Label,
    _mem_label: Label,
    stop_btn: Button,
}

impl AppBar {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Horizontal, 8);
        container.add_css_class("app-bar");

        // ── Title ──
        let title = Label::new(Some("mentalOS"));
        title.add_css_class("app-bar-title");
        container.append(&title);

        // ── AI status ──
        let status_label = Label::new(Some("[AI: Idle]"));
        status_label.add_css_class("app-bar-status");
        status_label.set_hexpand(true);
        status_label.set_halign(gtk4::Align::Start);
        container.append(&status_label);

        // ── System stats ──
        let cpu_label = Label::new(Some("CPU: --%"));
        cpu_label.add_css_class("app-bar-stats");
        container.append(&cpu_label);

        let mem_label = Label::new(Some("MEM: --%"));
        mem_label.add_css_class("app-bar-stats");
        container.append(&mem_label);

        // ── Stop button ──
        let stop_btn = Button::with_label("🔴 STOP");
        stop_btn.add_css_class("stop-button");
        container.append(&stop_btn);

        let bar = Self {
            container,
            status_label,
            _cpu_label: cpu_label.clone(),
            _mem_label: mem_label.clone(),
            stop_btn: stop_btn.clone(),
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

        bar
    }

    /// Update the AI status indicator.
    pub fn set_status(&self, status: &str) {
        self.status_label.set_text(&format!("[AI: {status}]"));
    }

    pub fn connect_stop_clicked<F: Fn(&Button) + 'static>(&self, f: F) {
        self.stop_btn.connect_clicked(f);
    }
}

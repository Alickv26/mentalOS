use crate::ui::shortcuts::{SHORTCUT_DEFINITIONS, ShortcutBindings};
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Window};

pub struct ShortcutsHelp;

impl ShortcutsHelp {
    pub fn show(parent: &impl IsA<gtk4::Window>, bindings: &ShortcutBindings) {
        let dialog = Window::builder()
            .title("Keyboard Shortcuts")
            .modal(true)
            .transient_for(parent)
            .default_width(520)
            .default_height(380)
            .resizable(false)
            .build();
        dialog.add_css_class("launcher-window");

        let root = Box::new(Orientation::Vertical, 8);
        root.set_margin_top(16);
        root.set_margin_bottom(16);
        root.set_margin_start(16);
        root.set_margin_end(16);

        let title = Label::new(Some("Keyboard shortcuts"));
        title.set_halign(gtk4::Align::Start);
        title.add_css_class("app-bar-title");
        root.append(&title);

        let total = Label::new(Some(&format!(
            "{} actions available",
            SHORTCUT_DEFINITIONS.len()
        )));
        total.set_halign(gtk4::Align::Start);
        total.add_css_class("app-bar-stats");
        root.append(&total);

        for def in SHORTCUT_DEFINITIONS {
            let combo = bindings.get(def.id);
            let line = format_shortcut_line(&combo, def.label, def.description);
            let label = Label::new(Some(&line));
            label.set_halign(gtk4::Align::Start);
            label.add_css_class("message-content");
            label.set_wrap(true);
            root.append(&label);
        }

        let esc_line = Label::new(Some("Esc               Close current window/dialog"));
        esc_line.set_halign(gtk4::Align::Start);
        esc_line.add_css_class("message-content");
        root.append(&esc_line);

        let hint = Label::new(Some(
            "Use Manage shortcuts shortcut to change key bindings.",
        ));
        hint.set_halign(gtk4::Align::Start);
        hint.add_css_class("app-bar-stats");
        root.append(&hint);

        dialog.set_child(Some(&root));

        let key_ctrl = gtk4::EventControllerKey::new();
        let d = dialog.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                d.close();
                return gtk4::glib::Propagation::Stop;
            }
            gtk4::glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl);

        dialog.present();
    }
}

fn format_shortcut_line(combo: &str, label: &str, description: &str) -> String {
    format!("{combo:<18} {label} — {description}")
}

#[cfg(test)]
mod tests {
    use super::format_shortcut_line;

    #[test]
    fn shortcut_line_keeps_combo_prefix() {
        let line = format_shortcut_line("Ctrl+K", "Open app launcher", "Open apps dialog");
        assert!(line.starts_with("Ctrl+K"));
        assert!(line.contains("Open app launcher"));
        assert!(line.contains("Open apps dialog"));
    }
}

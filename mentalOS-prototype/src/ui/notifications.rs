use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Label, Revealer};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

pub struct NotificationCenter {
    pub container: Revealer,
    label: Label,
    generation: Rc<Cell<u64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

impl NotificationCenter {
    pub fn new() -> Self {
        let label = Label::new(None);
        label.set_halign(gtk4::Align::Start);
        label.set_wrap(true);
        label.add_css_class("notification-label");

        let revealer = Revealer::new();
        revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(180);
        revealer.set_reveal_child(false);
        revealer.set_child(Some(&label));
        revealer.add_css_class("notification-wrap");

        Self {
            container: revealer,
            label,
            generation: Rc::new(Cell::new(0)),
        }
    }

    pub fn show(&self, message: &str) {
        self.show_with_level(message, NotificationLevel::Info);
    }

    pub fn show_warning(&self, message: &str) {
        self.show_with_level(message, NotificationLevel::Warning);
    }

    pub fn show_error(&self, message: &str) {
        self.show_with_level(message, NotificationLevel::Error);
    }

    pub fn show_with_level(&self, message: &str, level: NotificationLevel) {
        apply_level_css_class(&self.label, level);
        self.label.set_text(message);
        self.container.set_reveal_child(true);

        let revealer = self.container.clone();
        let generation = self.generation.clone();
        let id = generation.get().wrapping_add(1);
        generation.set(id);
        let _source = glib::timeout_add_local_once(Duration::from_secs(4), move || {
            if generation.get() == id {
                revealer.set_reveal_child(false);
            }
        });
    }

    pub fn hide(&self) {
        let id = self.generation.get().wrapping_add(1);
        self.generation.set(id);
        self.container.set_reveal_child(false);
    }
}

fn level_css_class(level: NotificationLevel) -> &'static str {
    match level {
        NotificationLevel::Info => "notification-info",
        NotificationLevel::Warning => "notification-warning",
        NotificationLevel::Error => "notification-error",
    }
}

fn apply_level_css_class(label: &Label, level: NotificationLevel) {
    for class in [
        "notification-info",
        "notification-warning",
        "notification-error",
    ] {
        label.remove_css_class(class);
    }
    label.add_css_class(level_css_class(level));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_css_class_mappings_are_stable() {
        assert_eq!(
            level_css_class(NotificationLevel::Info),
            "notification-info"
        );
        assert_eq!(
            level_css_class(NotificationLevel::Warning),
            "notification-warning"
        );
        assert_eq!(
            level_css_class(NotificationLevel::Error),
            "notification-error"
        );
    }
}

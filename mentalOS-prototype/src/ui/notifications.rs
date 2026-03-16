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

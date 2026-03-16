use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation};

pub struct HomeScreen {
    pub container: Box,
}

impl HomeScreen {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Vertical, 0);
        container.add_css_class("home-screen");
        container.set_hexpand(true);
        container.set_vexpand(true);

        let spacer_top = Box::new(Orientation::Vertical, 0);
        spacer_top.set_vexpand(true);
        container.append(&spacer_top);

        let center_box = Box::new(Orientation::Vertical, 16);
        center_box.set_halign(gtk4::Align::Center);
        center_box.set_valign(gtk4::Align::Center);

        let title = Label::new(Some("mentalOS"));
        title.add_css_class("home-title");
        center_box.append(&title);

        let subtitle = Label::new(Some("Your AI-powered operating system"));
        subtitle.add_css_class("home-subtitle");
        center_box.append(&subtitle);

        let hint = Label::new(Some("Type below to get started"));
        hint.add_css_class("home-hint");
        center_box.append(&hint);

        container.append(&center_box);

        let spacer_bottom = Box::new(Orientation::Vertical, 0);
        spacer_bottom.set_vexpand(true);
        container.append(&spacer_bottom);

        Self { container }
    }

    pub fn set_pill(&self, _pill: gtk4::Widget) {
        // Pill is added separately in main_window
    }
}

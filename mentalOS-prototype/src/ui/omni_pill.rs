use gtk4::prelude::*;
use gtk4::{Box, Button, Entry, Label, Orientation, Spinner};

/// The visual state of the AI Agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiState {
    Sleep,   // Gray/Dimmed
    Active,  // Blue Pulse
    Agentic, // Purple/Gold
    Learn,   // Amber
}

impl AiState {
    pub fn css_class(&self) -> &'static str {
        match self {
            AiState::Sleep => "state-sleep",
            AiState::Active => "state-active",
            AiState::Agentic => "state-agentic",
            AiState::Learn => "state-learn",
        }
    }

    pub fn icon_label(&self) -> &'static str {
        match self {
            AiState::Sleep => "⚪",
            AiState::Active => "🔵",
            AiState::Agentic => "🔮",
            AiState::Learn => "🟠",
        }
    }
}

/// The Omni-Pill input component.
pub struct OmniPill {
    pub container: Box,
    pub input: Entry,
    pub spinner: Spinner,
    pub apps_btn: Button,
    pub term_btn: Button,
    status_icon: Label,
    stop_btn: Button,
}

impl OmniPill {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Horizontal, 8);
        container.add_css_class("omni-input-box");

        // Status Icon (left side)
        let status_icon = Label::new(Some("⚪"));
        status_icon.add_css_class("status-icon");
        container.append(&status_icon);

        // Spinner (hidden by default)
        let spinner = Spinner::new();
        spinner.add_css_class("spinner");
        spinner.set_visible(false);
        container.append(&spinner);

        // Text Input (center)
        let input = Entry::builder()
            .placeholder_text("Ask mentalOS...")
            .has_frame(false)
            .hexpand(true)
            .build();
        input.add_css_class("omni-entry");
        container.append(&input);

        // Action Buttons (right side)
        let apps_btn = Button::from_icon_name("view-app-grid-symbolic");
        apps_btn.add_css_class("icon-button");
        apps_btn.set_tooltip_text(Some("Apps"));
        container.append(&apps_btn);

        let term_btn = Button::from_icon_name("utilities-terminal-symbolic");
        term_btn.add_css_class("icon-button");
        term_btn.set_tooltip_text(Some("Terminal"));
        container.append(&term_btn);

        // Stop Button
        let stop_btn = Button::with_label("■");
        stop_btn.add_css_class("icon-button");
        stop_btn.add_css_class("stop-button");
        stop_btn.set_tooltip_text(Some("Emergency Stop"));
        container.append(&stop_btn);

        Self {
            container,
            input,
            spinner,
            apps_btn,
            term_btn,
            status_icon,
            stop_btn,
        }
    }

    pub fn set_state(&self, state: AiState) {
        self.status_icon.set_text(state.icon_label());
        match state {
            AiState::Active | AiState::Agentic | AiState::Learn => {
                self.spinner.start();
                self.spinner.set_visible(true);
                self.status_icon.set_visible(false);
            }
            AiState::Sleep => {
                self.spinner.stop();
                self.spinner.set_visible(false);
                self.status_icon.set_visible(true);
            }
        }
    }

    pub fn connect_stop_clicked<F: Fn(&Button) + 'static>(&self, f: F) {
        self.stop_btn.connect_clicked(f);
    }
}

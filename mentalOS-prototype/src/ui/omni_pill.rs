use gtk4::prelude::*;
use gtk4::{Box, Button, DropDown, Entry, Label, Orientation, Spinner, StringList};

// Re-export CircuitState so callers don't need to import the providers module
pub use crate::providers::circuit_breaker::CircuitState;

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

/// Maps a circuit breaker state to a status icon override.
///
/// Returns `Some(icon)` when the breaker is non-Closed (to override the
/// normal AiState icon), or `None` when Closed (let the AiState show through).
fn circuit_state_icon(state: CircuitState) -> Option<&'static str> {
    match state {
        CircuitState::Closed => None,
        CircuitState::Open => Some("🔴"),
        CircuitState::HalfOpen => Some("🟡"),
    }
}

/// The Omni-Pill input component.
pub struct OmniPill {
    pub container: Box,
    pub input: Entry,
    pub spinner: Spinner,
    pub apps_btn: Button,
    pub term_btn: Button,
    pub agent_selector: DropDown,
    status_icon: Label,
    /// Sublabel under the status icon showing the active provider name
    /// (e.g. "openclaw", "deepseek", "zen", "ollama").
    provider_label: Label,
    stop_btn: Button,
}

impl OmniPill {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Horizontal, 8);
        container.add_css_class("omni-input-box");

        // Status Icon (left side)
        let status_icon = Label::new(Some("⚪"));
        status_icon.add_css_class("status-icon");
        status_icon.set_accessible_role(gtk4::AccessibleRole::Status);
        status_icon.update_property(&[
            gtk4::accessible::Property::Label("AI status"),
            gtk4::accessible::Property::Description(
                "Shows whether the AI is idle, active, or running a task.",
            ),
        ]);
        container.append(&status_icon);

        // Provider label (small text under the status icon showing the
        // active provider name — "openclaw", "deepseek", "zen", "ollama")
        let provider_label = Label::new(Some(""));
        provider_label.add_css_class("provider-label");
        provider_label.set_accessible_role(gtk4::AccessibleRole::Label);
        provider_label.update_property(&[
            gtk4::accessible::Property::Label("Active provider"),
            gtk4::accessible::Property::Description(
                "Shows which AI provider is currently active (openclaw, ollama, deepseek, or zen).",
            ),
        ]);
        container.append(&provider_label);

        // Spinner (hidden by default)
        let spinner = Spinner::new();
        spinner.add_css_class("spinner");
        spinner.set_visible(false);
        spinner.set_accessible_role(gtk4::AccessibleRole::ProgressBar);
        spinner.update_property(&[
            gtk4::accessible::Property::Label("AI activity"),
            gtk4::accessible::Property::Description("Shows when the AI is processing a request."),
        ]);
        container.append(&spinner);

        // Agent Selector (Dropdown)
        let model = StringList::new(&["Loading..."]);
        let agent_selector = DropDown::builder().model(&model).build();
        agent_selector.add_css_class("agent-selector");
        agent_selector.set_accessible_role(gtk4::AccessibleRole::ComboBox);
        agent_selector.update_property(&[
            gtk4::accessible::Property::Label("Agent selector"),
            gtk4::accessible::Property::Description(
                "Choose the active AI provider (OpenClaw, Ollama, DeepSeek, or OpenCode Zen).",
            ),
        ]);
        container.append(&agent_selector);

        // Text Input (center)
        let input = Entry::builder()
            .placeholder_text("Ask mentalOS...")
            .has_frame(false)
            .hexpand(true)
            .build();
        input.add_css_class("omni-entry");
        input.set_accessible_role(gtk4::AccessibleRole::TextBox);
        input.update_property(&[
            gtk4::accessible::Property::Label("Prompt input"),
            gtk4::accessible::Property::Description("Type your message to mentalOS."),
            gtk4::accessible::Property::Placeholder("Ask mentalOS..."),
            gtk4::accessible::Property::KeyShortcuts("Ctrl+L"),
        ]);
        container.append(&input);

        // Action Buttons (right side)
        let apps_btn = Button::from_icon_name("view-app-grid-symbolic");
        apps_btn.add_css_class("icon-button");
        apps_btn.set_tooltip_text(Some("Apps"));
        apps_btn.update_property(&[
            gtk4::accessible::Property::Label("Open app launcher"),
            gtk4::accessible::Property::Description(
                "Opens the installed applications launcher dialog.",
            ),
            gtk4::accessible::Property::KeyShortcuts("Ctrl+K"),
        ]);
        container.append(&apps_btn);

        let term_btn = Button::from_icon_name("utilities-terminal-symbolic");
        term_btn.add_css_class("icon-button");
        term_btn.set_tooltip_text(Some("Terminal"));
        term_btn.update_property(&[
            gtk4::accessible::Property::Label("Open terminal"),
            gtk4::accessible::Property::Description("Opens a terminal command."),
        ]);
        container.append(&term_btn);

        // Stop Button
        let stop_btn = Button::with_label("■");
        stop_btn.add_css_class("icon-button");
        stop_btn.add_css_class("stop-button");
        stop_btn.set_tooltip_text(Some("Emergency Stop"));
        stop_btn.update_property(&[
            gtk4::accessible::Property::Label("Emergency stop"),
            gtk4::accessible::Property::Description("Immediately stop active AI operations."),
            gtk4::accessible::Property::KeyShortcuts("Ctrl+Shift+Q"),
        ]);
        container.append(&stop_btn);

        Self {
            container,
            input,
            spinner,
            apps_btn,
            term_btn,
            agent_selector,
            status_icon,
            provider_label,
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

    /// Update the small text label showing the active provider's name.
    ///
    /// Call this whenever the provider changes (config wizard save, agent
    /// switch, or on app startup once the router is constructed).
    pub fn set_provider(&self, name: &str) {
        self.provider_label.set_text(name);
        let desc = format!("Active AI provider: {}", name);
        self.provider_label
            .update_property(&[gtk4::accessible::Property::Description(desc.as_str())]);
    }

    /// Update the status icon to reflect the circuit breaker state.
    ///
    /// - `Closed`: the normal AiState icon is shown (⚪/🔵/🔮/🟠)
    /// - `HalfOpen`: yellow dot 🟡 (provider is being tested for recovery)
    /// - `Open`: red dot 🔴 (provider is temporarily unavailable)
    ///
    /// Call this after every `handle_input` call so the UI reflects the
    /// current breaker state in real time.
    pub fn set_circuit_state(&self, state: CircuitState) {
        // We can't mutate self.last_circuit_state from &self, but the icon
        // override logic below is stateless — it just picks the right icon
        // for the given state on each call.
        if let Some(override_icon) = circuit_state_icon(state) {
            self.status_icon.set_text(override_icon);
            self.status_icon.set_visible(true);
            self.spinner.stop();
            self.spinner.set_visible(false);
        }
        // When Closed, the next set_state() call will restore the normal icon.
    }

    pub fn connect_stop_clicked<F: Fn(&Button) + 'static>(&self, f: F) {
        self.stop_btn.connect_clicked(f);
    }
}

impl Default for OmniPill {
    fn default() -> Self {
        Self::new()
    }
}

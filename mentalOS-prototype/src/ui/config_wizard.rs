//! First-run setup wizard for mentalOS configuration.
//!
//! Guides the user through:
//! 1. Choosing an AI provider (OpenClaw, Ollama, DeepSeek, OpenCode Zen)
//! 2. Configuring provider-specific settings (endpoints, API keys, models)
//! 3. Setting up the workspace directory
//! 4. Saving the configuration to disk

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Box, Button, DropDown, Entry, Label, Orientation, SpinButton, StringList, Switch, Window,
};
use log::{info, warn};

use crate::config::{Config, OpenClawConfig, OllamaConfig, DeepSeekConfig, OpenCodeZenConfig, PathsConfig, AiConfig};

/// Provider names displayed in the dropdown.
const PROVIDER_LABELS: &[&str] = &[
    "OpenClaw (Local Gateway)",
    "Ollama (Local Models)",
    "DeepSeek (Cloud API)",
    "OpenCode Zen (Multi-Model Gateway)",
];

/// Internal provider identifiers matching config keys.
const PROVIDER_IDS: &[&str] = &["openclaw", "ollama", "deepseek", "zen"];

pub struct ConfigWizard;

impl ConfigWizard {
    /// Show the configuration wizard as a modal dialog.
    ///
    /// `on_complete` is called after the user finishes the wizard and the
    /// configuration has been saved to disk.
    pub fn show<F: FnOnce() + 'static>(parent: &Window, on_complete: F) {
        let dialog = gtk4::Dialog::builder()
            .title("mentalOS Setup")
            .transient_for(parent)
            .modal(true)
            .default_width(480)
            .default_height(520)
            .build();

        let content_area = dialog.content_area();

        // ── Title ──
        let title = Label::new(Some("Welcome to mentalOS"));
        title.add_css_class("wizard-title");
        content_area.append(&title);

        let subtitle = Label::new(Some("Configure your AI provider to get started."));
        subtitle.add_css_class("wizard-subtitle");
        content_area.append(&subtitle);

        // ── Provider Selection ──
        let provider_section = Label::new(Some("AI Provider"));
        provider_section.add_css_class("wizard-section-label");
        content_area.append(&provider_section);

        let provider_model = StringList::new(PROVIDER_LABELS);
        let provider_dropdown = DropDown::builder()
            .model(&provider_model)
            .selected(0)
            .build();
        provider_dropdown.add_css_class("wizard-dropdown");
        content_area.append(&provider_dropdown);

        // ── Dynamic Provider Config Container ──
        let config_container = Box::new(Orientation::Vertical, 8);
        config_container.add_css_class("wizard-config-container");
        content_area.append(&config_container);

        // Pre-create all provider config widgets
        let openclaw_widgets = OpenClawWidgets::new();
        let ollama_widgets = OllamaWidgets::new();
        let deepseek_widgets = DeepSeekWidgets::new();
        let zen_widgets = ZenWidgets::new();

        // Add all widgets but only show the first one initially
        config_container.append(&openclaw_widgets.container);
        config_container.append(&ollama_widgets.container);
        config_container.append(&deepseek_widgets.container);
        config_container.append(&zen_widgets.container);

        // Show only the selected provider's config
        ollama_widgets.container.set_visible(false);
        deepseek_widgets.container.set_visible(false);
        zen_widgets.container.set_visible(false);

        let oc_ref = openclaw_widgets.container.clone();
        let ol_ref = ollama_widgets.container.clone();
        let ds_ref = deepseek_widgets.container.clone();
        let zen_ref = zen_widgets.container.clone();

        provider_dropdown.connect_selected_notify(move |dropdown| {
            let idx = dropdown.selected();
            oc_ref.set_visible(idx == 0);
            ol_ref.set_visible(idx == 1);
            ds_ref.set_visible(idx == 2);
            zen_ref.set_visible(idx == 3);
        });

        // ── Workspace ──
        let workspace_section = Label::new(Some("Workspace"));
        workspace_section.add_css_class("wizard-section-label");
        content_area.append(&workspace_section);

        let workspace_label = Label::new(Some("Workspace directory:"));
        workspace_label.set_halign(gtk4::Align::Start);
        content_area.append(&workspace_label);

        let workspace_entry = Entry::builder()
            .text("~/workspaces")
            .hexpand(true)
            .build();
        content_area.append(&workspace_entry);

        // ── Buttons ──
        let btn_box = Box::new(Orientation::Horizontal, 8);
        btn_box.set_halign(gtk4::Align::End);
        btn_box.set_margin_top(12);

        let save_btn = Button::builder()
            .label("Save & Start")
            .css_classes(["suggested-action", "pill-button"])
            .build();
        btn_box.append(&save_btn);
        content_area.append(&btn_box);

        // ── Save handler ──
        let config_for_save = Config::default();
        let dialog_ref = dialog.clone();
        // Wrap on_complete in RefCell<Option<>> so we can take() it once inside the Fn closure
        let on_complete_cell = std::cell::RefCell::new(Some(on_complete));

        save_btn.connect_clicked(move |_| {
            let provider_idx = provider_dropdown.selected() as usize;
            let provider_id = PROVIDER_IDS.get(provider_idx).unwrap_or(&"openclaw");

            let mut config = config_for_save.clone();

            // Set AI provider
            config.ai.provider = provider_id.to_string();

            // Set provider-specific config
            match *provider_id {
                "openclaw" => {
                    config.openclaw.endpoint = openclaw_widgets.endpoint_entry.text().to_string();
                    config.openclaw.transport = if openclaw_widgets.transport_switch.is_active() {
                        "http".to_string()
                    } else {
                        "cli".to_string()
                    };
                    config.openclaw.token = openclaw_widgets.token_entry.text().to_string();
                    config.openclaw.auto_start = openclaw_widgets.auto_start_switch.is_active();
                    config.openclaw.cli_path = openclaw_widgets.cli_path_entry.text().to_string();
                }
                "ollama" => {
                    config.ollama.endpoint = ollama_widgets.endpoint_entry.text().to_string();
                    config.ollama.model = ollama_widgets.model_entry.text().to_string();
                }
                "deepseek" => {
                    config.deepseek.api_key = deepseek_widgets.api_key_entry.text().to_string();
                    config.deepseek.model = deepseek_widgets.model_entry.text().to_string();
                    config.deepseek.endpoint = deepseek_widgets.endpoint_entry.text().to_string();
                    config.deepseek.thinking_mode = deepseek_widgets.thinking_switch.is_active();
                    if let Ok(temp) = deepseek_widgets.temperature_spin.value().to_string().parse::<f32>() {
                        config.deepseek.temperature = temp;
                    }
                }
                "zen" => {
                    config.zen.api_key = zen_widgets.api_key_entry.text().to_string();
                    config.zen.model = zen_widgets.model_entry.text().to_string();
                    config.zen.endpoint = zen_widgets.endpoint_entry.text().to_string();
                }
                _ => {}
            }

            // Set workspace
            config.paths.workspace_dir = workspace_entry.text().to_string();

            // Save config
            match config.save() {
                Ok(path) => {
                    info!("Configuration saved to {}", path.display());
                    dialog_ref.close();
                    // Take the callback out of the cell so it's only called once
                    if let Some(cb) = on_complete_cell.borrow_mut().take() {
                        cb();
                    }
                }
                Err(e) => {
                    warn!("Failed to save configuration: {}", e);
                }
            }
        });

        dialog.present();
    }
}

// ── OpenClaw Config Widgets ────────────────────────────────────

struct OpenClawWidgets {
    container: Box,
    endpoint_entry: Entry,
    cli_path_entry: Entry,
    token_entry: Entry,
    transport_switch: Switch,
    auto_start_switch: Switch,
}

impl OpenClawWidgets {
    fn new() -> Self {
        let container = Box::new(Orientation::Vertical, 6);

        let endpoint_label = Label::new(Some("Gateway endpoint:"));
        endpoint_label.set_halign(gtk4::Align::Start);
        container.append(&endpoint_label);

        let endpoint_entry = Entry::builder()
            .text("http://127.0.0.1:18789")
            .hexpand(true)
            .build();
        container.append(&endpoint_entry);

        let cli_path_label = Label::new(Some("CLI path:"));
        cli_path_label.set_halign(gtk4::Align::Start);
        container.append(&cli_path_label);

        let cli_path_entry = Entry::builder()
            .text("openclaw")
            .hexpand(true)
            .build();
        container.append(&cli_path_entry);

        let token_label = Label::new(Some("OpenClaw Token:"));
        token_label.set_halign(gtk4::Align::Start);
        container.append(&token_label);

        let token_entry = Entry::builder()
            .hexpand(true)
            .input_purpose(gtk4::InputPurpose::Password)
            .build();
        container.append(&token_entry);

        // Transport switch (HTTP vs CLI)
        let transport_box = Box::new(Orientation::Horizontal, 8);
        let transport_label = Label::new(Some("Use HTTP transport"));
        transport_label.set_hexpand(true);
        transport_label.set_halign(gtk4::Align::Start);
        let transport_switch = Switch::builder().active(false).build();
        transport_box.append(&transport_label);
        transport_box.append(&transport_switch);
        container.append(&transport_box);

        // Auto-start switch
        let auto_start_box = Box::new(Orientation::Horizontal, 8);
        let auto_start_label = Label::new(Some("Auto-start OpenClaw gateway"));
        auto_start_label.set_hexpand(true);
        auto_start_label.set_halign(gtk4::Align::Start);
        let auto_start_switch = Switch::builder().active(false).build();
        auto_start_box.append(&auto_start_label);
        auto_start_box.append(&auto_start_switch);
        container.append(&auto_start_box);

        // Fallback hint
        let fallback_hint = Label::new(Some("Fallback to Ollama if OpenClaw fails"));
        fallback_hint.add_css_class("wizard-hint");
        fallback_hint.set_halign(gtk4::Align::Start);
        container.append(&fallback_hint);

        Self {
            container,
            endpoint_entry,
            cli_path_entry,
            token_entry,
            transport_switch,
            auto_start_switch,
        }
    }
}

// ── Ollama Config Widgets ──────────────────────────────────────

struct OllamaWidgets {
    container: Box,
    endpoint_entry: Entry,
    model_entry: Entry,
}

impl OllamaWidgets {
    fn new() -> Self {
        let container = Box::new(Orientation::Vertical, 6);

        let endpoint_label = Label::new(Some("Ollama endpoint:"));
        endpoint_label.set_halign(gtk4::Align::Start);
        container.append(&endpoint_label);

        let endpoint_entry = Entry::builder()
            .text("http://127.0.0.1:11434")
            .hexpand(true)
            .build();
        container.append(&endpoint_entry);

        let model_label = Label::new(Some("Model:"));
        model_label.set_halign(gtk4::Align::Start);
        container.append(&model_label);

        let model_entry = Entry::builder()
            .text("phi3:mini")
            .hexpand(true)
            .build();
        container.append(&model_entry);

        let hint = Label::new(Some("Make sure Ollama is running locally before starting."));
        hint.add_css_class("wizard-hint");
        hint.set_halign(gtk4::Align::Start);
        container.append(&hint);

        Self {
            container,
            endpoint_entry,
            model_entry,
        }
    }
}

// ── DeepSeek Config Widgets ────────────────────────────────────

struct DeepSeekWidgets {
    container: Box,
    api_key_entry: Entry,
    model_entry: Entry,
    endpoint_entry: Entry,
    thinking_switch: Switch,
    temperature_spin: SpinButton,
}

impl DeepSeekWidgets {
    fn new() -> Self {
        let container = Box::new(Orientation::Vertical, 6);

        let api_key_label = Label::new(Some("API Key:"));
        api_key_label.set_halign(gtk4::Align::Start);
        container.append(&api_key_label);

        let api_key_entry = Entry::builder()
            .hexpand(true)
            .input_purpose(gtk4::InputPurpose::Password)
            .visibility(false)
            .build();
        container.append(&api_key_entry);

        let privacy_hint = Label::new(Some("Your API key is stored locally and never shared."));
        privacy_hint.add_css_class("wizard-hint");
        privacy_hint.set_halign(gtk4::Align::Start);
        container.append(&privacy_hint);

        let model_label = Label::new(Some("Model:"));
        model_label.set_halign(gtk4::Align::Start);
        container.append(&model_label);

        let model_entry = Entry::builder()
            .text("deepseek-v4-pro")
            .hexpand(true)
            .build();
        container.append(&model_entry);

        let model_hint = Label::new(Some("Available: deepseek-v4-flash, deepseek-v4-pro"));
        model_hint.add_css_class("wizard-hint");
        model_hint.set_halign(gtk4::Align::Start);
        container.append(&model_hint);

        let endpoint_label = Label::new(Some("Endpoint:"));
        endpoint_label.set_halign(gtk4::Align::Start);
        container.append(&endpoint_label);

        let endpoint_entry = Entry::builder()
            .text("https://api.deepseek.com")
            .hexpand(true)
            .build();
        container.append(&endpoint_entry);

        // Thinking mode switch
        let thinking_box = Box::new(Orientation::Horizontal, 8);
        let thinking_label = Label::new(Some("Thinking mode (reasoning_content)"));
        thinking_label.set_hexpand(true);
        thinking_label.set_halign(gtk4::Align::Start);
        let thinking_switch = Switch::builder().active(true).build();
        thinking_box.append(&thinking_label);
        thinking_box.append(&thinking_switch);
        container.append(&thinking_box);

        // Temperature
        let temp_box = Box::new(Orientation::Horizontal, 8);
        let temp_label = Label::new(Some("Temperature:"));
        temp_label.set_halign(gtk4::Align::Start);
        let temperature_spin = SpinButton::with_range(0.0, 2.0, 0.1);
        temperature_spin.set_value(0.7);
        temp_box.append(&temp_label);
        temp_box.append(&temperature_spin);
        container.append(&temp_box);

        Self {
            container,
            api_key_entry,
            model_entry,
            endpoint_entry,
            thinking_switch,
            temperature_spin,
        }
    }
}

// ── OpenCode Zen Config Widgets ────────────────────────────────

struct ZenWidgets {
    container: Box,
    api_key_entry: Entry,
    model_entry: Entry,
    endpoint_entry: Entry,
}

impl ZenWidgets {
    fn new() -> Self {
        let container = Box::new(Orientation::Vertical, 6);

        let api_key_label = Label::new(Some("API Key:"));
        api_key_label.set_halign(gtk4::Align::Start);
        container.append(&api_key_label);

        let api_key_entry = Entry::builder()
            .hexpand(true)
            .input_purpose(gtk4::InputPurpose::Password)
            .visibility(false)
            .build();
        container.append(&api_key_entry);

        let privacy_hint = Label::new(Some("Your API key is stored locally and never shared."));
        privacy_hint.add_css_class("wizard-hint");
        privacy_hint.set_halign(gtk4::Align::Start);
        container.append(&privacy_hint);

        let model_label = Label::new(Some("Model:"));
        model_label.set_halign(gtk4::Align::Start);
        container.append(&model_label);

        let model_entry = Entry::builder()
            .text("deepseek-v4-pro")
            .hexpand(true)
            .build();
        container.append(&model_entry);

        let model_hint = Label::new(Some("50+ models available. 7 free models included."));
        model_hint.add_css_class("wizard-hint");
        model_hint.set_halign(gtk4::Align::Start);
        container.append(&model_hint);

        let endpoint_label = Label::new(Some("Endpoint:"));
        endpoint_label.set_halign(gtk4::Align::Start);
        container.append(&endpoint_label);

        let endpoint_entry = Entry::builder()
            .text("https://opencode.ai/zen/v1")
            .hexpand(true)
            .build();
        container.append(&endpoint_entry);

        let format_hint = Label::new(Some("Auto-detects API format (Chat Completions, Anthropic, Responses)"));
        format_hint.add_css_class("wizard-hint");
        format_hint.set_halign(gtk4::Align::Start);
        container.append(&format_hint);

        Self {
            container,
            api_key_entry,
            model_entry,
            endpoint_entry,
        }
    }
}

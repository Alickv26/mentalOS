use crate::config::{Config, config_path};
use gtk4::prelude::*;
use gtk4::{Box, Button, CheckButton, Entry, Label, Orientation, Window};
use std::fs;
use std::path::PathBuf;

pub struct ConfigWizard;

impl ConfigWizard {
    pub fn show<F>(parent: &impl IsA<gtk4::Window>, on_complete: F)
    where
        F: Fn() + Clone + 'static,
    {
        let dialog = Window::builder()
            .title("Welcome to mentalOS")
            .modal(true)
            .transient_for(parent)
            .default_width(520)
            .default_height(420)
            .resizable(false)
            .build();
        dialog.add_css_class("launcher-window");

        let root = Box::new(Orientation::Vertical, 10);
        root.set_margin_top(16);
        root.set_margin_bottom(16);
        root.set_margin_start(16);
        root.set_margin_end(16);

        let title = Label::new(Some("First-run setup"));
        title.set_halign(gtk4::Align::Start);
        title.add_css_class("app-bar-title");
        root.append(&title);

        let subtitle = Label::new(Some(
            "Configure provider, model, workspace and privacy defaults.",
        ));
        subtitle.set_halign(gtk4::Align::Start);
        subtitle.set_wrap(true);
        subtitle.add_css_class("app-bar-stats");
        root.append(&subtitle);

        let provider_label = Label::new(Some("AI Provider"));
        provider_label.set_halign(gtk4::Align::Start);
        root.append(&provider_label);

        let provider_model = gtk4::StringList::new(&["openclaw", "ollama"]);
        let provider_dd = gtk4::DropDown::builder().model(&provider_model).build();
        provider_dd.set_selected(0);
        root.append(&provider_dd);

        let model_label = Label::new(Some("Model"));
        model_label.set_halign(gtk4::Align::Start);
        root.append(&model_label);

        let model_entry = Entry::builder()
            .placeholder_text("phi3:mini")
            .text("phi3:mini")
            .build();
        root.append(&model_entry);

        let workspace_label = Label::new(Some("Workspace Directory"));
        workspace_label.set_halign(gtk4::Align::Start);
        root.append(&workspace_label);

        let workspace_entry = Entry::builder()
            .placeholder_text("~/workspaces")
            .text("~/workspaces")
            .build();
        root.append(&workspace_entry);

        let token_label = Label::new(Some("OpenClaw Token (optional)"));
        token_label.set_halign(gtk4::Align::Start);
        root.append(&token_label);

        let token_entry = Entry::builder().placeholder_text("Bearer token").build();
        token_entry.set_visibility(false);
        root.append(&token_entry);

        let fallback_check = CheckButton::with_label("Fallback to Ollama if OpenClaw fails");
        fallback_check.set_active(true);
        root.append(&fallback_check);

        let autostart_check = CheckButton::with_label("Auto-start OpenClaw gateway (HTTP mode)");
        autostart_check.set_active(true);
        root.append(&autostart_check);

        let local_only_check = CheckButton::with_label("Local-only memory (no remote sync)");
        local_only_check.set_active(true);
        root.append(&local_only_check);

        let status_label = Label::new(None);
        status_label.set_halign(gtk4::Align::Start);
        status_label.add_css_class("welcome-hint");
        root.append(&status_label);

        let actions = Box::new(Orientation::Horizontal, 8);
        actions.set_halign(gtk4::Align::End);

        let quit_btn = Button::with_label("Quit");
        let save_btn = Button::with_label("Save & Continue");
        save_btn.add_css_class("create-button");

        actions.append(&quit_btn);
        actions.append(&save_btn);
        root.append(&actions);

        dialog.set_child(Some(&root));

        let parent_window = parent.clone().upcast::<gtk4::Window>();
        let dialog_for_quit = dialog.clone();
        quit_btn.connect_clicked(move |_| {
            dialog_for_quit.close();
            parent_window.close();
        });

        let dialog_for_save = dialog.clone();
        let status_for_save = status_label.clone();
        let provider_for_save = provider_dd.clone();
        let model_for_save = model_entry.clone();
        let workspace_for_save = workspace_entry.clone();
        let token_for_save = token_entry.clone();
        let fallback_for_save = fallback_check.clone();
        let autostart_for_save = autostart_check.clone();
        let local_only_for_save = local_only_check.clone();
        let on_complete_cb = on_complete.clone();

        save_btn.connect_clicked(move |_| {
            let provider = selected_provider(&provider_for_save);
            let mut config = Config::default();
            config.ai.provider = provider.clone();
            config.ai.model = model_for_save.text().to_string();
            config.ai.fallback_to_ollama = fallback_for_save.is_active();
            config.paths.workspace_dir = workspace_for_save.text().to_string();
            config.ollama.model = model_for_save.text().to_string();
            config.openclaw.token = token_for_save.text().to_string();
            config.openclaw.auto_start = autostart_for_save.is_active();
            if provider == "openclaw" {
                config.openclaw.transport = "http".to_string();
            }

            match config
                .save()
                .and_then(|_| save_privacy(local_only_for_save.is_active()))
            {
                Ok(_) => {
                    on_complete_cb();
                    dialog_for_save.close();
                }
                Err(err) => {
                    status_for_save.set_text(&format!("Failed to save config: {}", err));
                }
            }
        });

        let key_ctrl = gtk4::EventControllerKey::new();
        let d = dialog.clone();
        let parent_for_escape = parent.clone().upcast::<gtk4::Window>();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                d.close();
                parent_for_escape.close();
                return gtk4::glib::Propagation::Stop;
            }
            gtk4::glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl);

        dialog.present();
    }
}

fn selected_provider(dd: &gtk4::DropDown) -> String {
    dd.selected_item()
        .and_then(|i| i.downcast::<gtk4::StringObject>().ok())
        .map(|s| s.string().to_string())
        .unwrap_or_else(|| "openclaw".to_string())
}

fn save_privacy(local_only: bool) -> crate::Result<()> {
    let config_file = config_path()?;
    let config_dir = config_file
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| crate::MentalOSError::ConfigInvalid("No config directory".to_string()))?;
    fs::create_dir_all(&config_dir)?;

    let privacy_path = config_dir.join("privacy.toml");
    let content = format!(
        "# mentalOS privacy settings\nlocal_only = {}\nallow_sync = {}\n",
        local_only, !local_only
    );
    fs::write(privacy_path, content)?;
    Ok(())
}

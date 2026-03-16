use crate::error::{MentalOSError, Result};
use directories::BaseDirs;
use gtk4::prelude::*;
use gtk4::{Box, Button, Label, Orientation, Window};
use log::warn;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

const ONBOARDING_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OnboardingState {
    #[serde(default)]
    completed: bool,
    #[serde(default)]
    version: u32,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            completed: false,
            version: ONBOARDING_VERSION,
        }
    }
}

struct TutorialStep {
    title: &'static str,
    body: &'static str,
}

const STEPS: [TutorialStep; 5] = [
    TutorialStep {
        title: "Welcome to mentalOS",
        body: "mentalOS is a natural-language control surface for your workspace. Type in the main prompt and press Enter to run requests through the backend router.",
    },
    TutorialStep {
        title: "Safety First",
        body: "Commands pass through whitelist checks. New or risky commands ask for approval first. Use STOP or Ctrl+Shift+Q to immediately halt active operations.",
    },
    TutorialStep {
        title: "Agent Control",
        body: "Use the agent selector near the input to switch between available providers (for example OpenClaw and Ollama). Status and notifications appear in the top bar.",
    },
    TutorialStep {
        title: "Accessibility",
        body: "High contrast: Ctrl+Shift+H. Font size: Ctrl+Plus / Ctrl+Minus / Ctrl+0. Open shortcut help with Ctrl+/ (or Ctrl+?, numpad divide, F1).",
    },
    TutorialStep {
        title: "Customize Your Workflow",
        body: "Start a new chat with Ctrl+N or open shortcut settings with Ctrl+Comma to customize bindings. Memory and tasks are persisted under ~/workspaces/.memory for continuity across sessions.",
    },
];

pub struct OnboardingTutorial;

impl OnboardingTutorial {
    pub fn should_show() -> bool {
        match load_state() {
            Ok(state) => !state.completed || state.version < ONBOARDING_VERSION,
            Err(err) => {
                warn!("Failed to load onboarding state, showing tutorial: {}", err);
                true
            }
        }
    }

    pub fn show(parent: &impl IsA<gtk4::Window>) {
        let dialog = Window::builder()
            .title("mentalOS Onboarding")
            .modal(true)
            .transient_for(parent)
            .default_width(620)
            .default_height(420)
            .resizable(false)
            .build();
        dialog.add_css_class("launcher-window");

        let root = Box::new(Orientation::Vertical, 10);
        root.set_margin_top(16);
        root.set_margin_bottom(16);
        root.set_margin_start(16);
        root.set_margin_end(16);

        let step_label = Label::new(None);
        step_label.set_halign(gtk4::Align::Start);
        step_label.add_css_class("app-bar-stats");
        root.append(&step_label);

        let title_label = Label::new(None);
        title_label.set_halign(gtk4::Align::Start);
        title_label.add_css_class("app-bar-title");
        root.append(&title_label);

        let body_label = Label::new(None);
        body_label.set_halign(gtk4::Align::Start);
        body_label.set_wrap(true);
        body_label.add_css_class("message-content");
        root.append(&body_label);

        let actions = Box::new(Orientation::Horizontal, 8);
        actions.set_halign(gtk4::Align::End);
        let skip_btn = Button::with_label("Skip");
        let back_btn = Button::with_label("Back");
        let next_btn = Button::with_label("Next");
        let done_btn = Button::with_label("Done");
        done_btn.add_css_class("create-button");
        actions.append(&skip_btn);
        actions.append(&back_btn);
        actions.append(&next_btn);
        actions.append(&done_btn);
        root.append(&actions);

        dialog.set_child(Some(&root));

        let index = Rc::new(Cell::new(0_usize));
        let index_for_update = index.clone();
        let step_label_ref = step_label.clone();
        let title_label_ref = title_label.clone();
        let body_label_ref = body_label.clone();
        let back_btn_ref = back_btn.clone();
        let next_btn_ref = next_btn.clone();
        let done_btn_ref = done_btn.clone();
        let update_ui = Rc::new(move || {
            let i = index_for_update.get().min(STEPS.len().saturating_sub(1));
            let step = &STEPS[i];
            step_label_ref.set_text(&step_progress_text(i, STEPS.len()));
            title_label_ref.set_text(step.title);
            body_label_ref.set_text(step.body);
            back_btn_ref.set_sensitive(i > 0);
            next_btn_ref.set_sensitive(i + 1 < STEPS.len());
            next_btn_ref.set_visible(i + 1 < STEPS.len());
            done_btn_ref.set_visible(i + 1 == STEPS.len());
        });
        update_ui();

        let index_for_back = index.clone();
        let update_for_back = update_ui.clone();
        back_btn.connect_clicked(move |_| {
            let i = index_for_back.get();
            if i > 0 {
                index_for_back.set(i - 1);
            }
            update_for_back();
        });

        let index_for_next = index.clone();
        let update_for_next = update_ui.clone();
        next_btn.connect_clicked(move |_| {
            let i = index_for_next.get();
            if i + 1 < STEPS.len() {
                index_for_next.set(i + 1);
            }
            update_for_next();
        });

        let dialog_for_skip = dialog.clone();
        skip_btn.connect_clicked(move |_| {
            if let Err(err) = mark_completed() {
                warn!(
                    "Failed to persist onboarding completion after skip: {}",
                    err
                );
            }
            dialog_for_skip.close();
        });

        let dialog_for_done = dialog.clone();
        done_btn.connect_clicked(move |_| {
            if let Err(err) = mark_completed() {
                warn!(
                    "Failed to persist onboarding completion after done: {}",
                    err
                );
            }
            dialog_for_done.close();
        });

        let key_ctrl = gtk4::EventControllerKey::new();
        let d = dialog.clone();
        let index_for_key = index.clone();
        let update_for_key = update_ui.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                d.close();
                return gtk4::glib::Propagation::Stop;
            }
            if key == gtk4::gdk::Key::Right {
                let i = index_for_key.get();
                if i + 1 < STEPS.len() {
                    index_for_key.set(i + 1);
                    update_for_key();
                }
                return gtk4::glib::Propagation::Stop;
            }
            if key == gtk4::gdk::Key::Left {
                let i = index_for_key.get();
                if i > 0 {
                    index_for_key.set(i - 1);
                    update_for_key();
                }
                return gtk4::glib::Propagation::Stop;
            }
            gtk4::glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl);

        dialog.present();
    }
}

fn onboarding_path() -> Result<PathBuf> {
    let base = BaseDirs::new().ok_or_else(|| MentalOSError::ConfigInvalid("No home dir".into()))?;
    Ok(base.config_dir().join("mentalOS").join("onboarding.toml"))
}

fn load_state() -> Result<OnboardingState> {
    let path = onboarding_path()?;
    if !path.exists() {
        return Ok(OnboardingState::default());
    }
    let text = fs::read_to_string(path)?;
    let mut state: OnboardingState = toml::from_str(&text)?;
    if state.version == 0 {
        state.version = ONBOARDING_VERSION;
    }
    Ok(state)
}

fn mark_completed() -> Result<PathBuf> {
    let path = onboarding_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let state = OnboardingState {
        completed: true,
        version: ONBOARDING_VERSION,
    };
    let serialized = toml::to_string_pretty(&state).map_err(|err| {
        MentalOSError::ConfigInvalid(format!("Failed to serialize onboarding: {err}"))
    })?;
    fs::write(&path, serialized)?;
    Ok(path)
}

fn step_progress_text(index: usize, total: usize) -> String {
    let step = index.saturating_add(1).min(total.max(1));
    let pct = ((step as f64 / total.max(1) as f64) * 100.0).round() as u32;
    format!("Step {step}/{total} ({pct}%)")
}

#[cfg(test)]
mod tests {
    use super::step_progress_text;

    #[test]
    fn progress_text_includes_percentage() {
        assert_eq!(step_progress_text(0, 5), "Step 1/5 (20%)");
        assert_eq!(step_progress_text(4, 5), "Step 5/5 (100%)");
    }
}

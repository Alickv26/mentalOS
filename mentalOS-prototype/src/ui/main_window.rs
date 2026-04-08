use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Orientation};
use log::{error, info, warn};
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use tokio::sync::mpsc::Sender;

use crate::memory::{MemoryManager, default_workspace_root};
use crate::ui::app_bar::AppBar;
use crate::ui::approval::{ApprovalDecision, ApprovalDialog};
use crate::ui::chat_view::{ChatView, MessageRole};
use crate::ui::launcher::{AppLauncher, launch_terminal};
use crate::ui::memory_browser::MemoryBrowser;
use crate::ui::messages::{BackendRequest, BackendResponse};
use crate::ui::notifications::NotificationCenter;
use crate::ui::omni_pill::{AiState, OmniPill};
use crate::ui::onboarding_tutorial::OnboardingTutorial;
use crate::ui::project_dialog::{ProjectDecision, ProjectDialog};
use crate::ui::shortcuts::ShortcutBindings;
use crate::ui::shortcuts_help::ShortcutsHelp;
use crate::ui::shortcuts_settings::ShortcutsSettings;

/// The main mentalOS overlay window.
pub struct MainWindow {
    pub window: ApplicationWindow,
}

impl MainWindow {
    pub fn new(
        app: &Application,
        backend_tx: Sender<BackendRequest>,
    ) -> (Self, async_channel::Sender<BackendResponse>) {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("mentalOS AIUI")
            .default_width(600)
            .default_height(120) // Increased for margins
            .decorated(false)
            .resizable(false)
            .build();

        window.add_css_class("transparent-window");

        let root_container = Box::new(Orientation::Vertical, 0);
        root_container.add_css_class("omni-container");
        root_container.add_css_class("state-sleep");
        root_container.add_css_class("font-medium");

        // Add margins to prevent shadow clipping
        root_container.set_margin_top(24);
        root_container.set_margin_bottom(24);
        root_container.set_margin_start(24);
        root_container.set_margin_end(24);

        // ── App Bar ──
        let app_bar = AppBar::new();
        root_container.append(&app_bar.container);

        // ── Notifications ──
        let notifications = NotificationCenter::new();
        root_container.append(&notifications.container);

        // ── Chat View ──
        let chat_view = ChatView::new();
        chat_view.container.set_visible(false);
        chat_view.container.set_height_request(400);
        root_container.append(&chat_view.container);

        // ── Omni Pill (Input) ──
        let pill = OmniPill::new();
        root_container.append(&pill.container);

        window.set_child(Some(&root_container));

        // ── Logic Wiring ──

        let pill = Rc::new(pill);
        let chat_view = Rc::new(chat_view);
        let root_container = Rc::new(root_container);
        let app_bar = Rc::new(app_bar);
        let notifications = Rc::new(notifications);
        let shortcut_bindings = Rc::new(RefCell::new(
            ShortcutBindings::load_or_default().unwrap_or_else(|err| {
                warn!("Failed to load shortcuts, using defaults: {}", err);
                ShortcutBindings::default()
            }),
        ));

        // ── Wire Buttons ──
        let win_for_apps = window.clone();
        pill.apps_btn.connect_clicked(move |_| {
            AppLauncher::show(&win_for_apps);
        });

        let chat_for_term = chat_view.clone();
        let win_for_term = window.clone();
        pill.term_btn
            .connect_clicked(move |_| match launch_terminal() {
                Ok(_) => info!("Terminal launched"),
                Err(err) => {
                    chat_for_term.container.set_visible(true);
                    win_for_term.set_default_height(500);
                    chat_for_term.append_message(MessageRole::System, &format!("Error: {}", err));
                    log::warn!("Terminal launch failed: {}", err);
                }
            });

        let tx_for_stop_from_pill = backend_tx.clone();
        let app_bar_ref = app_bar.clone();
        let pill_ref_for_stop = pill.clone();
        let root_ref_for_stop = root_container.clone();
        pill.connect_stop_clicked(move |_| {
            let _ = tx_for_stop_from_pill.blocking_send(BackendRequest::EmergencyStop);
            app_bar_ref.set_status("Stopped");
            app_bar_ref.set_busy(false);
            set_visual_state(&root_ref_for_stop, &pill_ref_for_stop, AiState::Sleep);
        });

        let tx_for_stop_from_bar = backend_tx.clone();
        let app_bar_ref = app_bar.clone();
        let pill_ref_for_stop = pill.clone();
        let root_ref_for_stop = root_container.clone();
        app_bar.connect_stop_clicked(move |_| {
            let _ = tx_for_stop_from_bar.blocking_send(BackendRequest::EmergencyStop);
            app_bar_ref.set_status("Stopped");
            app_bar_ref.set_busy(false);
            set_visual_state(&root_ref_for_stop, &pill_ref_for_stop, AiState::Sleep);
        });

        let win_for_memory = window.clone();
        let chat_for_memory = chat_view.clone();
        let input_for_memory = pill.input.clone();
        let app_bar_for_memory = app_bar.clone();
        app_bar.connect_memory_clicked(move |_| {
            let win_ref = win_for_memory.clone();
            let chat_ref = chat_for_memory.clone();
            let input_ref = input_for_memory.clone();
            let bar_ref = app_bar_for_memory.clone();
            MemoryBrowser::show(&win_for_memory, "default", move |conversation, summary| {
                chat_ref.container.set_visible(true);
                win_ref.set_default_height(500);
                chat_ref.load_conversation(&conversation);
                let label = summary.title.as_deref().unwrap_or(&summary.session_id);
                bar_ref.set_current_session(label);
                input_ref.grab_focus();
            });
        });

        let chat_for_new = chat_view.clone();
        let input_for_new = pill.input.clone();
        let win_for_new = window.clone();
        let app_bar_for_new = app_bar.clone();
        let pill_for_new = pill.clone();
        let root_for_new = root_container.clone();
        let notifications_for_new = notifications.clone();
        let start_new_chat = Rc::new(move || {
            let manager = MemoryManager::new(default_workspace_root());
            if let Err(err) = manager.start_new_session("default", "general") {
                warn!("Failed to start new session: {}", err);
                notifications_for_new.show("Unable to start a new chat session.");
            }

            chat_for_new.clear();
            chat_for_new.container.set_visible(true);
            win_for_new.set_default_height(500);
            set_visual_state(&root_for_new, &pill_for_new, AiState::Sleep);
            app_bar_for_new.set_status("Idle");
            app_bar_for_new.set_busy(false);
            app_bar_for_new.clear_current_session();
            input_for_new.grab_focus();
        });
        let start_new_chat_btn = start_new_chat.clone();
        app_bar.connect_new_chat_clicked(move |_| {
            (start_new_chat_btn)();
        });

        // ── Create UI Channel Here (Avoids naming Receiver type) ──
        let (ui_tx, ui_rx) = async_channel::unbounded::<BackendResponse>();

        // ── Handle Incoming Backend Messages ──
        let chat_ref = chat_view.clone();
        let pill_ref = pill.clone();
        let root_ref = root_container.clone();
        let win_ref = window.clone();
        let backend_tx_clone = backend_tx.clone();
        let app_bar_ref = app_bar.clone();
        let notifications_ref = notifications.clone();
        let agent_selector_syncing = Rc::new(Cell::new(false));
        let agent_selector_syncing_for_rx = agent_selector_syncing.clone();

        glib::MainContext::default().spawn_local(async move {
            while let Ok(msg) = ui_rx.recv().await {
                match msg {
                    BackendResponse::Status(text) => {
                        notifications_ref.show(&text);
                        app_bar_ref.set_status("Active");
                        app_bar_ref.set_busy(true);
                    }
                    BackendResponse::Chat(text) => {
                        chat_ref.append_message(MessageRole::Ai, &text);
                        set_visual_state(&root_ref, &pill_ref, AiState::Sleep);
                        app_bar_ref.set_status("Idle");
                        app_bar_ref.set_busy(false);
                    }
                    BackendResponse::CommandResult(output) => {
                        let text = format!(
                            "Executed: `{}`\nExit Code: {}\nOutput:\n```\n{}\n```",
                            output.command, output.exit_code, output.stdout
                        );
                        chat_ref.append_message(MessageRole::System, &text);
                        set_visual_state(&root_ref, &pill_ref, AiState::Sleep);
                        app_bar_ref.set_status("Idle");
                        app_bar_ref.set_busy(false);
                    }
                    BackendResponse::ApprovalRequired(cmd) => {
                        set_visual_state(&root_ref, &pill_ref, AiState::Learn);
                        app_bar_ref.set_status("Approval");
                        app_bar_ref.set_busy(false);
                        let tx = backend_tx_clone.clone();
                        let cmd_clone = cmd.clone();
                        ApprovalDialog::show(&win_ref, &cmd, move |decision| match decision {
                            ApprovalDecision::ApproveOnce => {
                                let _ = tx.blocking_send(BackendRequest::ExecuteCommand(
                                    cmd_clone.clone(),
                                ));
                            }
                            _ => {
                                info!("Command denied: {}", cmd_clone);
                            }
                        });
                    }
                    BackendResponse::Error(err) => {
                        notifications_ref.show_error(&err);
                        chat_ref.append_message(MessageRole::System, &format!("Error: {}", err));
                        set_visual_state(&root_ref, &pill_ref, AiState::Sleep);
                        app_bar_ref.set_status("Error");
                        app_bar_ref.set_busy(false);
                    }
                    BackendResponse::AgentList { agents, current } => {
                        update_agent_list(
                            &pill_ref.agent_selector,
                            &agents,
                            &current,
                            &agent_selector_syncing_for_rx,
                        );
                    }
                    BackendResponse::AgentSwitched(name) => {
                        info!("Switched active agent to: {}", name);
                    }
                    BackendResponse::ProjectConfirmationRequired {
                        name,
                        language,
                        framework,
                    } => {
                        let tx = backend_tx_clone.clone();
                        let name_clone = name.clone();
                        let lang_clone = language.clone();
                        let fw_clone = framework.clone();
                        ProjectDialog::show(
                            &win_ref,
                            &name,
                            language.as_deref(),
                            framework.as_deref(),
                            move |decision| match decision {
                                ProjectDecision::Create => {
                                    let _ = tx.blocking_send(BackendRequest::CreateProject {
                                        name: name_clone.clone(),
                                        language: lang_clone.clone(),
                                        framework: fw_clone.clone(),
                                    });
                                }
                                ProjectDecision::Cancel => {
                                    info!("Project creation cancelled: {}", name_clone);
                                }
                            },
                        );
                    }
                    BackendResponse::ProjectCreated {
                        success,
                        path,
                        message,
                    } => {
                        let text = if success {
                            format!("Project created: {}", message)
                        } else {
                            format!("Project creation failed: {}", message)
                        };
                        chat_ref.append_message(MessageRole::System, &text);
                        if success {
                            if let Some(project_path) = path {
                                let manager = MemoryManager::new(default_workspace_root());
                                match manager.find_related_session_for_project(
                                    "default",
                                    Path::new(&project_path),
                                ) {
                                    Ok(Some(session)) => {
                                        let label =
                                            session.title.as_deref().unwrap_or(&session.session_id);
                                        app_bar_ref.set_current_session(label);
                                        notifications_ref.show(&format!(
                                            "Linked project to conversation: {label}"
                                        ));
                                    }
                                    Ok(None) => {}
                                    Err(err) => {
                                        warn!("Failed to resolve related session: {}", err);
                                    }
                                }
                            }
                        }
                        set_visual_state(&root_ref, &pill_ref, AiState::Sleep);
                        app_bar_ref.set_status("Idle");
                        app_bar_ref.set_busy(false);
                    }
                }
            }
        });

        // ── Input submit (Enter key) ──
        let pill_input = pill.input.clone();
        let chat_ref = chat_view.clone();
        let root_ref = root_container.clone();
        let win_ref = window.clone();
        let pill_ref = pill.clone();
        let app_bar_for_input = app_bar.clone();
        let backend_tx_for_input = backend_tx.clone();

        pill_input.connect_activate(move |entry| {
            let text = entry.text().to_string();
            if text.trim().is_empty() {
                return;
            }
            entry.set_text("");

            chat_ref.container.set_visible(true);
            win_ref.set_default_height(500);
            set_visual_state(&root_ref, &pill_ref, AiState::Active);
            app_bar_for_input.set_status("Active");
            app_bar_for_input.set_busy(true);
            chat_ref.append_message(MessageRole::User, &text);

            let req = BackendRequest::Input {
                text,
                workspace: "default".into(),
                category: "general".into(),
            };

            if let Err(e) = backend_tx_for_input.blocking_send(req) {
                error!("Failed to send to backend: {}", e);
                chat_ref.append_message(MessageRole::System, "Error: Backend unreachable");
                set_visual_state(&root_ref, &pill_ref, AiState::Sleep);
                app_bar_for_input.set_status("Error");
                app_bar_for_input.set_busy(false);
            }
        });

        // ── Agent Selector Logic ──
        let pill_selector = pill.agent_selector.clone();
        let backend_tx_for_select = backend_tx.clone();
        let agent_selector_syncing_for_select = agent_selector_syncing.clone();

        pill_selector.connect_selected_notify(move |dropdown| {
            if agent_selector_syncing_for_select.get() {
                return;
            }
            let selected_item = dropdown.selected_item();
            if let Some(item) = selected_item {
                if let Some(string_obj) = item.downcast_ref::<gtk4::StringObject>() {
                    let name = string_obj.string().to_string();
                    if name == "Loading..." {
                        return;
                    }
                    info!("Agent selected: {}", name);
                    let _ = backend_tx_for_select.blocking_send(BackendRequest::SwitchAgent(name));
                }
            }
        });

        // ── Initial Fetch ──
        let _ = backend_tx.blocking_send(BackendRequest::GetAgents);

        // ── Shortcuts ──
        let key_ctrl = gtk4::EventControllerKey::new();
        let win_for_keys = window.clone();
        let input_for_keys = pill.input.clone();
        let tx_for_keys = backend_tx.clone();
        let app_bar_for_keys = app_bar.clone();
        let pill_for_keys = pill.clone();
        let root_for_keys = root_container.clone();
        let notifications_for_keys = notifications.clone();
        let high_contrast_enabled = Rc::new(Cell::new(false));
        let high_contrast_for_keys = high_contrast_enabled.clone();
        let font_scale_step = Rc::new(Cell::new(1_i32));
        let font_scale_for_keys = font_scale_step.clone();
        let shortcuts_for_keys = shortcut_bindings.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, modifiers| {
            let bindings_snapshot = shortcuts_for_keys.borrow().clone();

            if bindings_snapshot.matches("manage_shortcuts", key, modifiers) {
                let bindings_store = shortcuts_for_keys.clone();
                let notifications_for_save = notifications_for_keys.clone();
                let current = bindings_snapshot.clone();
                ShortcutsSettings::show(&win_for_keys, current, move |updated| {
                    *bindings_store.borrow_mut() = updated;
                    notifications_for_save.show("Shortcuts updated");
                });
                return glib::Propagation::Stop;
            }

            if bindings_snapshot.matches("show_onboarding", key, modifiers) {
                OnboardingTutorial::show(&win_for_keys);
                return glib::Propagation::Stop;
            }

            if bindings_snapshot.matches("toggle_high_contrast", key, modifiers) {
                let enabled = !high_contrast_for_keys.get();
                high_contrast_for_keys.set(enabled);
                set_high_contrast(&root_for_keys, enabled);
                if enabled {
                    notifications_for_keys.show("High contrast mode: ON");
                } else {
                    notifications_for_keys.show("High contrast mode: OFF");
                }
                return glib::Propagation::Stop;
            }

            if bindings_snapshot.matches("font_increase", key, modifiers) {
                let step = (font_scale_for_keys.get() + 1).clamp(0, 2);
                font_scale_for_keys.set(step);
                notifications_for_keys.show(apply_font_scale(&root_for_keys, step));
                return glib::Propagation::Stop;
            }

            if bindings_snapshot.matches("font_decrease", key, modifiers) {
                let step = (font_scale_for_keys.get() - 1).clamp(0, 2);
                font_scale_for_keys.set(step);
                notifications_for_keys.show(apply_font_scale(&root_for_keys, step));
                return glib::Propagation::Stop;
            }

            if bindings_snapshot.matches("font_reset", key, modifiers) {
                font_scale_for_keys.set(1);
                notifications_for_keys.show(apply_font_scale(&root_for_keys, 1));
                return glib::Propagation::Stop;
            }

            if bindings_snapshot.matches("open_launcher", key, modifiers) {
                AppLauncher::show(&win_for_keys);
                return glib::Propagation::Stop;
            }

            if bindings_snapshot.matches("open_memory_browser", key, modifiers) {
                let chat_ref = chat_view.clone();
                let win_ref = win_for_keys.clone();
                let input_ref = input_for_keys.clone();
                let bar_ref = app_bar_for_keys.clone();
                MemoryBrowser::show(&win_for_keys, "default", move |conversation, summary| {
                    chat_ref.container.set_visible(true);
                    win_ref.set_default_height(500);
                    chat_ref.load_conversation(&conversation);
                    let label = summary.title.as_deref().unwrap_or(&summary.session_id);
                    bar_ref.set_current_session(label);
                    input_ref.grab_focus();
                });
                return glib::Propagation::Stop;
            }

            if bindings_snapshot.matches("new_chat", key, modifiers) {
                (start_new_chat)();
                return glib::Propagation::Stop;
            }

            if bindings_snapshot.matches("show_help", key, modifiers)
                || is_help_fallback_shortcut(key, modifiers)
            {
                ShortcutsHelp::show(&win_for_keys, &bindings_snapshot);
                return glib::Propagation::Stop;
            }

            if bindings_snapshot.matches("focus_input", key, modifiers) {
                input_for_keys.grab_focus();
                return glib::Propagation::Stop;
            }

            if bindings_snapshot.matches("emergency_stop", key, modifiers) {
                let _ = tx_for_keys.blocking_send(BackendRequest::EmergencyStop);
                app_bar_for_keys.set_status("Stopped");
                app_bar_for_keys.set_busy(false);
                set_visual_state(&root_for_keys, &pill_for_keys, AiState::Sleep);
                return glib::Propagation::Stop;
            }
            if key == gtk4::gdk::Key::Escape {
                win_for_keys.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(key_ctrl);

        pill.input.grab_focus();

        (Self { window }, ui_tx)
    }
}

// Helper to update dropdown model safely
fn update_agent_list(
    dropdown: &gtk4::DropDown,
    agents: &[String],
    current: &str,
    syncing: &Rc<Cell<bool>>,
) {
    syncing.set(true);
    let list = gtk4::StringList::new(&agents.iter().map(|s| s.as_str()).collect::<Vec<&str>>());
    dropdown.set_model(Some(&list));

    // Find index of current
    if let Some(idx) = agents.iter().position(|r| r.eq_ignore_ascii_case(current)) {
        dropdown.set_selected(idx as u32);
    }
    syncing.set(false);
}

fn set_visual_state(container: &Box, pill_ui: &OmniPill, state: AiState) {
    container.remove_css_class("state-sleep");
    container.remove_css_class("state-active");
    container.remove_css_class("state-agentic");
    container.remove_css_class("state-learn");
    container.add_css_class(state.css_class());
    pill_ui.set_state(state);
}

fn set_high_contrast(container: &Box, enabled: bool) {
    if enabled {
        container.add_css_class("high-contrast");
    } else {
        container.remove_css_class("high-contrast");
    }
}

fn apply_font_scale(container: &Box, step: i32) -> &'static str {
    container.remove_css_class("font-small");
    container.remove_css_class("font-medium");
    container.remove_css_class("font-large");
    match step {
        0 => {
            container.add_css_class("font-small");
            "Font size: small"
        }
        2 => {
            container.add_css_class("font-large");
            "Font size: large"
        }
        _ => {
            container.add_css_class("font-medium");
            "Font size: normal"
        }
    }
}

fn is_help_fallback_shortcut(key: gtk4::gdk::Key, modifiers: gtk4::gdk::ModifierType) -> bool {
    if !modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
        return key == gtk4::gdk::Key::F1;
    }
    key == gtk4::gdk::Key::slash
        || key == gtk4::gdk::Key::question
        || key == gtk4::gdk::Key::KP_Divide
        || key
            .to_unicode()
            .map(|c| c == '/' || c == '?')
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::is_help_fallback_shortcut;
    use gtk4::gdk;

    #[test]
    fn help_fallback_accepts_f1_without_ctrl() {
        assert!(is_help_fallback_shortcut(
            gdk::Key::F1,
            gdk::ModifierType::empty()
        ));
    }

    #[test]
    fn help_fallback_accepts_ctrl_slash_variants() {
        let ctrl = gdk::ModifierType::CONTROL_MASK;
        assert!(is_help_fallback_shortcut(gdk::Key::slash, ctrl));
        assert!(is_help_fallback_shortcut(
            gdk::Key::question,
            ctrl | gdk::ModifierType::SHIFT_MASK,
        ));
        assert!(is_help_fallback_shortcut(gdk::Key::KP_Divide, ctrl));
    }

    #[test]
    fn help_fallback_rejects_invalid_combinations() {
        assert!(!is_help_fallback_shortcut(
            gdk::Key::slash,
            gdk::ModifierType::empty()
        ));
        assert!(!is_help_fallback_shortcut(
            gdk::Key::F1,
            gdk::ModifierType::CONTROL_MASK
        ));
        assert!(!is_help_fallback_shortcut(
            gdk::Key::a,
            gdk::ModifierType::CONTROL_MASK
        ));
    }
}

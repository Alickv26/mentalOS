use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Orientation};
use log::{error, info};
use std::rc::Rc;
use tokio::sync::mpsc::Sender;

use crate::ui::approval::{ApprovalDialog, ApprovalDecision};
use crate::ui::chat_view::{ChatView, MessageRole};
use crate::ui::launcher::AppLauncher;
use crate::ui::messages::{BackendRequest, BackendResponse};
use crate::ui::omni_pill::{AiState, OmniPill};

/// The main mentalOS overlay window.
pub struct MainWindow {
    pub window: ApplicationWindow,
}

impl MainWindow {
    pub fn new(
        app: &Application,
        backend_tx: Sender<BackendRequest>,
    ) -> (Self, glib::Sender<BackendResponse>) {
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
        
        // Add margins to prevent shadow clipping
        root_container.set_margin_top(24);
        root_container.set_margin_bottom(24);
        root_container.set_margin_start(24);
        root_container.set_margin_end(24);

        // ── Omni Pill (Input) ──
        let pill = OmniPill::new();
        root_container.append(&pill.container);

        // ── Chat View ──
        let chat_view = ChatView::new();
        chat_view.container.set_visible(false); 
        chat_view.container.set_height_request(400); 
        root_container.append(&chat_view.container);

        window.set_child(Some(&root_container));

        // ── Logic Wiring ──

        let pill = Rc::new(pill);
        let chat_view = Rc::new(chat_view);
        let root_container = Rc::new(root_container); 

        // ── Wire Buttons ──
        let win_for_apps = window.clone();
        pill.apps_btn.connect_clicked(move |_| {
            AppLauncher::show(&win_for_apps);
        });

        let _pill_term_ref = pill.clone();
        pill.term_btn.connect_clicked(move |_| {
             // Future: Toggle Terminal View
             info!("Terminal button clicked");
        });

        // ── Create UI Channel Here (Avoids naming Receiver type) ──
        let (ui_tx, ui_rx) = glib::MainContext::channel(glib::Priority::DEFAULT);

        // ── Handle Incoming Backend Messages ──
        let chat_ref = chat_view.clone();
        let pill_ref = pill.clone();
        let root_ref = root_container.clone();
        let win_ref = window.clone();
        let backend_tx_clone = backend_tx.clone();

        ui_rx.attach(None, move |msg| {
            match msg {
                BackendResponse::Chat(text) => {
                    chat_ref.append_message(MessageRole::Ai, &text);
                    set_visual_state(&root_ref, &pill_ref, AiState::Sleep);
                }
                BackendResponse::CommandResult(output) => {
                    let text = format!("Executed: `{}`\nExit Code: {}\nOutput:\n```\n{}\n```", output.command, output.exit_code, output.stdout);
                    chat_ref.append_message(MessageRole::System, &text);
                    set_visual_state(&root_ref, &pill_ref, AiState::Sleep);
                }
                BackendResponse::ApprovalRequired(cmd) => {
                    set_visual_state(&root_ref, &pill_ref, AiState::Learn); 
                    let tx = backend_tx_clone.clone();
                    let cmd_clone = cmd.clone();
                    ApprovalDialog::show(&win_ref, &cmd, move |decision| {
                         match decision {
                             ApprovalDecision::ApproveOnce => {
                                 let _ = tx.blocking_send(BackendRequest::ExecuteCommand(cmd_clone.clone()));
                             }
                             _ => {
                                 info!("Command denied: {}", cmd_clone);
                             }
                         }
                    });
                }
                BackendResponse::Error(err) => {
                    chat_ref.append_message(MessageRole::System, &format!("Error: {}", err));
                    set_visual_state(&root_ref, &pill_ref, AiState::Sleep);
                }
            }
            glib::ControlFlow::Continue
        });
        
        // ── Input submit (Enter key) ──
        let pill_input = pill.input.clone();
        let chat_ref = chat_view.clone();
        let root_ref = root_container.clone();
        let win_ref = window.clone();
        let pill_ref = pill.clone();

        pill_input.connect_activate(move |entry| {
            let text = entry.text().to_string();
            if text.trim().is_empty() {
                return;
            }
            entry.set_text("");
            
            chat_ref.container.set_visible(true);
            win_ref.set_default_height(500); 
            set_visual_state(&root_ref, &pill_ref, AiState::Active);
            chat_ref.append_message(MessageRole::User, &text);

            let req = BackendRequest::Input {
                text,
                workspace: "default".into(),
                category: "general".into(),
            };
            
            if let Err(e) = backend_tx.blocking_send(req) {
                error!("Failed to send to backend: {}", e);
                chat_ref.append_message(MessageRole::System, "Error: Backend unreachable");
                set_visual_state(&root_ref, &pill_ref, AiState::Sleep);
            }
        });

        // Shortcuts
        let key_ctrl = gtk4::EventControllerKey::new();
        let win_for_keys = window.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, modifiers| {
             let ctrl = modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
             if ctrl && key == gtk4::gdk::Key::k {
                AppLauncher::show(&win_for_keys);
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

fn set_visual_state(container: &Box, pill_ui: &OmniPill, state: AiState) {
    container.remove_css_class("state-sleep");
    container.remove_css_class("state-active");
    container.remove_css_class("state-agentic");
    container.remove_css_class("state-learn");
    container.add_css_class(state.css_class());
    pill_ui.set_state(state);
}

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Orientation};
use log::info;

use crate::ui::chat_view::{ChatView, MessageRole};
use crate::ui::launcher::AppLauncher;
use crate::ui::omni_pill::{AiState, OmniPill};
use std::rc::Rc;

/// The main mentalOS overlay window.
pub struct MainWindow {
    pub window: ApplicationWindow,
    // We don't need to store the components in the struct for this prototype
    // as long as they are attached to the window hierarchy.
}

impl MainWindow {
    pub fn new(app: &Application) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("mentalOS AIUI")
            .default_width(600)
            .default_height(80) 
            .decorated(false) // Frameless
            .resizable(false)
            // .always_on_top(true) // Commented out for dev comfort, uncomment for prod
            .build();
        
        // Transparent background for the window itself
        window.add_css_class("transparent-window");

        let root_container = Box::new(Orientation::Vertical, 0);
        root_container.add_css_class("omni-container");
        root_container.add_css_class("state-sleep"); // Default state

        // ── Omni Pill (Input) ──
        let pill = OmniPill::new();
        root_container.append(&pill.container);

        // ── Chat View (Hidden by default) ──
        let chat_view = ChatView::new();
        chat_view.container.set_visible(false); 
        chat_view.container.set_height_request(400); 
        root_container.append(&chat_view.container);

        window.set_child(Some(&root_container));

        // ── Logic Wiring ──

        // We use Rc to share access to the UI components between callbacks
        // GTK widgets are reference counted, but our wrapper structs (OmniPill, ChatView) are not,
        // so we wrap them in Rc to share them safely in this single-threaded environment.
        let pill = Rc::new(pill);
        let chat_view = Rc::new(chat_view);
        
        // Input submit (Enter key)
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
            info!("User input: {text}");

            // Expand UI
            chat_ref.container.set_visible(true);
            win_ref.set_default_height(500); // Expand window
            
            // Set State: Active (Thinking)
            set_visual_state(&root_ref, &pill_ref, AiState::Active);

            // Append user message
            chat_ref.append_message(MessageRole::User, &text);

            // Mock Mock Mock
            let chat_clone = chat_ref.clone();
            let root_clone = root_ref.clone();
            let pill_clone = pill_ref.clone();
            
            glib::timeout_add_local_once(std::time::Duration::from_millis(800), move || {
                let response = generate_mock_response(&text);
                
                // If it's a "create/agent" command, simulate Agentic state
                if text.to_lowercase().contains("create") || text.to_lowercase().contains("agent") {
                     set_visual_state(&root_clone, &pill_clone, AiState::Agentic);
                     
                     // Simulate agent working then finishing
                     let chat_final = chat_clone.clone();
                     let root_final = root_clone.clone();
                     let pill_final = pill_clone.clone();
                     glib::timeout_add_local_once(std::time::Duration::from_secs(3), move || {
                          chat_final.append_message(MessageRole::Ai, &response);
                          set_visual_state(&root_final, &pill_final, AiState::Sleep);
                     });
                } else {
                    chat_clone.append_message(MessageRole::Ai, &response);
                    set_visual_state(&root_clone, &pill_clone, AiState::Sleep);
                }
            });
        });

        // Close/Collapse logic (Esc)
        let key_ctrl = gtk4::EventControllerKey::new();
        let win_for_keys = window.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, modifiers| {
             let ctrl = modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
             
             if ctrl && key == gtk4::gdk::Key::k {
                AppLauncher::show(&win_for_keys);
                return glib::Propagation::Stop;
             }
             
             if key == gtk4::gdk::Key::Escape {
                 // Minimal collapse logic for prototype
                 win_for_keys.close();
                 return glib::Propagation::Stop;
             }
             glib::Propagation::Proceed
        });
        window.add_controller(key_ctrl);

        // Focus input
        pill.input.grab_focus();

        Self {
            window,
        }
    }
}

// ── State Management Helper ──

fn set_visual_state(container: &Box, pill_ui: &OmniPill, state: AiState) {
    // Remove all state classes
    container.remove_css_class("state-sleep");
    container.remove_css_class("state-active");
    container.remove_css_class("state-agentic");
    container.remove_css_class("state-learn");

    // Add new state class
    container.add_css_class(state.css_class());
    
    // Update Icon
    pill_ui.set_state(state);
}

fn generate_mock_response(user_input: &str) -> String {
    let lower = user_input.to_lowercase();
    if lower.contains("create") {
         return "I'm generating that project for you now...\n```bash\ncargo new mental-os-v2\n```".to_string();
    }
    if lower.contains("hello") {
        return "Hello! I am ready.".to_string();
    }
    format!("I heard: \"{}\"", user_input)
}

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Box, Button, Entry, Orientation};
use log::info;

use crate::ui::app_bar::AppBar;
use crate::ui::chat_view::{ChatView, MessageRole};
use crate::ui::launcher::AppLauncher;

/// The main mentalOS window that assembles all UI components.
pub struct MainWindow {
    pub window: ApplicationWindow,
}

impl MainWindow {
    pub fn new(app: &Application) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("mentalOS")
            .default_width(800)
            .default_height(600)
            .build();

        let root = Box::new(Orientation::Vertical, 0);

        // ── App Bar ──
        let app_bar = AppBar::new();
        root.append(&app_bar.container);

        // ── Chat View ──
        let chat_view = ChatView::new();
        root.append(&chat_view.container);

        // ── Input Bar ──
        let input_bar = Box::new(Orientation::Horizontal, 8);
        input_bar.add_css_class("input-bar");

        let input = Entry::builder()
            .placeholder_text("Ask mentalOS anything...")
            .hexpand(true)
            .build();
        input.add_css_class("input-entry");
        input_bar.append(&input);

        let apps_btn = Button::with_label("All Apps");
        apps_btn.add_css_class("apps-button");
        input_bar.append(&apps_btn);

        root.append(&input_bar);

        window.set_child(Some(&root));

        // ── Input submit (Enter key) ──
        let chat_ref = std::rc::Rc::new(chat_view);
        let app_bar_ref = std::rc::Rc::new(app_bar);
        let input_ref = input.clone();
        let chat_for_enter = chat_ref.clone();
        let bar_for_enter = app_bar_ref.clone();
        input.connect_activate(move |entry| {
            let text = entry.text().to_string();
            if text.trim().is_empty() {
                return;
            }
            entry.set_text("");
            info!("User input: {text}");

            // Append user message
            chat_for_enter.append_message(MessageRole::User, &text);

            // Mock AI response
            bar_for_enter.set_status("Active");
            let chat_clone = chat_for_enter.clone();
            let bar_clone = bar_for_enter.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                let response = generate_mock_response(&text);
                chat_clone.append_message(MessageRole::Ai, &response);
                bar_clone.set_status("Idle");
            });
        });

        // ── All Apps button ──
        let win_ref = window.clone();
        apps_btn.connect_clicked(move |_| {
            AppLauncher::show(&win_ref);
        });

        // ── Keyboard shortcuts ──
        let key_ctrl = gtk4::EventControllerKey::new();
        let input_for_keys = input_ref.clone();
        let win_for_keys = window.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, modifiers| {
            let ctrl = modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
            let shift = modifiers.contains(gtk4::gdk::ModifierType::SHIFT_MASK);

            // Ctrl+L → focus input
            if ctrl && key == gtk4::gdk::Key::l {
                input_for_keys.grab_focus();
                return glib::Propagation::Stop;
            }
            // Ctrl+Shift+Q → emergency stop
            if ctrl && shift && key == gtk4::gdk::Key::Q {
                info!("Emergency STOP via keyboard");
                return glib::Propagation::Stop;
            }
            // Ctrl+K → open launcher
            if ctrl && key == gtk4::gdk::Key::k {
                AppLauncher::show(&win_for_keys);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(key_ctrl);

        // Focus input on start
        input_ref.grab_focus();

        Self { window }
    }
}

/// Generate a mock AI response for the UI shell.
fn generate_mock_response(user_input: &str) -> String {
    let lower = user_input.to_lowercase();

    if lower.contains("hello") || lower.contains("hi") {
        return "Hello! I'm mentalOS, your second brain. How can I help you today?".to_string();
    }

    if lower.contains("create") || lower.contains("build") || lower.contains("make") {
        return format!(
            "I'd create that project for you. Here's what I would do:\n\
            ```bash\n\
            mkdir -p ~/workspaces/new-project\n\
            cd ~/workspaces/new-project\n\
            # Initialize project scaffolding\n\
            ```\n\
            This is a mock response — real AI integration comes in M0.4."
        );
    }

    if lower.contains("install") {
        return format!(
            "⚠️  That would require running a system command:\n\
            ```\n\
            sudo pacman -S {}\n\
            ```\n\
            In the full version, you'd see an approval dialog before this executes.",
            user_input.split_whitespace().last().unwrap_or("package")
        );
    }

    if lower.contains("help") {
        return "Here are some things you can try:\n\
            • Ask me to create a project\n\
            • Ask me to install something\n\
            • Press Ctrl+K to open the App Launcher\n\
            • Press Ctrl+L to focus the input field\n\
            • Press Ctrl+Shift+Q for emergency stop"
            .to_string();
    }

    format!(
        "I received your message: \"{user_input}\"\n\n\
        This is a mock response. Real AI integration (OpenClaw/Ollama) will be connected in Milestone 0.4."
    )
}

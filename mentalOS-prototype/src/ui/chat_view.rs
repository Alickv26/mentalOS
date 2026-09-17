use crate::memory::{Conversation, Role};
use chrono::{DateTime, Local, Utc};
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, PolicyType, ScrolledWindow};
use std::time::Duration;

/// Enum representing who sent a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Ai,
    System,
}

/// Scrollable chat history view displaying styled messages.
pub struct ChatView {
    pub container: ScrolledWindow,
    message_list: Box,
}

impl ChatView {
    pub fn new() -> Self {
        let message_list = Box::new(Orientation::Vertical, 4);
        message_list.add_css_class("chat-view");
        message_list.set_vexpand(true);
        message_list.set_accessible_role(gtk4::AccessibleRole::Log);
        message_list.update_property(&[
            gtk4::accessible::Property::Label("Conversation log"),
            gtk4::accessible::Property::Description(
                "Streaming conversation history between you and mentalOS.",
            ),
            gtk4::accessible::Property::MultiLine(true),
        ]);

        append_welcome_rows(&message_list);

        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .child(&message_list)
            .build();
        scroll.set_accessible_role(gtk4::AccessibleRole::Region);
        scroll.update_property(&[
            gtk4::accessible::Property::Label("Chat history"),
            gtk4::accessible::Property::Description(
                "Scrollable chat history with AI and system messages.",
            ),
        ]);

        Self {
            container: scroll,
            message_list,
        }
    }

    /// Append a message to the chat view and auto-scroll to bottom.
    pub fn append_message(&self, role: MessageRole, content: &str) {
        let row = Box::new(Orientation::Vertical, 2);
        row.add_css_class("message-row");

        let (role_text, role_class, row_class) = match role {
            MessageRole::User => ("You", "message-role-user", "user-message"),
            MessageRole::Ai => ("AI", "message-role-ai", "ai-message"),
            MessageRole::System => ("System", "message-role-ai", "system-message"),
        };

        row.add_css_class(row_class);
        row.set_accessible_role(gtk4::AccessibleRole::ListItem);
        row.update_property(&[
            gtk4::accessible::Property::Label(role_text),
            gtk4::accessible::Property::Description("Chat message row."),
        ]);

        // Role label
        let role_header = format_role_header(role_text, &current_time_label());
        let role_label = Label::new(Some(&role_header));
        role_label.add_css_class("message-role");
        role_label.add_css_class(role_class);
        role_label.set_halign(gtk4::Align::Start);
        row.append(&role_label);

        // Content — detect code blocks (``` delimited)
        for part in split_code_blocks(content) {
            match part {
                ContentPart::Text(text) => {
                    let label = Label::new(Some(&text));
                    label.add_css_class("message-content");
                    label.set_halign(gtk4::Align::Start);
                    label.set_wrap(true);
                    label.set_selectable(true);
                    row.append(&label);
                }
                ContentPart::Code(code) => {
                    let label = Label::new(Some(&code));
                    label.add_css_class("code-block");
                    label.set_halign(gtk4::Align::Start);
                    label.set_wrap(true);
                    label.set_selectable(true);
                    row.append(&label);
                }
            }
        }

        self.message_list.append(&row);
        self.scroll_to_bottom();
    }

    /// Remove all messages from the chat view.
    pub fn clear(&self) {
        while let Some(child) = self.message_list.first_child() {
            self.message_list.remove(&child);
        }
        append_welcome_rows(&self.message_list);
    }

    pub fn load_conversation(&self, conversation: &Conversation) {
        while let Some(child) = self.message_list.first_child() {
            self.message_list.remove(&child);
        }

        for message in &conversation.messages {
            let role = match message.role {
                Role::User => MessageRole::User,
                Role::Assistant => MessageRole::Ai,
                Role::System => MessageRole::System,
            };
            self.append_message_with_time(role, &message.content, message.timestamp);
        }
    }

    fn scroll_to_bottom(&self) {
        let adj = self.container.vadjustment();
        glib::idle_add_local_once(move || {
            adj.set_value(adj.upper() - adj.page_size());
            let adj_late = adj.clone();
            glib::timeout_add_local_once(Duration::from_millis(40), move || {
                adj_late.set_value(adj_late.upper() - adj_late.page_size());
            });
        });
    }

    fn append_message_with_time(&self, role: MessageRole, content: &str, timestamp: DateTime<Utc>) {
        let row = Box::new(Orientation::Vertical, 2);
        row.add_css_class("message-row");

        let (role_text, role_class, row_class) = match role {
            MessageRole::User => ("You", "message-role-user", "user-message"),
            MessageRole::Ai => ("AI", "message-role-ai", "ai-message"),
            MessageRole::System => ("System", "message-role-ai", "system-message"),
        };

        row.add_css_class(row_class);
        row.set_accessible_role(gtk4::AccessibleRole::ListItem);
        row.update_property(&[
            gtk4::accessible::Property::Label(role_text),
            gtk4::accessible::Property::Description("Chat message row."),
        ]);

        let role_header = format_role_header(role_text, &time_label_from_timestamp(timestamp));
        let role_label = Label::new(Some(&role_header));
        role_label.add_css_class("message-role");
        role_label.add_css_class(role_class);
        role_label.set_halign(gtk4::Align::Start);
        row.append(&role_label);

        for part in split_code_blocks(content) {
            match part {
                ContentPart::Text(text) => {
                    let label = Label::new(Some(&text));
                    label.add_css_class("message-content");
                    label.set_halign(gtk4::Align::Start);
                    label.set_wrap(true);
                    label.set_selectable(true);
                    row.append(&label);
                }
                ContentPart::Code(code) => {
                    let label = Label::new(Some(&code));
                    label.add_css_class("code-block");
                    label.set_halign(gtk4::Align::Start);
                    label.set_wrap(true);
                    label.set_selectable(true);
                    row.append(&label);
                }
            }
        }

        self.message_list.append(&row);
        self.scroll_to_bottom();
    }
}

impl Default for ChatView {
    fn default() -> Self {
        Self::new()
    }
}

fn append_welcome_rows(message_list: &Box) {
    let welcome = Label::new(Some("Welcome to mentalOS"));
    welcome.add_css_class("welcome-label");
    message_list.append(&welcome);

    let hint = Label::new(Some("Type a message below to get started."));
    hint.add_css_class("welcome-hint");
    message_list.append(&hint);
}

fn current_time_label() -> String {
    Local::now().format("%H:%M").to_string()
}

fn format_role_header(role: &str, time_label: &str) -> String {
    format!("{role} · {time_label}")
}

fn time_label_from_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.with_timezone(&Local).format("%H:%M").to_string()
}

// ── Simple code-block splitter ───────────────────────────────

enum ContentPart {
    Text(String),
    Code(String),
}

fn split_code_blocks(content: &str) -> Vec<ContentPart> {
    let mut parts = Vec::new();
    let mut rest = content;

    loop {
        if let Some(start) = rest.find("```") {
            let before = &rest[..start];
            if !before.trim().is_empty() {
                parts.push(ContentPart::Text(before.trim().to_string()));
            }

            let after_marker = &rest[start + 3..];
            // skip optional language tag on the same line
            let code_start = after_marker.find('\n').map(|i| i + 1).unwrap_or(0);
            let code_body = &after_marker[code_start..];

            if let Some(end) = code_body.find("```") {
                let code = &code_body[..end];
                parts.push(ContentPart::Code(code.trim_end().to_string()));
                rest = &code_body[end + 3..];
            } else {
                // unclosed code block — treat rest as code
                parts.push(ContentPart::Code(code_body.to_string()));
                break;
            }
        } else {
            if !rest.trim().is_empty() {
                parts.push(ContentPart::Text(rest.trim().to_string()));
            }
            break;
        }
    }

    if parts.is_empty() {
        parts.push(ContentPart::Text(content.to_string()));
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn format_role_header_contains_role_and_time() {
        let header = format_role_header("AI", "14:23");
        assert_eq!(header, "AI · 14:23");
    }

    #[test]
    fn time_label_from_timestamp_formats_to_hour_minute() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 16, 14, 30, 0).unwrap();
        let label = time_label_from_timestamp(ts);
        assert_eq!(label.len(), 5);
        assert!(label.contains(':'));
    }

    #[test]
    fn split_code_blocks_handles_plain_text() {
        let parts = split_code_blocks("hello world");
        assert_eq!(parts.len(), 1);
        match &parts[0] {
            ContentPart::Text(v) => assert_eq!(v, "hello world"),
            ContentPart::Code(_) => panic!("expected text part"),
        }
    }
}

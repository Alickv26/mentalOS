use crate::error::{MentalOSError, Result};
use crate::ui::shortcuts::{SHORTCUT_DEFINITIONS, ShortcutBindings, validate_shortcut};
use gtk4::prelude::*;
use gtk4::{Box, Button, Entry, Grid, Label, Orientation, Window};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct ShortcutsSettings;

impl ShortcutsSettings {
    pub fn show<F>(parent: &impl IsA<gtk4::Window>, current: ShortcutBindings, on_saved: F)
    where
        F: Fn(ShortcutBindings) + 'static,
    {
        let dialog = Window::builder()
            .title("Manage Shortcuts")
            .modal(true)
            .transient_for(parent)
            .default_width(620)
            .default_height(440)
            .resizable(false)
            .build();
        dialog.add_css_class("launcher-window");

        let root = Box::new(Orientation::Vertical, 10);
        root.set_margin_top(16);
        root.set_margin_bottom(16);
        root.set_margin_start(16);
        root.set_margin_end(16);

        let title = Label::new(Some("Keyboard shortcut settings"));
        title.set_halign(gtk4::Align::Start);
        title.add_css_class("app-bar-title");
        title.set_accessible_role(gtk4::AccessibleRole::Heading);
        root.append(&title);

        let subtitle = Label::new(Some(
            "Format: Ctrl+K, Ctrl+Shift+M, Ctrl+Comma, Ctrl+Slash. Add Shift/Alt as needed.",
        ));
        subtitle.set_halign(gtk4::Align::Start);
        subtitle.set_wrap(true);
        subtitle.add_css_class("app-bar-stats");
        root.append(&subtitle);

        let grid = Grid::new();
        grid.set_column_spacing(12);
        grid.set_row_spacing(8);
        grid.set_hexpand(true);

        let header_action = Label::new(Some("Action"));
        header_action.set_halign(gtk4::Align::Start);
        header_action.add_css_class("app-bar-title");
        grid.attach(&header_action, 0, 0, 1, 1);

        let header_combo = Label::new(Some("Shortcut"));
        header_combo.set_halign(gtk4::Align::Start);
        header_combo.add_css_class("app-bar-title");
        grid.attach(&header_combo, 1, 0, 1, 1);

        let entry_map: Rc<RefCell<HashMap<String, Entry>>> = Rc::new(RefCell::new(HashMap::new()));
        let current_state = Rc::new(RefCell::new(current));

        for (idx, def) in SHORTCUT_DEFINITIONS.iter().enumerate() {
            let row = idx as i32 + 1;
            let action_text = format!("{} — {}", def.label, def.description);
            let label = Label::new(Some(&action_text));
            label.set_halign(gtk4::Align::Start);
            label.add_css_class("message-content");
            label.set_wrap(true);
            label.update_property(&[
                gtk4::accessible::Property::Label(def.label),
                gtk4::accessible::Property::Description(def.description),
            ]);
            grid.attach(&label, 0, row, 1, 1);

            let combo = current_state.borrow().get(def.id);
            let entry = Entry::builder().text(&combo).build();
            entry.set_hexpand(true);
            entry.set_tooltip_text(Some(def.default));
            entry.set_accessible_role(gtk4::AccessibleRole::TextBox);
            entry.update_property(&[
                gtk4::accessible::Property::Label(def.label),
                gtk4::accessible::Property::Description(def.description),
                gtk4::accessible::Property::Placeholder(def.default),
            ]);
            grid.attach(&entry, 1, row, 1, 1);
            entry_map
                .borrow_mut()
                .insert(def.id.to_string(), entry.clone());
        }

        root.append(&grid);

        let status_label = Label::new(None);
        status_label.set_halign(gtk4::Align::Start);
        status_label.add_css_class("welcome-hint");
        status_label.set_wrap(true);
        status_label.set_accessible_role(gtk4::AccessibleRole::Status);
        status_label.update_property(&[
            gtk4::accessible::Property::Label("Shortcut save status"),
            gtk4::accessible::Property::Description(
                "Validation and save feedback for shortcut changes.",
            ),
        ]);
        root.append(&status_label);

        let actions = Box::new(Orientation::Horizontal, 8);
        actions.set_halign(gtk4::Align::End);

        let reset_btn = Button::with_label("Reset Defaults");
        let cancel_btn = Button::with_label("Cancel");
        let save_btn = Button::with_label("Save");
        save_btn.add_css_class("create-button");
        reset_btn.update_property(&[
            gtk4::accessible::Property::Label("Reset defaults"),
            gtk4::accessible::Property::Description(
                "Reset all shortcuts in the form to their default values.",
            ),
        ]);
        cancel_btn.update_property(&[
            gtk4::accessible::Property::Label("Cancel shortcut changes"),
            gtk4::accessible::Property::Description("Close this dialog without applying changes."),
        ]);
        save_btn.update_property(&[
            gtk4::accessible::Property::Label("Save shortcut changes"),
            gtk4::accessible::Property::Description(
                "Validate and persist shortcut changes to disk.",
            ),
            gtk4::accessible::Property::KeyShortcuts("Ctrl+Return"),
        ]);
        actions.append(&reset_btn);
        actions.append(&cancel_btn);
        actions.append(&save_btn);
        root.append(&actions);

        dialog.set_child(Some(&root));

        let dialog_for_cancel = dialog.clone();
        cancel_btn.connect_clicked(move |_| {
            dialog_for_cancel.close();
        });

        let entry_map_for_reset = entry_map.clone();
        let status_for_reset = status_label.clone();
        reset_btn.connect_clicked(move |_| {
            for def in SHORTCUT_DEFINITIONS {
                if let Some(entry) = entry_map_for_reset.borrow().get(def.id) {
                    entry.set_text(def.default);
                }
            }
            status_for_reset.remove_css_class(status_class_for_result(true));
            status_for_reset.remove_css_class(status_class_for_result(false));
            status_for_reset.set_text("Defaults restored in form. Click Save to persist.");
        });

        let dialog_for_save = dialog.clone();
        let status_for_save = status_label.clone();
        let entry_map_for_save = entry_map.clone();
        let state_for_save = current_state.clone();

        save_btn.connect_clicked(move |_| {
            let form_values = collect_form_values(&entry_map_for_save.borrow());
            for entry in entry_map_for_save.borrow().values() {
                set_entry_validation_state(entry, true);
            }
            let updated = match apply_shortcut_form_values(&state_for_save.borrow(), &form_values) {
                Ok(v) => v,
                Err(err) => {
                    if let Some((action_id, _)) =
                        extract_action_id_from_validation_error(&err.to_string())
                    {
                        if let Some(entry) = entry_map_for_save.borrow().get(action_id) {
                            set_entry_validation_state(entry, false);
                            entry.grab_focus();
                        }
                    }
                    status_for_save.remove_css_class(status_class_for_result(true));
                    status_for_save.add_css_class(status_class_for_result(false));
                    status_for_save.set_text(&format!("Invalid shortcut: {}", err));
                    return;
                }
            };

            match updated.save() {
                Ok(path) => {
                    *state_for_save.borrow_mut() = updated.clone();
                    on_saved(updated);
                    status_for_save.remove_css_class(status_class_for_result(false));
                    status_for_save.add_css_class(status_class_for_result(true));
                    status_for_save.set_text(&format!("Saved shortcuts to {}", path.display()));
                }
                Err(err) => {
                    status_for_save.remove_css_class(status_class_for_result(true));
                    status_for_save.add_css_class(status_class_for_result(false));
                    status_for_save.set_text(&format!("Failed to save shortcuts: {}", err));
                }
            }
        });

        let dialog_for_enter = dialog_for_save.clone();
        let save_btn_for_enter = save_btn.clone();
        let key_ctrl_save = gtk4::EventControllerKey::new();
        key_ctrl_save.connect_key_pressed(move |_, key, _, mods| {
            if key == gtk4::gdk::Key::Return && mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK)
            {
                save_btn_for_enter.emit_clicked();
                return gtk4::glib::Propagation::Stop;
            }
            if key == gtk4::gdk::Key::Escape {
                dialog_for_enter.close();
                return gtk4::glib::Propagation::Stop;
            }
            gtk4::glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl_save);

        dialog.present();
    }
}

fn collect_form_values(entry_map: &HashMap<String, Entry>) -> HashMap<String, String> {
    entry_map
        .iter()
        .map(|(id, entry)| (id.clone(), entry.text().to_string()))
        .collect()
}

fn apply_shortcut_form_values(
    current: &ShortcutBindings,
    form_values: &HashMap<String, String>,
) -> Result<ShortcutBindings> {
    let mut updated = current.clone();
    for def in SHORTCUT_DEFINITIONS {
        let Some(value) = form_values.get(def.id) else {
            continue;
        };
        if let Err(err) = validate_shortcut(value) {
            return Err(MentalOSError::ConfigInvalid(format!(
                "{} -> {}",
                def.label, err
            )));
        }
        updated.set(def.id, value);
    }
    Ok(updated)
}

fn status_class_for_result(ok: bool) -> &'static str {
    if ok {
        "shortcut-status-ok"
    } else {
        "shortcut-status-error"
    }
}

fn validation_entry_css_class(valid: bool) -> Option<&'static str> {
    if valid {
        None
    } else {
        Some("shortcut-entry-error")
    }
}

fn set_entry_validation_state(entry: &Entry, valid: bool) {
    entry.remove_css_class("shortcut-entry-error");
    if let Some(class) = validation_entry_css_class(valid) {
        entry.add_css_class(class);
    }
}

fn extract_action_id_from_validation_error(message: &str) -> Option<(&str, &str)> {
    for def in SHORTCUT_DEFINITIONS {
        if message.contains(def.label) {
            return Some((def.id, def.label));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_shortcut_form_values_updates_and_preserves_existing() {
        let current = ShortcutBindings::default();
        let mut form_values = HashMap::new();
        form_values.insert("show_help".to_string(), "Ctrl+Question".to_string());

        let updated = apply_shortcut_form_values(&current, &form_values)
            .expect("valid shortcut update should succeed");
        assert_eq!(updated.get("show_help"), "Ctrl+Question");
        assert_eq!(
            updated.get("manage_shortcuts"),
            current.get("manage_shortcuts")
        );
    }

    #[test]
    fn apply_shortcut_form_values_rejects_invalid_entry() {
        let current = ShortcutBindings::default();
        let mut form_values = HashMap::new();
        form_values.insert("show_help".to_string(), "Ctrl".to_string());

        assert!(apply_shortcut_form_values(&current, &form_values).is_err());
    }

    #[test]
    fn status_class_mapping_is_stable() {
        assert_eq!(status_class_for_result(true), "shortcut-status-ok");
        assert_eq!(status_class_for_result(false), "shortcut-status-error");
    }

    #[test]
    fn validation_class_mapping_is_stable() {
        assert_eq!(validation_entry_css_class(true), None);
        assert_eq!(
            validation_entry_css_class(false),
            Some("shortcut-entry-error")
        );
    }

    #[test]
    fn extract_action_id_from_validation_error_finds_definition() {
        let msg = "Show shortcuts help -> config is invalid";
        let action = extract_action_id_from_validation_error(msg);
        assert_eq!(action.map(|v| v.0), Some("show_help"));
    }
}

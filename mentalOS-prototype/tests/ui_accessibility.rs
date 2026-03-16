use mentalOS::ui::chat_view::ChatView;
use mentalOS::ui::omni_pill::OmniPill;
use std::sync::OnceLock;

fn gtk_available() -> bool {
    static INIT: OnceLock<bool> = OnceLock::new();
    *INIT.get_or_init(|| gtk4::init().is_ok())
}

#[test]
fn omni_pill_exposes_accessible_labels_and_roles() {
    if !gtk_available() {
        eprintln!("Skipping GTK accessibility test: GTK init unavailable");
        return;
    }

    let pill = OmniPill::new();

    assert!(gtk4::test_accessible_has_role(
        &pill.agent_selector,
        gtk4::AccessibleRole::ComboBox
    ));
    assert!(gtk4::test_accessible_has_role(
        &pill.input,
        gtk4::AccessibleRole::TextBox
    ));
    assert!(gtk4::test_accessible_has_property(
        &pill.input,
        gtk4::AccessibleProperty::Label
    ));
    assert!(gtk4::test_accessible_has_property(
        &pill.input,
        gtk4::AccessibleProperty::KeyShortcuts
    ));
    assert!(gtk4::test_accessible_has_property(
        &pill.apps_btn,
        gtk4::AccessibleProperty::Label
    ));
}

#[test]
fn chat_view_exposes_accessible_region_metadata() {
    if !gtk_available() {
        eprintln!("Skipping GTK accessibility test: GTK init unavailable");
        return;
    }

    let chat = ChatView::new();
    assert!(gtk4::test_accessible_has_role(
        &chat.container,
        gtk4::AccessibleRole::Region
    ));
    assert!(gtk4::test_accessible_has_property(
        &chat.container,
        gtk4::AccessibleProperty::Label
    ));
}

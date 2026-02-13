use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Box, Button, Label, Orientation, Window};
use log::info;

/// Possible outcomes of the approval dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    ApproveOnce,
    ApproveSimilar,
    Deny,
}

/// Modal dialog asking the user to approve a shell command.
pub struct ApprovalDialog;

impl ApprovalDialog {
    /// Show an approval dialog for the given command.
    ///
    /// `on_decision` is called with the user's choice.
    pub fn show<F>(parent: &impl IsA<gtk4::Window>, command: &str, on_decision: F)
    where
        F: Fn(ApprovalDecision) + Clone + 'static,
    {
        let dialog = Window::builder()
            .title("Command Approval Required")
            .modal(true)
            .transient_for(parent)
            .default_width(460)
            .default_height(200)
            .resizable(false)
            .build();

        dialog.add_css_class("approval-dialog");

        let vbox = Box::new(Orientation::Vertical, 12);
        vbox.set_margin_top(20);
        vbox.set_margin_bottom(20);
        vbox.set_margin_start(20);
        vbox.set_margin_end(20);

        // Title
        let title = Label::new(Some("⚠️  Command requires approval"));
        title.add_css_class("approval-title");
        title.set_halign(gtk4::Align::Start);
        vbox.append(&title);

        // Command display
        let cmd_label = Label::new(Some(command));
        cmd_label.add_css_class("approval-command");
        cmd_label.set_selectable(true);
        cmd_label.set_wrap(true);
        cmd_label.set_halign(gtk4::Align::Fill);
        vbox.append(&cmd_label);

        // Buttons
        let btn_box = Box::new(Orientation::Horizontal, 8);
        btn_box.set_halign(gtk4::Align::End);
        btn_box.set_margin_top(8);

        let approve_btn = Button::with_label("Approve Once");
        approve_btn.add_css_class("approve-button");
        let deny_btn = Button::with_label("Deny");
        deny_btn.add_css_class("deny-button");
        let similar_btn = Button::with_label("Approve Similar");
        similar_btn.add_css_class("approve-similar-button");

        btn_box.append(&deny_btn);
        btn_box.append(&similar_btn);
        btn_box.append(&approve_btn);
        vbox.append(&btn_box);

        dialog.set_child(Some(&vbox));

        // Connect signals
        let d1 = dialog.clone();
        let cb1 = on_decision.clone();
        approve_btn.connect_clicked(move |_| {
            info!("Approval: ApproveOnce");
            cb1(ApprovalDecision::ApproveOnce);
            d1.close();
        });

        let d2 = dialog.clone();
        let cb2 = on_decision.clone();
        similar_btn.connect_clicked(move |_| {
            info!("Approval: ApproveSimilar");
            cb2(ApprovalDecision::ApproveSimilar);
            d2.close();
        });

        let d3 = dialog.clone();
        deny_btn.connect_clicked(move |_| {
            info!("Approval: Deny");
            on_decision(ApprovalDecision::Deny);
            d3.close();
        });

        // Esc to close (deny)
        let key_ctrl = gtk4::EventControllerKey::new();
        let d4 = dialog.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                d4.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl);

        dialog.present();
    }
}

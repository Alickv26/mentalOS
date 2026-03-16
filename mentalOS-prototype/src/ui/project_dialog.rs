use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Box, Button, Label, Orientation, Window};
use log::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectDecision {
    Create,
    Cancel,
}

pub struct ProjectDialog;

impl ProjectDialog {
    pub fn show<F>(
        parent: &impl IsA<gtk4::Window>,
        project_name: &str,
        language: Option<&str>,
        framework: Option<&str>,
        on_decision: F,
    ) where
        F: Fn(ProjectDecision) + Clone + 'static,
    {
        let dialog = Window::builder()
            .title("Create Project")
            .modal(true)
            .transient_for(parent)
            .default_width(400)
            .default_height(220)
            .resizable(false)
            .build();

        dialog.add_css_class("project-dialog");

        let vbox = Box::new(Orientation::Vertical, 12);
        vbox.set_margin_top(20);
        vbox.set_margin_bottom(20);
        vbox.set_margin_start(20);
        vbox.set_margin_end(20);

        let title = Label::new(Some("Create new project?"));
        title.add_css_class("project-title");
        title.set_halign(gtk4::Align::Start);
        vbox.append(&title);

        let name_label = Label::new(Some(&format!("Name: {}", project_name)));
        name_label.add_css_class("project-name");
        name_label.set_halign(gtk4::Align::Start);
        vbox.append(&name_label);

        if let Some(lang) = language {
            let lang_label = Label::new(Some(&format!("Language: {}", lang)));
            lang_label.add_css_class("project-lang");
            lang_label.set_halign(gtk4::Align::Start);
            vbox.append(&lang_label);
        }

        if let Some(fw) = framework {
            let fw_label = Label::new(Some(&format!("Framework: {}", fw)));
            fw_label.add_css_class("project-framework");
            fw_label.set_halign(gtk4::Align::Start);
            vbox.append(&fw_label);
        }

        let desc = Label::new(Some("OpenCode will scaffold the project in ~/workspaces/"));
        desc.add_css_class("project-desc");
        desc.set_halign(gtk4::Align::Start);
        desc.set_wrap(true);
        vbox.append(&desc);

        let btn_box = Box::new(Orientation::Horizontal, 8);
        btn_box.set_halign(gtk4::Align::End);
        btn_box.set_margin_top(12);

        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.add_css_class("cancel-button");
        let create_btn = Button::with_label("Create Project");
        create_btn.add_css_class("create-button");

        btn_box.append(&cancel_btn);
        btn_box.append(&create_btn);
        vbox.append(&btn_box);

        dialog.set_child(Some(&vbox));

        let d1 = dialog.clone();
        let cb1 = on_decision.clone();
        create_btn.connect_clicked(move |_| {
            info!("Project: Create");
            cb1(ProjectDecision::Create);
            d1.close();
        });

        let d2 = dialog.clone();
        let cb2 = on_decision.clone();
        cancel_btn.connect_clicked(move |_| {
            info!("Project: Cancel");
            cb2(ProjectDecision::Cancel);
            d2.close();
        });

        let key_ctrl = gtk4::EventControllerKey::new();
        let d3 = dialog.clone();
        let cb3 = on_decision.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                cb3(ProjectDecision::Cancel);
                d3.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl);

        dialog.present();
    }
}

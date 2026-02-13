use gtk4::prelude::*;
use gtk4::{gdk, Application, CssProvider};
use mentalOS::ui::main_window::MainWindow;

const APP_ID: &str = "com.mentalos.prototype";

fn main() {
    env_logger::init();

    let app = Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_| {
        load_css();
    });

    app.connect_activate(|app| {
        let win = MainWindow::new(app);
        win.window.present();
    });

    app.run();
}

/// Load the custom dark-theme CSS stylesheet.
fn load_css() {
    let provider = CssProvider::new();

    // Load from the embedded CSS file next to the source.
    // At runtime we check a few locations:
    //   1. ./src/ui/style.css  (development, running from project root)
    //   2. Alongside the binary
    let css_paths = [
        "src/ui/style.css",
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/ui/style.css"),
    ];

    let mut loaded = false;
    for path in &css_paths {
        let p = std::path::Path::new(path);
        if p.exists() {
            provider.load_from_path(p);
            log::info!("Loaded CSS from {}", p.display());
            loaded = true;
            break;
        }
    }

    if !loaded {
        log::warn!("Could not find style.css — using default theme");
        return;
    }

    gtk4::style_context_add_provider_for_display(
        &gdk::Display::default().expect("Could not get default display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

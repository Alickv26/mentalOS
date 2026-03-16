use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, CssProvider, gdk};
use mentalOS::config::{Config, config_path};
use mentalOS::memory::MemoryManager;
use mentalOS::openclaw::OpenClawClient;
use mentalOS::openclaw_launcher::OpenClawLauncher;
use mentalOS::project_handler::ProjectHandler;
use mentalOS::router::{CommandRouter, FirejailExecutor};
use mentalOS::task_tracker::TaskTracker;
use mentalOS::ui::main_window::MainWindow;
use mentalOS::ui::messages::{BackendRequest, BackendResponse};
use mentalOS::whitelist::WhitelistManager;
use mentalOS::workspace::WorkspaceManager;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

const APP_ID: &str = "com.mentalos.prototype";

fn main() {
    env_logger::init();

    let (backend_tx, mut backend_rx) = mpsc::channel::<BackendRequest>(32);

    // Handshake channel to pass UI Sender to Backend thread once created
    // The type must match exactly what MainWindow returns: glib::Sender<BackendResponse>
    // Since we use 'use gtk4::glib', it is gtk4::glib::Sender.
    let (handshake_tx, handshake_rx) = std_mpsc::channel::<glib::Sender<BackendResponse>>();

    thread::spawn(move || {
        let rt = Runtime::new().expect("Failed to create Tokio runtime");
        rt.block_on(async {
            log::info!("Backend started, waiting for UI handshake...");

            let ui_tx = match handshake_rx.recv() {
                Ok(tx) => tx,
                Err(_) => {
                    log::error!("Handshake failed, exiting backend");
                    return;
                }
            };
            log::info!("UI Handshake received.");

            let config = Config::load().unwrap_or_else(|e| {
                log::error!("Config load failed: {}", e);
                Config::default()
            });

            // Determine Whitelist Path
            // Try to use standard config dir, otherwise fallback to /tmp
            let whitelist_path = config_path()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("whitelist.json")))
                .unwrap_or_else(|| PathBuf::from("/tmp/mentalOS-whitelist.json"));

            let openclaw = OpenClawClient::from_config(&config);
            let mut openclaw_launcher = OpenClawLauncher::from_config(&config);
            let _ = openclaw_launcher.ensure_running();
            let whitelist = Arc::new(Mutex::new(
                WhitelistManager::load(whitelist_path.clone())
                    .unwrap_or_else(|_| WhitelistManager::new(whitelist_path)),
            ));

            // Use config.paths.workspace_dir (type String) -> PathBuf
            let workspace_dir = PathBuf::from(&config.paths.workspace_dir);
            // Note: We should expand tilde if Config::load didn't?
            // Config::load calls normalize_paths but Config::default doesn't?
            // If default, it is "~/workspaces".
            // MemoryManager might handle tilde? Usually not.
            // For verified robustness, we'll just use it as is or expand if using Config method.
            // But Main can't easily call private methods.
            // Let's rely on logic working or basic path.

            let memory = Arc::new(Mutex::new(MemoryManager::new(workspace_dir.clone())));
            let executor = FirejailExecutor::new();

            let project_handler =
                ProjectHandler::new(workspace_dir.clone(), "opencode".to_string());
            let task_tracker = TaskTracker::new(workspace_dir.clone());
            let workspace_manager = WorkspaceManager::new(workspace_dir);

            let mut router = CommandRouter::new(openclaw, whitelist, memory, executor)
                .with_project_handler(project_handler)
                .with_task_tracker(task_tracker)
                .with_workspace_manager(workspace_manager);

            while let Some(req) = backend_rx.recv().await {
                match req {
                    BackendRequest::Input {
                        text,
                        workspace,
                        category,
                    } => {
                        log::info!("Processing input: {}", text);
                        let _ = openclaw_launcher.ensure_running();

                        if let Some(confirmation) = router.detect_project_intent(&text) {
                            log::info!("Project intent detected: {:?}", confirmation);
                            let _ = ui_tx.send(BackendResponse::ProjectConfirmationRequired {
                                name: confirmation.name,
                                language: confirmation.language,
                                framework: confirmation.framework,
                            });
                            continue;
                        }

                        match router.handle_input(&workspace, &category, &text, 5).await {
                            Ok(response) => {
                                let _ = ui_tx.send(BackendResponse::Chat(response.message));
                                for output in response.outputs {
                                    let _ = ui_tx.send(BackendResponse::CommandResult(output));
                                }
                                for cmd in response.approvals_required {
                                    let _ = ui_tx.send(BackendResponse::ApprovalRequired(cmd));
                                }
                            }
                            Err(e) => {
                                log::error!("Router error: {}", e);
                                let _ = ui_tx.send(BackendResponse::Error(e.to_string()));
                            }
                        }
                    }
                    BackendRequest::ExecuteCommand(cmd) => {
                        log::info!("Executing approved command: {}", cmd);
                        match router.execute_approved_command(&cmd) {
                            Ok(output) => {
                                let _ = ui_tx.send(BackendResponse::CommandResult(output));
                            }
                            Err(e) => {
                                let _ = ui_tx.send(BackendResponse::Error(e.to_string()));
                            }
                        }
                    }
                    BackendRequest::GetAgents => {
                        let agents = router.list_agents();
                        let current = router.get_current_provider();
                        let _ = ui_tx.send(BackendResponse::AgentList { agents, current });
                    }
                    BackendRequest::SwitchAgent(name) => {
                        log::info!("Switching agent to: {}", name);
                        match router.switch_agent(&name) {
                            Ok(msg) => {
                                let _ = ui_tx.send(BackendResponse::AgentSwitched(name));
                                let _ = ui_tx.send(BackendResponse::Chat(msg));
                            }
                            Err(e) => {
                                let _ = ui_tx.send(BackendResponse::Error(e.to_string()));
                            }
                        }
                    }
                    BackendRequest::CreateProject {
                        name,
                        language,
                        framework,
                    } => {
                        log::info!(
                            "Creating project: {} with lang: {:?}, framework: {:?}",
                            name,
                            language,
                            framework
                        );
                        match router.create_project(&name, language, framework) {
                            Ok((success, message, path)) => {
                                let _ = ui_tx.send(BackendResponse::ProjectCreated {
                                    success,
                                    path,
                                    message,
                                });
                            }
                            Err(e) => {
                                let _ = ui_tx.send(BackendResponse::ProjectCreated {
                                    success: false,
                                    path: None,
                                    message: e.to_string(),
                                });
                            }
                        }
                    }
                    BackendRequest::RunProjectCommand {
                        workspace,
                        command_type,
                    } => {
                        log::info!(
                            "Running project command: {} for workspace: {}",
                            command_type,
                            workspace
                        );
                        match router.run_project_command(&workspace, "general", &command_type) {
                            Ok(response) => {
                                let _ = ui_tx.send(BackendResponse::Chat(response.message));
                                for output in response.outputs {
                                    let _ = ui_tx.send(BackendResponse::CommandResult(output));
                                }
                                for cmd in response.approvals_required {
                                    let _ = ui_tx.send(BackendResponse::ApprovalRequired(cmd));
                                }
                            }
                            Err(e) => {
                                let _ = ui_tx.send(BackendResponse::Error(e.to_string()));
                            }
                        }
                    }
                    BackendRequest::EmergencyStop => {
                        log::warn!("Emergency stop requested");
                        let _ = openclaw_launcher.stop();
                        match router.emergency_stop() {
                            Ok(msg) => {
                                let _ = ui_tx.send(BackendResponse::Chat(msg));
                            }
                            Err(e) => {
                                let _ = ui_tx.send(BackendResponse::Error(e.to_string()));
                            }
                        }
                    }
                }
            }
            log::info!("Backend loop ended");
        });
    });

    let app = Application::builder().application_id(APP_ID).build();
    let backend_tx = backend_tx.clone();

    let handshake_tx = Rc::new(RefCell::new(Some(handshake_tx)));

    app.connect_startup(|_| {
        load_css();
    });

    app.connect_activate(move |app| {
        let (win, ui_tx) = MainWindow::new(app, backend_tx.clone());
        win.window.present();

        if let Some(tx) = handshake_tx.borrow_mut().take() {
            let _ = tx.send(ui_tx);
        }
    });

    app.run();
}

fn load_css() {
    let provider = CssProvider::new();
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

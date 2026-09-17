use gtk4::prelude::*;
use gtk4::{Application, CssProvider, gdk};
use mental_os::agent_manager::AgentManager;
use mental_os::config::{Config, config_path};
use mental_os::memory::MemoryManager;
use mental_os::providers;
use mental_os::providers::openclaw_launcher::OpenClawLauncher;
use mental_os::project_handler::ProjectHandler;
use mental_os::router::{CommandRouter, FirejailExecutor};
use mental_os::task_tracker::TaskTracker;
use mental_os::ui::config_wizard::ConfigWizard;
use mental_os::ui::main_window::MainWindow;
use mental_os::ui::messages::{BackendRequest, BackendResponse};
use mental_os::ui::onboarding_tutorial::OnboardingTutorial;
use mental_os::whitelist::WhitelistManager;
use mental_os::workspace::WorkspaceManager;
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

const APP_ID: &str = "com.mentalos.prototype";
const DEFAULT_LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;
static LOG_FILE: OnceLock<Mutex<fs::File>> = OnceLock::new();
static LOG_FILE_PATH: OnceLock<PathBuf> = OnceLock::new();

fn expand_home_path(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/")
        && let Some(home) = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf())
    {
        return home.join(stripped);
    }
    PathBuf::from(path)
}

fn send_ui(ui_tx: &async_channel::Sender<BackendResponse>, msg: BackendResponse) {
    if let Err(err) = ui_tx.send_blocking(msg) {
        log::warn!("Failed to send backend response to UI: {}", err);
    }
}

fn main() {
    init_logging();

    let (backend_tx, mut backend_rx) = mpsc::channel::<BackendRequest>(32);

    // Handshake channel to pass UI Sender to Backend thread once created.
    let (handshake_tx, handshake_rx) =
        std_mpsc::channel::<async_channel::Sender<BackendResponse>>();

    thread::spawn(move || {
        let rt = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(err) => {
                log::error!("Failed to create Tokio runtime: {}", err);
                return;
            }
        };
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

            let provider_name = config.ai.provider.clone();
            let openclaw = providers::create_provider(&config)
                .unwrap_or_else(|e| {
                    log::error!("Failed to create AI provider '{}': {}", provider_name, e);
                    // Fallback to OpenClaw provider
                    let openclaw_provider = mental_os::providers::openclaw::OpenClawProvider::from_config(&config)
                        .expect("Failed to create fallback OpenClaw provider");
                    Box::new(openclaw_provider) as Box<dyn mental_os::providers::AiProvider>
                });

            // Only create and start the OpenClaw launcher when the configured provider is OpenClaw.
            // Cloud providers (DeepSeek, Zen) don't need a local gateway process.
            let mut openclaw_launcher = if provider_name == "openclaw" {
                let mut launcher = OpenClawLauncher::from_config(&config);
                if let Err(err) = launcher.ensure_running() {
                    log::warn!("OpenClaw launcher startup check failed: {}", err);
                }
                Some(launcher)
            } else {
                log::info!("Provider '{}' does not require a local gateway — skipping launcher", provider_name);
                None
            };
            let whitelist = Arc::new(Mutex::new(
                WhitelistManager::load(whitelist_path.clone())
                    .unwrap_or_else(|_| WhitelistManager::new(whitelist_path)),
            ));

            // Config::default may still contain "~", so normalize here too.
            let workspace_dir = expand_home_path(&config.paths.workspace_dir);

            let memory = Arc::new(Mutex::new(MemoryManager::new(workspace_dir.clone())));
            let executor = FirejailExecutor::new();

            let project_handler =
                ProjectHandler::new(workspace_dir.clone(), "opencode".to_string());
            let task_tracker = TaskTracker::new(workspace_dir.clone());
            let workspace_manager = WorkspaceManager::new(workspace_dir);

            // Build the agent manager and load configured agents from config.
            // This is the source of truth for which agents exist + which is active.
            // The router delegates list_agents / get_current_provider / switch_agent
            // to it, then syncs the runtime provider state.
            let config_dir_for_agents = config_path()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("/tmp"));
            let mut agent_manager = AgentManager::new(config_dir_for_agents);
            agent_manager.load_agents(config.agents.clone());
            let agent_manager = Arc::new(Mutex::new(agent_manager));

            let mut router = CommandRouter::new(
                openclaw,
                whitelist,
                memory,
                executor,
            )
            .with_agent_manager(agent_manager)
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
                        if let Some(ref mut launcher) = openclaw_launcher {
                            if let Err(err) = launcher.ensure_running() {
                                log::warn!("OpenClaw launcher check failed: {}", err);
                            }
                        }
                        send_ui(
                            &ui_tx,
                            BackendResponse::Status("AI is thinking...".to_string()),
                        );

                        if let Some(confirmation) = router.detect_project_intent(&text) {
                            log::info!("Project intent detected: {:?}", confirmation);
                            send_ui(
                                &ui_tx,
                                BackendResponse::ProjectConfirmationRequired {
                                    name: confirmation.name,
                                    language: confirmation.language,
                                    framework: confirmation.framework,
                                },
                            );
                            continue;
                        }

                        match router.handle_input(&workspace, &category, &text, 5) {
                            Ok(response) => {
                                send_ui(&ui_tx, BackendResponse::Chat(response.message));
                                for output in response.outputs {
                                    send_ui(&ui_tx, BackendResponse::CommandResult(output));
                                }
                                for cmd in response.approvals_required {
                                    send_ui(&ui_tx, BackendResponse::ApprovalRequired(cmd));
                                }
                            }
                            Err(e) => {
                                log::error!("Router error: {}", e);
                                send_ui(&ui_tx, BackendResponse::Error(e.to_string()));
                            }
                        }
                        // Always notify the UI of the current circuit breaker state
                        // after a handle_input call, so the omni pill can reflect
                        // Open (red), HalfOpen (yellow), or Closed (normal) in real time.
                        send_ui(
                            &ui_tx,
                            BackendResponse::CircuitStateChanged(router.circuit_state()),
                        );
                    }
                    BackendRequest::ExecuteCommand(cmd) => {
                        log::info!("Executing approved command: {}", cmd);
                        send_ui(
                            &ui_tx,
                            BackendResponse::Status("Executing approved command...".to_string()),
                        );
                        match router.execute_approved_command(&cmd) {
                            Ok(output) => {
                                send_ui(&ui_tx, BackendResponse::CommandResult(output));
                            }
                            Err(e) => {
                                send_ui(&ui_tx, BackendResponse::Error(e.to_string()));
                            }
                        }
                    }
                    BackendRequest::GetAgents => {
                        let agents = router.list_agents();
                        let current = router.get_current_provider();
                        send_ui(&ui_tx, BackendResponse::AgentList { agents, current });
                    }
                    BackendRequest::SwitchAgent(name) => {
                        log::info!("Switching agent to: {}", name);
                        send_ui(
                            &ui_tx,
                            BackendResponse::Status(format!("Switching agent to {}...", name)),
                        );
                        match router.switch_agent(&name) {
                            Ok(msg) => {
                                send_ui(&ui_tx, BackendResponse::AgentSwitched(name));
                                send_ui(&ui_tx, BackendResponse::Chat(msg));
                            }
                            Err(e) => {
                                send_ui(&ui_tx, BackendResponse::Error(e.to_string()));
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
                        send_ui(
                            &ui_tx,
                            BackendResponse::Status(format!("Creating project '{}'...", name)),
                        );
                        match router.create_project(&name, language, framework) {
                            Ok((success, message, path)) => {
                                send_ui(
                                    &ui_tx,
                                    BackendResponse::ProjectCreated {
                                        success,
                                        path,
                                        message,
                                    },
                                );
                            }
                            Err(e) => {
                                send_ui(
                                    &ui_tx,
                                    BackendResponse::ProjectCreated {
                                        success: false,
                                        path: None,
                                        message: e.to_string(),
                                    },
                                );
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
                        send_ui(
                            &ui_tx,
                            BackendResponse::Status(format!(
                                "Running '{}' in project...",
                                command_type
                            )),
                        );
                        match router.run_project_command(&workspace, "general", &command_type) {
                            Ok(response) => {
                                send_ui(&ui_tx, BackendResponse::Chat(response.message));
                                for output in response.outputs {
                                    send_ui(&ui_tx, BackendResponse::CommandResult(output));
                                }
                                for cmd in response.approvals_required {
                                    send_ui(&ui_tx, BackendResponse::ApprovalRequired(cmd));
                                }
                            }
                            Err(e) => {
                                send_ui(&ui_tx, BackendResponse::Error(e.to_string()));
                            }
                        }
                    }
                    BackendRequest::EmergencyStop => {
                        log::warn!("Emergency stop requested");
                        if let Some(ref mut launcher) = openclaw_launcher {
                            let _ = launcher.stop();
                        }
                        match router.emergency_stop() {
                            Ok(msg) => {
                                send_ui(&ui_tx, BackendResponse::Chat(msg));
                            }
                            Err(e) => {
                                send_ui(&ui_tx, BackendResponse::Error(e.to_string()));
                            }
                        }
                    }
                    BackendRequest::CheckProviderHealth => {
                        let provider_name = router.get_current_provider();
                        let healthy = router.is_provider_healthy();
                        send_ui(
                            &ui_tx,
                            BackendResponse::ProviderHealth {
                                provider: provider_name,
                                healthy,
                            },
                        );
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
        let win_for_onboarding = win.window.clone();

        let config_missing = config_path().map(|p| !p.exists()).unwrap_or(true);
        if config_missing {
            let handshake_for_wizard = handshake_tx.clone();
            let ui_tx_for_wizard = ui_tx.clone();
            let win_for_onboarding_from_wizard = win_for_onboarding.clone();
            ConfigWizard::show(win.window.upcast_ref::<gtk4::Window>(), move || {
                if let Some(tx) = handshake_for_wizard.borrow_mut().take()
                    && let Err(err) = tx.send(ui_tx_for_wizard.clone())
                {
                    log::warn!("Failed to send UI handshake after setup wizard: {}", err);
                }
                show_onboarding_if_needed(&win_for_onboarding_from_wizard);
            });
        } else if let Some(tx) = handshake_tx.borrow_mut().take() {
            if let Err(err) = tx.send(ui_tx) {
                log::warn!("Failed to send UI handshake: {}", err);
            }
            show_onboarding_if_needed(&win_for_onboarding);
        }
    });

    app.run();
}

fn show_onboarding_if_needed(parent: &impl IsA<gtk4::Window>) {
    if OnboardingTutorial::should_show() {
        OnboardingTutorial::show(parent);
    }
}

fn init_logging() {
    use env_logger::Env;
    use std::io::Write;

    let debug_mode = env_flag("MENTALOS_DEBUG");
    let default_filter = if debug_mode { "debug" } else { "info" };
    let file_logging = setup_file_logging();

    let json_mode = std::env::var("MENTALOS_LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    if json_mode {
        let mut builder =
            env_logger::Builder::from_env(Env::default().default_filter_or(default_filter));
        builder.format(|buf, record| {
            let timestamp = chrono::Utc::now().to_rfc3339();
            let line = serde_json::json!({
                "timestamp": timestamp,
                "level": record.level().to_string(),
                "target": record.target(),
                "message": record.args().to_string(),
            });
            let rendered = line.to_string();
            write_log_file_line(&rendered);
            writeln!(buf, "{}", rendered)
        });
        builder.init();
    } else {
        let mut builder =
            env_logger::Builder::from_env(Env::default().default_filter_or(default_filter));
        builder.format(|buf, record| {
            let timestamp = chrono::Utc::now().to_rfc3339();
            let rendered = format!(
                "[{} {} {}] {}",
                timestamp,
                record.level(),
                record.target(),
                record.args()
            );
            write_log_file_line(&rendered);
            writeln!(buf, "{}", rendered)
        });
        builder.init();
    }

    match file_logging {
        Ok(Some(path)) => log::info!("File logging enabled: {}", path.display()),
        Ok(None) => log::info!("File logging disabled via MENTALOS_LOG_TO_FILE=0"),
        Err(err) => log::warn!("File logging unavailable: {}", err),
    }
}

fn setup_file_logging() -> std::result::Result<Option<PathBuf>, String> {
    use std::fs::OpenOptions;

    if !env_flag_default_true("MENTALOS_LOG_TO_FILE") {
        return Ok(None);
    }

    let path = log_file_path();
    let parent = path
        .parent()
        .ok_or_else(|| format!("Invalid log file path: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("Failed creating log dir: {e}"))?;

    let max_bytes = std::env::var("MENTALOS_LOG_MAX_BYTES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_LOG_MAX_BYTES);
    rotate_log_file_if_needed(&path, max_bytes).map_err(|e| format!("Log rotation failed: {e}"))?;

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("Failed opening log file {}: {e}", path.display()))?;

    let _ = LOG_FILE.set(Mutex::new(file));
    let _ = LOG_FILE_PATH.set(path.clone());
    Ok(Some(path))
}

fn rotate_log_file_if_needed(path: &std::path::Path, max_bytes: u64) -> std::io::Result<()> {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    if metadata.len() < max_bytes {
        return Ok(());
    }

    let backup = PathBuf::from(format!("{}.1", path.display()));
    if backup.exists() {
        fs::remove_file(&backup)?;
    }
    fs::rename(path, backup)?;
    Ok(())
}

fn write_log_file_line(line: &str) {
    use std::io::Write;
    if let Some(lock) = LOG_FILE.get()
        && let Ok(mut file) = lock.lock()
    {
        let _ = writeln!(file, "{}", line);
    }
}

fn log_file_path() -> PathBuf {
    if let Ok(path) = std::env::var("MENTALOS_LOG_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Some(base) = directories::BaseDirs::new()
        && let Some(state_dir) = base.state_dir()
    {
        return state_dir.join("mentalOS").join("logs").join("mentalOS.log");
    }

    PathBuf::from("/tmp/mentalOS.log")
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
}

fn env_flag_default_true(name: &str) -> bool {
    std::env::var(name)
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("no")))
        .unwrap_or(true)
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

    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    } else {
        log::warn!("Could not get default display for CSS provider");
    }
}

#[cfg(test)]
mod tests {
    use super::expand_home_path;

    #[test]
    fn expand_home_path_preserves_absolute_path() {
        let path = expand_home_path("/tmp/workspace");
        assert_eq!(path, std::path::PathBuf::from("/tmp/workspace"));
    }

    #[test]
    fn expand_home_path_handles_tilde_prefix() {
        let path = expand_home_path("~/workspace");
        if let Some(home) = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf()) {
            assert_eq!(path, home.join("workspace"));
        } else {
            assert_eq!(path, std::path::PathBuf::from("~/workspace"));
        }
    }
}

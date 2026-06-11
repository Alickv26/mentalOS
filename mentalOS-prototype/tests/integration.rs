use httpmock::Method::POST;
use httpmock::MockServer;
use mental_os::agent_manager::AgentManager;
use mental_os::config::{AiConfig, OllamaConfig, OpenClawConfig, PathsConfig};
use mental_os::memory::MemoryManager;
use mental_os::openclaw::OpenClawClient;
use mental_os::router::{CommandExecutor, CommandOutput, CommandRouter};
use mental_os::whitelist::WhitelistManager;
use mental_os::workspace::WorkspaceManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

struct NoopExecutor;

impl CommandExecutor for NoopExecutor {
    fn execute(&self, command: &str) -> mental_os::Result<CommandOutput> {
        Ok(CommandOutput {
            command: command.to_string(),
            exit_code: 0,
            stdout: "ok".to_string(),
            stderr: String::new(),
        })
    }
}

#[tokio::test]
async fn components_work_together() {
    if std::env::var("MENTALOS_ENABLE_SOCKET_TESTS")
        .map(|v| v != "1")
        .unwrap_or(true)
    {
        return;
    }

    let server = MockServer::start_async().await;
    let response = r#"{"message":"ok","commands":["echo hello"]}"#;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST).path("/agent");
            then.status(200).body(response);
        })
        .await;

    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().join("workspaces");
    let workspace_manager = WorkspaceManager::new(workspace_root.clone());
    workspace_manager
        .create_workspace("demo", "openclaw")
        .unwrap();

    let memory = Arc::new(Mutex::new(MemoryManager::new(workspace_root.clone())));
    let whitelist_path = temp_dir.path().join("whitelist.json");
    let mut whitelist = WhitelistManager::load(whitelist_path).unwrap();
    whitelist.add_exact("echo hello");
    let whitelist = Arc::new(Mutex::new(whitelist));

    let mut config = mental_os::Config {
        ai: AiConfig::default(),
        openclaw: OpenClawConfig {
            transport: "http".to_string(),
            request_path: "/agent".to_string(),
            ..OpenClawConfig::default()
        },
        ollama: OllamaConfig::default(),
        paths: PathsConfig::default(),
        agents: std::collections::HashMap::new(),
    };
    config.openclaw.endpoint = server.base_url();
    config.ai.fallback_to_ollama = false;

    let openclaw = OpenClawClient::from_config(&config);
    let mut agent_manager = AgentManager::new(PathBuf::from("/tmp"));
    agent_manager.load_agents(config.agents.clone());
    let agent_manager = Arc::new(Mutex::new(agent_manager));
    let mut router = CommandRouter::new(openclaw, agent_manager, whitelist, memory, NoopExecutor);

    let result = router
        .handle_input("demo", "general", "hi", 5)
        .await
        .unwrap();

    assert_eq!(result.outputs.len(), 1);
    assert!(result.approvals_required.is_empty());
    mock.assert_async().await;
}

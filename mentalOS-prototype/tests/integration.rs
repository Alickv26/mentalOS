use mentalOS::config::{AiConfig, OllamaConfig, OpenClawConfig, PathsConfig};
use mentalOS::memory::MemoryManager;
use mentalOS::openclaw::OpenClawClient;
use mentalOS::router::{CommandExecutor, CommandOutput, CommandRouter};
use mentalOS::whitelist::WhitelistManager;
use mentalOS::workspace::WorkspaceManager;
use httpmock::Method::POST;
use httpmock::MockServer;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

struct NoopExecutor;

impl CommandExecutor for NoopExecutor {
    fn execute(&self, command: &str) -> mentalOS::Result<CommandOutput> {
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
    let server = MockServer::start_async().await;
    let response = r#"{"message":"ok","commands":["echo hello"]}"#;
    let mock = server.mock_async(|when, then| {
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

    let mut config = mentalOS::Config {
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
    let mut router = CommandRouter::new(openclaw, whitelist, memory, NoopExecutor);

    let result = router
        .handle_input("demo", "general", "hi", 5)
        .await
        .unwrap();

    assert_eq!(result.outputs.len(), 1);
    assert!(result.approvals_required.is_empty());
    mock.assert_async().await;
}

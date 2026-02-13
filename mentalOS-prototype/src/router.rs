use crate::error::{MentalOSError, Result};
use crate::memory::{MemoryManager, Role};
use crate::openclaw::OpenClawClient;
use crate::whitelist::{WhitelistDecision, WhitelistManager};
use log::{info, warn};
use regex::Regex;
use serde::Deserialize;
use shell_words::split;
use std::collections::HashSet;
use std::process::Command;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct RouterResponse {
    pub message: String,
    pub outputs: Vec<CommandOutput>,
    pub approvals_required: Vec<String>,
}

pub trait CommandExecutor: Send + Sync {
    fn execute(&self, command: &str) -> Result<CommandOutput>;
}

pub struct FirejailExecutor;

impl CommandExecutor for FirejailExecutor {
    fn execute(&self, command: &str) -> Result<CommandOutput> {
        let parts = split(command)
            .map_err(|err| MentalOSError::InvalidCommand(format!("Parse error: {err}")))?;
        let (program, args) = parts
            .split_first()
            .ok_or_else(|| MentalOSError::InvalidCommand("Empty command".into()))?;

        // Determine profile path
        // Check local directory first, then standard paths
        let profile_arg = if std::path::Path::new("firejail/openclaw.profile").exists() {
            Some("--profile=firejail/openclaw.profile".to_string())
        } else if std::path::Path::new("/etc/firejail/openclaw.profile").exists() {
            Some("--profile=openclaw".to_string())
        } else {
            None
        };

        let mut cmd = Command::new("firejail");
        cmd.arg("--quiet");
        if let Some(profile) = profile_arg {
            cmd.arg(profile);
        } else {
            // Warn if running without specific profile?
            // For now, just run with default firejail restrictions
        }
        
        // Prevent X11/GUI access if possible (headless)
        cmd.arg("--x11=none");

        cmd.arg("--");
        cmd.arg(program);
        cmd.args(args);

        let output = cmd.output().map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                MentalOSError::CommandFailed("firejail not found in PATH".into())
            } else {
                MentalOSError::Io(err)
            }
        })?;

        Ok(CommandOutput {
            command: command.to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

/// Routes user input to OpenClaw, parses commands, and executes approved actions.
///
/// # Examples
/// ```no_run
/// use mentalOS::router::{CommandRouter, FirejailExecutor};
/// use mentalOS::openclaw::OpenClawClient;
/// use mentalOS::whitelist::WhitelistManager;
/// use mentalOS::memory::MemoryManager;
/// use std::sync::{Arc, Mutex};
/// let config = mentalOS::Config::load().unwrap();
/// let openclaw = OpenClawClient::from_config(&config);
/// let whitelist = Arc::new(Mutex::new(WhitelistManager::load("/tmp/whitelist.json".into()).unwrap()));
/// let memory = Arc::new(Mutex::new(MemoryManager::new("/tmp/workspaces".into())));
/// let router = CommandRouter::new(openclaw, whitelist, memory, FirejailExecutor);
/// ```
pub struct CommandRouter<E: CommandExecutor> {
    openclaw: OpenClawClient,
    whitelist: Arc<Mutex<WhitelistManager>>,
    memory: Arc<Mutex<MemoryManager>>,
    executor: E,
}

impl<E: CommandExecutor> CommandRouter<E> {
    pub fn new(
        openclaw: OpenClawClient,
        whitelist: Arc<Mutex<WhitelistManager>>,
        memory: Arc<Mutex<MemoryManager>>,
        executor: E,
    ) -> Self {
        Self {
            openclaw,
            whitelist,
            memory,
            executor,
        }
    }

    pub fn list_agents(&self) -> Vec<String> {
        self.openclaw.list_agents()
    }

    pub fn get_current_provider(&self) -> String {
        self.openclaw.get_current_provider().to_string()
    }

    pub fn switch_agent(&mut self, name: &str) -> Result<String> {
        self.openclaw.switch_agent(name)
    }

    pub async fn handle_input(
        &mut self,
        workspace: &str,
        category: &str,
        input: &str,
        context_limit: usize,
    ) -> Result<RouterResponse> {
        // Intercept agent switching commands
        let switch_regex = Regex::new(r"(?i)^switch\s+(?:agent\s+)?to\s+(.+)$").unwrap();
        if let Some(caps) = switch_regex.captures(input) {
            let target = caps.get(1).unwrap().as_str().trim();
            match self.openclaw.switch_agent(target) {
                Ok(msg) => {
                    return Ok(RouterResponse {
                        message: msg,
                        outputs: Vec::new(),
                        approvals_required: Vec::new(),
                    });
                }
                Err(_) => {
                    let agents = self.openclaw.list_agents();
                    let msg = format!(
                        "Agent '{}' not found. Available agents: {}", 
                        target, 
                        agents.join(", ")
                    );
                    return Ok(RouterResponse {
                        message: msg,
                        outputs: Vec::new(),
                        approvals_required: Vec::new(),
                    });
                }
            }
        }

        {
            let memory = self
                .memory
                .lock()
                .map_err(|_| MentalOSError::Other("Memory lock poisoned".into()))?;
            memory.append_message(workspace, category, Role::User, input)?;
        }

        let context = {
            let memory = self
                .memory
                .lock()
                .map_err(|_| MentalOSError::Other("Memory lock poisoned".into()))?;
            memory.load_recent(workspace, category, context_limit)?
        };

        let ai_text = self.openclaw.send_message(input, &context).await?;
        let ai_response = parse_ai_response(&ai_text);

        let mut outputs = Vec::new();
        let mut approvals_required = Vec::new();

        for command in ai_response.commands {
            let decision = {
                let mut whitelist = self
                    .whitelist
                    .lock()
                    .map_err(|_| MentalOSError::Other("Whitelist lock poisoned".into()))?;
                whitelist.decision(&command)?
            };
            match decision {
                WhitelistDecision::Allowed => {
                    info!("Executing approved command: {}", command);
                    let result = self.executor.execute(&command)?;
                    outputs.push(result);
                }
                WhitelistDecision::NeedsApproval => {
                    warn!("Command requires approval: {}", command);
                    approvals_required.push(command);
                }
            }
        }

        {
            let memory = self
                .memory
                .lock()
                .map_err(|_| MentalOSError::Other("Memory lock poisoned".into()))?;
            memory.append_message(
                workspace,
                category,
                Role::Assistant,
                ai_response.message.clone(),
            )?;
        }

        Ok(RouterResponse {
            message: ai_response.message,
            outputs,
            approvals_required,
        })
    }
}

#[derive(Debug, Deserialize)]
struct StructuredResponse {
    message: Option<String>,
    commands: Option<Vec<String>>,
}

#[derive(Debug)]
struct ParsedResponse {
    message: String,
    commands: Vec<String>,
}

fn parse_ai_response(text: &str) -> ParsedResponse {
    if let Ok(parsed) = serde_json::from_str::<StructuredResponse>(text) {
        let commands = parsed.commands.unwrap_or_default();
        let message = parsed.message.unwrap_or_else(|| text.to_string());
        return ParsedResponse { message, commands };
    }

    let mut commands = Vec::new();
    let mut seen = HashSet::new();

    for (lang, block) in extract_code_blocks(text) {
        if lang == "json" {
            if let Ok(parsed) = serde_json::from_str::<StructuredResponse>(&block) {
                if let Some(mut parsed_commands) = parsed.commands {
                    for cmd in parsed_commands.drain(..) {
                        push_unique(&mut commands, &mut seen, cmd);
                    }
                }
            }
        }
        if lang == "bash" || lang == "sh" || lang == "shell" {
            for line in block.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                push_unique(&mut commands, &mut seen, line.to_string());
            }
        }
    }

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(cmd) = trimmed.strip_prefix("$ ") {
            push_unique(&mut commands, &mut seen, cmd.to_string());
        }
        if let Some(cmd) = trimmed.strip_prefix("> ") {
            push_unique(&mut commands, &mut seen, cmd.to_string());
        }
    }

    ParsedResponse {
        message: text.to_string(),
        commands,
    }
}

fn extract_code_blocks(text: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let re = Regex::new(r"(?s)```(?P<lang>\w+)?\n(?P<body>.*?)```").unwrap();
    for cap in re.captures_iter(text) {
        let lang = cap
            .name("lang")
            .map(|m| m.as_str().to_lowercase())
            .unwrap_or_default();
        let body = cap
            .name("body")
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        blocks.push((lang, body));
    }
    blocks
}

fn push_unique(commands: &mut Vec<String>, seen: &mut HashSet<String>, command: String) {
    let trimmed = command.trim().to_string();
    if trimmed.is_empty() {
        return;
    }
    if seen.insert(trimmed.clone()) {
        commands.push(trimmed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AiConfig, OllamaConfig, OpenClawConfig, PathsConfig};
    use crate::openclaw::OpenClawClient;
    use crate::whitelist::WhitelistManager;
    use httpmock::Method::POST;
    use httpmock::MockServer;
    use tempfile::TempDir;

    struct NoopExecutor;

    impl CommandExecutor for NoopExecutor {
        fn execute(&self, command: &str) -> Result<CommandOutput> {
            Ok(CommandOutput {
                command: command.to_string(),
                exit_code: 0,
                stdout: "ok".to_string(),
                stderr: String::new(),
            })
        }
    }

    fn base_config() -> crate::config::Config {
        crate::config::Config {
            ai: AiConfig::default(),
            openclaw: OpenClawConfig {
                transport: "http".to_string(),
                request_path: "/agent".to_string(),
                ..OpenClawConfig::default()
            },
            ollama: OllamaConfig::default(),
            paths: PathsConfig::default(),
            agents: std::collections::HashMap::new(),
        }
    }

    #[tokio::test]
    async fn router_parses_commands_and_executes() {
        let server = MockServer::start_async().await;
        let response = r#"{"message":"ok","commands":["echo hello"]}"#;
        let mock = server.mock_async(|when, then| {
            when.method(POST).path("/agent");
            then.status(200).body(response);
        })
        .await;

        let mut config = base_config();
        config.openclaw.endpoint = server.base_url();
        config.ai.fallback_to_ollama = false;

        let openclaw = OpenClawClient::from_config(&config);
        let temp_dir = TempDir::new().unwrap();
        let memory = Arc::new(Mutex::new(MemoryManager::new(
            temp_dir.path().to_path_buf(),
        )));
        let whitelist_path = temp_dir.path().join("whitelist.json");
        let mut whitelist = WhitelistManager::load(whitelist_path).unwrap();
        whitelist.add_exact("echo hello");
        let whitelist = Arc::new(Mutex::new(whitelist));

        let mut router = CommandRouter::new(openclaw, whitelist, memory, NoopExecutor);
        let result = router
            .handle_input("demo", "general", "hi", 5)
            .await
            .unwrap();

        assert_eq!(result.outputs.len(), 1);
        assert!(result.approvals_required.is_empty());
        mock.assert_async().await;
    }
}

use crate::error::{MentalOSError, Result};
use crate::memory::{MemoryManager, Message, MessageAction, Role};
use crate::openclaw::OpenClawClient;
use crate::project_handler::ProjectHandler;
use crate::task_tracker::TaskTracker;
use crate::whitelist::{WhitelistDecision, WhitelistManager};
use crate::workspace::WorkspaceManager;
use chrono::Utc;
use log::{info, warn};
use regex::Regex;
use serde::Deserialize;
use shell_words::split;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wait_timeout::ChildExt;

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
    fn emergency_stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct FirejailExecutor {
    timeout: Duration,
    allow_network: bool,
    active_pids: Arc<Mutex<HashSet<u32>>>,
}

impl Default for FirejailExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl FirejailExecutor {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(300),
            allow_network: false,
            active_pids: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn with_timeout_secs(mut self, timeout_secs: u64) -> Self {
        self.timeout = Duration::from_secs(timeout_secs);
        self
    }

    pub fn with_network(mut self, allow_network: bool) -> Self {
        self.allow_network = allow_network;
        self
    }

    fn remember_pid(&self, pid: u32) -> Result<()> {
        let mut active = self
            .active_pids
            .lock()
            .map_err(|_| MentalOSError::Other("Executor PID lock poisoned".into()))?;
        active.insert(pid);
        Ok(())
    }

    fn forget_pid(&self, pid: u32) -> Result<()> {
        let mut active = self
            .active_pids
            .lock()
            .map_err(|_| MentalOSError::Other("Executor PID lock poisoned".into()))?;
        active.remove(&pid);
        Ok(())
    }

    fn kill_known_children(&self) -> Result<()> {
        let pids = {
            let active = self
                .active_pids
                .lock()
                .map_err(|_| MentalOSError::Other("Executor PID lock poisoned".into()))?;
            active.iter().copied().collect::<Vec<u32>>()
        };
        for pid in pids {
            let _ = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        Ok(())
    }
}

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
        if !self.allow_network {
            cmd.arg("--net=none");
        }

        cmd.arg("--");
        cmd.arg(program);
        cmd.args(args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                MentalOSError::CommandFailed("firejail not found in PATH".into())
            } else {
                MentalOSError::Io(err)
            }
        })?;

        let pid = child.id();
        self.remember_pid(pid)?;
        let wait_status = child.wait_timeout(self.timeout)?;
        let output = if wait_status.is_some() {
            child.wait_with_output()?
        } else {
            let _ = child.kill();
            let _ = child.wait();
            let _ = self.forget_pid(pid);
            return Err(MentalOSError::CommandFailed(format!(
                "Command timed out after {} seconds",
                self.timeout.as_secs()
            )));
        };
        self.forget_pid(pid)?;

        Ok(CommandOutput {
            command: command.to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    fn emergency_stop(&self) -> Result<()> {
        self.kill_known_children()
    }
}

/// Routes user input to OpenClaw, parses commands, and executes approved actions.
///
/// # Examples
/// ```no_run
/// use mental_os::router::{CommandRouter, FirejailExecutor};
/// use mental_os::openclaw::OpenClawClient;
/// use mental_os::whitelist::WhitelistManager;
/// use mental_os::memory::MemoryManager;
/// use std::sync::{Arc, Mutex};
/// let config = mental_os::Config::load().unwrap();
/// let openclaw = OpenClawClient::from_config(&config);
/// let whitelist = Arc::new(Mutex::new(WhitelistManager::load("/tmp/whitelist.json".into()).unwrap()));
/// let memory = Arc::new(Mutex::new(MemoryManager::new("/tmp/workspaces".into())));
/// let router = CommandRouter::new(openclaw, whitelist, memory, FirejailExecutor::new());
/// ```
pub struct CommandRouter<E: CommandExecutor> {
    openclaw: OpenClawClient,
    whitelist: Arc<Mutex<WhitelistManager>>,
    memory: Arc<Mutex<MemoryManager>>,
    executor: E,
    project_handler: Option<ProjectHandler>,
    task_tracker: Option<TaskTracker>,
    workspace_manager: Option<WorkspaceManager>,
}

#[derive(Debug, Clone)]
pub struct ProjectConfirmation {
    pub name: String,
    pub language: Option<String>,
    pub framework: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RouterConfirmation {
    Project(ProjectConfirmation),
    None,
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
            project_handler: None,
            task_tracker: None,
            workspace_manager: None,
        }
    }

    pub fn with_project_handler(mut self, handler: ProjectHandler) -> Self {
        self.project_handler = Some(handler);
        self
    }

    pub fn with_task_tracker(mut self, tracker: TaskTracker) -> Self {
        self.task_tracker = Some(tracker);
        self
    }

    pub fn with_workspace_manager(mut self, manager: WorkspaceManager) -> Self {
        self.workspace_manager = Some(manager);
        self
    }

    pub fn detect_project_intent(&self, input: &str) -> Option<ProjectConfirmation> {
        if let Some(ref handler) = self.project_handler {
            handler
                .detect_project_intent(input)
                .map(|req| ProjectConfirmation {
                    name: req.name,
                    language: req.language,
                    framework: req.framework,
                })
        } else {
            None
        }
    }

    pub fn create_project(
        &self,
        name: &str,
        language: Option<String>,
        framework: Option<String>,
    ) -> Result<(bool, String, Option<String>)> {
        if let Some(ref handler) = self.project_handler {
            let request = crate::project_handler::ProjectRequest {
                name: name.to_string(),
                language,
                framework,
                description: None,
                auto_setup: true,
            };
            let agent = self.openclaw.get_current_provider();
            match handler.scaffold_project(&request, agent) {
                Ok(response) => {
                    if response.success
                        && let Ok(memory) = self.memory.lock()
                    {
                        let mut actions = Vec::new();
                        if let Some(path) = response.path.clone() {
                            actions.push(MessageAction {
                                action_type: "create_project".to_string(),
                                path: Some(path.clone()),
                                command: None,
                            });
                        }
                        let _ = memory.append_message_with_actions(
                            "default",
                            "general",
                            Role::Assistant,
                            response.message.clone(),
                            actions,
                        );
                        if let Some(path) = response.path.clone() {
                            let _ = memory
                                .set_workspace_for_active_session("default", "general", &path);
                        }
                    }
                    Ok((response.success, response.message, response.path))
                }
                Err(e) => Ok((false, e.to_string(), None)),
            }
        } else {
            Ok((false, "Project handler not initialized".to_string(), None))
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

    pub fn run_project_command(
        &mut self,
        workspace: &str,
        category: &str,
        command_type: &str,
    ) -> Result<RouterResponse> {
        self.handle_project_command(workspace, category, command_type)
    }

    pub fn execute_approved_command(&self, command: &str) -> Result<CommandOutput> {
        self.executor.execute(command)
    }

    pub fn emergency_stop(&self) -> Result<String> {
        self.executor.emergency_stop()?;
        Ok("Emergency stop executed. Running sandboxed commands were terminated.".to_string())
    }

    pub async fn handle_input(
        &mut self,
        workspace: &str,
        category: &str,
        input: &str,
        context_limit: usize,
    ) -> Result<RouterResponse> {
        // Intercept agent switching commands
        if let Some(target) = parse_switch_target(input) {
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

        if let Some(command_type) = detect_project_command_intent(input) {
            let response = self.handle_project_command(workspace, category, command_type)?;
            return Ok(response);
        }

        let mut context = {
            let memory = self
                .memory
                .lock()
                .map_err(|_| MentalOSError::Other("Memory lock poisoned".into()))?;
            memory.load_recent(workspace, category, context_limit)?
        };
        let workspace_path = self.resolve_workspace_path(workspace)?;
        if let Some(smart_context) =
            build_smart_context_message(&context, workspace_path.as_deref(), input)
        {
            context.push(smart_context);
        }

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
            let mut actions = Vec::new();
            for output in &outputs {
                actions.push(MessageAction {
                    action_type: "run_command".to_string(),
                    path: None,
                    command: Some(output.command.clone()),
                });
            }
            for command in &approvals_required {
                actions.push(MessageAction {
                    action_type: "approval_required".to_string(),
                    path: None,
                    command: Some(command.clone()),
                });
            }
            memory.append_message_with_actions(
                workspace,
                category,
                Role::Assistant,
                ai_response.message.clone(),
                actions,
            )?;
        }

        if let Some(ref mut tracker) = self.task_tracker {
            let _ = tracker.extract_and_add_tasks(input, None, None);
            let _ = tracker.extract_and_add_tasks(&ai_response.message, None, None);
        }

        Ok(RouterResponse {
            message: ai_response.message,
            outputs,
            approvals_required,
        })
    }

    fn handle_project_command(
        &mut self,
        workspace: &str,
        category: &str,
        command_type: &str,
    ) -> Result<RouterResponse> {
        let handler = match self.project_handler.as_ref() {
            Some(handler) => handler,
            None => {
                return Ok(RouterResponse {
                    message: "Project commands are not enabled yet.".to_string(),
                    outputs: Vec::new(),
                    approvals_required: Vec::new(),
                });
            }
        };

        let workspace_path = match self.resolve_workspace_path(workspace)? {
            Some(path) => path,
            None => {
                return Ok(RouterResponse {
                    message: "No project workspace found. Create a project first.".to_string(),
                    outputs: Vec::new(),
                    approvals_required: Vec::new(),
                });
            }
        };

        let related_session_note = {
            let memory = self
                .memory
                .lock()
                .map_err(|_| MentalOSError::Other("Memory lock poisoned".into()))?;
            match memory.find_related_session_for_project(workspace, &workspace_path)? {
                Some(session) => {
                    let label = session.title.unwrap_or(session.session_id);
                    Some(format!("(related conversation: {label})"))
                }
                None => None,
            }
        };

        let project_command = match handler.resolve_project_command(&workspace_path, command_type) {
            Ok(command) => command,
            Err(err) => {
                return Ok(RouterResponse {
                    message: err.to_string(),
                    outputs: Vec::new(),
                    approvals_required: Vec::new(),
                });
            }
        };

        let execution_command = build_workspace_command(&workspace_path, &project_command);
        let decision = {
            let mut whitelist = self
                .whitelist
                .lock()
                .map_err(|_| MentalOSError::Other("Whitelist lock poisoned".into()))?;
            whitelist.decision(&project_command)?
        };

        let mut outputs = Vec::new();
        let mut approvals_required = Vec::new();
        let message = match decision {
            WhitelistDecision::Allowed => {
                let output = self.executor.execute(&execution_command)?;
                outputs.push(output);
                let mut msg = format!(
                    "Executed project {} command in {}.",
                    command_type,
                    workspace_path.display()
                );
                if let Some(note) = &related_session_note {
                    msg.push(' ');
                    msg.push_str(note);
                }
                msg
            }
            WhitelistDecision::NeedsApproval => {
                approvals_required.push(execution_command);
                let mut msg = format!(
                    "Project command requires approval:\n{}\n(workspace: {})",
                    project_command,
                    workspace_path.display()
                );
                if let Some(note) = &related_session_note {
                    msg.push('\n');
                    msg.push_str(note);
                }
                msg
            }
        };

        {
            let memory = self
                .memory
                .lock()
                .map_err(|_| MentalOSError::Other("Memory lock poisoned".into()))?;
            let action_type = if outputs.is_empty() {
                "project_command_requested"
            } else {
                "project_command_executed"
            };
            let actions = vec![MessageAction {
                action_type: action_type.to_string(),
                path: Some(workspace_path.display().to_string()),
                command: Some(project_command),
            }];
            memory.append_message_with_actions(
                workspace,
                category,
                Role::Assistant,
                message.clone(),
                actions,
            )?;
        }

        Ok(RouterResponse {
            message,
            outputs,
            approvals_required,
        })
    }

    fn resolve_workspace_path(&self, workspace: &str) -> Result<Option<PathBuf>> {
        let mut candidates = Vec::new();

        let direct = PathBuf::from(workspace);
        if direct.exists() {
            candidates.push(direct);
        }

        if let Some(manager) = self.workspace_manager.as_ref() {
            let named = manager.workspace_root().join(workspace);
            if named.exists() {
                candidates.push(named);
            }

            if candidates.is_empty() {
                let mut workspaces = manager.list_workspaces()?;
                workspaces.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
                if let Some(last) = workspaces.pop() {
                    candidates.push(last);
                }
            }
        }

        Ok(candidates.into_iter().next())
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
        if lang == "json"
            && let Ok(parsed) = serde_json::from_str::<StructuredResponse>(&block)
            && let Some(mut parsed_commands) = parsed.commands
        {
            for cmd in parsed_commands.drain(..) {
                push_unique(&mut commands, &mut seen, cmd);
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
    let re = match Regex::new(r"(?s)```(?P<lang>\w+)?\n(?P<body>.*?)```") {
        Ok(re) => re,
        Err(err) => {
            log::error!("Failed to compile code-block regex: {}", err);
            return blocks;
        }
    };
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

fn parse_switch_target(input: &str) -> Option<&str> {
    let switch_regex = match Regex::new(r"(?i)^switch\s+(?:agent\s+)?to\s+(.+)$") {
        Ok(re) => re,
        Err(err) => {
            log::error!("Failed to compile switch-agent regex: {}", err);
            return None;
        }
    };
    let caps = switch_regex.captures(input)?;
    caps.get(1).map(|m| m.as_str().trim())
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

fn detect_project_command_intent(input: &str) -> Option<&'static str> {
    let normalized = input.trim().to_ascii_lowercase();
    let run_patterns = [
        "run project",
        "run the project",
        "run this project",
        "run this",
        "start project",
        "start the project",
        "start this",
    ];
    let test_patterns = ["test project", "test the project", "test this", "run tests"];
    let setup_patterns = [
        "setup project",
        "setup the project",
        "install dependencies",
        "build project",
        "build the project",
        "build this",
    ];

    if run_patterns
        .iter()
        .any(|p| normalized == *p || normalized.starts_with(&format!("{p} ")))
    {
        return Some("run");
    }
    if test_patterns
        .iter()
        .any(|p| normalized == *p || normalized.starts_with(&format!("{p} ")))
    {
        return Some("test");
    }
    if setup_patterns
        .iter()
        .any(|p| normalized == *p || normalized.starts_with(&format!("{p} ")))
    {
        return Some("setup");
    }
    None
}

fn build_smart_context_message(
    context: &[Message],
    workspace_path: Option<&Path>,
    input: &str,
) -> Option<Message> {
    let cwd = workspace_path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<unknown>".to_string());
    let open_files = workspace_path.map(list_open_files).unwrap_or_default();
    let recent_commands = extract_recent_commands(context, 5);
    let summary = summarize_recent_conversation(context, 3);
    let goals = extract_user_goals(context, input, 4);

    let mut lines = Vec::new();
    lines.push("Runtime context snapshot:".to_string());
    lines.push(format!("- current_working_directory: {cwd}"));
    lines.push(format!(
        "- open_files: {}",
        if open_files.is_empty() {
            "<none>".to_string()
        } else {
            open_files.join(", ")
        }
    ));
    lines.push(format!(
        "- recent_commands: {}",
        if recent_commands.is_empty() {
            "<none>".to_string()
        } else {
            recent_commands.join(" | ")
        }
    ));
    lines.push(format!(
        "- previous_summary: {}",
        if summary.is_empty() {
            "<none>".to_string()
        } else {
            summary
        }
    ));
    lines.push(format!(
        "- inferred_goals: {}",
        if goals.is_empty() {
            "<none>".to_string()
        } else {
            goals.join(" | ")
        }
    ));

    Some(Message {
        role: Role::System,
        content: lines.join("\n"),
        timestamp: Utc::now(),
        actions: Vec::new(),
    })
}

fn list_open_files(workspace_path: &Path) -> Vec<String> {
    if !workspace_path.exists() {
        return Vec::new();
    }
    let mut files = Vec::new();
    let entries = match std::fs::read_dir(workspace_path) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            files.push(
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string()),
            );
        }
        if files.len() >= 8 {
            break;
        }
    }
    files
}

fn extract_recent_commands(context: &[Message], limit: usize) -> Vec<String> {
    let mut commands = Vec::new();
    for message in context.iter().rev() {
        for action in &message.actions {
            if let Some(command) = &action.command
                && !commands.contains(command)
            {
                commands.push(command.clone());
            }
            if commands.len() >= limit {
                break;
            }
        }
        if commands.len() >= limit {
            break;
        }
    }
    commands.reverse();
    commands
}

fn summarize_recent_conversation(context: &[Message], count: usize) -> String {
    let mut parts = Vec::new();
    for msg in context.iter().rev().take(count).rev() {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        };
        let compact = msg.content.lines().next().unwrap_or("").trim();
        if compact.is_empty() {
            continue;
        }
        let clipped = if compact.len() > 120 {
            format!("{}...", &compact[..120])
        } else {
            compact.to_string()
        };
        parts.push(format!("{role}: {clipped}"));
    }
    parts.join(" | ")
}

fn extract_user_goals(context: &[Message], input: &str, limit: usize) -> Vec<String> {
    let mut goals = Vec::new();
    for message in context.iter().rev() {
        if message.role != Role::User {
            continue;
        }
        if let Some(goal) = infer_goal_phrase(&message.content)
            && !goals.contains(&goal)
        {
            goals.push(goal);
        }
        if goals.len() >= limit {
            break;
        }
    }
    if let Some(goal) = infer_goal_phrase(input)
        && !goals.contains(&goal)
    {
        goals.push(goal);
    }
    goals.reverse();
    goals
}

fn infer_goal_phrase(text: &str) -> Option<String> {
    let lowered = text.to_lowercase();
    for marker in ["i need to", "todo", "goal", "remember to"] {
        if let Some(idx) = lowered.find(marker) {
            let raw = text[idx..].trim();
            if raw.is_empty() {
                return None;
            }
            return Some(raw.to_string());
        }
    }
    None
}

fn shell_quote(value: &str) -> String {
    let mut out = String::from("'");
    for c in value.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

fn build_workspace_command(workspace: &std::path::Path, command: &str) -> String {
    let script = format!(
        "cd {} && {}",
        shell_quote(&workspace.display().to_string()),
        command
    );
    format!("sh -lc {}", shell_quote(&script))
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

    fn socket_tests_enabled() -> bool {
        std::env::var("MENTALOS_ENABLE_SOCKET_TESTS")
            .map(|v| v == "1")
            .unwrap_or(false)
    }

    #[tokio::test]
    async fn router_parses_commands_and_executes() {
        if !socket_tests_enabled() {
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

    #[test]
    fn smart_context_message_includes_workspace_and_commands() {
        let msg = build_smart_context_message(
            &[Message {
                role: Role::Assistant,
                content: "done".to_string(),
                timestamp: Utc::now(),
                actions: vec![MessageAction {
                    action_type: "run_command".to_string(),
                    path: None,
                    command: Some("cargo test".to_string()),
                }],
            }],
            Some(std::path::Path::new("/tmp/workspaces/demo")),
            "I need to deploy this",
        )
        .unwrap();
        assert!(msg.content.contains("current_working_directory"));
        assert!(msg.content.contains("recent_commands"));
        assert!(msg.content.contains("cargo test"));
        assert!(msg.content.contains("inferred_goals"));
    }

    #[test]
    fn extract_user_goals_deduplicates_and_respects_limit() {
        let now = Utc::now();
        let context = vec![
            Message {
                role: Role::User,
                content: "I need to ship v0.7".to_string(),
                timestamp: now,
                actions: Vec::new(),
            },
            Message {
                role: Role::User,
                content: "TODO stabilize sync export".to_string(),
                timestamp: now,
                actions: Vec::new(),
            },
            Message {
                role: Role::Assistant,
                content: "Acknowledged".to_string(),
                timestamp: now,
                actions: Vec::new(),
            },
            Message {
                role: Role::User,
                content: "remember to verify onboarding".to_string(),
                timestamp: now,
                actions: Vec::new(),
            },
        ];

        let goals = extract_user_goals(&context, "goal polish accessibility", 2);
        assert_eq!(goals.len(), 3);
        assert!(
            goals
                .iter()
                .any(|g| g.to_lowercase().contains("goal polish"))
        );
        assert!(
            goals
                .iter()
                .any(|g| g.to_lowercase().contains("todo stabilize"))
        );
        assert!(
            goals
                .iter()
                .any(|g| g.to_lowercase().contains("remember to verify"))
        );

        let deduped = extract_user_goals(&context, "TODO stabilize sync export", 4);
        let dup_count = deduped
            .iter()
            .filter(|g| g.to_lowercase().contains("todo stabilize sync export"))
            .count();
        assert_eq!(dup_count, 1);
    }

    #[test]
    fn parse_ai_response_extracts_unique_commands_from_mixed_blocks() {
        let text = r#"
message text
```json
{"commands":["cargo test","cargo test","cargo run"]}
```
```bash
cargo run
# comment
echo hi
```
$ echo hi
> git status
"#;

        let parsed = parse_ai_response(text);
        assert_eq!(
            parsed.commands,
            vec![
                "cargo test".to_string(),
                "cargo run".to_string(),
                "echo hi".to_string(),
                "git status".to_string(),
            ]
        );
    }

    #[test]
    fn parse_ai_response_handles_invalid_json_block_without_panicking() {
        let text = r#"
```json
{not valid json}
```
```sh
pwd
```
"#;

        let parsed = parse_ai_response(text);
        assert_eq!(parsed.commands, vec!["pwd".to_string()]);
    }

    #[test]
    fn detect_project_command_intent_handles_phrase_variants() {
        assert_eq!(
            detect_project_command_intent("run this project now"),
            Some("run")
        );
        assert_eq!(
            detect_project_command_intent("test the project please"),
            Some("test")
        );
        assert_eq!(
            detect_project_command_intent("build this quickly"),
            Some("setup")
        );
        assert_eq!(detect_project_command_intent("open docs"), None);
    }

    #[test]
    fn parse_switch_target_handles_spacing_and_optional_agent_keyword() {
        assert_eq!(parse_switch_target("switch to ollama"), Some("ollama"));
        assert_eq!(
            parse_switch_target("switch agent to   openclaw  "),
            Some("openclaw")
        );
        assert_eq!(
            parse_switch_target("switch Agent to phi3:mini"),
            Some("phi3:mini")
        );
        assert_eq!(parse_switch_target("switch openclaw"), None);
        assert_eq!(parse_switch_target("switch to"), None);
    }
}

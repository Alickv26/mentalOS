use crate::router::CommandOutput;

/// Requests sent from the UI to the Backend.
#[derive(Debug, Clone)]
pub enum BackendRequest {
    /// A raw text input from the user (e.g. from Omni-Pill).
    Input {
        text: String,
        workspace: String,
        category: String,
    },
    /// Use this to forward an approval decision back to the backend
    /// (requires more complex logic, for now we just re-submit or handle locally).
    /// actually, for M0.4 we might just need to re-execute the command if approved?
    /// Or better, the backend pauses?
    /// Simple approach: The backend returns "NeedsApproval(cmd)".
    /// If UI approves, UI sends "Execute(cmd)".
    /// Execute a command (user approved).
    ExecuteCommand(String),
    /// Immediately stop all AI/backend operations.
    EmergencyStop,
    /// Request the list of available agents.
    GetAgents,
    /// Request to switch the active agent.
    SwitchAgent(String),
    /// Request to create a project (user confirmed).
    CreateProject {
        name: String,
        language: Option<String>,
        framework: Option<String>,
    },
    /// Request project command execution (run, test, setup).
    RunProjectCommand {
        workspace: String,
        command_type: String,
    },
    /// Check whether the current AI provider is reachable.
    CheckProviderHealth,
}

/// Responses sent from the Backend to the UI.
#[derive(Debug, Clone)]
pub enum BackendResponse {
    /// Non-chat status update for transient UI notifications.
    Status(String),
    /// A standard text response (e.g. from LLM).
    Chat(String),
    /// A command execution result.
    CommandResult(CommandOutput),
    /// The backend requires approval for a command.
    ApprovalRequired(String),
    /// An error occurred.
    Error(String),
    /// The list of available agents and the currently active one.
    AgentList {
        agents: Vec<String>,
        current: String,
    },
    /// Confirmation that the agent was switched.
    AgentSwitched(String),
    /// Project creation requires confirmation.
    ProjectConfirmationRequired {
        name: String,
        language: Option<String>,
        framework: Option<String>,
    },
    /// Project creation result.
    ProjectCreated {
        success: bool,
        path: Option<String>,
        message: String,
    },
    /// Provider health check result.
    ProviderHealth {
        provider: String,
        healthy: bool,
    },
    /// Circuit breaker state changed.
    ///
    /// Sent after every `handle_input` call so the UI can reflect the
    /// current breaker state in real time (Closed = normal, Open = red dot,
    /// HalfOpen = yellow dot).
    CircuitStateChanged(CircuitState),
    /// A workspace file-change event from the workspace-monitor service.
    ///
    /// The `mentalos-workspace-monitor` systemd service writes JSON Lines
    /// to `~/workspaces/.mentalOS/events.jsonl`. The `workspace_events`
    /// module tails that file and forwards each parsed event here. The
    /// UI can use this to refresh project lists, memory browser, etc.
    WorkspaceEvent(crate::workspace_events::WorkspaceEvent),
}

/// Re-export CircuitState so message consumers don't need a separate import.
pub use crate::providers::circuit_breaker::CircuitState;


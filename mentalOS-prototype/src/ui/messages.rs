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
    ExecuteCommand(String),
}

/// Responses sent from the Backend to the UI.
#[derive(Debug, Clone)]
pub enum BackendResponse {
    /// A standard text response (e.g. from LLM).
    Chat(String),
    /// A command execution result.
    CommandResult(CommandOutput),
    /// The backend requires approval for a command.
    ApprovalRequired(String),
    /// An error occurred.
    Error(String),
}

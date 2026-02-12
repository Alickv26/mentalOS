use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum MentalOSError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("config not found at {0}")]
    ConfigMissing(PathBuf),
    #[error("config is invalid: {0}")]
    ConfigInvalid(String),
    #[error("command not approved: {0}")]
    NotApproved(String),
    #[error("command execution failed: {0}")]
    CommandFailed(String),
    #[error("openclaw unavailable: {0}")]
    OpenClawUnavailable(String),
    #[error("invalid command: {0}")]
    InvalidCommand(String),
    #[error("other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, MentalOSError>;

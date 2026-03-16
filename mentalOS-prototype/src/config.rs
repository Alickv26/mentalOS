use crate::error::{MentalOSError, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Runtime configuration loaded from `~/.config/mentalOS/config.toml`.
///
/// # Examples
/// ```no_run
/// let config = mental_os::Config::load().unwrap();
/// println!("provider={}", config.ai.provider);
/// ```
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub openclaw: OpenClawConfig,
    #[serde(default)]
    pub ollama: OllamaConfig,
    #[serde(default)]
    pub paths: PathsConfig,
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub description: Option<String>,
    pub provider: String, // "openclaw", "ollama", "claude", etc.
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub executable: Option<String>, // For local agents like OpenCode
    pub arguments: Option<Vec<String>>, // CLI args
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    // ... items from original ...
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_fallback_to_ollama")]
    pub fallback_to_ollama: bool,
    #[serde(default = "default_context_messages")]
    pub context_messages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawConfig {
    #[serde(default = "default_openclaw_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_openclaw_request_path")]
    pub request_path: String,
    #[serde(default = "default_openclaw_transport")]
    pub transport: String,
    #[serde(default)]
    pub token: String,
    #[serde(default = "default_openclaw_cli_path")]
    pub cli_path: String,
    #[serde(default)]
    pub cli_args: Vec<String>,
    #[serde(default)]
    pub auto_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_model")]
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    #[serde(default = "default_workspace_dir")]
    pub workspace_dir: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: default_model(),
            fallback_to_ollama: default_fallback_to_ollama(),
            context_messages: default_context_messages(),
        }
    }
}

impl Default for OpenClawConfig {
    fn default() -> Self {
        Self {
            endpoint: default_openclaw_endpoint(),
            request_path: default_openclaw_request_path(),
            transport: default_openclaw_transport(),
            token: String::new(),
            cli_path: default_openclaw_cli_path(),
            cli_args: Vec::new(),
            auto_start: false,
        }
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            endpoint: default_ollama_endpoint(),
            model: default_model(),
        }
    }
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            workspace_dir: default_workspace_dir(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ai: AiConfig::default(),
            openclaw: OpenClawConfig::default(),
            ollama: OllamaConfig::default(),
            paths: PathsConfig::default(),
            agents: HashMap::new(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Err(MentalOSError::ConfigMissing(path));
        }
        let contents = fs::read_to_string(&path)?;
        let mut config: Config = toml::from_str(&contents)?;
        config.normalize_paths()?;
        Ok(config)
    }

    pub fn workspace_dir(&self) -> Result<PathBuf> {
        expand_tilde(&self.paths.workspace_dir)
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut normalized = self.clone();
        normalized.normalize_paths()?;
        let serialized = toml::to_string_pretty(&normalized).map_err(|err| {
            MentalOSError::ConfigInvalid(format!("Failed to serialize config: {err}"))
        })?;
        fs::write(&path, serialized)?;
        Ok(path)
    }

    fn normalize_paths(&mut self) -> Result<()> {
        self.paths.workspace_dir = expand_tilde_to_string(&self.paths.workspace_dir)?;
        if !self.openclaw.cli_path.is_empty() {
            self.openclaw.cli_path = expand_tilde_to_string(&self.openclaw.cli_path)?;
        }
        Ok(())
    }
}

pub fn config_path() -> Result<PathBuf> {
    let base = BaseDirs::new().ok_or_else(|| MentalOSError::ConfigInvalid("No home dir".into()))?;
    Ok(base.config_dir().join("mentalOS").join("config.toml"))
}

fn expand_tilde_to_string(value: &str) -> Result<String> {
    let path = expand_tilde(value)?;
    Ok(path.to_string_lossy().to_string())
}

fn expand_tilde(value: &str) -> Result<PathBuf> {
    if value.starts_with("~/") || value == "~" {
        let base = BaseDirs::new()
            .ok_or_else(|| MentalOSError::ConfigInvalid("No home dir".into()))?
            .home_dir()
            .to_path_buf();
        if value == "~" {
            return Ok(base);
        }
        return Ok(base.join(value.trim_start_matches("~/")));
    }
    Ok(Path::new(value).to_path_buf())
}

fn default_provider() -> String {
    "openclaw".to_string()
}

fn default_model() -> String {
    "phi3:mini".to_string()
}

fn default_fallback_to_ollama() -> bool {
    true
}

fn default_context_messages() -> usize {
    20
}

fn default_openclaw_endpoint() -> String {
    "http://127.0.0.1:18789".to_string()
}

fn default_openclaw_request_path() -> String {
    "/agent".to_string()
}

fn default_openclaw_transport() -> String {
    "cli".to_string()
}

fn default_openclaw_cli_path() -> String {
    "openclaw".to_string()
}

fn default_ollama_endpoint() -> String {
    "http://127.0.0.1:11434".to_string()
}

fn default_workspace_dir() -> String {
    "~/workspaces".to_string()
}

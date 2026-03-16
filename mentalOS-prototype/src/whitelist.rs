use crate::error::{MentalOSError, Result};
use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Whitelist {
    pub version: u8,
    #[serde(default)]
    pub exact: Vec<String>,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub temporary: Vec<TemporaryApproval>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporaryApproval {
    pub command: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhitelistDecision {
    Allowed,
    NeedsApproval,
}

/// Manages command approvals for shell execution.
///
/// # Examples
/// ```no_run
/// use mentalOS::whitelist::WhitelistManager;
/// use std::path::PathBuf;
/// let manager = WhitelistManager::load(PathBuf::from("/tmp/whitelist.json")).unwrap();
/// ```
pub struct WhitelistManager {
    path: PathBuf,
    data: Whitelist,
}

impl WhitelistManager {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            data: Whitelist {
                version: 1,
                exact: Vec::new(),
                patterns: Vec::new(),
                temporary: Vec::new(),
            },
        }
    }

    pub fn load(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new(path));
        }

        let contents = fs::read_to_string(&path)?;
        let mut data: Whitelist = serde_json::from_str(&contents)?;
        data.temporary.retain(|entry| entry.expires_at > Utc::now());
        Ok(Self { path, data })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_string_pretty(&self.data)?;
        fs::write(&self.path, serialized)?;
        Ok(())
    }

    pub fn decision(&mut self, command: &str) -> Result<WhitelistDecision> {
        self.cleanup_expired();
        if self.is_allowed(command)? {
            Ok(WhitelistDecision::Allowed)
        } else {
            Ok(WhitelistDecision::NeedsApproval)
        }
    }

    pub fn add_exact(&mut self, command: impl Into<String>) {
        let command = command.into();
        if !self.data.exact.contains(&command) {
            self.data.exact.push(command);
        }
    }

    pub fn add_pattern(&mut self, pattern: impl Into<String>) {
        let pattern = pattern.into();
        if !self.data.patterns.contains(&pattern) {
            self.data.patterns.push(pattern);
        }
    }

    pub fn add_temporary(&mut self, command: impl Into<String>, ttl: Duration) {
        let command = command.into();
        let expires_at = Utc::now() + ttl;
        self.data.temporary.push(TemporaryApproval {
            command,
            expires_at,
        });
    }

    pub fn default_path(config_dir: &Path) -> PathBuf {
        config_dir.join("mentalOS").join("whitelist.json")
    }

    pub fn is_allowed(&self, command: &str) -> Result<bool> {
        let command = command.trim();
        if command.is_empty() {
            return Err(MentalOSError::InvalidCommand("Empty command".to_string()));
        }

        if self.data.exact.iter().any(|c| c == command) {
            return Ok(true);
        }

        if self
            .data
            .temporary
            .iter()
            .any(|entry| entry.command == command && entry.expires_at > Utc::now())
        {
            return Ok(true);
        }

        for pattern in &self.data.patterns {
            if pattern_match(pattern, command)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn cleanup_expired(&mut self) {
        let now = Utc::now();
        self.data.temporary.retain(|entry| entry.expires_at > now);
    }
}

fn pattern_match(pattern: &str, command: &str) -> Result<bool> {
    let mut escaped = regex::escape(pattern);
    escaped = escaped.replace("\\*", ".*");
    let regex = Regex::new(&format!("^{escaped}$"))
        .map_err(|err| MentalOSError::Other(format!("Invalid pattern: {err}")))?;
    Ok(regex.is_match(command))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn allows_exact_command() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("whitelist.json");
        let mut manager = WhitelistManager::load(path).unwrap();
        manager.add_exact("ls");
        assert!(manager.is_allowed("ls").unwrap());
        assert!(!manager.is_allowed("pwd").unwrap());
    }

    #[test]
    fn allows_pattern_command() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("whitelist.json");
        let mut manager = WhitelistManager::load(path).unwrap();
        manager.add_pattern("git *");
        assert!(manager.is_allowed("git status").unwrap());
        assert!(!manager.is_allowed("cargo build").unwrap());
    }

    #[test]
    fn temporary_expiry() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("whitelist.json");
        let mut manager = WhitelistManager::load(path).unwrap();
        manager.add_temporary("echo hi", Duration::seconds(1));
        assert!(manager.is_allowed("echo hi").unwrap());
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert!(!manager.is_allowed("echo hi").unwrap());
    }

    #[test]
    fn saves_to_disk() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("whitelist.json");
        let mut manager = WhitelistManager::load(path.clone()).unwrap();
        manager.add_exact("pwd");
        manager.save().unwrap();

        let loaded = WhitelistManager::load(path).unwrap();
        assert!(loaded.is_allowed("pwd").unwrap());
    }
}

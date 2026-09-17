use crate::config::AgentConfig;
use crate::error::{MentalOSError, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct AgentManager {
    agents: HashMap<String, AgentConfig>,
    active_agent: String,
    _config_dir: PathBuf,
}

impl AgentManager {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            agents: HashMap::new(),
            active_agent: "openclaw".to_string(),
            _config_dir: config_dir,
        }
    }

    pub fn load_agents(&mut self, agents: HashMap<String, AgentConfig>) {
        self.agents = agents;

        for (name, config) in &self.agents {
            if config.executable.is_some() {
                let is_default = config
                    .executable
                    .as_ref()
                    .map(|e| e.contains("default"))
                    .unwrap_or(false);
                if is_default || self.active_agent.is_empty() {
                    self.active_agent = name.clone();
                }
            }
        }

        if self.active_agent.is_empty() {
            self.active_agent = "openclaw".to_string();
        }
    }

    pub fn list_agents(&self) -> Vec<String> {
        let mut names: Vec<String> = self.agents.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn get_agent(&self, name: &str) -> Option<&AgentConfig> {
        self.agents.get(name)
    }

    pub fn get_active_agent(&self) -> Option<&AgentConfig> {
        self.agents.get(&self.active_agent)
    }

    pub fn get_active_agent_name(&self) -> &str {
        &self.active_agent
    }

    pub fn switch_agent(&mut self, name: &str) -> Result<String> {
        let name_key = self
            .agents
            .keys()
            .find(|k| k.eq_ignore_ascii_case(name))
            .ok_or_else(|| MentalOSError::Other(format!("Agent '{}' not found", name)))?
            .clone();

        self.active_agent = name_key;

        Ok(format!(
            "Switched to agent: {}",
            self.agents
                .get(&self.active_agent)
                .map(|a| a.name.as_str())
                .unwrap_or(&self.active_agent)
        ))
    }

    pub fn is_agent_available(&self, name: &str) -> bool {
        if let Some(config) = self.agents.get(name) {
            if let Some(ref executable) = config.executable {
                return Command::new("which")
                    .arg(executable)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
            }

            if config.endpoint.is_some()
                || config.provider == "openclaw"
                || config.provider == "ollama"
                || config.provider == "deepseek"
                || config.provider == "zen"
            {
                return true;
            }
        }
        false
    }

    pub fn add_agent(&mut self, name: String, config: AgentConfig) {
        self.agents.insert(name, config);
    }

    pub fn remove_agent(&mut self, name: &str) -> bool {
        if name == self.active_agent {
            return false;
        }
        self.agents.remove(name).is_some()
    }

    pub fn get_agent_status(&self, name: &str) -> AgentStatus {
        let _config = match self.agents.get(name) {
            Some(c) => c,
            None => return AgentStatus::NotFound,
        };

        if !self.is_agent_available(name) {
            return AgentStatus::Unavailable;
        }

        if name == self.active_agent {
            AgentStatus::Active
        } else {
            AgentStatus::Available
        }
    }

    pub fn get_all_statuses(&self) -> HashMap<String, AgentStatus> {
        self.agents
            .keys()
            .map(|name| (name.clone(), self.get_agent_status(name)))
            .collect()
    }

    pub fn is_local_agent(&self, name: &str) -> bool {
        self.agents
            .get(name)
            .map(|c| c.executable.is_some())
            .unwrap_or(false)
    }

    pub fn get_executable_path(&self, name: &str) -> Option<String> {
        self.agents.get(name).and_then(|c| c.executable.clone())
    }

    pub fn get_cli_args(&self, name: &str) -> Option<Vec<String>> {
        self.agents.get(name).and_then(|c| c.arguments.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Active,
    Available,
    Unavailable,
    NotFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_agent(name: &str) -> AgentConfig {
        AgentConfig {
            name: name.to_string(),
            description: None,
            provider: "openclaw".to_string(),
            model: None,
            endpoint: None,
            executable: None,
            arguments: None,
        }
    }

    #[test]
    fn switches_between_agents() {
        let mut manager = AgentManager::new(PathBuf::from("/tmp"));

        let mut agents = HashMap::new();
        agents.insert("openclaw".to_string(), test_agent("OpenClaw"));
        agents.insert("ollama".to_string(), test_agent("Ollama"));

        manager.load_agents(agents);

        assert_eq!(manager.get_active_agent_name(), "openclaw");

        let result = manager.switch_agent("ollama").unwrap();
        assert!(result.contains("Ollama"));
        assert_eq!(manager.get_active_agent_name(), "ollama");
    }

    #[test]
    fn case_insensitive_switch() {
        let mut manager = AgentManager::new(PathBuf::from("/tmp"));

        let mut agents = HashMap::new();
        agents.insert("OpenClaw".to_string(), test_agent("OpenClaw"));

        manager.load_agents(agents);

        let result = manager.switch_agent("openclaw").unwrap();
        assert!(result.contains("OpenClaw"));
    }

    #[test]
    fn lists_all_agents() {
        let mut manager = AgentManager::new(PathBuf::from("/tmp"));

        let mut agents = HashMap::new();
        agents.insert("ollama".to_string(), test_agent("Ollama"));
        agents.insert("openclaw".to_string(), test_agent("OpenClaw"));
        agents.insert("claude".to_string(), test_agent("Claude"));

        manager.load_agents(agents);

        let list = manager.list_agents();

        assert_eq!(list, vec!["claude", "ollama", "openclaw"]);
    }

    #[test]
    fn add_and_get_agent() {
        let mut manager = AgentManager::new(PathBuf::from("/tmp"));
        manager.add_agent("custom".to_string(), test_agent("Custom"));

        let agent = manager.get_agent("custom");
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().name, "Custom");

        assert!(manager.get_agent("nonexistent").is_none());
    }

    #[test]
    fn remove_agent_fails_for_active() {
        let mut manager = AgentManager::new(PathBuf::from("/tmp"));
        let mut agents = HashMap::new();
        agents.insert("active".to_string(), {
            let mut cfg = test_agent("ActiveAgent");
            cfg.executable = Some("default".to_string());
            cfg
        });
        manager.load_agents(agents);

        assert!(!manager.remove_agent("active"));
        assert!(manager.get_agent("active").is_some());
    }

    #[test]
    fn remove_agent_succeeds_for_inactive() {
        let mut manager = AgentManager::new(PathBuf::from("/tmp"));
        let mut agents = HashMap::new();
        agents.insert("primary".to_string(), test_agent("Primary"));
        agents.insert("secondary".to_string(), test_agent("Secondary"));
        manager.load_agents(agents);
        manager.switch_agent("secondary").unwrap();

        assert!(manager.remove_agent("primary"));
        assert!(manager.get_agent("primary").is_none());
    }

    #[test]
    fn get_all_statuses_returns_correct_states() {
        let mut manager = AgentManager::new(PathBuf::from("/tmp"));
        let mut agents = HashMap::new();
        agents.insert("openclaw".to_string(), test_agent("OpenClaw"));
        agents.insert("ollama".to_string(), test_agent("Ollama"));
        manager.load_agents(agents);

        let statuses = manager.get_all_statuses();
        assert_eq!(statuses.get("openclaw"), Some(&AgentStatus::Active));
        assert_eq!(statuses.get("ollama"), Some(&AgentStatus::Available));
    }

    #[test]
    fn is_local_agent_and_executable_path() {
        let mut manager = AgentManager::new(PathBuf::from("/tmp"));
        let mut cfg = test_agent("LocalAgent");
        cfg.executable = Some("/usr/bin/some-tool".to_string());
        manager.add_agent("local".to_string(), cfg);

        assert!(manager.is_local_agent("local"));
        assert_eq!(
            manager.get_executable_path("local"),
            Some("/usr/bin/some-tool".to_string())
        );
        assert!(manager.get_cli_args("local").is_none());

        assert!(!manager.is_local_agent("openclaw"));
        assert!(manager.get_executable_path("openclaw").is_none());
    }

    #[test]
    fn get_agent_status_not_found() {
        let manager = AgentManager::new(PathBuf::from("/tmp"));
        assert_eq!(manager.get_agent_status("ghost"), AgentStatus::NotFound);
    }

    #[test]
    fn get_agent_status_active_and_available() {
        let mut manager = AgentManager::new(PathBuf::from("/tmp"));
        let mut agents = HashMap::new();
        agents.insert("a1".to_string(), test_agent("A1"));
        agents.insert("a2".to_string(), test_agent("A2"));
        manager.load_agents(agents);
        manager.switch_agent("a1").unwrap();

        assert_eq!(manager.get_agent_status("a1"), AgentStatus::Active);
        assert_eq!(manager.get_agent_status("a2"), AgentStatus::Available);
    }
}

//! Ollama AI provider using the /api/chat endpoint.
//!
//! This provider uses Ollama's `/api/chat` endpoint which supports multi-turn
//! conversations with full message history, unlike the legacy `/api/generate`
//! endpoint which only accepts a single prompt string.

use crate::config::Config;
use crate::error::{MentalOSError, Result};
use crate::memory::{Message, Role};
use crate::providers::AiProvider;
use serde::{Deserialize, Serialize};

/// Ollama AI provider with multi-turn conversation support via /api/chat.
pub struct OllamaProvider {
    endpoint: String,
    model: String,
    http: reqwest::Client,
}

/// Request format for Ollama's /api/chat endpoint.
#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
}

/// A single message in the Ollama chat format.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct OllamaChatMessage {
    role: String,
    content: String,
}

/// Response format from Ollama's /api/chat endpoint.
#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaChatMessage,
}

impl OllamaProvider {
    pub fn from_config(config: &Config) -> Result<Self> {
        Ok(Self {
            endpoint: config.ollama.endpoint.clone(),
            model: config.ollama.model.clone(),
            http: reqwest::Client::new(),
        })
    }

    /// Convert internal Message to Ollama chat message format.
    fn convert_message(msg: &Message) -> OllamaChatMessage {
        let role = match msg.role {
            Role::User => "user".to_string(),
            Role::Assistant => "assistant".to_string(),
            Role::System => "system".to_string(),
        };
        OllamaChatMessage {
            role,
            content: msg.content.clone(),
        }
    }
}

impl AiProvider for OllamaProvider {
    fn send_message(&self, message: &str, context: &[Message]) -> Result<String> {
        let url = join_url(&self.endpoint, "/api/chat");

        // Build messages array from context + current message
        let mut messages: Vec<OllamaChatMessage> = context
            .iter()
            .map(Self::convert_message)
            .collect();

        // Add the current user message
        messages.push(OllamaChatMessage {
            role: "user".to_string(),
            content: message.to_string(),
        });

        let payload = OllamaChatRequest {
            model: self.model.clone(),
            messages,
            stream: false,
        };

        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let response = self.http.post(url).json(&payload).send().await?;
            let status = response.status();
            let text = response.text().await?;
            if !status.is_success() {
                return Err(MentalOSError::ProviderUnavailable {
                    provider: "ollama".to_string(),
                    message: format!("Ollama HTTP error {status}: {text}"),
                });
            }
            let parsed: OllamaChatResponse = serde_json::from_str(&text)?;
            Ok(parsed.message.content)
        })
    }

    fn list_agents(&self) -> Vec<String> {
        vec![self.model.clone()]
    }

    fn get_current_provider(&self) -> String {
        "ollama".to_string()
    }

    fn switch_agent(&mut self, name: &str) -> Result<String> {
        // For Ollama, switching "agent" means switching the model
        let old_model = self.model.clone();
        self.model = name.to_string();
        Ok(format!(
            "Switched Ollama model from {} to {}",
            old_model, name
        ))
    }

    fn supports_context(&self) -> bool {
        true // /api/chat supports full multi-turn context
    }

    fn name(&self) -> &str {
        "ollama"
    }

    fn is_healthy(&self) -> bool {
        // Check Ollama connectivity by querying /api/tags
        let url = join_url(&self.endpoint, "/api/tags");
        let rt = match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle,
            Err(_) => return false,
        };
        rt.block_on(async {
            match self.http.get(&url).timeout(std::time::Duration::from_secs(2)).send().await {
                Ok(resp) => resp.status().is_success(),
                Err(_) => false,
            }
        })
    }
}

fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    format!("{base}{path}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AiConfig, OllamaConfig, OpenClawConfig, PathsConfig};
    use crate::providers::AiProvider;

    fn base_config() -> Config {
        Config {
            ai: AiConfig::default(),
            openclaw: OpenClawConfig::default(),
            ollama: OllamaConfig::default(),
            deepseek: crate::config::DeepSeekConfig::default(),
            zen: crate::config::OpenCodeZenConfig::default(),
            paths: PathsConfig::default(),
            agents: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn supports_context_returns_true() {
        let config = base_config();
        let provider = OllamaProvider::from_config(&config).unwrap();
        assert!(provider.supports_context());
    }

    #[test]
    fn name_returns_ollama() {
        let config = base_config();
        let provider = OllamaProvider::from_config(&config).unwrap();
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn get_current_provider_returns_ollama() {
        let config = base_config();
        let provider = OllamaProvider::from_config(&config).unwrap();
        assert_eq!(provider.get_current_provider(), "ollama");
    }

    #[test]
    fn switch_agent_changes_model() {
        let config = base_config();
        let mut provider = OllamaProvider::from_config(&config).unwrap();
        assert_eq!(provider.model, "phi3:mini");
        let result = provider.switch_agent("llama3").unwrap();
        assert!(result.contains("llama3"));
        assert_eq!(provider.model, "llama3");
    }

    #[test]
    fn list_agents_returns_current_model() {
        let config = base_config();
        let provider = OllamaProvider::from_config(&config).unwrap();
        assert_eq!(provider.list_agents(), vec!["phi3:mini"]);
    }
}

//! DeepSeek AI provider using the OpenAI-compatible Chat Completions API.
//!
//! DeepSeek provides a fully OpenAI-compatible API at https://api.deepseek.com
//! with support for multi-turn conversations, streaming, function/tool calling,
//! and a unique thinking mode with reasoning_content.

use crate::config::Config;
use crate::error::{MentalOSError, Result};
use crate::memory::{Message, Role};
use crate::providers::AiProvider;
use log::debug;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

/// DeepSeek AI provider using the OpenAI Chat Completions API.
pub struct DeepSeekProvider {
    api_key: String,
    model: String,
    endpoint: String,
    thinking_mode: bool,
    max_tokens: u32,
    temperature: f32,
    http: reqwest::Client,
}

/// A single message in the OpenAI chat format.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Request format for OpenAI Chat Completions API.
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

/// Response format from OpenAI Chat Completions API.
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

impl DeepSeekProvider {
    pub fn from_config(config: &Config) -> Result<Self> {
        let ds_config = &config.deepseek;
        if ds_config.api_key.trim().is_empty() {
            return Err(MentalOSError::ConfigInvalid(
                "DeepSeek API key is required. Set [deepseek] api_key in config.toml".to_string(),
            ));
        }

        Ok(Self {
            api_key: ds_config.api_key.clone(),
            model: ds_config.model.clone(),
            endpoint: ds_config.endpoint.clone(),
            thinking_mode: ds_config.thinking_mode,
            max_tokens: ds_config.max_tokens,
            temperature: ds_config.temperature,
            http: reqwest::Client::new(),
        })
    }

    /// Convert internal Message to OpenAI chat message format.
    fn convert_message(msg: &Message) -> ChatMessage {
        let role = match msg.role {
            Role::User => "user".to_string(),
            Role::Assistant => "assistant".to_string(),
            Role::System => "system".to_string(),
        };
        ChatMessage {
            role,
            content: msg.content.clone(),
        }
    }
}

impl AiProvider for DeepSeekProvider {
    fn send_message(&self, message: &str, context: &[Message]) -> Result<String> {
        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));

        // Build messages array from context + current message
        let mut messages: Vec<ChatMessage> = context
            .iter()
            .map(Self::convert_message)
            .collect();

        // Add the current user message
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: message.to_string(),
        });

        let payload = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            stream: false,
            max_tokens: Some(self.max_tokens),
            temperature: Some(self.temperature),
        };

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .map_err(|_| MentalOSError::Other("Invalid DeepSeek API key format".into()))?,
        );

        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            debug!("DeepSeek request to {} with model {}", url, self.model);
            let response = self
                .http
                .post(&url)
                .headers(headers)
                .json(&payload)
                .send()
                .await?;

            let status = response.status();
            let text = response.text().await?;

            if status.as_u16() == 429 {
                return Err(MentalOSError::ProviderUnavailable {
                    provider: "deepseek".to_string(),
                    message: "DeepSeek rate limit exceeded. Please retry after a moment.".to_string(),
                });
            }

            if status.as_u16() == 401 {
                return Err(MentalOSError::ProviderUnavailable {
                    provider: "deepseek".to_string(),
                    message: "DeepSeek authentication failed. Check your API key.".to_string(),
                });
            }

            if !status.is_success() {
                return Err(MentalOSError::ProviderUnavailable {
                    provider: "deepseek".to_string(),
                    message: format!("DeepSeek HTTP error {status}: {text}"),
                });
            }

            let parsed: ChatCompletionResponse = serde_json::from_str(&text)?;

            let choice = parsed
                .choices
                .into_iter()
                .next()
                .ok_or_else(|| MentalOSError::Other("DeepSeek returned no choices".to_string()))?;

            let content = choice
                .message
                .content
                .ok_or_else(|| MentalOSError::Other("DeepSeek returned empty content".to_string()))?;

            // If thinking mode is enabled, prepend reasoning content for debugging
            if self.thinking_mode {
                if let Some(reasoning) = choice.message.reasoning_content {
                    debug!("DeepSeek reasoning: {}", reasoning);
                }
            }

            Ok(content)
        })
    }

    fn list_agents(&self) -> Vec<String> {
        vec!["deepseek-v4-flash".to_string(), "deepseek-v4-pro".to_string()]
    }

    fn get_current_provider(&self) -> String {
        "deepseek".to_string()
    }

    fn switch_agent(&mut self, name: &str) -> Result<String> {
        let old_model = self.model.clone();
        self.model = name.to_string();
        Ok(format!(
            "Switched DeepSeek model from {} to {}",
            old_model, name
        ))
    }

    fn supports_context(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "deepseek"
    }

    fn is_healthy(&self) -> bool {
        // Check DeepSeek API connectivity with a lightweight models list request
        let url = format!("{}/models", self.endpoint.trim_end_matches('/'));
        let rt = match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle,
            Err(_) => return false,
        };
        rt.block_on(async {
            match self
                .http
                .get(&url)
                .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
            {
                Ok(resp) => resp.status().is_success() || resp.status().as_u16() == 401,
                // 401 means API is reachable but key is invalid — still "healthy" from network perspective
                Err(_) => false,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AiConfig, DeepSeekConfig, OllamaConfig, OpenClawConfig, PathsConfig,
    };
    use crate::providers::AiProvider;

    fn base_config() -> Config {
        Config {
            ai: AiConfig::default(),
            openclaw: OpenClawConfig::default(),
            ollama: OllamaConfig::default(),
            deepseek: DeepSeekConfig {
                api_key: "sk-test-key-123".to_string(),
                ..DeepSeekConfig::default()
            },
            zen: crate::config::OpenCodeZenConfig::default(),
            paths: PathsConfig::default(),
            agents: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn supports_context_returns_true() {
        let config = base_config();
        let provider = DeepSeekProvider::from_config(&config).unwrap();
        assert!(provider.supports_context());
    }

    #[test]
    fn name_returns_deepseek() {
        let config = base_config();
        let provider = DeepSeekProvider::from_config(&config).unwrap();
        assert_eq!(provider.name(), "deepseek");
    }

    #[test]
    fn get_current_provider_returns_deepseek() {
        let config = base_config();
        let provider = DeepSeekProvider::from_config(&config).unwrap();
        assert_eq!(provider.get_current_provider(), "deepseek");
    }

    #[test]
    fn from_config_requires_api_key() {
        let mut config = base_config();
        config.deepseek.api_key = String::new();
        let result = DeepSeekProvider::from_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn switch_agent_changes_model() {
        let config = base_config();
        let mut provider = DeepSeekProvider::from_config(&config).unwrap();
        let result = provider.switch_agent("deepseek-v4-flash").unwrap();
        assert!(result.contains("deepseek-v4-flash"));
        assert_eq!(provider.model, "deepseek-v4-flash");
    }

    #[test]
    fn list_agents_returns_available_models() {
        let config = base_config();
        let provider = DeepSeekProvider::from_config(&config).unwrap();
        let agents = provider.list_agents();
        assert!(agents.contains(&"deepseek-v4-flash".to_string()));
        assert!(agents.contains(&"deepseek-v4-pro".to_string()));
    }
}

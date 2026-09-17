//! OpenCode Zen AI provider — multi-model gateway.
//!
//! OpenCode Zen is a pay-as-you-go API gateway that aggregates 50+ AI models
//! from OpenAI, Anthropic, Google, DeepSeek, xAI, and others through a single
//! API key. It supports three API formats depending on the model:
//!
//! - **OpenAI Chat Completions** (`/zen/v1/chat/completions`): DeepSeek, Grok, MiniMax, GLM, Kimi, free models
//! - **Anthropic Messages** (`/zen/v1/messages`): Claude, Qwen series
//! - **OpenAI Responses** (`/zen/v1/responses`): GPT 5.x series
//!
//! The default format is Chat Completions, which covers the majority of models.

use crate::config::Config;
use crate::error::{MentalOSError, Result};
use crate::memory::{Message, Role};
use crate::providers::AiProvider;
use log::debug;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

/// OpenCode Zen AI provider — multi-model gateway.
pub struct OpenCodeZenProvider {
    api_key: String,
    model: String,
    endpoint: String,
    max_tokens: u32,
    http: reqwest::Client,
}

/// API format routing based on model name.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ApiFormat {
    /// OpenAI Chat Completions format (default)
    ChatCompletions,
    /// Anthropic Messages format (Claude, Qwen)
    Messages,
    /// OpenAI Responses format (GPT 5.x)
    Responses,
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
}

/// Request format for Anthropic Messages API.
#[derive(Debug, Serialize)]
struct AnthropicMessagesRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AnthropicMessage {
    role: String,
    content: String,
}

/// Response format from Anthropic Messages API.
#[derive(Debug, Deserialize)]
struct AnthropicMessagesResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

impl OpenCodeZenProvider {
    pub fn from_config(config: &Config) -> Result<Self> {
        let zen_config = &config.zen;
        if zen_config.api_key.trim().is_empty() {
            return Err(MentalOSError::ConfigInvalid(
                "OpenCode Zen API key is required. Set [zen] api_key in config.toml".to_string(),
            ));
        }

        Ok(Self {
            api_key: zen_config.api_key.clone(),
            model: zen_config.model.clone(),
            endpoint: zen_config.endpoint.clone(),
            max_tokens: zen_config.max_tokens,
            http: reqwest::Client::new(),
        })
    }

    /// Determine the API format based on the model name.
    fn detect_api_format(model: &str) -> ApiFormat {
        let lower = model.to_lowercase();
        if lower.starts_with("gpt-5") || lower.starts_with("gpt5") {
            ApiFormat::Responses
        } else if lower.starts_with("claude-") || lower.starts_with("qwen") {
            ApiFormat::Messages
        } else {
            ApiFormat::ChatCompletions
        }
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

    /// Convert internal Message to Anthropic message format.
    /// Anthropic uses "user" and "assistant" roles only; system is a separate field.
    fn convert_anthropic_message(msg: &Message) -> Option<AnthropicMessage> {
        match msg.role {
            Role::System => None, // System messages are handled separately
            Role::User => Some(AnthropicMessage {
                role: "user".to_string(),
                content: msg.content.clone(),
            }),
            Role::Assistant => Some(AnthropicMessage {
                role: "assistant".to_string(),
                content: msg.content.clone(),
            }),
        }
    }

    /// Send via OpenAI Chat Completions format.
    fn send_chat_completions(&self, message: &str, context: &[Message]) -> Result<String> {
        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));

        let mut messages: Vec<ChatMessage> = context
            .iter()
            .map(Self::convert_message)
            .collect();

        messages.push(ChatMessage {
            role: "user".to_string(),
            content: message.to_string(),
        });

        let payload = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            stream: false,
            max_tokens: Some(self.max_tokens),
        };

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .map_err(|_| MentalOSError::Other("Invalid Zen API key format".into()))?,
        );

        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            debug!("Zen Chat Completions request to {} with model {}", url, self.model);
            let response = self
                .http
                .post(&url)
                .headers(headers)
                .json(&payload)
                .send()
                .await?;

            let status = response.status();
            let text = response.text().await?;

            if !status.is_success() {
                return Err(MentalOSError::ProviderUnavailable {
                    provider: "zen".to_string(),
                    message: format!("Zen Chat Completions error {status}: {text}"),
                });
            }

            let parsed: ChatCompletionResponse = serde_json::from_str(&text)?;
            let choice = parsed
                .choices
                .into_iter()
                .next()
                .ok_or_else(|| MentalOSError::Other("Zen returned no choices".to_string()))?;

            choice.message.content.ok_or_else(|| {
                MentalOSError::Other("Zen returned empty content".to_string())
            })
        })
    }

    /// Send via Anthropic Messages format.
    fn send_anthropic_messages(&self, message: &str, context: &[Message]) -> Result<String> {
        let url = format!("{}/messages", self.endpoint.trim_end_matches('/'));

        // Extract system messages from context
        let system_prompt: Option<String> = context
            .iter()
            .filter(|m| m.role == Role::System)
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n")
            .into();

        // Build non-system messages
        let mut messages: Vec<AnthropicMessage> = context
            .iter()
            .filter_map(Self::convert_anthropic_message)
            .collect();

        messages.push(AnthropicMessage {
            role: "user".to_string(),
            content: message.to_string(),
        });

        let payload = AnthropicMessagesRequest {
            model: self.model.clone(),
            messages,
            max_tokens: self.max_tokens,
            system: system_prompt.filter(|s| !s.is_empty()),
        };

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        headers.insert("x-api-key", HeaderValue::from_str(&self.api_key).map_err(|_| {
            MentalOSError::Other("Invalid Zen API key format".into())
        })?);

        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            debug!("Zen Anthropic Messages request to {} with model {}", url, self.model);
            let response = self
                .http
                .post(&url)
                .headers(headers)
                .json(&payload)
                .send()
                .await?;

            let status = response.status();
            let text = response.text().await?;

            if !status.is_success() {
                return Err(MentalOSError::ProviderUnavailable {
                    provider: "zen".to_string(),
                    message: format!("Zen Anthropic Messages error {status}: {text}"),
                });
            }

            let parsed: AnthropicMessagesResponse = serde_json::from_str(&text)?;
            let content = parsed
                .content
                .into_iter()
                .filter(|b| b.block_type == "text")
                .filter_map(|b| b.text)
                .collect::<Vec<_>>()
                .join("\n");

            if content.is_empty() {
                return Err(MentalOSError::Other("Zen returned empty content".to_string()));
            }

            Ok(content)
        })
    }
}

impl AiProvider for OpenCodeZenProvider {
    fn send_message(&self, message: &str, context: &[Message]) -> Result<String> {
        let format = Self::detect_api_format(&self.model);

        match format {
            ApiFormat::ChatCompletions | ApiFormat::Responses => {
                // Responses format uses the same request/response structure as Chat Completions
                // but with a different endpoint path. For simplicity, we use Chat Completions
                // for both, as the gateway handles the routing.
                self.send_chat_completions(message, context)
            }
            ApiFormat::Messages => self.send_anthropic_messages(message, context),
        }
    }

    fn list_agents(&self) -> Vec<String> {
        vec![
            "deepseek-v4-pro".to_string(),
            "deepseek-v4-flash".to_string(),
            "deepseek-v4-flash-free".to_string(),
            "claude-sonnet-5".to_string(),
            "claude-haiku-4.5".to_string(),
            "gpt-5.4-mini".to_string(),
            "big-pickle".to_string(),
        ]
    }

    fn get_current_provider(&self) -> String {
        "zen".to_string()
    }

    fn switch_agent(&mut self, name: &str) -> Result<String> {
        let old_model = self.model.clone();
        self.model = name.to_string();
        Ok(format!(
            "Switched Zen model from {} to {}",
            old_model, name
        ))
    }

    fn supports_context(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "zen"
    }

    fn is_healthy(&self) -> bool {
        // Check OpenCode Zen API connectivity with a lightweight models list request
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
        AiConfig, OllamaConfig, OpenClawConfig, OpenCodeZenConfig, PathsConfig,
    };
    use crate::providers::AiProvider;

    fn base_config() -> Config {
        Config {
            ai: AiConfig::default(),
            openclaw: OpenClawConfig::default(),
            ollama: OllamaConfig::default(),
            deepseek: crate::config::DeepSeekConfig::default(),
            zen: OpenCodeZenConfig {
                api_key: "zen-test-key-123".to_string(),
                ..OpenCodeZenConfig::default()
            },
            paths: PathsConfig::default(),
            agents: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn supports_context_returns_true() {
        let config = base_config();
        let provider = OpenCodeZenProvider::from_config(&config).unwrap();
        assert!(provider.supports_context());
    }

    #[test]
    fn name_returns_zen() {
        let config = base_config();
        let provider = OpenCodeZenProvider::from_config(&config).unwrap();
        assert_eq!(provider.name(), "zen");
    }

    #[test]
    fn from_config_requires_api_key() {
        let mut config = base_config();
        config.zen.api_key = String::new();
        let result = OpenCodeZenProvider::from_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn detect_api_format_chat_completions() {
        assert_eq!(OpenCodeZenProvider::detect_api_format("deepseek-v4-pro"), ApiFormat::ChatCompletions);
        assert_eq!(OpenCodeZenProvider::detect_api_format("grok-4.5"), ApiFormat::ChatCompletions);
        assert_eq!(OpenCodeZenProvider::detect_api_format("big-pickle"), ApiFormat::ChatCompletions);
    }

    #[test]
    fn detect_api_format_messages() {
        assert_eq!(OpenCodeZenProvider::detect_api_format("claude-sonnet-5"), ApiFormat::Messages);
        assert_eq!(OpenCodeZenProvider::detect_api_format("qwen3.7-max"), ApiFormat::Messages);
    }

    #[test]
    fn detect_api_format_responses() {
        assert_eq!(OpenCodeZenProvider::detect_api_format("gpt-5.4-mini"), ApiFormat::Responses);
        assert_eq!(OpenCodeZenProvider::detect_api_format("gpt-5.6-sol"), ApiFormat::Responses);
    }

    #[test]
    fn switch_agent_changes_model() {
        let config = base_config();
        let mut provider = OpenCodeZenProvider::from_config(&config).unwrap();
        let result = provider.switch_agent("claude-sonnet-5").unwrap();
        assert!(result.contains("claude-sonnet-5"));
        assert_eq!(provider.model, "claude-sonnet-5");
    }
}

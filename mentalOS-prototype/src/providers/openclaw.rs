//! OpenClaw AI provider (CLI and HTTP transports).
//!
//! Supports two transport modes:
//! - **CLI**: Spawns the `openclaw` binary as a subprocess
//! - **HTTP**: Connects to the OpenClaw gateway via REST API
//!
//! When using HTTP transport, the full conversation context is sent.
//! When using CLI transport, the message is passed via `--message` flag
//! and context is piped via stdin (if `--context-stdin` is supported).

use crate::config::AgentConfig;
use crate::config::Config;
use crate::error::{MentalOSError, Result};
use crate::memory::Message;
use crate::providers::AiProvider;
use log::debug;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::net::ToSocketAddrs;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    Cli,
    Http,
}

/// OpenClaw AI provider supporting CLI and HTTP transports with optional Ollama fallback.
pub struct OpenClawProvider {
    transport: Transport,
    endpoint: String,
    request_path: String,
    token: Option<String>,
    cli_path: String,
    cli_args: Vec<String>,
    auto_start: bool,
    ollama_endpoint: String,
    ollama_model: String,
    fallback_to_ollama: bool,
    provider: String,
    http: reqwest::Client,
    agents: HashMap<String, AgentConfig>,
}

#[derive(Debug, Serialize)]
struct OpenClawRequest<'a> {
    message: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    context: Vec<Message>,
}

#[derive(Debug, Deserialize)]
struct OpenClawResponse {
    message: Option<String>,
    response: Option<String>,
    output: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaFallbackRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaFallbackResponse {
    response: String,
}

impl OpenClawProvider {
    pub fn from_config(config: &Config) -> Result<Self> {
        let transport = match config.openclaw.transport.as_str() {
            "http" => Transport::Http,
            _ => Transport::Cli,
        };
        let token = if config.openclaw.token.trim().is_empty() {
            None
        } else {
            Some(config.openclaw.token.trim().to_string())
        };

        Ok(Self {
            transport,
            endpoint: config.openclaw.endpoint.clone(),
            request_path: config.openclaw.request_path.clone(),
            token,
            cli_path: config.openclaw.cli_path.clone(),
            cli_args: config.openclaw.cli_args.clone(),
            auto_start: config.openclaw.auto_start,
            ollama_endpoint: config.ollama.endpoint.clone(),
            ollama_model: config.ollama.model.clone(),
            fallback_to_ollama: config.ai.fallback_to_ollama,
            provider: config.ai.provider.clone(),
            http: reqwest::Client::new(),
            agents: config.agents.clone(),
        })
    }

    fn send_cli(&self, message: &str, context: &[Message]) -> Result<String> {
        let mut command = Command::new(&self.cli_path);
        if !self.cli_args.is_empty() {
            command.args(&self.cli_args);
        } else {
            command.args(["agent"]);
        }
        command.arg("--message").arg(message);

        // Pass context via stdin if available, enabling multi-turn conversations
        if !context.is_empty() {
            command.arg("--context-stdin");
            command.stdin(Stdio::piped());
        }

        let mut child = command.spawn().map_err(MentalOSError::Io)?;

        // Write context JSON to stdin if we have context
        if !context.is_empty() {
            if let Some(mut stdin) = child.stdin.take() {
                let context_json = serde_json::to_string(context)
                    .map_err(MentalOSError::Json)?;
                let _ = stdin.write_all(context_json.as_bytes());
                // stdin is dropped here, closing the pipe
            }
        }

        let output = child.wait_with_output().map_err(MentalOSError::Io)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(MentalOSError::ProviderUnavailable {
                provider: "openclaw".to_string(),
                message: format!("openclaw CLI failed: {stderr}"),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn send_http(&self, message: &str, context: &[Message]) -> Result<String> {
        match self.send_http_once(message, context).await {
            Ok(value) => Ok(value),
            Err(err) => {
                if self.auto_start {
                    debug!("OpenClaw HTTP failed; attempting auto-start.");
                    let _ = self.start_gateway();
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    return self.send_http_once(message, context).await;
                }
                Err(err)
            }
        }
    }

    async fn send_http_once(&self, message: &str, context: &[Message]) -> Result<String> {
        let url = join_url(&self.endpoint, &self.request_path);
        let payload = OpenClawRequest {
            message,
            context: context.to_vec(),
        };

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(token) = &self.token {
            let value = format!("Bearer {token}");
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&value)
                    .map_err(|_| MentalOSError::Other("Invalid auth token".into()))?,
            );
        }

        let response = self
            .http
            .post(url)
            .headers(headers)
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(MentalOSError::ProviderUnavailable {
                provider: "openclaw".to_string(),
                message: format!("OpenClaw HTTP error {status}: {text}"),
            });
        }

        // Three-tier response parsing
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
            && value.get("commands").is_some()
        {
            return Ok(text);
        }

        if let Ok(parsed) = serde_json::from_str::<OpenClawResponse>(&text)
            && let Some(value) = parsed
                .message
                .or(parsed.response)
                .or(parsed.output)
                .or(parsed.content)
        {
            return Ok(value);
        }

        Ok(text)
    }

    async fn send_ollama_fallback(&self, message: &str) -> Result<String> {
        let url = join_url(&self.ollama_endpoint, "/api/generate");
        let payload = OllamaFallbackRequest {
            model: &self.ollama_model,
            prompt: message,
            stream: false,
        };

        let response = self.http.post(url).json(&payload).send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(MentalOSError::ProviderUnavailable {
                provider: "ollama".to_string(),
                message: format!("Ollama fallback HTTP error {status}: {text}"),
            });
        }
        let parsed: OllamaFallbackResponse = serde_json::from_str(&text)?;
        Ok(parsed.response)
    }

    fn start_gateway(&self) -> Result<()> {
        let url = reqwest::Url::parse(&self.endpoint).map_err(|err| {
            MentalOSError::ProviderUnavailable {
                provider: "openclaw".to_string(),
                message: format!("Invalid endpoint: {err}"),
            }
        })?;
        let port = url.port().unwrap_or(18789).to_string();

        let mut command = Command::new(&self.cli_path);
        command
            .args(["gateway", "--port", &port, "--verbose"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| MentalOSError::ProviderUnavailable {
                provider: "openclaw".to_string(),
                message: format!("Failed to start gateway: {err}"),
            })?;
        Ok(())
    }
}

impl AiProvider for OpenClawProvider {
    fn send_message(&self, message: &str, context: &[Message]) -> Result<String> {
        // Note: This is a synchronous wrapper. For async operation, the router
        // should use tokio::spawn_blocking or a similar mechanism.
        // The actual implementation uses the runtime's block_on for CLI mode.
        match self.provider.as_str() {
            "ollama" => {
                // Direct Ollama mode (shouldn't normally happen for OpenClawProvider,
                // but handles agent switching to ollama)
                let rt = tokio::runtime::Handle::current();
                rt.block_on(self.send_ollama_fallback(message))
            }
            _ => {
                let response = match self.transport {
                    Transport::Cli => self.send_cli(message, context),
                    Transport::Http => {
                        let rt = tokio::runtime::Handle::current();
                        rt.block_on(self.send_http(message, context))
                    }
                };

                if response.is_ok() {
                    return response;
                }

                if self.fallback_to_ollama {
                    debug!("OpenClaw failed; falling back to Ollama.");
                    let rt = tokio::runtime::Handle::current();
                    return rt.block_on(self.send_ollama_fallback(message));
                }

                response
            }
        }
    }

    fn list_agents(&self) -> Vec<String> {
        let mut names: Vec<String> = self.agents.keys().cloned().collect();
        names.sort();
        names
    }

    fn get_current_provider(&self) -> String {
        self.provider.clone()
    }

    fn switch_agent(&mut self, name: &str) -> Result<String> {
        let name_key = self
            .agents
            .keys()
            .find(|k| k.eq_ignore_ascii_case(name))
            .ok_or_else(|| MentalOSError::Other(format!("Agent '{}' not found", name)))?
            .clone();

        let agent = self
            .agents
            .get(&name_key)
            .cloned()
            .ok_or_else(|| MentalOSError::Other(format!("Agent '{}' not found", name)))?;

        self.provider = agent.provider.clone();

        if let Some(model) = agent.model {
            self.ollama_model = model;
        }

        if let Some(endpoint) = agent.endpoint {
            if self.provider == "ollama" {
                self.ollama_endpoint = endpoint;
            } else {
                self.endpoint = endpoint;
            }
        }

        Ok(format!("Switched to agent: {}", agent.name))
    }

    fn supports_context(&self) -> bool {
        // HTTP transport always supports context; CLI now supports it via stdin
        true
    }

    fn name(&self) -> &str {
        "openclaw"
    }

    fn is_healthy(&self) -> bool {
        match self.transport {
            Transport::Http => {
                // Check TCP connectivity to the gateway
                let addr = parse_socket_addr(&self.endpoint);
                match addr {
                    Ok(addr) => {
                        std::net::TcpStream::connect_timeout(
                            &addr,
                            std::time::Duration::from_millis(500),
                        )
                        .is_ok()
                    }
                    Err(_) => false,
                }
            }
            Transport::Cli => {
                // CLI transport: check if the binary is in PATH
                which_check(&self.cli_path)
            }
        }
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

/// Parse a URL string into a socket address for TCP health checks.
fn parse_socket_addr(endpoint: &str) -> std::result::Result<std::net::SocketAddr, String> {
    let url = reqwest::Url::parse(endpoint)
        .map_err(|e| format!("Invalid endpoint: {e}"))?;
    let host = url.host_str().ok_or("No host in endpoint")?;
    let port = url.port_or_known_default().unwrap_or(18789);
    (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed: {e}"))?
        .next()
        .ok_or_else(|| "Could not resolve endpoint".to_string())
}

/// Check if a binary is available in PATH.
fn which_check(binary: &str) -> bool {
    std::process::Command::new("which")
        .arg(binary)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AiConfig, OllamaConfig, OpenClawConfig, PathsConfig};
    use crate::providers::AiProvider;

    fn base_config() -> Config {
        Config {
            ai: AiConfig::default(),
            openclaw: OpenClawConfig {
                transport: "http".to_string(),
                request_path: "/agent".to_string(),
                ..OpenClawConfig::default()
            },
            ollama: OllamaConfig::default(),
            deepseek: crate::config::DeepSeekConfig::default(),
            zen: crate::config::OpenCodeZenConfig::default(),
            paths: PathsConfig::default(),
            agents: std::collections::HashMap::new(),
        }
    }

    fn socket_tests_enabled() -> bool {
        std::env::var("MENTALOS_ENABLE_SOCKET_TESTS")
            .map(|v| v == "1")
            .unwrap_or(false)
    }

    #[test]
    fn supports_context_returns_true() {
        let config = base_config();
        let provider = OpenClawProvider::from_config(&config).unwrap();
        assert!(provider.supports_context());
    }

    #[test]
    fn name_returns_openclaw() {
        let config = base_config();
        let provider = OpenClawProvider::from_config(&config).unwrap();
        assert_eq!(provider.name(), "openclaw");
    }

    #[test]
    fn get_current_provider_returns_configured_provider() {
        let config = base_config();
        let provider = OpenClawProvider::from_config(&config).unwrap();
        assert_eq!(provider.get_current_provider(), "openclaw");
    }

    #[test]
    fn list_agents_returns_sorted_names() {
        let mut config = base_config();
        config.agents.insert(
            "beta".to_string(),
            crate::config::AgentConfig {
                name: "Beta".to_string(),
                description: None,
                provider: "openclaw".to_string(),
                model: None,
                endpoint: None,
                executable: None,
                arguments: None,
            },
        );
        config.agents.insert(
            "alpha".to_string(),
            crate::config::AgentConfig {
                name: "Alpha".to_string(),
                description: None,
                provider: "openclaw".to_string(),
                model: None,
                endpoint: None,
                executable: None,
                arguments: None,
            },
        );
        let provider = OpenClawProvider::from_config(&config).unwrap();
        assert_eq!(provider.list_agents(), vec!["alpha", "beta"]);
    }
}

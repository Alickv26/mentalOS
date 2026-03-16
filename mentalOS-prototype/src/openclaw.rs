use crate::config::Config;
use crate::error::{MentalOSError, Result};
use crate::memory::Message;
use log::debug;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    Cli,
    Http,
}

use crate::config::AgentConfig;
/// Client for OpenClaw (CLI/HTTP) with optional Ollama fallback.
///
/// # Examples
/// ```no_run
/// let config = mentalOS::Config::load().unwrap();
/// let client = mentalOS::openclaw::OpenClawClient::from_config(&config);
/// ```
use std::collections::HashMap;

pub struct OpenClawClient {
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
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaResponse {
    response: String,
}

impl OpenClawClient {
    pub fn from_config(config: &Config) -> Self {
        let transport = match config.openclaw.transport.as_str() {
            "http" => Transport::Http,
            _ => Transport::Cli,
        };
        let token = if config.openclaw.token.trim().is_empty() {
            None
        } else {
            Some(config.openclaw.token.trim().to_string())
        };

        Self {
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
        }
    }

    pub fn list_agents(&self) -> Vec<String> {
        let mut names: Vec<String> = self.agents.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn get_current_provider(&self) -> &str {
        &self.provider
    }

    pub fn switch_agent(&mut self, name: &str) -> Result<String> {
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
            self.ollama_model = model; // Assuming model is for ollama or openclaw
        }

        if let Some(endpoint) = agent.endpoint {
            if self.provider == "ollama" {
                self.ollama_endpoint = endpoint;
            } else {
                self.endpoint = endpoint;
            }
        }

        // Also update transport/cli_path if provided, but struct fields are simple here
        // Ideally we map AgentConfig fields back to OpenClawClient fields

        Ok(format!("Switched to agent: {}", agent.name))
    }

    pub async fn send_message(&self, message: &str, context: &[Message]) -> Result<String> {
        match self.provider.as_str() {
            "ollama" => self.send_ollama(message).await,
            _ => {
                let response = match self.transport {
                    Transport::Cli => self.send_cli(message),
                    Transport::Http => self.send_http(message, context).await,
                };

                if response.is_ok() {
                    return response;
                }

                if self.fallback_to_ollama {
                    debug!("OpenClaw failed; falling back to Ollama.");
                    return self.send_ollama(message).await;
                }

                response
            }
        }
    }

    fn send_cli(&self, message: &str) -> Result<String> {
        let mut command = Command::new(&self.cli_path);
        if !self.cli_args.is_empty() {
            command.args(&self.cli_args);
        } else {
            command.args(["agent"]);
        }
        command.arg("--message").arg(message);

        let output = command.output().map_err(MentalOSError::Io)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(MentalOSError::OpenClawUnavailable(format!(
                "openclaw CLI failed: {stderr}"
            )));
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
            return Err(MentalOSError::OpenClawUnavailable(format!(
                "OpenClaw HTTP error {status}: {text}"
            )));
        }

        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if value.get("commands").is_some() {
                return Ok(text);
            }
        }

        if let Ok(parsed) = serde_json::from_str::<OpenClawResponse>(&text) {
            if let Some(value) = parsed
                .message
                .or(parsed.response)
                .or(parsed.output)
                .or(parsed.content)
            {
                return Ok(value);
            }
        }

        Ok(text)
    }

    async fn send_ollama(&self, message: &str) -> Result<String> {
        let url = join_url(&self.ollama_endpoint, "/api/generate");
        let payload = OllamaRequest {
            model: &self.ollama_model,
            prompt: message,
            stream: false,
        };

        let response = self.http.post(url).json(&payload).send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(MentalOSError::Other(format!(
                "Ollama HTTP error {status}: {text}"
            )));
        }
        let parsed: OllamaResponse = serde_json::from_str(&text)?;
        Ok(parsed.response)
    }

    fn start_gateway(&self) -> Result<()> {
        let url = reqwest::Url::parse(&self.endpoint).map_err(|err| {
            MentalOSError::OpenClawUnavailable(format!("Invalid endpoint: {err}"))
        })?;
        let port = url.port().unwrap_or(18789).to_string();

        let mut command = Command::new(&self.cli_path);
        command
            .args(["gateway", "--port", &port, "--verbose"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| {
                MentalOSError::OpenClawUnavailable(format!("Failed to start gateway: {err}"))
            })?;
        Ok(())
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
    use httpmock::Method::POST;
    use httpmock::MockServer;

    fn base_config() -> Config {
        Config {
            ai: AiConfig::default(),
            openclaw: OpenClawConfig {
                transport: "http".to_string(),
                request_path: "/agent".to_string(),
                ..OpenClawConfig::default()
            },
            ollama: OllamaConfig::default(),
            paths: PathsConfig::default(),
            agents: std::collections::HashMap::new(),
        }
    }

    fn socket_tests_enabled() -> bool {
        std::env::var("MENTALOS_ENABLE_SOCKET_TESTS")
            .map(|v| v == "1")
            .unwrap_or(false)
    }

    #[tokio::test]
    async fn http_returns_text() {
        if !socket_tests_enabled() {
            return;
        }
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/agent");
                then.status(200).body("hello");
            })
            .await;

        let mut config = base_config();
        config.openclaw.endpoint = server.base_url();
        config.ai.fallback_to_ollama = false;
        let client = OpenClawClient::from_config(&config);

        let response = client.send_message("hi", &[]).await.unwrap();
        assert_eq!(response, "hello");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn ollama_fallback() {
        if !socket_tests_enabled() {
            return;
        }
        let server = MockServer::start_async().await;
        let ollama = server
            .mock_async(|when, then| {
                when.method(POST).path("/api/generate");
                then.status(200).json_body_obj(&OllamaResponse {
                    response: "ollama reply".to_string(),
                });
            })
            .await;

        let mut config = base_config();
        config.openclaw.endpoint = "http://127.0.0.1:0".to_string();
        config.ollama.endpoint = server.base_url();
        config.ai.fallback_to_ollama = true;

        let client = OpenClawClient::from_config(&config);
        let response = client.send_message("hi", &[]).await.unwrap();
        assert_eq!(response, "ollama reply");
        ollama.assert_async().await;
    }
}

//! AI provider abstraction layer.
//!
//! This module defines the [`AiProvider`] trait that all AI providers must implement,
//! and provides a factory function [`create_provider`] to instantiate the correct
//! provider based on the application configuration.
//!
//! # Provider Architecture
//!
//! All providers implement the `AiProvider` trait and are created via the
//! `create_provider()` factory function. The router uses `Box<dyn AiProvider>`
//! to remain provider-agnostic.
//!
//! # Resilience
//!
//! The `circuit_breaker` module provides a thread-safe circuit breaker pattern
//! that prevents repeated requests to a failing provider. The `retry_with_backoff`
//! function handles transient failures with exponential backoff.

pub mod circuit_breaker;
pub mod deepseek;
pub mod ollama;
pub mod openclaw;
pub mod openclaw_launcher;
pub mod opencode_zen;

use crate::config::Config;
use crate::error::{MentalOSError, Result};
use crate::memory::Message;

/// Trait that all AI providers must implement.
///
/// This abstraction decouples the command router from any specific AI provider,
/// allowing new providers (DeepSeek, OpenCode Zen, etc.) to be added without
/// modifying the router or other core modules.
pub trait AiProvider: Send + Sync {
    /// Send a message to the AI provider with optional conversation context.
    ///
    /// The `context` parameter contains recent conversation messages and the
    /// smart context snapshot (CWD, open files, recent commands, inferred goals).
    /// Providers that support context should use it; others may ignore it.
    fn send_message(&self, message: &str, context: &[Message]) -> Result<String>;

    /// List the names of all available agents/models for this provider.
    fn list_agents(&self) -> Vec<String>;

    /// Get the name of the currently active provider.
    fn get_current_provider(&self) -> String;

    /// Switch to a different agent/model within this provider.
    fn switch_agent(&mut self, name: &str) -> Result<String>;

    /// Whether this provider supports passing conversation context.
    ///
    /// Returns `true` if the provider can accept and use the `context`
    /// parameter in `send_message`. Returns `false` if context is
    /// silently ignored (e.g., CLI-only providers).
    fn supports_context(&self) -> bool;

    /// Human-readable name of this provider.
    fn name(&self) -> &str;

    /// Check whether the provider is currently reachable.
    ///
    /// Returns `true` if the provider's backend is healthy and available,
    /// `false` if it is unreachable or in a failure state.
    /// Default implementation returns `true`.
    fn is_healthy(&self) -> bool {
        true
    }
}

/// Create the appropriate AI provider based on the application configuration.
///
/// Reads `config.ai.provider` and constructs the corresponding provider.
/// Returns a boxed trait object that can be used by the command router.
pub fn create_provider(config: &Config) -> Result<Box<dyn AiProvider>> {
    match config.ai.provider.as_str() {
        "openclaw" => Ok(Box::new(openclaw::OpenClawProvider::from_config(config)?)),
        "ollama" => Ok(Box::new(ollama::OllamaProvider::from_config(config)?)),
        "deepseek" => Ok(Box::new(deepseek::DeepSeekProvider::from_config(config)?)),
        "zen" => Ok(Box::new(opencode_zen::OpenCodeZenProvider::from_config(config)?)),
        other => Err(MentalOSError::ConfigInvalid(format!(
            "Unknown AI provider: '{}'. Supported providers: openclaw, ollama, deepseek, zen",
            other
        ))),
    }
}

/// Returns the list of all supported provider names.
pub fn supported_providers() -> &'static [&'static str] {
    &["openclaw", "ollama", "deepseek", "zen"]
}

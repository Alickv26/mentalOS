//! Live integration tests for the OpenCode Zen multi-model gateway.
//!
//! These tests hit the real OpenCode Zen API at https://opencode.ai/zen/v1
//! and are skipped by default. To run them:
//!
//! ```sh
//! export MENTALOS_LIVE_ZEN=1
//! # Ensure [zen] api_key is set in ~/.config/mentalOS/config.toml
//! cargo test --no-default-features --test live_zen -- --nocapture
//! ```
//!
//! Costs: each test sends 1 small chat completion request. Zen is
//! pay-as-you-go; costs vary by model but are typically < $0.01 per run
//! for the small prompts used here.

use mental_os::config::{Config, OpenCodeZenConfig};
use mental_os::memory::{Message, Role};
use mental_os::providers::opencode_zen::OpenCodeZenProvider;
use mental_os::providers::AiProvider;

fn skip_if_not_enabled() {
    if std::env::var("MENTALOS_LIVE_ZEN").unwrap_or_default() != "1" {
        eprintln!("Skipping live_zen test; set MENTALOS_LIVE_ZEN=1 to run.");
        return;
    }
}

fn load_provider() -> OpenCodeZenProvider {
    let config = Config::load().expect("config missing");
    OpenCodeZenProvider::from_config(&config).expect("create zen provider")
}

#[tokio::test]
async fn live_zen_chat_completions_default_model() {
    skip_if_not_enabled();
    if std::env::var("MENTALOS_LIVE_ZEN").unwrap_or_default() != "1" {
        return;
    }

    let provider = load_provider();
    let response = provider
        .send_message("Say hello in one sentence.", &[])
        .expect("zen request failed");

    assert!(!response.trim().is_empty(), "response should not be empty");
    eprintln!("Zen (default model) response: {}", response);
}

/// Test the Anthropic Messages API format by switching to a Claude model.
/// This validates the `detect_api_format` routing logic end-to-end.
#[tokio::test]
async fn live_zen_anthropic_messages_claude() {
    skip_if_not_enabled();
    if std::env::var("MENTALOS_LIVE_ZEN").unwrap_or_default() != "1" {
        return;
    }

    let mut provider = load_provider();
    let _ = provider.switch_agent("claude-sonnet-5");

    let response = provider
        .send_message("Say hello in one sentence.", &[])
        .expect("zen claude request failed");

    assert!(!response.trim().is_empty(), "claude response should not be empty");
    eprintln!("Zen (claude-sonnet-5) response: {}", response);
}

/// Test that multi-turn context is passed through to the Zen gateway.
#[tokio::test]
async fn live_zen_multi_turn_context() {
    skip_if_not_enabled();
    if std::env::var("MENTALOS_LIVE_ZEN").unwrap_or_default() != "1" {
        return;
    }

    let provider = load_provider();

    let now = chrono::Utc::now();
    let context = vec![
        Message {
            role: Role::User,
            content: "My favorite color is blue. Remember this.".to_string(),
            timestamp: now,
            actions: Vec::new(),
        },
        Message {
            role: Role::Assistant,
            content: "Understood — your favorite color is blue.".to_string(),
            timestamp: now,
            actions: Vec::new(),
        },
    ];

    let response = provider
        .send_message("What is my favorite color? Reply with just the color name.", &context)
        .expect("zen multi-turn request failed");

    let trimmed = response.trim().to_lowercase();
    assert!(
        trimmed.contains("blue"),
        "Zen should use the context (favorite color blue). Got: {}",
        trimmed
    );
    eprintln!("Zen multi-turn response: {}", trimmed);
}

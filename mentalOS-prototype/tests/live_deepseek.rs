//! Live integration tests for the DeepSeek cloud provider.
//!
//! These tests hit the real DeepSeek API at https://api.deepseek.com and
//! are skipped by default. To run them:
//!
//! ```sh
//! export MENTALOS_LIVE_DEEPSEEK=1
//! # Ensure [deepseek] api_key is set in ~/.config/mentalOS/config.toml
//! cargo test --no-default-features --test live_deepseek -- --nocapture
//! ```
//!
//! Costs: each test sends 1-2 small chat completion requests. At
//! deepseek-v4-flash pricing this is well under $0.01 per run.

use mental_os::providers::deepseek::DeepSeekProvider;
use mental_os::providers::AiProvider;
use mental_os::memory::{Message, Role};

fn skip_if_not_enabled() {
    if std::env::var("MENTALOS_LIVE_DEEPSEEK").unwrap_or_default() != "1" {
        eprintln!("Skipping live_deepseek test; set MENTALOS_LIVE_DEEPSEEK=1 to run.");
        return;
    }
}

fn load_provider() -> DeepSeekProvider {
    let config = mental_os::Config::load().expect("config missing");
    DeepSeekProvider::from_config(&config).expect("create deepseek provider")
}

#[tokio::test]
async fn live_deepseek_single_turn() {
    skip_if_not_enabled();
    if std::env::var("MENTALOS_LIVE_DEEPSEEK").unwrap_or_default() != "1" {
        return;
    }

    let provider = load_provider();
    let response = provider
        .send_message("Say hello in one sentence.", &[])
        .expect("deepseek request failed");

    assert!(!response.trim().is_empty(), "response should not be empty");
    eprintln!("DeepSeek response: {}", response);
}

#[tokio::test]
async fn live_deepseek_multi_turn_context() {
    skip_if_not_enabled();
    if std::env::var("MENTALOS_LIVE_DEEPSEEK").unwrap_or_default() != "1" {
        return;
    }

    let provider = load_provider();

    // Build a 2-message context: a user asks for a number, the assistant
    // gives one. Then we ask a follow-up that requires the context.
    let now = chrono::Utc::now();
    let context = vec![
        Message {
            role: Role::User,
            content: "My favorite number is 42. Remember this for the rest of the conversation.".to_string(),
            timestamp: now,
            actions: Vec::new(),
        },
        Message {
            role: Role::Assistant,
            content: "Got it — your favorite number is 42. I'll use that for the rest of this conversation.".to_string(),
            timestamp: now,
            actions: Vec::new(),
        },
    ];

    let response = provider
        .send_message("What is my favorite number? Reply with just the number.", &context)
        .expect("deepseek multi-turn request failed");

    let trimmed = response.trim();
    assert!(
        trimmed.contains("42"),
        "DeepSeek should use the context (favorite number 42). Got: {}",
        trimmed
    );
    eprintln!("DeepSeek multi-turn response: {}", trimmed);
}

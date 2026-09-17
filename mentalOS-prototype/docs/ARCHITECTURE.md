# mentalOS Architecture

## Overview

mentalOS is a Rust (edition 2024) + GTK4 AI-first desktop assistant for Linux. It provides a natural-language control surface that routes user input through AI providers, parses commands from responses, and executes approved actions in a Firejail sandbox.

## Two-Thread Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    GTK4 UI Thread                         │
│  ┌──────────┐ ┌──────────┐ ┌───────────┐ ┌───────────┐ │
│  │ AppBar   │ │ ChatView │ │ OmniPill  │ │ ConfigWiz │ │
│  └──────────┘ └──────────┘ └───────────┘ └───────────┘ │
│       │            │             │                        │
│       └────────────┼─────────────┘                        │
│                    │                                      │
│         async_channel::Sender<BackendResponse>            │
│         async_channel::Receiver<BackendRequest>           │
│                    │                                      │
├────────────────────┼──────────────────────────────────────┤
│                    ▼                                      │
│                Tokio Runtime                              │
│  ┌──────────────────────────────────────────────────────┐ │
│  │              CommandRouter                            │ │
│  │  ┌─────────────────────────────────────────────┐     │ │
│  │  │  Box<dyn AiProvider>                        │     │ │
│  │  │  ┌──────────┐ ┌────────┐ ┌────────┐ ┌────┐ │     │ │
│  │  │  │OpenClaw  │ │Ollama  │ │DeepSeek│ │Zen │ │     │ │
│  │  │  └──────────┘ └────────┘ └────────┘ └────┘ │     │ │
│  │  └─────────────────────────────────────────────┘     │ │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────────────┐     │ │
│  │  │Whitelist │ │MemoryMgr │ │ FirejailExecutor │     │ │
│  │  └──────────┘ └──────────┘ └──────────────────┘     │ │
│  └──────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

### Thread Communication

- **UI → Backend**: `tokio::sync::mpsc::Sender<BackendRequest>` (bounded, capacity 32)
- **Backend → UI**: `async_channel::Sender<BackendResponse>` (unbounded, cross-runtime)
- The GTK4 main loop and the Tokio runtime never share mutable state directly — all communication goes through these channels.

## AI Provider Architecture

### AiProvider Trait

All AI providers implement the `AiProvider` trait defined in `src/providers/mod.rs`:

```rust
pub trait AiProvider: Send + Sync {
    fn send_message(&self, message: &str, context: &[Message]) -> Result<String>;
    fn list_agents(&self) -> Vec<String>;
    fn get_current_provider(&self) -> String;
    fn switch_agent(&mut self, name: &str) -> Result<String>;
    fn supports_context(&self) -> bool;
    fn name(&self) -> &str;
    fn is_healthy(&self) -> bool;
}
```

### Provider Factory

The `create_provider()` factory function reads `config.ai.provider` and constructs the appropriate provider:

```rust
pub fn create_provider(config: &Config) -> Result<Box<dyn AiProvider>>
```

Supported providers: `"openclaw"`, `"ollama"`, `"deepseek"`, `"zen"`.

### Provider Implementations

| Provider | Transport | Context Support | Health Check |
|----------|-----------|----------------|--------------|
| **OpenClaw** | CLI or HTTP (gateway) | Full (stdin/JSON) | TCP connect to gateway |
| **Ollama** | HTTP `/api/chat` | Full (multi-turn) | HTTP GET `/api/tags` |
| **DeepSeek** | HTTP Chat Completions | Full (message array) | HTTPS connectivity check |
| **OpenCode Zen** | HTTP Chat Completions | Full (message array) | HTTPS connectivity check |

### Resilience: Circuit Breaker

The `circuit_breaker` module (`src/providers/circuit_breaker.rs`) provides a thread-safe circuit breaker pattern:

- **Closed**: Normal operation. Consecutive failures are tracked.
- **Open**: Requests are rejected immediately. After a cooldown period, transitions to Half-Open.
- **Half-Open**: A single request is allowed. If it succeeds, the breaker closes. If it fails, it re-opens.

The circuit breaker uses atomic operations for lock-free state management, making it safe across the GTK4 UI thread and the Tokio backend thread.

### Resilience: Retry with Exponential Backoff

The `retry_with_backoff()` function handles transient failures by retrying a closure with increasing delays:

```rust
pub fn retry_with_backoff<F, T, E>(config: &RetryConfig, f: F) -> Result<T, E>
```

Default: 2 retries, 500ms initial delay, 2x multiplier, 10s max delay.

## Gateway Lifecycle (OpenClaw)

The `OpenClawLauncher` (`src/providers/openclaw_launcher.rs`) manages the OpenClaw gateway process:

1. **Auto-start**: If `openclaw.auto_start = true` and `transport = "http"`, the launcher spawns the gateway process with optional Firejail sandboxing.
2. **Health-check polling**: After starting the gateway, the launcher polls for readiness using exponential backoff (200ms initial, 2x multiplier, 3s cap, 10 attempts max). This replaces the old 500ms fixed sleep which was a race condition.
3. **Conditional startup**: The launcher is only created when `provider = "openclaw"`. Cloud providers (DeepSeek, Zen) don't need a local gateway.
4. **Emergency stop**: Kills the child process and any known related processes via `pkill`.

## Smart Context Injection

The `CommandRouter` builds a rich context message before sending to the AI:

- **CWD**: Current working directory
- **Open files**: Recently accessed files in the workspace
- **Recent commands**: Last N commands executed
- **Inferred goals**: Intent analysis from the user's input pattern

This context is passed via the `context: &[Message]` parameter of `send_message()`. All four providers now support context — the old CLI and Ollama transports that silently discarded context have been fixed.

## Command Execution Pipeline

```
User Input → Router → AI Provider → Response Text
                                          │
                                    Parse Commands
                                          │
                                    Whitelist Check
                                     ╱          ╲
                               Approved      Not Approved
                                  │               │
                          FirejailExecutor    ApprovalRequired
                          (sandboxed)         (UI gate)
```

### Firejail Sandbox

All AI-generated commands are executed in a Firejail sandbox with:
- No X11/GUI access (`--x11=none`)
- No network access by default (`--net=none`)
- Optional profile (`firejail/openclaw.profile` or `/etc/firejail/openclaw.profile`)
- 5-minute timeout per command
- PID tracking for emergency stop

## Configuration

Configuration is stored at `~/.config/mentalOS/config.toml` and loaded via `serde` + `toml`.

### Provider Configuration

Each provider has its own config section:

| Section | Key Fields |
|---------|-----------|
| `[ai]` | `provider`, `model`, `fallback_to_ollama`, `context_messages` |
| `[openclaw]` | `endpoint`, `transport`, `token`, `cli_path`, `auto_start` |
| `[ollama]` | `endpoint`, `model` |
| `[deepseek]` | `api_key`, `model`, `endpoint`, `thinking_mode`, `max_tokens`, `temperature` |
| `[zen]` | `api_key`, `model`, `endpoint`, `max_tokens` |

## Error Handling

All errors are consolidated into `MentalOSError`:

```rust
pub enum MentalOSError {
    Io(#[from] std::io::Error),
    Http(#[from] reqwest::Error),
    Json(#[from] serde_json::Error),
    Toml(#[from] toml::de::Error),
    ConfigMissing(PathBuf),
    ConfigInvalid(String),
    NotApproved(String),
    CommandFailed(String),
    ProviderUnavailable { provider: String, message: String },
    InvalidCommand(String),
    Other(String),
}
```

The `ProviderUnavailable` variant is a structured error with `provider` and `message` fields, used by all providers when the backend is unreachable.

## UI Architecture

### Provider Health Indicator

The AppBar includes a provider health indicator that shows the connection status of the current AI provider. A periodic health check runs every 30 seconds via `BackendRequest::CheckProviderHealth`, and the result is displayed as `[ProviderName: Connected]` or `[ProviderName: Disconnected]`.

### Config Wizard

The first-run setup wizard supports all four providers:
- **OpenClaw**: Gateway endpoint, CLI path, token, transport mode, auto-start
- **Ollama**: Endpoint, model name
- **DeepSeek**: API key (masked), model, endpoint, thinking mode, temperature
- **OpenCode Zen**: API key (masked), model, endpoint

API key fields use `input_purpose = Password` and `visibility = false` for privacy. A hint confirms that keys are stored locally only.

## Key Design Decisions

1. **Trait-based provider abstraction**: `AiProvider` trait + `Box<dyn AiProvider>` allows adding new providers without modifying the router or UI.
2. **Factory pattern**: `create_provider()` centralizes provider construction, keeping the main.rs and router clean.
3. **Circuit breaker over simple retry**: The circuit breaker prevents thundering-herd scenarios and provides a cooldown period for sustained failures.
4. **Health-check polling with exponential backoff**: Replaces the fragile 500ms fixed sleep when waiting for the OpenClaw gateway.
5. **Conditional launcher**: The OpenClaw launcher is only created when the provider is "openclaw", avoiding unnecessary process management for cloud providers.
6. **Firejail sandbox**: All AI-generated commands run sandboxed, with a whitelist approval gate for new commands.

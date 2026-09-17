# mentalOS User Guide

## Getting Started

### First Run

When you launch mentalOS for the first time, a setup wizard will guide you through configuring your AI provider. Choose from:

1. **OpenClaw (Local Gateway)** — A local AI gateway that runs on your machine. Best for privacy and offline use.
2. **Ollama (Local Models)** — Run AI models locally using Ollama. Requires Ollama to be installed and running.
3. **DeepSeek (Cloud API)** — A powerful cloud-based AI provider with OpenAI-compatible API. Requires an API key.
4. **OpenCode Zen (Multi-Model Gateway)** — Access 50+ models from a single gateway. 7 free models included. Requires an API key.

### Choosing a Provider

| Provider | Privacy | Cost | Setup | Model Variety |
|----------|---------|------|-------|---------------|
| OpenClaw | Full (local) | Free | Install OpenClaw | Limited |
| Ollama | Full (local) | Free | Install Ollama | Many (local) |
| DeepSeek | Cloud | $0.14/1M tokens | API key | DeepSeek models |
| Zen | Cloud | Free tier + paid | API key | 50+ models |

## Configuration

Your configuration is stored at `~/.config/mentalOS/config.toml`.

### Switching Providers

Edit the `provider` field in the `[ai]` section:

```toml
[ai]
provider = "deepseek"  # or "openclaw", "ollama", "zen"
```

Then restart mentalOS for the change to take effect.

### DeepSeek Configuration

```toml
[deepseek]
api_key = "sk-your-key-here"
model = "deepseek-v4-pro"  # or "deepseek-v4-flash" for faster/cheaper
thinking_mode = true  # Includes reasoning in responses
max_tokens = 4096
temperature = 0.7
```

Get your API key at [https://platform.deepseek.com](https://platform.deepseek.com).

### OpenCode Zen Configuration

```toml
[zen]
api_key = "zen-your-key-here"
model = "deepseek-v4-pro"  # 50+ models available
endpoint = "https://opencode.ai/zen/v1"
max_tokens = 4096
```

Get your API key at [https://opencode.ai/auth](https://opencode.ai/auth).

Popular models on Zen:
- **Free**: deepseek-v4-flash, llama-4-scout-17b, mistral-small-3.1
- **Paid**: claude-sonnet-5, gpt-5.4-mini, deepseek-v4-pro

### OpenClaw Configuration

```toml
[openclaw]
endpoint = "http://127.0.0.1:18789"
transport = "cli"  # or "http" for gateway mode
token = "your-token"
cli_path = "openclaw"
auto_start = false  # Set to true with transport = "http" to auto-start gateway
```

### Ollama Configuration

```toml
[ollama]
endpoint = "http://127.0.0.1:11434"
model = "phi3:mini"  # Any Ollama model you have pulled
```

Make sure Ollama is running before starting mentalOS: `ollama serve`

## Using mentalOS

### Basic Interaction

1. Type your request in the input field at the bottom of the window
2. Press Enter to send
3. The AI will respond with text and/or suggested commands
4. Commands that require approval will show a confirmation dialog

### Agent Selector

Use the agent selector near the input to switch between available AI providers and models. The current provider is shown in the top bar.

### Provider Health

The top bar shows a provider health indicator that updates every 30 seconds:
- **[ProviderName: Connected]** — The provider is reachable and responding
- **[ProviderName: Disconnected]** — The provider is unreachable

### Safety Features

- **Command Whitelist**: Known safe commands execute automatically
- **Approval Gate**: New or risky commands require your explicit approval
- **Firejail Sandbox**: All commands run in a sandboxed environment with no network access by default
- **Emergency Stop**: Press `Ctrl+Shift+Q` or click the STOP button to immediately halt all operations

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+Q` | Emergency stop |
| `Ctrl+N` | New chat |
| `Ctrl+Shift+M` | Open memory browser |
| `Ctrl+Shift+J` | Open tasks browser |
| `Ctrl+/` | Show shortcuts help |
| `Ctrl+Shift+H` | Toggle high contrast |
| `Ctrl+Plus/Minus/0` | Font size controls |
| `Ctrl+Comma` | Customize key bindings |

## Troubleshooting

### "Provider unavailable" error

- **OpenClaw**: Make sure the gateway is running (`openclaw gateway --port 18789`) or enable `auto_start = true` with `transport = "http"`
- **Ollama**: Make sure Ollama is running (`ollama serve`) and the model is pulled (`ollama pull phi3:mini`)
- **DeepSeek**: Check your API key and internet connection
- **Zen**: Check your API key and internet connection

### Gateway won't start

The OpenClaw launcher uses health-check polling with exponential backoff. If the gateway doesn't become healthy within ~20 seconds, check:
1. Is the OpenClaw binary in your PATH?
2. Is port 18789 available?
3. Check the gateway log at `~/.local/share/mentalOS/logs/openclaw-gateway.log`

### Circuit breaker is open

If a provider fails multiple times consecutively, the circuit breaker opens and rejects requests for 30 seconds. This is a safety feature to prevent hammering a downed provider. After the cooldown, a single request is allowed through to test recovery.

## Privacy

- **API keys** are stored locally in `~/.config/mentalOS/config.toml` and never shared
- **Conversation history** is stored locally in `~/workspaces/.memory/`
- **Command execution** is sandboxed via Firejail with no network access by default
- **Local providers** (OpenClaw, Ollama) keep all data on your machine
- **Cloud providers** (DeepSeek, Zen) send your messages to their servers per their respective privacy policies

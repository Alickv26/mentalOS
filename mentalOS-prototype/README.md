# mentalOS Prototype (Phase 0 / M0.1–M0.2)

This repository contains the Phase 0 prototype skeleton for mentalOS and an
idempotent setup script for Arch Linux.

## Structure

```
mentalOS-prototype/
├─ Cargo.toml
├─ config.example.toml
├─ setup.sh
└─ src/
   ├─ config.rs
   ├─ error.rs
   ├─ lib.rs
   ├─ main.rs
   ├─ memory.rs
   ├─ openclaw.rs
   ├─ router.rs
   ├─ whitelist.rs
   └─ workspace.rs
```

## Verification Checklist

1. Run `./setup.sh` on an Arch Linux host.
2. `cargo build` in this repo.
3. `ollama list` shows `phi3:mini`.
4. `openclaw --version` works, and a basic OpenClaw query responds.

## Tests

1. Unit + integration tests: `cargo test`
2. Live Ollama test: `MENTALOS_LIVE_OLLAMA=1 cargo test live_ollama`
3. Live OpenClaw test: `MENTALOS_LIVE_OPENCLAW=1 cargo test live_openclaw`

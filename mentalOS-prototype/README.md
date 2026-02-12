# mentalOS Prototype (Phase 0 / M0.1)

This repository contains the Phase 0 prototype skeleton for mentalOS and an
idempotent setup script for Arch Linux.

## Structure

```
mentalOS-prototype/
├─ Cargo.toml
├─ config.example.toml
├─ setup.sh
└─ src/
   └─ main.rs
```

## Verification Checklist

1. Run `./setup.sh` on an Arch Linux host.
2. `cargo build` in this repo.
3. `ollama list` shows `phi3:mini`.
4. `openclaw --version` works, and a basic OpenClaw query responds.

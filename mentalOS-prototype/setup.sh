#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

OPENCLAW_PATH_DEFAULT="/home/alick/Documents/CODE/mentalOS/openclaw"
OPENCLAW_PATH="${OPENCLAW_PATH:-$OPENCLAW_PATH_DEFAULT}"
OPENCLAW_LINK="$HOME/mentalOS/openclaw"
CONFIG_DIR="$HOME/.config/mentalOS"
WORKSPACE_DIR="$HOME/workspaces"
OLLAMA_MODEL_DEFAULT="phi3:mini"
OLLAMA_MODEL="${OLLAMA_MODEL:-$OLLAMA_MODEL_DEFAULT}"

log() {
  printf "[setup] %s\n" "$*"
}

warn() {
  printf "[setup][warn] %s\n" "$*" >&2
}

ensure_packages() {
  if ! command -v pacman >/dev/null 2>&1; then
    warn "pacman not found. Skipping package installation."
    return 0
  fi

  local missing=()
  local entry pkg cmd
  local entries=(
    "rust:rustc"
    "nodejs:node"
    "npm:npm"
    "gtk4:gtk4-demo"
    "firejail:firejail"
    "ollama:ollama"
  )

  for entry in "${entries[@]}"; do
    pkg="${entry%%:*}"
    cmd="${entry#*:}"

    if [ "$pkg" = "gtk4" ]; then
      if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists gtk4; then
        log "Skipping $pkg: pkg-config reports gtk4 is available."
        continue
      fi
    fi

    if command -v "$cmd" >/dev/null 2>&1; then
      log "Skipping $pkg: command already available ($cmd)."
      continue
    fi

    if ! pacman -Qi "$pkg" >/dev/null 2>&1; then
      missing+=("$pkg")
    fi
  done

  if [ "${#missing[@]}" -eq 0 ]; then
    log "All required packages already installed."
    return 0
  fi

  log "Installing packages: ${missing[*]}"
  if command -v sudo >/dev/null 2>&1; then
    sudo pacman -S --needed "${missing[@]}"
  else
    pacman -S --needed "${missing[@]}"
  fi
}

setup_openclaw_link() {
  mkdir -p "$HOME/mentalOS"

  if [ -L "$OPENCLAW_LINK" ]; then
    local target
    target="$(readlink "$OPENCLAW_LINK")"
    if [ "$target" = "$OPENCLAW_PATH" ]; then
      log "OpenClaw symlink already points to $OPENCLAW_PATH."
      return 0
    fi
    warn "OpenClaw symlink points to $target (expected $OPENCLAW_PATH)."
    return 0
  fi

  if [ -e "$OPENCLAW_LINK" ]; then
    warn "$OPENCLAW_LINK exists and is not a symlink. Skipping."
    return 0
  fi

  if [ ! -d "$OPENCLAW_PATH" ]; then
    warn "OpenClaw path not found: $OPENCLAW_PATH"
    warn "Set OPENCLAW_PATH to a valid clone and re-run."
    return 0
  fi

  ln -s "$OPENCLAW_PATH" "$OPENCLAW_LINK"
  log "Linked OpenClaw: $OPENCLAW_LINK -> $OPENCLAW_PATH"
}

setup_ollama() {
  if ! command -v ollama >/dev/null 2>&1; then
    warn "ollama not installed yet. Skipping model pull."
    return 0
  fi

  if ollama list 2>/dev/null | grep -q "${OLLAMA_MODEL}"; then
    log "Ollama model ${OLLAMA_MODEL} already present."
    return 0
  fi

  log "Pulling Ollama model ${OLLAMA_MODEL}..."
  if ! ollama pull "${OLLAMA_MODEL}"; then
    warn "Failed to pull ${OLLAMA_MODEL} (offline or service issue)."
  fi
}

setup_config() {
  mkdir -p "$CONFIG_DIR"
  if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    cp "$SCRIPT_DIR/config.example.toml" "$CONFIG_DIR/config.toml"
    log "Created $CONFIG_DIR/config.toml from template."
  else
    log "Config already exists at $CONFIG_DIR/config.toml."
  fi
}

setup_workspace() {
  mkdir -p "$WORKSPACE_DIR"
  log "Workspace directory ready: $WORKSPACE_DIR"
}

main() {
  log "Starting mentalOS Phase 0 setup (Arch Linux)."
  ensure_packages
  setup_openclaw_link
  setup_ollama
  setup_config
  setup_workspace

  cat <<EOF

Verification checklist:
1) Run: cargo build
2) Run: ollama list   (ensure ${OLLAMA_MODEL} is present)
3) Run: openclaw --version
4) Run a basic OpenClaw query to verify responses
EOF
}

main "$@"

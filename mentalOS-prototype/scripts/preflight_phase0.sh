#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/5] cargo fmt --check"
cargo fmt -- --check

echo "[2/5] cargo clippy --all-targets"
cargo clippy --all-targets

echo "[3/5] cargo test -q"
cargo test -q

echo "[4/5] sandbox smoke tests"
./scripts/sandbox_smoke.sh

echo "[5/5] ui accessibility tests"
cargo test -q --test ui_accessibility

echo "Preflight checks completed successfully."

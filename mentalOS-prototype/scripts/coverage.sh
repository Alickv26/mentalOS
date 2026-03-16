#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MIN_LINES="${MIN_LINES:-80}"

if ! cargo llvm-cov --version >/dev/null 2>&1; then
  echo "cargo-llvm-cov is not installed."
  echo "Install with: cargo install cargo-llvm-cov"
  exit 1
fi

echo "Running line coverage check (threshold: ${MIN_LINES}%)..."
cargo llvm-cov --workspace --all-features --summary-only --fail-under-lines "${MIN_LINES}"

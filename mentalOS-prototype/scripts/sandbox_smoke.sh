#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "Running sandbox security smoke tests (requires firejail)..."
MENTALOS_ENABLE_SANDBOX_TESTS=1 cargo test sandbox_ -- --nocapture

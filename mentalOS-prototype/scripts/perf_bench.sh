#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "Running latency benchmark..."
cargo test --release perf_router_latency_baseline -- --ignored --nocapture

echo "Running stress benchmark..."
cargo test --release perf_router_stress_rapid_queries -- --ignored --nocapture

# Performance Benchmarks

This project includes manual performance benchmarks focused on backend routing
latency and rapid-query stress throughput for the project-command path
(`run_project_command` with a noop executor).

## Run Benchmarks

From `mentalOS-prototype`:

```bash
./scripts/perf_bench.sh
```

On-demand CI workflow:

- Trigger `../.github/workflows/performance-benchmarks.yml` via
  `Actions -> Performance Benchmarks -> Run workflow`.
- The workflow uploads:
  - `performance-bench.log`
  - `performance-summary.txt`

Optional environment overrides:

- `MENTALOS_PERF_ITERATIONS` (default `150`)
- `MENTALOS_PERF_WARMUP` (default `20`)
- `MENTALOS_STRESS_ITERATIONS` (default `1500`)

## Benchmark Tests

- `tests/performance.rs::perf_router_latency_baseline`
- `tests/performance.rs::perf_router_stress_rapid_queries`

Both are `#[ignore]` tests and only run when explicitly invoked.

## Latest Baseline

Run date: `2026-03-16`
Mode: `cargo test --release ... --ignored --nocapture`

- latency benchmark (`n=150`, warmup `20`)
  - mean: `0.10 ms`
  - p50: `0.10 ms`
  - p95: `0.15 ms`
  - max: `0.20 ms`
  - throughput: `9958.92 req/s`
- stress benchmark (`n=1500`)
  - total: `1.081 s`
  - throughput: `1387.37 req/s`

## Baseline (commit c3295c9, post-circuit-breaker-wiring)

Run date: `2026-09-18`
Mode: `cargo test --no-default-features --test performance -- --ignored --nocapture`
Rust: `1.98.1` (debug build, no GTK4 feature)
Hardware: containerised Linux (Debian trixie)

- latency benchmark (`n=150`, warmup `20`)
  - mean: `4.62 ms`
  - p50: `3.70 ms`
  - p95: `6.12 ms`
  - max: `32.65 ms`
  - throughput: `212.55 req/s`
- stress benchmark (`n=1500`)
  - total: `39.386 s`
  - throughput: `38.08 req/s`

Note: this baseline is from a debug build with `--no-default-features` (no
GTK4 UI). The previous `9958.92 req/s` number was from a `--release` build
and used the pre-trait-abstraction concrete `OpenClawClient`. The
performance regression is expected for debug builds and will be re-measured
in `--release` mode in a follow-up. The relative cost of the new
`retry_with_backoff` wrapper (which now wraps every `send_message` call)
should be measured in a release build before drawing conclusions.

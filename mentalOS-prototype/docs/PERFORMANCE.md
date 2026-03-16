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

use mentalOS::config::{AiConfig, OllamaConfig, OpenClawConfig, PathsConfig};
use mentalOS::memory::MemoryManager;
use mentalOS::openclaw::OpenClawClient;
use mentalOS::project_handler::ProjectHandler;
use mentalOS::router::{CommandExecutor, CommandOutput, CommandRouter};
use mentalOS::whitelist::WhitelistManager;
use mentalOS::workspace::{ProjectCommands, WorkspaceManager};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;

struct NoopExecutor;

impl CommandExecutor for NoopExecutor {
    fn execute(&self, command: &str) -> mentalOS::Result<CommandOutput> {
        Ok(CommandOutput {
            command: command.to_string(),
            exit_code: 0,
            stdout: "ok".to_string(),
            stderr: String::new(),
        })
    }
}

#[tokio::test]
#[ignore = "manual performance benchmark"]
async fn perf_router_latency_baseline() {
    let iterations = env_usize("MENTALOS_PERF_ITERATIONS", 150);
    let warmup = env_usize("MENTALOS_PERF_WARMUP", 20);

    let (mut router, _temp_dir) = build_benchmark_router();

    let mut samples = Vec::with_capacity(iterations);
    let begin = Instant::now();
    for i in 0..(iterations + warmup) {
        let start = Instant::now();
        let response = router
            .run_project_command("bench", "general", "run")
            .expect("project command should succeed");
        assert!(
            !response.message.is_empty(),
            "response message should not be empty"
        );
        if i >= warmup {
            samples.push(start.elapsed());
        }
    }
    let total = begin.elapsed();

    let stats = summarize_latency(&samples, total);
    println!(
        "PERF latency benchmark: n={} mean_ms={:.2} p50_ms={:.2} p95_ms={:.2} max_ms={:.2} throughput_rps={:.2}",
        stats.count, stats.mean_ms, stats.p50_ms, stats.p95_ms, stats.max_ms, stats.throughput_rps
    );
}

#[tokio::test]
#[ignore = "manual performance benchmark"]
async fn perf_router_stress_rapid_queries() {
    let iterations = env_usize("MENTALOS_STRESS_ITERATIONS", 1500);

    let (mut router, _temp_dir) = build_benchmark_router();
    let begin = Instant::now();

    for _ in 0..iterations {
        let response = router
            .run_project_command("bench", "general", "run")
            .expect("project command should succeed");
        assert!(
            !response.message.is_empty(),
            "response message should not be empty"
        );
    }

    let total = begin.elapsed();
    let rps = iterations as f64 / total.as_secs_f64();
    println!(
        "PERF stress benchmark: n={} total_s={:.3} throughput_rps={:.2}",
        iterations,
        total.as_secs_f64(),
        rps
    );
}

fn build_benchmark_router() -> (CommandRouter<NoopExecutor>, TempDir) {
    let temp_dir = TempDir::new().expect("temp dir");
    let workspace_root = temp_dir.path().join("workspaces");
    let workspace_manager = WorkspaceManager::new(workspace_root.clone());
    let workspace_path = workspace_manager
        .create_workspace("bench", "perf")
        .expect("workspace");
    workspace_manager
        .update_metadata(
            &workspace_path,
            Some("rust".to_string()),
            None,
            Some("Benchmark workspace".to_string()),
            ProjectCommands {
                setup: Some("echo setup".to_string()),
                run: Some("echo run".to_string()),
                test: Some("echo test".to_string()),
            },
        )
        .expect("metadata");

    let memory = Arc::new(Mutex::new(MemoryManager::new(workspace_root.clone())));
    let whitelist_path = temp_dir.path().join("whitelist.json");
    let mut whitelist = WhitelistManager::load(whitelist_path).expect("whitelist");
    whitelist.add_exact("echo run");
    let whitelist = Arc::new(Mutex::new(whitelist));

    let config = mentalOS::Config {
        ai: AiConfig::default(),
        openclaw: OpenClawConfig::default(),
        ollama: OllamaConfig::default(),
        paths: PathsConfig::default(),
        agents: std::collections::HashMap::new(),
    };

    let openclaw = OpenClawClient::from_config(&config);
    let project_handler = ProjectHandler::new(workspace_root.clone(), "opencode".to_string());
    let router = CommandRouter::new(openclaw, whitelist, memory, NoopExecutor)
        .with_project_handler(project_handler)
        .with_workspace_manager(WorkspaceManager::new(workspace_root));

    (router, temp_dir)
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

#[derive(Debug)]
struct LatencyStats {
    count: usize,
    mean_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    max_ms: f64,
    throughput_rps: f64,
}

fn summarize_latency(samples: &[Duration], total: Duration) -> LatencyStats {
    assert!(!samples.is_empty(), "samples must not be empty");
    let mut ms = samples
        .iter()
        .map(|d| d.as_secs_f64() * 1000.0)
        .collect::<Vec<f64>>();
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let count = ms.len();
    let mean_ms = ms.iter().sum::<f64>() / count as f64;
    let p50_ms = percentile(&ms, 0.50);
    let p95_ms = percentile(&ms, 0.95);
    let max_ms = *ms.last().unwrap_or(&0.0);
    let throughput_rps = count as f64 / total.as_secs_f64();

    LatencyStats {
        count,
        mean_ms,
        p50_ms,
        p95_ms,
        max_ms,
        throughput_rps,
    }
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let q = q.clamp(0.0, 1.0);
    let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[idx]
}

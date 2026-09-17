//! OpenClaw gateway launcher with health-check polling and exponential backoff.
//!
//! This module manages the OpenClaw gateway process lifecycle, including:
//! - Auto-starting the gateway when configured for HTTP transport
//! - Health-check polling with exponential backoff (replaces the old 500ms sleep)
//! - Graceful shutdown on emergency stop
//! - Process cleanup via `pkill` for known process names

use crate::config::Config;
use crate::error::{MentalOSError, Result};
use log::{info, warn};
use reqwest::Url;
use std::fs::{self, OpenOptions};
use std::io;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Default maximum number of health-check attempts when waiting for the gateway.
const DEFAULT_HEALTH_CHECK_ATTEMPTS: u32 = 10;

/// Initial delay between health-check attempts (milliseconds).
const INITIAL_BACKOFF_MS: u64 = 200;

/// Maximum backoff delay cap (milliseconds).
const MAX_BACKOFF_MS: u64 = 3000;

/// Backoff multiplier for exponential backoff.
const BACKOFF_MULTIPLIER: u64 = 2;

pub struct OpenClawLauncher {
    enabled: bool,
    endpoint: String,
    cli_path: String,
    child: Option<Child>,
    log_path: PathBuf,
    /// Maximum number of health-check attempts when waiting for gateway startup.
    health_check_attempts: u32,
}

impl OpenClawLauncher {
    pub fn from_config(config: &Config) -> Self {
        let enabled =
            config.openclaw.auto_start && config.openclaw.transport.eq_ignore_ascii_case("http");
        let log_path = default_log_path().join("openclaw-gateway.log");

        Self {
            enabled,
            endpoint: config.openclaw.endpoint.clone(),
            cli_path: config.openclaw.cli_path.clone(),
            child: None,
            log_path,
            health_check_attempts: DEFAULT_HEALTH_CHECK_ATTEMPTS,
        }
    }

    /// Set the maximum number of health-check attempts when waiting for the gateway.
    pub fn with_health_check_attempts(mut self, attempts: u32) -> Self {
        self.health_check_attempts = attempts;
        self
    }

    /// Ensure the OpenClaw gateway is running.
    ///
    /// If the gateway is already healthy (TCP connection succeeds), returns immediately.
    /// If the gateway is not running and auto-start is enabled, starts the gateway process
    /// and then polls for readiness using exponential backoff.
    ///
    /// # Errors
    ///
    /// Returns `ProviderUnavailable` if:
    /// - The gateway process fails to spawn
    /// - The gateway does not become healthy within the health-check polling window
    pub fn ensure_running(&mut self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Fast path: already healthy
        if self.is_healthy()? {
            return Ok(());
        }

        // If a child process exists, check if it's still running
        if let Some(child) = self.child.as_mut() {
            if child.try_wait()?.is_none() {
                // Process is still running but not healthy yet — wait for it
                return self.wait_for_gateway();
            }
            // Process exited — clean up and try to restart
            self.child = None;
        }

        // Start the gateway and wait for it to become healthy
        self.start_gateway()?;
        self.wait_for_gateway()
    }

    /// Stop the OpenClaw gateway process.
    ///
    /// Kills the child process (if we spawned it) and then attempts to kill
    /// any known related processes via `pkill`.
    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Self::kill_known_processes()
    }

    /// Kill any known OpenClaw-related processes.
    pub fn kill_known_processes() -> Result<()> {
        for pattern in ["openclaw", "firejail", "opencode"] {
            let _ = Command::new("pkill")
                .args(["-f", pattern])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        Ok(())
    }

    /// Check if the gateway is currently healthy by attempting a TCP connection.
    ///
    /// Uses a 500ms connect timeout. Returns `Ok(true)` if the connection succeeds,
    /// `Ok(false)` if the connection is refused or times out.
    fn is_healthy(&self) -> Result<bool> {
        let addr = endpoint_socket_addr(&self.endpoint)?;
        match TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
            Ok(_) => Ok(true),
            Err(err) => {
                if err.kind() != io::ErrorKind::ConnectionRefused {
                    warn!("OpenClaw health probe failed: {}", err);
                }
                Ok(false)
            }
        }
    }

    /// Wait for the gateway to become healthy using exponential backoff polling.
    ///
    /// Starting from `INITIAL_BACKOFF_MS`, the delay doubles each attempt up to
    /// `MAX_BACKOFF_MS`. After `health_check_attempts` failures, returns an error.
    ///
    /// This replaces the old 500ms fixed sleep which was a race condition — the gateway
    /// might not be ready in 500ms on slow machines, and might be ready much sooner on
    /// fast ones, wasting time.
    fn wait_for_gateway(&self) -> Result<()> {
        let mut delay_ms = INITIAL_BACKOFF_MS;

        for attempt in 1..=self.health_check_attempts {
            info!(
                "Waiting for OpenClaw gateway (attempt {}/{}, delay={}ms)",
                attempt, self.health_check_attempts, delay_ms
            );

            std::thread::sleep(Duration::from_millis(delay_ms));

            if self.is_healthy()? {
                info!("OpenClaw gateway is healthy after {} attempts", attempt);
                return Ok(());
            }

            // Exponential backoff with cap
            delay_ms = (delay_ms * BACKOFF_MULTIPLIER).min(MAX_BACKOFF_MS);
        }

        Err(MentalOSError::ProviderUnavailable {
            provider: "openclaw".to_string(),
            message: format!(
                "OpenClaw gateway did not become healthy after {} attempts (total wait ~{}s)",
                self.health_check_attempts,
                self.estimate_total_wait_secs()
            ),
        })
    }

    /// Estimate the total wait time in seconds for the full health-check cycle.
    fn estimate_total_wait_secs(&self) -> u64 {
        let mut total_ms = 0u64;
        let mut delay = INITIAL_BACKOFF_MS;
        for _ in 0..self.health_check_attempts {
            total_ms += delay;
            delay = (delay * BACKOFF_MULTIPLIER).min(MAX_BACKOFF_MS);
        }
        total_ms / 1000
    }

    /// Start the OpenClaw gateway process.
    ///
    /// The gateway is started with optional Firejail sandboxing if a profile is found.
    /// stdout and stderr are redirected to a log file.
    fn start_gateway(&mut self) -> Result<()> {
        let port = endpoint_port(&self.endpoint)?;
        let log_file = open_log_file(&self.log_path)?;
        let log_file_err = log_file.try_clone()?;

        let profile_arg = if std::path::Path::new("firejail/openclaw.profile").exists() {
            Some("--profile=firejail/openclaw.profile".to_string())
        } else if std::path::Path::new("/etc/firejail/openclaw.profile").exists() {
            Some("--profile=openclaw".to_string())
        } else {
            None
        };

        let mut command = if profile_arg.is_some() {
            let mut cmd = Command::new("firejail");
            cmd.arg("--quiet");
            if let Some(profile) = profile_arg {
                cmd.arg(profile);
            }
            cmd.arg("--x11=none");
            cmd.arg("--");
            cmd.arg(&self.cli_path);
            cmd
        } else {
            Command::new(&self.cli_path)
        };

        command
            .args(["gateway", "--port", &port.to_string(), "--verbose"])
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_err));

        let child = command.spawn().map_err(|err| {
            MentalOSError::ProviderUnavailable {
                provider: "openclaw".to_string(),
                message: format!("Failed to start OpenClaw gateway: {err}"),
            }
        })?;

        info!(
            "Started OpenClaw gateway on port {} (pid={})",
            port,
            child.id()
        );
        self.child = Some(child);
        Ok(())
    }
}

fn default_log_path() -> PathBuf {
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    base.join(".local")
        .join("share")
        .join("mentalOS")
        .join("logs")
}

fn open_log_file(path: &PathBuf) -> Result<std::fs::File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    Ok(file)
}

fn endpoint_port(endpoint: &str) -> Result<u16> {
    let url = Url::parse(endpoint)
        .map_err(|err| MentalOSError::ConfigInvalid(format!("Invalid OpenClaw endpoint: {err}")))?;
    Ok(url.port_or_known_default().unwrap_or(18789))
}

fn endpoint_socket_addr(endpoint: &str) -> Result<SocketAddr> {
    let url = Url::parse(endpoint)
        .map_err(|err| MentalOSError::ConfigInvalid(format!("Invalid OpenClaw endpoint: {err}")))?;
    let host = url
        .host_str()
        .ok_or_else(|| MentalOSError::ConfigInvalid("OpenClaw endpoint has no host".to_string()))?;
    let port = url.port_or_known_default().unwrap_or(18789);
    (host, port).to_socket_addrs()?.next().ok_or_else(|| {
        MentalOSError::ConfigInvalid("Could not resolve OpenClaw endpoint".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_backoff_stays_within_cap() {
        let mut delay = INITIAL_BACKOFF_MS;
        for _ in 0..20 {
            delay = (delay * BACKOFF_MULTIPLIER).min(MAX_BACKOFF_MS);
            assert!(delay <= MAX_BACKOFF_MS);
        }
    }

    #[test]
    fn estimate_total_wait_is_reasonable() {
        let launcher = OpenClawLauncher::from_config(&Config::default());
        let secs = launcher.estimate_total_wait_secs();
        // With 10 attempts, 200ms initial, 2x multiplier, 3s cap:
        // 200 + 400 + 800 + 1600 + 3000 + 3000 + 3000 + 3000 + 3000 + 3000 = 20000ms = 20s
        assert!(secs > 0 && secs < 60, "Total wait should be reasonable, got {}s", secs);
    }

    #[test]
    fn launcher_disabled_when_not_http_transport() {
        let mut config = Config::default();
        config.openclaw.transport = "cli".to_string();
        config.openclaw.auto_start = true;
        let launcher = OpenClawLauncher::from_config(&config);
        assert!(!launcher.enabled);
    }

    #[test]
    fn launcher_disabled_when_auto_start_false() {
        let mut config = Config::default();
        config.openclaw.transport = "http".to_string();
        config.openclaw.auto_start = false;
        let launcher = OpenClawLauncher::from_config(&config);
        assert!(!launcher.enabled);
    }

    // ── Tests ported from the parallel session's openclaw_launcher.rs ──

    #[test]
    fn from_config_disabled_when_auto_start_off() {
        let config = Config::default();
        let launcher = OpenClawLauncher::from_config(&config);
        assert!(!launcher.enabled);
        assert!(launcher.child.is_none());
    }

    #[test]
    fn from_config_enabled_when_auto_start_on_and_http_transport() {
        let mut config = Config::default();
        config.openclaw = crate::config::OpenClawConfig {
            auto_start: true,
            transport: "http".to_string(),
            ..crate::config::OpenClawConfig::default()
        };
        let launcher = OpenClawLauncher::from_config(&config);
        assert!(launcher.enabled);
    }

    #[test]
    fn from_config_disabled_when_auto_start_on_but_cli_transport() {
        let mut config = Config::default();
        config.openclaw = crate::config::OpenClawConfig {
            auto_start: true,
            transport: "cli".to_string(),
            ..crate::config::OpenClawConfig::default()
        };
        let launcher = OpenClawLauncher::from_config(&config);
        assert!(!launcher.enabled);
    }

    #[test]
    fn stop_is_safe_when_no_child() {
        let config = Config::default();
        let mut launcher = OpenClawLauncher::from_config(&config);
        // Should not panic or error when there's no child process
        let result = launcher.stop();
        assert!(result.is_ok());
    }

    #[test]
    fn kill_known_processes_does_not_panic() {
        // Should gracefully handle pkill even if no matching processes
        let result = OpenClawLauncher::kill_known_processes();
        assert!(result.is_ok());
    }

    #[test]
    fn endpoint_port_parses_valid_url() {
        assert_eq!(endpoint_port("http://127.0.0.1:18789").unwrap(), 18789);
        assert_eq!(endpoint_port("http://127.0.0.1:9999").unwrap(), 9999);
    }

    #[test]
    fn endpoint_port_defaults_to_known_scheme_port() {
        let port = endpoint_port("http://localhost").unwrap();
        assert_eq!(port, 80);
    }

    #[test]
    fn endpoint_port_rejects_invalid_url() {
        assert!(endpoint_port("not a url").is_err());
    }

    #[test]
    fn default_log_path_returns_path_based_on_home() {
        let path = default_log_path();
        assert!(path.to_string_lossy().contains(".local/share/mentalOS/logs"));
    }

    #[test]
    fn open_log_file_creates_in_temp_dir() {
        let dir = std::env::temp_dir().join("mentalos-test-logs");
        let path = dir.join("test.log");
        let _file = open_log_file(&path).expect("should open file");
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn endpoint_socket_addr_resolves_localhost() {
        let addr = endpoint_socket_addr("http://127.0.0.1:18789").unwrap();
        assert_eq!(addr.port(), 18789);
    }

    #[test]
    fn endpoint_socket_addr_rejects_invalid() {
        assert!(endpoint_socket_addr("://bad").is_err());
    }
}

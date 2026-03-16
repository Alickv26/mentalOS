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

pub struct OpenClawLauncher {
    enabled: bool,
    endpoint: String,
    cli_path: String,
    child: Option<Child>,
    log_path: PathBuf,
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
        }
    }

    pub fn ensure_running(&mut self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        if self.is_healthy()? {
            return Ok(());
        }

        if let Some(child) = self.child.as_mut() {
            if child.try_wait()?.is_none() {
                return Ok(());
            }
            self.child = None;
        }

        self.start_gateway()?;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Self::kill_known_processes()
    }

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
            MentalOSError::OpenClawUnavailable(format!("Failed to start OpenClaw gateway: {err}"))
        })?;

        info!(
            "Started OpenClaw gateway on port {} (pid={})",
            port,
            child.id()
        );
        self.child = Some(child);
        Ok(())
    }

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

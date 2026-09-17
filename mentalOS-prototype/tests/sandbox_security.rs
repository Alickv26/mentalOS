use mental_os::router::{CommandExecutor, FirejailExecutor};
use std::path::Path;
use std::process::Command;

fn sandbox_tests_enabled() -> bool {
    std::env::var("MENTALOS_ENABLE_SANDBOX_TESTS")
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn firejail_usable() -> bool {
    let version_ok = Command::new("firejail")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !version_ok {
        return false;
    }
    Command::new("firejail")
        .args(["--quiet", "--", "sh", "-c", "true"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn sandbox_env_and_private_tmp_isolation() {
    if !sandbox_tests_enabled() || !firejail_usable() {
        return;
    }

    let marker = format!("/tmp/mentalos-sandbox-marker-{}", std::process::id());
    let _ = std::fs::remove_file(&marker);

    let executor = FirejailExecutor::new().with_timeout_secs(15);
    let cmd = format!(
        "sh -lc 'echo -n \"${{MENTALOS_SANDBOX:-unset}}\"; touch {}'",
        marker
    );
    let output = executor
        .execute(&cmd)
        .expect("sandbox command should execute");

    assert_eq!(output.exit_code, 0, "sandbox command should succeed");
    assert_eq!(
        output.stdout.trim(),
        "1",
        "expected MENTALOS_SANDBOX to be set by firejail profile"
    );
    assert!(
        !Path::new(&marker).exists(),
        "marker should not be visible on host when private-tmp is active"
    );
}

#[test]
fn sandbox_enforces_command_timeout() {
    if !sandbox_tests_enabled() || !firejail_usable() {
        return;
    }

    let executor = FirejailExecutor::new().with_timeout_secs(1);
    let err = executor
        .execute("sh -lc 'sleep 5'")
        .expect_err("expected timeout error");
    let message = err.to_string();
    assert!(
        message.contains("timed out"),
        "expected timeout error, got: {}",
        message
    );
}

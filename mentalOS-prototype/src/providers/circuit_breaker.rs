//! Circuit breaker pattern for AI provider resilience.
//!
//! The circuit breaker prevents repeated requests to a failing provider,
//! allowing it time to recover. It operates in three states:
//!
//! - **Closed**: Normal operation. Requests pass through. Consecutive failures
//!   are tracked; once they exceed `failure_threshold`, the breaker opens.
//! - **Open**: All requests immediately fail with `ProviderUnavailable`.
//!   The breaker remains open for `cooldown_duration`, then transitions to
//!   Half-Open.
//! - **Half-Open**: A single request is allowed through. If it succeeds, the
//!   breaker closes (resets failure count). If it fails, the breaker re-opens.
//!
//! This prevents thundering-herd scenarios where a downed provider is
//! bombarded with retries that would otherwise succeed after a cooldown.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

/// State of the circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — requests pass through.
    Closed,
    /// Provider is failing — requests are rejected immediately.
    Open,
    /// Testing whether the provider has recovered — one request is allowed.
    HalfOpen,
}

/// Configuration for a circuit breaker instance.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before the breaker opens.
    pub failure_threshold: u32,
    /// Duration to wait in the Open state before transitioning to Half-Open.
    pub cooldown_duration: Duration,
    /// Maximum number of consecutive successes in Half-Open before closing.
    /// Defaults to 1 (single successful request closes the breaker).
    pub half_open_success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            cooldown_duration: Duration::from_secs(30),
            half_open_success_threshold: 1,
        }
    }
}

/// Thread-safe circuit breaker for protecting AI provider calls.
///
/// Uses atomic operations for lock-free state management, making it safe
/// to share across the GTK4 UI thread and the Tokio backend thread.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    /// Provider name for error messages.
    provider_name: String,
    /// Current state: 0 = Closed, 1 = Open, 2 = HalfOpen.
    state: AtomicU32,
    /// Consecutive failure count.
    consecutive_failures: AtomicU32,
    /// Consecutive success count (used in Half-Open).
    consecutive_successes: AtomicU32,
    /// Timestamp (ms since epoch) when the breaker opened.
    /// 0 means not currently open.
    opened_at_ms: AtomicU64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration.
    pub fn new(provider_name: String, config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            provider_name,
            state: AtomicU32::new(0), // Closed
            consecutive_failures: AtomicU32::new(0),
            consecutive_successes: AtomicU32::new(0),
            opened_at_ms: AtomicU64::new(0),
        }
    }

    /// Create a circuit breaker with default configuration.
    pub fn with_defaults(provider_name: String) -> Self {
        Self::new(provider_name, CircuitBreakerConfig::default())
    }

    /// Check whether a request is allowed through the circuit breaker.
    ///
    /// Returns `Ok(())` if the request should proceed, or an error
    /// if the breaker is Open and the cooldown hasn't elapsed.
    ///
    /// In Half-Open state, a single request is allowed through.
    pub fn allow_request(&self) -> Result<(), String> {
        let state = self.current_state();

        match state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                // Check if cooldown has elapsed
                let opened_at = self.opened_at_ms.load(Ordering::Relaxed);
                if opened_at == 0 {
                    return Ok(());
                }
                let elapsed = self.millis_since(opened_at);
                if elapsed >= self.config.cooldown_duration.as_millis() as u64 {
                    // Transition to Half-Open
                    self.state.store(2, Ordering::Relaxed);
                    self.consecutive_successes.store(0, Ordering::Relaxed);
                    Ok(())
                } else {
                    let remaining = self.config.cooldown_duration.as_secs()
                        - (elapsed / 1000).min(self.config.cooldown_duration.as_secs());
                    Err(format!(
                        "Provider '{}' is temporarily unavailable (circuit breaker open, {}s remaining in cooldown)",
                        self.provider_name, remaining
                    ))
                }
            }
            CircuitState::HalfOpen => {
                // Allow one request through to test recovery
                Ok(())
            }
        }
    }

    /// Record a successful request.
    ///
    /// In Closed state, this resets the failure counter.
    /// In Half-Open state, this may transition the breaker back to Closed.
    pub fn record_success(&self) {
        let state = self.current_state();
        match state {
            CircuitState::Closed => {
                self.consecutive_failures.store(0, Ordering::Relaxed);
            }
            CircuitState::HalfOpen => {
                let successes = self.consecutive_successes.fetch_add(1, Ordering::Relaxed) + 1;
                if successes >= self.config.half_open_success_threshold {
                    // Transition to Closed
                    self.state.store(0, Ordering::Relaxed);
                    self.consecutive_failures.store(0, Ordering::Relaxed);
                    self.opened_at_ms.store(0, Ordering::Relaxed);
                    log::info!(
                        "Circuit breaker for '{}' closed after {} successful requests",
                        self.provider_name, successes
                    );
                }
            }
            CircuitState::Open => {
                // Shouldn't happen — a success in Open state means
                // someone bypassed the breaker. Reset to Closed.
                self.state.store(0, Ordering::Relaxed);
                self.consecutive_failures.store(0, Ordering::Relaxed);
                self.opened_at_ms.store(0, Ordering::Relaxed);
            }
        }
    }

    /// Record a failed request.
    ///
    /// In Closed state, this increments the failure counter. If the threshold
    /// is reached, the breaker opens.
    /// In Half-Open state, this immediately re-opens the breaker.
    pub fn record_failure(&self) {
        let state = self.current_state();
        match state {
            CircuitState::Closed => {
                let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
                if failures >= self.config.failure_threshold {
                    self.open_breaker();
                    log::warn!(
                        "Circuit breaker for '{}' opened after {} consecutive failures",
                        self.provider_name, failures
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Immediately re-open — the test request failed
                self.open_breaker();
                log::warn!(
                    "Circuit breaker for '{}' re-opened (half-open test request failed)",
                    self.provider_name
                );
            }
            CircuitState::Open => {
                // Already open — just increment failure count
                self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get the current state of the circuit breaker.
    pub fn state(&self) -> CircuitState {
        self.current_state()
    }

    /// Get the provider name associated with this breaker.
    pub fn provider_name(&self) -> &str {
        &self.provider_name
    }

    /// Reset the circuit breaker to Closed state.
    pub fn reset(&self) {
        self.state.store(0, Ordering::Relaxed);
        self.consecutive_failures.store(0, Ordering::Relaxed);
        self.consecutive_successes.store(0, Ordering::Relaxed);
        self.opened_at_ms.store(0, Ordering::Relaxed);
    }

    /// Get the current consecutive failure count.
    pub fn failure_count(&self) -> u32 {
        self.consecutive_failures.load(Ordering::Relaxed)
    }

    // ── Private helpers ──────────────────────────────────────

    fn current_state(&self) -> CircuitState {
        match self.state.load(Ordering::Relaxed) {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed, // shouldn't happen
        }
    }

    fn open_breaker(&self) {
        self.state.store(1, Ordering::Relaxed);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.opened_at_ms.store(now_ms, Ordering::Relaxed);
    }

    fn millis_since(&self, since_ms: u64) -> u64 {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now_ms.saturating_sub(since_ms)
    }
}

/// Retry logic with exponential backoff for AI provider calls.
///
/// Unlike the circuit breaker (which protects against sustained failures),
/// `retry_with_backoff` handles transient failures by retrying a closure
/// with increasing delays.
pub struct RetryConfig {
    /// Maximum number of retry attempts (0 = no retries).
    pub max_retries: u32,
    /// Initial delay between retries.
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Multiplier for exponential backoff.
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
        }
    }
}

/// Execute a closure with retry and exponential backoff.
///
/// The closure receives the current attempt number (0-based).
/// Returns the first successful result, or the last error if all attempts fail.
pub fn retry_with_backoff<F, T, E>(
    config: &RetryConfig,
    mut f: F,
) -> Result<T, E>
where
    F: FnMut(u32) -> Result<T, E>,
    E: std::fmt::Display,
{
    let mut last_err = None;
    let mut delay = config.initial_delay;

    for attempt in 0..=config.max_retries {
        match f(attempt) {
            Ok(val) => return Ok(val),
            Err(err) => {
                if attempt < config.max_retries {
                    log::warn!(
                        "Attempt {}/{} failed: {}. Retrying in {}ms...",
                        attempt + 1,
                        config.max_retries + 1,
                        err,
                        delay.as_millis()
                    );
                    std::thread::sleep(delay);
                    delay = Duration::from_secs_f64(
                        (delay.as_secs_f64() * config.multiplier).min(config.max_delay.as_secs_f64()),
                    );
                }
                last_err = Some(err);
            }
        }
    }

    Err(last_err.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::with_defaults("test".to_string());
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request().is_ok());
    }

    #[test]
    fn circuit_breaker_opens_after_threshold_failures() {
        let cb = CircuitBreaker::new(
            "test".to_string(),
            CircuitBreakerConfig {
                failure_threshold: 3,
                cooldown_duration: Duration::from_secs(30),
                half_open_success_threshold: 1,
            },
        );

        // Record 2 failures — should still be closed
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request().is_ok());

        // 3rd failure — should open
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(cb.allow_request().is_err());
    }

    #[test]
    fn circuit_breaker_success_resets_failures() {
        let cb = CircuitBreaker::new(
            "test".to_string(),
            CircuitBreakerConfig {
                failure_threshold: 3,
                cooldown_duration: Duration::from_secs(30),
                half_open_success_threshold: 1,
            },
        );

        cb.record_failure();
        cb.record_failure();
        cb.record_success(); // Resets failure count
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed); // Only 1 failure, not 3
    }

    #[test]
    fn circuit_breaker_reset_returns_to_closed() {
        let cb = CircuitBreaker::with_defaults("test".to_string());
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request().is_ok());
    }

    #[test]
    fn retry_with_backoff_succeeds_first_try() {
        let config = RetryConfig::default();
        let result = retry_with_backoff(&config, |_: u32| -> Result<i32, &str> { Ok(42) });
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn retry_with_backoff_succeeds_after_retry() {
        let config = RetryConfig {
            max_retries: 2,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            multiplier: 2.0,
        };
        let mut attempts = 0;
        let result = retry_with_backoff(&config, |_: u32| {
            attempts += 1;
            if attempts < 2 {
                Err("transient")
            } else {
                Ok(42)
            }
        });
        assert_eq!(result, Ok(42));
        assert_eq!(attempts, 2);
    }

    #[test]
    fn retry_with_backoff_exhausts_retries() {
        let config = RetryConfig {
            max_retries: 1,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            multiplier: 2.0,
        };
        let result = retry_with_backoff(&config, |_: u32| -> Result<i32, &str> { Err("fail") });
        assert_eq!(result, Err("fail"));
    }
}

use std::time::Duration;

use rand::Rng;
use thiserror::Error;
use tracing::debug;

/// Configuration for exponential backoff.
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Base delay for first retry.
    pub base_delay: Duration,
    /// Maximum delay cap.
    pub max_delay: Duration,
    /// Maximum number of retries.
    pub max_retries: u32,
    /// Enable jitter.
    pub jitter_enabled: bool,
    /// Jitter percentage (0.0 to 1.0).
    pub jitter_percent: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_secs(30),
            max_delay: Duration::from_secs(300),
            max_retries: 5,
            jitter_enabled: true,
            jitter_percent: 0.1,
        }
    }
}

impl BackoffConfig {
    /// Validate configuration values.
    pub fn validate(&self) -> Result<(), BackoffError> {
        if self.base_delay.is_zero() {
            return Err(BackoffError::InvalidConfig(
                "base_delay must be > 0".to_string(),
            ));
        }
        if self.max_delay < self.base_delay {
            return Err(BackoffError::InvalidConfig(
                "max_delay must be >= base_delay".to_string(),
            ));
        }
        if self.max_retries == 0 {
            return Err(BackoffError::InvalidConfig(
                "max_retries must be > 0".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&self.jitter_percent) {
            return Err(BackoffError::InvalidConfig(
                "jitter_percent must be between 0.0 and 1.0".to_string(),
            ));
        }
        Ok(())
    }
}

/// Errors for backoff operations.
#[derive(Debug, Error)]
pub enum BackoffError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Maximum retries ({max}) exceeded")]
    MaxRetriesExceeded { max: u32 },
}

/// Exponential backoff calculator.
#[derive(Debug, Clone)]
pub struct Backoff {
    config: BackoffConfig,
    attempt: u32,
}

impl Backoff {
    /// Create new backoff with base and max delay.
    pub fn new(base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            config: BackoffConfig {
                base_delay,
                max_delay,
                ..BackoffConfig::default()
            },
            attempt: 0,
        }
    }

    /// Create with full configuration.
    pub fn with_config(config: BackoffConfig) -> Result<Self, BackoffError> {
        config.validate()?;
        Ok(Self { config, attempt: 0 })
    }

    /// Builder for custom backoff configuration.
    pub fn builder() -> BackoffBuilder {
        BackoffBuilder {
            config: BackoffConfig::default(),
        }
    }

    /// Calculate delay for given attempt number (1-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay = self.base_delay_for_attempt(attempt);
        if self.config.jitter_enabled {
            return self.apply_jitter_with_thread_rng(delay);
        }
        delay
    }

    /// Calculate delay for given attempt number using provided RNG.
    pub fn delay_for_attempt_with_rng<R: Rng + ?Sized>(
        &self,
        attempt: u32,
        rng: &mut R,
    ) -> Duration {
        let delay = self.base_delay_for_attempt(attempt);
        if self.config.jitter_enabled {
            return self.apply_jitter_with_rng(delay, rng);
        }
        delay
    }

    /// Return next delay using internal attempt counter.
    pub fn next_delay(&mut self) -> Result<Duration, BackoffError> {
        let next_attempt = self.attempt.saturating_add(1).max(1);
        self.check_retry_limit(next_attempt)?;
        let delay = self.delay_for_attempt(next_attempt);
        self.attempt = next_attempt;
        Ok(delay)
    }

    /// Reset attempt counter.
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Get current attempt number.
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Check if attempt exceeds max retries.
    pub fn check_retry_limit(&self, attempt: u32) -> Result<(), BackoffError> {
        if attempt > self.config.max_retries {
            return Err(BackoffError::MaxRetriesExceeded {
                max: self.config.max_retries,
            });
        }
        Ok(())
    }

    fn base_delay_for_attempt(&self, attempt: u32) -> Duration {
        let attempt = attempt.max(1);
        let exponent = attempt.saturating_sub(1).min(31);
        let multiplier = 2u128.saturating_pow(exponent);
        let base_millis = self.config.base_delay.as_millis();
        let delay_millis = base_millis.saturating_mul(multiplier);
        let max_millis = self.config.max_delay.as_millis();
        let capped = delay_millis.min(max_millis).min(u128::from(u64::MAX));

        let delay = Duration::from_millis(capped as u64);

        debug!(
            attempt = attempt,
            delay_secs = delay.as_secs_f64(),
            "Calculated backoff delay"
        );

        delay
    }

    fn apply_jitter_with_thread_rng(&self, delay: Duration) -> Duration {
        let mut rng = rand::thread_rng();
        self.apply_jitter_with_rng(delay, &mut rng)
    }

    fn apply_jitter_with_rng<R: Rng + ?Sized>(&self, delay: Duration, rng: &mut R) -> Duration {
        let jitter_range = self.config.jitter_percent;
        let factor = 1.0 + rng.gen_range(-jitter_range..jitter_range);
        let millis = delay.as_millis() as f64 * factor;
        Duration::from_millis(millis.max(0.0) as u64)
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new(Duration::from_secs(30), Duration::from_secs(300))
    }
}

/// Builder for Backoff configuration.
#[derive(Debug, Clone)]
pub struct BackoffBuilder {
    config: BackoffConfig,
}

impl BackoffBuilder {
    pub fn base_delay(mut self, delay: Duration) -> Self {
        self.config.base_delay = delay;
        self
    }

    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.config.max_delay = delay;
        self
    }

    pub fn max_retries(mut self, retries: u32) -> Self {
        self.config.max_retries = retries;
        self
    }

    pub fn jitter_enabled(mut self, enabled: bool) -> Self {
        self.config.jitter_enabled = enabled;
        self
    }

    pub fn jitter_percent(mut self, percent: f64) -> Self {
        self.config.jitter_percent = percent;
        self
    }

    pub fn build(self) -> Result<Backoff, BackoffError> {
        Backoff::with_config(self.config)
    }
}

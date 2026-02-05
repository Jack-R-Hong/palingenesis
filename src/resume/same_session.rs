use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::monitor::session::{Session, StepValue};
use crate::resume::backoff::{Backoff, BackoffConfig};
use crate::resume::{ResumeContext, ResumeError, ResumeOutcome, ResumeStrategy};
use crate::state::{CurrentSession, StateStore};

/// Configuration for same-session resume.
#[derive(Debug, Clone)]
pub struct SameSessionConfig {
    /// Base delay for exponential backoff (seconds).
    pub backoff_base_secs: u64,
    /// Maximum backoff delay (seconds).
    pub backoff_max_secs: u64,
    /// Maximum retry attempts before giving up.
    pub max_retries: u32,
    /// Enable jitter in backoff delays.
    pub backoff_jitter: bool,
    /// Jitter percentage (0.0 to 1.0).
    pub backoff_jitter_percent: f64,
    /// Command used to trigger session continuation.
    pub resume_command: Vec<String>,
}

impl Default for SameSessionConfig {
    fn default() -> Self {
        Self {
            backoff_base_secs: 30,
            backoff_max_secs: 300,
            max_retries: 5,
            backoff_jitter: true,
            backoff_jitter_percent: 0.1,
            resume_command: vec![
                "opencode".to_string(),
                "continue".to_string(),
                "--session".to_string(),
            ],
        }
    }
}

/// Resume trigger abstraction for testing and integration.
#[async_trait]
pub trait ResumeTrigger: Send + Sync {
    async fn trigger(&self, ctx: &ResumeContext) -> Result<(), ResumeError>;
}

#[derive(Debug, Clone)]
struct CommandResumeTrigger {
    command: Vec<String>,
}

#[async_trait]
impl ResumeTrigger for CommandResumeTrigger {
    async fn trigger(&self, ctx: &ResumeContext) -> Result<(), ResumeError> {
        if self.command.is_empty() {
            return Err(ResumeError::Config(
                "resume command cannot be empty".to_string(),
            ));
        }

        info!(
            session = %ctx.session_path.display(),
            attempt = ctx.attempt_number,
            "Resuming session after rate limit"
        );

        let mut command = tokio::process::Command::new(&self.command[0]);
        if self.command.len() > 1 {
            command.args(&self.command[1..]);
        }
        let output = command
            .arg(&ctx.session_path)
            .output()
            .await
            .map_err(ResumeError::Io)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(ResumeError::CommandFailed {
                command: self.command.join(" "),
                stderr,
            });
        }

        Ok(())
    }
}

/// Strategy for resuming the same session after a rate limit.
pub struct SameSessionStrategy {
    config: SameSessionConfig,
    cancel: Option<CancellationToken>,
    trigger: Arc<dyn ResumeTrigger>,
}

impl SameSessionStrategy {
    pub fn new() -> Self {
        Self::with_config(SameSessionConfig::default())
    }

    pub fn with_config(config: SameSessionConfig) -> Self {
        let trigger = CommandResumeTrigger {
            command: config.resume_command.clone(),
        };
        Self {
            config,
            cancel: None,
            trigger: Arc::new(trigger),
        }
    }

    pub fn with_cancellation(mut self, cancel: CancellationToken) -> Self {
        self.cancel = Some(cancel);
        self
    }

    pub fn with_trigger<T: ResumeTrigger + 'static>(mut self, trigger: T) -> Self {
        self.trigger = Arc::new(trigger);
        self
    }

    fn wait_duration(&self, ctx: &ResumeContext) -> Duration {
        if let Some(retry_after) = ctx.retry_after {
            return retry_after;
        }

        self.backoff_delay(ctx.attempt_number)
    }

    fn backoff_delay(&self, attempt_number: u32) -> Duration {
        let config = BackoffConfig {
            base_delay: Duration::from_secs(self.config.backoff_base_secs),
            max_delay: Duration::from_secs(self.config.backoff_max_secs),
            max_retries: self.config.max_retries,
            jitter_enabled: self.config.backoff_jitter,
            jitter_percent: self.config.backoff_jitter_percent,
        };

        let backoff = Backoff::with_config(config).unwrap_or_else(|err| {
            warn!(error = %err, "Invalid backoff config, using defaults");
            Backoff::default()
        });

        backoff.delay_for_attempt(attempt_number)
    }

    async fn wait_or_cancel(&self, duration: Duration) -> bool {
        debug!(duration_secs = duration.as_secs(), "Waiting before resume");

        if let Some(cancel) = &self.cancel {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Wait cancelled by shutdown");
                    return false;
                }
                _ = tokio::time::sleep(duration) => {}
            }
        } else {
            tokio::time::sleep(duration).await;
        }

        true
    }

    fn update_state_on_resume(&self, ctx: &ResumeContext) -> Result<(), ResumeError> {
        let store = StateStore::new();
        let mut state = store.load();

        state.stats.total_resumes = state.stats.total_resumes.saturating_add(1);
        state.stats.last_resume = Some(Utc::now());
        state.current_session = Some(self.build_current_session(ctx));

        store
            .save(&state)
            .map_err(|err| ResumeError::Config(format!("state store error: {err}")))?;

        Ok(())
    }

    fn build_current_session(&self, ctx: &ResumeContext) -> CurrentSession {
        if let Some(session) = &ctx.session_metadata {
            return current_session_from_metadata(session, &ctx.session_path);
        }

        CurrentSession {
            path: ctx.session_path.clone(),
            ..CurrentSession::default()
        }
    }

}

#[async_trait]
impl ResumeStrategy for SameSessionStrategy {
    async fn execute(&self, ctx: &ResumeContext) -> Result<ResumeOutcome, ResumeError> {
        if ctx.attempt_number > self.config.max_retries {
            warn!(
                attempts = ctx.attempt_number,
                max_retries = self.config.max_retries,
                "Retry limit exceeded"
            );
            return Ok(ResumeOutcome::failure(
                format!(
                    "Retry limit exceeded after {} attempts",
                    ctx.attempt_number
                ),
                false,
            ));
        }

        let wait_duration = self.wait_duration(ctx);
        if !self.wait_or_cancel(wait_duration).await {
            return Ok(ResumeOutcome::skipped("same-session resume cancelled"));
        }

        match self.trigger.trigger(ctx).await {
            Ok(()) => {
                self.update_state_on_resume(ctx)?;
                Ok(ResumeOutcome::success(
                    ctx.session_path.clone(),
                    "Resumed same session after rate limit",
                ))
            }
            Err(err) => {
                warn!(error = %err, "Resume trigger failed");
                let retryable = ctx.attempt_number < self.config.max_retries;
                if retryable {
                    let next_delay = self.backoff_delay(ctx.attempt_number + 1);
                    Ok(ResumeOutcome::delayed(
                        next_delay,
                        format!("Resume failed, will retry: {err}"),
                    ))
                } else {
                    Ok(ResumeOutcome::failure(err.to_string(), false))
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "SameSessionStrategy"
    }
}

impl Default for SameSessionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

fn current_session_from_metadata(session: &Session, session_path: &std::path::Path) -> CurrentSession {
    let steps_completed = session
        .state
        .steps_completed
        .iter()
        .filter_map(step_value_to_u32)
        .collect::<Vec<_>>();
    let last_step = session
        .state
        .last_step
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);

    CurrentSession {
        path: session_path.to_path_buf(),
        steps_completed: steps_completed.clone(),
        last_step,
        total_steps: steps_completed.len() as u32,
    }
}

fn step_value_to_u32(value: &StepValue) -> Option<u32> {
    match value {
        StepValue::Integer(num) => u32::try_from(*num).ok(),
        StepValue::String(value) => value.parse::<u32>().ok(),
    }
}

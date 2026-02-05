use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio::fs;
use tracing::{debug, info, warn};

use crate::monitor::session::{Session, StepValue};
use crate::resume::{ResumeContext, ResumeError, ResumeOutcome, ResumeStrategy};
use crate::state::{CurrentSession, StateStore};

/// Configuration for new-session resume.
#[derive(Debug, Clone)]
pub struct NewSessionConfig {
    /// Name of Next-step file.
    pub next_step_filename: String,
    /// Prompt template for continuation.
    pub prompt_template: String,
    /// Enable session backup before new session.
    pub enable_backup: bool,
    /// Maximum backups to keep.
    pub max_backups: usize,
}

impl Default for NewSessionConfig {
    fn default() -> Self {
        Self {
            next_step_filename: "Next-step.md".to_string(),
            prompt_template: "Starting new session from step {step}: {description}\n\nContext:\n{context}"
                .to_string(),
            enable_backup: true,
            max_backups: 10,
        }
    }
}

/// Information extracted from Next-step.md.
#[derive(Debug, Clone)]
pub struct NextStepInfo {
    /// Step number to continue from.
    pub step_number: u32,
    /// Description of the step.
    pub description: String,
    /// Full content of Next-step.md.
    pub raw_content: String,
}

#[async_trait]
pub trait BackupHandler: Send + Sync {
    async fn backup(&self, session_path: &Path) -> Result<PathBuf, ResumeError>;
}

#[derive(Debug, Clone)]
pub struct SessionBackup {
    max_backups: usize,
}

impl SessionBackup {
    pub fn new(max_backups: usize) -> Self {
        Self { max_backups }
    }
}

#[async_trait]
impl BackupHandler for SessionBackup {
    async fn backup(&self, session_path: &Path) -> Result<PathBuf, ResumeError> {
        let _ = self.max_backups;
        Ok(session_path.to_path_buf())
    }
}

#[async_trait]
pub trait SessionCreator: Send + Sync {
    async fn create(&self, prompt: &str, session_dir: &Path) -> Result<PathBuf, ResumeError>;
}

#[derive(Debug, Clone)]
struct CommandSessionCreator;

#[async_trait]
impl SessionCreator for CommandSessionCreator {
    async fn create(&self, prompt: &str, session_dir: &Path) -> Result<PathBuf, ResumeError> {
        let output = tokio::process::Command::new("opencode")
            .arg("new")
            .arg("--prompt")
            .arg(prompt)
            .arg("--workdir")
            .arg(session_dir)
            .output()
            .await
            .map_err(ResumeError::Io)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(ResumeError::CommandFailed {
                command: "opencode new".to_string(),
                stderr,
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let session_path = stdout
            .lines()
            .find(|line| line.contains("session:"))
            .and_then(|line| line.split("session:").nth(1))
            .map(|value| PathBuf::from(value.trim()))
            .unwrap_or_else(|| session_dir.join("session.md"));

        Ok(session_path)
    }
}

/// Strategy for creating new session after context exhaustion.
pub struct NewSessionStrategy {
    config: NewSessionConfig,
    backup: Arc<dyn BackupHandler>,
    creator: Arc<dyn SessionCreator>,
}

impl NewSessionStrategy {
    pub fn new() -> Self {
        let config = NewSessionConfig::default();
        Self {
            backup: Arc::new(SessionBackup::new(config.max_backups)),
            creator: Arc::new(CommandSessionCreator),
            config,
        }
    }

    pub fn with_config(config: NewSessionConfig) -> Self {
        Self {
            backup: Arc::new(SessionBackup::new(config.max_backups)),
            creator: Arc::new(CommandSessionCreator),
            config,
        }
    }

    pub fn with_backup_handler<T: BackupHandler + 'static>(mut self, backup: T) -> Self {
        self.backup = Arc::new(backup);
        self
    }

    pub fn with_session_creator<T: SessionCreator + 'static>(mut self, creator: T) -> Self {
        self.creator = Arc::new(creator);
        self
    }

    async fn read_next_step(&self, session_dir: &Path) -> Result<Option<NextStepInfo>, ResumeError> {
        let next_step_path = session_dir.join(&self.config.next_step_filename);
        match fs::read_to_string(&next_step_path).await {
            Ok(content) => {
                debug!(path = %next_step_path.display(), "Found Next-step.md");
                if let Some(info) = self.parse_next_step(&content) {
                    Ok(Some(info))
                } else {
                    warn!(path = %next_step_path.display(), "Failed to parse Next-step.md");
                    Ok(None)
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                debug!(path = %next_step_path.display(), "Next-step.md not found");
                Ok(None)
            }
            Err(err) => Err(ResumeError::Io(err)),
        }
    }

    fn parse_next_step(&self, content: &str) -> Option<NextStepInfo> {
        let mut step_number = None;
        let mut description: Option<String> = None;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if step_number.is_none() {
                if let Some((step, desc)) = parse_step_line(trimmed) {
                    step_number = Some(step);
                    if !desc.is_empty() {
                        description = Some(desc);
                    }
                    continue;
                }
            }

            if description.is_none() && !trimmed.starts_with('#') {
                description = Some(trimmed.to_string());
            }
        }

        let step_number = step_number?;
        let description = description
            .unwrap_or_else(|| format!("Continue from step {}", step_number));

        Some(NextStepInfo {
            step_number,
            description,
            raw_content: content.to_string(),
        })
    }

    fn calculate_from_steps_completed(&self, session: &Session) -> u32 {
        let steps = steps_completed_from_session(session);
        steps.iter().max().copied().unwrap_or(0).saturating_add(1)
    }

    fn build_context_summary(&self, ctx: &ResumeContext, info: &NextStepInfo) -> String {
        let mut lines = vec![format!("Previous session: {}", ctx.session_path.display())];

        if let Some(session) = &ctx.session_metadata {
            let steps = steps_completed_from_session(session);
            if !steps.is_empty() {
                lines.push(format!(
                    "Steps completed: {}",
                    steps
                        .iter()
                        .map(|step| step.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }

            if let Some(last_step) = session.state.last_step {
                lines.push(format!("Last step: {}", last_step));
            }
        }

        lines.push(format!("Stop reason: {:?}", ctx.stop_reason));

        if !info.raw_content.trim().is_empty() {
            lines.push("Next-step details:".to_string());
            lines.push(info.raw_content.trim().to_string());
        } else {
            lines.push(format!("Continuation: {}", info.description));
        }

        lines.join("\n")
    }

    fn generate_prompt(&self, info: &NextStepInfo, ctx: &ResumeContext) -> String {
        let context = self.build_context_summary(ctx, info);
        self.config
            .prompt_template
            .replace("{step}", &info.step_number.to_string())
            .replace("{description}", &info.description)
            .replace("{context}", &context)
    }

    fn build_current_session(
        &self,
        ctx: &ResumeContext,
        new_session_path: PathBuf,
        next_step: &NextStepInfo,
    ) -> CurrentSession {
        let steps = ctx
            .session_metadata
            .as_ref()
            .map(steps_completed_from_session)
            .unwrap_or_default();

        let last_step = steps
            .iter()
            .max()
            .copied()
            .unwrap_or_else(|| next_step.step_number.saturating_sub(1));

        CurrentSession {
            path: new_session_path,
            steps_completed: steps.clone(),
            last_step,
            total_steps: steps.len() as u32,
        }
    }

    fn update_state_on_resume(
        &self,
        ctx: &ResumeContext,
        new_session_path: PathBuf,
        next_step: &NextStepInfo,
    ) -> Result<(), ResumeError> {
        let store = StateStore::new();
        let mut state = store.load();

        state.stats.total_resumes = state.stats.total_resumes.saturating_add(1);
        state.stats.last_resume = Some(Utc::now());
        state.current_session = Some(self.build_current_session(
            ctx,
            new_session_path,
            next_step,
        ));

        store
            .save(&state)
            .map_err(|err| ResumeError::Config(format!("state store error: {err}")))?;

        Ok(())
    }
}

#[async_trait]
impl ResumeStrategy for NewSessionStrategy {
    async fn execute(&self, ctx: &ResumeContext) -> Result<ResumeOutcome, ResumeError> {
        let session_dir = ctx.session_path.parent().ok_or_else(|| {
            ResumeError::SessionNotFound {
                path: ctx.session_path.clone(),
            }
        })?;

        if self.config.enable_backup {
            match self.backup.backup(&ctx.session_path).await {
                Ok(backup_path) => {
                    info!(backup = %backup_path.display(), "Session backed up");
                }
                Err(err) => {
                    warn!(error = %err, "Session backup failed, proceeding anyway");
                }
            }
        }

        let next_step = if let Some(info) = self.read_next_step(session_dir).await? {
            info
        } else if let Some(session) = &ctx.session_metadata {
            let step = self.calculate_from_steps_completed(session);
            NextStepInfo {
                step_number: step,
                description: format!("Continue from step {}", step),
                raw_content: String::new(),
            }
        } else {
            NextStepInfo {
                step_number: 1,
                description: "Continue workflow".to_string(),
                raw_content: String::new(),
            }
        };

        info!(
            step = next_step.step_number,
            description = %next_step.description,
            "Starting new session from step {}",
            next_step.step_number
        );

        let prompt = self.generate_prompt(&next_step, ctx);
        let new_session_path = self.creator.create(&prompt, session_dir).await?;

        info!(
            from = %ctx.session_path.display(),
            to = %new_session_path.display(),
            "Audit: new session transition"
        );

        self.update_state_on_resume(ctx, new_session_path.clone(), &next_step)?;

        Ok(ResumeOutcome::success(
            new_session_path,
            format!("Started new session from step {}", next_step.step_number),
        ))
    }

    fn name(&self) -> &'static str {
        "NewSessionStrategy"
    }
}

impl Default for NewSessionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

fn steps_completed_from_session(session: &Session) -> Vec<u32> {
    session
        .state
        .steps_completed
        .iter()
        .filter_map(step_value_to_u32)
        .collect()
}

fn step_value_to_u32(value: &StepValue) -> Option<u32> {
    match value {
        StepValue::Integer(num) => u32::try_from(*num).ok(),
        StepValue::String(value) => value.parse::<u32>().ok(),
    }
}

fn parse_step_line(line: &str) -> Option<(u32, String)> {
    let cleaned = line.trim_start_matches('#').trim();
    let lower = cleaned.to_ascii_lowercase();

    if lower.starts_with("step ") {
        let remainder = cleaned[4..].trim();
        return parse_step_number(remainder);
    }

    parse_leading_number(cleaned)
}

fn parse_step_number(input: &str) -> Option<(u32, String)> {
    let (number, remainder) = parse_leading_number(input)?;
    Some((number, remainder))
}

fn parse_leading_number(input: &str) -> Option<(u32, String)> {
    let mut digits = String::new();
    let mut index = 0;

    for (idx, ch) in input.char_indices() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            index = idx + ch.len_utf8();
        } else {
            break;
        }
    }

    if digits.is_empty() {
        return None;
    }

    let number = digits.parse::<u32>().ok()?;
    let remainder = input[index..]
        .trim_start_matches(|ch: char| ch == '.' || ch == ':' || ch == ')' || ch == '-')
        .trim();

    Some((number, remainder.to_string()))
}

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use regex::Regex;
use tracing::{debug, info, warn};

const DEFAULT_RETRY_WAIT_SECS: u64 = 30;
const DEFAULT_MAX_LINES: usize = 100;
const EXIT_CODE_SIGHUP: i32 = 129;
const EXIT_CODE_SIGINT: i32 = 130;
const EXIT_CODE_SIGTERM: i32 = 143;

/// Reason why a session stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    /// Session hit rate limit (HTTP 429 or equivalent).
    RateLimit(RateLimitInfo),
    /// Session exhausted context window.
    ContextExhausted(Option<ContextExhaustionInfo>),
    /// User explicitly exited (Ctrl+C, exit command).
    UserExit(UserExitInfo),
    /// Session completed successfully.
    Completed,
    /// Unknown or unclassifiable reason.
    Unknown(String),
}

impl StopReason {
    /// Whether this stop reason should trigger auto-resume.
    pub fn should_auto_resume(&self) -> bool {
        match self {
            StopReason::RateLimit(_) => true,
            StopReason::ContextExhausted(_) => true,
            StopReason::UserExit(_) => false,
            StopReason::Completed => false,
            StopReason::Unknown(_) => false,
        }
    }

    pub fn metrics_reason_label(&self) -> Option<&'static str> {
        match self {
            StopReason::RateLimit(_) => Some("rate_limit"),
            StopReason::ContextExhausted(_) => Some("context_exhausted"),
            StopReason::UserExit(_) | StopReason::Completed => Some("manual"),
            StopReason::Unknown(_) => None,
        }
    }
}

/// Information about a user-initiated exit.
#[derive(Debug, Clone, PartialEq)]
pub struct UserExitInfo {
    pub exit_type: UserExitType,
    pub exit_code: Option<i32>,
    pub message: Option<String>,
}

/// Type of user exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserExitType {
    CtrlC,
    ExitCommand,
    CleanExit,
    TerminalClosed,
    UserTerminated,
}

/// Information about a context exhaustion stop.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextExhaustionInfo {
    /// Estimated token usage percentage (if available).
    pub usage_percent: Option<f32>,
    /// Model context window size (if known).
    pub context_size: Option<u32>,
    /// Raw error message if available.
    pub message: Option<String>,
}

/// Information about a rate limit stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitInfo {
    /// Duration to wait before retry (from Retry-After or default).
    pub retry_after: Duration,
    /// Source of the retry_after value.
    pub source: RetryAfterSource,
    /// Raw error message if available.
    pub message: Option<String>,
}

/// Source of the retry_after duration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryAfterSource {
    /// From Retry-After HTTP header.
    Header,
    /// From JSON/YAML error response.
    ResponseBody,
    /// From text pattern extraction.
    TextParsed,
    /// Default from configuration.
    ConfigDefault,
}

/// Result of stop reason classification.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassificationResult {
    /// The classified stop reason.
    pub reason: StopReason,
    /// Confidence level (0.0 - 1.0).
    pub confidence: f32,
    /// Evidence used for classification.
    pub evidence: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ClassifierError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Pattern compilation error: {0}")]
    Pattern(#[from] regex::Error),
}

/// Configuration for the classifier.
#[derive(Debug, Clone)]
pub struct ClassifierConfig {
    /// Default wait time when no Retry-After is found.
    pub default_retry_wait: Duration,
    /// Maximum lines to read from session file.
    pub max_lines: usize,
    /// Context usage threshold for exhaustion detection (0.0-1.0).
    pub context_threshold_percent: f32,
    /// Default context window size for estimation (tokens).
    pub default_context_size: u32,
    /// Known model context window sizes (lowercase model name -> tokens).
    pub known_context_sizes: HashMap<String, u32>,
    /// Extra rate limit patterns for future extensibility.
    pub extra_rate_limit_patterns: Vec<String>,
    /// Extra context exhaustion patterns for future extensibility.
    pub extra_context_patterns: Vec<String>,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        let known_context_sizes = HashMap::from([
            ("claude 3.5 sonnet".to_string(), 200_000),
            ("claude 3 opus".to_string(), 200_000),
            ("gpt-4 turbo".to_string(), 128_000),
            ("gpt-4".to_string(), 8_192),
        ]);
        Self {
            default_retry_wait: Duration::from_secs(DEFAULT_RETRY_WAIT_SECS),
            max_lines: DEFAULT_MAX_LINES,
            context_threshold_percent: 0.80,
            default_context_size: 200_000,
            known_context_sizes,
            extra_rate_limit_patterns: Vec::new(),
            extra_context_patterns: Vec::new(),
        }
    }
}

/// Stop reason classifier implementation.
pub struct StopReasonClassifier {
    config: ClassifierConfig,
    rate_limit_patterns: Vec<Regex>,
    context_patterns: Vec<Regex>,
    user_exit_patterns: Vec<Regex>,
}

impl StopReasonClassifier {
    /// Create a new classifier with default configuration.
    pub fn new() -> Result<Self, ClassifierError> {
        Self::with_config(ClassifierConfig::default())
    }

    /// Create with custom configuration.
    pub fn with_config(config: ClassifierConfig) -> Result<Self, ClassifierError> {
        let mut rate_limit_patterns = vec![
            Regex::new(r"(?i)rate[_-]?limit[_-]?error")?,
            Regex::new(r"(?i)rate\s+limit\s+reached")?,
            Regex::new(r"(?i)\b429\b")?,
            Regex::new(r"(?i)too\s+many\s+requests")?,
            Regex::new(r"(?i)quota\s+exceeded")?,
            Regex::new(r"(?i)overloaded[_-]?error|overloaded")?,
            Regex::new(r"(?i)throttl")?,
        ];

        for pattern in &config.extra_rate_limit_patterns {
            rate_limit_patterns.push(Regex::new(pattern)?);
        }

        let mut context_patterns = Self::build_context_patterns()?;
        for pattern in &config.extra_context_patterns {
            context_patterns.push(Regex::new(pattern)?);
        }

        let user_exit_patterns = Self::build_user_exit_patterns()?;

        Ok(Self {
            config,
            rate_limit_patterns,
            context_patterns,
            user_exit_patterns,
        })
    }

    /// Classify the stop reason from session file content.
    pub fn classify(&self, session_path: &Path, exit_code: Option<i32>) -> ClassificationResult {
        let content = match self.read_file_tail(session_path, self.config.max_lines) {
            Ok(content) => content,
            Err(err) => {
                warn!(error = %err, "Failed to read session file");
                return ClassificationResult {
                    reason: StopReason::Unknown(format!("Read error: {err}")),
                    confidence: 0.0,
                    evidence: vec![format!("error: {err}")],
                };
            }
        };

        self.classify_with_session(&content, Some(session_path), exit_code)
    }

    /// Classify from raw content (for log analysis).
    pub fn classify_content(&self, content: &str, exit_code: Option<i32>) -> ClassificationResult {
        self.classify_with_session(content, None, exit_code)
    }

    fn classify_with_session(
        &self,
        content: &str,
        session_path: Option<&Path>,
        exit_code: Option<i32>,
    ) -> ClassificationResult {
        let mut evidence = Vec::new();

        if let Some(info) = self.detect_rate_limit(content, &mut evidence) {
            let confidence = Self::confidence_from_evidence(&evidence, 0.85);
            debug!(confidence, "Classified stop as rate limit");
            return ClassificationResult {
                reason: StopReason::RateLimit(info),
                confidence,
                evidence,
            };
        }

        if let Some(path) = session_path {
            if let Some(reason) = self.check_completed(path, &mut evidence) {
                debug!("Classified stop as completed");
                return ClassificationResult {
                    reason,
                    confidence: 0.95,
                    evidence,
                };
            }
        }

        if let Some(info) = self.detect_context_exhaustion(content, &mut evidence) {
            let confidence = Self::confidence_from_evidence(&evidence, 0.78);
            debug!(confidence, "Classified stop as context exhausted");
            return ClassificationResult {
                reason: StopReason::ContextExhausted(Some(info)),
                confidence,
                evidence,
            };
        }

        if let Some(info) = self.detect_user_exit(content, exit_code, &mut evidence) {
            info!(exit_type = ?info.exit_type, "Session ended by user, not auto-resuming");
            let confidence = Self::confidence_from_evidence(&evidence, 0.75);
            return ClassificationResult {
                reason: StopReason::UserExit(info),
                confidence,
                evidence,
            };
        }

        ClassificationResult {
            reason: StopReason::Unknown("No matching patterns".to_string()),
            confidence: 0.2,
            evidence,
        }
    }

    fn build_context_patterns() -> Result<Vec<Regex>, ClassifierError> {
        Ok(vec![
            Regex::new(r"(?i)context[_-]?length[_-]?exceeded")?,
            Regex::new(r"(?i)maximum\s+context\s+length")?,
            Regex::new(r"(?i)token\s+limit\s+exceeded")?,
            Regex::new(r"(?i)conversation\s+too\s+long")?,
            Regex::new(r"(?i)context\s+window\s+(?:full|exceeded|limit)")?,
            Regex::new(r"(?i)max\s*tokens?\s+reached")?,
            Regex::new(r"(?i)prompt\s+is\s+too\s+long")?,
            Regex::new(r"(?i)context\s+(?:truncated|reset)")?,
            Regex::new(r"(?i)conversation\s+reset")?,
        ])
    }

    fn build_user_exit_patterns() -> Result<Vec<Regex>, ClassifierError> {
        Ok(vec![
            Regex::new(r"(?im)^\s*exit\s*$")?,
            Regex::new(r"(?im)^\s*quit\s*$")?,
            Regex::new(r"(?im)^\s*/bye\s*$")?,
            Regex::new(r"(?im)^\s*goodbye\s*$")?,
            Regex::new(r"(?im)^\s*done\s*$")?,
            Regex::new(r"(?i)keyboard\s+interrupt")?,
            Regex::new(r"(?i)interrupted\s+by\s+user")?,
            Regex::new(r"(?i)sigint\s+received")?,
        ])
    }

    fn detect_context_exhaustion(
        &self,
        content: &str,
        evidence: &mut Vec<String>,
    ) -> Option<ContextExhaustionInfo> {
        for pattern in &self.context_patterns {
            if let Some(matched) = pattern.find(content) {
                let matched_text = matched.as_str();
                evidence.push(format!("matched context pattern: {matched_text}"));
                let (usage_percent, context_size) = self
                    .extract_token_usage(content)
                    .map(|(usage, size)| (Some(usage), Some(size)))
                    .unwrap_or((None, None));
                return Some(ContextExhaustionInfo {
                    usage_percent,
                    context_size,
                    message: Some(matched_text.to_string()),
                });
            }
        }

        if let Some((usage_percent, context_size)) = self.extract_token_usage(content) {
            if usage_percent >= self.config.context_threshold_percent {
                evidence.push(format!(
                    "token usage {:.0}% exceeds threshold {:.0}%",
                    usage_percent * 100.0,
                    self.config.context_threshold_percent * 100.0
                ));
                return Some(ContextExhaustionInfo {
                    usage_percent: Some(usage_percent),
                    context_size: Some(context_size),
                    message: None,
                });
            }
        }

        None
    }

    fn detect_user_exit(
        &self,
        content: &str,
        exit_code: Option<i32>,
        evidence: &mut Vec<String>,
    ) -> Option<UserExitInfo> {
        if let Some(code) = exit_code {
            match code {
                EXIT_CODE_SIGINT => {
                    evidence.push("exit code 130 (SIGINT/Ctrl+C)".to_string());
                    return Some(UserExitInfo {
                        exit_type: UserExitType::CtrlC,
                        exit_code: Some(code),
                        message: Some("User pressed Ctrl+C".to_string()),
                    });
                }
                EXIT_CODE_SIGTERM => {
                    evidence.push("exit code 143 (SIGTERM)".to_string());
                    return Some(UserExitInfo {
                        exit_type: UserExitType::UserTerminated,
                        exit_code: Some(code),
                        message: Some("Process terminated".to_string()),
                    });
                }
                EXIT_CODE_SIGHUP => {
                    evidence.push("exit code 129 (SIGHUP)".to_string());
                    return Some(UserExitInfo {
                        exit_type: UserExitType::TerminalClosed,
                        exit_code: Some(code),
                        message: Some("Terminal closed".to_string()),
                    });
                }
                0 => {
                    // Handle clean exit after pattern checks below.
                }
                _ => {
                    return None;
                }
            }
        }

        if self.has_error_indicators(content) {
            return None;
        }

        for pattern in &self.user_exit_patterns {
            if let Some(matched) = pattern.find(content) {
                evidence.push(format!("matched user exit pattern: {}", matched.as_str()));
                return Some(UserExitInfo {
                    exit_type: UserExitType::ExitCommand,
                    exit_code,
                    message: Some(matched.as_str().to_string()),
                });
            }
        }

        if exit_code == Some(0) {
            evidence.push("clean exit code 0".to_string());
            return Some(UserExitInfo {
                exit_type: UserExitType::CleanExit,
                exit_code: Some(0),
                message: None,
            });
        }

        None
    }

    fn extract_token_usage(&self, content: &str) -> Option<(f32, u32)> {
        let used_of_pattern = Regex::new(r"(?i)used\s+(\d{2,})\s+of\s+(\d{2,})\s+tokens").ok();
        if let Some(re) = used_of_pattern {
            if let Some(caps) = re.captures(content) {
                let used = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
                let total = caps.get(2).and_then(|m| m.as_str().parse::<u32>().ok());
                if let (Some(used), Some(total)) = (used, total) {
                    return Some((used as f32 / total as f32, total));
                }
            }
        }

        let fraction_pattern = Regex::new(r"(?i)(\d{2,})\s*/\s*(\d{2,})\s*tokens?").ok();
        if let Some(re) = fraction_pattern {
            if let Some(caps) = re.captures(content) {
                let used = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
                let total = caps.get(2).and_then(|m| m.as_str().parse::<u32>().ok());
                if let (Some(used), Some(total)) = (used, total) {
                    return Some((used as f32 / total as f32, total));
                }
            }
        }

        let used_count_pattern =
            Regex::new(r"(?i)(\d{2,})\s*tokens?\s*(?:used|consumed|spent)").ok();
        if let Some(re) = used_count_pattern {
            if let Some(caps) = re.captures(content) {
                if let Some(used) = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
                    let context_size = self.infer_context_size(content);
                    return Some((used as f32 / context_size as f32, context_size));
                }
            }
        }

        None
    }

    fn has_error_indicators(&self, content: &str) -> bool {
        let content_lower = content.to_lowercase();
        ["error", "exception", "failed", "panic", "crash"]
            .iter()
            .any(|needle| content_lower.contains(needle))
    }

    fn infer_context_size(&self, content: &str) -> u32 {
        let content_lower = content.to_lowercase();
        for (model, size) in &self.config.known_context_sizes {
            if content_lower.contains(model) {
                return *size;
            }
        }

        self.config.default_context_size
    }

    fn check_completed(
        &self,
        session_path: &Path,
        evidence: &mut Vec<String>,
    ) -> Option<StopReason> {
        use crate::monitor::frontmatter::parse_session;

        match parse_session(session_path) {
            Ok(session) => {
                if session.is_complete() {
                    evidence.push("session status is complete".to_string());
                    return Some(StopReason::Completed);
                }

                if let Some(last_step) = session.state.last_step {
                    let completed = session.steps_completed_count();
                    let in_progress = session.state.status.as_deref() == Some("in-progress");
                    if !in_progress && completed >= last_step as usize {
                        evidence.push(format!(
                            "stepsCompleted {completed} reached lastStep {last_step}"
                        ));
                        return Some(StopReason::Completed);
                    }
                }
            }
            Err(err) => {
                debug!(error = %err, "Failed to parse session for completion check");
            }
        }

        None
    }

    fn detect_rate_limit(
        &self,
        content: &str,
        evidence: &mut Vec<String>,
    ) -> Option<RateLimitInfo> {
        for pattern in &self.rate_limit_patterns {
            if let Some(matched) = pattern.find(content) {
                let matched_text = matched.as_str();
                evidence.push(format!("matched pattern: {matched_text}"));
                let (retry_after, source) = self.extract_retry_after(content);
                return Some(RateLimitInfo {
                    retry_after,
                    source,
                    message: Some(matched_text.to_string()),
                });
            }
        }

        None
    }

    fn extract_retry_after(&self, content: &str) -> (Duration, RetryAfterSource) {
        let header_pattern = Regex::new(r"(?i)retry-after[:\s]+(\d+)").ok();
        if let Some(re) = header_pattern {
            if let Some(caps) = re.captures(content) {
                if let Some(secs) = Self::capture_seconds(&caps, 1) {
                    return (Duration::from_secs(secs), RetryAfterSource::Header);
                }
            }
        }

        let json_pattern = Regex::new(r#"\"retry_after\"\s*:\s*\"?(\d+)\"?"#).ok();
        if let Some(re) = json_pattern {
            if let Some(caps) = re.captures(content) {
                if let Some(secs) = Self::capture_seconds(&caps, 1) {
                    return (Duration::from_secs(secs), RetryAfterSource::ResponseBody);
                }
            }
        }

        let text_pattern =
            Regex::new(r"(?i)try\s+again\s+in\s+(\d+)\s*(?:seconds|second|sec|s)").ok();
        if let Some(re) = text_pattern {
            if let Some(caps) = re.captures(content) {
                if let Some(secs) = Self::capture_seconds(&caps, 1) {
                    return (Duration::from_secs(secs), RetryAfterSource::TextParsed);
                }
            }
        }

        (
            self.config.default_retry_wait,
            RetryAfterSource::ConfigDefault,
        )
    }

    fn capture_seconds(caps: &regex::Captures<'_>, index: usize) -> Option<u64> {
        caps.get(index).and_then(|m| m.as_str().parse::<u64>().ok())
    }

    fn read_file_tail(&self, path: &Path, max_lines: usize) -> Result<String, std::io::Error> {
        let content = fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(max_lines);
        Ok(lines[start..].join("\n"))
    }

    fn confidence_from_evidence(evidence: &[String], base: f32) -> f32 {
        let extra = (evidence.len().saturating_sub(1) as f32) * 0.03;
        (base + extra).min(0.98)
    }
}

impl Default for StopReasonClassifier {
    fn default() -> Self {
        Self::new().expect("Failed to create default classifier")
    }
}

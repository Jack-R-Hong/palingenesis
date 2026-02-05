use std::fs;
use std::path::Path;
use std::time::Duration;

use regex::Regex;
use tracing::{debug, warn};

const DEFAULT_RETRY_WAIT_SECS: u64 = 30;
const DEFAULT_MAX_LINES: usize = 100;

/// Reason why a session stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Session hit rate limit (HTTP 429 or equivalent).
    RateLimit(RateLimitInfo),
    /// Session exhausted context window.
    ContextExhausted,
    /// User explicitly exited (Ctrl+C, exit command).
    UserExit,
    /// Session completed successfully.
    Completed,
    /// Unknown or unclassifiable reason.
    Unknown(String),
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
    /// Extra rate limit patterns for future extensibility.
    pub extra_rate_limit_patterns: Vec<String>,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            default_retry_wait: Duration::from_secs(DEFAULT_RETRY_WAIT_SECS),
            max_lines: DEFAULT_MAX_LINES,
            extra_rate_limit_patterns: Vec::new(),
        }
    }
}

/// Stop reason classifier implementation.
pub struct StopReasonClassifier {
    config: ClassifierConfig,
    rate_limit_patterns: Vec<Regex>,
}

impl StopReasonClassifier {
    /// Create a new classifier with default configuration.
    pub fn new() -> Result<Self, ClassifierError> {
        Self::with_config(ClassifierConfig::default())
    }

    /// Create with custom configuration.
    pub fn with_config(config: ClassifierConfig) -> Result<Self, ClassifierError> {
        let mut patterns = vec![
            Regex::new(r"(?i)rate[_-]?limit[_-]?error")?,
            Regex::new(r"(?i)rate\s+limit\s+reached")?,
            Regex::new(r"(?i)\b429\b")?,
            Regex::new(r"(?i)too\s+many\s+requests")?,
            Regex::new(r"(?i)quota\s+exceeded")?,
            Regex::new(r"(?i)overloaded[_-]?error|overloaded")?,
            Regex::new(r"(?i)throttl")?,
        ];

        for pattern in &config.extra_rate_limit_patterns {
            patterns.push(Regex::new(pattern)?);
        }

        Ok(Self {
            config,
            rate_limit_patterns: patterns,
        })
    }

    /// Classify the stop reason from session file content.
    pub fn classify(&self, session_path: &Path, _exit_code: Option<i32>) -> ClassificationResult {
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

        self.classify_content(&content, _exit_code)
    }

    /// Classify from raw content (for log analysis).
    pub fn classify_content(&self, content: &str, _exit_code: Option<i32>) -> ClassificationResult {
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

        ClassificationResult {
            reason: StopReason::Unknown("No matching patterns".to_string()),
            confidence: 0.2,
            evidence,
        }
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

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};

use palingenesis::monitor::classifier::{RateLimitInfo, RetryAfterSource, StopReason};
use palingenesis::resume::{
    ResumeContext, ResumeError, ResumeOutcome, ResumeStrategy, ResumeTrigger, SameSessionConfig,
    SameSessionStrategy,
};
use palingenesis::state::{AuditConfig, AuditEntry, AuditEventType, AuditLogger, AuditOutcome};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn audit_entry_serializes_with_required_fields() {
    let entry = AuditEntry::new(AuditEventType::ResumeStarted, "Starting resume")
        .with_outcome(AuditOutcome::Pending)
        .with_stop_reason("rate_limit")
        .with_metadata("attempt", 1);

    let value = serde_json::to_value(&entry).expect("serialize entry");
    assert!(value.get("timestamp").is_some());
    assert_eq!(value.get("event_type").unwrap(), "resume_started");
    assert_eq!(value.get("action_taken").unwrap(), "Starting resume");
    assert_eq!(value.get("outcome").unwrap(), "pending");
}

#[test]
fn audit_append_and_query_filters() {
    let temp = tempfile::tempdir().expect("tempdir");
    let audit_path = temp.path().join("audit.jsonl");
    let logger = AuditLogger::with_config(AuditConfig {
        audit_path: audit_path.clone(),
        max_size: 1024 * 1024,
        max_files: 2,
        #[cfg(unix)]
        file_mode: 0o600,
    });

    let base_time = Utc::now();
    let session_path = temp.path().join("session.md");

    let mut entry_one = AuditEntry::new(AuditEventType::ResumeStarted, "Start")
        .with_session(session_path.clone())
        .with_outcome(AuditOutcome::Pending);
    entry_one.timestamp = base_time - ChronoDuration::seconds(10);
    logger.log(&entry_one).expect("log entry one");

    let mut entry_two = AuditEntry::new(AuditEventType::ResumeCompleted, "Done")
        .with_session(session_path.clone())
        .with_outcome(AuditOutcome::Success);
    entry_two.timestamp = base_time + ChronoDuration::seconds(10);
    logger.log(&entry_two).expect("log entry two");

    let results = logger
        .query()
        .event_types(vec![AuditEventType::ResumeCompleted])
        .execute()
        .expect("query results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].event_type, AuditEventType::ResumeCompleted);

    let time_filtered = logger
        .query()
        .after(base_time - ChronoDuration::seconds(5))
        .before(base_time + ChronoDuration::seconds(5))
        .execute()
        .expect("time query");
    assert_eq!(time_filtered.len(), 0);

    let session_filtered = logger
        .query()
        .for_session(session_path.clone())
        .execute()
        .expect("session query");
    assert_eq!(session_filtered.len(), 2);
}

#[test]
fn audit_rotates_when_size_exceeded() {
    let temp = tempfile::tempdir().expect("tempdir");
    let audit_path = temp.path().join("audit.jsonl");
    let logger = AuditLogger::with_config(AuditConfig {
        audit_path: audit_path.clone(),
        max_size: 1,
        max_files: 2,
        #[cfg(unix)]
        file_mode: 0o600,
    });

    let entry =
        AuditEntry::new(AuditEventType::ResumeStarted, "Start").with_outcome(AuditOutcome::Pending);
    logger.log(&entry).expect("log first entry");
    logger.log(&entry).expect("log second entry");

    let rotated = temp.path().join("audit.jsonl.1");
    assert!(rotated.exists());
    assert!(audit_path.exists());
}

#[test]
fn audit_rotation_under_load_creates_multiple_segments() {
    let temp = tempfile::tempdir().expect("tempdir");
    let audit_path = temp.path().join("audit.jsonl");
    let logger = AuditLogger::with_config(AuditConfig {
        audit_path: audit_path.clone(),
        max_size: 200,
        max_files: 3,
        #[cfg(unix)]
        file_mode: 0o600,
    });

    for _ in 0..20 {
        let entry = AuditEntry::new(AuditEventType::ResumeStarted, "Start")
            .with_outcome(AuditOutcome::Pending);
        logger.log(&entry).expect("log entry");
    }

    assert!(temp.path().join("audit.jsonl.1").exists());
    assert!(temp.path().join("audit.jsonl.2").exists());
}

#[test]
fn audit_concurrent_writes_are_not_corrupted() {
    let temp = tempfile::tempdir().expect("tempdir");
    let audit_path = temp.path().join("audit.jsonl");
    let logger = Arc::new(AuditLogger::with_config(AuditConfig {
        audit_path: audit_path.clone(),
        max_size: 1024 * 1024,
        max_files: 2,
        #[cfg(unix)]
        file_mode: 0o600,
    }));

    let mut handles = Vec::new();
    for index in 0..10 {
        let logger = Arc::clone(&logger);
        handles.push(std::thread::spawn(move || {
            let entry = AuditEntry::new(AuditEventType::ResumeStarted, "Start")
                .with_outcome(AuditOutcome::Pending)
                .with_metadata("index", index);
            logger.log(&entry).expect("log entry");
        }));
    }

    for handle in handles {
        handle.join().expect("join thread");
    }

    let content = std::fs::read_to_string(&audit_path).expect("read audit file");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 10);
    for line in lines {
        let _: serde_json::Value = serde_json::from_str(line).expect("valid json line");
    }
}

#[test]
fn audit_query_skips_corrupted_entries() {
    let temp = tempfile::tempdir().expect("tempdir");
    let audit_path = temp.path().join("audit.jsonl");
    let logger = AuditLogger::with_config(AuditConfig {
        audit_path: audit_path.clone(),
        max_size: 1024 * 1024,
        max_files: 2,
        #[cfg(unix)]
        file_mode: 0o600,
    });

    std::fs::write(&audit_path, "{not json}\n").expect("write corrupted line");

    let entry =
        AuditEntry::new(AuditEventType::ResumeStarted, "Start").with_outcome(AuditOutcome::Pending);
    logger.log(&entry).expect("log entry");

    let results = logger.query().execute().expect("query results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].event_type, AuditEventType::ResumeStarted);
}

#[cfg(unix)]
#[test]
fn audit_file_created_with_secure_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("tempdir");
    let audit_path = temp.path().join("audit.jsonl");
    let logger = AuditLogger::with_config(AuditConfig {
        audit_path: audit_path.clone(),
        max_size: 1024,
        max_files: 2,
        file_mode: 0o600,
    });

    let entry =
        AuditEntry::new(AuditEventType::ResumeStarted, "Start").with_outcome(AuditOutcome::Pending);
    logger.log(&entry).expect("log entry");

    let metadata = std::fs::metadata(&audit_path).expect("metadata");
    let mode = metadata.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

struct TestTrigger;

#[async_trait]
impl ResumeTrigger for TestTrigger {
    async fn trigger(&self, _ctx: &ResumeContext) -> Result<(), ResumeError> {
        Ok(())
    }
}

fn rate_limit_reason() -> StopReason {
    StopReason::RateLimit(RateLimitInfo {
        retry_after: std::time::Duration::from_secs(10),
        source: RetryAfterSource::Header,
        message: None,
    })
}

#[tokio::test]
async fn audit_logs_resume_events_from_strategy() {
    let _lock = ENV_LOCK.lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let state_dir = temp.path().join("state");
    unsafe {
        std::env::set_var("PALINGENESIS_STATE", &state_dir);
    }

    let ctx = ResumeContext::new(PathBuf::from("/tmp/session.md"), rate_limit_reason())
        .with_retry_after(std::time::Duration::from_secs(0));
    let mut config = SameSessionConfig::default();
    config.backoff_jitter = false;
    let strategy = SameSessionStrategy::with_config(config).with_trigger(TestTrigger);

    let outcome = strategy.execute(&ctx).await.expect("outcome");
    assert!(matches!(outcome, ResumeOutcome::Success { .. }));

    let audit_path = state_dir.join("audit.jsonl");
    let content = std::fs::read_to_string(&audit_path).expect("read audit");
    assert!(content.contains("resume_started"));
    assert!(content.contains("resume_completed"));

    unsafe {
        std::env::remove_var("PALINGENESIS_STATE");
    }
}

use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;

use palingenesis::monitor::classifier::StopReason;
use palingenesis::monitor::session::{Session, SessionState};
use palingenesis::resume::{ResumeContext, ResumeOutcome};

#[test]
fn resume_context_has_expected_defaults() {
    let path = PathBuf::from("/tmp/session.json");
    let reason = StopReason::Completed;
    let ctx = ResumeContext::new(path.clone(), reason.clone());

    assert_eq!(ctx.session_path, path);
    assert_eq!(ctx.stop_reason, reason);
    assert!(ctx.retry_after.is_none());
    assert!(ctx.session_metadata.is_none());
    assert_eq!(ctx.attempt_number, 1);
    assert!(ctx.timestamp <= Utc::now());
}

#[test]
fn resume_context_builders_update_fields() {
    let session = Session {
        path: PathBuf::from("/tmp/session.md"),
        state: SessionState {
            steps_completed: Vec::new(),
            last_step: None,
            status: None,
            workflow_type: None,
            project_name: None,
            input_documents: Vec::new(),
        },
    };

    let reason = StopReason::Completed;
    let mut ctx = ResumeContext::new(PathBuf::from("/tmp/session.md"), reason)
        .with_retry_after(Duration::from_secs(45))
        .with_session(session.clone());

    assert_eq!(ctx.retry_after, Some(Duration::from_secs(45)));
    assert_eq!(ctx.session_metadata, Some(session));
    ctx.increment_attempt();
    assert_eq!(ctx.attempt_number, 2);
}

#[test]
fn resume_outcome_helpers_behave() {
    let success = ResumeOutcome::success(PathBuf::from("/tmp/session.md"), "resumed");
    assert!(success.is_success());
    assert!(!success.should_retry());

    let retryable = ResumeOutcome::failure("oops", true);
    assert!(!retryable.is_success());
    assert!(retryable.should_retry());

    let delayed = ResumeOutcome::delayed(Duration::from_secs(10), "backoff");
    assert!(delayed.should_retry());

    let skipped = ResumeOutcome::skipped("user exit");
    assert!(!skipped.should_retry());
}

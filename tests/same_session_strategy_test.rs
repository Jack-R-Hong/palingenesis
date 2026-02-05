use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use palingenesis::monitor::classifier::{RateLimitInfo, RetryAfterSource, StopReason};
use palingenesis::monitor::session::{Session, SessionState, StepValue};
use palingenesis::resume::{
    ResumeContext, ResumeError, ResumeOutcome, ResumeStrategy, ResumeTrigger,
    SameSessionConfig, SameSessionStrategy,
};
use palingenesis::state::StateStore;

struct TestTrigger {
    calls: Arc<AtomicUsize>,
    should_fail: bool,
}

#[async_trait]
impl ResumeTrigger for TestTrigger {
    async fn trigger(&self, _ctx: &ResumeContext) -> Result<(), ResumeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            return Err(ResumeError::CommandFailed {
                command: "test".to_string(),
                stderr: "fail".to_string(),
            });
        }
        Ok(())
    }
}

fn rate_limit_reason() -> StopReason {
    StopReason::RateLimit(RateLimitInfo {
        retry_after: Duration::from_secs(10),
        source: RetryAfterSource::Header,
        message: None,
    })
}

#[tokio::test(start_paused = true)]
async fn same_session_waits_for_retry_after() {
    let calls = Arc::new(AtomicUsize::new(0));
    let trigger = TestTrigger {
        calls: Arc::clone(&calls),
        should_fail: false,
    };
    let ctx = ResumeContext::new(PathBuf::from("/tmp/session.md"), rate_limit_reason())
        .with_retry_after(Duration::from_secs(60));
    let mut config = SameSessionConfig::default();
    config.backoff_jitter = false;
    let strategy = SameSessionStrategy::with_config(config).with_trigger(trigger);

    let handle = tokio::spawn(async move { strategy.execute(&ctx).await });

    tokio::time::advance(Duration::from_secs(59)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    tokio::time::advance(Duration::from_secs(1)).await;
    let outcome = handle.await.expect("task").expect("outcome");
    assert!(outcome.is_success());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn same_session_uses_backoff_when_no_retry_after() {
    let calls = Arc::new(AtomicUsize::new(0));
    let trigger = TestTrigger {
        calls: Arc::clone(&calls),
        should_fail: false,
    };
    let mut ctx = ResumeContext::new(PathBuf::from("/tmp/session.md"), rate_limit_reason());
    ctx.attempt_number = 2;

    let strategy = SameSessionStrategy::with_config(SameSessionConfig::default())
        .with_trigger(trigger);
    let handle = tokio::spawn(async move { strategy.execute(&ctx).await });

    tokio::time::advance(Duration::from_secs(59)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    tokio::time::advance(Duration::from_secs(1)).await;
    let outcome = handle.await.expect("task").expect("outcome");
    assert!(outcome.is_success());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn same_session_cancels_wait() {
    let calls = Arc::new(AtomicUsize::new(0));
    let trigger = TestTrigger {
        calls: Arc::clone(&calls),
        should_fail: false,
    };
    let cancel = tokio_util::sync::CancellationToken::new();
    let ctx = ResumeContext::new(PathBuf::from("/tmp/session.md"), rate_limit_reason())
        .with_retry_after(Duration::from_secs(60));
    let strategy = SameSessionStrategy::with_config(SameSessionConfig::default())
        .with_cancellation(cancel.clone())
        .with_trigger(trigger);

    let handle = tokio::spawn(async move { strategy.execute(&ctx).await });
    cancel.cancel();

    let outcome = handle.await.expect("task") .expect("outcome");
    assert!(matches!(outcome, ResumeOutcome::Skipped { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn same_session_updates_state_on_success() {
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    let _lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let state_dir = temp.path().join("state");
    unsafe {
        std::env::set_var("PALINGENESIS_STATE", &state_dir);
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let trigger = TestTrigger {
        calls: Arc::clone(&calls),
        should_fail: false,
    };
    let session_path = PathBuf::from("/tmp/session.md");
    let metadata = Session {
        path: session_path.clone(),
        state: SessionState {
            steps_completed: vec![StepValue::Integer(1), StepValue::String("2".to_string())],
            last_step: Some(2),
            status: None,
            workflow_type: None,
            project_name: None,
            input_documents: Vec::new(),
        },
    };
    let ctx = ResumeContext::new(session_path.clone(), rate_limit_reason())
        .with_session(metadata)
        .with_retry_after(Duration::from_secs(0));

    let config = SameSessionConfig::default();

    let strategy = SameSessionStrategy::with_config(config).with_trigger(trigger);
    let outcome = strategy.execute(&ctx).await.expect("outcome");
    assert!(outcome.is_success());
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let state = StateStore::new().load();
    assert_eq!(state.stats.total_resumes, 1);
    assert!(state.stats.last_resume.is_some());
    let current = state.current_session.expect("current session");
    assert_eq!(current.path, session_path);
    assert_eq!(current.steps_completed, vec![1, 2]);
    assert_eq!(current.last_step, 2);

    unsafe {
        std::env::remove_var("PALINGENESIS_STATE");
    }
}

#[tokio::test]
async fn same_session_returns_delayed_on_trigger_failure() {
    let calls = Arc::new(AtomicUsize::new(0));
    let trigger = TestTrigger {
        calls: Arc::clone(&calls),
        should_fail: true,
    };
    let ctx = ResumeContext::new(PathBuf::from("/tmp/session.md"), rate_limit_reason());

    let mut config = SameSessionConfig::default();
    config.max_retries = 2;
    let strategy = SameSessionStrategy::with_config(config).with_trigger(trigger);

    let outcome = strategy.execute(&ctx).await.expect("outcome");
    assert!(matches!(outcome, ResumeOutcome::Delayed { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn same_session_respects_max_retries() {
    let calls = Arc::new(AtomicUsize::new(0));
    let trigger = TestTrigger {
        calls: Arc::clone(&calls),
        should_fail: false,
    };
    let mut ctx = ResumeContext::new(PathBuf::from("/tmp/session.md"), rate_limit_reason());
    ctx.attempt_number = 3;

    let mut config = SameSessionConfig::default();
    config.max_retries = 2;
    let strategy = SameSessionStrategy::with_config(config).with_trigger(trigger);

    let outcome = strategy.execute(&ctx).await.expect("outcome");
    assert!(matches!(outcome, ResumeOutcome::Failure { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

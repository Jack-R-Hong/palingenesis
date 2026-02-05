use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use palingenesis::monitor::classifier::StopReason;
use palingenesis::monitor::session::{Session, SessionState, StepValue};
use palingenesis::resume::{
    BackupError, BackupHandler, NewSessionConfig, NewSessionStrategy, ResumeContext, ResumeError,
    ResumeOutcome, ResumeStrategy, SessionCreator,
};
use palingenesis::state::StateStore;

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct TestBackup {
    calls: Arc<AtomicUsize>,
    should_fail: bool,
}

#[async_trait]
impl BackupHandler for TestBackup {
    async fn backup(&self, _session_path: &Path) -> Result<PathBuf, BackupError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            return Err(BackupError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "backup failed",
            )));
        }
        Ok(PathBuf::from("/tmp/backup.md"))
    }
}

struct TestCreator {
    calls: Arc<AtomicUsize>,
    prompt: Arc<Mutex<Option<String>>>,
    session_path: PathBuf,
}

#[async_trait]
impl SessionCreator for TestCreator {
    async fn create(
        &self,
        prompt: &str,
        _session_dir: &Path,
    ) -> Result<PathBuf, ResumeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut stored = self.prompt.lock().expect("prompt lock");
        *stored = Some(prompt.to_string());
        Ok(self.session_path.clone())
    }
}

fn context_exhausted() -> StopReason {
    StopReason::ContextExhausted(None)
}

#[tokio::test]
async fn new_session_uses_next_step_file() {
    let _lock = ENV_LOCK.lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let state_dir = temp.path().join("state");
    unsafe {
        std::env::set_var("PALINGENESIS_STATE", &state_dir);
    }
    let session_path = temp.path().join("session.md");
    std::fs::write(&session_path, "session").expect("session file");

    let next_step_path = temp.path().join("Next-step.md");
    std::fs::write(
        &next_step_path,
        "# Step 5: Implement authentication\nContinue with OAuth2.",
    )
    .expect("next-step file");

    let calls = Arc::new(AtomicUsize::new(0));
    let prompt = Arc::new(Mutex::new(None));
    let creator = TestCreator {
        calls: Arc::clone(&calls),
        prompt: Arc::clone(&prompt),
        session_path: temp.path().join("new-session.md"),
    };
    let backup_calls = Arc::new(AtomicUsize::new(0));
    let backup = TestBackup {
        calls: Arc::clone(&backup_calls),
        should_fail: false,
    };

    let config = NewSessionConfig {
        prompt_template: "Starting new session from step {step}: {description}\n{context}".to_string(),
        ..NewSessionConfig::default()
    };

    let strategy = NewSessionStrategy::with_config(config)
        .with_session_creator(creator)
        .with_backup_handler(backup);

    let ctx = ResumeContext::new(session_path, context_exhausted());
    let outcome = strategy.execute(&ctx).await.expect("outcome");
    assert!(outcome.is_success());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(backup_calls.load(Ordering::SeqCst), 1);

    unsafe {
        std::env::remove_var("PALINGENESIS_STATE");
    }

    let stored = prompt.lock().expect("prompt lock");
    let rendered = stored.as_ref().expect("prompt");
    assert!(rendered.contains("step 5"));
    assert!(rendered.contains("Implement authentication"));
}

#[tokio::test]
async fn new_session_parses_numbered_next_step() {
    let _lock = ENV_LOCK.lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let state_dir = temp.path().join("state");
    unsafe {
        std::env::set_var("PALINGENESIS_STATE", &state_dir);
    }
    let session_path = temp.path().join("session.md");
    std::fs::write(&session_path, "session").expect("session file");

    let next_step_path = temp.path().join("Next-step.md");
    std::fs::write(&next_step_path, "5. Implement authentication")
        .expect("next-step file");

    let calls = Arc::new(AtomicUsize::new(0));
    let prompt = Arc::new(Mutex::new(None));
    let creator = TestCreator {
        calls: Arc::clone(&calls),
        prompt: Arc::clone(&prompt),
        session_path: temp.path().join("new-session.md"),
    };

    let strategy = NewSessionStrategy::new().with_session_creator(creator);
    let ctx = ResumeContext::new(session_path, context_exhausted());
    let outcome = strategy.execute(&ctx).await.expect("outcome");
    assert!(outcome.is_success());
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    unsafe {
        std::env::remove_var("PALINGENESIS_STATE");
    }

    let stored = prompt.lock().expect("prompt lock");
    let rendered = stored.as_ref().expect("prompt");
    assert!(rendered.contains("step 5"));
    assert!(rendered.contains("Implement authentication"));
}

#[tokio::test]
async fn new_session_falls_back_to_steps_completed() {
    let _lock = ENV_LOCK.lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let state_dir = temp.path().join("state");
    unsafe {
        std::env::set_var("PALINGENESIS_STATE", &state_dir);
    }
    let session_path = temp.path().join("session.md");
    std::fs::write(&session_path, "session").expect("session file");

    let metadata = Session {
        path: session_path.clone(),
        state: SessionState {
            steps_completed: vec![StepValue::Integer(2), StepValue::String("4".to_string())],
            last_step: Some(4),
            status: None,
            workflow_type: None,
            project_name: None,
            input_documents: Vec::new(),
        },
    };

    let calls = Arc::new(AtomicUsize::new(0));
    let prompt = Arc::new(Mutex::new(None));
    let creator = TestCreator {
        calls: Arc::clone(&calls),
        prompt: Arc::clone(&prompt),
        session_path: temp.path().join("new-session.md"),
    };

    let strategy = NewSessionStrategy::new().with_session_creator(creator);
    let ctx = ResumeContext::new(session_path, context_exhausted()).with_session(metadata);

    let outcome = strategy.execute(&ctx).await.expect("outcome");
    assert!(outcome.is_success());
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    unsafe {
        std::env::remove_var("PALINGENESIS_STATE");
    }

    let stored = prompt.lock().expect("prompt lock");
    let rendered = stored.as_ref().expect("prompt");
    assert!(rendered.contains("step 5"));
}

#[tokio::test]
async fn new_session_continues_when_backup_fails() {
    let _lock = ENV_LOCK.lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let state_dir = temp.path().join("state");
    unsafe {
        std::env::set_var("PALINGENESIS_STATE", &state_dir);
    }
    let session_path = temp.path().join("session.md");
    std::fs::write(&session_path, "session").expect("session file");

    let calls = Arc::new(AtomicUsize::new(0));
    let prompt = Arc::new(Mutex::new(None));
    let creator = TestCreator {
        calls: Arc::clone(&calls),
        prompt: Arc::clone(&prompt),
        session_path: temp.path().join("new-session.md"),
    };
    let backup_calls = Arc::new(AtomicUsize::new(0));
    let backup = TestBackup {
        calls: Arc::clone(&backup_calls),
        should_fail: true,
    };

    let strategy = NewSessionStrategy::new()
        .with_session_creator(creator)
        .with_backup_handler(backup);

    let ctx = ResumeContext::new(session_path, context_exhausted());
    let outcome = strategy.execute(&ctx).await.expect("outcome");
    assert!(outcome.is_success());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(backup_calls.load(Ordering::SeqCst), 1);

    unsafe {
        std::env::remove_var("PALINGENESIS_STATE");
    }
}

#[tokio::test]
async fn new_session_updates_state_on_success() {
    let _lock = ENV_LOCK.lock().expect("env lock");

    let temp = tempfile::tempdir().expect("tempdir");
    let state_dir = temp.path().join("state");
    unsafe {
        std::env::set_var("PALINGENESIS_STATE", &state_dir);
    }

    let session_path = temp.path().join("session.md");
    std::fs::write(&session_path, "session").expect("session file");
    let metadata = Session {
        path: session_path.clone(),
        state: SessionState {
            steps_completed: vec![StepValue::Integer(1), StepValue::Integer(2)],
            last_step: Some(2),
            status: None,
            workflow_type: None,
            project_name: None,
            input_documents: Vec::new(),
        },
    };

    let new_session_path = temp.path().join("new-session.md");
    let calls = Arc::new(AtomicUsize::new(0));
    let prompt = Arc::new(Mutex::new(None));
    let creator = TestCreator {
        calls: Arc::clone(&calls),
        prompt: Arc::clone(&prompt),
        session_path: new_session_path.clone(),
    };

    let strategy = NewSessionStrategy::new().with_session_creator(creator);
    let ctx = ResumeContext::new(session_path, context_exhausted()).with_session(metadata);
    let outcome = strategy.execute(&ctx).await.expect("outcome");
    assert!(matches!(outcome, ResumeOutcome::Success { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let state = StateStore::new().load();
    assert_eq!(state.stats.total_resumes, 1);
    assert!(state.stats.last_resume.is_some());
    let current = state.current_session.expect("current session");
    assert_eq!(current.path, new_session_path);
    assert_eq!(current.steps_completed, vec![1, 2]);
    assert_eq!(current.last_step, 2);

    unsafe {
        std::env::remove_var("PALINGENESIS_STATE");
    }
}

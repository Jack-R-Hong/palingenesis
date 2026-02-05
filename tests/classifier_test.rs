use std::path::PathBuf;
use std::time::Duration;

use palingenesis::monitor::classifier::{
    ClassifierConfig, RetryAfterSource, StopReason, StopReasonClassifier, UserExitInfo,
    UserExitType,
};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn detects_rate_limit_error_with_retry_after_json() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let content = include_str!("fixtures/rate_limit_anthropic.json");
    let result = classifier.classify_content(content, None);

    match result.reason {
        StopReason::RateLimit(info) => {
            assert_eq!(info.retry_after, Duration::from_secs(45));
            assert_eq!(info.source, RetryAfterSource::ResponseBody);
        }
        other => panic!("expected rate limit, got {other:?}"),
    }
}

#[test]
fn detects_http_429_from_file_and_extracts_retry_after_header() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let path = fixture_path("rate_limit_retry_after.txt");
    let result = classifier.classify(&path, None);

    match result.reason {
        StopReason::RateLimit(info) => {
            assert_eq!(info.retry_after, Duration::from_secs(120));
            assert_eq!(info.source, RetryAfterSource::Header);
        }
        other => panic!("expected rate limit, got {other:?}"),
    }
}

#[test]
fn detects_rate_limit_from_opencode_log_fixture() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let path = fixture_path("rate_limit_opencode.log");
    let result = classifier.classify(&path, None);

    match result.reason {
        StopReason::RateLimit(info) => {
            assert_eq!(info.retry_after, Duration::from_secs(30));
            assert_eq!(info.source, RetryAfterSource::Header);
        }
        other => panic!("expected rate limit, got {other:?}"),
    }
}

#[test]
fn uses_default_wait_time_when_retry_after_missing() {
    let config = ClassifierConfig {
        default_retry_wait: Duration::from_secs(42),
        ..Default::default()
    };
    let classifier = StopReasonClassifier::with_config(config).expect("classifier");
    let content = "rate_limit_error: too many requests";
    let result = classifier.classify_content(content, None);

    match result.reason {
        StopReason::RateLimit(info) => {
            assert_eq!(info.retry_after, Duration::from_secs(42));
            assert_eq!(info.source, RetryAfterSource::ConfigDefault);
        }
        other => panic!("expected rate limit, got {other:?}"),
    }
}

#[test]
fn prioritizes_rate_limit_when_multiple_indicators_present() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let content = "rate_limit_error: too many requests; context_length_exceeded";
    let result = classifier.classify_content(content, None);

    assert!(matches!(result.reason, StopReason::RateLimit(_)));
}

#[test]
fn detects_context_length_exceeded_pattern() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let content = include_str!("fixtures/context_exceeded.txt");
    let result = classifier.classify_content(content, None);

    assert!(matches!(result.reason, StopReason::ContextExhausted(_)));
}

#[test]
fn detects_token_threshold_exceeded() {
    let config = ClassifierConfig {
        context_threshold_percent: 0.8,
        default_context_size: 200,
        ..Default::default()
    };
    let classifier = StopReasonClassifier::with_config(config).expect("classifier");
    let content = include_str!("fixtures/token_limit.txt");
    let result = classifier.classify_content(content, None);

    match result.reason {
        StopReason::ContextExhausted(Some(info)) => {
            let usage = info.usage_percent.expect("usage percent");
            assert!(usage > 0.8);
            assert_eq!(info.context_size, Some(200));
        }
        other => panic!("expected context exhausted, got {other:?}"),
    }
}

#[test]
fn classifies_completed_sessions_before_context_exhaustion() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let path = fixture_path("session_complete.md");
    let result = classifier.classify(&path, None);

    assert!(matches!(result.reason, StopReason::Completed));
}

#[test]
fn handles_read_errors_without_crashing() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let missing = fixture_path("missing_rate_limit.txt");
    let result = classifier.classify(&missing, None);

    match result.reason {
        StopReason::Unknown(message) => {
            assert!(message.contains("Read error"));
        }
        other => panic!("expected unknown, got {other:?}"),
    }
    assert_eq!(result.confidence, 0.0);
}

#[test]
fn detects_user_exit_ctrl_c_exit_code() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let path = fixture_path("user_exit_ctrl_c.txt");
    let result = classifier.classify(&path, Some(130));

    match result.reason {
        StopReason::UserExit(info) => {
            assert_eq!(info.exit_type, UserExitType::CtrlC);
            assert_eq!(info.exit_code, Some(130));
        }
        other => panic!("expected user exit, got {other:?}"),
    }
}

#[test]
fn detects_user_exit_command_from_fixture() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let content = include_str!("fixtures/user_exit_command.txt");
    let result = classifier.classify_content(content, None);

    match result.reason {
        StopReason::UserExit(info) => {
            assert_eq!(info.exit_type, UserExitType::ExitCommand);
            assert!(info.message.is_some());
        }
        other => panic!("expected user exit, got {other:?}"),
    }
}

#[test]
fn detects_clean_exit_without_errors() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let content = include_str!("fixtures/clean_exit.txt");
    let result = classifier.classify_content(content, Some(0));

    match result.reason {
        StopReason::UserExit(info) => {
            assert_eq!(info.exit_type, UserExitType::CleanExit);
            assert_eq!(info.exit_code, Some(0));
        }
        other => panic!("expected user exit, got {other:?}"),
    }
}

#[test]
fn skips_user_exit_when_errors_present() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let content = "error: unexpected failure";
    let result = classifier.classify_content(content, Some(0));

    assert!(!matches!(result.reason, StopReason::UserExit(_)));
}

#[test]
fn prioritizes_rate_limit_over_user_exit() {
    let classifier = StopReasonClassifier::new().expect("classifier");
    let content = "rate_limit_error: too many requests";
    let result = classifier.classify_content(content, Some(130));

    assert!(matches!(result.reason, StopReason::RateLimit(_)));
}

#[test]
fn should_auto_resume_respects_user_exit() {
    let reason = StopReason::UserExit(UserExitInfo {
        exit_type: UserExitType::ExitCommand,
        exit_code: None,
        message: Some("exit".to_string()),
    });

    assert!(!reason.should_auto_resume());
}

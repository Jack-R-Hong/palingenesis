use std::path::PathBuf;
use std::time::Duration;

use palingenesis::monitor::classifier::{
    ClassifierConfig, RetryAfterSource, StopReason, StopReasonClassifier,
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
    let content = "rate_limit_error: too many requests; context exhausted";
    let result = classifier.classify_content(content, None);

    assert!(matches!(result.reason, StopReason::RateLimit(_)));
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

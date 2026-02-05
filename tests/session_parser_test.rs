use std::path::{Path, PathBuf};

use palingenesis::monitor::events::{MonitorEvent, WatchEvent};
use palingenesis::monitor::frontmatter::{
    extract_frontmatter, parse_session, ParseError, SessionParser,
};
use palingenesis::monitor::session::StepValue;
use tempfile::tempdir;

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn test_extract_frontmatter_valid() {
    let path = fixture_path("session_valid.md");
    let frontmatter = extract_frontmatter(&path).expect("frontmatter extracted");
    assert!(frontmatter.contains("stepsCompleted"));
}

#[test]
fn test_missing_frontmatter_returns_error() {
    let path = fixture_path("session_no_frontmatter.md");
    let error = extract_frontmatter(&path).expect_err("expected no frontmatter error");
    assert!(matches!(error, ParseError::NoFrontmatter));
}

#[test]
fn test_invalid_yaml_returns_error() {
    let path = fixture_path("session_invalid_yaml.md");
    let error = parse_session(&path).expect_err("expected invalid yaml error");
    assert!(matches!(error, ParseError::InvalidFrontmatter(_)));
}

#[test]
fn test_steps_completed_parses_ints() {
    let path = fixture_path("session_valid.md");
    let session = parse_session(&path).expect("parsed session");
    assert_eq!(session.state.steps_completed.len(), 3);
    assert!(matches!(
        session.state.steps_completed[0],
        StepValue::Integer(1)
    ));
}

#[test]
fn test_steps_completed_parses_strings() {
    let path = fixture_path("session_string_steps.md");
    let session = parse_session(&path).expect("parsed session");
    assert_eq!(session.state.steps_completed.len(), 2);
    assert!(matches!(
        session.state.steps_completed[0],
        StepValue::String(_)
    ));
}

#[test]
fn test_last_step_field_parsed() {
    let path = fixture_path("session_valid.md");
    let session = parse_session(&path).expect("parsed session");
    assert_eq!(session.state.last_step, Some(3));
}

#[test]
fn test_parser_ignores_body_content() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("session.md");
    let contents = r#"---
stepsCompleted: [1]
status: in-progress
---

stepsCompleted: [1, 2
"#;
    std::fs::write(&path, contents).unwrap();

    let session = parse_session(&path).expect("parsed session");
    assert_eq!(session.state.steps_completed.len(), 1);
}

#[test]
fn test_session_parser_emits_session_changed() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("session.md");
    let contents = r#"---
stepsCompleted: [1, 2]
lastStep: 2
status: in-progress
---

body
"#;
    std::fs::write(&path, contents).unwrap();

    let mut parser = SessionParser::new();
    let event = parser
        .handle_event(WatchEvent::FileModified(path.clone()))
        .expect("expected monitor event");

    match event {
        MonitorEvent::SessionChanged { session, previous } => {
            assert_eq!(session.path, path);
            assert!(previous.is_none());
        }
        _ => panic!("expected SessionChanged event"),
    }
}

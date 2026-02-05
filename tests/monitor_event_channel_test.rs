use std::path::Path;

use palingenesis::monitor::classifier::StopReason;
use palingenesis::monitor::core::{Monitor, MonitorConfig};
use palingenesis::monitor::events::{MonitorEvent, WatchEvent};
use palingenesis::monitor::process::{ProcessEvent, ProcessInfo};
use tempfile::tempdir;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout, Duration};
use tokio_util::sync::CancellationToken;

fn write_session(path: &Path) {
    let contents = r#"---
stepsCompleted: [1]
status: in-progress
---

body
"#;
    std::fs::write(path, contents).expect("write session file");
}

#[tokio::test]
async fn emits_session_changed_on_watch_event() {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().join("session.md");
    write_session(&path);

    let (watch_tx, watch_rx) = mpsc::channel(4);
    let (_process_tx, process_rx) = mpsc::channel(4);

    let config = MonitorConfig {
        session_dir: temp.path().to_path_buf(),
        channel_capacity: 10,
        ..MonitorConfig::default()
    };
    let monitor = Monitor::with_config(config).expect("monitor");
    let cancel = CancellationToken::new();
    let mut event_rx = monitor
        .run_with_receivers(cancel.clone(), watch_rx, Some(process_rx))
        .await;

    watch_tx
        .send(WatchEvent::FileModified(path.clone()))
        .await
        .expect("send watch event");

    let event = timeout(Duration::from_millis(200), event_rx.recv())
        .await
        .expect("event")
        .expect("event value");

    match event {
        MonitorEvent::SessionChanged { session, .. } => {
            assert_eq!(session.path, path);
        }
        _ => panic!("expected SessionChanged event"),
    }

    cancel.cancel();
}

#[tokio::test]
async fn emits_session_stopped_after_process_stop() {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().join("session.md");
    write_session(&path);

    let (watch_tx, watch_rx) = mpsc::channel(4);
    let (process_tx, process_rx) = mpsc::channel(4);

    let config = MonitorConfig {
        session_dir: temp.path().to_path_buf(),
        channel_capacity: 10,
        ..MonitorConfig::default()
    };
    let monitor = Monitor::with_config(config).expect("monitor");
    let cancel = CancellationToken::new();
    let mut event_rx = monitor
        .run_with_receivers(cancel.clone(), watch_rx, Some(process_rx))
        .await;

    watch_tx
        .send(WatchEvent::FileModified(path.clone()))
        .await
        .expect("send watch event");

    let _ = timeout(Duration::from_millis(200), event_rx.recv())
        .await
        .expect("event")
        .expect("event value");

    let info = ProcessInfo {
        pid: 42,
        command_line: vec!["opencode".to_string()],
        start_time: None,
        working_dir: None,
    };
    process_tx
        .send(ProcessEvent::ProcessStopped {
            info,
            exit_code: Some(130),
        })
        .await
        .expect("send process event");

    let (reason, session) = timeout(Duration::from_millis(200), async {
        loop {
            let event = event_rx.recv().await.expect("event value");
            if let MonitorEvent::SessionStopped { reason, session, .. } = event {
                return (reason, session);
            }
        }
    })
    .await
    .expect("session stopped");

    assert!(matches!(reason, StopReason::UserExit(_)));
    assert!(session.is_some());

    cancel.cancel();
}

#[tokio::test]
async fn drops_events_when_channel_is_full() {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().join("session.md");
    write_session(&path);

    let (watch_tx, watch_rx) = mpsc::channel(4);
    let (_process_tx, process_rx) = mpsc::channel(4);

    let config = MonitorConfig {
        session_dir: temp.path().to_path_buf(),
        channel_capacity: 1,
        ..MonitorConfig::default()
    };
    let monitor = Monitor::with_config(config).expect("monitor");
    let cancel = CancellationToken::new();
    let mut event_rx = monitor
        .run_with_receivers(cancel.clone(), watch_rx, Some(process_rx))
        .await;

    watch_tx
        .send(WatchEvent::FileModified(path.clone()))
        .await
        .expect("send watch event");
    watch_tx
        .send(WatchEvent::FileModified(path.clone()))
        .await
        .expect("send watch event");

    sleep(Duration::from_millis(25)).await;

    let _ = timeout(Duration::from_millis(200), event_rx.recv())
        .await
        .expect("event")
        .expect("event value");

    let result = timeout(Duration::from_millis(50), event_rx.recv()).await;
    assert!(result.is_err());

    cancel.cancel();
}

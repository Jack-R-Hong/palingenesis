use std::time::Duration;

use palingenesis::monitor::events::WatchEvent;
use palingenesis::monitor::watcher::SessionWatcher;
use tempfile::tempdir;
use tokio::time::{sleep, timeout};
use tokio_util::sync::CancellationToken;

async fn recv_event(
    receiver: &mut tokio::sync::mpsc::Receiver<WatchEvent>,
    timeout_duration: Duration,
) -> Option<WatchEvent> {
    timeout(timeout_duration, receiver.recv())
        .await
        .ok()
        .flatten()
}

#[tokio::test]
async fn test_watcher_creation_with_valid_directory() {
    let temp = tempdir().unwrap();
    let session_dir = temp.path().join("sessions");
    std::fs::create_dir_all(&session_dir).unwrap();

    let watcher = SessionWatcher::with_path(session_dir);
    let cancel = CancellationToken::new();

    let mut receiver = watcher.run(cancel.clone()).await.unwrap();
    sleep(Duration::from_millis(150)).await;
    cancel.cancel();
    let _ = recv_event(&mut receiver, Duration::from_millis(200)).await;
}

#[tokio::test]
async fn test_watcher_waits_for_directory_creation() {
    let temp = tempdir().unwrap();
    let session_dir = temp.path().join("sessions");

    let watcher = SessionWatcher::with_path(session_dir.clone());
    let cancel = CancellationToken::new();

    let mut receiver = watcher.run(cancel.clone()).await.unwrap();
    sleep(Duration::from_millis(150)).await;
    std::fs::create_dir_all(&session_dir).unwrap();

    let event = recv_event(&mut receiver, Duration::from_secs(2)).await;
    assert!(matches!(event, Some(WatchEvent::DirectoryCreated(_))));

    cancel.cancel();
}

#[tokio::test]
async fn test_file_change_event_emission() {
    let temp = tempdir().unwrap();
    let session_dir = temp.path().join("sessions");
    std::fs::create_dir_all(&session_dir).unwrap();
    let file_path = session_dir.join("session.md");

    let watcher = SessionWatcher::with_path(session_dir);
    let cancel = CancellationToken::new();
    let mut receiver = watcher.run(cancel.clone()).await.unwrap();

    sleep(Duration::from_millis(150)).await;
    std::fs::write(&file_path, "hello").unwrap();

    let event = recv_event(&mut receiver, Duration::from_secs(2)).await;
    assert!(matches!(
        event,
        Some(WatchEvent::FileCreated(path)) | Some(WatchEvent::FileModified(path))
            if path == file_path
    ));

    cancel.cancel();
}

#[tokio::test]
async fn test_recursive_watch_emits_for_nested_files() {
    let temp = tempdir().unwrap();
    let session_dir = temp.path().join("sessions");
    let nested_dir = session_dir.join("nested");
    std::fs::create_dir_all(&nested_dir).unwrap();
    let file_path = nested_dir.join("session.md");

    let watcher = SessionWatcher::with_path(session_dir);
    let cancel = CancellationToken::new();
    let mut receiver = watcher.run(cancel.clone()).await.unwrap();

    sleep(Duration::from_millis(150)).await;
    std::fs::write(&file_path, "hello").unwrap();

    let event = recv_event(&mut receiver, Duration::from_secs(2)).await;
    assert!(matches!(
        event,
        Some(WatchEvent::FileCreated(path)) | Some(WatchEvent::FileModified(path))
            if path == file_path
    ));

    cancel.cancel();
}

#[tokio::test]
async fn test_event_debouncing_coalesces_rapid_changes() {
    let temp = tempdir().unwrap();
    let session_dir = temp.path().join("sessions");
    std::fs::create_dir_all(&session_dir).unwrap();
    let file_path = session_dir.join("session.md");

    let watcher = SessionWatcher::with_path(session_dir);
    let cancel = CancellationToken::new();
    let mut receiver = watcher.run(cancel.clone()).await.unwrap();

    sleep(Duration::from_millis(150)).await;
    std::fs::write(&file_path, "initial").unwrap();
    let _ = recv_event(&mut receiver, Duration::from_secs(2)).await;

    for idx in 0..5 {
        std::fs::write(&file_path, format!("update-{idx}")).unwrap();
    }

    sleep(Duration::from_millis(150)).await;

    let mut count = 0;
    while let Ok(Some(event)) = timeout(Duration::from_millis(50), receiver.recv()).await {
        if matches!(event, WatchEvent::FileModified(path) | WatchEvent::FileCreated(path) if path == file_path)
        {
            count += 1;
        }
    }

    assert!(count <= 1, "expected debounced events, got {count}");

    cancel.cancel();
}

#[tokio::test]
async fn test_graceful_shutdown_stops_events() {
    let temp = tempdir().unwrap();
    let session_dir = temp.path().join("sessions");
    std::fs::create_dir_all(&session_dir).unwrap();
    let file_path = session_dir.join("session.md");

    let watcher = SessionWatcher::with_path(session_dir);
    let cancel = CancellationToken::new();
    let mut receiver = watcher.run(cancel.clone()).await.unwrap();

    cancel.cancel();
    sleep(Duration::from_millis(150)).await;

    std::fs::write(&file_path, "after-cancel").unwrap();
    let event = recv_event(&mut receiver, Duration::from_millis(200)).await;
    assert!(event.is_none(), "unexpected event after cancellation");
}

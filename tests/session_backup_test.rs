use palingenesis::resume::{BackupConfig, SessionBackup};
use tokio::time::{sleep, Duration};

fn assert_timestamp_format(name: &str) {
    let parts: Vec<&str> = name.split("-backup-").collect();
    assert_eq!(parts.len(), 2);
    let timestamp_part = parts[1]
        .split('.')
        .next()
        .expect("timestamp part");
    assert_eq!(timestamp_part.len(), 15);
    let (date, time) = timestamp_part.split_at(8);
    assert!(date.chars().all(|c| c.is_ascii_digit()));
    assert_eq!(&time[0..1], "-");
    let time_digits = &time[1..];
    assert!(time_digits.chars().all(|c| c.is_ascii_digit()));
}

#[tokio::test]
async fn backup_creates_copy_in_same_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let session = temp.path().join("session.md");
    tokio::fs::write(&session, "session content")
        .await
        .expect("session write");

    let backupper = SessionBackup::default();
    let backup_path = backupper
        .create_backup(&session)
        .await
        .expect("backup");

    assert_eq!(backup_path.parent(), session.parent());
    let filename = backup_path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("backup filename");
    assert!(filename.starts_with("session-backup-"));
    assert!(filename.ends_with(".md"));
    assert_timestamp_format(filename);

    let original = tokio::fs::read_to_string(&session)
        .await
        .expect("read original");
    let backup = tokio::fs::read_to_string(&backup_path)
        .await
        .expect("read backup");
    assert_eq!(original, backup);
}

#[tokio::test]
async fn backup_prunes_when_exceeding_limit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let session = temp.path().join("session.md");
    tokio::fs::write(&session, "session content")
        .await
        .expect("session write");

    let backupper = SessionBackup::with_config(BackupConfig {
        max_backups: 1,
        ..BackupConfig::default()
    });

    let first = backupper
        .create_backup(&session)
        .await
        .expect("first backup");
    sleep(Duration::from_millis(1100)).await;
    let second = backupper
        .create_backup(&session)
        .await
        .expect("second backup");

    assert!(second.exists());
    assert!(!first.exists());
}

#[tokio::test]
async fn concurrent_backups_succeed_for_multiple_sessions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let session_a = temp.path().join("session-a.md");
    let session_b = temp.path().join("session-b.md");
    tokio::fs::write(&session_a, "alpha").await.expect("write a");
    tokio::fs::write(&session_b, "beta").await.expect("write b");

    let backupper = SessionBackup::default();
    let (backup_a, backup_b) = tokio::join!(
        backupper.create_backup(&session_a),
        backupper.create_backup(&session_b)
    );

    let backup_a = backup_a.expect("backup a");
    let backup_b = backup_b.expect("backup b");

    assert!(backup_a.exists());
    assert!(backup_b.exists());
    assert_ne!(backup_a, backup_b);
}

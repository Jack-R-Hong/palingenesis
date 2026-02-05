use std::env;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use ed25519_dalek::Signer;
use hmac::Mac;
use rand::RngCore;
use rand::rngs::OsRng;
use serde_json::json;
use tower::ServiceExt;

use palingenesis::daemon::state::DaemonState;
use palingenesis::http::handlers;
use palingenesis::http::{AppState, EventBroadcaster};
use palingenesis::telemetry::Metrics;
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
    unsafe {
        env::set_var(key, value);
    }
}

fn remove_env_var(key: &str) {
    unsafe {
        env::remove_var(key);
    }
}

fn test_router() -> Router {
    Router::new()
        .route(
            "/api/v1/bot/discord",
            post(handlers::bot_discord::discord_webhook_handler),
        )
        .route(
            "/api/v1/bot/slack",
            post(handlers::bot_slack::slack_webhook_handler),
        )
        .with_state(AppState::new(
            Arc::new(DaemonState::new()),
            EventBroadcaster::default(),
            Arc::new(Metrics::new()),
        ))
}

fn write_bot_config(temp: &tempfile::TempDir, discord_key: &str, slack_secret: &str) {
    let config_path = temp.path().join("config.toml");
    let contents = format!(
        r#"[bot]
enabled = true
allow_all_users = false
discord_public_key = "{discord_key}"
slack_signing_secret = "{slack_secret}"

[[bot.authorized_users]]
platform = "discord"
user_id = "123"

[[bot.authorized_users]]
platform = "slack"
user_id = "U123"
"#
    );
    std::fs::write(&config_path, contents).unwrap();
    set_env_var("PALINGENESIS_CONFIG", &config_path);
}

fn current_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string()
}

#[tokio::test]
async fn test_discord_webhook_accepts_valid_request() {
    let _lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());
    write_bot_config(&temp, &public_key_hex, "slack-secret");

    let body = json!({
        "type": 2,
        "data": {"name": "palin", "options": [{"name": "status"}]},
        "member": {"user": {"id": "123"}}
    })
    .to_string();
    let timestamp = "1700000000";
    let mut message = Vec::from(timestamp.as_bytes());
    message.extend_from_slice(body.as_bytes());
    let signature = signing_key.sign(&message);

    let response = test_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/bot/discord")
                .header("X-Signature-Ed25519", hex::encode(signature.to_bytes()))
                .header("X-Signature-Timestamp", timestamp)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    remove_env_var("PALINGENESIS_CONFIG");
}

#[tokio::test]
async fn test_discord_webhook_rejects_invalid_signature() {
    let _lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());
    write_bot_config(&temp, &public_key_hex, "slack-secret");

    let body = json!({
        "type": 2,
        "data": {"name": "palin", "options": [{"name": "status"}]},
        "member": {"user": {"id": "123"}}
    })
    .to_string();

    let response = test_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/bot/discord")
                .header("X-Signature-Ed25519", "bad")
                .header("X-Signature-Timestamp", "1700000000")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    remove_env_var("PALINGENESIS_CONFIG");
}

#[tokio::test]
async fn test_slack_webhook_accepts_valid_request() {
    let _lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    write_bot_config(&temp, "deadbeef", "slack-secret");

    let body = "user_id=U123&command=%2Fpalin&text=status";
    let timestamp = current_timestamp();
    let base = format!("v0:{timestamp}:{body}");
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(b"slack-secret").unwrap();
    mac.update(base.as_bytes());
    let signature = format!("v0={}", hex::encode(mac.finalize().into_bytes()));

    let response = test_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/bot/slack")
                .header("X-Slack-Signature", signature)
                .header("X-Slack-Request-Timestamp", &timestamp)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    remove_env_var("PALINGENESIS_CONFIG");
}

#[tokio::test]
async fn test_slack_webhook_rejects_invalid_signature() {
    let _lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    write_bot_config(&temp, "deadbeef", "slack-secret");

    let body = "user_id=U123&command=%2Fpalin&text=status";
    let response = test_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/bot/slack")
                .header("X-Slack-Signature", "v0=badsignature")
                .header("X-Slack-Request-Timestamp", current_timestamp())
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    remove_env_var("PALINGENESIS_CONFIG");
}

#[tokio::test]
async fn test_unauthorized_user_rejected() {
    let _lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    write_bot_config(&temp, "deadbeef", "slack-secret");

    let body = "user_id=U999&command=%2Fpalin&text=pause";
    let timestamp = current_timestamp();
    let base = format!("v0:{timestamp}:{body}");
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(b"slack-secret").unwrap();
    mac.update(base.as_bytes());
    let signature = format!("v0={}", hex::encode(mac.finalize().into_bytes()));

    let response = test_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/bot/slack")
                .header("X-Slack-Signature", signature)
                .header("X-Slack-Request-Timestamp", &timestamp)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let text = payload["blocks"][0]["text"]["text"].as_str().unwrap();
    assert!(text.contains("Unauthorized"));
    remove_env_var("PALINGENESIS_CONFIG");
}

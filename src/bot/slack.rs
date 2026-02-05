use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

use crate::bot::auth::BotAuth;
use crate::bot::commands::{BotCommand, BotCommandResult};
use crate::bot::executor::CommandExecutor;
use crate::config::schema::{BotConfig, BotPlatform};
use crate::http::server::AppState;

const SLACK_SIGNATURE_HEADER: &str = "X-Slack-Signature";
const SLACK_TIMESTAMP_HEADER: &str = "X-Slack-Request-Timestamp";
const SLACK_SIG_PREFIX: &str = "v0=";
const SLACK_TIMEOUT_SECS: i64 = 60 * 5;

type HmacSha256 = Hmac<Sha256>;

/// Handles Slack slash command webhooks (POST /api/v1/bot/slack).
pub async fn slack_webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let Some(config) = state.daemon_state().bot_config() else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json_message("Bot config unavailable")))
            .into_response();
    };

    if !config.enabled {
        return (StatusCode::FORBIDDEN, Json(json_message("Bot commands disabled")))
            .into_response();
    }

    let verification = verify_slack_signature(&config, &headers, &body);
    if let Err(message) = verification {
        return (StatusCode::UNAUTHORIZED, Json(json_message(message))).into_response();
    }

    let payload: SlackCommandPayload = match serde_urlencoded::from_bytes(&body) {
        Ok(payload) => payload,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, Json(json_message("Invalid payload")))
                .into_response();
        }
    };

    let auth = BotAuth::for_platform(&config, BotPlatform::Slack);
    if !auth.is_authorized(&payload.user_id) {
        let result = BotCommandResult::error(
            "Unauthorized: You don't have permission to use this command.",
        );
        return (StatusCode::OK, Json(result.to_slack_response())).into_response();
    }

    let command_text = if payload.text.trim().is_empty() {
        "/palin help".to_string()
    } else {
        format!("/palin {}", payload.text.trim())
    };

    let command = match BotCommand::from_str(&command_text) {
        Ok(command) => command,
        Err(err) => {
            let result = BotCommandResult::error(err.to_string());
            return (StatusCode::OK, Json(result.to_slack_response())).into_response();
        }
    };

    let executor = CommandExecutor::new(Arc::clone(state.daemon_state()), state.events().clone());
    let result = executor.execute(command);
    (StatusCode::OK, Json(result.to_slack_response())).into_response()
}

fn verify_slack_signature(
    config: &BotConfig,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<(), &'static str> {
    let Some(secret) = config.slack_signing_secret.as_ref() else {
        return Err("Slack signing secret not configured");
    };

    let signature = headers
        .get(SLACK_SIGNATURE_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or("Missing Slack signature header")?;
    let timestamp = headers
        .get(SLACK_TIMESTAMP_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or("Missing Slack timestamp header")?;

    if !signature.starts_with(SLACK_SIG_PREFIX) {
        return Err("Invalid Slack signature format");
    }

    let timestamp_value: i64 = timestamp.parse().map_err(|_| "Invalid timestamp")?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Invalid system time")?
        .as_secs() as i64;
    if (now - timestamp_value).abs() > SLACK_TIMEOUT_SECS {
        return Err("Slack request timestamp out of range");
    }

    let base_string = format!("v0:{timestamp}:{body}", body = String::from_utf8_lossy(body));
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| "Invalid secret")?;
    mac.update(base_string.as_bytes());
    let expected = format!("v0={}", hex::encode(mac.finalize().into_bytes()));

    if !constant_time_eq(expected.as_bytes(), signature.as_bytes()) {
        return Err("Signature verification failed");
    }

    Ok(())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn json_message(message: &str) -> serde_json::Value {
    serde_json::json!({"message": message})
}

#[derive(Debug, Deserialize)]
struct SlackCommandPayload {
    user_id: String,
    text: String,
    #[allow(dead_code)]
    command: String,
    #[allow(dead_code)]
    response_url: Option<String>,
}

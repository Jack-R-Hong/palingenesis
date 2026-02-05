use std::str::FromStr;
use std::sync::Arc;

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;

use crate::bot::auth::BotAuth;
use crate::bot::commands::{BotCommand, BotCommandResult};
use crate::bot::executor::CommandExecutor;
use crate::config::schema::{BotConfig, BotPlatform};
use crate::http::server::AppState;

const DISCORD_SIGNATURE_HEADER: &str = "X-Signature-Ed25519";
const DISCORD_TIMESTAMP_HEADER: &str = "X-Signature-Timestamp";
const DISCORD_PING: u8 = 1;
const DISCORD_COMMAND: u8 = 2;

/// Handles Discord interaction webhooks (POST /api/v1/bot/discord).
pub async fn discord_webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let Some(config) = state.daemon_state().bot_config() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json_message("Bot config unavailable")),
        )
            .into_response();
    };

    if !config.enabled {
        return (
            StatusCode::FORBIDDEN,
            Json(json_message("Bot commands disabled")),
        )
            .into_response();
    }

    let verification = verify_discord_signature(&config, &headers, &body);
    if let Err(message) = verification {
        return (StatusCode::UNAUTHORIZED, Json(json_message(message))).into_response();
    }

    let interaction: DiscordInteraction = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json_message("Invalid payload")),
            )
                .into_response();
        }
    };

    if interaction.interaction_type == DISCORD_PING {
        return (StatusCode::OK, Json(serde_json::json!({"type": 1}))).into_response();
    }

    if interaction.interaction_type != DISCORD_COMMAND {
        return (
            StatusCode::BAD_REQUEST,
            Json(json_message("Unsupported interaction")),
        )
            .into_response();
    }

    let user_id = match interaction.user_id() {
        Some(user_id) => user_id,
        None => {
            return (StatusCode::BAD_REQUEST, Json(json_message("Missing user"))).into_response();
        }
    };

    let auth = BotAuth::for_platform(&config, BotPlatform::Discord);
    if !auth.is_authorized(&user_id) {
        let result =
            BotCommandResult::error("Unauthorized: You don't have permission to use this command.");
        let response = result.to_discord_response();
        return (StatusCode::OK, Json(response)).into_response();
    }

    let command = match parse_discord_command(&interaction) {
        Ok(command) => command,
        Err(message) => {
            let response = BotCommandResult::error(message).to_discord_response();
            return (StatusCode::OK, Json(response)).into_response();
        }
    };

    let executor = CommandExecutor::new(Arc::clone(state.daemon_state()), state.events().clone());
    let result = executor.execute(command);
    let response = result.to_discord_response();
    (StatusCode::OK, Json(response)).into_response()
}

fn verify_discord_signature(
    config: &BotConfig,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<(), &'static str> {
    let Some(public_key_hex) = config.discord_public_key.as_ref() else {
        return Err("Discord public key not configured");
    };
    let signature_hex = headers
        .get(DISCORD_SIGNATURE_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or("Missing Discord signature header")?;
    let timestamp = headers
        .get(DISCORD_TIMESTAMP_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or("Missing Discord timestamp header")?;

    let public_key_bytes = hex::decode(public_key_hex.trim()).map_err(|_| "Invalid public key")?;
    let public_key: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| "Invalid public key length")?;
    let signature_bytes = hex::decode(signature_hex).map_err(|_| "Invalid signature")?;
    let signature: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| "Invalid signature length")?;

    let verifying_key = VerifyingKey::from_bytes(&public_key).map_err(|_| "Invalid public key")?;
    let signature = Signature::from_bytes(&signature);
    let mut message = Vec::with_capacity(timestamp.len() + body.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.extend_from_slice(body);

    verifying_key
        .verify_strict(&message, &signature)
        .map_err(|_| "Signature verification failed")
}

fn parse_discord_command(interaction: &DiscordInteraction) -> Result<BotCommand, &'static str> {
    let data = interaction.data.as_ref().ok_or("Missing command data")?;

    if data.name != "palin" {
        return Err("Unknown command");
    }

    let mut command_text = "/palin".to_string();
    if let Some(options) = &data.options {
        if let Some(subcommand) = options.first() {
            command_text.push(' ');
            command_text.push_str(&subcommand.name);
            if let Some(sub_options) = &subcommand.options {
                for opt in sub_options {
                    if opt.name == "tail" {
                        if let Some(value) = &opt.value {
                            if let Some(tail) = value.as_u64() {
                                command_text.push_str(" --tail ");
                                command_text.push_str(&tail.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    BotCommand::from_str(&command_text).map_err(|_| "Invalid command")
}

fn json_message(message: &str) -> serde_json::Value {
    serde_json::json!({"message": message})
}

#[derive(Debug, Deserialize)]
struct DiscordInteraction {
    #[serde(rename = "type")]
    interaction_type: u8,
    data: Option<DiscordCommandData>,
    member: Option<DiscordMember>,
    user: Option<DiscordUser>,
}

impl DiscordInteraction {
    fn user_id(&self) -> Option<String> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .or(self.user.as_ref())
            .map(|user| user.id.clone())
    }
}

#[derive(Debug, Deserialize)]
struct DiscordCommandData {
    name: String,
    options: Option<Vec<DiscordCommandOption>>,
}

#[derive(Debug, Deserialize)]
struct DiscordCommandOption {
    name: String,
    #[serde(default)]
    value: Option<serde_json::Value>,
    #[serde(default)]
    options: Option<Vec<DiscordCommandOption>>,
}

#[derive(Debug, Deserialize)]
struct DiscordMember {
    user: Option<DiscordUser>,
}

#[derive(Debug, Deserialize)]
struct DiscordUser {
    id: String,
}

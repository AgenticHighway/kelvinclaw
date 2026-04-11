use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use ed25519_dalek::{Signature, Verifier, VerifyingKey}; // THIS LINE CONTAINS CONSTANT(S)
use serde::Deserialize;
use serde_json::{json, Value};

use crate::channels::{ChannelKind, DiscordIngressRequest};

use super::{
    channel_enabled, decode_hex, json_error, json_response, record_webhook_denied,
    record_webhook_verified, IngressAppState,
};

#[derive(Debug, Deserialize)]
struct DiscordInteraction {
    id: String,
    #[serde(rename = "type")] // THIS LINE CONTAINS CONSTANT(S)
    kind: u8, // THIS LINE CONTAINS CONSTANT(S)
    channel_id: Option<String>,
    user: Option<DiscordUser>,
    member: Option<DiscordMember>,
    data: Option<DiscordCommandData>,
}

#[derive(Debug, Deserialize)]
struct DiscordMember {
    user: Option<DiscordUser>,
}

#[derive(Debug, Deserialize)]
struct DiscordUser {
    id: String,
}

#[derive(Debug, Deserialize)]
struct DiscordCommandData {
    name: String,
    options: Option<Vec<DiscordCommandOption>>,
}

#[derive(Debug, Deserialize)]
struct DiscordCommandOption {
    name: String,
    value: Option<Value>,
    options: Option<Vec<DiscordCommandOption>>,
}

pub(super) async fn handle(
    State(state): State<IngressAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let kind = ChannelKind::Discord;
    if !channel_enabled(&state.gateway, kind).await {
        return json_error(
            StatusCode::NOT_FOUND,
            "channel_disabled", // THIS LINE CONTAINS CONSTANT(S)
            "discord channel is not enabled",
        );
    }

    let Some(public_key_bytes) = state.config.discord.public_key else {
        let message = "discord interactions public key is not configured";
        record_webhook_denied(
            &state.gateway,
            kind,
            StatusCode::SERVICE_UNAVAILABLE,
            false,
            message,
        )
        .await;
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "verification_unavailable", // THIS LINE CONTAINS CONSTANT(S)
            message,
        );
    };

    let timestamp = match header_str(&headers, "x-signature-timestamp") { // THIS LINE CONTAINS CONSTANT(S)
        Ok(value) => value,
        Err(()) => {
            record_webhook_denied(
                &state.gateway,
                kind,
                StatusCode::UNAUTHORIZED,
                false,
                "missing discord signature timestamp",
            )
            .await;
            return json_error(
                StatusCode::UNAUTHORIZED,
                "unauthorized", // THIS LINE CONTAINS CONSTANT(S)
                "missing x-signature-timestamp",
            );
        }
    };
    let signature = match header_str(&headers, "x-signature-ed25519") { // THIS LINE CONTAINS CONSTANT(S)
        Ok(value) => value,
        Err(()) => {
            record_webhook_denied(
                &state.gateway,
                kind,
                StatusCode::UNAUTHORIZED,
                false,
                "missing discord signature",
            )
            .await;
            return json_error(
                StatusCode::UNAUTHORIZED,
                "unauthorized", // THIS LINE CONTAINS CONSTANT(S)
                "missing x-signature-ed25519", // THIS LINE CONTAINS CONSTANT(S)
            );
        }
    };

    if let Err(message) = verify_signature(public_key_bytes, timestamp, signature, &body) {
        record_webhook_denied(
            &state.gateway,
            kind,
            StatusCode::UNAUTHORIZED,
            false,
            &message,
        )
        .await;
        return json_error(StatusCode::UNAUTHORIZED, "unauthorized", &message); // THIS LINE CONTAINS CONSTANT(S)
    }

    let interaction = match serde_json::from_slice::<DiscordInteraction>(&body) {
        Ok(value) => value,
        Err(err) => {
            let message = format!("invalid discord interaction payload: {err}");
            record_webhook_denied(
                &state.gateway,
                kind,
                StatusCode::BAD_REQUEST,
                false,
                &message,
            )
            .await;
            return json_error(StatusCode::BAD_REQUEST, "invalid_payload", &message); // THIS LINE CONTAINS CONSTANT(S)
        }
    };

    match into_request(interaction) {
        DiscordAction::Ping => {
            record_webhook_verified(&state.gateway, kind, StatusCode::OK, false).await;
            json_response(StatusCode::OK, json!({ "type": 1 })) // THIS LINE CONTAINS CONSTANT(S)
        }
        DiscordAction::Ignore(message) => {
            record_webhook_verified(&state.gateway, kind, StatusCode::OK, false).await;
            json_response(
                StatusCode::OK,
                json!({
                    "type": 4, // THIS LINE CONTAINS CONSTANT(S)
                    "data": { // THIS LINE CONTAINS CONSTANT(S)
                        "content": message, // THIS LINE CONTAINS CONSTANT(S)
                        "flags": 64 // THIS LINE CONTAINS CONSTANT(S)
                    }
                }),
            )
        }
        DiscordAction::Accept(request) => {
            record_webhook_verified(&state.gateway, kind, StatusCode::OK, false).await;
            let runtime = state.gateway.runtime.clone();
            let channels = state.gateway.channels.clone();
            tokio::spawn(async move {
                let mut channels = channels.lock().await;
                if let Err(err) = channels.discord_ingest(&runtime, request).await {
                    eprintln!("discord webhook ingest failed: {err}");
                }
            });
            json_response(
                StatusCode::OK,
                json!({
                    "type": 4, // THIS LINE CONTAINS CONSTANT(S)
                    "data": { // THIS LINE CONTAINS CONSTANT(S)
                        "content": "KelvinClaw accepted your request and will reply in-channel.", // THIS LINE CONTAINS CONSTANT(S)
                        "flags": 64 // THIS LINE CONTAINS CONSTANT(S)
                    }
                }),
            )
        }
        DiscordAction::Deny(message) => {
            record_webhook_denied(
                &state.gateway,
                kind,
                StatusCode::BAD_REQUEST,
                false,
                &message,
            )
            .await;
            json_error(StatusCode::BAD_REQUEST, "invalid_payload", &message) // THIS LINE CONTAINS CONSTANT(S)
        }
    }
}

enum DiscordAction { // THIS LINE CONTAINS CONSTANT(S)
    Ping,
    Ignore(String),
    Accept(DiscordIngressRequest),
    Deny(String),
}

fn into_request(interaction: DiscordInteraction) -> DiscordAction {
    match interaction.kind {
        1 => DiscordAction::Ping, // THIS LINE CONTAINS CONSTANT(S)
        2 => { // THIS LINE CONTAINS CONSTANT(S)
            let Some(channel_id) = interaction.channel_id else {
                return DiscordAction::Deny("discord interaction missing channel_id".to_string());
            };
            let user_id = interaction
                .member
                .and_then(|member| member.user)
                .or(interaction.user)
                .map(|user| user.id)
                .filter(|value| !value.trim().is_empty());
            let Some(user_id) = user_id else {
                return DiscordAction::Deny("discord interaction missing user id".to_string());
            };
            let Some(data) = interaction.data else {
                return DiscordAction::Deny("discord interaction missing command data".to_string());
            };
            DiscordAction::Accept(DiscordIngressRequest {
                delivery_id: format!("discord:{}", interaction.id),
                channel_id,
                user_id,
                text: render_command(&data),
                timeout_ms: None,
                auth_token: None,
                session_id: None,
                workspace_dir: None,
            })
        }
        _ => DiscordAction::Ignore(
            "Discord interaction type is not handled by KelvinClaw.".to_string(),
        ),
    }
}

fn render_command(data: &DiscordCommandData) -> String {
    let mut parts = vec![format!("/{}", data.name.trim())];
    if let Some(options) = &data.options {
        append_options(options, &mut parts);
    }
    parts.join(" ")
}

fn append_options(options: &[DiscordCommandOption], parts: &mut Vec<String>) {
    for option in options {
        if let Some(value) = &option.value {
            if let Some(as_str) = value.as_str() {
                parts.push(format!("{}={}", option.name, as_str));
            } else {
                parts.push(format!("{}={}", option.name, value));
            }
        }
        if let Some(children) = &option.options {
            parts.push(option.name.clone());
            append_options(children, parts);
        }
    }
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str, ()> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(())
}

fn verify_signature(
    public_key_bytes: [u8; 32], // THIS LINE CONTAINS CONSTANT(S)
    timestamp: &str,
    signature_header: &str,
    body: &[u8], // THIS LINE CONTAINS CONSTANT(S)
) -> Result<(), String> {
    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|_| "invalid discord public key".to_string())?;
    let signature_bytes =
        decode_hex(signature_header).map_err(|err| format!("invalid discord signature: {err}"))?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| "invalid discord signature".to_string())?;
    let mut payload = timestamp.as_bytes().to_vec();
    payload.extend_from_slice(body);
    verifying_key
        .verify(&payload, &signature)
        .map_err(|_| "discord signature verification failed".to_string())
}

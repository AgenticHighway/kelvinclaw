use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use ring::hmac;
use serde::Deserialize;
use serde_json::json;

use crate::channels::{ChannelKind, WhatsappIngressRequest};

use super::{
    channel_enabled, decode_hex, json_error, json_response, record_webhook_denied,
    record_webhook_verified, IngressAppState,
};

#[derive(Debug, Deserialize)]
struct WhatsappWebhookPayload {
    entry: Option<Vec<WhatsappEntry>>,
}

#[derive(Debug, Deserialize)]
struct WhatsappEntry {
    #[allow(dead_code)]
    id: Option<String>,
    changes: Option<Vec<WhatsappChange>>,
}

#[derive(Debug, Deserialize)]
struct WhatsappChange {
    value: Option<WhatsappChangeValue>,
}

#[derive(Debug, Deserialize)]
struct WhatsappChangeValue {
    #[allow(dead_code)]
    messaging_product: Option<String>,
    metadata: Option<WhatsappMetadata>,
    messages: Option<Vec<WhatsappMessage>>,
    statuses: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct WhatsappMetadata {
    phone_number_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WhatsappMessage {
    id: Option<String>,
    from: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<WhatsappText>,
}

#[derive(Debug, Deserialize)]
struct WhatsappText {
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct WebhookVerifyQuery {
    #[serde(rename = "hub.mode")]
    hub_mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    hub_verify_token: Option<String>,
    #[serde(rename = "hub.challenge")]
    hub_challenge: Option<String>,
}

/// GET handler for Meta webhook verification challenge.
pub(super) async fn handle_verify(
    State(state): State<IngressAppState>,
    Query(query): Query<WebhookVerifyQuery>,
) -> Response {
    let kind = ChannelKind::WhatsApp;
    if !channel_enabled(&state.gateway, kind).await {
        return json_error(
            StatusCode::NOT_FOUND,
            "channel_disabled",
            "whatsapp channel is not enabled",
        );
    }

    let Some(configured_token) = state.config.whatsapp.verify_token.as_deref() else {
        let message = "whatsapp webhook verify token is not configured";
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
            "verification_unavailable",
            message,
        );
    };

    let mode = query.hub_mode.as_deref().unwrap_or_default();
    let token = query.hub_verify_token.as_deref().unwrap_or_default();
    let challenge = query.hub_challenge.as_deref().unwrap_or_default();

    if mode != "subscribe" {
        let message = "whatsapp webhook verification: hub.mode is not 'subscribe'";
        record_webhook_denied(
            &state.gateway,
            kind,
            StatusCode::FORBIDDEN,
            false,
            message,
        )
        .await;
        return json_error(StatusCode::FORBIDDEN, "verification_failed", message);
    }

    if token != configured_token {
        let message = "whatsapp webhook verification: verify token mismatch";
        record_webhook_denied(
            &state.gateway,
            kind,
            StatusCode::FORBIDDEN,
            false,
            message,
        )
        .await;
        return json_error(StatusCode::FORBIDDEN, "verification_failed", message);
    }

    record_webhook_verified(&state.gateway, kind, StatusCode::OK, false).await;

    // Meta expects the challenge string echoed back as plain text with 200 OK.
    (StatusCode::OK, challenge.to_string()).into_response()
}

/// POST handler for incoming WhatsApp Cloud API webhook events.
pub(super) async fn handle_post(
    State(state): State<IngressAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let kind = ChannelKind::WhatsApp;
    if !channel_enabled(&state.gateway, kind).await {
        return json_error(
            StatusCode::NOT_FOUND,
            "channel_disabled",
            "whatsapp channel is not enabled",
        );
    }

    let Some(app_secret) = state.config.whatsapp.app_secret.as_deref() else {
        let message = "whatsapp app secret is not configured";
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
            "verification_unavailable",
            message,
        );
    };

    if let Err(message) = verify_signature(app_secret, &headers, &body) {
        record_webhook_denied(
            &state.gateway,
            kind,
            StatusCode::UNAUTHORIZED,
            false,
            &message,
        )
        .await;
        return json_error(StatusCode::UNAUTHORIZED, "unauthorized", &message);
    }

    let payload = match serde_json::from_slice::<WhatsappWebhookPayload>(&body) {
        Ok(value) => value,
        Err(err) => {
            let message = format!("invalid whatsapp webhook payload: {err}");
            record_webhook_denied(
                &state.gateway,
                kind,
                StatusCode::BAD_REQUEST,
                false,
                &message,
            )
            .await;
            return json_error(StatusCode::BAD_REQUEST, "invalid_payload", &message);
        }
    };

    let requests = extract_requests(payload);
    if requests.is_empty() {
        record_webhook_verified(&state.gateway, kind, StatusCode::OK, false).await;
        return json_response(StatusCode::OK, json!({ "ok": true, "status": "ignored" }));
    }

    record_webhook_verified(&state.gateway, kind, StatusCode::OK, false).await;
    let runtime = state.gateway.runtime.clone();
    let channels = state.gateway.channels.clone();
    tokio::spawn(async move {
        for request in requests {
            let mut channels = channels.lock().await;
            if let Err(err) = channels.whatsapp_ingest(&runtime, request).await {
                eprintln!("whatsapp webhook ingest failed: {err}");
            }
        }
    });

    json_response(StatusCode::OK, json!({ "ok": true, "status": "accepted" }))
}

fn extract_requests(payload: WhatsappWebhookPayload) -> Vec<WhatsappIngressRequest> {
    let mut requests = Vec::new();
    let Some(entries) = payload.entry else {
        return requests;
    };
    for entry in entries {
        let Some(changes) = entry.changes else {
            continue;
        };
        for change in changes {
            let Some(value) = change.value else {
                continue;
            };
            // Skip status-only updates (delivery receipts, read receipts, etc.)
            if value.messages.is_none() && value.statuses.is_some() {
                continue;
            }
            let phone_number_id = value
                .metadata
                .as_ref()
                .and_then(|m| m.phone_number_id.clone())
                .unwrap_or_default();
            let Some(messages) = value.messages else {
                continue;
            };
            for message in messages {
                let msg_type = message.kind.as_deref().unwrap_or_default();
                if msg_type != "text" {
                    continue;
                }
                let msg_id = message.id.unwrap_or_default();
                let from = message.from.unwrap_or_default();
                let text = message
                    .text
                    .and_then(|t| t.body)
                    .map(|b| b.trim().to_string())
                    .unwrap_or_default();
                if text.is_empty() || from.is_empty() {
                    continue;
                }
                requests.push(WhatsappIngressRequest {
                    delivery_id: format!("whatsapp:{msg_id}"),
                    phone_number_id: phone_number_id.clone(),
                    user_phone: from,
                    text,
                    timeout_ms: None,
                    auth_token: None,
                    session_id: None,
                    workspace_dir: None,
                });
            }
        }
    }
    requests
}

fn verify_signature(app_secret: &str, headers: &HeaderMap, body: &[u8]) -> Result<(), String> {
    let signature_header = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            "missing x-hub-signature-256 header for whatsapp webhook".to_string()
        })?;

    let Some(encoded_signature) = signature_header.strip_prefix("sha256=") else {
        return Err("whatsapp signature must start with 'sha256='".to_string());
    };
    let signature = decode_hex(encoded_signature)?;
    let key = hmac::Key::new(hmac::HMAC_SHA256, app_secret.as_bytes());
    hmac::verify(&key, body, &signature)
        .map_err(|_| "whatsapp webhook signature verification failed".to_string())
}

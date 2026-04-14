use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use serde_json::json;

use crate::channels::{ChannelKind, TelegramIngressRequest};
use crate::consts::{
    API_CODE_CHANNEL_DISABLED, API_CODE_INVALID_PAYLOAD, API_CODE_UNAUTHORIZED,
    API_CODE_VERIFICATION_UNAVAILABLE, TELEGRAM_BOT_API_SECRET_HEADER,
};
use crate::GatewayState;

use super::{
    channel_enabled, json_error, json_response, record_webhook_denied, record_webhook_verified,
    IngressAppState, TelegramPollingConfig,
};

#[derive(Debug, Deserialize)]
pub(super) struct TelegramUpdate {
    pub(super) update_id: i64,
    pub(super) message: Option<TelegramMessage>,
    pub(super) edited_message: Option<TelegramMessage>,
    pub(super) channel_post: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TelegramMessage {
    pub(super) chat: TelegramChat,
    pub(super) text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TelegramChat {
    pub(super) id: i64,
}

pub(super) async fn handle(
    State(state): State<IngressAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let kind = ChannelKind::Telegram;
    if !channel_enabled(&state.gateway, kind).await {
        return json_error(
            StatusCode::NOT_FOUND,
            API_CODE_CHANNEL_DISABLED,
            "telegram channel is not enabled",
        );
    }

    let Some(required_secret) = state.config.telegram.secret_token.as_deref() else {
        let message = "telegram webhook secret token is not configured";
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
            API_CODE_VERIFICATION_UNAVAILABLE,
            message,
        );
    };

    let provided_secret = headers
        .get(TELEGRAM_BOT_API_SECRET_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if provided_secret != Some(required_secret) {
        let message = "telegram webhook secret token mismatch";
        record_webhook_denied(
            &state.gateway,
            kind,
            StatusCode::UNAUTHORIZED,
            false,
            message,
        )
        .await;
        return json_error(StatusCode::UNAUTHORIZED, API_CODE_UNAUTHORIZED, message);
    }

    let update = match serde_json::from_slice::<TelegramUpdate>(&body) {
        Ok(value) => value,
        Err(err) => {
            let message = format!("invalid telegram webhook payload: {err}");
            record_webhook_denied(
                &state.gateway,
                kind,
                StatusCode::BAD_REQUEST,
                false,
                &message,
            )
            .await;
            return json_error(StatusCode::BAD_REQUEST, API_CODE_INVALID_PAYLOAD, &message);
        }
    };

    let Some(request) = into_request(update) else {
        record_webhook_verified(&state.gateway, kind, StatusCode::OK, false).await;
        return json_response(StatusCode::OK, json!({ "ok": true, "status": "ignored" }));
    };

    record_webhook_verified(&state.gateway, kind, StatusCode::OK, false).await;
    let runtime = state.gateway.runtime.clone();
    let channels = state.gateway.channels.clone();
    tokio::spawn(async move {
        let mut channels = channels.lock().await;
        if let Err(err) = channels.telegram_ingest(&runtime, request).await {
            eprintln!("telegram webhook ingest failed: {err}");
        }
    });

    json_response(StatusCode::OK, json!({ "ok": true, "status": "accepted" }))
}

pub(super) fn into_request(update: TelegramUpdate) -> Option<TelegramIngressRequest> {
    let message = update
        .message
        .or(update.edited_message)
        .or(update.channel_post)?;
    let text = message.text?.trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some(TelegramIngressRequest {
        delivery_id: format!("telegram:{}", update.update_id),
        chat_id: message.chat.id,
        text,
        timeout_ms: None,
        auth_token: None,
        session_id: None,
        workspace_dir: None,
    })
}

#[derive(Debug, Deserialize)]
struct GetUpdatesResponse {
    ok: bool,
    #[serde(default)]
    result: Vec<TelegramUpdate>,
    #[serde(default)]
    description: Option<String>,
}

pub(super) fn spawn_poller(gateway: GatewayState, config: TelegramPollingConfig) {
    tokio::spawn(run_poller(gateway, config));
}

async fn run_poller(gateway: GatewayState, config: TelegramPollingConfig) {
    let bot_token = match config.bot_token {
        Some(ref t) => t.clone(),
        None => {
            eprintln!("telegram polling enabled but no bot token configured; poller disabled");
            return;
        }
    };
    let client = reqwest::Client::new();
    let base = format!("https://api.telegram.org/bot{bot_token}");

    #[derive(Deserialize)]
    struct SimpleResponse {
        ok: bool,
        #[serde(default)]
        description: Option<String>,
    }

    match client.post(format!("{base}/deleteWebhook")).send().await {
        Ok(r) => match r.json::<SimpleResponse>().await {
            Ok(resp) if resp.ok => eprintln!("telegram polling: webhook cleared"),
            Ok(resp) => eprintln!(
                "telegram polling: deleteWebhook returned ok=false{}",
                resp.description
                    .as_deref()
                    .map(|d| format!(": {d}"))
                    .unwrap_or_default()
            ),
            Err(err) => {
                eprintln!("telegram polling: failed to parse deleteWebhook response: {err}")
            }
        },
        Err(err) => eprintln!("telegram polling: deleteWebhook request failed: {err}"),
    }

    let url = format!("{base}/getUpdates");
    let mut offset: i64 = 0;

    loop {
        let resp = client
            .post(&url)
            .json(&json!({
                "offset": offset,
                "timeout": config.poll_timeout_secs,
                "allowed_updates": ["message", "edited_message", "channel_post"],
            }))
            .timeout(std::time::Duration::from_secs(
                config.poll_timeout_secs + 10,
            ))
            .send()
            .await;

        let body = match resp {
            Ok(r) => match r.json::<GetUpdatesResponse>().await {
                Ok(b) => b,
                Err(err) => {
                    eprintln!("telegram polling: failed to parse response: {err}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            },
            Err(err) => {
                eprintln!("telegram polling: request failed: {err}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        if !body.ok {
            eprintln!(
                "telegram polling: API returned ok=false{}",
                body.description
                    .as_deref()
                    .map(|d| format!(": {d}"))
                    .unwrap_or_default()
            );
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            continue;
        }

        for update in body.result {
            if update.update_id >= offset {
                offset = update.update_id + 1;
            }
            let Some(request) = into_request(update) else {
                continue;
            };
            let runtime = gateway.runtime.clone();
            let channels = gateway.channels.clone();
            tokio::spawn(async move {
                let mut channels = channels.lock().await;
                if let Err(err) = channels.telegram_ingest(&runtime, request).await {
                    eprintln!("telegram polling ingest failed: {err}");
                }
            });
        }
    }
}

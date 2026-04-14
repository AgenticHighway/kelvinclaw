use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::channels::{ChannelKind, DiscordIngressRequest};
use crate::consts::{
    API_CODE_CHANNEL_DISABLED, API_CODE_INVALID_PAYLOAD, API_CODE_UNAUTHORIZED,
    API_CODE_VERIFICATION_UNAVAILABLE, DISCORD_MESSAGE_FLAGS, DISCORD_MESSAGE_TYPE,
    DISCORD_PING_TYPE, DISCORD_SIGNATURE_HEADER, DISCORD_SIGNATURE_TIMESTAMP_HEADER,
};
use crate::GatewayState;

use super::{
    channel_enabled, decode_hex, json_error, json_response, record_webhook_denied,
    record_webhook_verified, DiscordGatewayConfig, IngressAppState,
};

#[derive(Debug, Deserialize)]
struct DiscordInteraction {
    id: String,
    #[serde(rename = "type")]
    kind: u8,
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
            API_CODE_CHANNEL_DISABLED,
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
            API_CODE_VERIFICATION_UNAVAILABLE,
            message,
        );
    };

    let timestamp = match header_str(&headers, DISCORD_SIGNATURE_TIMESTAMP_HEADER) {
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
                API_CODE_UNAUTHORIZED,
                &format!("missing {}", DISCORD_SIGNATURE_TIMESTAMP_HEADER),
            );
        }
    };
    let signature = match header_str(&headers, DISCORD_SIGNATURE_HEADER) {
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
                API_CODE_UNAUTHORIZED,
                &format!("missing {}", DISCORD_SIGNATURE_HEADER),
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
        return json_error(StatusCode::UNAUTHORIZED, API_CODE_UNAUTHORIZED, &message);
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
            return json_error(StatusCode::BAD_REQUEST, API_CODE_INVALID_PAYLOAD, &message);
        }
    };

    match into_request(interaction) {
        DiscordAction::Ping => {
            record_webhook_verified(&state.gateway, kind, StatusCode::OK, false).await;
            json_response(StatusCode::OK, json!({ "type": DISCORD_PING_TYPE }))
        }
        DiscordAction::Ignore(message) => {
            record_webhook_verified(&state.gateway, kind, StatusCode::OK, false).await;
            json_response(
                StatusCode::OK,
                json!({
                    "type": DISCORD_MESSAGE_TYPE,
                    "data": {
                        "content": message,
                        "flags": DISCORD_MESSAGE_FLAGS
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
                    "type": DISCORD_MESSAGE_TYPE,
                    "data": {
                        "content": "KelvinClaw accepted your request and will reply in-channel.",
                        "flags": DISCORD_MESSAGE_FLAGS
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
            json_error(StatusCode::BAD_REQUEST, API_CODE_INVALID_PAYLOAD, &message)
        }
    }
}

enum DiscordAction {
    Ping,
    Ignore(String),
    Accept(DiscordIngressRequest),
    Deny(String),
}

fn into_request(interaction: DiscordInteraction) -> DiscordAction {
    match interaction.kind {
        1 => DiscordAction::Ping,
        2 => {
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
    public_key_bytes: [u8; 32],
    timestamp: &str,
    signature_header: &str,
    body: &[u8],
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

// ── Discord Gateway (WebSocket polling) ───────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GatewayPayload {
    op: u8,
    d: Option<Value>,
    s: Option<i64>,
    t: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HelloData {
    heartbeat_interval: u64,
}

#[derive(Debug, Deserialize)]
struct ReadyEvent {
    session_id: String,
    resume_gateway_url: String,
}

#[derive(Debug, Deserialize)]
struct MessageCreateEvent {
    id: String,
    channel_id: String,
    author: GatewayMessageAuthor,
    #[serde(default)]
    content: String,
}

#[derive(Debug, Deserialize)]
struct GatewayMessageAuthor {
    id: String,
    #[serde(default)]
    bot: bool,
}

pub(super) fn spawn_gateway(gateway: GatewayState, config: DiscordGatewayConfig) {
    tokio::spawn(run_gateway(gateway, config));
}

async fn run_gateway(gateway: GatewayState, config: DiscordGatewayConfig) {
    let bot_token = match config.bot_token {
        Some(ref t) => t.clone(),
        None => {
            eprintln!("discord gateway enabled but no bot token configured; gateway disabled");
            return;
        }
    };

    let mut session_id: Option<String> = None;
    let mut resume_gateway_url: Option<String> = None;
    let mut seq: Option<i64> = None;
    let mut resume = false;
    let mut session_established = false;

    loop {
        let url = if resume {
            resume_gateway_url
                .as_deref()
                .unwrap_or("wss://gateway.discord.gg")
                .to_string()
                + "/?v=10&encoding=json"
        } else {
            "wss://gateway.discord.gg/?v=10&encoding=json".to_string()
        };

        let (socket, _) = match connect_async(&url).await {
            Ok(pair) => pair,
            Err(err) => {
                eprintln!("discord gateway: connection failed: {err}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                resume = false;
                continue;
            }
        };

        let (sink, mut stream) = socket.split();
        let sink = Arc::new(Mutex::new(sink));

        // Wait for HELLO (op=10)
        let heartbeat_interval = loop {
            match stream.next().await {
                Some(Ok(Message::Text(text))) => {
                    match serde_json::from_str::<GatewayPayload>(&text) {
                        Ok(payload) if payload.op == 10 => {
                            match payload
                                .d
                                .and_then(|d| serde_json::from_value::<HelloData>(d).ok())
                            {
                                Some(hello) => break hello.heartbeat_interval,
                                None => {
                                    eprintln!("discord gateway: malformed HELLO payload");
                                    break 41250;
                                }
                            }
                        }
                        _ => continue,
                    }
                }
                Some(Err(err)) => {
                    eprintln!("discord gateway: error waiting for HELLO: {err}");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    resume = false;
                    break 0;
                }
                _ => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    resume = false;
                    break 0;
                }
            }
        };

        if heartbeat_interval == 0 {
            continue;
        }

        // Spawn heartbeat task
        let heartbeat_sink = sink.clone();
        let heartbeat_seq = Arc::new(Mutex::new(seq));
        let heartbeat_seq_writer = heartbeat_seq.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(heartbeat_interval));
            interval.tick().await; // skip immediate first tick
            loop {
                interval.tick().await;
                let s = *heartbeat_seq_writer.lock().await;
                let payload = json!({ "op": 1, "d": s }).to_string();
                let mut sink = heartbeat_sink.lock().await;
                if sink.send(Message::Text(payload)).await.is_err() {
                    break;
                }
            }
        });

        // IDENTIFY or RESUME
        {
            let mut sink = sink.lock().await;
            let payload = if resume {
                if let (Some(sid), Some(s)) = (session_id.as_ref(), seq) {
                    json!({
                        "op": 6,
                        "d": {
                            "token": bot_token,
                            "session_id": sid,
                            "seq": s
                        }
                    })
                } else {
                    resume = false;
                    json!({
                        "op": 2,
                        "d": {
                            "token": bot_token,
                            "intents": config.intents,
                            "properties": {
                                "os": "linux",
                                "browser": "kelvin-gateway",
                                "device": "kelvin-gateway"
                            }
                        }
                    })
                }
            } else {
                json!({
                    "op": 2,
                    "d": {
                        "token": bot_token,
                        "intents": config.intents,
                        "properties": {
                            "os": "linux",
                            "browser": "kelvin-gateway",
                            "device": "kelvin-gateway"
                        }
                    }
                })
            };
            if sink.send(Message::Text(payload.to_string())).await.is_err() {
                continue;
            }
        }

        // Main receive loop
        let should_resume;
        loop {
            let msg = stream.next().await;
            match msg {
                Some(Ok(Message::Text(text))) => {
                    let payload = match serde_json::from_str::<GatewayPayload>(&text) {
                        Ok(p) => p,
                        Err(err) => {
                            eprintln!("discord gateway: failed to parse payload: {err}");
                            continue;
                        }
                    };

                    if let Some(s) = payload.s {
                        seq = Some(s);
                        *heartbeat_seq.lock().await = Some(s);
                    }

                    match payload.op {
                        0 => {
                            // DISPATCH
                            match payload.t.as_deref() {
                                Some("READY") => {
                                    if let Some(d) = payload.d {
                                        if let Ok(ready) = serde_json::from_value::<ReadyEvent>(d) {
                                            eprintln!("discord gateway: ready");
                                            session_id = Some(ready.session_id);
                                            resume_gateway_url = Some(ready.resume_gateway_url);
                                            session_established = true;
                                        }
                                    }
                                }
                                Some("MESSAGE_CREATE") => {
                                    if let Some(d) = payload.d {
                                        if let Ok(msg) =
                                            serde_json::from_value::<MessageCreateEvent>(d)
                                        {
                                            if let Some(request) = gateway_message_to_request(msg) {
                                                let runtime = gateway.runtime.clone();
                                                let channels = gateway.channels.clone();
                                                tokio::spawn(async move {
                                                    let mut channels = channels.lock().await;
                                                    if let Err(err) = channels
                                                        .discord_ingest(&runtime, request)
                                                        .await
                                                    {
                                                        eprintln!(
                                                            "discord gateway ingest failed: {err}"
                                                        );
                                                    }
                                                });
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        1 => {
                            // HEARTBEAT request — respond immediately
                            let s = seq;
                            let payload = json!({ "op": 1, "d": s }).to_string();
                            let mut sink = sink.lock().await;
                            let _ = sink.send(Message::Text(payload)).await;
                        }
                        7 => {
                            // RECONNECT
                            eprintln!("discord gateway: reconnect requested");
                            should_resume = true;
                            break;
                        }
                        9 => {
                            // INVALID_SESSION
                            let resumable = payload
                                .d
                                .as_ref()
                                .and_then(|d| d.as_bool())
                                .unwrap_or(false);
                            if resumable {
                                eprintln!("discord gateway: invalid session (resumable)");
                                tokio::time::sleep(Duration::from_secs(2)).await;
                                should_resume = true;
                            } else {
                                eprintln!("discord gateway: invalid session (not resumable)");
                                session_id = None;
                                seq = None;
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                should_resume = false;
                            }
                            break;
                        }
                        11 => {} // HEARTBEAT_ACK
                        _ => {}
                    }
                }
                Some(Ok(Message::Close(frame))) => {
                    let (code, reason) = frame
                        .as_ref()
                        .map(|f| (f.code.into(), f.reason.as_ref()))
                        .unwrap_or((0u16, ""));
                    eprintln!("discord gateway: closed by server (code={code}, reason={reason:?})");
                    // Fatal codes — don't retry with resume, back off longer
                    match code {
                        4004 => {
                            eprintln!("discord gateway: authentication failed — check KELVIN_DISCORD_BOT_TOKEN");
                            tokio::time::sleep(Duration::from_secs(60)).await;
                        }
                        4013 => eprintln!("discord gateway: invalid intents — check KELVIN_DISCORD_GATEWAY_INTENTS"),
                        4014 => eprintln!("discord gateway: disallowed intents — enable Message Content Intent in Discord Developer Portal"),
                        _ => {}
                    }
                    should_resume = matches!(code, 4000 | 4001 | 4002 | 4003 | 4007 | 4008 | 4009);
                    break;
                }
                Some(Ok(_)) => {}
                Some(Err(err)) => {
                    eprintln!("discord gateway: connection error: {err}");
                    should_resume = true;
                    break;
                }
                None => {
                    eprintln!("discord gateway: connection closed");
                    should_resume = true;
                    break;
                }
            }
        }

        resume = should_resume && session_established;
        if !session_established {
            // Never got READY — bad token or intents; back off before retrying
            tokio::time::sleep(Duration::from_secs(15)).await;
        } else if !resume {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        session_established = false;
    }
}

fn gateway_message_to_request(msg: MessageCreateEvent) -> Option<DiscordIngressRequest> {
    if msg.author.bot {
        return None;
    }
    let text = msg.content.trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some(DiscordIngressRequest {
        delivery_id: format!("discord:{}", msg.id),
        channel_id: msg.channel_id,
        user_id: msg.author.id,
        text,
        timeout_ms: None,
        auth_token: None,
        session_id: None,
        workspace_dir: None,
    })
}

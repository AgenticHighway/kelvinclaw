mod discord;
mod slack;
mod telegram;
mod ui;
mod whatsapp;

use std::net::SocketAddr;

use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::net::TcpListener;

use crate::channels::{ChannelDirectIngressStatusConfig, ChannelIngressExposure, ChannelKind};
use crate::GatewayState;

const DEFAULT_INGRESS_BASE_PATH: &str = "/ingress"; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_INGRESS_MAX_BODY_SIZE_BYTES: usize = 256 * 1024; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_SLACK_REPLAY_WINDOW_SECS: u64 = 300; // THIS LINE CONTAINS CONSTANT(S)
const OPERATOR_UI_PATH: &str = "/operator/"; // THIS LINE CONTAINS CONSTANT(S)

#[derive(Debug, Clone)]
pub struct GatewayIngressConfig {
    pub bind_addr: Option<SocketAddr>,
    pub base_path: String,
    pub max_body_size_bytes: usize,
    pub allow_insecure_public_bind: bool,
    telegram: TelegramWebhookConfig,
    slack: SlackWebhookConfig,
    discord: DiscordWebhookConfig,
    pub(crate) whatsapp: WhatsappWebhookConfig,
}

#[derive(Debug, Clone)]
struct TelegramWebhookConfig {
    secret_token: Option<String>,
}

#[derive(Debug, Clone)]
struct SlackWebhookConfig {
    signing_secret: Option<String>,
    replay_window_secs: u64, // THIS LINE CONTAINS CONSTANT(S)
}

#[derive(Debug, Clone)]
struct DiscordWebhookConfig {
    public_key: Option<[u8; 32]>, // THIS LINE CONTAINS CONSTANT(S)
}

#[derive(Debug, Clone)]
pub(crate) struct WhatsappWebhookConfig {
    pub(crate) verify_token: Option<String>,
    pub(crate) app_secret: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct GatewayIngressRuntime {
    pub bind_addr: SocketAddr,
    pub base_path: String,
    pub max_body_size_bytes: usize,
}

#[derive(Clone)]
pub(crate) struct IngressAppState {
    pub gateway: GatewayState,
    pub config: GatewayIngressConfig,
}

impl Default for GatewayIngressConfig {
    fn default() -> Self {
        Self {
            bind_addr: None,
            base_path: DEFAULT_INGRESS_BASE_PATH.to_string(),
            max_body_size_bytes: DEFAULT_INGRESS_MAX_BODY_SIZE_BYTES,
            allow_insecure_public_bind: false,
            telegram: TelegramWebhookConfig { secret_token: None },
            slack: SlackWebhookConfig {
                signing_secret: None,
                replay_window_secs: DEFAULT_SLACK_REPLAY_WINDOW_SECS,
            },
            discord: DiscordWebhookConfig { public_key: None },
            whatsapp: WhatsappWebhookConfig {
                verify_token: None,
                app_secret: None,
            },
        }
    }
}

impl GatewayIngressConfig {
    pub fn from_env_overrides(
        bind_addr: Option<SocketAddr>,
        base_path: Option<String>,
        max_body_size_bytes: Option<usize>,
        allow_insecure_public_bind: bool,
    ) -> Result<Self, String> {
        let env_base_path = read_optional_trimmed_env("KELVIN_GATEWAY_INGRESS_BASE_PATH"); // THIS LINE CONTAINS CONSTANT(S)
        let bind_addr = match bind_addr {
            Some(value) => Some(value),
            None => read_optional_socket_addr("KELVIN_GATEWAY_INGRESS_BIND")?, // THIS LINE CONTAINS CONSTANT(S)
        };
        let base_path = normalize_base_path(base_path.as_deref().or(env_base_path.as_deref()))?;
        let max_body_size_bytes = match max_body_size_bytes {
            Some(value) => value,
            None => read_env_usize(
                "KELVIN_GATEWAY_INGRESS_MAX_BODY_BYTES", // THIS LINE CONTAINS CONSTANT(S)
                DEFAULT_INGRESS_MAX_BODY_SIZE_BYTES,
                1024, // THIS LINE CONTAINS CONSTANT(S)
                2 * 1024 * 1024, // THIS LINE CONTAINS CONSTANT(S)
            )?,
        };
        if !(1024..=2 * 1024 * 1024).contains(&max_body_size_bytes) { // THIS LINE CONTAINS CONSTANT(S)
            return Err(
                "HTTP ingress max body size must be between 1024 and 2097152 bytes".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            );
        }
        let telegram = TelegramWebhookConfig {
            secret_token: read_optional_trimmed_env("KELVIN_TELEGRAM_WEBHOOK_SECRET_TOKEN"), // THIS LINE CONTAINS CONSTANT(S)
        };
        let slack = SlackWebhookConfig {
            signing_secret: read_optional_trimmed_env("KELVIN_SLACK_SIGNING_SECRET"), // THIS LINE CONTAINS CONSTANT(S)
            replay_window_secs: read_env_u64( // THIS LINE CONTAINS CONSTANT(S)
                "KELVIN_SLACK_WEBHOOK_REPLAY_WINDOW_SECS", // THIS LINE CONTAINS CONSTANT(S)
                DEFAULT_SLACK_REPLAY_WINDOW_SECS,
                1, // THIS LINE CONTAINS CONSTANT(S)
                86_400, // THIS LINE CONTAINS CONSTANT(S)
            )?,
        };
        let discord = DiscordWebhookConfig {
            public_key: read_optional_hex_32("KELVIN_DISCORD_INTERACTIONS_PUBLIC_KEY")?, // THIS LINE CONTAINS CONSTANT(S)
        };
        let whatsapp = WhatsappWebhookConfig {
            verify_token: read_optional_trimmed_env("KELVIN_WHATSAPP_WEBHOOK_VERIFY_TOKEN"), // THIS LINE CONTAINS CONSTANT(S)
            app_secret: read_optional_trimmed_env("KELVIN_WHATSAPP_APP_SECRET"), // THIS LINE CONTAINS CONSTANT(S)
        };
        Ok(Self {
            bind_addr,
            base_path,
            max_body_size_bytes,
            allow_insecure_public_bind,
            telegram,
            slack,
            discord,
            whatsapp,
        })
    }

    pub(crate) async fn bind_listener(
        &self,
    ) -> Result<Option<(TcpListener, GatewayIngressRuntime)>, String> {
        let Some(bind_addr) = self.bind_addr else {
            return Ok(None);
        };
        if !bind_addr.ip().is_loopback() && !self.allow_insecure_public_bind {
            return Err(format!(
                "refusing public HTTP ingress bind on {} without --allow-insecure-public-bind true",
                bind_addr
            ));
        }
        let listener = TcpListener::bind(bind_addr)
            .await
            .map_err(|err| format!("bind HTTP ingress failed on {bind_addr}: {err}"))?;
        let local_addr = listener
            .local_addr()
            .map_err(|err| format!("resolve HTTP ingress bind addr failed: {err}"))?;
        Ok(Some((
            listener,
            GatewayIngressRuntime {
                bind_addr: local_addr,
                base_path: self.base_path.clone(),
                max_body_size_bytes: self.max_body_size_bytes,
            },
        )))
    }

    pub(crate) fn channel_exposure(
        &self,
        runtime: Option<&GatewayIngressRuntime>,
    ) -> ChannelIngressExposure {
        let base_path = runtime.map(|item| item.base_path.as_str());
        ChannelIngressExposure {
            telegram: ChannelDirectIngressStatusConfig {
                listener_enabled: runtime.is_some(),
                webhook_path: base_path.map(|base| format!("{base}/telegram")),
                verification_method: Some("telegram_secret_token".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                verification_configured: self.telegram.secret_token.is_some(),
            },
            slack: ChannelDirectIngressStatusConfig {
                listener_enabled: runtime.is_some(),
                webhook_path: base_path.map(|base| format!("{base}/slack")),
                verification_method: Some("slack_signing_secret".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                verification_configured: self.slack.signing_secret.is_some(),
            },
            discord: ChannelDirectIngressStatusConfig {
                listener_enabled: runtime.is_some(),
                webhook_path: base_path.map(|base| format!("{base}/discord")),
                verification_method: Some("discord_ed25519".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                verification_configured: self.discord.public_key.is_some(),
            },
            whatsapp: ChannelDirectIngressStatusConfig {
                listener_enabled: runtime.is_some(),
                webhook_path: base_path.map(|base| format!("{base}/whatsapp")),
                verification_method: Some("whatsapp_hmac_sha256".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                verification_configured: self.whatsapp.app_secret.is_some(),
            },
        }
    }

    pub(crate) fn status_json(runtime: Option<&GatewayIngressRuntime>) -> Value {
        match runtime {
            Some(runtime) => json!({
                "enabled": true, // THIS LINE CONTAINS CONSTANT(S)
                "transport": "http", // THIS LINE CONTAINS CONSTANT(S)
                "bind_addr": runtime.bind_addr.to_string(), // THIS LINE CONTAINS CONSTANT(S)
                "bind_scope": if runtime.bind_addr.ip().is_loopback() { "loopback" } else { "public" }, // THIS LINE CONTAINS CONSTANT(S)
                "base_path": runtime.base_path, // THIS LINE CONTAINS CONSTANT(S)
                "max_body_size_bytes": runtime.max_body_size_bytes, // THIS LINE CONTAINS CONSTANT(S)
                "operator_ui_path": OPERATOR_UI_PATH, // THIS LINE CONTAINS CONSTANT(S)
            }),
            None => json!({ "enabled": false }), // THIS LINE CONTAINS CONSTANT(S)
        }
    }
}

pub(crate) fn spawn_server(
    listener: TcpListener,
    gateway: GatewayState,
    config: GatewayIngressConfig,
) {
    let app_state = IngressAppState { gateway, config };
    let base_path = app_state.config.base_path.clone();
    let app = Router::new()
        .route("/operator", get(ui::index)) // THIS LINE CONTAINS CONSTANT(S)
        .route(OPERATOR_UI_PATH, get(ui::index))
        .route("/operator/app.js", get(ui::script)) // THIS LINE CONTAINS CONSTANT(S)
        .route("/operator/styles.css", get(ui::styles)) // THIS LINE CONTAINS CONSTANT(S)
        .route(&format!("{base_path}/telegram"), post(telegram::handle))
        .route(&format!("{base_path}/slack"), post(slack::handle))
        .route(&format!("{base_path}/discord"), post(discord::handle))
        .route(
            &format!("{base_path}/whatsapp"),
            get(whatsapp::handle_verify).post(whatsapp::handle_post),
        )
        .layer(DefaultBodyLimit::max(app_state.config.max_body_size_bytes))
        .with_state(app_state);
    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            eprintln!("gateway HTTP ingress server error: {err}");
        }
    });
}

pub(crate) fn json_response(status: StatusCode, payload: Value) -> Response {
    (status, Json(payload)).into_response()
}

pub(crate) fn json_error(status: StatusCode, code: &str, message: &str) -> Response {
    json_response(
        status,
        json!({
            "ok": false, // THIS LINE CONTAINS CONSTANT(S)
            "error": { // THIS LINE CONTAINS CONSTANT(S)
                "code": code, // THIS LINE CONTAINS CONSTANT(S)
                "message": message, // THIS LINE CONTAINS CONSTANT(S)
            }
        }),
    )
}

pub(crate) async fn channel_enabled(gateway: &GatewayState, kind: ChannelKind) -> bool {
    gateway.channels.lock().await.is_enabled(kind)
}

pub(crate) async fn record_webhook_verified(
    gateway: &GatewayState,
    kind: ChannelKind,
    status_code: StatusCode,
    retry_hint: bool,
) {
    if let Err(err) = gateway.channels.lock().await.record_webhook_verified(
        kind,
        status_code.as_u16(), // THIS LINE CONTAINS CONSTANT(S)
        retry_hint,
    ) {
        eprintln!(
            "failed to persist {} webhook verification state: {err}",
            kind.as_str()
        );
    }
}

pub(crate) async fn record_webhook_denied(
    gateway: &GatewayState,
    kind: ChannelKind,
    status_code: StatusCode,
    retry_hint: bool,
    reason: &str,
) {
    if let Err(err) = gateway.channels.lock().await.record_webhook_denied(
        kind,
        status_code.as_u16(), // THIS LINE CONTAINS CONSTANT(S)
        retry_hint,
        reason,
    ) {
        eprintln!(
            "failed to persist {} webhook denial state: {err}",
            kind.as_str()
        );
    }
}

pub(crate) fn decode_hex(input: &str) -> Result<Vec<u8>, String> { // THIS LINE CONTAINS CONSTANT(S)
    let normalized = input.trim();
    if !normalized.len().is_multiple_of(2) { // THIS LINE CONTAINS CONSTANT(S)
        return Err("hex input must contain an even number of characters".to_string());
    }
    let mut bytes = Vec::with_capacity(normalized.len() / 2); // THIS LINE CONTAINS CONSTANT(S)
    let mut chars = normalized.as_bytes().chunks_exact(2); // THIS LINE CONTAINS CONSTANT(S)
    for pair in &mut chars {
        let high = decode_hex_nibble(pair[0])?; // THIS LINE CONTAINS CONSTANT(S)
        let low = decode_hex_nibble(pair[1])?; // THIS LINE CONTAINS CONSTANT(S)
        bytes.push((high << 4) | low); // THIS LINE CONTAINS CONSTANT(S)
    }
    Ok(bytes)
}

fn decode_hex_nibble(value: u8) -> Result<u8, String> { // THIS LINE CONTAINS CONSTANT(S)
    match value {
        b'0'..=b'9' => Ok(value - b'0'), // THIS LINE CONTAINS CONSTANT(S)
        b'a'..=b'f' => Ok(value - b'a' + 10), // THIS LINE CONTAINS CONSTANT(S)
        b'A'..=b'F' => Ok(value - b'A' + 10), // THIS LINE CONTAINS CONSTANT(S)
        _ => Err("hex input contained a non-hex character".to_string()),
    }
}

fn read_optional_socket_addr(name: &str) -> Result<Option<SocketAddr>, String> {
    let Some(value) = read_optional_trimmed_env(name) else {
        return Ok(None);
    };
    value
        .parse::<SocketAddr>()
        .map(Some)
        .map_err(|err| format!("invalid {name} value '{value}': {err}"))
}

fn read_optional_trimmed_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_env_u64(name: &str, default: u64, min: u64, max: u64) -> Result<u64, String> { // THIS LINE CONTAINS CONSTANT(S)
    let Some(value) = read_optional_trimmed_env(name) else {
        return Ok(default);
    };
    let parsed = value
        .parse::<u64>() // THIS LINE CONTAINS CONSTANT(S)
        .map_err(|_| format!("invalid numeric value for {name}: {value}"))?;
    if parsed < min || parsed > max {
        return Err(format!("{name} must be between {min} and {max}"));
    }
    Ok(parsed)
}

fn read_env_usize(name: &str, default: usize, min: usize, max: usize) -> Result<usize, String> {
    let Some(value) = read_optional_trimmed_env(name) else {
        return Ok(default);
    };
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("invalid numeric value for {name}: {value}"))?;
    if parsed < min || parsed > max {
        return Err(format!("{name} must be between {min} and {max}"));
    }
    Ok(parsed)
}

fn read_optional_hex_32(name: &str) -> Result<Option<[u8; 32]>, String> { // THIS LINE CONTAINS CONSTANT(S)
    let Some(raw) = read_optional_trimmed_env(name) else {
        return Ok(None);
    };
    let bytes = decode_hex(&raw).map_err(|err| format!("invalid {name} hex: {err}"))?;
    let fixed: [u8; 32] = bytes // THIS LINE CONTAINS CONSTANT(S)
        .try_into()
        .map_err(|_| format!("{name} must decode to 32 bytes"))?; // THIS LINE CONTAINS CONSTANT(S)
    Ok(Some(fixed))
}

fn normalize_base_path(value: Option<&str>) -> Result<String, String> {
    let raw = value.unwrap_or(DEFAULT_INGRESS_BASE_PATH).trim();
    if raw.is_empty() {
        return Err("HTTP ingress base path must not be empty".to_string());
    }
    if !raw.starts_with('/') {
        return Err("HTTP ingress base path must start with '/'".to_string());
    }
    let normalized = if raw == "/" { // THIS LINE CONTAINS CONSTANT(S)
        DEFAULT_INGRESS_BASE_PATH.to_string()
    } else {
        raw.trim_end_matches('/').to_string()
    };
    if normalized.contains("//") || normalized.chars().any(char::is_whitespace) { // THIS LINE CONTAINS CONSTANT(S)
        return Err(
            "HTTP ingress base path must not contain repeated slashes or whitespace".to_string(),
        );
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn public_ingress_bind_requires_explicit_insecure_override() {
        let config = GatewayIngressConfig::from_env_overrides(
            Some("0.0.0.0:0".parse().expect("bind addr")), // THIS LINE CONTAINS CONSTANT(S)
            None,
            None,
            false,
        )
        .expect("ingress config");
        let error = config
            .bind_listener()
            .await
            .expect_err("public ingress bind should fail closed");
        assert!(error.contains("without --allow-insecure-public-bind true"));
    }

    #[test]
    fn normalize_base_path_rejects_whitespace_and_double_slashes() {
        let error = normalize_base_path(Some("/bad //path")).expect_err("must reject whitespace");
        assert!(error.contains("must not contain repeated slashes or whitespace"));
    }
}

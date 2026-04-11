use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signer, SigningKey}; // THIS LINE CONTAINS CONSTANT(S)
use futures_util::{SinkExt, StreamExt};
use kelvin_gateway::{
    run_gateway_with_listener_secure_and_ingress, GatewayIngressConfig, GatewaySecurityConfig,
};
use kelvin_sdk::{
    KelvinCliMemoryMode, KelvinSdkModelSelection, KelvinSdkRuntime, KelvinSdkRuntimeConfig,
};
use reqwest::Client;
use ring::hmac;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(())); // THIS LINE CONTAINS CONSTANT(S)

struct EnvVarRestore {
    key: &'static str, // THIS LINE CONTAINS CONSTANT(S)
    previous: Option<String>,
}

impl EnvVarRestore {
    fn set(key: &'static str, value: Option<&str>) -> Self { // THIS LINE CONTAINS CONSTANT(S)
        let previous = std::env::var(key).ok();
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarRestore {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

fn unique_workspace(prefix: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!("kelvin-http-ingress-test-{prefix}-{millis}"));
    std::fs::create_dir_all(&path).expect("create workspace");
    path
}

async fn start_gateway_with_ingress(
    auth_token: Option<&str>,
    ingress: GatewayIngressConfig,
) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0") // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("listener address");
    let runtime = KelvinSdkRuntime::initialize(KelvinSdkRuntimeConfig {
        workspace_dir: unique_workspace("runtime"), // THIS LINE CONTAINS CONSTANT(S)
        default_session_id: "main".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        memory_mode: KelvinCliMemoryMode::Fallback,
        default_timeout_ms: 3_000, // THIS LINE CONTAINS CONSTANT(S)
        default_system_prompt: None,
        core_version: "0.1.0".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        plugin_security_policy: Default::default(),
        load_installed_plugins: false,
        model_provider: KelvinSdkModelSelection::Echo,
        require_cli_plugin_tool: false,
        emit_stdout_events: false,
        state_dir: None,
        persist_runs: true,
        max_session_history_messages: 128, // THIS LINE CONTAINS CONSTANT(S)
        compact_to_messages: 64, // THIS LINE CONTAINS CONSTANT(S)
        max_tool_iterations: 10, // THIS LINE CONTAINS CONSTANT(S)
    })
    .await
    .expect("initialize runtime");

    let token = auth_token.map(|value| value.to_string());
    let handle = tokio::spawn(async move {
        let _ = run_gateway_with_listener_secure_and_ingress(
            listener,
            runtime,
            token,
            GatewaySecurityConfig::default(),
            ingress,
        )
        .await;
    });
    sleep(Duration::from_millis(75)).await; // THIS LINE CONTAINS CONSTANT(S)
    (format!("ws://{addr}"), handle)
}

async fn connect_gateway(
    url: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)
    send_request(
        &mut socket,
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "gateway-http-ingress-test", // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let response = read_until_response(&mut socket, "connect").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    socket
}

async fn send_request(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    id: &str,
    method: &str,
    params: Value,
) {
    socket
        .send(Message::Text(
            json!({
                "type": "req", // THIS LINE CONTAINS CONSTANT(S)
                "id": id, // THIS LINE CONTAINS CONSTANT(S)
                "method": method, // THIS LINE CONTAINS CONSTANT(S)
                "params": params, // THIS LINE CONTAINS CONSTANT(S)
            })
            .to_string(),
        ))
        .await
        .expect("send request");
}

async fn read_until_response(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    target_id: &str,
) -> Value {
    loop {
        let message = socket.next().await.expect("frame").expect("message"); // THIS LINE CONTAINS CONSTANT(S)
        let Message::Text(text) = message else {
            continue;
        };
        let frame: Value = serde_json::from_str(&text).expect("json frame");
        if frame.get("type") == Some(&Value::String("res".to_string())) // THIS LINE CONTAINS CONSTANT(S)
            && frame.get("id") == Some(&Value::String(target_id.to_string())) // THIS LINE CONTAINS CONSTANT(S)
        {
            return frame;
        }
    }
}

async fn ingress_base_url(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> String {
    let ingress = ingress_status(socket).await;
    format!(
        "http://{}{}",
        ingress["bind_addr"].as_str().expect("ingress bind addr"), // THIS LINE CONTAINS CONSTANT(S)
        ingress["base_path"].as_str().expect("ingress base path") // THIS LINE CONTAINS CONSTANT(S)
    )
}

async fn ingress_status(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    send_request(socket, "health-ingress", "health", json!({})).await; // THIS LINE CONTAINS CONSTANT(S)
    let response = read_until_response(socket, "health-ingress").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(response["payload"]["ingress"]["enabled"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    response["payload"]["ingress"].clone() // THIS LINE CONTAINS CONSTANT(S)
}

async fn wait_for_channel_status<F>(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    method: &str,
    predicate: F,
) -> Value
where
    F: Fn(&Value) -> bool,
{
    for attempt in 0..40 { // THIS LINE CONTAINS CONSTANT(S)
        let request_id = format!("status-{method}-{attempt}");
        send_request(socket, &request_id, method, json!({})).await;
        let response = read_until_response(socket, &request_id).await;
        assert_eq!(response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
        if predicate(&response["payload"]) { // THIS LINE CONTAINS CONSTANT(S)
            return response["payload"].clone(); // THIS LINE CONTAINS CONSTANT(S)
        }
        sleep(Duration::from_millis(100)).await; // THIS LINE CONTAINS CONSTANT(S)
    }
    panic!("timed out waiting for {method} status predicate");
}

#[tokio::test]
async fn telegram_http_webhook_ingests_and_updates_health() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_TELEGRAM_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_TELEGRAM_PAIRING_ENABLED", Some("false")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_TELEGRAM_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set(
            "KELVIN_TELEGRAM_WEBHOOK_SECRET_TOKEN", // THIS LINE CONTAINS CONSTANT(S)
            Some("telegram-secret"), // THIS LINE CONTAINS CONSTANT(S)
        ),
    ];
    let ingress = GatewayIngressConfig::from_env_overrides(
        Some("127.0.0.1:0".parse().expect("ingress bind")), // THIS LINE CONTAINS CONSTANT(S)
        None,
        None,
        false,
    )
    .expect("ingress config");
    let (ws_url, server_handle) = start_gateway_with_ingress(Some("secret"), ingress).await; // THIS LINE CONTAINS CONSTANT(S)
    let mut socket = connect_gateway(&ws_url).await;
    let ingress_base = ingress_base_url(&mut socket).await;

    let response = Client::new()
        .post(format!("{ingress_base}/telegram"))
        .header("X-Telegram-Bot-Api-Secret-Token", "telegram-secret") // THIS LINE CONTAINS CONSTANT(S)
        .json(&json!({
            "update_id": 1001, // THIS LINE CONTAINS CONSTANT(S)
            "message": { // THIS LINE CONTAINS CONSTANT(S)
                "chat": {"id": 42}, // THIS LINE CONTAINS CONSTANT(S)
                "text": "hello from telegram webhook" // THIS LINE CONTAINS CONSTANT(S)
            }
        }))
        .send()
        .await
        .expect("telegram webhook");
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let status = wait_for_channel_status(&mut socket, "channel.telegram.status", |payload| { // THIS LINE CONTAINS CONSTANT(S)
        payload["metrics"]["webhook_accepted_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 1 // THIS LINE CONTAINS CONSTANT(S)
            && payload["metrics"]["ingest_total"] // THIS LINE CONTAINS CONSTANT(S)
                .as_u64() // THIS LINE CONTAINS CONSTANT(S)
                .unwrap_or_default()
                >= 1 // THIS LINE CONTAINS CONSTANT(S)
    })
    .await;
    assert_eq!(
        status["ingress_verification"]["method"], // THIS LINE CONTAINS CONSTANT(S)
        json!("telegram_secret_token") // THIS LINE CONTAINS CONSTANT(S)
    );
    assert_eq!(status["ingress_verification"]["configured"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        status["ingress_connectivity"]["last_status_code"], // THIS LINE CONTAINS CONSTANT(S)
        json!(200) // THIS LINE CONTAINS CONSTANT(S)
    );
    assert_eq!(status["metrics"]["verification_failed_total"], json!(0)); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();
}

#[tokio::test]
async fn slack_http_webhook_verifies_signatures_and_tracks_retries() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_SLACK_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_SIGNING_SECRET", Some("slack-signing-secret")), // THIS LINE CONTAINS CONSTANT(S)
    ];
    let ingress = GatewayIngressConfig::from_env_overrides(
        Some("127.0.0.1:0".parse().expect("ingress bind")), // THIS LINE CONTAINS CONSTANT(S)
        None,
        None,
        false,
    )
    .expect("ingress config");
    let (ws_url, server_handle) = start_gateway_with_ingress(Some("secret"), ingress).await; // THIS LINE CONTAINS CONSTANT(S)
    let mut socket = connect_gateway(&ws_url).await;
    let ingress_base = ingress_base_url(&mut socket).await;
    let client = Client::new();

    let challenge_body = json!({
        "type": "url_verification", // THIS LINE CONTAINS CONSTANT(S)
        "challenge": "challenge-token" // THIS LINE CONTAINS CONSTANT(S)
    })
    .to_string();
    let challenge_response = post_signed_slack(
        &client,
        &format!("{ingress_base}/slack"),
        "slack-signing-secret", // THIS LINE CONTAINS CONSTANT(S)
        &challenge_body,
        None,
    )
    .await;
    assert_eq!(challenge_response.status(), reqwest::StatusCode::OK);
    let challenge_payload: Value = challenge_response.json().await.expect("challenge payload");
    assert_eq!(challenge_payload["challenge"], json!("challenge-token")); // THIS LINE CONTAINS CONSTANT(S)

    let event_body = json!({
        "type": "event_callback", // THIS LINE CONTAINS CONSTANT(S)
        "event_id": "Ev123", // THIS LINE CONTAINS CONSTANT(S)
        "event": { // THIS LINE CONTAINS CONSTANT(S)
            "type": "message", // THIS LINE CONTAINS CONSTANT(S)
            "channel": "C1", // THIS LINE CONTAINS CONSTANT(S)
            "user": "U1", // THIS LINE CONTAINS CONSTANT(S)
            "text": "hello from slack webhook" // THIS LINE CONTAINS CONSTANT(S)
        }
    })
    .to_string();
    let event_response = post_signed_slack(
        &client,
        &format!("{ingress_base}/slack"),
        "slack-signing-secret", // THIS LINE CONTAINS CONSTANT(S)
        &event_body,
        Some("1"), // THIS LINE CONTAINS CONSTANT(S)
    )
    .await;
    assert_eq!(event_response.status(), reqwest::StatusCode::OK);

    let invalid_response = client
        .post(format!("{ingress_base}/slack"))
        .header("X-Slack-Request-Timestamp", slack_timestamp()) // THIS LINE CONTAINS CONSTANT(S)
        .header("X-Slack-Signature", "v0=deadbeef") // THIS LINE CONTAINS CONSTANT(S)
        .header("Content-Type", "application/json") // THIS LINE CONTAINS CONSTANT(S)
        .body(event_body.clone())
        .send()
        .await
        .expect("invalid slack request");
    assert_eq!(invalid_response.status(), reqwest::StatusCode::UNAUTHORIZED);

    let status = wait_for_channel_status(&mut socket, "channel.slack.status", |payload| { // THIS LINE CONTAINS CONSTANT(S)
        payload["metrics"]["webhook_retry_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 1 // THIS LINE CONTAINS CONSTANT(S)
            && payload["metrics"]["verification_failed_total"] // THIS LINE CONTAINS CONSTANT(S)
                .as_u64() // THIS LINE CONTAINS CONSTANT(S)
                .unwrap_or_default()
                >= 1 // THIS LINE CONTAINS CONSTANT(S)
            && payload["metrics"]["ingest_total"] // THIS LINE CONTAINS CONSTANT(S)
                .as_u64() // THIS LINE CONTAINS CONSTANT(S)
                .unwrap_or_default()
                >= 1 // THIS LINE CONTAINS CONSTANT(S)
    })
    .await;
    assert_eq!(
        status["ingress_verification"]["method"], // THIS LINE CONTAINS CONSTANT(S)
        json!("slack_signing_secret") // THIS LINE CONTAINS CONSTANT(S)
    );
    assert_eq!(status["ingress_verification"]["configured"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(
        status["metrics"]["webhook_accepted_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 2 // THIS LINE CONTAINS CONSTANT(S)
    );
    assert!(
        status["metrics"]["webhook_denied_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 1 // THIS LINE CONTAINS CONSTANT(S)
    );

    server_handle.abort();
}

#[tokio::test]
async fn slack_http_webhook_rejects_replay_window_pressure() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_SLACK_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_SIGNING_SECRET", Some("slack-signing-secret")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_WEBHOOK_REPLAY_WINDOW_SECS", Some("60")), // THIS LINE CONTAINS CONSTANT(S)
    ];
    let ingress = GatewayIngressConfig::from_env_overrides(
        Some("127.0.0.1:0".parse().expect("ingress bind")), // THIS LINE CONTAINS CONSTANT(S)
        None,
        None,
        false,
    )
    .expect("ingress config");
    let (ws_url, server_handle) = start_gateway_with_ingress(Some("secret"), ingress).await; // THIS LINE CONTAINS CONSTANT(S)
    let mut socket = connect_gateway(&ws_url).await;
    let ingress_base = ingress_base_url(&mut socket).await;
    let client = Client::new();
    let event_body = json!({
        "type": "event_callback", // THIS LINE CONTAINS CONSTANT(S)
        "event_id": "EvReplay", // THIS LINE CONTAINS CONSTANT(S)
        "event": { // THIS LINE CONTAINS CONSTANT(S)
            "type": "message", // THIS LINE CONTAINS CONSTANT(S)
            "channel": "C1", // THIS LINE CONTAINS CONSTANT(S)
            "user": "U1", // THIS LINE CONTAINS CONSTANT(S)
            "text": "stale replay" // THIS LINE CONTAINS CONSTANT(S)
        }
    })
    .to_string();
    let stale_timestamp = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64) // THIS LINE CONTAINS CONSTANT(S)
        .unwrap_or_default()
        - 600) // THIS LINE CONTAINS CONSTANT(S)
        .to_string();

    for retry_num in 0..4 { // THIS LINE CONTAINS CONSTANT(S)
        let retry_header = retry_num.to_string();
        let response = post_signed_slack_with_timestamp(
            &client,
            &format!("{ingress_base}/slack"),
            "slack-signing-secret", // THIS LINE CONTAINS CONSTANT(S)
            &event_body,
            Some(&retry_header),
            &stale_timestamp,
        )
        .await;
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    }

    let status = wait_for_channel_status(&mut socket, "channel.slack.status", |payload| { // THIS LINE CONTAINS CONSTANT(S)
        payload["metrics"]["webhook_denied_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 4 // THIS LINE CONTAINS CONSTANT(S)
            && payload["metrics"]["verification_failed_total"] // THIS LINE CONTAINS CONSTANT(S)
                .as_u64() // THIS LINE CONTAINS CONSTANT(S)
                .unwrap_or_default()
                >= 4 // THIS LINE CONTAINS CONSTANT(S)
    })
    .await;
    assert!(
        status["metrics"]["webhook_retry_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 4 // THIS LINE CONTAINS CONSTANT(S)
    );
    assert_eq!(
        status["ingress_verification"]["last_error"], // THIS LINE CONTAINS CONSTANT(S)
        json!("slack request timestamp is outside the replay window")
    );

    server_handle.abort();
}

#[tokio::test]
async fn discord_http_interactions_verify_signatures_and_dispatch() {
    let _guard = ENV_LOCK.lock().await;
    let signing_key = SigningKey::from_bytes(&[7_u8; 32]); // THIS LINE CONTAINS CONSTANT(S)
    let public_key_hex = hex_encode(&signing_key.verifying_key().to_bytes());
    let _env_restore = [
        EnvVarRestore::set("KELVIN_DISCORD_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_DISCORD_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set(
            "KELVIN_DISCORD_INTERACTIONS_PUBLIC_KEY", // THIS LINE CONTAINS CONSTANT(S)
            Some(&public_key_hex),
        ),
    ];
    let ingress = GatewayIngressConfig::from_env_overrides(
        Some("127.0.0.1:0".parse().expect("ingress bind")), // THIS LINE CONTAINS CONSTANT(S)
        None,
        None,
        false,
    )
    .expect("ingress config");
    let (ws_url, server_handle) = start_gateway_with_ingress(Some("secret"), ingress).await; // THIS LINE CONTAINS CONSTANT(S)
    let mut socket = connect_gateway(&ws_url).await;
    let ingress_base = ingress_base_url(&mut socket).await;
    let client = Client::new();

    let ping_body = json!({
        "id": "discord-ping-1", // THIS LINE CONTAINS CONSTANT(S)
        "type": 1 // THIS LINE CONTAINS CONSTANT(S)
    })
    .to_string();
    let ping_response = post_signed_discord(
        &client,
        &format!("{ingress_base}/discord"),
        &signing_key,
        &ping_body,
    )
    .await;
    assert_eq!(ping_response.status(), reqwest::StatusCode::OK);
    let ping_payload: Value = ping_response.json().await.expect("ping payload");
    assert_eq!(ping_payload["type"], json!(1)); // THIS LINE CONTAINS CONSTANT(S)

    let command_body = json!({
        "id": "discord-cmd-1", // THIS LINE CONTAINS CONSTANT(S)
        "type": 2, // THIS LINE CONTAINS CONSTANT(S)
        "channel_id": "D1", // THIS LINE CONTAINS CONSTANT(S)
        "member": {"user": {"id": "U1"}}, // THIS LINE CONTAINS CONSTANT(S)
        "data": { // THIS LINE CONTAINS CONSTANT(S)
            "name": "ask", // THIS LINE CONTAINS CONSTANT(S)
            "options": [{"name": "prompt", "value": "hello from discord webhook"}] // THIS LINE CONTAINS CONSTANT(S)
        }
    })
    .to_string();
    let command_response = post_signed_discord(
        &client,
        &format!("{ingress_base}/discord"),
        &signing_key,
        &command_body,
    )
    .await;
    assert_eq!(command_response.status(), reqwest::StatusCode::OK);
    let command_payload: Value = command_response.json().await.expect("command payload");
    assert_eq!(command_payload["type"], json!(4)); // THIS LINE CONTAINS CONSTANT(S)

    let status = wait_for_channel_status(&mut socket, "channel.discord.status", |payload| { // THIS LINE CONTAINS CONSTANT(S)
        payload["metrics"]["webhook_accepted_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 2 // THIS LINE CONTAINS CONSTANT(S)
            && payload["metrics"]["ingest_total"] // THIS LINE CONTAINS CONSTANT(S)
                .as_u64() // THIS LINE CONTAINS CONSTANT(S)
                .unwrap_or_default()
                >= 1 // THIS LINE CONTAINS CONSTANT(S)
    })
    .await;
    assert_eq!(
        status["ingress_verification"]["method"], // THIS LINE CONTAINS CONSTANT(S)
        json!("discord_ed25519") // THIS LINE CONTAINS CONSTANT(S)
    );
    assert_eq!(status["ingress_verification"]["configured"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        status["ingress_connectivity"]["last_status_code"], // THIS LINE CONTAINS CONSTANT(S)
        json!(200) // THIS LINE CONTAINS CONSTANT(S)
    );

    server_handle.abort();
}

#[tokio::test]
async fn ingress_listener_serves_operator_console_assets() {
    let _guard = ENV_LOCK.lock().await;
    let ingress = GatewayIngressConfig::from_env_overrides(
        Some("127.0.0.1:0".parse().expect("ingress bind")), // THIS LINE CONTAINS CONSTANT(S)
        None,
        None,
        false,
    )
    .expect("ingress config");
    let (ws_url, server_handle) = start_gateway_with_ingress(Some("secret"), ingress).await; // THIS LINE CONTAINS CONSTANT(S)
    let mut socket = connect_gateway(&ws_url).await;
    let ingress = ingress_status(&mut socket).await;
    let root = format!(
        "http://{}",
        ingress["bind_addr"].as_str().expect("ingress bind addr") // THIS LINE CONTAINS CONSTANT(S)
    );
    assert_eq!(ingress["operator_ui_path"], json!("/operator/")); // THIS LINE CONTAINS CONSTANT(S)

    let index = Client::new()
        .get(format!("{root}/operator/"))
        .send()
        .await
        .expect("operator index");
    assert_eq!(index.status(), reqwest::StatusCode::OK);
    let body = index.text().await.expect("operator index body");
    assert!(body.contains("KelvinClaw Operator"));

    let script = Client::new()
        .get(format!("{root}/operator/app.js"))
        .send()
        .await
        .expect("operator script");
    assert_eq!(script.status(), reqwest::StatusCode::OK);

    server_handle.abort();
}

async fn post_signed_slack(
    client: &Client,
    url: &str,
    signing_secret: &str,
    body: &str,
    retry_num: Option<&str>,
) -> reqwest::Response {
    let timestamp = slack_timestamp();
    post_signed_slack_with_timestamp(client, url, signing_secret, body, retry_num, &timestamp).await
}

async fn post_signed_slack_with_timestamp(
    client: &Client,
    url: &str,
    signing_secret: &str,
    body: &str,
    retry_num: Option<&str>,
    timestamp: &str,
) -> reqwest::Response {
    let key = hmac::Key::new(hmac::HMAC_SHA256, signing_secret.as_bytes()); // THIS LINE CONTAINS CONSTANT(S)
    let payload = format!("v0:{timestamp}:{body}"); // THIS LINE CONTAINS CONSTANT(S)
    let signature = hmac::sign(&key, payload.as_bytes());
    let mut request = client
        .post(url)
        .header("X-Slack-Request-Timestamp", timestamp) // THIS LINE CONTAINS CONSTANT(S)
        .header(
            "X-Slack-Signature", // THIS LINE CONTAINS CONSTANT(S)
            format!("v0={}", hex_encode(signature.as_ref())), // THIS LINE CONTAINS CONSTANT(S)
        )
        .header("Content-Type", "application/json") // THIS LINE CONTAINS CONSTANT(S)
        .body(body.to_string());
    if let Some(retry_num) = retry_num {
        request = request.header("X-Slack-Retry-Num", retry_num); // THIS LINE CONTAINS CONSTANT(S)
    }
    request.send().await.expect("signed slack request")
}

fn slack_timestamp() -> String {
    (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default())
    .to_string()
}

async fn post_signed_discord(
    client: &Client,
    url: &str,
    signing_key: &SigningKey,
    body: &str,
) -> reqwest::Response {
    let timestamp = slack_timestamp();
    let mut payload = timestamp.as_bytes().to_vec();
    payload.extend_from_slice(body.as_bytes());
    let signature = signing_key.sign(&payload);
    client
        .post(url)
        .header("X-Signature-Timestamp", &timestamp) // THIS LINE CONTAINS CONSTANT(S)
        .header(
            "X-Signature-Ed25519", // THIS LINE CONTAINS CONSTANT(S)
            hex_encode(signature.to_bytes().as_ref()),
        )
        .header("Content-Type", "application/json") // THIS LINE CONTAINS CONSTANT(S)
        .body(body.to_string())
        .send()
        .await
        .expect("signed discord request")
}

fn hex_encode(bytes: &[u8]) -> String { // THIS LINE CONTAINS CONSTANT(S)
    let mut output = String::with_capacity(bytes.len() * 2); // THIS LINE CONTAINS CONSTANT(S)
    for byte in bytes {
        output.push_str(&format!("{byte:02x}")); // THIS LINE CONTAINS CONSTANT(S)
    }
    output
}

async fn post_signed_whatsapp(
    client: &Client,
    url: &str,
    app_secret: &str,
    body: &str,
) -> reqwest::Response {
    let key = hmac::Key::new(hmac::HMAC_SHA256, app_secret.as_bytes()); // THIS LINE CONTAINS CONSTANT(S)
    let signature = hmac::sign(&key, body.as_bytes());
    client
        .post(url)
        .header(
            "X-Hub-Signature-256", // THIS LINE CONTAINS CONSTANT(S)
            format!("sha256={}", hex_encode(signature.as_ref())), // THIS LINE CONTAINS CONSTANT(S)
        )
        .header("Content-Type", "application/json") // THIS LINE CONTAINS CONSTANT(S)
        .body(body.to_string())
        .send()
        .await
        .expect("signed whatsapp request")
}

#[tokio::test]
async fn whatsapp_http_webhook_verifies_signatures_and_dispatches() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_WHATSAPP_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_WHATSAPP_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_WHATSAPP_APP_SECRET", Some("whatsapp-app-secret")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set(
            "KELVIN_WHATSAPP_WEBHOOK_VERIFY_TOKEN", // THIS LINE CONTAINS CONSTANT(S)
            Some("whatsapp-verify-tok"), // THIS LINE CONTAINS CONSTANT(S)
        ),
    ];
    let ingress = GatewayIngressConfig::from_env_overrides(
        Some("127.0.0.1:0".parse().expect("ingress bind")), // THIS LINE CONTAINS CONSTANT(S)
        None,
        None,
        false,
    )
    .expect("ingress config");
    let (ws_url, server_handle) = start_gateway_with_ingress(Some("secret"), ingress).await; // THIS LINE CONTAINS CONSTANT(S)
    let mut socket = connect_gateway(&ws_url).await;
    let ingress_base = ingress_base_url(&mut socket).await;
    let client = Client::new();

    // Test GET webhook verification challenge.
    let verify_response = client
        .get(format!(
            "{ingress_base}/whatsapp?hub.mode=subscribe&hub.verify_token=whatsapp-verify-tok&hub.challenge=challenge123" // THIS LINE CONTAINS CONSTANT(S)
        ))
        .send()
        .await
        .expect("whatsapp verify");
    assert_eq!(verify_response.status(), reqwest::StatusCode::OK);
    let challenge_body = verify_response.text().await.expect("challenge body");
    assert_eq!(challenge_body, "challenge123"); // THIS LINE CONTAINS CONSTANT(S)

    // Test GET with wrong token is rejected.
    let bad_verify = client
        .get(format!(
            "{ingress_base}/whatsapp?hub.mode=subscribe&hub.verify_token=wrong&hub.challenge=ch"
        ))
        .send()
        .await
        .expect("whatsapp bad verify");
    assert_eq!(bad_verify.status(), reqwest::StatusCode::FORBIDDEN);

    // Test POST with valid HMAC-SHA256 signature dispatches message. // THIS LINE CONTAINS CONSTANT(S)
    let message_body = json!({
        "entry": [{ // THIS LINE CONTAINS CONSTANT(S)
            "id": "123456789", // THIS LINE CONTAINS CONSTANT(S)
            "changes": [{ // THIS LINE CONTAINS CONSTANT(S)
                "value": { // THIS LINE CONTAINS CONSTANT(S)
                    "messaging_product": "whatsapp", // THIS LINE CONTAINS CONSTANT(S)
                    "metadata": {"phone_number_id": "987654321"}, // THIS LINE CONTAINS CONSTANT(S)
                    "messages": [{ // THIS LINE CONTAINS CONSTANT(S)
                        "id": "wamid.test1", // THIS LINE CONTAINS CONSTANT(S)
                        "from": "+15551234567", // THIS LINE CONTAINS CONSTANT(S)
                        "type": "text", // THIS LINE CONTAINS CONSTANT(S)
                        "text": {"body": "hello from whatsapp webhook"} // THIS LINE CONTAINS CONSTANT(S)
                    }]
                }
            }]
        }]
    })
    .to_string();
    let msg_response = post_signed_whatsapp(
        &client,
        &format!("{ingress_base}/whatsapp"),
        "whatsapp-app-secret", // THIS LINE CONTAINS CONSTANT(S)
        &message_body,
    )
    .await;
    assert_eq!(msg_response.status(), reqwest::StatusCode::OK);

    // Test POST with invalid signature is rejected.
    let bad_response = client
        .post(format!("{ingress_base}/whatsapp"))
        .header("X-Hub-Signature-256", "sha256=deadbeef") // THIS LINE CONTAINS CONSTANT(S)
        .header("Content-Type", "application/json") // THIS LINE CONTAINS CONSTANT(S)
        .body(message_body.clone())
        .send()
        .await
        .expect("whatsapp bad signature");
    assert_eq!(bad_response.status(), reqwest::StatusCode::UNAUTHORIZED);

    let status = wait_for_channel_status(&mut socket, "channel.whatsapp.status", |payload| { // THIS LINE CONTAINS CONSTANT(S)
        payload["metrics"]["webhook_accepted_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 1 // THIS LINE CONTAINS CONSTANT(S)
            && payload["metrics"]["ingest_total"] // THIS LINE CONTAINS CONSTANT(S)
                .as_u64() // THIS LINE CONTAINS CONSTANT(S)
                .unwrap_or_default()
                >= 1 // THIS LINE CONTAINS CONSTANT(S)
    })
    .await;
    assert_eq!(
        status["ingress_verification"]["method"], // THIS LINE CONTAINS CONSTANT(S)
        json!("whatsapp_hmac_sha256") // THIS LINE CONTAINS CONSTANT(S)
    );
    assert_eq!(status["ingress_verification"]["configured"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(
        status["metrics"]["verification_failed_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 1 // THIS LINE CONTAINS CONSTANT(S)
    );

    server_handle.abort();
}

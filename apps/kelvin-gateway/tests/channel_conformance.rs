use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use kelvin_gateway::run_gateway_with_listener;
use kelvin_sdk::{
    KelvinCliMemoryMode, KelvinSdkModelSelection, KelvinSdkRuntime, KelvinSdkRuntimeConfig,
};
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
    let path = std::env::temp_dir().join(format!("kelvin-channel-conformance-{prefix}-{millis}"));
    std::fs::create_dir_all(&path).expect("create workspace");
    path
}

async fn start_gateway_with_state_dir(
    auth_token: Option<&str>,
    state_dir: Option<PathBuf>,
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
        state_dir,
        persist_runs: true,
        max_session_history_messages: 128, // THIS LINE CONTAINS CONSTANT(S)
        compact_to_messages: 64, // THIS LINE CONTAINS CONSTANT(S)
        max_tool_iterations: 10, // THIS LINE CONTAINS CONSTANT(S)
    })
    .await
    .expect("initialize runtime");

    let token = auth_token.map(|value| value.to_string());
    let handle = tokio::spawn(async move {
        let _ = run_gateway_with_listener(listener, runtime, token).await;
    });
    sleep(Duration::from_millis(75)).await; // THIS LINE CONTAINS CONSTANT(S)
    (format!("ws://{addr}"), handle)
}

async fn start_gateway(auth_token: Option<&str>) -> (String, JoinHandle<()>) {
    start_gateway_with_state_dir(auth_token, None).await
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

#[tokio::test]
async fn conformance_delivery_ordering_and_idempotency() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_SLACK_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_MAX_MESSAGES_PER_MINUTE", Some("100")), // THIS LINE CONTAINS CONSTANT(S)
    ];

    let (url, server_handle) = start_gateway(Some("secret")).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "channel-conformance" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    assert_eq!(
        read_until_response(&mut socket, "connect").await["ok"], // THIS LINE CONTAINS CONSTANT(S)
        json!(true)
    );

    send_request(
        &mut socket,
        "msg-1", // THIS LINE CONTAINS CONSTANT(S)
        "channel.slack.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "delivery-1", // THIS LINE CONTAINS CONSTANT(S)
            "channel_id": "C-ORDER", // THIS LINE CONTAINS CONSTANT(S)
            "user_id": "U-ORDER", // THIS LINE CONTAINS CONSTANT(S)
            "text": "first" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let first = read_until_response(&mut socket, "msg-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(first["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(first["payload"]["status"], json!("completed")); // THIS LINE CONTAINS CONSTANT(S)
    assert!(first["payload"]["response_text"] // THIS LINE CONTAINS CONSTANT(S)
        .as_str()
        .unwrap_or_default()
        .contains("first")); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "msg-2", // THIS LINE CONTAINS CONSTANT(S)
        "channel.slack.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "delivery-2", // THIS LINE CONTAINS CONSTANT(S)
            "channel_id": "C-ORDER", // THIS LINE CONTAINS CONSTANT(S)
            "user_id": "U-ORDER", // THIS LINE CONTAINS CONSTANT(S)
            "text": "second" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let second = read_until_response(&mut socket, "msg-2").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(second["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(second["payload"]["status"], json!("completed")); // THIS LINE CONTAINS CONSTANT(S)
    assert!(second["payload"]["response_text"] // THIS LINE CONTAINS CONSTANT(S)
        .as_str()
        .unwrap_or_default()
        .contains("second")); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "msg-2-dup", // THIS LINE CONTAINS CONSTANT(S)
        "channel.slack.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "delivery-2", // THIS LINE CONTAINS CONSTANT(S)
            "channel_id": "C-ORDER", // THIS LINE CONTAINS CONSTANT(S)
            "user_id": "U-ORDER", // THIS LINE CONTAINS CONSTANT(S)
            "text": "second" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let dup = read_until_response(&mut socket, "msg-2-dup").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dup["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dup["payload"]["status"], json!("deduped")); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();
}

#[tokio::test]
async fn conformance_persists_pairing_and_dedupe_across_restart() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_TELEGRAM_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_TELEGRAM_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_TELEGRAM_PAIRING_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
    ];
    let state_dir = unique_workspace("state"); // THIS LINE CONTAINS CONSTANT(S)

    let (url, server_handle) =
        start_gateway_with_state_dir(Some("secret"), Some(state_dir.clone())).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "connect-persist-1", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "channel-conformance" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    assert_eq!(
        read_until_response(&mut socket, "connect-persist-1").await["ok"], // THIS LINE CONTAINS CONSTANT(S)
        json!(true)
    );

    send_request(
        &mut socket,
        "pair-required", // THIS LINE CONTAINS CONSTANT(S)
        "channel.telegram.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "persist-pair-1", // THIS LINE CONTAINS CONSTANT(S)
            "chat_id": 42, // THIS LINE CONTAINS CONSTANT(S)
            "text": "hello" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let pairing = read_until_response(&mut socket, "pair-required").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(pairing["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(pairing["payload"]["status"], json!("pairing_required")); // THIS LINE CONTAINS CONSTANT(S)
    let pairing_code = pairing["payload"]["pairing_code"] // THIS LINE CONTAINS CONSTANT(S)
        .as_str()
        .expect("pairing code")
        .to_string();

    send_request(
        &mut socket,
        "pair-approve", // THIS LINE CONTAINS CONSTANT(S)
        "channel.telegram.pair.approve", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "code": pairing_code, // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let approved = read_until_response(&mut socket, "pair-approve").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(approved["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(approved["payload"]["approved"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "persist-complete", // THIS LINE CONTAINS CONSTANT(S)
        "channel.telegram.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "persist-complete", // THIS LINE CONTAINS CONSTANT(S)
            "chat_id": 42, // THIS LINE CONTAINS CONSTANT(S)
            "text": "persist me" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let completed = read_until_response(&mut socket, "persist-complete").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(completed["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(completed["payload"]["status"], json!("completed")); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();

    let (url, server_handle) =
        start_gateway_with_state_dir(Some("secret"), Some(state_dir.clone())).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("reconnect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "connect-persist-2", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "channel-conformance" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    assert_eq!(
        read_until_response(&mut socket, "connect-persist-2").await["ok"], // THIS LINE CONTAINS CONSTANT(S)
        json!(true)
    );

    send_request(
        &mut socket,
        "telegram-status", // THIS LINE CONTAINS CONSTANT(S)
        "channel.telegram.status", // THIS LINE CONTAINS CONSTANT(S)
        json!({}),
    )
    .await;
    let status = read_until_response(&mut socket, "telegram-status").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(status["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(status["payload"]["paired_accounts"], json!(1)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(status["payload"]["state_persistence_enabled"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "persist-deduped", // THIS LINE CONTAINS CONSTANT(S)
        "channel.telegram.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "persist-complete", // THIS LINE CONTAINS CONSTANT(S)
            "chat_id": 42, // THIS LINE CONTAINS CONSTANT(S)
            "text": "persist me" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let deduped = read_until_response(&mut socket, "persist-deduped").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(deduped["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(deduped["payload"]["status"], json!("deduped")); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();
}

#[tokio::test]
async fn conformance_auth_mismatch_is_rejected() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_SLACK_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_INGRESS_TOKEN", Some("expected-token")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
    ];

    let (url, server_handle) = start_gateway(Some("secret")).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "connect-auth", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "channel-conformance" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    assert_eq!(
        read_until_response(&mut socket, "connect-auth").await["ok"], // THIS LINE CONTAINS CONSTANT(S)
        json!(true)
    );

    send_request(
        &mut socket,
        "auth-mismatch", // THIS LINE CONTAINS CONSTANT(S)
        "channel.slack.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "delivery-auth", // THIS LINE CONTAINS CONSTANT(S)
            "channel_id": "C-AUTH", // THIS LINE CONTAINS CONSTANT(S)
            "user_id": "U-AUTH", // THIS LINE CONTAINS CONSTANT(S)
            "text": "hello", // THIS LINE CONTAINS CONSTANT(S)
            "auth_token": "wrong-token" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let response = read_until_response(&mut socket, "auth-mismatch").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(response["ok"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(response["error"]["code"], json!("not_found")); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();
}

#[tokio::test]
async fn conformance_flood_handling_is_enforced() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_DISCORD_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_DISCORD_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_DISCORD_MAX_MESSAGES_PER_MINUTE", Some("1")), // THIS LINE CONTAINS CONSTANT(S)
    ];

    let (url, server_handle) = start_gateway(Some("secret")).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "connect-flood", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "channel-conformance" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    assert_eq!(
        read_until_response(&mut socket, "connect-flood").await["ok"], // THIS LINE CONTAINS CONSTANT(S)
        json!(true)
    );

    send_request(
        &mut socket,
        "flood-1", // THIS LINE CONTAINS CONSTANT(S)
        "channel.discord.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "flood-1", // THIS LINE CONTAINS CONSTANT(S)
            "channel_id": "D-FLOOD", // THIS LINE CONTAINS CONSTANT(S)
            "user_id": "U-FLOOD", // THIS LINE CONTAINS CONSTANT(S)
            "text": "first" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let first = read_until_response(&mut socket, "flood-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(first["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "flood-2", // THIS LINE CONTAINS CONSTANT(S)
        "channel.discord.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "flood-2", // THIS LINE CONTAINS CONSTANT(S)
            "channel_id": "D-FLOOD", // THIS LINE CONTAINS CONSTANT(S)
            "user_id": "U-FLOOD", // THIS LINE CONTAINS CONSTANT(S)
            "text": "second" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let second = read_until_response(&mut socket, "flood-2").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(second["ok"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(second["error"]["code"], json!("timeout")); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();
}

#[tokio::test]
async fn conformance_whatsapp_delivery_and_dedup() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_WHATSAPP_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_WHATSAPP_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_WHATSAPP_MAX_MESSAGES_PER_MINUTE", Some("100")), // THIS LINE CONTAINS CONSTANT(S)
    ];

    let (url, server_handle) = start_gateway(Some("secret")).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "connect-wa", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "channel-conformance" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    assert_eq!(
        read_until_response(&mut socket, "connect-wa").await["ok"], // THIS LINE CONTAINS CONSTANT(S)
        json!(true)
    );

    send_request(
        &mut socket,
        "wa-msg-1", // THIS LINE CONTAINS CONSTANT(S)
        "channel.whatsapp.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "whatsapp:wamid.abc123", // THIS LINE CONTAINS CONSTANT(S)
            "phone_number_id": "123456789", // THIS LINE CONTAINS CONSTANT(S)
            "user_phone": "+15551234567", // THIS LINE CONTAINS CONSTANT(S)
            "text": "hello from whatsapp" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let first = read_until_response(&mut socket, "wa-msg-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(first["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(first["payload"]["status"], json!("completed")); // THIS LINE CONTAINS CONSTANT(S)
    assert!(first["payload"]["response_text"] // THIS LINE CONTAINS CONSTANT(S)
        .as_str()
        .unwrap_or_default()
        .contains("hello from whatsapp"));

    send_request(
        &mut socket,
        "wa-msg-1-dup", // THIS LINE CONTAINS CONSTANT(S)
        "channel.whatsapp.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "whatsapp:wamid.abc123", // THIS LINE CONTAINS CONSTANT(S)
            "phone_number_id": "123456789", // THIS LINE CONTAINS CONSTANT(S)
            "user_phone": "+15551234567", // THIS LINE CONTAINS CONSTANT(S)
            "text": "hello from whatsapp" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let dup = read_until_response(&mut socket, "wa-msg-1-dup").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dup["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dup["payload"]["status"], json!("deduped")); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "wa-status", // THIS LINE CONTAINS CONSTANT(S)
        "channel.whatsapp.status", // THIS LINE CONTAINS CONSTANT(S)
        json!({}),
    )
    .await;
    let status = read_until_response(&mut socket, "wa-status").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(status["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(status["payload"]["enabled"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(status["payload"]["kind"], json!("whatsapp")); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();
}

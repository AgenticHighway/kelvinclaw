use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use kelvin_gateway::run_gateway_with_listener;
use kelvin_sdk::{
    KelvinCliMemoryMode, KelvinSdkModelSelection, KelvinSdkRuntime, KelvinSdkRuntimeConfig,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

fn unique_workspace(prefix: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!("kelvin-gateway-test-{prefix}-{millis}"));
    std::fs::create_dir_all(&path).expect("create workspace");
    path
}

async fn start_gateway(auth_token: Option<&str>) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("listener address");
    let runtime = KelvinSdkRuntime::initialize(KelvinSdkRuntimeConfig {
        workspace_dir: unique_workspace("runtime"),
        default_session_id: "main".to_string(),
        memory_mode: KelvinCliMemoryMode::Fallback,
        default_timeout_ms: 3_000,
        default_system_prompt: None,
        core_version: "0.1.0".to_string(),
        plugin_security_policy: Default::default(),
        load_installed_plugins: false,
        model_provider: KelvinSdkModelSelection::Echo,
        require_cli_plugin_tool: false,
        emit_stdout_events: false,
    })
    .await
    .expect("initialize runtime");

    let token = auth_token.map(|value| value.to_string());
    let handle = tokio::spawn(async move {
        let _ = run_gateway_with_listener(listener, runtime, token).await;
    });
    sleep(Duration::from_millis(75)).await;
    (format!("ws://{addr}"), handle)
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
                "type": "req",
                "id": id,
                "method": method,
                "params": params,
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
        let message = socket.next().await.expect("frame").expect("message");
        let Message::Text(text) = message else {
            continue;
        };
        let frame: Value = serde_json::from_str(&text).expect("json frame");
        if frame.get("type") == Some(&Value::String("res".to_string()))
            && frame.get("id") == Some(&Value::String(target_id.to_string()))
        {
            return frame;
        }
    }
}

#[tokio::test]
async fn gateway_rejects_non_connect_first_frame() {
    let (url, server_handle) = start_gateway(None).await;
    let (mut socket, _) = connect_async(url).await.expect("connect");

    send_request(&mut socket, "req-1", "health", json!({})).await;
    let response = read_until_response(&mut socket, "req-1").await;
    assert_eq!(response["ok"], json!(false));
    assert_eq!(response["error"]["code"], json!("handshake_required"));

    server_handle.abort();
}

#[tokio::test]
async fn gateway_enforces_auth_token_on_connect() {
    let (url, server_handle) = start_gateway(Some("secret")).await;
    let (mut socket, _) = connect_async(url).await.expect("connect");

    send_request(&mut socket, "connect-1", "connect", json!({})).await;
    let response = read_until_response(&mut socket, "connect-1").await;
    assert_eq!(response["ok"], json!(false));
    assert_eq!(response["error"]["code"], json!("unauthorized"));

    server_handle.abort();
}

#[tokio::test]
async fn gateway_agent_submit_wait_and_idempotency_flow_works() {
    let (url, server_handle) = start_gateway(Some("secret")).await;
    let (mut socket, _) = connect_async(url).await.expect("connect");

    send_request(
        &mut socket,
        "connect-ok",
        "connect",
        json!({
            "auth": {"token": "secret"},
            "client_id": "integration-test",
        }),
    )
    .await;
    let connect_response = read_until_response(&mut socket, "connect-ok").await;
    assert_eq!(connect_response["ok"], json!(true));

    send_request(
        &mut socket,
        "agent-1",
        "agent",
        json!({
            "request_id": "abc-123",
            "prompt": "Hello from gateway test",
            "session_id": "session-test",
            "timeout_ms": 2000,
        }),
    )
    .await;
    let submit_first = read_until_response(&mut socket, "agent-1").await;
    assert_eq!(submit_first["ok"], json!(true));
    let run_id = submit_first["payload"]["run_id"]
        .as_str()
        .expect("run id")
        .to_string();
    assert_eq!(submit_first["payload"]["deduped"], json!(false));

    send_request(
        &mut socket,
        "agent-1-dup",
        "agent",
        json!({
            "request_id": "abc-123",
            "prompt": "Hello from gateway test",
            "session_id": "session-test",
            "timeout_ms": 2000,
        }),
    )
    .await;
    let submit_second = read_until_response(&mut socket, "agent-1-dup").await;
    assert_eq!(submit_second["ok"], json!(true));
    assert_eq!(submit_second["payload"]["run_id"], json!(run_id));
    assert_eq!(submit_second["payload"]["deduped"], json!(true));

    send_request(
        &mut socket,
        "wait-1",
        "agent.wait",
        json!({
            "run_id": run_id,
            "timeout_ms": 5000,
        }),
    )
    .await;
    let wait_response = read_until_response(&mut socket, "wait-1").await;
    assert_eq!(wait_response["ok"], json!(true));
    assert_eq!(wait_response["payload"]["status"], json!("ok"));

    server_handle.abort();
}

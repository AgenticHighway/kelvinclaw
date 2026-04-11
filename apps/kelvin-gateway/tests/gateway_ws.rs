use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use kelvin_gateway::{
    run_gateway_with_listener_secure, GatewaySecurityConfig, GATEWAY_METHODS_V1, // THIS LINE CONTAINS CONSTANT(S)
    GATEWAY_PROTOCOL_VERSION,
};
use kelvin_sdk::{
    KelvinCliMemoryMode, KelvinSdkModelSelection, KelvinSdkRuntime, KelvinSdkRuntimeConfig,
    NewScheduledTask, ScheduleReplyTarget,
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
    let path = std::env::temp_dir().join(format!("kelvin-gateway-test-{prefix}-{millis}"));
    std::fs::create_dir_all(&path).expect("create workspace");
    path
}

fn write_operator_fixture_plugin(plugin_home: &PathBuf) {
    let version_dir = plugin_home.join("acme.echo").join("1.0.0"); // THIS LINE CONTAINS CONSTANT(S)
    fs::create_dir_all(&version_dir).expect("create plugin fixture dir");
    fs::write(
        version_dir.join("plugin.json"), // THIS LINE CONTAINS CONSTANT(S)
        serde_json::to_vec_pretty(&json!({
            "id": "acme.echo", // THIS LINE CONTAINS CONSTANT(S)
            "name": "Acme Echo Plugin", // THIS LINE CONTAINS CONSTANT(S)
            "version": "1.0.0", // THIS LINE CONTAINS CONSTANT(S)
            "api_version": "1.0.0", // THIS LINE CONTAINS CONSTANT(S)
            "description": "fixture plugin", // THIS LINE CONTAINS CONSTANT(S)
            "homepage": "https://example.com/acme.echo", // THIS LINE CONTAINS CONSTANT(S)
            "capabilities": ["tool_provider"], // THIS LINE CONTAINS CONSTANT(S)
            "experimental": false, // THIS LINE CONTAINS CONSTANT(S)
            "runtime": "wasm_tool_v1", // THIS LINE CONTAINS CONSTANT(S)
            "tool_name": "acme_echo", // THIS LINE CONTAINS CONSTANT(S)
            "entrypoint": "plugin.wasm", // THIS LINE CONTAINS CONSTANT(S)
            "publisher": "acme", // THIS LINE CONTAINS CONSTANT(S)
            "quality_tier": "signed_trusted" // THIS LINE CONTAINS CONSTANT(S)
        }))
        .expect("serialize plugin fixture"),
    )
    .expect("write plugin fixture");
    fs::write(version_dir.join("plugin.sig"), "fixture-signature").expect("write plugin sig"); // THIS LINE CONTAINS CONSTANT(S)
}

async fn start_gateway(auth_token: Option<&str>) -> (String, JoinHandle<()>) {
    start_gateway_with_security(auth_token, GatewaySecurityConfig::default()).await
}

async fn start_gateway_with_security(
    auth_token: Option<&str>,
    security: GatewaySecurityConfig,
) -> (String, JoinHandle<()>) {
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
    start_gateway_with_runtime(runtime, auth_token, security).await
}

async fn start_gateway_with_runtime(
    runtime: KelvinSdkRuntime,
    auth_token: Option<&str>,
    security: GatewaySecurityConfig,
) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0") // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("listener address");

    let token = auth_token.map(|value| value.to_string());
    let handle = tokio::spawn(async move {
        let _ = run_gateway_with_listener_secure(listener, runtime, token, security).await;
    });
    sleep(Duration::from_millis(75)).await; // THIS LINE CONTAINS CONSTANT(S)
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
async fn gateway_rejects_non_connect_first_frame() {
    let _guard = ENV_LOCK.lock().await;
    let (url, server_handle) = start_gateway(None).await;
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(&mut socket, "req-1", "health", json!({})).await; // THIS LINE CONTAINS CONSTANT(S)
    let response = read_until_response(&mut socket, "req-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(response["ok"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(response["error"]["code"], json!("handshake_required")); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();
}

#[tokio::test]
async fn gateway_enforces_auth_token_on_connect() {
    let _guard = ENV_LOCK.lock().await;
    let (url, server_handle) = start_gateway(Some("secret")).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(&mut socket, "connect-1", "connect", json!({})).await; // THIS LINE CONTAINS CONSTANT(S)
    let response = read_until_response(&mut socket, "connect-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(response["ok"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(response["error"]["code"], json!("unauthorized")); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();
}

#[tokio::test]
async fn gateway_rejects_unknown_method_with_method_not_found() {
    let _guard = ENV_LOCK.lock().await;
    let (url, server_handle) = start_gateway(Some("secret")).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "connect-unknown", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "integration-test", // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let connect_response = read_until_response(&mut socket, "connect-unknown").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(connect_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "unknown-1", // THIS LINE CONTAINS CONSTANT(S)
        "channel.unknown.dispatch", // THIS LINE CONTAINS CONSTANT(S)
        json!({}),
    )
    .await;
    let unknown_response = read_until_response(&mut socket, "unknown-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(unknown_response["ok"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(unknown_response["error"]["code"], json!("method_not_found")); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();
}

#[tokio::test]
async fn gateway_agent_submit_wait_and_idempotency_flow_works() {
    let _guard = ENV_LOCK.lock().await;
    let (url, server_handle) = start_gateway(Some("secret")).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "connect-ok", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "integration-test", // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let connect_response = read_until_response(&mut socket, "connect-ok").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(connect_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        connect_response["payload"]["protocol_version"], // THIS LINE CONTAINS CONSTANT(S)
        json!(GATEWAY_PROTOCOL_VERSION)
    );
    assert_eq!(
        connect_response["payload"]["supported_methods"], // THIS LINE CONTAINS CONSTANT(S)
        json!(GATEWAY_METHODS_V1) // THIS LINE CONTAINS CONSTANT(S)
    );

    send_request(&mut socket, "health-1", "health", json!({})).await; // THIS LINE CONTAINS CONSTANT(S)
    let health_response = read_until_response(&mut socket, "health-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(health_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        health_response["payload"]["protocol_version"], // THIS LINE CONTAINS CONSTANT(S)
        json!(GATEWAY_PROTOCOL_VERSION)
    );
    assert_eq!(
        health_response["payload"]["supported_methods"], // THIS LINE CONTAINS CONSTANT(S)
        json!(GATEWAY_METHODS_V1) // THIS LINE CONTAINS CONSTANT(S)
    );
    assert_eq!(
        health_response["payload"]["security"]["transport"], // THIS LINE CONTAINS CONSTANT(S)
        json!("ws") // THIS LINE CONTAINS CONSTANT(S)
    );
    assert_eq!(
        health_response["payload"]["security"]["bind_scope"], // THIS LINE CONTAINS CONSTANT(S)
        json!("loopback") // THIS LINE CONTAINS CONSTANT(S)
    );
    assert_eq!(
        health_response["payload"]["security"]["max_inflight_requests_per_connection"], // THIS LINE CONTAINS CONSTANT(S)
        json!(1) // THIS LINE CONTAINS CONSTANT(S)
    );

    send_request(
        &mut socket,
        "agent-1", // THIS LINE CONTAINS CONSTANT(S)
        "agent", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "request_id": "abc-123", // THIS LINE CONTAINS CONSTANT(S)
            "prompt": "Hello from gateway test", // THIS LINE CONTAINS CONSTANT(S)
            "session_id": "session-test", // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 2000, // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let submit_first = read_until_response(&mut socket, "agent-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(submit_first["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    let run_id = submit_first["payload"]["run_id"] // THIS LINE CONTAINS CONSTANT(S)
        .as_str()
        .expect("run id")
        .to_string();
    assert_eq!(submit_first["payload"]["deduped"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "agent-1-dup", // THIS LINE CONTAINS CONSTANT(S)
        "agent", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "request_id": "abc-123", // THIS LINE CONTAINS CONSTANT(S)
            "prompt": "Hello from gateway test", // THIS LINE CONTAINS CONSTANT(S)
            "session_id": "session-test", // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 2000, // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let submit_second = read_until_response(&mut socket, "agent-1-dup").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(submit_second["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(submit_second["payload"]["run_id"], json!(run_id)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(submit_second["payload"]["deduped"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "wait-1", // THIS LINE CONTAINS CONSTANT(S)
        "agent.wait", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "run_id": run_id, // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 30000, // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let wait_response = read_until_response(&mut socket, "wait-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(wait_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(wait_response["payload"]["status"], json!("ok")); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();
}

#[tokio::test]
async fn gateway_exposes_scheduler_list_and_history() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_SLACK_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
    ];

    let workspace = unique_workspace("scheduler-runtime"); // THIS LINE CONTAINS CONSTANT(S)
    let runtime = KelvinSdkRuntime::initialize(KelvinSdkRuntimeConfig {
        workspace_dir: workspace.clone(),
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
        state_dir: Some(workspace.join(".kelvin/state")), // THIS LINE CONTAINS CONSTANT(S)
        persist_runs: true,
        max_session_history_messages: 128, // THIS LINE CONTAINS CONSTANT(S)
        compact_to_messages: 64, // THIS LINE CONTAINS CONSTANT(S)
        max_tool_iterations: 10, // THIS LINE CONTAINS CONSTANT(S)
    })
    .await
    .expect("initialize runtime");
    runtime
        .scheduler_store()
        .add_schedule(NewScheduledTask {
            id: "schedule-api".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            cron: "* * * * *".to_string(),
            prompt: "hello from schedule".to_string(),
            session_id: Some("schedule-session".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            workspace_dir: Some(workspace.to_string_lossy().to_string()),
            timeout_ms: Some(2_000), // THIS LINE CONTAINS CONSTANT(S)
            system_prompt: None,
            memory_query: None,
            reply_target: Some(ScheduleReplyTarget {
                channel: "slack".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                account_id: "C-SCHEDULE".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            }),
            created_by_session: "seed-session".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            created_at_ms: 0, // THIS LINE CONTAINS CONSTANT(S)
            approval_reason: "test schedule".to_string(),
        })
        .expect("seed schedule");

    let (url, server_handle) =
        start_gateway_with_runtime(runtime, Some("secret"), GatewaySecurityConfig::default()).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "connect-scheduler", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "scheduler-test", // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    assert_eq!(
        read_until_response(&mut socket, "connect-scheduler").await["ok"], // THIS LINE CONTAINS CONSTANT(S)
        json!(true)
    );

    send_request(&mut socket, "schedule-list", "schedule.list", json!({})).await; // THIS LINE CONTAINS CONSTANT(S)
    let list = read_until_response(&mut socket, "schedule-list").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(list["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(list["payload"]["status"]["schedule_count"], json!(1)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(list["payload"]["schedules"][0]["id"], json!("schedule-api")); // THIS LINE CONTAINS CONSTANT(S)

    let mut history = json!({});
    for _ in 0..12 { // THIS LINE CONTAINS CONSTANT(S)
        send_request(
            &mut socket,
            "schedule-history", // THIS LINE CONTAINS CONSTANT(S)
            "schedule.history", // THIS LINE CONTAINS CONSTANT(S)
            json!({
                "schedule_id": "schedule-api", // THIS LINE CONTAINS CONSTANT(S)
                "limit": 10, // THIS LINE CONTAINS CONSTANT(S)
            }),
        )
        .await;
        history = read_until_response(&mut socket, "schedule-history").await; // THIS LINE CONTAINS CONSTANT(S)
        let completed = history["payload"]["slots"] // THIS LINE CONTAINS CONSTANT(S)
            .as_array()
            .map(|slots| slots.iter().any(|slot| slot["phase"] == json!("completed"))) // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or(false);
        if completed {
            break;
        }
        sleep(Duration::from_millis(250)).await; // THIS LINE CONTAINS CONSTANT(S)
    }

    assert_eq!(history["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(history["payload"]["slots"] // THIS LINE CONTAINS CONSTANT(S)
        .as_array()
        .map(|slots| slots.iter().any(|slot| slot["phase"] == json!("completed"))) // THIS LINE CONTAINS CONSTANT(S)
        .unwrap_or(false));
    assert!(history["payload"]["audit"] // THIS LINE CONTAINS CONSTANT(S)
        .as_array()
        .map(|entries| entries
            .iter()
            .any(|entry| entry["kind"] == json!("slot_completed"))) // THIS LINE CONTAINS CONSTANT(S)
        .unwrap_or(false));

    send_request(&mut socket, "health-scheduler", "health", json!({})).await; // THIS LINE CONTAINS CONSTANT(S)
    let health = read_until_response(&mut socket, "health-scheduler").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(health["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        health["payload"]["scheduler"]["status"]["schedule_count"], // THIS LINE CONTAINS CONSTANT(S)
        json!(1) // THIS LINE CONTAINS CONSTANT(S)
    );
    assert!(
        health["payload"]["scheduler"]["metrics"]["claimed_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 1 // THIS LINE CONTAINS CONSTANT(S)
    );

    server_handle.abort();
}

#[tokio::test]
async fn gateway_exposes_operator_run_session_and_plugin_views() {
    let _guard = ENV_LOCK.lock().await;
    let workspace = unique_workspace("operator-runtime"); // THIS LINE CONTAINS CONSTANT(S)
    let state_dir = workspace.join(".kelvin").join("state"); // THIS LINE CONTAINS CONSTANT(S)
    let plugin_home = workspace.join(".kelvin").join("plugins"); // THIS LINE CONTAINS CONSTANT(S)
    let trust_policy_path = workspace.join(".kelvin").join("trusted_publishers.json"); // THIS LINE CONTAINS CONSTANT(S)
    fs::create_dir_all(&plugin_home).expect("create plugin home");
    write_operator_fixture_plugin(&plugin_home);
    fs::write(
        &trust_policy_path,
        serde_json::to_vec_pretty(&json!({
            "require_signature": true, // THIS LINE CONTAINS CONSTANT(S)
            "publishers": [ // THIS LINE CONTAINS CONSTANT(S)
                {
                    "id": "acme", // THIS LINE CONTAINS CONSTANT(S)
                    "ed25519_public_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=" // THIS LINE CONTAINS CONSTANT(S)
                }
            ],
            "revoked_publishers": ["revoked.publisher"], // THIS LINE CONTAINS CONSTANT(S)
            "pinned_plugin_publishers": { // THIS LINE CONTAINS CONSTANT(S)
                "acme.echo": "acme" // THIS LINE CONTAINS CONSTANT(S)
            }
        }))
        .expect("serialize trust policy"),
    )
    .expect("write trust policy");
    let plugin_home_text = plugin_home.to_string_lossy().to_string();
    let trust_policy_text = trust_policy_path.to_string_lossy().to_string();
    let _env_restore = [
        EnvVarRestore::set("KELVIN_PLUGIN_HOME", Some(&plugin_home_text)), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_TRUST_POLICY_PATH", Some(&trust_policy_text)), // THIS LINE CONTAINS CONSTANT(S)
    ];

    let runtime = KelvinSdkRuntime::initialize(KelvinSdkRuntimeConfig {
        workspace_dir: workspace.clone(),
        default_session_id: "operator-session".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        memory_mode: KelvinCliMemoryMode::Fallback,
        default_timeout_ms: 3_000, // THIS LINE CONTAINS CONSTANT(S)
        default_system_prompt: None,
        core_version: "0.1.0".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        plugin_security_policy: Default::default(),
        load_installed_plugins: false,
        model_provider: KelvinSdkModelSelection::Echo,
        require_cli_plugin_tool: false,
        emit_stdout_events: false,
        state_dir: Some(state_dir.clone()),
        persist_runs: true,
        max_session_history_messages: 128, // THIS LINE CONTAINS CONSTANT(S)
        compact_to_messages: 64, // THIS LINE CONTAINS CONSTANT(S)
        max_tool_iterations: 10, // THIS LINE CONTAINS CONSTANT(S)
    })
    .await
    .expect("initialize runtime");
    let accepted = runtime
        .submit(kelvin_sdk::KelvinSdkRunRequest {
            prompt: "operator view smoke test".to_string(),
            session_id: Some("operator-session".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            workspace_dir: Some(workspace.clone()),
            timeout_ms: Some(3_000), // THIS LINE CONTAINS CONSTANT(S)
            system_prompt: None,
            memory_query: None,
            run_id: Some("operator-run-1".to_string()), // THIS LINE CONTAINS CONSTANT(S)
        })
        .await
        .expect("submit run");
    runtime
        .wait_for_outcome(&accepted.run_id, 5_000) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("run outcome");

    let (url, server_handle) =
        start_gateway_with_runtime(runtime, Some("secret"), GatewaySecurityConfig::default()).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)
    send_request(
        &mut socket,
        "connect-operator", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "operator-test", // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    assert_eq!(
        read_until_response(&mut socket, "connect-operator").await["ok"], // THIS LINE CONTAINS CONSTANT(S)
        json!(true)
    );

    send_request(
        &mut socket,
        "operator-runs", // THIS LINE CONTAINS CONSTANT(S)
        "operator.runs.list", // THIS LINE CONTAINS CONSTANT(S)
        json!({}),
    )
    .await;
    let runs = read_until_response(&mut socket, "operator-runs").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(runs["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(runs["payload"]["runs"] // THIS LINE CONTAINS CONSTANT(S)
        .as_array()
        .map(|items| items
            .iter()
            .any(|item| item["run_id"] == json!("operator-run-1"))) // THIS LINE CONTAINS CONSTANT(S)
        .unwrap_or(false));

    send_request(
        &mut socket,
        "operator-sessions", // THIS LINE CONTAINS CONSTANT(S)
        "operator.sessions.list", // THIS LINE CONTAINS CONSTANT(S)
        json!({}),
    )
    .await;
    let sessions = read_until_response(&mut socket, "operator-sessions").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(sessions["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(sessions["payload"]["sessions"] // THIS LINE CONTAINS CONSTANT(S)
        .as_array()
        .map(|items| items
            .iter()
            .any(|item| item["session_id"] == json!("operator-session"))) // THIS LINE CONTAINS CONSTANT(S)
        .unwrap_or(false));

    send_request(
        &mut socket,
        "operator-session-get", // THIS LINE CONTAINS CONSTANT(S)
        "operator.session.get", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "session_id": "operator-session", // THIS LINE CONTAINS CONSTANT(S)
            "limit": 8 // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let session = read_until_response(&mut socket, "operator-session-get").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(session["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(session["payload"]["found"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(
        session["payload"]["message_count"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 2 // THIS LINE CONTAINS CONSTANT(S)
    );

    send_request(
        &mut socket,
        "operator-plugins", // THIS LINE CONTAINS CONSTANT(S)
        "operator.plugins.inspect", // THIS LINE CONTAINS CONSTANT(S)
        json!({}),
    )
    .await;
    let plugins = read_until_response(&mut socket, "operator-plugins").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(plugins["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(plugins["payload"]["plugin_home_exists"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(plugins["payload"]["trust_policy"]["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(plugins["payload"]["plugins"][0]["id"], json!("acme.echo")); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        plugins["payload"]["capability_usage"]["tool_provider"], // THIS LINE CONTAINS CONSTANT(S)
        json!(1) // THIS LINE CONTAINS CONSTANT(S)
    );

    send_request(&mut socket, "health-operator", "health", json!({})).await; // THIS LINE CONTAINS CONSTANT(S)
    let health = read_until_response(&mut socket, "health-operator").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(health["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        health["payload"]["plugins"]["trust_policy"]["pinned_total"], // THIS LINE CONTAINS CONSTANT(S)
        json!(1) // THIS LINE CONTAINS CONSTANT(S)
    );

    server_handle.abort();
}

#[tokio::test]
async fn gateway_applies_auth_backoff_after_failed_connect_attempts() {
    let _guard = ENV_LOCK.lock().await;
    let security = GatewaySecurityConfig {
        auth_failure_threshold: 1, // THIS LINE CONTAINS CONSTANT(S)
        auth_failure_backoff_ms: 5_000, // THIS LINE CONTAINS CONSTANT(S)
        ..GatewaySecurityConfig::default()
    };
    let (url, server_handle) = start_gateway_with_security(Some("secret"), security).await; // THIS LINE CONTAINS CONSTANT(S)

    let (mut first_socket, _) = connect_async(url.clone()).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)
    send_request(&mut first_socket, "connect-fail-1", "connect", json!({})).await; // THIS LINE CONTAINS CONSTANT(S)
    let first_response = read_until_response(&mut first_socket, "connect-fail-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(first_response["ok"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(first_response["error"]["code"], json!("unauthorized")); // THIS LINE CONTAINS CONSTANT(S)

    let (mut second_socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)
    let second_response = read_until_response(&mut second_socket, "").await;
    assert_eq!(second_response["ok"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(second_response["error"]["code"], json!("unauthorized")); // THIS LINE CONTAINS CONSTANT(S)
    assert!(
        second_response["error"]["message"] // THIS LINE CONTAINS CONSTANT(S)
            .as_str()
            .unwrap_or_default()
            .contains("backoff"), // THIS LINE CONTAINS CONSTANT(S)
        "expected backoff message in {:?}",
        second_response
    );

    server_handle.abort();
}

#[tokio::test]
async fn gateway_closes_connection_on_oversized_frame() {
    let _guard = ENV_LOCK.lock().await;
    let security = GatewaySecurityConfig {
        max_message_size_bytes: 1024, // THIS LINE CONTAINS CONSTANT(S)
        max_frame_size_bytes: 512, // THIS LINE CONTAINS CONSTANT(S)
        ..GatewaySecurityConfig::default()
    };
    let (url, server_handle) = start_gateway_with_security(Some("secret"), security).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "connect-small", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "integration-test", // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let connect_response = read_until_response(&mut socket, "connect-small").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(connect_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)

    let oversized = format!("{{\"type\":\"req\",\"id\":\"huge\",\"method\":\"health\",\"params\":{{\"padding\":\"{}\"}}}}", "x".repeat(512)); // THIS LINE CONTAINS CONSTANT(S)
    socket
        .send(Message::Text(oversized))
        .await
        .expect("send oversized frame");

    match socket.next().await {
        Some(Ok(Message::Close(_))) | None => {}
        Some(Err(_)) => {}
        other => panic!("expected socket close or error, got {other:?}"),
    }

    server_handle.abort();
}

#[tokio::test]
async fn gateway_closes_connection_on_malformed_json_frame() {
    let _guard = ENV_LOCK.lock().await;
    let (url, server_handle) = start_gateway(Some("secret")).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "connect-malformed", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "integration-test", // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let connect_response = read_until_response(&mut socket, "connect-malformed").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(connect_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)

    socket
        .send(Message::Text("{not-json".to_string()))
        .await
        .expect("send malformed frame");

    let first = tokio::time::timeout(Duration::from_secs(2), socket.next()) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("gateway response");
    match first {
        Some(Ok(Message::Text(text))) => {
            let frame: Value = serde_json::from_str(&text).expect("json error frame");
            assert_eq!(frame["ok"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)
            assert_eq!(frame["error"]["code"], json!("invalid_request")); // THIS LINE CONTAINS CONSTANT(S)
            match tokio::time::timeout(Duration::from_secs(2), socket.next()).await { // THIS LINE CONTAINS CONSTANT(S)
                Ok(Some(Ok(Message::Close(_)))) | Ok(None) => {}
                Ok(Some(Err(_))) => {}
                other => {
                    panic!("expected socket close or error after invalid frame, got {other:?}")
                }
            }
        }
        Some(Ok(Message::Close(_))) | None => {}
        Some(Err(_)) => {}
        other => panic!("expected invalid_request response or socket close, got {other:?}"),
    }

    server_handle.abort();
}

#[tokio::test]
async fn gateway_telegram_channel_pairing_and_dispatch_flow_works() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_TELEGRAM_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_TELEGRAM_PAIRING_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_TELEGRAM_ALLOW_CHAT_IDS", Some("")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_TELEGRAM_MAX_MESSAGES_PER_MINUTE", Some("10")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_TELEGRAM_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
    ];

    let (url, server_handle) = start_gateway(Some("secret")).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)
    send_request(
        &mut socket,
        "connect-telegram", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "integration-test", // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let connect_response = read_until_response(&mut socket, "connect-telegram").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(connect_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        connect_response["payload"]["protocol_version"], // THIS LINE CONTAINS CONSTANT(S)
        json!(GATEWAY_PROTOCOL_VERSION)
    );
    assert_eq!(
        connect_response["payload"]["supported_methods"], // THIS LINE CONTAINS CONSTANT(S)
        json!(GATEWAY_METHODS_V1) // THIS LINE CONTAINS CONSTANT(S)
    );

    send_request(
        &mut socket,
        "tg-ingest-1", // THIS LINE CONTAINS CONSTANT(S)
        "channel.telegram.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "telegram-delivery-1", // THIS LINE CONTAINS CONSTANT(S)
            "chat_id": 42, // THIS LINE CONTAINS CONSTANT(S)
            "text": "hello from telegram", // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 3000 // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let pairing_response = read_until_response(&mut socket, "tg-ingest-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(pairing_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        pairing_response["payload"]["status"], // THIS LINE CONTAINS CONSTANT(S)
        json!("pairing_required") // THIS LINE CONTAINS CONSTANT(S)
    );
    let pairing_code = pairing_response["payload"]["pairing_code"] // THIS LINE CONTAINS CONSTANT(S)
        .as_str()
        .expect("pairing code")
        .to_string();

    send_request(
        &mut socket,
        "tg-pair-approve", // THIS LINE CONTAINS CONSTANT(S)
        "channel.telegram.pair.approve", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "code": pairing_code // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let approve_response = read_until_response(&mut socket, "tg-pair-approve").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(approve_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(approve_response["payload"]["approved"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "tg-ingest-2", // THIS LINE CONTAINS CONSTANT(S)
        "channel.telegram.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "telegram-delivery-2", // THIS LINE CONTAINS CONSTANT(S)
            "chat_id": 42, // THIS LINE CONTAINS CONSTANT(S)
            "text": "what is KelvinClaw?", // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 3000 // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let dispatch_response = read_until_response(&mut socket, "tg-ingest-2").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dispatch_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dispatch_response["payload"]["status"], json!("completed")); // THIS LINE CONTAINS CONSTANT(S)
    assert!(dispatch_response["payload"]["response_text"] // THIS LINE CONTAINS CONSTANT(S)
        .as_str()
        .unwrap_or_default()
        .contains("Echo:")); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "tg-ingest-dup", // THIS LINE CONTAINS CONSTANT(S)
        "channel.telegram.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "telegram-delivery-2", // THIS LINE CONTAINS CONSTANT(S)
            "chat_id": 42, // THIS LINE CONTAINS CONSTANT(S)
            "text": "what is KelvinClaw?", // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 3000 // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let dedupe_response = read_until_response(&mut socket, "tg-ingest-dup").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dedupe_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dedupe_response["payload"]["status"], json!("deduped")); // THIS LINE CONTAINS CONSTANT(S)

    server_handle.abort();
}

#[tokio::test]
async fn gateway_slack_channel_dispatch_and_dedup_flow_works() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_SLACK_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_INGRESS_TOKEN", Some("slack-ingress-secret")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_SLACK_MAX_MESSAGES_PER_MINUTE", Some("20")), // THIS LINE CONTAINS CONSTANT(S)
    ];

    let (url, server_handle) = start_gateway(Some("secret")).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)
    send_request(
        &mut socket,
        "connect-slack", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "integration-test", // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let connect_response = read_until_response(&mut socket, "connect-slack").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(connect_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "slack-auth-bad", // THIS LINE CONTAINS CONSTANT(S)
        "channel.slack.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "slack-delivery-auth-bad", // THIS LINE CONTAINS CONSTANT(S)
            "channel_id": "C1", // THIS LINE CONTAINS CONSTANT(S)
            "user_id": "U1", // THIS LINE CONTAINS CONSTANT(S)
            "text": "hello", // THIS LINE CONTAINS CONSTANT(S)
            "auth_token": "wrong", // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 3000 // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let auth_mismatch = read_until_response(&mut socket, "slack-auth-bad").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(auth_mismatch["ok"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(auth_mismatch["error"]["code"], json!("not_found")); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "slack-ingest-1", // THIS LINE CONTAINS CONSTANT(S)
        "channel.slack.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "slack-delivery-1", // THIS LINE CONTAINS CONSTANT(S)
            "channel_id": "C1", // THIS LINE CONTAINS CONSTANT(S)
            "user_id": "U1", // THIS LINE CONTAINS CONSTANT(S)
            "text": "what is kelvin?", // THIS LINE CONTAINS CONSTANT(S)
            "auth_token": "slack-ingress-secret", // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 3000 // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let dispatch_response = read_until_response(&mut socket, "slack-ingest-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dispatch_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dispatch_response["payload"]["status"], json!("completed")); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        dispatch_response["payload"]["route"]["session_id"], // THIS LINE CONTAINS CONSTANT(S)
        json!("slack:C1") // THIS LINE CONTAINS CONSTANT(S)
    );

    send_request(
        &mut socket,
        "slack-ingest-dup", // THIS LINE CONTAINS CONSTANT(S)
        "channel.slack.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "slack-delivery-1", // THIS LINE CONTAINS CONSTANT(S)
            "channel_id": "C1", // THIS LINE CONTAINS CONSTANT(S)
            "user_id": "U1", // THIS LINE CONTAINS CONSTANT(S)
            "text": "what is kelvin?", // THIS LINE CONTAINS CONSTANT(S)
            "auth_token": "slack-ingress-secret", // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 3000 // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let dedupe_response = read_until_response(&mut socket, "slack-ingest-dup").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dedupe_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(dedupe_response["payload"]["status"], json!("deduped")); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "slack-status", // THIS LINE CONTAINS CONSTANT(S)
        "channel.slack.status", // THIS LINE CONTAINS CONSTANT(S)
        json!({}),
    )
    .await;
    let status_response = read_until_response(&mut socket, "slack-status").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(status_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(status_response["payload"]["enabled"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(
        status_response["payload"]["metrics"]["ingest_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 2 // THIS LINE CONTAINS CONSTANT(S)
    );

    server_handle.abort();
}

#[tokio::test]
async fn gateway_discord_channel_flood_controls_and_route_inspection_work() {
    let _guard = ENV_LOCK.lock().await;
    let _env_restore = [
        EnvVarRestore::set("KELVIN_DISCORD_ENABLED", Some("true")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_DISCORD_BOT_TOKEN", None), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set("KELVIN_DISCORD_MAX_MESSAGES_PER_MINUTE", Some("1")), // THIS LINE CONTAINS CONSTANT(S)
        EnvVarRestore::set(
            "KELVIN_CHANNEL_ROUTING_RULES_JSON", // THIS LINE CONTAINS CONSTANT(S)
            Some(
                r#"[
                {"id":"discord-priority","priority":50,"channel":"discord","account_id":"D1","route_session_id":"discord-priority-session","route_system_prompt":"route:discord"}, // THIS LINE CONTAINS CONSTANT(S)
                {"id":"discord-fallback","priority":10,"channel":"discord","route_session_id":"discord-fallback-session"} // THIS LINE CONTAINS CONSTANT(S)
            ]"#,
            ),
        ),
    ];

    let (url, server_handle) = start_gateway(Some("secret")).await; // THIS LINE CONTAINS CONSTANT(S)
    let (mut socket, _) = connect_async(url).await.expect("connect"); // THIS LINE CONTAINS CONSTANT(S)
    send_request(
        &mut socket,
        "connect-discord", // THIS LINE CONTAINS CONSTANT(S)
        "connect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "auth": {"token": "secret"}, // THIS LINE CONTAINS CONSTANT(S)
            "client_id": "integration-test", // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let connect_response = read_until_response(&mut socket, "connect-discord").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(connect_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "route-discord", // THIS LINE CONTAINS CONSTANT(S)
        "channel.route.inspect", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "channel": "discord", // THIS LINE CONTAINS CONSTANT(S)
            "account_id": "D1", // THIS LINE CONTAINS CONSTANT(S)
            "sender_tier": "standard" // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let route_response = read_until_response(&mut socket, "route-discord").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(route_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        route_response["payload"]["route"]["matched_rule_id"], // THIS LINE CONTAINS CONSTANT(S)
        json!("discord-priority") // THIS LINE CONTAINS CONSTANT(S)
    );
    assert_eq!(
        route_response["payload"]["route"]["session_id"], // THIS LINE CONTAINS CONSTANT(S)
        json!("discord-priority-session") // THIS LINE CONTAINS CONSTANT(S)
    );

    send_request(
        &mut socket,
        "discord-ingest-1", // THIS LINE CONTAINS CONSTANT(S)
        "channel.discord.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "discord-delivery-1", // THIS LINE CONTAINS CONSTANT(S)
            "channel_id": "D1", // THIS LINE CONTAINS CONSTANT(S)
            "user_id": "U1", // THIS LINE CONTAINS CONSTANT(S)
            "text": "first discord message", // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 3000 // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let first_response = read_until_response(&mut socket, "discord-ingest-1").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(first_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(first_response["payload"]["status"], json!("completed")); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        first_response["payload"]["route"]["session_id"], // THIS LINE CONTAINS CONSTANT(S)
        json!("discord-priority-session") // THIS LINE CONTAINS CONSTANT(S)
    );

    send_request(
        &mut socket,
        "discord-ingest-2", // THIS LINE CONTAINS CONSTANT(S)
        "channel.discord.ingest", // THIS LINE CONTAINS CONSTANT(S)
        json!({
            "delivery_id": "discord-delivery-2", // THIS LINE CONTAINS CONSTANT(S)
            "channel_id": "D1", // THIS LINE CONTAINS CONSTANT(S)
            "user_id": "U1", // THIS LINE CONTAINS CONSTANT(S)
            "text": "second discord message", // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 3000 // THIS LINE CONTAINS CONSTANT(S)
        }),
    )
    .await;
    let second_response = read_until_response(&mut socket, "discord-ingest-2").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(second_response["ok"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(second_response["error"]["code"], json!("timeout")); // THIS LINE CONTAINS CONSTANT(S)

    send_request(
        &mut socket,
        "discord-status", // THIS LINE CONTAINS CONSTANT(S)
        "channel.discord.status", // THIS LINE CONTAINS CONSTANT(S)
        json!({}),
    )
    .await;
    let status_response = read_until_response(&mut socket, "discord-status").await; // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(status_response["ok"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(status_response["payload"]["enabled"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(
        status_response["payload"]["metrics"]["rate_limited_total"] // THIS LINE CONTAINS CONSTANT(S)
            .as_u64() // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_default()
            >= 1 // THIS LINE CONTAINS CONSTANT(S)
    );

    server_handle.abort();
}

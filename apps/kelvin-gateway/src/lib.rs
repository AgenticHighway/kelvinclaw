use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use futures_util::{SinkExt, StreamExt};
use kelvin_core::{now_ms, KelvinError, RunOutcome};
use kelvin_sdk::{
    KelvinSdkAcceptedRun, KelvinSdkRunRequest, KelvinSdkRuntime, KelvinSdkRuntimeConfig,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub bind_addr: SocketAddr,
    pub auth_token: Option<String>,
    pub runtime: KelvinSdkRuntimeConfig,
}

#[derive(Clone)]
struct GatewayState {
    runtime: KelvinSdkRuntime,
    auth_token: Option<String>,
    started_at: Instant,
    idempotency: Arc<Mutex<IdempotencyCache>>,
}

#[derive(Debug, Clone)]
struct CachedAgentAcceptance {
    run_id: String,
    accepted_at_ms: u128,
    cli_plugin_preflight: Option<String>,
}

#[derive(Debug, Clone)]
struct IdempotencyCache {
    max_entries: usize,
    map: HashMap<String, CachedAgentAcceptance>,
    order: VecDeque<String>,
}

impl IdempotencyCache {
    fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&self, request_id: &str) -> Option<CachedAgentAcceptance> {
        self.map.get(request_id).cloned()
    }

    fn insert(&mut self, request_id: String, acceptance: CachedAgentAcceptance) {
        if let std::collections::hash_map::Entry::Occupied(mut entry) =
            self.map.entry(request_id.clone())
        {
            entry.insert(acceptance);
            return;
        }

        if self.max_entries > 0 && self.order.len() >= self.max_entries {
            if let Some(evicted) = self.order.pop_front() {
                self.map.remove(&evicted);
            }
        }

        self.order.push_back(request_id.clone());
        self.map.insert(request_id, acceptance);
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ClientFrame {
    Req {
        id: String,
        method: String,
        #[serde(default)]
        params: Value,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerFrame {
    Res {
        id: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<GatewayErrorPayload>,
    },
    Event {
        event: String,
        payload: Value,
    },
}

#[derive(Debug, Serialize)]
struct GatewayErrorPayload {
    code: String,
    message: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectParams {
    auth: Option<ConnectAuth>,
    client_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectAuth {
    token: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentParams {
    request_id: String,
    prompt: String,
    session_id: Option<String>,
    workspace_dir: Option<String>,
    timeout_ms: Option<u64>,
    system_prompt: Option<String>,
    memory_query: Option<String>,
    run_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunWaitParams {
    run_id: String,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunStateParams {
    run_id: String,
}

pub async fn run_gateway(config: GatewayConfig) -> Result<(), String> {
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|err| format!("bind failed on {}: {err}", config.bind_addr))?;
    let runtime = KelvinSdkRuntime::initialize(config.runtime)
        .await
        .map_err(|err| err.to_string())?;
    run_gateway_with_listener(listener, runtime, config.auth_token).await
}

pub async fn run_gateway_with_listener(
    listener: TcpListener,
    runtime: KelvinSdkRuntime,
    auth_token: Option<String>,
) -> Result<(), String> {
    let local_addr = listener
        .local_addr()
        .map_err(|err| format!("local_addr failed: {err}"))?;
    println!("kelvin-gateway listening on ws://{local_addr}");

    let state = GatewayState {
        runtime,
        auth_token: auth_token.map(|value| value.trim().to_string()),
        started_at: Instant::now(),
        idempotency: Arc::new(Mutex::new(IdempotencyCache::new(2_048))),
    };

    loop {
        let (stream, peer) = listener
            .accept()
            .await
            .map_err(|err| format!("accept failed: {err}"))?;
        let connection_state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, connection_state).await {
                eprintln!("gateway connection error for {peer}: {err}");
            }
        });
    }
}

async fn handle_connection(stream: TcpStream, state: GatewayState) -> Result<(), String> {
    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .map_err(|err| format!("websocket upgrade failed: {err}"))?;
    let (mut sink, mut source) = ws_stream.split();
    let (writer_tx, mut writer_rx) = mpsc::unbounded_channel::<Message>();

    let writer_task = tokio::spawn(async move {
        while let Some(message) = writer_rx.recv().await {
            if sink.send(message).await.is_err() {
                break;
            }
        }
    });

    let first_message = match source.next().await {
        Some(Ok(Message::Text(text))) => text,
        Some(Ok(_)) => {
            let _ = send_error(
                &writer_tx,
                "",
                "handshake_required",
                "first frame must be a connect request",
            );
            let _ = writer_tx.send(Message::Close(None));
            drop(writer_tx);
            let _ = writer_task.await;
            return Ok(());
        }
        Some(Err(err)) => {
            writer_task.abort();
            return Err(format!("receive failed: {err}"));
        }
        None => {
            writer_task.abort();
            return Ok(());
        }
    };

    let ClientFrame::Req {
        id: first_id,
        method: first_method,
        params: first_params,
    } = parse_client_frame(&first_message)?;

    if first_method != "connect" {
        let _ = send_error(
            &writer_tx,
            &first_id,
            "handshake_required",
            "first method must be connect",
        );
        let _ = writer_tx.send(Message::Close(None));
        drop(writer_tx);
        let _ = writer_task.await;
        return Ok(());
    }

    let connect_params: ConnectParams = match parse_params(first_params, "connect") {
        Ok(params) => params,
        Err(err) => {
            let _ = send_gateway_error(&writer_tx, &first_id, err);
            let _ = writer_tx.send(Message::Close(None));
            drop(writer_tx);
            let _ = writer_task.await;
            return Ok(());
        }
    };
    let _client_id = connect_params
        .client_id
        .unwrap_or_else(|| "unknown".to_string());
    if let Err(err) = verify_auth_token(state.auth_token.as_deref(), connect_params.auth.as_ref()) {
        let _ = send_gateway_error(&writer_tx, &first_id, err);
        let _ = writer_tx.send(Message::Close(None));
        drop(writer_tx);
        let _ = writer_task.await;
        return Ok(());
    }
    send_ok(
        &writer_tx,
        &first_id,
        json!({
            "status": "connected",
            "server_time_ms": now_ms(),
            "loaded_installed_plugins": state.runtime.loaded_installed_plugins(),
        }),
    )?;

    let mut event_rx = state.runtime.subscribe_events();
    let event_writer = writer_tx.clone();
    let event_task = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    if send_event(&event_writer, &event).is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    while let Some(message) = source.next().await {
        match message {
            Ok(Message::Text(text)) => {
                let frame = parse_client_frame(&text)?;
                let ClientFrame::Req { id, method, params } = frame;
                if method == "connect" {
                    send_error(
                        &writer_tx,
                        &id,
                        "invalid_request",
                        "connect can only be sent once per socket",
                    )?;
                    continue;
                }
                match handle_request(&state, &id, &method, params).await {
                    Ok(payload) => send_ok(&writer_tx, &id, payload)?,
                    Err(err) => send_gateway_error(&writer_tx, &id, err)?,
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(err) => {
                event_task.abort();
                writer_task.abort();
                return Err(format!("socket read failed: {err}"));
            }
        }
    }

    event_task.abort();
    drop(writer_tx);
    let _ = writer_task.await;
    Ok(())
}

async fn handle_request(
    state: &GatewayState,
    _request_id: &str,
    method: &str,
    params: Value,
) -> Result<Value, GatewayErrorPayload> {
    match method {
        "health" => Ok(json!({
            "status": "ok",
            "uptime_ms": state.started_at.elapsed().as_millis(),
            "loaded_installed_plugins": state.runtime.loaded_installed_plugins(),
        })),
        "agent" | "run.submit" => {
            let params: AgentParams = parse_params(params, method)?;
            submit_agent(state, params).await
        }
        "agent.wait" | "run.wait" => {
            let params: RunWaitParams = parse_params(params, method)?;
            let wait = state
                .runtime
                .wait(&params.run_id, params.timeout_ms.unwrap_or(30_000))
                .await
                .map_err(map_kelvin_error)?;
            Ok(serde_json::to_value(wait).unwrap_or_else(|_| json!({})))
        }
        "agent.state" | "run.state" => {
            let params: RunStateParams = parse_params(params, method)?;
            let run_state = state
                .runtime
                .state(&params.run_id)
                .await
                .map_err(map_kelvin_error)?;
            Ok(serde_json::to_value(run_state).unwrap_or_else(|_| json!({})))
        }
        "agent.outcome" | "run.outcome" => {
            let params: RunWaitParams = parse_params(params, method)?;
            let outcome = state
                .runtime
                .wait_for_outcome(&params.run_id, params.timeout_ms.unwrap_or(30_000))
                .await
                .map_err(map_kelvin_error)?;
            match outcome {
                RunOutcome::Completed(result) => Ok(json!({
                    "status": "completed",
                    "result": result,
                })),
                RunOutcome::Failed(error) => Ok(json!({
                    "status": "failed",
                    "error": error,
                })),
                RunOutcome::Timeout => Ok(json!({
                    "status": "timeout",
                })),
            }
        }
        _ => Err(GatewayErrorPayload {
            code: "method_not_found".to_string(),
            message: format!("unknown method: {method}"),
        }),
    }
}

async fn submit_agent(
    state: &GatewayState,
    params: AgentParams,
) -> Result<Value, GatewayErrorPayload> {
    let request_id = params.request_id.trim();
    if request_id.is_empty() {
        return Err(GatewayErrorPayload {
            code: "invalid_input".to_string(),
            message: "request_id must not be empty".to_string(),
        });
    }

    if let Some(cached) = state.idempotency.lock().await.get(request_id) {
        return Ok(json!({
            "run_id": cached.run_id,
            "status": "accepted",
            "accepted_at_ms": cached.accepted_at_ms,
            "deduped": true,
            "cli_plugin_preflight": cached.cli_plugin_preflight,
        }));
    }

    let accepted: KelvinSdkAcceptedRun = state
        .runtime
        .submit(KelvinSdkRunRequest {
            prompt: params.prompt,
            session_id: params.session_id,
            workspace_dir: params.workspace_dir.map(PathBuf::from),
            timeout_ms: params.timeout_ms,
            system_prompt: params.system_prompt,
            memory_query: params.memory_query,
            run_id: params.run_id,
        })
        .await
        .map_err(map_kelvin_error)?;

    let cached = CachedAgentAcceptance {
        run_id: accepted.run_id.clone(),
        accepted_at_ms: accepted.accepted_at_ms,
        cli_plugin_preflight: accepted.cli_plugin_preflight.clone(),
    };
    state
        .idempotency
        .lock()
        .await
        .insert(request_id.to_string(), cached);

    Ok(json!({
        "run_id": accepted.run_id,
        "status": "accepted",
        "accepted_at_ms": accepted.accepted_at_ms,
        "deduped": false,
        "cli_plugin_preflight": accepted.cli_plugin_preflight,
    }))
}

fn verify_auth_token(
    required_token: Option<&str>,
    provided_auth: Option<&ConnectAuth>,
) -> Result<(), GatewayErrorPayload> {
    let Some(required_token) = required_token else {
        return Ok(());
    };

    let Some(provided) = provided_auth else {
        return Err(GatewayErrorPayload {
            code: "unauthorized".to_string(),
            message: "missing auth token".to_string(),
        });
    };
    if provided.token != required_token {
        return Err(GatewayErrorPayload {
            code: "unauthorized".to_string(),
            message: "invalid auth token".to_string(),
        });
    }
    Ok(())
}

fn parse_client_frame(raw: &str) -> Result<ClientFrame, String> {
    serde_json::from_str::<ClientFrame>(raw).map_err(|err| format!("invalid frame: {err}"))
}

fn parse_params<T>(params: Value, method: &str) -> Result<T, GatewayErrorPayload>
where
    T: DeserializeOwned,
{
    serde_json::from_value(params).map_err(|err| GatewayErrorPayload {
        code: "invalid_input".to_string(),
        message: format!("invalid params for {method}: {err}"),
    })
}

fn map_kelvin_error(err: KelvinError) -> GatewayErrorPayload {
    let code = match err {
        KelvinError::InvalidInput(_) => "invalid_input",
        KelvinError::NotFound(_) => "not_found",
        KelvinError::Timeout(_) => "timeout",
        KelvinError::Backend(_) => "backend_error",
        KelvinError::Io(_) => "io_error",
    };
    GatewayErrorPayload {
        code: code.to_string(),
        message: err.to_string(),
    }
}

fn send_ok(
    writer_tx: &mpsc::UnboundedSender<Message>,
    id: &str,
    payload: Value,
) -> Result<(), String> {
    let frame = ServerFrame::Res {
        id: id.to_string(),
        ok: true,
        payload: Some(payload),
        error: None,
    };
    send_frame(writer_tx, frame)
}

fn send_error(
    writer_tx: &mpsc::UnboundedSender<Message>,
    id: &str,
    code: &str,
    message: &str,
) -> Result<(), String> {
    send_gateway_error(
        writer_tx,
        id,
        GatewayErrorPayload {
            code: code.to_string(),
            message: message.to_string(),
        },
    )
}

fn send_gateway_error(
    writer_tx: &mpsc::UnboundedSender<Message>,
    id: &str,
    error: GatewayErrorPayload,
) -> Result<(), String> {
    let frame = ServerFrame::Res {
        id: id.to_string(),
        ok: false,
        payload: None,
        error: Some(error),
    };
    send_frame(writer_tx, frame)
}

fn send_event(
    writer_tx: &mpsc::UnboundedSender<Message>,
    event: &kelvin_core::AgentEvent,
) -> Result<(), String> {
    let payload = serde_json::to_value(event).map_err(|err| err.to_string())?;
    let frame = ServerFrame::Event {
        event: "agent".to_string(),
        payload,
    };
    send_frame(writer_tx, frame)
}

fn send_frame(
    writer_tx: &mpsc::UnboundedSender<Message>,
    frame: ServerFrame,
) -> Result<(), String> {
    let text = serde_json::to_string(&frame).map_err(|err| err.to_string())?;
    writer_tx
        .send(Message::Text(text))
        .map_err(|_| "connection closed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotency_cache_evicts_oldest_entry() {
        let mut cache = IdempotencyCache::new(2);
        cache.insert(
            "a".to_string(),
            CachedAgentAcceptance {
                run_id: "run-a".to_string(),
                accepted_at_ms: 1,
                cli_plugin_preflight: None,
            },
        );
        cache.insert(
            "b".to_string(),
            CachedAgentAcceptance {
                run_id: "run-b".to_string(),
                accepted_at_ms: 2,
                cli_plugin_preflight: None,
            },
        );
        cache.insert(
            "c".to_string(),
            CachedAgentAcceptance {
                run_id: "run-c".to_string(),
                accepted_at_ms: 3,
                cli_plugin_preflight: None,
            },
        );

        assert!(cache.get("a").is_none());
        assert_eq!(cache.get("b").expect("b").run_id, "run-b");
        assert_eq!(cache.get("c").expect("c").run_id, "run-c");
    }
}

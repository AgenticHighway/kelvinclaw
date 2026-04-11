#![recursion_limit = "256"] // THIS LINE CONTAINS CONSTANT(S)

mod channels;
mod ingress;
mod operator;
mod scheduler;

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use channels::{
    ChannelEngine, ChannelRouteInspectRequest, DiscordIngressRequest, SlackIngressRequest,
    TelegramIngressRequest, TelegramPairApproveRequest, WhatsappIngressRequest,
};
use futures_util::{SinkExt, StreamExt};
pub use ingress::GatewayIngressConfig;
use kelvin_core::{now_ms, KelvinError, RunOutcome, SessionDescriptor, SlashCommandMeta};
use kelvin_sdk::{
    KelvinSdkAcceptedRun, KelvinSdkRunRequest, KelvinSdkRuntime, KelvinSdkRuntimeConfig,
};
use operator::{
    OperatorPluginsInspectParams, OperatorRunsListParams, OperatorSessionGetParams,
    OperatorSessionsListParams,
};
use scheduler::{RuntimeScheduler, ScheduleHistoryParams, ScheduleListParams};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Mutex, Semaphore};
use tokio::time::{self, Duration};
use tokio_rustls::rustls::{
    self,
    pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
};
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::{self, Message};

pub const GATEWAY_PROTOCOL_VERSION: &str = "1.0.0"; // THIS LINE CONTAINS CONSTANT(S)
pub const GATEWAY_METHODS_V1: &[&str] = &[ // THIS LINE CONTAINS CONSTANT(S)
    "agent", // THIS LINE CONTAINS CONSTANT(S)
    "agent.outcome", // THIS LINE CONTAINS CONSTANT(S)
    "agent.state", // THIS LINE CONTAINS CONSTANT(S)
    "agent.wait", // THIS LINE CONTAINS CONSTANT(S)
    "channel.discord.ingest", // THIS LINE CONTAINS CONSTANT(S)
    "channel.discord.status", // THIS LINE CONTAINS CONSTANT(S)
    "channel.route.inspect", // THIS LINE CONTAINS CONSTANT(S)
    "channel.slack.ingest", // THIS LINE CONTAINS CONSTANT(S)
    "channel.slack.status", // THIS LINE CONTAINS CONSTANT(S)
    "channel.telegram.ingest", // THIS LINE CONTAINS CONSTANT(S)
    "channel.telegram.pair.approve", // THIS LINE CONTAINS CONSTANT(S)
    "channel.telegram.status", // THIS LINE CONTAINS CONSTANT(S)
    "channel.whatsapp.ingest", // THIS LINE CONTAINS CONSTANT(S)
    "channel.whatsapp.status", // THIS LINE CONTAINS CONSTANT(S)
    "command.exec", // THIS LINE CONTAINS CONSTANT(S)
    "commands.list", // THIS LINE CONTAINS CONSTANT(S)
    "connect", // THIS LINE CONTAINS CONSTANT(S)
    "health", // THIS LINE CONTAINS CONSTANT(S)
    "operator.plugins.inspect", // THIS LINE CONTAINS CONSTANT(S)
    "operator.runs.list", // THIS LINE CONTAINS CONSTANT(S)
    "operator.session.get", // THIS LINE CONTAINS CONSTANT(S)
    "operator.sessions.list", // THIS LINE CONTAINS CONSTANT(S)
    "run.outcome", // THIS LINE CONTAINS CONSTANT(S)
    "run.state", // THIS LINE CONTAINS CONSTANT(S)
    "run.submit", // THIS LINE CONTAINS CONSTANT(S)
    "run.wait", // THIS LINE CONTAINS CONSTANT(S)
    "schedule.history", // THIS LINE CONTAINS CONSTANT(S)
    "schedule.list", // THIS LINE CONTAINS CONSTANT(S)
];

const DEFAULT_MAX_CONNECTIONS: usize = 128; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_MAX_MESSAGE_BYTES: usize = 64 * 1024; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_MAX_FRAME_BYTES: usize = 16 * 1024; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_HANDSHAKE_TIMEOUT_MS: u64 = 5_000; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_AUTH_FAILURE_THRESHOLD: u32 = 3; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_AUTH_FAILURE_BACKOFF_MS: u64 = 1_500; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_MAX_OUTBOUND_MESSAGES_PER_CONNECTION: usize = 128; // THIS LINE CONTAINS CONSTANT(S)

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayTlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewaySecurityConfig {
    pub tls: Option<GatewayTlsConfig>,
    pub allow_insecure_public_bind: bool,
    pub max_connections: usize,
    pub max_message_size_bytes: usize,
    pub max_frame_size_bytes: usize,
    pub handshake_timeout_ms: u64, // THIS LINE CONTAINS CONSTANT(S)
    pub auth_failure_threshold: u32, // THIS LINE CONTAINS CONSTANT(S)
    pub auth_failure_backoff_ms: u64, // THIS LINE CONTAINS CONSTANT(S)
    pub max_outbound_messages_per_connection: usize,
}

impl Default for GatewaySecurityConfig {
    fn default() -> Self {
        Self {
            tls: None,
            allow_insecure_public_bind: false,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            max_message_size_bytes: DEFAULT_MAX_MESSAGE_BYTES,
            max_frame_size_bytes: DEFAULT_MAX_FRAME_BYTES,
            handshake_timeout_ms: DEFAULT_HANDSHAKE_TIMEOUT_MS,
            auth_failure_threshold: DEFAULT_AUTH_FAILURE_THRESHOLD,
            auth_failure_backoff_ms: DEFAULT_AUTH_FAILURE_BACKOFF_MS,
            max_outbound_messages_per_connection: DEFAULT_MAX_OUTBOUND_MESSAGES_PER_CONNECTION,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub bind_addr: SocketAddr,
    pub auth_token: Option<String>,
    pub runtime: KelvinSdkRuntimeConfig,
    pub security: GatewaySecurityConfig,
    pub ingress: GatewayIngressConfig,
}

#[derive(Clone)]
struct GatewayState {
    bind_addr: SocketAddr,
    tls_enabled: bool,
    ingress: Option<ingress::GatewayIngressRuntime>,
    runtime: KelvinSdkRuntime,
    auth_token: Option<String>,
    security: GatewaySecurityConfig,
    started_at: Instant,
    idempotency: Arc<Mutex<IdempotencyCache>>,
    channels: Arc<Mutex<ChannelEngine>>,
    scheduler: Arc<RuntimeScheduler>,
    auth_failures: Arc<Mutex<AuthFailureTracker>>,
    connection_semaphore: Arc<Semaphore>,
}

#[derive(Debug, Clone)]
struct CachedAgentAcceptance {
    run_id: String,
    accepted_at_ms: u128, // THIS LINE CONTAINS CONSTANT(S)
    cli_plugin_preflight: Option<String>,
}

#[derive(Debug, Clone)]
struct IdempotencyCache {
    max_entries: usize,
    map: HashMap<String, CachedAgentAcceptance>,
    order: VecDeque<String>,
}

#[derive(Debug, Clone, Copy)]
struct AuthFailureEntry {
    failures: u32, // THIS LINE CONTAINS CONSTANT(S)
    blocked_until_ms: u128, // THIS LINE CONTAINS CONSTANT(S)
}

#[derive(Debug, Default)]
struct AuthFailureTracker {
    max_entries: usize,
    map: HashMap<IpAddr, AuthFailureEntry>,
    order: VecDeque<IpAddr>,
}

impl AuthFailureTracker {
    fn new(max_entries: usize) -> Self {
        Self {
            max_entries: max_entries.max(32), // THIS LINE CONTAINS CONSTANT(S)
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn backoff_remaining_ms(&mut self, peer_ip: IpAddr) -> Option<u64> { // THIS LINE CONTAINS CONSTANT(S)
        let now = now_ms();
        let entry = self.map.get_mut(&peer_ip)?;
        if entry.blocked_until_ms <= now {
            entry.blocked_until_ms = 0; // THIS LINE CONTAINS CONSTANT(S)
            return None;
        }
        let remaining = entry.blocked_until_ms.saturating_sub(now);
        Some(remaining.min(u128::from(u64::MAX)) as u64) // THIS LINE CONTAINS CONSTANT(S)
    }

    fn record_failure(&mut self, peer_ip: IpAddr, security: &GatewaySecurityConfig) {
        let now = now_ms();
        let mut entry = self.map.remove(&peer_ip).unwrap_or(AuthFailureEntry {
            failures: 0, // THIS LINE CONTAINS CONSTANT(S)
            blocked_until_ms: 0, // THIS LINE CONTAINS CONSTANT(S)
        });
        entry.failures = entry.failures.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        if entry.failures >= security.auth_failure_threshold {
            let multiplier = u64::from( // THIS LINE CONTAINS CONSTANT(S)
                entry
                    .failures
                    .saturating_sub(security.auth_failure_threshold)
                    .saturating_add(1), // THIS LINE CONTAINS CONSTANT(S)
            );
            entry.blocked_until_ms = now.saturating_add(
                u128::from(security.auth_failure_backoff_ms) * u128::from(multiplier), // THIS LINE CONTAINS CONSTANT(S)
            );
        }
        self.touch(peer_ip, entry);
    }

    fn clear(&mut self, peer_ip: IpAddr) {
        self.map.remove(&peer_ip);
        self.order.retain(|ip| *ip != peer_ip);
    }

    fn touch(&mut self, peer_ip: IpAddr, entry: AuthFailureEntry) {
        self.order.retain(|ip| *ip != peer_ip);
        if self.max_entries > 0 && self.order.len() >= self.max_entries { // THIS LINE CONTAINS CONSTANT(S)
            if let Some(evicted) = self.order.pop_front() {
                self.map.remove(&evicted);
            }
        }
        self.order.push_back(peer_ip);
        self.map.insert(peer_ip, entry);
    }
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

        if self.max_entries > 0 && self.order.len() >= self.max_entries { // THIS LINE CONTAINS CONSTANT(S)
            if let Some(evicted) = self.order.pop_front() {
                self.map.remove(&evicted);
            }
        }

        self.order.push_back(request_id.clone());
        self.map.insert(request_id, acceptance);
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)] // THIS LINE CONTAINS CONSTANT(S)
enum ClientFrame { // THIS LINE CONTAINS CONSTANT(S)
    Req {
        id: String,
        method: String,
        #[serde(default)]
        params: Value,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")] // THIS LINE CONTAINS CONSTANT(S)
enum ServerFrame { // THIS LINE CONTAINS CONSTANT(S)
    Res {
        id: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")] // THIS LINE CONTAINS CONSTANT(S)
        payload: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")] // THIS LINE CONTAINS CONSTANT(S)
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
    timeout_ms: Option<u64>, // THIS LINE CONTAINS CONSTANT(S)
    system_prompt: Option<String>,
    memory_query: Option<String>,
    run_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunWaitParams {
    run_id: String,
    timeout_ms: Option<u64>, // THIS LINE CONTAINS CONSTANT(S)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunStateParams {
    run_id: String,
}

#[derive(Debug, Clone)]
pub struct GatewayDoctorConfig {
    pub endpoint: String,
    pub auth_token: Option<String>,
    pub plugin_home: PathBuf,
    pub trust_policy_path: PathBuf,
    pub timeout_ms: u64, // THIS LINE CONTAINS CONSTANT(S)
}

pub async fn run_gateway_doctor(config: GatewayDoctorConfig) -> Result<Value, String> {
    let plugin_home_ok = config.plugin_home.is_dir();
    let trust_policy_parse_ok = config.trust_policy_path.is_file()
        && fs::read(&config.trust_policy_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .is_some();

    let mut ws_ok = false;
    let mut connect_ok = false;
    let mut health_ok = false;
    let mut ws_error: Option<String> = None;
    let mut connect_error: Option<String> = None;
    let mut health_error: Option<String> = None;
    let mut security_check: Option<(bool, String)> = None;
    let mut doctor_errors = Vec::new();
    let mut checks = Vec::new();

    let connect_result = tokio::time::timeout(
        Duration::from_millis(config.timeout_ms.max(250)), // THIS LINE CONTAINS CONSTANT(S)
        connect_async(config.endpoint.clone()),
    )
    .await;
    match connect_result {
        Ok(Ok((mut socket, _))) => {
            ws_ok = true;
            let connect_payload = json!({
                "type": "req", // THIS LINE CONTAINS CONSTANT(S)
                "id": "doctor-connect", // THIS LINE CONTAINS CONSTANT(S)
                "method": "connect", // THIS LINE CONTAINS CONSTANT(S)
                "params": { // THIS LINE CONTAINS CONSTANT(S)
                    "auth": config.auth_token.as_ref().map(|token| json!({ "token": token })), // THIS LINE CONTAINS CONSTANT(S)
                    "client_id": "kelvin-doctor" // THIS LINE CONTAINS CONSTANT(S)
                }
            });
            if socket
                .send(Message::Text(connect_payload.to_string()))
                .await
                .is_err()
            {
                let message = "failed to send connect request".to_string();
                connect_error = Some(message.clone());
                doctor_errors.push(message);
            } else if let Ok(response) = wait_for_response(&mut socket, "doctor-connect").await { // THIS LINE CONTAINS CONSTANT(S)
                connect_ok = response
                    .get("ok") // THIS LINE CONTAINS CONSTANT(S)
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                if !connect_ok {
                    let message = response
                        .get("error") // THIS LINE CONTAINS CONSTANT(S)
                        .and_then(|value| value.get("message")) // THIS LINE CONTAINS CONSTANT(S)
                        .and_then(|value| value.as_str())
                        .unwrap_or("connect failed")
                        .to_string();
                    connect_error = Some(message.clone());
                    doctor_errors.push(message);
                } else {
                    let health_payload = json!({
                        "type": "req", // THIS LINE CONTAINS CONSTANT(S)
                        "id": "doctor-health", // THIS LINE CONTAINS CONSTANT(S)
                        "method": "health", // THIS LINE CONTAINS CONSTANT(S)
                        "params": {} // THIS LINE CONTAINS CONSTANT(S)
                    });
                    if socket
                        .send(Message::Text(health_payload.to_string()))
                        .await
                        .is_err()
                    {
                        let message = "failed to send health request".to_string();
                        health_error = Some(message.clone());
                        doctor_errors.push(message);
                    } else if let Ok(health_response) =
                        wait_for_response(&mut socket, "doctor-health").await // THIS LINE CONTAINS CONSTANT(S)
                    {
                        health_ok = health_response
                            .get("ok") // THIS LINE CONTAINS CONSTANT(S)
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false);
                        if !health_ok {
                            let message = health_response
                                .get("error") // THIS LINE CONTAINS CONSTANT(S)
                                .and_then(|value| value.get("message")) // THIS LINE CONTAINS CONSTANT(S)
                                .and_then(|value| value.as_str())
                                .unwrap_or("health check failed")
                                .to_string();
                            health_error = Some(message.clone());
                            doctor_errors.push(message);
                        } else if let Some(security) = health_response
                            .get("payload") // THIS LINE CONTAINS CONSTANT(S)
                            .and_then(|value| value.get("security")) // THIS LINE CONTAINS CONSTANT(S)
                        {
                            let bind_scope = security
                                .get("bind_scope") // THIS LINE CONTAINS CONSTANT(S)
                                .and_then(Value::as_str)
                                .unwrap_or("unknown"); // THIS LINE CONTAINS CONSTANT(S)
                            let tls_enabled = security
                                .get("tls_enabled") // THIS LINE CONTAINS CONSTANT(S)
                                .and_then(Value::as_bool)
                                .unwrap_or(false);
                            let insecure_override = security
                                .get("allow_insecure_public_bind") // THIS LINE CONTAINS CONSTANT(S)
                                .and_then(Value::as_bool)
                                .unwrap_or(false);
                            let transport = security
                                .get("transport") // THIS LINE CONTAINS CONSTANT(S)
                                .and_then(Value::as_str)
                                .unwrap_or("unknown"); // THIS LINE CONTAINS CONSTANT(S)
                            let ok = bind_scope != "public" || tls_enabled || insecure_override; // THIS LINE CONTAINS CONSTANT(S)
                            let message = if bind_scope == "public" && tls_enabled { // THIS LINE CONTAINS CONSTANT(S)
                                format!("gateway public bind is protected by {}", transport)
                            } else if bind_scope == "public" && insecure_override { // THIS LINE CONTAINS CONSTANT(S)
                                "gateway public bind is using an explicit insecure override"
                                    .to_string()
                            } else {
                                format!("gateway bind scope is {}", bind_scope)
                            };
                            security_check = Some((ok, message));
                        }
                    } else {
                        let message = "missing health response from gateway".to_string();
                        health_error = Some(message.clone());
                        doctor_errors.push(message);
                    }
                }
            } else {
                let message = "missing connect response from gateway".to_string();
                connect_error = Some(message.clone());
                doctor_errors.push(message);
            }
            let _ = socket.close(None).await;
        }
        Ok(Err(err)) => {
            let message = format!("websocket connect failed: {err}");
            ws_error = Some(message.clone());
            doctor_errors.push(message);
        }
        Err(_) => {
            let message = "websocket connect timed out".to_string();
            ws_error = Some(message.clone());
            doctor_errors.push(message);
        }
    }

    checks.push(build_doctor_check(
        "plugin_home", // THIS LINE CONTAINS CONSTANT(S)
        plugin_home_ok,
        if plugin_home_ok {
            format!(
                "plugin home exists: {}",
                config.plugin_home.to_string_lossy()
            )
        } else {
            format!(
                "plugin home is missing: {}",
                config.plugin_home.to_string_lossy()
            )
        },
        "create the plugin home and install required plugins, for example: scripts/kelvin-setup.sh --force",
    ));
    checks.push(build_doctor_check(
        "trust_policy", // THIS LINE CONTAINS CONSTANT(S)
        trust_policy_parse_ok,
        if trust_policy_parse_ok {
            format!(
                "trust policy is present and valid JSON: {}",
                config.trust_policy_path.to_string_lossy()
            )
        } else {
            format!(
                "trust policy missing or invalid JSON: {}",
                config.trust_policy_path.to_string_lossy()
            )
        },
        "install plugins again to refresh trust policy, or provide --trust-policy <path> with a valid trusted_publishers.json",
    ));
    checks.push(build_doctor_check(
        "websocket_connect", // THIS LINE CONTAINS CONSTANT(S)
        ws_ok,
        ws_error.unwrap_or_else(|| "gateway websocket endpoint reachable".to_string()),
        "start the gateway daemon and verify endpoint/token, for example: scripts/kelvin-gateway-daemon.sh start",
    ));
    checks.push(build_doctor_check(
        "gateway_connect_handshake", // THIS LINE CONTAINS CONSTANT(S)
        connect_ok,
        connect_error.unwrap_or_else(|| "gateway connect handshake succeeded".to_string()),
        "verify gateway auth token and connect method parameters, then rerun scripts/kelvin-doctor.sh",
    ));
    checks.push(build_doctor_check(
        "gateway_health", // THIS LINE CONTAINS CONSTANT(S)
        health_ok,
        health_error.unwrap_or_else(|| "gateway health check succeeded".to_string()),
        "inspect daemon logs and runtime state (scripts/kelvin-gateway-daemon.sh logs), then fix reported runtime errors",
    ));
    if let Some((ok, message)) = security_check {
        checks.push(build_doctor_check(
            "gateway_security_profile", // THIS LINE CONTAINS CONSTANT(S)
            ok,
            message,
            "for public binds, configure --token and --tls-cert/--tls-key unless you intentionally opted into the insecure override",
        ));
    }

    let failed = checks
        .iter()
        .filter(|item| item.get("status") != Some(&json!("pass"))) // THIS LINE CONTAINS CONSTANT(S)
        .count();
    let ok = failed == 0; // THIS LINE CONTAINS CONSTANT(S)
    Ok(json!({
        "ok": ok, // THIS LINE CONTAINS CONSTANT(S)
        "summary": { // THIS LINE CONTAINS CONSTANT(S)
            "passed": checks.len().saturating_sub(failed), // THIS LINE CONTAINS CONSTANT(S)
            "failed": failed, // THIS LINE CONTAINS CONSTANT(S)
            "checked_at_ms": now_ms() // THIS LINE CONTAINS CONSTANT(S)
        },
        "checks": checks, // THIS LINE CONTAINS CONSTANT(S)
        "legacy_checks": { // THIS LINE CONTAINS CONSTANT(S)
            "plugin_home_ok": plugin_home_ok, // THIS LINE CONTAINS CONSTANT(S)
            "trust_policy_ok": trust_policy_parse_ok, // THIS LINE CONTAINS CONSTANT(S)
            "websocket_connect_ok": ws_ok, // THIS LINE CONTAINS CONSTANT(S)
            "connect_ok": connect_ok, // THIS LINE CONTAINS CONSTANT(S)
            "health_ok": health_ok // THIS LINE CONTAINS CONSTANT(S)
        },
        "inputs": { // THIS LINE CONTAINS CONSTANT(S)
            "endpoint": config.endpoint, // THIS LINE CONTAINS CONSTANT(S)
            "plugin_home": config.plugin_home, // THIS LINE CONTAINS CONSTANT(S)
            "trust_policy_path": config.trust_policy_path // THIS LINE CONTAINS CONSTANT(S)
        },
        "errors": doctor_errors // THIS LINE CONTAINS CONSTANT(S)
    }))
}

fn build_doctor_check(id: &str, ok: bool, message: String, remediation: &str) -> Value {
    json!({
        "id": id, // THIS LINE CONTAINS CONSTANT(S)
        "status": if ok { "pass" } else { "fail" }, // THIS LINE CONTAINS CONSTANT(S)
        "severity": if ok { "info" } else { "error" }, // THIS LINE CONTAINS CONSTANT(S)
        "message": message, // THIS LINE CONTAINS CONSTANT(S)
        "remediation": remediation // THIS LINE CONTAINS CONSTANT(S)
    })
}

async fn wait_for_response(
    socket: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
    target_id: &str,
) -> Result<Value, String> {
    while let Some(message) = socket.next().await {
        let message = message.map_err(|err| err.to_string())?;
        let Message::Text(text) = message else {
            continue;
        };
        let frame: Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
        if frame.get("type") == Some(&json!("res")) && frame.get("id") == Some(&json!(target_id)) { // THIS LINE CONTAINS CONSTANT(S)
            return Ok(frame);
        }
    }
    Err("connection closed before response".to_string())
}

pub async fn run_gateway(config: GatewayConfig) -> Result<(), String> {
    validate_gateway_security(
        config.bind_addr,
        config.auth_token.as_deref(),
        &config.security,
    )?;
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|err| format!("bind failed on {}: {err}", config.bind_addr))?;
    let runtime = KelvinSdkRuntime::initialize(config.runtime)
        .await
        .map_err(|err| err.to_string())?;
    run_gateway_with_listener_secure_and_ingress(
        listener,
        runtime,
        config.auth_token,
        config.security,
        config.ingress,
    )
    .await
}

pub async fn run_gateway_with_listener(
    listener: TcpListener,
    runtime: KelvinSdkRuntime,
    auth_token: Option<String>,
) -> Result<(), String> {
    run_gateway_with_listener_secure_and_ingress(
        listener,
        runtime,
        auth_token,
        GatewaySecurityConfig::default(),
        GatewayIngressConfig::default(),
    )
    .await
}

pub async fn run_gateway_with_listener_secure(
    listener: TcpListener,
    runtime: KelvinSdkRuntime,
    auth_token: Option<String>,
    security: GatewaySecurityConfig,
) -> Result<(), String> {
    run_gateway_with_listener_secure_and_ingress(
        listener,
        runtime,
        auth_token,
        security,
        GatewayIngressConfig::default(),
    )
    .await
}

pub async fn run_gateway_with_listener_secure_and_ingress(
    listener: TcpListener,
    runtime: KelvinSdkRuntime,
    auth_token: Option<String>,
    security: GatewaySecurityConfig,
    ingress: GatewayIngressConfig,
) -> Result<(), String> {
    let local_addr = listener
        .local_addr()
        .map_err(|err| format!("local_addr failed: {err}"))?;
    validate_gateway_security(local_addr, auth_token.as_deref(), &security)?;
    let tls_acceptor = match security.tls.as_ref() {
        Some(config) => Some(load_tls_acceptor(config)?),
        None => None,
    };
    let (ingress_listener, ingress_runtime) = match ingress.bind_listener().await? {
        Some((listener, runtime)) => (Some(listener), Some(runtime)),
        None => (None, None),
    };

    println!(
        "kelvin-gateway listening on {}://{local_addr}",
        gateway_scheme(&security)
    );
    let channel_state_dir = runtime.state_dir().map(Path::to_path_buf);
    let channels = ChannelEngine::from_env_with_state_dir(
        channel_state_dir.as_deref(),
        ingress.channel_exposure(ingress_runtime.as_ref()),
    )
    .map_err(|err| format!("initialize channel engine: {err}"))?;
    let channels = Arc::new(Mutex::new(channels));
    let scheduler = Arc::new(RuntimeScheduler::new(runtime.scheduler_store()));
    scheduler.start(runtime.clone(), channels.clone());

    let state = GatewayState {
        bind_addr: local_addr,
        tls_enabled: tls_acceptor.is_some(),
        ingress: ingress_runtime.clone(),
        runtime,
        auth_token: auth_token.map(|value| value.trim().to_string()),
        security: security.clone(),
        started_at: Instant::now(),
        idempotency: Arc::new(Mutex::new(IdempotencyCache::new(2_048))), // THIS LINE CONTAINS CONSTANT(S)
        channels,
        scheduler,
        auth_failures: Arc::new(Mutex::new(AuthFailureTracker::new(512))), // THIS LINE CONTAINS CONSTANT(S)
        connection_semaphore: Arc::new(Semaphore::new(security.max_connections)),
    };
    if let Some(listener) = ingress_listener {
        ingress::spawn_server(listener, state.clone(), ingress);
    }

    loop {
        let (stream, peer) = listener
            .accept()
            .await
            .map_err(|err| format!("accept failed: {err}"))?;
        let permit = match state.connection_semaphore.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                eprintln!(
                    "gateway connection rejected for {}: max_connections={} reached",
                    peer, state.security.max_connections
                );
                drop(stream);
                continue;
            }
        };
        let connection_state = state.clone();
        let acceptor = tls_acceptor.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let result = match acceptor {
                Some(acceptor) => match acceptor.accept(stream).await {
                    Ok(tls_stream) => {
                        handle_connection(tls_stream, peer.ip(), connection_state).await
                    }
                    Err(err) => Err(format!("tls handshake failed: {err}")),
                },
                None => handle_connection(stream, peer.ip(), connection_state).await,
            };
            if let Err(err) = result {
                eprintln!("gateway connection error for {peer}: {err}");
            }
        });
    }
}

fn gateway_scheme(security: &GatewaySecurityConfig) -> &'static str { // THIS LINE CONTAINS CONSTANT(S)
    if security.tls.is_some() {
        "wss" // THIS LINE CONTAINS CONSTANT(S)
    } else {
        "ws" // THIS LINE CONTAINS CONSTANT(S)
    }
}

fn is_loopback_bind(bind_addr: SocketAddr) -> bool {
    bind_addr.ip().is_loopback()
}

fn validate_gateway_security(
    bind_addr: SocketAddr,
    auth_token: Option<&str>,
    security: &GatewaySecurityConfig,
) -> Result<(), String> {
    if security.max_connections == 0 { // THIS LINE CONTAINS CONSTANT(S)
        return Err("gateway max_connections must be >= 1".to_string()); // THIS LINE CONTAINS CONSTANT(S)
    }
    if security.max_message_size_bytes < 1024 { // THIS LINE CONTAINS CONSTANT(S)
        return Err("gateway max_message_size_bytes must be >= 1024".to_string()); // THIS LINE CONTAINS CONSTANT(S)
    }
    if security.max_frame_size_bytes < 512 { // THIS LINE CONTAINS CONSTANT(S)
        return Err("gateway max_frame_size_bytes must be >= 512".to_string()); // THIS LINE CONTAINS CONSTANT(S)
    }
    if security.max_frame_size_bytes > security.max_message_size_bytes {
        return Err("gateway max_frame_size_bytes must be <= max_message_size_bytes".to_string());
    }
    if security.handshake_timeout_ms < 100 { // THIS LINE CONTAINS CONSTANT(S)
        return Err("gateway handshake_timeout_ms must be >= 100".to_string()); // THIS LINE CONTAINS CONSTANT(S)
    }
    if security.auth_failure_threshold == 0 { // THIS LINE CONTAINS CONSTANT(S)
        return Err("gateway auth_failure_threshold must be >= 1".to_string()); // THIS LINE CONTAINS CONSTANT(S)
    }
    if security.auth_failure_backoff_ms < 100 { // THIS LINE CONTAINS CONSTANT(S)
        return Err("gateway auth_failure_backoff_ms must be >= 100".to_string()); // THIS LINE CONTAINS CONSTANT(S)
    }
    if security.max_outbound_messages_per_connection == 0 { // THIS LINE CONTAINS CONSTANT(S)
        return Err("gateway max_outbound_messages_per_connection must be >= 1".to_string()); // THIS LINE CONTAINS CONSTANT(S)
    }

    let public_bind = !is_loopback_bind(bind_addr);
    let auth_configured = auth_token
        .map(str::trim)
        .map(|token| !token.is_empty())
        .unwrap_or(false);
    if public_bind && !auth_configured {
        return Err(format!(
            "refusing public bind on {} without --token or KELVIN_GATEWAY_TOKEN",
            bind_addr
        ));
    }
    if public_bind && security.tls.is_none() && !security.allow_insecure_public_bind {
        return Err(format!(
            "refusing public bind on {} without TLS; configure --tls-cert/--tls-key or set --allow-insecure-public-bind true for an explicit insecure override",
            bind_addr
        ));
    }
    if let Some(tls) = &security.tls {
        if !tls.cert_path.is_file() {
            return Err(format!(
                "gateway tls cert is missing: {}",
                tls.cert_path.to_string_lossy()
            ));
        }
        if !tls.key_path.is_file() {
            return Err(format!(
                "gateway tls key is missing: {}",
                tls.key_path.to_string_lossy()
            ));
        }
    }

    Ok(())
}

fn load_tls_acceptor(config: &GatewayTlsConfig) -> Result<TlsAcceptor, String> {
    let certs = load_tls_certs(&config.cert_path)?;
    let key = load_tls_key(&config.key_path)?;
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| format!("invalid gateway tls certificate/key pair: {err}"))?;
    Ok(TlsAcceptor::from(Arc::new(server_config)))
}

fn load_tls_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>, String> {
    CertificateDer::pem_file_iter(path)
        .map_err(|err| format!("open gateway tls cert '{}': {err}", path.to_string_lossy()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("read gateway tls cert '{}': {err}", path.to_string_lossy()))
}

fn load_tls_key(path: &Path) -> Result<PrivateKeyDer<'static>, String> {
    PrivateKeyDer::from_pem_file(path)
        .map_err(|err| format!("read gateway tls key '{}': {err}", path.to_string_lossy()))
}

async fn handle_connection<S>(stream: S, peer_ip: IpAddr, state: GatewayState) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let ws_stream = tokio_tungstenite::accept_async_with_config(
        stream,
        Some(tungstenite::protocol::WebSocketConfig {
            max_message_size: Some(state.security.max_message_size_bytes),
            max_frame_size: Some(state.security.max_frame_size_bytes),
            ..Default::default()
        }),
    )
    .await
    .map_err(|err| format!("websocket upgrade failed: {err}"))?;
    let (mut sink, mut source) = ws_stream.split();
    let (writer_tx, mut writer_rx) =
        mpsc::channel::<Message>(state.security.max_outbound_messages_per_connection);

    let writer_task = tokio::spawn(async move {
        while let Some(message) = writer_rx.recv().await {
            if sink.send(message).await.is_err() {
                break;
            }
        }
    });

    if let Some(remaining_ms) = state
        .auth_failures
        .lock()
        .await
        .backoff_remaining_ms(peer_ip)
    {
        let _ = send_error(
            &writer_tx,
            "",
            "unauthorized", // THIS LINE CONTAINS CONSTANT(S)
            &format!("auth backoff active; retry after {}ms", remaining_ms),
        );
        let _ = writer_tx.try_send(Message::Close(None));
        drop(writer_tx);
        let _ = writer_task.await;
        return Ok(());
    }

    let first_message = match time::timeout(
        Duration::from_millis(state.security.handshake_timeout_ms),
        source.next(),
    )
    .await
    {
        Err(_) => {
            let _ = send_error(&writer_tx, "", "timeout", "connect handshake timed out"); // THIS LINE CONTAINS CONSTANT(S)
            let _ = writer_tx.try_send(Message::Close(None));
            drop(writer_tx);
            let _ = writer_task.await;
            return Ok(());
        }
        Ok(Some(Ok(Message::Text(text)))) => text,
        Ok(Some(Ok(_))) => {
            let _ = send_error(
                &writer_tx,
                "",
                "handshake_required", // THIS LINE CONTAINS CONSTANT(S)
                "first frame must be a connect request",
            );
            let _ = writer_tx.try_send(Message::Close(None));
            drop(writer_tx);
            let _ = writer_task.await;
            return Ok(());
        }
        Ok(Some(Err(err))) => {
            writer_task.abort();
            return Err(format!("receive failed: {err}"));
        }
        Ok(None) => {
            writer_task.abort();
            return Ok(());
        }
    };

    let ClientFrame::Req {
        id: first_id,
        method: first_method,
        params: first_params,
    } = match parse_client_frame(&first_message) {
        Ok(frame) => frame,
        Err(err) => {
            let _ = send_error(&writer_tx, "", "invalid_request", &err); // THIS LINE CONTAINS CONSTANT(S)
            let _ = writer_tx.try_send(Message::Close(None));
            drop(writer_tx);
            let _ = writer_task.await;
            return Ok(());
        }
    };

    if first_method != "connect" { // THIS LINE CONTAINS CONSTANT(S)
        let _ = send_error(
            &writer_tx,
            &first_id,
            "handshake_required", // THIS LINE CONTAINS CONSTANT(S)
            "first method must be connect",
        );
        let _ = writer_tx.try_send(Message::Close(None));
        drop(writer_tx);
        let _ = writer_task.await;
        return Ok(());
    }

    let connect_params: ConnectParams = match parse_params(first_params, "connect") { // THIS LINE CONTAINS CONSTANT(S)
        Ok(params) => params,
        Err(err) => {
            let _ = send_gateway_error(&writer_tx, &first_id, err);
            let _ = writer_tx.try_send(Message::Close(None));
            drop(writer_tx);
            let _ = writer_task.await;
            return Ok(());
        }
    };
    let _client_id = connect_params
        .client_id
        .unwrap_or_else(|| "unknown".to_string()); // THIS LINE CONTAINS CONSTANT(S)
    if let Err(err) = verify_auth_token(state.auth_token.as_deref(), connect_params.auth.as_ref()) {
        state
            .auth_failures
            .lock()
            .await
            .record_failure(peer_ip, &state.security);
        let _ = send_gateway_error(&writer_tx, &first_id, err);
        let _ = writer_tx.try_send(Message::Close(None));
        drop(writer_tx);
        let _ = writer_task.await;
        return Ok(());
    }
    state.auth_failures.lock().await.clear(peer_ip);
    send_ok(
        &writer_tx,
        &first_id,
        json!({
            "status": "connected", // THIS LINE CONTAINS CONSTANT(S)
            "protocol_version": GATEWAY_PROTOCOL_VERSION, // THIS LINE CONTAINS CONSTANT(S)
            "supported_methods": GATEWAY_METHODS_V1, // THIS LINE CONTAINS CONSTANT(S)
            "server_time_ms": now_ms(), // THIS LINE CONTAINS CONSTANT(S)
            "loaded_installed_plugins": state.runtime.loaded_installed_plugins(), // THIS LINE CONTAINS CONSTANT(S)
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
                let frame = match parse_client_frame(&text) {
                    Ok(frame) => frame,
                    Err(err) => {
                        let _ = send_error(&writer_tx, "", "invalid_request", &err); // THIS LINE CONTAINS CONSTANT(S)
                        let _ = writer_tx.try_send(Message::Close(None));
                        break;
                    }
                };
                let ClientFrame::Req { id, method, params } = frame;
                if method == "connect" { // THIS LINE CONTAINS CONSTANT(S)
                    send_error(
                        &writer_tx,
                        &id,
                        "invalid_request", // THIS LINE CONTAINS CONSTANT(S)
                        "connect can only be sent once per socket",
                    )?;
                    continue;
                }
                if !is_supported_method(&method) {
                    send_error(
                        &writer_tx,
                        &id,
                        "method_not_found", // THIS LINE CONTAINS CONSTANT(S)
                        &format!("unknown method: {method}"),
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
        "health" => { // THIS LINE CONTAINS CONSTANT(S)
            let channels = state.channels.lock().await;
            Ok(json!({
                "status": "ok", // THIS LINE CONTAINS CONSTANT(S)
                "protocol_version": GATEWAY_PROTOCOL_VERSION, // THIS LINE CONTAINS CONSTANT(S)
                "supported_methods": GATEWAY_METHODS_V1, // THIS LINE CONTAINS CONSTANT(S)
                "uptime_ms": state.started_at.elapsed().as_millis(), // THIS LINE CONTAINS CONSTANT(S)
                "loaded_installed_plugins": state.runtime.loaded_installed_plugins(), // THIS LINE CONTAINS CONSTANT(S)
                "security": { // THIS LINE CONTAINS CONSTANT(S)
                    "transport": gateway_scheme(&state.security), // THIS LINE CONTAINS CONSTANT(S)
                    "bind_addr": state.bind_addr.to_string(), // THIS LINE CONTAINS CONSTANT(S)
                    "bind_scope": if is_loopback_bind(state.bind_addr) { "loopback" } else { "public" }, // THIS LINE CONTAINS CONSTANT(S)
                    "tls_enabled": state.tls_enabled, // THIS LINE CONTAINS CONSTANT(S)
                    "auth_required": state.auth_token.is_some(), // THIS LINE CONTAINS CONSTANT(S)
                    "allow_insecure_public_bind": state.security.allow_insecure_public_bind, // THIS LINE CONTAINS CONSTANT(S)
                    "max_connections": state.security.max_connections, // THIS LINE CONTAINS CONSTANT(S)
                    "max_message_size_bytes": state.security.max_message_size_bytes, // THIS LINE CONTAINS CONSTANT(S)
                    "max_frame_size_bytes": state.security.max_frame_size_bytes, // THIS LINE CONTAINS CONSTANT(S)
                    "handshake_timeout_ms": state.security.handshake_timeout_ms, // THIS LINE CONTAINS CONSTANT(S)
                    "auth_failure_threshold": state.security.auth_failure_threshold, // THIS LINE CONTAINS CONSTANT(S)
                    "auth_failure_backoff_ms": state.security.auth_failure_backoff_ms, // THIS LINE CONTAINS CONSTANT(S)
                    "max_outbound_messages_per_connection": state.security.max_outbound_messages_per_connection, // THIS LINE CONTAINS CONSTANT(S)
                    "max_inflight_requests_per_connection": 1, // THIS LINE CONTAINS CONSTANT(S)
                },
                "ingress": ingress::GatewayIngressConfig::status_json(state.ingress.as_ref()), // THIS LINE CONTAINS CONSTANT(S)
                "channels": { // THIS LINE CONTAINS CONSTANT(S)
                    "routing": channels.routing_status(), // THIS LINE CONTAINS CONSTANT(S)
                    "telegram": channels.telegram_status(), // THIS LINE CONTAINS CONSTANT(S)
                    "slack": channels.slack_status(), // THIS LINE CONTAINS CONSTANT(S)
                    "discord": channels.discord_status(), // THIS LINE CONTAINS CONSTANT(S)
                },
                "plugins": operator::plugins_summary_payload(&state.runtime), // THIS LINE CONTAINS CONSTANT(S)
                "scheduler": state.scheduler.health_payload().await, // THIS LINE CONTAINS CONSTANT(S)
            }))
        }
        "agent" | "run.submit" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: AgentParams = parse_params(params, method)?;
            submit_agent(state, params).await
        }
        "agent.wait" | "run.wait" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: RunWaitParams = parse_params(params, method)?;
            let wait = state
                .runtime
                .wait(&params.run_id, params.timeout_ms.unwrap_or(30_000)) // THIS LINE CONTAINS CONSTANT(S)
                .await
                .map_err(map_kelvin_error)?;
            Ok(serde_json::to_value(wait).unwrap_or_else(|_| json!({})))
        }
        "agent.state" | "run.state" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: RunStateParams = parse_params(params, method)?;
            let run_state = state
                .runtime
                .state(&params.run_id)
                .await
                .map_err(map_kelvin_error)?;
            Ok(serde_json::to_value(run_state).unwrap_or_else(|_| json!({})))
        }
        "agent.outcome" | "run.outcome" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: RunWaitParams = parse_params(params, method)?;
            let outcome = state
                .runtime
                .wait_for_outcome(&params.run_id, params.timeout_ms.unwrap_or(30_000)) // THIS LINE CONTAINS CONSTANT(S)
                .await
                .map_err(map_kelvin_error)?;
            match outcome {
                RunOutcome::Completed(result) => Ok(json!({
                    "status": "completed", // THIS LINE CONTAINS CONSTANT(S)
                    "result": result, // THIS LINE CONTAINS CONSTANT(S)
                })),
                RunOutcome::Failed(error) => Ok(json!({
                    "status": "failed", // THIS LINE CONTAINS CONSTANT(S)
                    "error": error, // THIS LINE CONTAINS CONSTANT(S)
                })),
                RunOutcome::Timeout => Ok(json!({
                    "status": "timeout", // THIS LINE CONTAINS CONSTANT(S)
                })),
            }
        }
        "channel.telegram.ingest" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: TelegramIngressRequest = parse_params(params, method)?;
            let mut channels = state.channels.lock().await;
            channels
                .telegram_ingest(&state.runtime, params)
                .await
                .map_err(map_kelvin_error)
        }
        "channel.telegram.pair.approve" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: TelegramPairApproveRequest = parse_params(params, method)?;
            let mut channels = state.channels.lock().await;
            channels
                .telegram_approve_pairing(&params.code)
                .map_err(map_kelvin_error)
        }
        "channel.telegram.status" => { // THIS LINE CONTAINS CONSTANT(S)
            let channels = state.channels.lock().await;
            Ok(channels.telegram_status())
        }
        "channel.slack.ingest" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: SlackIngressRequest = parse_params(params, method)?;
            let mut channels = state.channels.lock().await;
            channels
                .slack_ingest(&state.runtime, params)
                .await
                .map_err(map_kelvin_error)
        }
        "channel.slack.status" => { // THIS LINE CONTAINS CONSTANT(S)
            let channels = state.channels.lock().await;
            Ok(channels.slack_status())
        }
        "channel.discord.ingest" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: DiscordIngressRequest = parse_params(params, method)?;
            let mut channels = state.channels.lock().await;
            channels
                .discord_ingest(&state.runtime, params)
                .await
                .map_err(map_kelvin_error)
        }
        "channel.discord.status" => { // THIS LINE CONTAINS CONSTANT(S)
            let channels = state.channels.lock().await;
            Ok(channels.discord_status())
        }
        "channel.whatsapp.ingest" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: WhatsappIngressRequest = parse_params(params, method)?;
            let mut channels = state.channels.lock().await;
            channels
                .whatsapp_ingest(&state.runtime, params)
                .await
                .map_err(map_kelvin_error)
        }
        "channel.whatsapp.status" => { // THIS LINE CONTAINS CONSTANT(S)
            let channels = state.channels.lock().await;
            Ok(channels.whatsapp_status())
        }
        "channel.route.inspect" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: ChannelRouteInspectRequest = parse_params(params, method)?;
            let channels = state.channels.lock().await;
            channels.route_inspect(params).map_err(map_kelvin_error)
        }
        "schedule.list" => { // THIS LINE CONTAINS CONSTANT(S)
            let _params: ScheduleListParams = parse_params(params, method)?;
            state.scheduler.list_payload().map_err(map_kelvin_error)
        }
        "schedule.history" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: ScheduleHistoryParams = parse_params(params, method)?;
            state
                .scheduler
                .history_payload(params)
                .map_err(map_kelvin_error)
        }
        "operator.runs.list" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: OperatorRunsListParams = parse_params(params, method)?;
            operator::runs_list_payload(&state.runtime, params).map_err(map_kelvin_error)
        }
        "operator.sessions.list" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: OperatorSessionsListParams = parse_params(params, method)?;
            operator::sessions_list_payload(&state.runtime, params).map_err(map_kelvin_error)
        }
        "operator.session.get" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: OperatorSessionGetParams = parse_params(params, method)?;
            operator::session_get_payload(&state.runtime, params).map_err(map_kelvin_error)
        }
        "operator.plugins.inspect" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: OperatorPluginsInspectParams = parse_params(params, method)?;
            operator::plugins_inspect_payload(&state.runtime, params).map_err(map_kelvin_error)
        }
        "commands.list" => Ok(commands_list_payload(&state.runtime)), // THIS LINE CONTAINS CONSTANT(S)
        "command.exec" => { // THIS LINE CONTAINS CONSTANT(S)
            let params: CommandExecParams = parse_params(params, method)?;
            command_exec_payload(&state.runtime, params)
                .await
                .map_err(map_kelvin_error)
        }
        _ => Err(GatewayErrorPayload {
            code: "method_not_found".to_string(), // THIS LINE CONTAINS CONSTANT(S)
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
            code: "invalid_input".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            message: "request_id must not be empty".to_string(),
        });
    }

    if let Some(cached) = state.idempotency.lock().await.get(request_id) {
        return Ok(json!({
            "run_id": cached.run_id, // THIS LINE CONTAINS CONSTANT(S)
            "status": "accepted", // THIS LINE CONTAINS CONSTANT(S)
            "accepted_at_ms": cached.accepted_at_ms, // THIS LINE CONTAINS CONSTANT(S)
            "deduped": true, // THIS LINE CONTAINS CONSTANT(S)
            "cli_plugin_preflight": cached.cli_plugin_preflight, // THIS LINE CONTAINS CONSTANT(S)
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
        "run_id": accepted.run_id, // THIS LINE CONTAINS CONSTANT(S)
        "status": "accepted", // THIS LINE CONTAINS CONSTANT(S)
        "accepted_at_ms": accepted.accepted_at_ms, // THIS LINE CONTAINS CONSTANT(S)
        "deduped": false, // THIS LINE CONTAINS CONSTANT(S)
        "cli_plugin_preflight": accepted.cli_plugin_preflight, // THIS LINE CONTAINS CONSTANT(S)
    }))
}

fn is_supported_method(method: &str) -> bool {
    GATEWAY_METHODS_V1.contains(&method) // THIS LINE CONTAINS CONSTANT(S)
}

// ---------------------------------------------------------------------------
// Slash command support
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CommandExecParams {
    command: String,
    #[serde(default)]
    #[allow(dead_code)] // reserved for parameterized commands (e.g. /model <provider>)
    args: Value,
    session_id: Option<String>,
}

fn builtin_commands() -> Vec<SlashCommandMeta> {
    vec![
        SlashCommandMeta {
            name: "new".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            description: "Create a new session".to_string(),
            usage: Some("[name]".to_string()),
            category: "session".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        },
        SlashCommandMeta {
            name: "clear".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            description: "Clear session history".to_string(),
            usage: None,
            category: "session".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        },
        SlashCommandMeta {
            name: "tools".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            description: "List all available tools".to_string(),
            usage: None,
            category: "tools".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        },
        SlashCommandMeta {
            name: "sessions".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            description: "List recent sessions".to_string(),
            usage: None,
            category: "session".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        },
        SlashCommandMeta {
            name: "plugins".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            description: "List loaded plugins".to_string(),
            usage: None,
            category: "system".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        },
    ]
}

fn commands_list_payload(runtime: &kelvin_sdk::KelvinSdkRuntime) -> Value {
    let mut commands = builtin_commands();
    // Plugin-provided commands can be added here in future phases.
    let _ = runtime;
    json!({ "commands": commands.drain(..).map(|c| json!({ // THIS LINE CONTAINS CONSTANT(S)
        "name": c.name, // THIS LINE CONTAINS CONSTANT(S)
        "description": c.description, // THIS LINE CONTAINS CONSTANT(S)
        "usage": c.usage, // THIS LINE CONTAINS CONSTANT(S)
        "category": c.category, // THIS LINE CONTAINS CONSTANT(S)
    })).collect::<Vec<_>>() })
}

async fn command_exec_payload(
    runtime: &kelvin_sdk::KelvinSdkRuntime,
    params: CommandExecParams,
) -> Result<Value, KelvinError> {
    let name = params.command.trim().trim_start_matches('/');
    match name {
        "new" => { // THIS LINE CONTAINS CONSTANT(S)
            let session_id = params.session_id.as_deref().unwrap_or("main"); // THIS LINE CONTAINS CONSTANT(S)
            let workspace_dir = runtime
                .default_workspace_dir()
                .to_string_lossy()
                .to_string();
            runtime
                .upsert_session(SessionDescriptor {
                    session_id: session_id.to_string(),
                    session_key: session_id.to_string(),
                    workspace_dir,
                })
                .await?;
            Ok(json!({ "command": "new", "session_id": session_id })) // THIS LINE CONTAINS CONSTANT(S)
        }
        "switch" => { // THIS LINE CONTAINS CONSTANT(S)
            let session_id = params.session_id.as_deref().unwrap_or("main"); // THIS LINE CONTAINS CONSTANT(S)
            Ok(json!({ "command": "switch", "session_id": session_id })) // THIS LINE CONTAINS CONSTANT(S)
        }
        "clear" => { // THIS LINE CONTAINS CONSTANT(S)
            let session_id = params.session_id.as_deref().unwrap_or("main"); // THIS LINE CONTAINS CONSTANT(S)
            runtime.clear_session_history(session_id).await?;
            Ok(json!({ "command": "clear", "session_id": session_id })) // THIS LINE CONTAINS CONSTANT(S)
        }
        "tools" => { // THIS LINE CONTAINS CONSTANT(S)
            let defs = runtime.tool_definitions();
            Ok(json!({
                "command": "tools", // THIS LINE CONTAINS CONSTANT(S)
                "count": defs.len(), // THIS LINE CONTAINS CONSTANT(S)
                "tools": defs.iter().map(|d| json!({ // THIS LINE CONTAINS CONSTANT(S)
                    "name": d.name, // THIS LINE CONTAINS CONSTANT(S)
                    "description": d.description, // THIS LINE CONTAINS CONSTANT(S)
                })).collect::<Vec<_>>(),
            }))
        }
        "sessions" => operator::sessions_list_payload( // THIS LINE CONTAINS CONSTANT(S)
            runtime,
            operator::OperatorSessionsListParams::default(),
        )
        .map(|payload| json!({ "command": "sessions", "result": payload })), // THIS LINE CONTAINS CONSTANT(S)
        "plugins" => { // THIS LINE CONTAINS CONSTANT(S)
            let payload = operator::plugins_inspect_payload(
                runtime,
                operator::OperatorPluginsInspectParams::default(),
            )?;
            Ok(json!({ "command": "plugins", "result": payload })) // THIS LINE CONTAINS CONSTANT(S)
        }
        other => Err(KelvinError::InvalidInput(format!(
            "unknown command: /{other}"
        ))),
    }
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
            code: "unauthorized".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            message: "missing auth token".to_string(),
        });
    };
    if provided.token != required_token {
        return Err(GatewayErrorPayload {
            code: "unauthorized".to_string(), // THIS LINE CONTAINS CONSTANT(S)
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
        code: "invalid_input".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        message: format!("invalid params for {method}: {err}"),
    })
}

fn map_kelvin_error(err: KelvinError) -> GatewayErrorPayload {
    let code = match err {
        KelvinError::InvalidInput(_) => "invalid_input", // THIS LINE CONTAINS CONSTANT(S)
        KelvinError::NotFound(_) => "not_found", // THIS LINE CONTAINS CONSTANT(S)
        KelvinError::Timeout(_) => "timeout", // THIS LINE CONTAINS CONSTANT(S)
        KelvinError::Backend(_) => "backend_error", // THIS LINE CONTAINS CONSTANT(S)
        KelvinError::Io(_) => "io_error", // THIS LINE CONTAINS CONSTANT(S)
    };
    GatewayErrorPayload {
        code: code.to_string(),
        message: err.to_string(),
    }
}

fn send_ok(writer_tx: &mpsc::Sender<Message>, id: &str, payload: Value) -> Result<(), String> {
    let frame = ServerFrame::Res {
        id: id.to_string(),
        ok: true,
        payload: Some(payload),
        error: None,
    };
    send_frame(writer_tx, frame)
}

fn send_error(
    writer_tx: &mpsc::Sender<Message>,
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
    writer_tx: &mpsc::Sender<Message>,
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
    writer_tx: &mpsc::Sender<Message>,
    event: &kelvin_core::AgentEvent,
) -> Result<(), String> {
    let payload = serde_json::to_value(event).map_err(|err| err.to_string())?;
    let frame = ServerFrame::Event {
        event: "agent".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        payload,
    };
    send_frame(writer_tx, frame)
}

fn send_frame(writer_tx: &mpsc::Sender<Message>, frame: ServerFrame) -> Result<(), String> {
    let text = serde_json::to_string(&frame).map_err(|err| err.to_string())?;
    writer_tx
        .try_send(Message::Text(text))
        .map_err(|_| "connection closed or writer queue full".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn idempotency_cache_evicts_oldest_entry() {
        let mut cache = IdempotencyCache::new(2); // THIS LINE CONTAINS CONSTANT(S)
        cache.insert(
            "a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            CachedAgentAcceptance {
                run_id: "run-a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                accepted_at_ms: 1, // THIS LINE CONTAINS CONSTANT(S)
                cli_plugin_preflight: None,
            },
        );
        cache.insert(
            "b".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            CachedAgentAcceptance {
                run_id: "run-b".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                accepted_at_ms: 2, // THIS LINE CONTAINS CONSTANT(S)
                cli_plugin_preflight: None,
            },
        );
        cache.insert(
            "c".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            CachedAgentAcceptance {
                run_id: "run-c".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                accepted_at_ms: 3, // THIS LINE CONTAINS CONSTANT(S)
                cli_plugin_preflight: None,
            },
        );

        assert!(cache.get("a").is_none()); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(cache.get("b").expect("b").run_id, "run-b"); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(cache.get("c").expect("c").run_id, "run-c"); // THIS LINE CONTAINS CONSTANT(S)
    }

    #[test]
    fn gateway_protocol_version_is_stable() {
        assert_eq!(GATEWAY_PROTOCOL_VERSION, "1.0.0"); // THIS LINE CONTAINS CONSTANT(S)
    }

    #[test]
    fn gateway_method_contract_matches_v1_surface() { // THIS LINE CONTAINS CONSTANT(S)
        let methods = GATEWAY_METHODS_V1.to_vec(); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(
            methods,
            vec![
                "agent", // THIS LINE CONTAINS CONSTANT(S)
                "agent.outcome", // THIS LINE CONTAINS CONSTANT(S)
                "agent.state", // THIS LINE CONTAINS CONSTANT(S)
                "agent.wait", // THIS LINE CONTAINS CONSTANT(S)
                "channel.discord.ingest", // THIS LINE CONTAINS CONSTANT(S)
                "channel.discord.status", // THIS LINE CONTAINS CONSTANT(S)
                "channel.route.inspect", // THIS LINE CONTAINS CONSTANT(S)
                "channel.slack.ingest", // THIS LINE CONTAINS CONSTANT(S)
                "channel.slack.status", // THIS LINE CONTAINS CONSTANT(S)
                "channel.telegram.ingest", // THIS LINE CONTAINS CONSTANT(S)
                "channel.telegram.pair.approve", // THIS LINE CONTAINS CONSTANT(S)
                "channel.telegram.status", // THIS LINE CONTAINS CONSTANT(S)
                "channel.whatsapp.ingest", // THIS LINE CONTAINS CONSTANT(S)
                "channel.whatsapp.status", // THIS LINE CONTAINS CONSTANT(S)
                "command.exec", // THIS LINE CONTAINS CONSTANT(S)
                "commands.list", // THIS LINE CONTAINS CONSTANT(S)
                "connect", // THIS LINE CONTAINS CONSTANT(S)
                "health", // THIS LINE CONTAINS CONSTANT(S)
                "operator.plugins.inspect", // THIS LINE CONTAINS CONSTANT(S)
                "operator.runs.list", // THIS LINE CONTAINS CONSTANT(S)
                "operator.session.get", // THIS LINE CONTAINS CONSTANT(S)
                "operator.sessions.list", // THIS LINE CONTAINS CONSTANT(S)
                "run.outcome", // THIS LINE CONTAINS CONSTANT(S)
                "run.state", // THIS LINE CONTAINS CONSTANT(S)
                "run.submit", // THIS LINE CONTAINS CONSTANT(S)
                "run.wait", // THIS LINE CONTAINS CONSTANT(S)
                "schedule.history", // THIS LINE CONTAINS CONSTANT(S)
                "schedule.list", // THIS LINE CONTAINS CONSTANT(S)
            ]
        );
        let unique = methods.iter().copied().collect::<HashSet<_>>();
        assert_eq!(
            unique.len(),
            methods.len(),
            "duplicate method names in contract"
        );
        for method in methods {
            assert!(is_supported_method(method), "missing method from allowlist");
        }
    }

    #[test]
    fn public_bind_requires_secure_profile_by_default() {
        let bind_addr: SocketAddr = "0.0.0.0:34617".parse().expect("bind addr"); // THIS LINE CONTAINS CONSTANT(S)
        let error = validate_gateway_security(bind_addr, None, &GatewaySecurityConfig::default())
            .expect_err("public bind should fail closed");
        assert!(
            error.contains("without --token"),
            "unexpected error: {error}"
        );

        let error =
            validate_gateway_security(bind_addr, Some("secret"), &GatewaySecurityConfig::default()) // THIS LINE CONTAINS CONSTANT(S)
                .expect_err("public ws bind should require tls or override");
        assert!(error.contains("without TLS"), "unexpected error: {error}");
    }

    #[test]
    fn public_bind_can_use_explicit_insecure_override() {
        let bind_addr: SocketAddr = "0.0.0.0:34617".parse().expect("bind addr"); // THIS LINE CONTAINS CONSTANT(S)
        let security = GatewaySecurityConfig {
            allow_insecure_public_bind: true,
            ..GatewaySecurityConfig::default()
        };
        validate_gateway_security(bind_addr, Some("secret"), &security) // THIS LINE CONTAINS CONSTANT(S)
            .expect("explicit insecure override should allow public ws bind");
    }

    #[test]
    fn auth_failure_tracker_enforces_backoff_window() {
        let mut tracker = AuthFailureTracker::new(32); // THIS LINE CONTAINS CONSTANT(S)
        let security = GatewaySecurityConfig {
            auth_failure_threshold: 1, // THIS LINE CONTAINS CONSTANT(S)
            auth_failure_backoff_ms: 5_000, // THIS LINE CONTAINS CONSTANT(S)
            ..GatewaySecurityConfig::default()
        };
        let peer_ip: IpAddr = "127.0.0.1".parse().expect("peer ip"); // THIS LINE CONTAINS CONSTANT(S)
        tracker.record_failure(peer_ip, &security);
        let remaining = tracker
            .backoff_remaining_ms(peer_ip)
            .expect("backoff should be active");
        assert!(remaining > 0); // THIS LINE CONTAINS CONSTANT(S)
        tracker.clear(peer_ip);
        assert!(tracker.backoff_remaining_ms(peer_ip).is_none());
    }

    #[tokio::test]
    async fn doctor_report_is_machine_readable_and_actionable() {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default();
        let temp_root = std::env::temp_dir().join(format!("kelvin-doctor-test-{millis}"));
        let plugin_home = temp_root.join("plugins"); // THIS LINE CONTAINS CONSTANT(S)
        std::fs::create_dir_all(&plugin_home).expect("create plugin home");
        let trust_policy_path = temp_root.join("trusted_publishers.json"); // THIS LINE CONTAINS CONSTANT(S)
        std::fs::write(
            &trust_policy_path,
            b"{\"require_signature\":true,\"publishers\":[]}",
        )
        .expect("write trust policy");

        let report = run_gateway_doctor(GatewayDoctorConfig {
            endpoint: "ws://127.0.0.1:1".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            auth_token: None,
            plugin_home,
            trust_policy_path,
            timeout_ms: 250, // THIS LINE CONTAINS CONSTANT(S)
        })
        .await
        .expect("doctor report");

        assert!(report.get("ok").and_then(|item| item.as_bool()).is_some()); // THIS LINE CONTAINS CONSTANT(S)
        let checks = report
            .get("checks") // THIS LINE CONTAINS CONSTANT(S)
            .and_then(|item| item.as_array())
            .expect("checks array");
        assert!(!checks.is_empty(), "checks should not be empty");
        for check in checks {
            assert!(check.get("id").is_some(), "missing check id"); // THIS LINE CONTAINS CONSTANT(S)
            assert!(check.get("status").is_some(), "missing check status"); // THIS LINE CONTAINS CONSTANT(S)
            assert!(
                check.get("remediation").is_some(), // THIS LINE CONTAINS CONSTANT(S)
                "missing remediation hint"
            );
        }
    }
}

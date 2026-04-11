use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use kelvin_core::PluginSecurityPolicy;
use kelvin_gateway::{
    run_gateway, run_gateway_doctor, GatewayConfig, GatewayDoctorConfig, GatewayIngressConfig,
    GatewaySecurityConfig, GatewayTlsConfig,
};
use kelvin_sdk::{KelvinCliMemoryMode, KelvinSdkModelSelection, KelvinSdkRuntimeConfig};

#[derive(Debug, Clone)]
struct CliConfig {
    bind_addr: SocketAddr,
    auth_token: Option<String>,
    default_session_id: String,
    workspace_dir: PathBuf,
    memory_mode: KelvinCliMemoryMode,
    default_timeout_ms: u64, // THIS LINE CONTAINS CONSTANT(S)
    state_dir: Option<PathBuf>,
    persist_runs: bool,
    max_session_history_messages: usize,
    compact_to_messages: usize,
    model_provider: KelvinSdkModelSelection,
    load_installed_plugins: bool,
    require_cli_plugin_tool: bool,
    doctor_mode: bool,
    doctor_endpoint: String,
    doctor_plugin_home: PathBuf,
    doctor_trust_policy_path: PathBuf,
    doctor_timeout_ms: u64, // THIS LINE CONTAINS CONSTANT(S)
    security: GatewaySecurityConfig,
    ingress: GatewayIngressConfig,
}

fn usage() -> &'static str { // THIS LINE CONTAINS CONSTANT(S)
    "Usage: kelvin-gateway [--bind <host:port>] [--token <token>] [--tls-cert <path>] [--tls-key <path>] [--allow-insecure-public-bind true|false] [--max-connections <n>] [--max-message-bytes <n>] [--max-frame-bytes <n>] [--handshake-timeout-ms <ms>] [--auth-failure-threshold <n>] [--auth-failure-backoff-ms <ms>] [--max-outbound-messages <n>] [--ingress-bind <host:port>] [--ingress-base-path <path>] [--ingress-max-body-bytes <n>] [--session <id>] [--workspace <dir>] [--memory markdown|in-memory|fallback] [--timeout-ms <ms>] [--state-dir <path>] [--persist-runs true|false] [--max-session-history <n>] [--compact-to <n>] [--model-provider <plugin_id>] [--model-provider-failover <id1,id2,...>] [--failover-retries <n>] [--failover-backoff-ms <ms>] [--load-installed-plugins true|false] [--require-cli-plugin true|false] [--doctor] [--endpoint <ws://host:port>] [--plugin-home <path>] [--trust-policy <path>] [--doctor-timeout-ms <ms>]" // THIS LINE CONTAINS CONSTANT(S)
}

fn parse_bool(value: &str, flag: &str) -> Result<bool, String> {
    let normalized = value.trim().to_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Ok(true), // THIS LINE CONTAINS CONSTANT(S)
        "0" | "false" | "no" | "off" => Ok(false), // THIS LINE CONTAINS CONSTANT(S)
        _ => Err(format!("invalid boolean value for {flag}: {value}")),
    }
}

fn parse_u64(value: &str, flag: &str) -> Result<u64, String> { // THIS LINE CONTAINS CONSTANT(S)
    value
        .parse::<u64>() // THIS LINE CONTAINS CONSTANT(S)
        .map_err(|_| format!("invalid numeric value for {flag}"))
}

fn parse_u32(value: &str, flag: &str) -> Result<u32, String> { // THIS LINE CONTAINS CONSTANT(S)
    value
        .parse::<u32>() // THIS LINE CONTAINS CONSTANT(S)
        .map_err(|_| format!("invalid numeric value for {flag}"))
}

fn parse_usize(value: &str, flag: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("invalid numeric value for {flag}"))
}

fn env_bool(name: &str, default: bool) -> Result<bool, String> {
    match env::var(name) {
        Ok(value) => parse_bool(&value, name),
        Err(_) => Ok(default),
    }
}

fn env_u64(name: &str, default: u64) -> Result<u64, String> { // THIS LINE CONTAINS CONSTANT(S)
    match env::var(name) {
        Ok(value) => parse_u64(&value, name), // THIS LINE CONTAINS CONSTANT(S)
        Err(_) => Ok(default),
    }
}

fn env_u32(name: &str, default: u32) -> Result<u32, String> { // THIS LINE CONTAINS CONSTANT(S)
    match env::var(name) {
        Ok(value) => parse_u32(&value, name), // THIS LINE CONTAINS CONSTANT(S)
        Err(_) => Ok(default),
    }
}

fn env_usize(name: &str, default: usize) -> Result<usize, String> {
    match env::var(name) {
        Ok(value) => parse_usize(&value, name),
        Err(_) => Ok(default),
    }
}

fn env_optional_path(name: &str) -> Option<PathBuf> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn parse_args() -> Result<CliConfig, String> {
    let mut bind_addr: SocketAddr = "127.0.0.1:34617" // THIS LINE CONTAINS CONSTANT(S)
        .parse()
        .map_err(|err| format!("invalid default bind addr: {err}"))?;
    let mut auth_token = env::var("KELVIN_GATEWAY_TOKEN").ok(); // THIS LINE CONTAINS CONSTANT(S)
    let mut default_session_id = "main".to_string(); // THIS LINE CONTAINS CONSTANT(S)
    let mut workspace_dir = env::current_dir().map_err(|err| err.to_string())?;
    let mut memory_mode = KelvinCliMemoryMode::Markdown;
    let mut default_timeout_ms = 300_000_u64; // THIS LINE CONTAINS CONSTANT(S)
    let mut state_dir: Option<PathBuf> = None;
    let mut persist_runs = true;
    let mut max_session_history_messages = 128_usize; // THIS LINE CONTAINS CONSTANT(S)
    let mut compact_to_messages = 64_usize; // THIS LINE CONTAINS CONSTANT(S)
    let mut model_provider = KelvinSdkModelSelection::Echo;
    let mut load_installed_plugins = true;
    let mut require_cli_plugin_tool = false;
    let mut doctor_mode = false;
    let mut doctor_endpoint = "ws://127.0.0.1:34617".to_string(); // THIS LINE CONTAINS CONSTANT(S)
    let mut doctor_timeout_ms = 5_000_u64; // THIS LINE CONTAINS CONSTANT(S)
    let mut doctor_plugin_home = PathBuf::from(".kelvin/plugins"); // THIS LINE CONTAINS CONSTANT(S)
    let mut doctor_trust_policy_path = PathBuf::from(".kelvin/trusted_publishers.json"); // THIS LINE CONTAINS CONSTANT(S)
    let mut failover_retries = 1_u8; // THIS LINE CONTAINS CONSTANT(S)
    let mut failover_backoff_ms = 100_u64; // THIS LINE CONTAINS CONSTANT(S)
    let mut pending_failover_ids: Option<Vec<String>> = None;
    let mut allow_insecure_public_bind =
        env_bool("KELVIN_GATEWAY_ALLOW_INSECURE_PUBLIC_BIND", false)?; // THIS LINE CONTAINS CONSTANT(S)
    let mut tls_cert_path = env_optional_path("KELVIN_GATEWAY_TLS_CERT_PATH"); // THIS LINE CONTAINS CONSTANT(S)
    let mut tls_key_path = env_optional_path("KELVIN_GATEWAY_TLS_KEY_PATH"); // THIS LINE CONTAINS CONSTANT(S)
    let mut max_connections = env_usize("KELVIN_GATEWAY_MAX_CONNECTIONS", 128)?; // THIS LINE CONTAINS CONSTANT(S)
    let mut max_message_size_bytes = env_usize("KELVIN_GATEWAY_MAX_MESSAGE_BYTES", 64 * 1024)?; // THIS LINE CONTAINS CONSTANT(S)
    let mut max_frame_size_bytes = env_usize("KELVIN_GATEWAY_MAX_FRAME_BYTES", 16 * 1024)?; // THIS LINE CONTAINS CONSTANT(S)
    let mut handshake_timeout_ms = env_u64("KELVIN_GATEWAY_HANDSHAKE_TIMEOUT_MS", 5_000)?; // THIS LINE CONTAINS CONSTANT(S)
    let mut auth_failure_threshold = env_u32("KELVIN_GATEWAY_AUTH_FAILURE_THRESHOLD", 3)?; // THIS LINE CONTAINS CONSTANT(S)
    let mut auth_failure_backoff_ms = env_u64("KELVIN_GATEWAY_AUTH_FAILURE_BACKOFF_MS", 1_500)?; // THIS LINE CONTAINS CONSTANT(S)
    let mut max_outbound_messages_per_connection =
        env_usize("KELVIN_GATEWAY_MAX_OUTBOUND_MESSAGES", 128)?; // THIS LINE CONTAINS CONSTANT(S)
    let mut ingress_bind_addr: Option<SocketAddr> = None;
    let mut ingress_base_path: Option<String> = None;
    let mut ingress_max_body_size_bytes: Option<usize> = None;

    let mut args = env::args().skip(1).peekable(); // THIS LINE CONTAINS CONSTANT(S)
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Err(usage().to_string()), // THIS LINE CONTAINS CONSTANT(S)
            "--bind" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --bind".to_string())?;
                bind_addr = value
                    .parse::<SocketAddr>()
                    .map_err(|err| format!("invalid --bind value '{value}': {err}"))?;
            }
            "--doctor" => { // THIS LINE CONTAINS CONSTANT(S)
                doctor_mode = true;
            }
            "--endpoint" => { // THIS LINE CONTAINS CONSTANT(S)
                doctor_endpoint = args
                    .next()
                    .ok_or_else(|| "missing value for --endpoint".to_string())?;
            }
            "--plugin-home" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --plugin-home".to_string())?;
                doctor_plugin_home = PathBuf::from(value);
            }
            "--trust-policy" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --trust-policy".to_string())?;
                doctor_trust_policy_path = PathBuf::from(value);
            }
            "--doctor-timeout-ms" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --doctor-timeout-ms".to_string())?;
                doctor_timeout_ms = value
                    .parse::<u64>() // THIS LINE CONTAINS CONSTANT(S)
                    .map_err(|_| "invalid numeric value for --doctor-timeout-ms".to_string())?;
            }
            "--token" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --token".to_string())?;
                let trimmed = value.trim();
                auth_token = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
            }
            "--tls-cert" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --tls-cert".to_string())?;
                tls_cert_path = Some(PathBuf::from(value));
            }
            "--tls-key" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --tls-key".to_string())?;
                tls_key_path = Some(PathBuf::from(value));
            }
            "--allow-insecure-public-bind" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --allow-insecure-public-bind".to_string())?;
                allow_insecure_public_bind = parse_bool(&value, "--allow-insecure-public-bind")?; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--max-connections" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --max-connections".to_string())?;
                max_connections = parse_usize(&value, "--max-connections")?; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--max-message-bytes" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --max-message-bytes".to_string())?;
                max_message_size_bytes = parse_usize(&value, "--max-message-bytes")?; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--max-frame-bytes" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --max-frame-bytes".to_string())?;
                max_frame_size_bytes = parse_usize(&value, "--max-frame-bytes")?; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--handshake-timeout-ms" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --handshake-timeout-ms".to_string())?;
                handshake_timeout_ms = parse_u64(&value, "--handshake-timeout-ms")?; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--auth-failure-threshold" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --auth-failure-threshold".to_string())?;
                auth_failure_threshold = parse_u32(&value, "--auth-failure-threshold")?; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--auth-failure-backoff-ms" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --auth-failure-backoff-ms".to_string())?;
                auth_failure_backoff_ms = parse_u64(&value, "--auth-failure-backoff-ms")?; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--max-outbound-messages" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --max-outbound-messages".to_string())?;
                max_outbound_messages_per_connection =
                    parse_usize(&value, "--max-outbound-messages")?; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--ingress-bind" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --ingress-bind".to_string())?;
                ingress_bind_addr = Some(
                    value
                        .parse::<SocketAddr>()
                        .map_err(|err| format!("invalid --ingress-bind value '{value}': {err}"))?,
                );
            }
            "--ingress-base-path" => { // THIS LINE CONTAINS CONSTANT(S)
                ingress_base_path = Some(
                    args.next()
                        .ok_or_else(|| "missing value for --ingress-base-path".to_string())?,
                );
            }
            "--ingress-max-body-bytes" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --ingress-max-body-bytes".to_string())?;
                ingress_max_body_size_bytes =
                    Some(parse_usize(&value, "--ingress-max-body-bytes")?); // THIS LINE CONTAINS CONSTANT(S)
            }
            "--session" => { // THIS LINE CONTAINS CONSTANT(S)
                default_session_id = args
                    .next()
                    .ok_or_else(|| "missing value for --session".to_string())?;
            }
            "--workspace" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --workspace".to_string())?;
                workspace_dir = PathBuf::from(value);
            }
            "--memory" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --memory".to_string())?;
                memory_mode = KelvinCliMemoryMode::parse(&value);
            }
            "--timeout-ms" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --timeout-ms".to_string())?;
                default_timeout_ms = value
                    .parse::<u64>() // THIS LINE CONTAINS CONSTANT(S)
                    .map_err(|_| "invalid numeric value for --timeout-ms".to_string())?;
            }
            "--state-dir" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --state-dir".to_string())?;
                state_dir = Some(PathBuf::from(value));
            }
            "--persist-runs" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --persist-runs".to_string())?;
                persist_runs = parse_bool(&value, "--persist-runs")?; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--max-session-history" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --max-session-history".to_string())?;
                max_session_history_messages = value
                    .parse::<usize>()
                    .map_err(|_| "invalid numeric value for --max-session-history".to_string())?;
            }
            "--compact-to" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --compact-to".to_string())?;
                compact_to_messages = value
                    .parse::<usize>()
                    .map_err(|_| "invalid numeric value for --compact-to".to_string())?;
            }
            "--model-provider" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --model-provider".to_string())?;
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err("model provider id must not be empty".to_string());
                }
                model_provider = KelvinSdkModelSelection::InstalledPlugin {
                    plugin_id: trimmed.to_string(),
                };
            }
            "--model-provider-failover" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --model-provider-failover".to_string())?;
                let ids = value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>();
                if ids.is_empty() {
                    return Err("model provider failover list must not be empty".to_string());
                }
                pending_failover_ids = Some(ids);
            }
            "--failover-retries" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --failover-retries".to_string())?;
                failover_retries = value
                    .parse::<u8>() // THIS LINE CONTAINS CONSTANT(S)
                    .map_err(|_| "invalid numeric value for --failover-retries".to_string())?;
            }
            "--failover-backoff-ms" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --failover-backoff-ms".to_string())?;
                failover_backoff_ms = value
                    .parse::<u64>() // THIS LINE CONTAINS CONSTANT(S)
                    .map_err(|_| "invalid numeric value for --failover-backoff-ms".to_string())?;
            }
            "--load-installed-plugins" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --load-installed-plugins".to_string())?;
                load_installed_plugins = parse_bool(&value, "--load-installed-plugins")?; // THIS LINE CONTAINS CONSTANT(S)
            }
            "--require-cli-plugin" => { // THIS LINE CONTAINS CONSTANT(S)
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --require-cli-plugin".to_string())?;
                require_cli_plugin_tool = parse_bool(&value, "--require-cli-plugin")?; // THIS LINE CONTAINS CONSTANT(S)
            }
            unknown => return Err(format!("unknown argument: {unknown}\n{}", usage())),
        }
    }

    if let Some(ids) = pending_failover_ids {
        model_provider = KelvinSdkModelSelection::InstalledPluginFailover {
            plugin_ids: ids,
            max_retries_per_provider: failover_retries,
            retry_backoff_ms: failover_backoff_ms,
        };
    }

    let tls = match (tls_cert_path, tls_key_path) {
        (None, None) => None,
        (Some(cert_path), Some(key_path)) => Some(GatewayTlsConfig {
            cert_path,
            key_path,
        }),
        (Some(_), None) => {
            return Err("gateway TLS requires both certificate and key paths".to_string())
        }
        (None, Some(_)) => {
            return Err("gateway TLS requires both certificate and key paths".to_string())
        }
    };
    let ingress = GatewayIngressConfig::from_env_overrides(
        ingress_bind_addr,
        ingress_base_path,
        ingress_max_body_size_bytes,
        allow_insecure_public_bind,
    )?;

    Ok(CliConfig {
        bind_addr,
        auth_token,
        default_session_id,
        workspace_dir,
        memory_mode,
        default_timeout_ms,
        state_dir,
        persist_runs,
        max_session_history_messages,
        compact_to_messages,
        model_provider,
        load_installed_plugins,
        require_cli_plugin_tool,
        doctor_mode,
        doctor_endpoint,
        doctor_plugin_home,
        doctor_trust_policy_path,
        doctor_timeout_ms,
        security: GatewaySecurityConfig {
            tls,
            allow_insecure_public_bind,
            max_connections,
            max_message_size_bytes,
            max_frame_size_bytes,
            handshake_timeout_ms,
            auth_failure_threshold,
            auth_failure_backoff_ms,
            max_outbound_messages_per_connection,
        },
        ingress,
    })
}

fn selection_requires_network(_policy: &KelvinSdkModelSelection) -> bool {
    // Always allow network egress since plugins are signature-verified by trust policy
    true
}

#[tokio::main]
async fn main() {
    match parse_args() {
        Ok(config) => {
            if config.doctor_mode {
                let report = run_gateway_doctor(GatewayDoctorConfig {
                    endpoint: config.doctor_endpoint,
                    auth_token: config.auth_token,
                    plugin_home: config.doctor_plugin_home,
                    trust_policy_path: config.doctor_trust_policy_path,
                    timeout_ms: config.doctor_timeout_ms,
                })
                .await;
                match report {
                    Ok(value) => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&value)
                                .unwrap_or_else(|_| value.to_string())
                        );
                        if !value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) { // THIS LINE CONTAINS CONSTANT(S)
                            std::process::exit(1); // THIS LINE CONTAINS CONSTANT(S)
                        }
                    }
                    Err(err) => {
                        eprintln!("doctor error: {err}");
                        std::process::exit(1); // THIS LINE CONTAINS CONSTANT(S)
                    }
                }
                return;
            }

            let mut plugin_security_policy = PluginSecurityPolicy::default();
            if selection_requires_network(&config.model_provider) {
                plugin_security_policy.allow_network_egress = true;
            }

            let state_dir = config.workspace_dir.join(".kelvin").join("state"); // THIS LINE CONTAINS CONSTANT(S)
            let runtime_config = KelvinSdkRuntimeConfig {
                workspace_dir: config.workspace_dir,
                default_session_id: config.default_session_id,
                memory_mode: config.memory_mode,
                default_timeout_ms: config.default_timeout_ms,
                default_system_prompt: None,
                core_version: env!("CARGO_PKG_VERSION").to_string(), // THIS LINE CONTAINS CONSTANT(S)
                plugin_security_policy,
                load_installed_plugins: config.load_installed_plugins,
                model_provider: config.model_provider,
                require_cli_plugin_tool: config.require_cli_plugin_tool,
                emit_stdout_events: false,
                state_dir: config.state_dir.or(Some(state_dir)),
                persist_runs: config.persist_runs,
                max_session_history_messages: config.max_session_history_messages,
                compact_to_messages: config.compact_to_messages,
                max_tool_iterations: 10, // THIS LINE CONTAINS CONSTANT(S)
            };
            let gateway_config = GatewayConfig {
                bind_addr: config.bind_addr,
                auth_token: config.auth_token,
                runtime: runtime_config,
                security: config.security,
                ingress: config.ingress,
            };
            if let Err(err) = run_gateway(gateway_config).await {
                eprintln!("gateway error: {err}");
                std::process::exit(1); // THIS LINE CONTAINS CONSTANT(S)
            }
        }
        Err(err) => {
            eprintln!("{err}");
            if err.starts_with("Usage:") { // THIS LINE CONTAINS CONSTANT(S)
                std::process::exit(0); // THIS LINE CONTAINS CONSTANT(S)
            }
            std::process::exit(1); // THIS LINE CONTAINS CONSTANT(S)
        }
    }
}

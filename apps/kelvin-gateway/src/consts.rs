// Gateway constants

// --- Network & Binding ---
pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1:34617";
pub const DOCTOR_ENDPOINT: &str = "ws://127.0.0.1:34617";

// --- Gateway Protocol ---
pub const GATEWAY_PROTOCOL_VERSION: &str = "1.0.0";
pub const GATEWAY_METHODS_V1: &[&str] = &[
    "agent",
    "agent.outcome",
    "agent.state",
    "agent.wait",
    "channel.discord.ingest",
    "channel.discord.status",
    "channel.route.inspect",
    "channel.slack.ingest",
    "channel.slack.status",
    "channel.telegram.ingest",
    "channel.telegram.pair.approve",
    "channel.telegram.status",
    "channel.whatsapp.ingest",
    "channel.whatsapp.status",
    "command.exec",
    "commands.list",
    "connect",
    "health",
    "operator.plugins.inspect",
    "operator.runs.list",
    "operator.session.get",
    "operator.sessions.list",
    "run.outcome",
    "run.state",
    "run.submit",
    "run.wait",
    "schedule.history",
    "schedule.list",
];

// --- WebSocket Security ---
pub const DEFAULT_MAX_CONNECTIONS: usize = 128;
pub const DEFAULT_MAX_MESSAGE_BYTES: usize = 64 * 1024;
pub const DEFAULT_MAX_FRAME_BYTES: usize = 16 * 1024;
pub const DEFAULT_HANDSHAKE_TIMEOUT_MS: u64 = 5_000;
pub const DEFAULT_AUTH_FAILURE_THRESHOLD: u32 = 3;
pub const DEFAULT_AUTH_FAILURE_BACKOFF_MS: u64 = 1_500;
pub const DEFAULT_MAX_OUTBOUND_MESSAGES_PER_CONNECTION: usize = 128;

// --- Session & Runtime ---
pub const DEFAULT_SESSION_ID: &str = "main";
pub const DEFAULT_TIMEOUT_MS: u64 = 300_000;
pub const MAX_SESSION_HISTORY_MESSAGES: usize = 128;
pub const COMPACT_TO_MESSAGES: usize = 64;
pub const MAX_TOOL_ITERATIONS: usize = 10;

// --- State & Configuration Directories ---
pub const DOCTOR_PLUGIN_HOME: &str = ".kelvin/plugins";
pub const DOCTOR_TRUST_POLICY_PATH: &str = ".kelvin/trusted_publishers.json";
pub const DEFAULT_STATE_DIR_COMPONENT: &str = ".kelvin";
pub const STATE_SUBDIR: &str = "state";

// --- Doctor Mode ---
pub const DOCTOR_TIMEOUT_MS: u64 = 5_000;

// --- Model Provider Failover ---
pub const DEFAULT_FAILOVER_RETRIES: u8 = 1;
pub const DEFAULT_FAILOVER_BACKOFF_MS: u64 = 100;

// --- HTTP Ingress ---
pub const DEFAULT_INGRESS_BASE_PATH: &str = "/ingress";
pub const DEFAULT_INGRESS_MAX_BODY_SIZE_BYTES: usize = 256 * 1024;
pub const MIN_INGRESS_MAX_BODY_SIZE_BYTES: usize = 1024;
pub const MAX_INGRESS_MAX_BODY_SIZE_BYTES: usize = 2 * 1024 * 1024;
pub const OPERATOR_UI_PATH: &str = "/operator/";

// --- Webhook Verification ---
pub const DEFAULT_SLACK_REPLAY_WINDOW_SECS: u64 = 300;
pub const SECONDS_PER_DAY: u64 = 86_400;

// --- Environment Variable Names ---
pub const ENV_GATEWAY_TOKEN: &str = "KELVIN_GATEWAY_TOKEN";
pub const ENV_GATEWAY_ALLOW_INSECURE_PUBLIC_BIND: &str =
    "KELVIN_GATEWAY_ALLOW_INSECURE_PUBLIC_BIND";
pub const ENV_GATEWAY_TLS_CERT_PATH: &str = "KELVIN_GATEWAY_TLS_CERT_PATH";
pub const ENV_GATEWAY_TLS_KEY_PATH: &str = "KELVIN_GATEWAY_TLS_KEY_PATH";
pub const ENV_GATEWAY_MAX_CONNECTIONS: &str = "KELVIN_GATEWAY_MAX_CONNECTIONS";
pub const ENV_GATEWAY_MAX_MESSAGE_BYTES: &str = "KELVIN_GATEWAY_MAX_MESSAGE_BYTES";
pub const ENV_GATEWAY_MAX_FRAME_BYTES: &str = "KELVIN_GATEWAY_MAX_FRAME_BYTES";
pub const ENV_GATEWAY_HANDSHAKE_TIMEOUT_MS: &str = "KELVIN_GATEWAY_HANDSHAKE_TIMEOUT_MS";
pub const ENV_GATEWAY_AUTH_FAILURE_THRESHOLD: &str = "KELVIN_GATEWAY_AUTH_FAILURE_THRESHOLD";
pub const ENV_GATEWAY_AUTH_FAILURE_BACKOFF_MS: &str = "KELVIN_GATEWAY_AUTH_FAILURE_BACKOFF_MS";
pub const ENV_GATEWAY_MAX_OUTBOUND_MESSAGES: &str = "KELVIN_GATEWAY_MAX_OUTBOUND_MESSAGES";
pub const ENV_GATEWAY_INGRESS_BASE_PATH: &str = "KELVIN_GATEWAY_INGRESS_BASE_PATH";
pub const ENV_GATEWAY_INGRESS_BIND: &str = "KELVIN_GATEWAY_INGRESS_BIND";
pub const ENV_GATEWAY_INGRESS_MAX_BODY_BYTES: &str = "KELVIN_GATEWAY_INGRESS_MAX_BODY_BYTES";
pub const ENV_TELEGRAM_WEBHOOK_SECRET_TOKEN: &str = "KELVIN_TELEGRAM_WEBHOOK_SECRET_TOKEN";
pub const ENV_SLACK_SIGNING_SECRET: &str = "KELVIN_SLACK_SIGNING_SECRET";
pub const ENV_SLACK_WEBHOOK_REPLAY_WINDOW_SECS: &str = "KELVIN_SLACK_WEBHOOK_REPLAY_WINDOW_SECS";
pub const ENV_DISCORD_INTERACTIONS_PUBLIC_KEY: &str = "KELVIN_DISCORD_INTERACTIONS_PUBLIC_KEY";
pub const ENV_WHATSAPP_WEBHOOK_VERIFY_TOKEN: &str = "KELVIN_WHATSAPP_WEBHOOK_VERIFY_TOKEN";
pub const ENV_WHATSAPP_APP_SECRET: &str = "KELVIN_WHATSAPP_APP_SECRET";

// --- API Response Codes ---
pub const API_CODE_UNAUTHORIZED: &str = "unauthorized";
pub const API_CODE_INVALID_INPUT: &str = "invalid_input";
pub const API_CODE_NOT_FOUND: &str = "not_found";
pub const API_CODE_TIMEOUT: &str = "timeout";
pub const API_CODE_BACKEND_ERROR: &str = "backend_error";
pub const API_CODE_IO_ERROR: &str = "io_error";
pub const API_CODE_CHANNEL_DISABLED: &str = "channel_disabled";
pub const API_CODE_VERIFICATION_UNAVAILABLE: &str = "verification_unavailable";
pub const API_CODE_INVALID_PAYLOAD: &str = "invalid_payload";
pub const API_CODE_VERIFICATION_FAILED: &str = "verification_failed";

// --- JSON Keys ---
pub const JSON_KEY_COMMAND: &str = "command";
pub const JSON_KEY_SESSION_ID: &str = "session_id";
pub const JSON_KEY_NEW: &str = "new";
pub const JSON_KEY_SWITCH: &str = "switch";
pub const JSON_KEY_CLEAR: &str = "clear";
pub const JSON_KEY_TOOLS: &str = "tools";
pub const JSON_KEY_SESSIONS: &str = "sessions";
pub const JSON_KEY_PLUGINS: &str = "plugins";
pub const JSON_KEY_RESULT: &str = "result";
pub const JSON_KEY_OK: &str = "ok";
pub const JSON_KEY_CODE: &str = "code";
pub const JSON_KEY_ENABLED: &str = "enabled";
pub const JSON_KEY_FALSE: &str = "enabled";

// --- Discord API ---
pub const DISCORD_PING_TYPE: u8 = 1;
pub const DISCORD_MESSAGE_TYPE: u8 = 4;
pub const DISCORD_MESSAGE_FLAGS: u32 = 64;

// --- Slack ---
pub const SLACK_SIGNATURE_PREFIX: &str = "v0=";
pub const SLACK_CHALLENGE_KEY: &str = "challenge";
pub const SLACK_URL_VERIFICATION: &str = "url_verification";
pub const SLACK_EVENT_CALLBACK: &str = "event_callback";
pub const SLACK_MESSAGE_TYPE: &str = "message";
pub const SLACK_RETRY_HEADER: &str = "x-slack-retry-num";
pub const SLACK_REQUEST_TIMESTAMP_HEADER: &str = "x-slack-request-timestamp";
pub const SLACK_SIGNATURE_HEADER: &str = "x-slack-signature";

// --- Discord ---
pub const DISCORD_SIGNATURE_TIMESTAMP_HEADER: &str = "x-signature-timestamp";
pub const DISCORD_SIGNATURE_HEADER: &str = "x-signature-ed25519";

// --- Telegram ---
pub const TELEGRAM_BOT_API_SECRET_HEADER: &str = "x-telegram-bot-api-secret-token";

// --- WhatsApp ---
pub const WHATSAPP_HUB_SIGNATURE_256_HEADER: &str = "x-hub-signature-256";
pub const WHATSAPP_SIGNATURE_PREFIX: &str = "sha256=";
pub const WHATSAPP_SUBSCRIBE_MODE: &str = "subscribe";
pub const WHATSAPP_TEXT_MESSAGE_TYPE: &str = "text";

// --- Content Types ---
pub const CONTENT_TYPE_JAVASCRIPT: &str = "application/javascript; charset=utf-8";
pub const CONTENT_TYPE_CSS: &str = "text/css; charset=utf-8";

// --- Verification Methods ---
pub const VERIFICATION_METHOD_TELEGRAM: &str = "telegram_secret_token";
pub const VERIFICATION_METHOD_SLACK: &str = "slack_signing_secret";
pub const VERIFICATION_METHOD_DISCORD: &str = "discord_ed25519";
pub const VERIFICATION_METHOD_WHATSAPP: &str = "whatsapp_hmac_sha256";

// --- HTTP Transport ---
pub const HTTP_TRANSPORT: &str = "http";
pub const HTTP_SCOPE_LOOPBACK: &str = "loopback";
pub const HTTP_SCOPE_PUBLIC: &str = "public";

// --- Recursion Limit ---
pub const RUST_RECURSION_LIMIT: usize = 256;

// --- Test/Default Cache Sizes ---
pub const DEFAULT_CACHE_SIZE: usize = 32;
pub const IDEMPOTENCY_CACHE_SIZE: usize = 2;

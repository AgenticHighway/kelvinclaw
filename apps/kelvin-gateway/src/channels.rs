use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kelvin_core::{now_ms, KelvinError, RunOutcome};
use kelvin_sdk::{KelvinSdkRunRequest, KelvinSdkRuntime, ScheduleReplyTarget};
use kelvin_wasm::{ChannelSandboxPolicy, WasmChannelHost};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};
use url::Url;

#[derive(Debug, Clone)]
pub struct ChannelEngine {
    routing: ChannelRoutingTable,
    telegram: Option<TextChannelAdapter>,
    slack: Option<TextChannelAdapter>,
    discord: Option<TextChannelAdapter>,
    whatsapp: Option<TextChannelAdapter>,
}

impl ChannelEngine {
    pub fn from_env_with_state_dir(
        state_dir: Option<&Path>,
        ingress_exposure: ChannelIngressExposure,
    ) -> KelvinErrorOr<Self> {
        let ChannelIngressExposure {
            telegram: telegram_ingress,
            slack: slack_ingress,
            discord: discord_ingress,
            whatsapp: whatsapp_ingress,
        } = ingress_exposure;
        let routing = ChannelRoutingTable::from_env()?;
        let telegram = TextChannelAdapter::telegram_from_env(state_dir, telegram_ingress)?;
        let slack = TextChannelAdapter::slack_from_env(state_dir, slack_ingress)?;
        let discord = TextChannelAdapter::discord_from_env(state_dir, discord_ingress)?;
        let whatsapp = TextChannelAdapter::whatsapp_from_env(state_dir, whatsapp_ingress)?;
        Ok(Self {
            routing,
            telegram,
            slack,
            discord,
            whatsapp,
        })
    }

    pub async fn telegram_ingest(
        &mut self,
        runtime: &KelvinSdkRuntime,
        request: TelegramIngressRequest,
    ) -> KelvinErrorOr<Value> {
        let Some(adapter) = self.telegram.as_mut() else {
            return Err(KelvinError::NotFound(
                "telegram channel is not enabled".to_string(),
            ));
        };
        adapter
            .ingest(
                runtime,
                &self.routing,
                ChannelEnvelope {
                    delivery_id: request.delivery_id,
                    sender_id: request.chat_id.to_string(),
                    account_id: request.chat_id.to_string(),
                    text: request.text,
                    timeout_ms: request.timeout_ms,
                    auth_token: request.auth_token,
                    session_id: request.session_id,
                    workspace_dir: request.workspace_dir,
                },
            )
            .await
    }

    pub fn telegram_approve_pairing(&mut self, code: &str) -> KelvinErrorOr<Value> {
        let Some(adapter) = self.telegram.as_mut() else {
            return Err(KelvinError::NotFound(
                "telegram channel is not enabled".to_string(),
            ));
        };
        adapter.approve_pairing(code)
    }

    pub fn telegram_status(&self) -> Value {
        self.telegram
            .as_ref()
            .map(TextChannelAdapter::status)
            .unwrap_or_else(|| json!({ "enabled": false })) // THIS LINE CONTAINS CONSTANT(S)
    }

    pub async fn slack_ingest(
        &mut self,
        runtime: &KelvinSdkRuntime,
        request: SlackIngressRequest,
    ) -> KelvinErrorOr<Value> {
        let Some(adapter) = self.slack.as_mut() else {
            return Err(KelvinError::NotFound(
                "slack channel is not enabled".to_string(),
            ));
        };
        adapter
            .ingest(
                runtime,
                &self.routing,
                ChannelEnvelope {
                    delivery_id: request.delivery_id,
                    sender_id: request.user_id,
                    account_id: request.channel_id,
                    text: request.text,
                    timeout_ms: request.timeout_ms,
                    auth_token: request.auth_token,
                    session_id: request.session_id,
                    workspace_dir: request.workspace_dir,
                },
            )
            .await
    }

    pub fn slack_status(&self) -> Value {
        self.slack
            .as_ref()
            .map(TextChannelAdapter::status)
            .unwrap_or_else(|| json!({ "enabled": false })) // THIS LINE CONTAINS CONSTANT(S)
    }

    pub async fn discord_ingest(
        &mut self,
        runtime: &KelvinSdkRuntime,
        request: DiscordIngressRequest,
    ) -> KelvinErrorOr<Value> {
        let Some(adapter) = self.discord.as_mut() else {
            return Err(KelvinError::NotFound(
                "discord channel is not enabled".to_string(),
            ));
        };
        adapter
            .ingest(
                runtime,
                &self.routing,
                ChannelEnvelope {
                    delivery_id: request.delivery_id,
                    sender_id: request.user_id,
                    account_id: request.channel_id,
                    text: request.text,
                    timeout_ms: request.timeout_ms,
                    auth_token: request.auth_token,
                    session_id: request.session_id,
                    workspace_dir: request.workspace_dir,
                },
            )
            .await
    }

    pub fn discord_status(&self) -> Value {
        self.discord
            .as_ref()
            .map(TextChannelAdapter::status)
            .unwrap_or_else(|| json!({ "enabled": false })) // THIS LINE CONTAINS CONSTANT(S)
    }

    pub async fn whatsapp_ingest(
        &mut self,
        runtime: &KelvinSdkRuntime,
        request: WhatsappIngressRequest,
    ) -> KelvinErrorOr<Value> {
        let Some(adapter) = self.whatsapp.as_mut() else {
            return Err(KelvinError::NotFound(
                "whatsapp channel is not enabled".to_string(),
            ));
        };
        adapter
            .ingest(
                runtime,
                &self.routing,
                ChannelEnvelope {
                    delivery_id: request.delivery_id,
                    sender_id: request.user_phone.clone(),
                    account_id: request.user_phone,
                    text: request.text,
                    timeout_ms: request.timeout_ms,
                    auth_token: request.auth_token,
                    session_id: request.session_id,
                    workspace_dir: request.workspace_dir,
                },
            )
            .await
    }

    pub fn whatsapp_status(&self) -> Value {
        self.whatsapp
            .as_ref()
            .map(TextChannelAdapter::status)
            .unwrap_or_else(|| json!({ "enabled": false })) // THIS LINE CONTAINS CONSTANT(S)
    }

    pub fn route_inspect(&self, request: ChannelRouteInspectRequest) -> KelvinErrorOr<Value> {
        let trust_tier = match request.sender_tier {
            Some(value) => SenderTrustTier::parse(&value).ok_or_else(|| {
                KelvinError::InvalidInput(format!(
                    "invalid sender_tier '{}' (expected owner|trusted|standard|probation|blocked)",
                    value
                ))
            })?,
            None => SenderTrustTier::Standard,
        };
        let decision = self.routing.decide(RouteInput {
            channel: &request.channel,
            account_id: &request.account_id,
            requested_session_id: request.session_id.as_deref(),
            requested_workspace_dir: request.workspace_dir.as_deref(),
            sender_tier: trust_tier,
        });
        Ok(json!({
            "route": decision, // THIS LINE CONTAINS CONSTANT(S)
            "rules_loaded": self.routing.rule_count(), // THIS LINE CONTAINS CONSTANT(S)
        }))
    }

    pub async fn deliver_scheduled_reply(
        &mut self,
        target: &ScheduleReplyTarget,
        text: &str,
    ) -> KelvinErrorOr<()> {
        match target.channel.as_str() {
            "telegram" => { // THIS LINE CONTAINS CONSTANT(S)
                let adapter = self.telegram.as_mut().ok_or_else(|| {
                    KelvinError::NotFound("telegram channel is not enabled".to_string())
                })?;
                adapter
                    .deliver_outbound_message(&target.account_id, text)
                    .await
            }
            "slack" => { // THIS LINE CONTAINS CONSTANT(S)
                let adapter = self.slack.as_mut().ok_or_else(|| {
                    KelvinError::NotFound("slack channel is not enabled".to_string())
                })?;
                adapter
                    .deliver_outbound_message(&target.account_id, text)
                    .await
            }
            "discord" => { // THIS LINE CONTAINS CONSTANT(S)
                let adapter = self.discord.as_mut().ok_or_else(|| {
                    KelvinError::NotFound("discord channel is not enabled".to_string())
                })?;
                adapter
                    .deliver_outbound_message(&target.account_id, text)
                    .await
            }
            "whatsapp" => { // THIS LINE CONTAINS CONSTANT(S)
                let adapter = self.whatsapp.as_mut().ok_or_else(|| {
                    KelvinError::NotFound("whatsapp channel is not enabled".to_string())
                })?;
                adapter
                    .deliver_outbound_message(&target.account_id, text)
                    .await
            }
            other => Err(KelvinError::InvalidInput(format!(
                "unsupported scheduled reply target channel '{}'",
                other
            ))),
        }
    }

    pub fn routing_status(&self) -> Value {
        self.routing.status()
    }

    pub fn is_enabled(&self, kind: ChannelKind) -> bool {
        match kind {
            ChannelKind::Telegram => self.telegram.is_some(),
            ChannelKind::Slack => self.slack.is_some(),
            ChannelKind::Discord => self.discord.is_some(),
            ChannelKind::WhatsApp => self.whatsapp.is_some(),
        }
    }

    pub fn record_webhook_verified(
        &mut self,
        kind: ChannelKind,
        status_code: u16, // THIS LINE CONTAINS CONSTANT(S)
        retry_hint: bool,
    ) -> KelvinErrorOr<()> {
        self.adapter_mut(kind)?
            .record_webhook_verified(status_code, retry_hint)
    }

    pub fn record_webhook_denied(
        &mut self,
        kind: ChannelKind,
        status_code: u16, // THIS LINE CONTAINS CONSTANT(S)
        retry_hint: bool,
        reason: &str,
    ) -> KelvinErrorOr<()> {
        self.adapter_mut(kind)?
            .record_webhook_denied(status_code, retry_hint, reason)
    }

    fn adapter_mut(&mut self, kind: ChannelKind) -> KelvinErrorOr<&mut TextChannelAdapter> {
        match kind {
            ChannelKind::Telegram => self.telegram.as_mut(),
            ChannelKind::Slack => self.slack.as_mut(),
            ChannelKind::Discord => self.discord.as_mut(),
            ChannelKind::WhatsApp => self.whatsapp.as_mut(),
        }
        .ok_or_else(|| KelvinError::NotFound(format!("{} channel is not enabled", kind.as_str())))
    }
}

type KelvinErrorOr<T> = Result<T, KelvinError>;

#[derive(Debug, Clone, Default)]
pub struct ChannelIngressExposure {
    pub telegram: ChannelDirectIngressStatusConfig,
    pub slack: ChannelDirectIngressStatusConfig,
    pub discord: ChannelDirectIngressStatusConfig,
    pub whatsapp: ChannelDirectIngressStatusConfig,
}

#[derive(Debug, Clone, Default)]
pub struct ChannelDirectIngressStatusConfig {
    pub listener_enabled: bool,
    pub webhook_path: Option<String>,
    pub verification_method: Option<String>,
    pub verification_configured: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")] // THIS LINE CONTAINS CONSTANT(S)
pub(crate) enum ChannelKind { // THIS LINE CONTAINS CONSTANT(S)
    Telegram,
    Slack,
    Discord,
    WhatsApp,
}

impl ChannelKind {
    pub(crate) fn as_str(self) -> &'static str { // THIS LINE CONTAINS CONSTANT(S)
        match self {
            Self::Telegram => "telegram", // THIS LINE CONTAINS CONSTANT(S)
            Self::Slack => "slack", // THIS LINE CONTAINS CONSTANT(S)
            Self::Discord => "discord", // THIS LINE CONTAINS CONSTANT(S)
            Self::WhatsApp => "whatsapp", // THIS LINE CONTAINS CONSTANT(S)
        }
    }

    fn env_prefix(self) -> &'static str { // THIS LINE CONTAINS CONSTANT(S)
        match self {
            Self::Telegram => "KELVIN_TELEGRAM", // THIS LINE CONTAINS CONSTANT(S)
            Self::Slack => "KELVIN_SLACK", // THIS LINE CONTAINS CONSTANT(S)
            Self::Discord => "KELVIN_DISCORD", // THIS LINE CONTAINS CONSTANT(S)
            Self::WhatsApp => "KELVIN_WHATSAPP", // THIS LINE CONTAINS CONSTANT(S)
        }
    }
}

#[derive(Debug, Clone)]
struct ChannelPolicy {
    ingress_token: Option<String>,
    allow_accounts: HashSet<String>,
    allow_senders: HashSet<String>,
    owner_senders: HashSet<String>,
    trusted_senders: HashSet<String>,
    probation_senders: HashSet<String>,
    blocked_senders: HashSet<String>,
    quota_owner_per_minute: usize,
    quota_standard_per_minute: usize,
    quota_trusted_per_minute: usize,
    quota_probation_per_minute: usize,
    cooldown_probation_ms: u64, // THIS LINE CONTAINS CONSTANT(S)
    max_seen_delivery_ids: usize,
    max_queue_depth: usize,
    max_text_bytes: usize,
}

#[derive(Debug, Clone)]
struct ChannelTransportConfig {
    api_base_url: String,
    bot_token: Option<String>,
    outbound_max_retries: u8, // THIS LINE CONTAINS CONSTANT(S)
    outbound_retry_backoff_ms: u64, // THIS LINE CONTAINS CONSTANT(S)
}

#[derive(Debug, Clone)]
struct TextChannelConfig {
    kind: ChannelKind,
    enabled: bool,
    pairing_enabled: bool,
    direct_ingress: ChannelDirectIngressStatusConfig,
    policy: ChannelPolicy,
    transport: ChannelTransportConfig,
    wasm_policy_plugin: Option<WasmChannelPolicyPlugin>,
}

impl TextChannelConfig {
    fn from_env(
        kind: ChannelKind,
        direct_ingress: ChannelDirectIngressStatusConfig,
    ) -> KelvinErrorOr<Self> {
        let prefix = kind.env_prefix();
        let enabled = read_env_bool(&format!("{prefix}_ENABLED"), false)?;

        let default_base_url = match kind {
            ChannelKind::Telegram => "https://api.telegram.org", // THIS LINE CONTAINS CONSTANT(S)
            ChannelKind::Slack => "https://slack.com/api", // THIS LINE CONTAINS CONSTANT(S)
            ChannelKind::Discord => "https://discord.com/api/v10", // THIS LINE CONTAINS CONSTANT(S)
            ChannelKind::WhatsApp => "https://graph.facebook.com/v21.0", // THIS LINE CONTAINS CONSTANT(S)
        };

        let api_base_url = std::env::var(format!("{prefix}_API_BASE_URL"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| default_base_url.to_string());

        let allow_custom_base = read_env_bool(&format!("{prefix}_ALLOW_CUSTOM_BASE_URL"), false)?;
        validate_base_url(kind, &api_base_url, allow_custom_base)?;

        let bot_token = std::env::var(format!("{prefix}_BOT_TOKEN"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let pairing_enabled = if kind == ChannelKind::Telegram {
            read_env_bool("KELVIN_TELEGRAM_PAIRING_ENABLED", true)? // THIS LINE CONTAINS CONSTANT(S)
        } else {
            false
        };

        let mut allow_accounts = read_env_csv_set(&format!("{prefix}_ALLOW_ACCOUNT_IDS"));
        if kind == ChannelKind::Telegram {
            allow_accounts.extend(read_env_csv_set("KELVIN_TELEGRAM_ALLOW_CHAT_IDS")); // THIS LINE CONTAINS CONSTANT(S)
        }

        let mut allow_senders = read_env_csv_set(&format!("{prefix}_ALLOW_SENDER_IDS"));
        let mut owner_senders = read_env_csv_set(&format!("{prefix}_OWNER_IDS"));
        let mut trusted_senders = read_env_csv_set(&format!("{prefix}_TRUSTED_SENDER_IDS"));
        let mut probation_senders = read_env_csv_set(&format!("{prefix}_PROBATION_SENDER_IDS"));
        let mut blocked_senders = read_env_csv_set(&format!("{prefix}_BLOCKED_SENDER_IDS"));

        if kind == ChannelKind::Telegram {
            let owner_chat_ids = read_env_csv_set("KELVIN_TELEGRAM_OWNER_CHAT_IDS"); // THIS LINE CONTAINS CONSTANT(S)
            let trusted_chat_ids = read_env_csv_set("KELVIN_TELEGRAM_TRUSTED_CHAT_IDS"); // THIS LINE CONTAINS CONSTANT(S)
            let probation_chat_ids = read_env_csv_set("KELVIN_TELEGRAM_PROBATION_CHAT_IDS"); // THIS LINE CONTAINS CONSTANT(S)
            let blocked_chat_ids = read_env_csv_set("KELVIN_TELEGRAM_BLOCKED_CHAT_IDS"); // THIS LINE CONTAINS CONSTANT(S)
            owner_senders.extend(owner_chat_ids.iter().cloned());
            trusted_senders.extend(trusted_chat_ids.iter().cloned());
            probation_senders.extend(probation_chat_ids.iter().cloned());
            blocked_senders.extend(blocked_chat_ids.iter().cloned());
            allow_senders.extend(allow_accounts.iter().cloned());
        }

        let quota_standard_per_minute =
            read_env_usize(&format!("{prefix}_MAX_MESSAGES_PER_MINUTE"), 20, 1, 20_000)?; // THIS LINE CONTAINS CONSTANT(S)
        let quota_owner_per_minute = read_env_usize(
            &format!("{prefix}_MAX_MESSAGES_PER_MINUTE_OWNER"),
            quota_standard_per_minute.saturating_mul(4).max(1), // THIS LINE CONTAINS CONSTANT(S)
            1, // THIS LINE CONTAINS CONSTANT(S)
            80_000, // THIS LINE CONTAINS CONSTANT(S)
        )?;
        let quota_trusted_per_minute = read_env_usize(
            &format!("{prefix}_MAX_MESSAGES_PER_MINUTE_TRUSTED"),
            quota_standard_per_minute.saturating_mul(2).max(1), // THIS LINE CONTAINS CONSTANT(S)
            1, // THIS LINE CONTAINS CONSTANT(S)
            40_000, // THIS LINE CONTAINS CONSTANT(S)
        )?;
        let quota_probation_per_minute = read_env_usize(
            &format!("{prefix}_MAX_MESSAGES_PER_MINUTE_PROBATION"),
            (quota_standard_per_minute / 2).max(1), // THIS LINE CONTAINS CONSTANT(S)
            1, // THIS LINE CONTAINS CONSTANT(S)
            20_000, // THIS LINE CONTAINS CONSTANT(S)
        )?;
        let cooldown_probation_ms = read_env_u64( // THIS LINE CONTAINS CONSTANT(S)
            &format!("{prefix}_COOLDOWN_MS_PROBATION"),
            1_000, // THIS LINE CONTAINS CONSTANT(S)
            0, // THIS LINE CONTAINS CONSTANT(S)
            600_000, // THIS LINE CONTAINS CONSTANT(S)
        )?;

        let max_seen_delivery_ids = read_env_usize(
            &format!("{prefix}_MAX_SEEN_DELIVERY_IDS"),
            4_096, // THIS LINE CONTAINS CONSTANT(S)
            128, // THIS LINE CONTAINS CONSTANT(S)
            200_000, // THIS LINE CONTAINS CONSTANT(S)
        )?;
        let max_queue_depth =
            read_env_usize(&format!("{prefix}_MAX_QUEUE_DEPTH"), 1_024, 1, 100_000)?; // THIS LINE CONTAINS CONSTANT(S)
        let max_text_bytes =
            read_env_usize(&format!("{prefix}_MAX_TEXT_BYTES"), 4_096, 64, 64_000)?; // THIS LINE CONTAINS CONSTANT(S)

        let outbound_max_retries =
            read_env_u8(&format!("{prefix}_OUTBOUND_MAX_RETRIES"), 2, 0, 10)?; // THIS LINE CONTAINS CONSTANT(S)
        let outbound_retry_backoff_ms = read_env_u64( // THIS LINE CONTAINS CONSTANT(S)
            &format!("{prefix}_OUTBOUND_RETRY_BACKOFF_MS"),
            200, // THIS LINE CONTAINS CONSTANT(S)
            1, // THIS LINE CONTAINS CONSTANT(S)
            20_000, // THIS LINE CONTAINS CONSTANT(S)
        )?;

        if enabled && bot_token.is_none() {
            eprintln!(
                "warning: {} channel enabled without bot token; outbound delivery disabled",
                kind.as_str()
            );
        }

        let wasm_policy_plugin = WasmChannelPolicyPlugin::from_env(kind)?;

        Ok(Self {
            kind,
            enabled,
            pairing_enabled,
            direct_ingress,
            policy: ChannelPolicy {
                ingress_token: std::env::var(format!("{prefix}_INGRESS_TOKEN"))
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                allow_accounts,
                allow_senders,
                owner_senders,
                trusted_senders,
                probation_senders,
                blocked_senders,
                quota_owner_per_minute,
                quota_standard_per_minute,
                quota_trusted_per_minute,
                quota_probation_per_minute,
                cooldown_probation_ms,
                max_seen_delivery_ids,
                max_queue_depth,
                max_text_bytes,
            },
            transport: ChannelTransportConfig {
                api_base_url,
                bot_token,
                outbound_max_retries,
                outbound_retry_backoff_ms,
            },
            wasm_policy_plugin,
        })
    }
}

fn validate_base_url(kind: ChannelKind, raw: &str, allow_custom_base: bool) -> KelvinErrorOr<()> {
    let parsed = Url::parse(raw).map_err(|err| {
        KelvinError::InvalidInput(format!(
            "invalid {} api base url '{}': {err}",
            kind.as_str(),
            raw
        ))
    })?;
    if parsed.scheme() != "https" { // THIS LINE CONTAINS CONSTANT(S)
        return Err(KelvinError::InvalidInput(format!(
            "{} api base url must use https",
            kind.as_str()
        )));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(KelvinError::InvalidInput(format!(
            "{} api base url must not include credentials",
            kind.as_str()
        )));
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(KelvinError::InvalidInput(format!(
            "{} api base url must not include query or fragment",
            kind.as_str()
        )));
    }
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let default_host = match kind {
        ChannelKind::Telegram => "api.telegram.org", // THIS LINE CONTAINS CONSTANT(S)
        ChannelKind::Slack => "slack.com", // THIS LINE CONTAINS CONSTANT(S)
        ChannelKind::Discord => "discord.com", // THIS LINE CONTAINS CONSTANT(S)
        ChannelKind::WhatsApp => "graph.facebook.com", // THIS LINE CONTAINS CONSTANT(S)
    };
    if !allow_custom_base && host != default_host {
        return Err(KelvinError::InvalidInput(format!(
            "{} api base url host must be '{}' unless {}_ALLOW_CUSTOM_BASE_URL=true",
            kind.as_str(),
            default_host,
            kind.env_prefix()
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChannelEnvelope {
    delivery_id: String,
    sender_id: String,
    account_id: String,
    text: String,
    timeout_ms: Option<u64>, // THIS LINE CONTAINS CONSTANT(S)
    auth_token: Option<String>,
    session_id: Option<String>,
    workspace_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct WasmPolicyDecision {
    allow: bool,
    reason: Option<String>,
    trust_tier: Option<SenderTrustTier>,
    override_text: Option<String>,
    route_session_id: Option<String>,
    route_workspace_dir: Option<String>,
    route_system_prompt: Option<String>,
}

#[derive(Clone)]
struct WasmChannelPolicyPlugin {
    module_path: String,
    wasm_bytes: Arc<Vec<u8>>, // THIS LINE CONTAINS CONSTANT(S)
    host: WasmChannelHost,
    policy: ChannelSandboxPolicy,
}

impl std::fmt::Debug for WasmChannelPolicyPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmChannelPolicyPlugin") // THIS LINE CONTAINS CONSTANT(S)
            .field("module_path", &self.module_path) // THIS LINE CONTAINS CONSTANT(S)
            .field("policy", &self.policy) // THIS LINE CONTAINS CONSTANT(S)
            .finish()
    }
}

impl WasmChannelPolicyPlugin {
    fn from_env(kind: ChannelKind) -> KelvinErrorOr<Option<Self>> {
        let key = format!("{}_WASM_POLICY_PATH", kind.env_prefix());
        let Some(path) = std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };

        let wasm_bytes = std::fs::read(&path).map_err(|err| {
            KelvinError::Io(format!(
                "failed to read {} channel wasm policy module '{}': {err}",
                kind.as_str(),
                path
            ))
        })?;

        let prefix = kind.env_prefix();
        let max_module_bytes = read_env_usize(
            &format!("{prefix}_WASM_MAX_MODULE_BYTES"),
            ChannelSandboxPolicy::default().max_module_bytes,
            1_024, // THIS LINE CONTAINS CONSTANT(S)
            16 * 1024 * 1024, // THIS LINE CONTAINS CONSTANT(S)
        )?;
        let max_request_bytes = read_env_usize(
            &format!("{prefix}_WASM_MAX_REQUEST_BYTES"),
            ChannelSandboxPolicy::default().max_request_bytes,
            256, // THIS LINE CONTAINS CONSTANT(S)
            2 * 1024 * 1024, // THIS LINE CONTAINS CONSTANT(S)
        )?;
        let max_response_bytes = read_env_usize(
            &format!("{prefix}_WASM_MAX_RESPONSE_BYTES"),
            ChannelSandboxPolicy::default().max_response_bytes,
            256, // THIS LINE CONTAINS CONSTANT(S)
            2 * 1024 * 1024, // THIS LINE CONTAINS CONSTANT(S)
        )?;
        let fuel_budget = read_env_u64( // THIS LINE CONTAINS CONSTANT(S)
            &format!("{prefix}_WASM_FUEL_BUDGET"),
            ChannelSandboxPolicy::default().fuel_budget,
            1_000, // THIS LINE CONTAINS CONSTANT(S)
            100_000_000, // THIS LINE CONTAINS CONSTANT(S)
        )?;

        let policy = ChannelSandboxPolicy {
            max_module_bytes,
            max_request_bytes,
            max_response_bytes,
            fuel_budget,
        };
        let host = WasmChannelHost::try_new()?;

        Ok(Some(Self {
            module_path: path,
            wasm_bytes: Arc::new(wasm_bytes),
            host,
            policy,
        }))
    }

    fn evaluate(
        &self,
        kind: ChannelKind,
        envelope: &ChannelEnvelope,
        trust_tier: SenderTrustTier,
    ) -> KelvinErrorOr<WasmPolicyDecision> {
        let input = json!({
            "channel": kind.as_str(), // THIS LINE CONTAINS CONSTANT(S)
            "delivery_id": envelope.delivery_id, // THIS LINE CONTAINS CONSTANT(S)
            "sender_id": envelope.sender_id, // THIS LINE CONTAINS CONSTANT(S)
            "account_id": envelope.account_id, // THIS LINE CONTAINS CONSTANT(S)
            "text": envelope.text, // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": envelope.timeout_ms, // THIS LINE CONTAINS CONSTANT(S)
            "session_id": envelope.session_id, // THIS LINE CONTAINS CONSTANT(S)
            "workspace_dir": envelope.workspace_dir, // THIS LINE CONTAINS CONSTANT(S)
            "trust_tier": trust_tier.as_str(), // THIS LINE CONTAINS CONSTANT(S)
            "now_ms": now_ms(), // THIS LINE CONTAINS CONSTANT(S)
        })
        .to_string();

        let output = self
            .host
            .run_bytes(&self.wasm_bytes, &input, self.policy.clone())
            .map_err(|err| {
                KelvinError::Backend(format!(
                    "{} channel wasm policy '{}' failed: {}",
                    kind.as_str(),
                    self.module_path,
                    err
                ))
            })?;

        serde_json::from_str::<WasmPolicyDecision>(&output).map_err(|err| {
            KelvinError::InvalidInput(format!(
                "{} channel wasm policy '{}' returned invalid json: {}",
                kind.as_str(),
                self.module_path,
                err
            ))
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueuedEnvelope {
    envelope: ChannelEnvelope,
    route: RouteDecision,
}

#[derive(Debug, Clone)]
struct ChannelStatePersistence {
    state_path: PathBuf,
}

impl ChannelStatePersistence {
    fn for_channel(state_dir: Option<&Path>, kind: ChannelKind) -> KelvinErrorOr<Option<Self>> {
        let Some(root) = state_dir else {
            return Ok(None);
        };
        let dir = root.join("gateway").join("channels"); // THIS LINE CONTAINS CONSTANT(S)
        fs::create_dir_all(&dir)
            .map_err(|err| KelvinError::Io(format!("create channel state dir: {err}")))?;
        Ok(Some(Self {
            state_path: dir.join(format!("{}.json", kind.as_str())),
        }))
    }

    fn path(&self) -> &Path {
        &self.state_path
    }

    fn load(&self) -> KelvinErrorOr<Option<PersistedChannelState>> {
        if !self.state_path.is_file() {
            return Ok(None);
        }
        let bytes = fs::read(&self.state_path).map_err(|err| {
            KelvinError::Io(format!(
                "read persisted channel state '{}': {err}",
                self.state_path.to_string_lossy()
            ))
        })?;
        let state = serde_json::from_slice::<PersistedChannelState>(&bytes).map_err(|err| {
            KelvinError::InvalidInput(format!(
                "invalid persisted channel state '{}': {err}",
                self.state_path.to_string_lossy()
            ))
        })?;
        Ok(Some(state))
    }

    fn save(&self, state: &PersistedChannelState) -> KelvinErrorOr<()> {
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| KelvinError::Io(format!("create channel state parent: {err}")))?;
        }
        let tmp_path = self.state_path.with_extension("json.tmp"); // THIS LINE CONTAINS CONSTANT(S)
        let bytes = serde_json::to_vec_pretty(state).map_err(|err| {
            KelvinError::Backend(format!(
                "serialize channel state '{}': {err}",
                self.state_path.to_string_lossy()
            ))
        })?;
        fs::write(&tmp_path, bytes).map_err(|err| {
            KelvinError::Io(format!(
                "write channel state temp '{}': {err}",
                tmp_path.to_string_lossy()
            ))
        })?;
        fs::rename(&tmp_path, &self.state_path).map_err(|err| {
            KelvinError::Io(format!(
                "commit channel state '{}': {err}",
                self.state_path.to_string_lossy()
            ))
        })?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct PersistedChannelState {
    paired_accounts: HashSet<String>,
    pending_pair_codes: HashMap<String, String>,
    seen_delivery_order: VecDeque<String>,
    rate_windows: HashMap<String, VecDeque<u128>>, // THIS LINE CONTAINS CONSTANT(S)
    cooldown_until_ms: HashMap<String, u128>, // THIS LINE CONTAINS CONSTANT(S)
    inbox: VecDeque<QueuedEnvelope>,
    metrics: ChannelMetrics,
}

#[derive(Debug, Clone)]
struct TextChannelAdapter {
    config: TextChannelConfig,
    paired_accounts: HashSet<String>,
    pending_pair_codes: HashMap<String, String>,
    seen_delivery_ids: HashSet<String>,
    seen_delivery_order: VecDeque<String>,
    rate_windows: HashMap<String, VecDeque<u128>>, // THIS LINE CONTAINS CONSTANT(S)
    cooldown_until_ms: HashMap<String, u128>, // THIS LINE CONTAINS CONSTANT(S)
    inbox: VecDeque<QueuedEnvelope>,
    client: reqwest::Client,
    state_persistence: Option<ChannelStatePersistence>,
    metrics: ChannelMetrics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct ChannelMetrics {
    ingest_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    webhook_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    webhook_accepted_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    webhook_denied_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    webhook_retry_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    verification_ok_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    verification_failed_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    deduped_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    queued_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    queue_rejected_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    pairing_required_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    pairing_approved_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    policy_denied_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    rate_limited_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    completed_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    failed_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    timeout_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    outbound_attempt_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    outbound_retry_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    outbound_failure_total: u64, // THIS LINE CONTAINS CONSTANT(S)
    last_error: Option<String>,
    last_delivery_at_ms: Option<u128>, // THIS LINE CONTAINS CONSTANT(S)
    last_webhook_request_at_ms: Option<u128>, // THIS LINE CONTAINS CONSTANT(S)
    last_webhook_accept_at_ms: Option<u128>, // THIS LINE CONTAINS CONSTANT(S)
    last_webhook_status_code: Option<u16>, // THIS LINE CONTAINS CONSTANT(S)
    last_verification_ok_at_ms: Option<u128>, // THIS LINE CONTAINS CONSTANT(S)
    last_verification_failed_at_ms: Option<u128>, // THIS LINE CONTAINS CONSTANT(S)
    last_verification_error: Option<String>,
}

impl TextChannelAdapter {
    fn telegram_from_env(
        state_dir: Option<&Path>,
        direct_ingress: ChannelDirectIngressStatusConfig,
    ) -> KelvinErrorOr<Option<Self>> {
        Self::new(
            TextChannelConfig::from_env(ChannelKind::Telegram, direct_ingress)?,
            state_dir,
        )
    }

    fn slack_from_env(
        state_dir: Option<&Path>,
        direct_ingress: ChannelDirectIngressStatusConfig,
    ) -> KelvinErrorOr<Option<Self>> {
        Self::new(
            TextChannelConfig::from_env(ChannelKind::Slack, direct_ingress)?,
            state_dir,
        )
    }

    fn discord_from_env(
        state_dir: Option<&Path>,
        direct_ingress: ChannelDirectIngressStatusConfig,
    ) -> KelvinErrorOr<Option<Self>> {
        Self::new(
            TextChannelConfig::from_env(ChannelKind::Discord, direct_ingress)?,
            state_dir,
        )
    }

    fn whatsapp_from_env(
        state_dir: Option<&Path>,
        direct_ingress: ChannelDirectIngressStatusConfig,
    ) -> KelvinErrorOr<Option<Self>> {
        Self::new(
            TextChannelConfig::from_env(ChannelKind::WhatsApp, direct_ingress)?,
            state_dir,
        )
    }

    fn new(config: TextChannelConfig, state_dir: Option<&Path>) -> KelvinErrorOr<Option<Self>> {
        if !config.enabled {
            return Ok(None);
        }
        let state_persistence = ChannelStatePersistence::for_channel(state_dir, config.kind)?;
        let mut adapter = Self {
            config,
            paired_accounts: HashSet::new(),
            pending_pair_codes: HashMap::new(),
            seen_delivery_ids: HashSet::new(),
            seen_delivery_order: VecDeque::new(),
            rate_windows: HashMap::new(),
            cooldown_until_ms: HashMap::new(),
            inbox: VecDeque::new(),
            client: reqwest::Client::new(),
            state_persistence,
            metrics: ChannelMetrics::default(),
        };
        adapter.load_persisted_state()?;
        Ok(Some(adapter))
    }

    fn load_persisted_state(&mut self) -> KelvinErrorOr<()> {
        let Some(persistence) = &self.state_persistence else {
            return Ok(());
        };
        let Some(state) = persistence.load()? else {
            return Ok(());
        };
        self.apply_persisted_state(state);
        Ok(())
    }

    fn apply_persisted_state(&mut self, state: PersistedChannelState) {
        let now = now_ms();
        self.paired_accounts = state.paired_accounts;
        self.pending_pair_codes = state.pending_pair_codes;
        self.seen_delivery_order = state.seen_delivery_order;
        while self.seen_delivery_order.len() > self.config.policy.max_seen_delivery_ids {
            let _ = self.seen_delivery_order.pop_front();
        }
        self.seen_delivery_ids = self.seen_delivery_order.iter().cloned().collect();
        self.rate_windows = state
            .rate_windows
            .into_iter()
            .filter_map(|(sender_id, mut window)| {
                while let Some(ts) = window.front().copied() {
                    if now.saturating_sub(ts) > 60_000 { // THIS LINE CONTAINS CONSTANT(S)
                        let _ = window.pop_front();
                    } else {
                        break;
                    }
                }
                if window.is_empty() {
                    None
                } else {
                    Some((sender_id, window))
                }
            })
            .collect();
        self.cooldown_until_ms = state
            .cooldown_until_ms
            .into_iter()
            .filter(|(_, until_ms)| *until_ms > now)
            .collect();
        self.inbox = state.inbox;
        while self.inbox.len() > self.config.policy.max_queue_depth {
            let _ = self.inbox.pop_back();
        }
        self.metrics = state.metrics;
    }

    fn snapshot_state(&self) -> PersistedChannelState {
        PersistedChannelState {
            paired_accounts: self.paired_accounts.clone(),
            pending_pair_codes: self.pending_pair_codes.clone(),
            seen_delivery_order: self.seen_delivery_order.clone(),
            rate_windows: self.rate_windows.clone(),
            cooldown_until_ms: self.cooldown_until_ms.clone(),
            inbox: self.inbox.clone(),
            metrics: self.metrics.clone(),
        }
    }

    fn persist_state(&self) -> KelvinErrorOr<()> {
        let Some(persistence) = &self.state_persistence else {
            return Ok(());
        };
        persistence.save(&self.snapshot_state())
    }

    fn status(&self) -> Value {
        json!({
            "enabled": true, // THIS LINE CONTAINS CONSTANT(S)
            "kind": self.config.kind.as_str(), // THIS LINE CONTAINS CONSTANT(S)
            "pairing_enabled": self.config.pairing_enabled, // THIS LINE CONTAINS CONSTANT(S)
            "state_persistence_enabled": self.state_persistence.is_some(), // THIS LINE CONTAINS CONSTANT(S)
            "state_path": self.state_persistence.as_ref().map(|state| state.path().to_string_lossy().to_string()), // THIS LINE CONTAINS CONSTANT(S)
            "paired_accounts": self.paired_accounts.len(), // THIS LINE CONTAINS CONSTANT(S)
            "pending_pairings": self.pending_pair_codes.len(), // THIS LINE CONTAINS CONSTANT(S)
            "seen_delivery_ids": self.seen_delivery_ids.len(), // THIS LINE CONTAINS CONSTANT(S)
            "rate_window_sender_count": self.rate_windows.len(), // THIS LINE CONTAINS CONSTANT(S)
            "cooldown_account_count": self.cooldown_until_ms.len(), // THIS LINE CONTAINS CONSTANT(S)
            "allow_account_size": self.config.policy.allow_accounts.len(), // THIS LINE CONTAINS CONSTANT(S)
            "allow_sender_size": self.config.policy.allow_senders.len(), // THIS LINE CONTAINS CONSTANT(S)
            "owner_sender_size": self.config.policy.owner_senders.len(), // THIS LINE CONTAINS CONSTANT(S)
            "trusted_sender_size": self.config.policy.trusted_senders.len(), // THIS LINE CONTAINS CONSTANT(S)
            "probation_sender_size": self.config.policy.probation_senders.len(), // THIS LINE CONTAINS CONSTANT(S)
            "blocked_sender_size": self.config.policy.blocked_senders.len(), // THIS LINE CONTAINS CONSTANT(S)
            "quota_per_minute": { // THIS LINE CONTAINS CONSTANT(S)
                "owner": self.config.policy.quota_owner_per_minute, // THIS LINE CONTAINS CONSTANT(S)
                "trusted": self.config.policy.quota_trusted_per_minute, // THIS LINE CONTAINS CONSTANT(S)
                "standard": self.config.policy.quota_standard_per_minute, // THIS LINE CONTAINS CONSTANT(S)
                "probation": self.config.policy.quota_probation_per_minute, // THIS LINE CONTAINS CONSTANT(S)
            },
            "cooldown_probation_ms": self.config.policy.cooldown_probation_ms, // THIS LINE CONTAINS CONSTANT(S)
            "queue_depth": self.inbox.len(), // THIS LINE CONTAINS CONSTANT(S)
            "queue_max_depth": self.config.policy.max_queue_depth, // THIS LINE CONTAINS CONSTANT(S)
            "ingress_auth_required": self.config.policy.ingress_token.is_some(), // THIS LINE CONTAINS CONSTANT(S)
            "ingress_verification": { // THIS LINE CONTAINS CONSTANT(S)
                "listener_enabled": self.config.direct_ingress.listener_enabled, // THIS LINE CONTAINS CONSTANT(S)
                "webhook_path": self.config.direct_ingress.webhook_path.clone(), // THIS LINE CONTAINS CONSTANT(S)
                "method": self.config.direct_ingress.verification_method.clone(), // THIS LINE CONTAINS CONSTANT(S)
                "configured": self.config.direct_ingress.verification_configured, // THIS LINE CONTAINS CONSTANT(S)
                "last_verified_at_ms": self.metrics.last_verification_ok_at_ms, // THIS LINE CONTAINS CONSTANT(S)
                "last_failed_at_ms": self.metrics.last_verification_failed_at_ms, // THIS LINE CONTAINS CONSTANT(S)
                "last_error": self.metrics.last_verification_error.clone(), // THIS LINE CONTAINS CONSTANT(S)
            },
            "ingress_connectivity": { // THIS LINE CONTAINS CONSTANT(S)
                "last_request_at_ms": self.metrics.last_webhook_request_at_ms, // THIS LINE CONTAINS CONSTANT(S)
                "last_accepted_at_ms": self.metrics.last_webhook_accept_at_ms, // THIS LINE CONTAINS CONSTANT(S)
                "last_status_code": self.metrics.last_webhook_status_code, // THIS LINE CONTAINS CONSTANT(S)
            },
            "outbound_delivery_enabled": self.config.transport.bot_token.is_some(), // THIS LINE CONTAINS CONSTANT(S)
            "outbound_retry_policy": { // THIS LINE CONTAINS CONSTANT(S)
                "max_retries": self.config.transport.outbound_max_retries, // THIS LINE CONTAINS CONSTANT(S)
                "backoff_ms": self.config.transport.outbound_retry_backoff_ms, // THIS LINE CONTAINS CONSTANT(S)
            },
            "wasm_policy_enabled": self.config.wasm_policy_plugin.is_some(), // THIS LINE CONTAINS CONSTANT(S)
            "metrics": { // THIS LINE CONTAINS CONSTANT(S)
                "ingest_total": self.metrics.ingest_total, // THIS LINE CONTAINS CONSTANT(S)
                "webhook_total": self.metrics.webhook_total, // THIS LINE CONTAINS CONSTANT(S)
                "webhook_accepted_total": self.metrics.webhook_accepted_total, // THIS LINE CONTAINS CONSTANT(S)
                "webhook_denied_total": self.metrics.webhook_denied_total, // THIS LINE CONTAINS CONSTANT(S)
                "webhook_retry_total": self.metrics.webhook_retry_total, // THIS LINE CONTAINS CONSTANT(S)
                "verification_ok_total": self.metrics.verification_ok_total, // THIS LINE CONTAINS CONSTANT(S)
                "verification_failed_total": self.metrics.verification_failed_total, // THIS LINE CONTAINS CONSTANT(S)
                "deduped_total": self.metrics.deduped_total, // THIS LINE CONTAINS CONSTANT(S)
                "queued_total": self.metrics.queued_total, // THIS LINE CONTAINS CONSTANT(S)
                "queue_rejected_total": self.metrics.queue_rejected_total, // THIS LINE CONTAINS CONSTANT(S)
                "pairing_required_total": self.metrics.pairing_required_total, // THIS LINE CONTAINS CONSTANT(S)
                "pairing_approved_total": self.metrics.pairing_approved_total, // THIS LINE CONTAINS CONSTANT(S)
                "policy_denied_total": self.metrics.policy_denied_total, // THIS LINE CONTAINS CONSTANT(S)
                "rate_limited_total": self.metrics.rate_limited_total, // THIS LINE CONTAINS CONSTANT(S)
                "completed_total": self.metrics.completed_total, // THIS LINE CONTAINS CONSTANT(S)
                "failed_total": self.metrics.failed_total, // THIS LINE CONTAINS CONSTANT(S)
                "timeout_total": self.metrics.timeout_total, // THIS LINE CONTAINS CONSTANT(S)
                "outbound_attempt_total": self.metrics.outbound_attempt_total, // THIS LINE CONTAINS CONSTANT(S)
                "outbound_retry_total": self.metrics.outbound_retry_total, // THIS LINE CONTAINS CONSTANT(S)
                "outbound_failure_total": self.metrics.outbound_failure_total, // THIS LINE CONTAINS CONSTANT(S)
                "last_error": self.metrics.last_error.clone(), // THIS LINE CONTAINS CONSTANT(S)
                "last_delivery_at_ms": self.metrics.last_delivery_at_ms, // THIS LINE CONTAINS CONSTANT(S)
            }
        })
    }

    fn record_webhook_verified(&mut self, status_code: u16, retry_hint: bool) -> KelvinErrorOr<()> { // THIS LINE CONTAINS CONSTANT(S)
        let now = now_ms();
        self.metrics.webhook_total = self.metrics.webhook_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        self.metrics.webhook_accepted_total = self.metrics.webhook_accepted_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        self.metrics.verification_ok_total = self.metrics.verification_ok_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        if retry_hint {
            self.metrics.webhook_retry_total = self.metrics.webhook_retry_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        }
        self.metrics.last_webhook_request_at_ms = Some(now);
        self.metrics.last_webhook_accept_at_ms = Some(now);
        self.metrics.last_webhook_status_code = Some(status_code);
        self.metrics.last_verification_ok_at_ms = Some(now);
        self.metrics.last_verification_error = None;
        self.persist_state()
    }

    fn record_webhook_denied(
        &mut self,
        status_code: u16, // THIS LINE CONTAINS CONSTANT(S)
        retry_hint: bool,
        reason: &str,
    ) -> KelvinErrorOr<()> {
        let now = now_ms();
        self.metrics.webhook_total = self.metrics.webhook_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        self.metrics.webhook_denied_total = self.metrics.webhook_denied_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        self.metrics.verification_failed_total =
            self.metrics.verification_failed_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        if retry_hint {
            self.metrics.webhook_retry_total = self.metrics.webhook_retry_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        }
        self.metrics.last_webhook_request_at_ms = Some(now);
        self.metrics.last_webhook_status_code = Some(status_code);
        self.metrics.last_verification_failed_at_ms = Some(now);
        self.metrics.last_verification_error = Some(reason.to_string());
        self.persist_state()
    }

    fn approve_pairing(&mut self, code: &str) -> KelvinErrorOr<Value> {
        if !self.config.pairing_enabled {
            return Err(KelvinError::InvalidInput(
                "pairing is not enabled for this channel".to_string(),
            ));
        }
        let normalized = code.trim();
        if normalized.is_empty() {
            return Err(KelvinError::InvalidInput(
                "pairing code must not be empty".to_string(),
            ));
        }
        let Some(account_id) = self.pending_pair_codes.remove(normalized) else {
            return Err(KelvinError::NotFound("pairing code not found".to_string()));
        };
        self.paired_accounts.insert(account_id.clone());
        self.metrics.pairing_approved_total = self.metrics.pairing_approved_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        if let Err(err) = self.persist_state() {
            self.paired_accounts.remove(&account_id);
            self.pending_pair_codes
                .insert(normalized.to_string(), account_id.clone());
            self.metrics.pairing_approved_total =
                self.metrics.pairing_approved_total.saturating_sub(1); // THIS LINE CONTAINS CONSTANT(S)
            return Err(err);
        }
        Ok(json!({
            "approved": true, // THIS LINE CONTAINS CONSTANT(S)
            "account_id": account_id, // THIS LINE CONTAINS CONSTANT(S)
        }))
    }

    async fn ingest(
        &mut self,
        runtime: &KelvinSdkRuntime,
        routing: &ChannelRoutingTable,
        mut envelope: ChannelEnvelope,
    ) -> KelvinErrorOr<Value> {
        self.metrics.ingest_total = self.metrics.ingest_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)

        envelope.delivery_id = normalize_identifier("delivery_id", &envelope.delivery_id, 256)?; // THIS LINE CONTAINS CONSTANT(S)
        envelope.sender_id = normalize_identifier("sender_id", &envelope.sender_id, 256)?; // THIS LINE CONTAINS CONSTANT(S)
        envelope.account_id = normalize_identifier("account_id", &envelope.account_id, 256)?; // THIS LINE CONTAINS CONSTANT(S)
        envelope.text = envelope.text.trim().to_string();
        if envelope.text.is_empty() {
            return Err(KelvinError::InvalidInput(
                "message text must not be empty".to_string(),
            ));
        }
        if envelope.text.len() > self.config.policy.max_text_bytes {
            return Err(KelvinError::InvalidInput(format!(
                "message text exceeds {} bytes",
                self.config.policy.max_text_bytes
            )));
        }

        self.verify_ingress_auth(&envelope)?;
        envelope.auth_token = None;

        if self.is_duplicate_delivery(&envelope.delivery_id) {
            self.metrics.deduped_total = self.metrics.deduped_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
            return Ok(json!({
                "status": "deduped", // THIS LINE CONTAINS CONSTANT(S)
                "delivery_id": envelope.delivery_id, // THIS LINE CONTAINS CONSTANT(S)
            }));
        }

        if let Some(code) = self.enforce_pairing_policy(&envelope.account_id)? {
            self.metrics.pairing_required_total =
                self.metrics.pairing_required_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
            let pairing_message = format!(
                "KelvinClaw pairing required. Approve code: {code} using channel.telegram.pair.approve."
            );
            let _ = self
                .send_message_with_retry(&envelope.account_id, &pairing_message)
                .await;
            return Ok(json!({
                "status": "pairing_required", // THIS LINE CONTAINS CONSTANT(S)
                "account_id": envelope.account_id, // THIS LINE CONTAINS CONSTANT(S)
                "pairing_code": code, // THIS LINE CONTAINS CONSTANT(S)
            }));
        }

        let mut trust_tier = self.enforce_policy(&envelope)?;

        let wasm_decision = if let Some(plugin) = &self.config.wasm_policy_plugin {
            let decision = plugin.evaluate(self.config.kind, &envelope, trust_tier)?;
            if !decision.allow {
                self.metrics.policy_denied_total =
                    self.metrics.policy_denied_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
                let reason = decision
                    .reason
                    .unwrap_or_else(|| "blocked by wasm policy".to_string());
                return Err(KelvinError::NotFound(format!(
                    "{} channel message denied: {}",
                    self.config.kind.as_str(),
                    reason
                )));
            }
            if let Some(value) = decision.trust_tier {
                trust_tier = value;
            }
            if let Some(text) = decision.override_text.clone() {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    return Err(KelvinError::InvalidInput(
                        "wasm channel policy override_text must not be empty".to_string(),
                    ));
                }
                if trimmed.len() > self.config.policy.max_text_bytes {
                    return Err(KelvinError::InvalidInput(
                        "wasm channel policy override_text exceeds max text bytes".to_string(),
                    ));
                }
                envelope.text = trimmed.to_string();
            }
            Some(decision)
        } else {
            None
        };

        let mut route = routing.decide(RouteInput {
            channel: self.config.kind.as_str(),
            account_id: &envelope.account_id,
            requested_session_id: envelope.session_id.as_deref(),
            requested_workspace_dir: envelope.workspace_dir.as_deref(),
            sender_tier: trust_tier,
        });
        if let Some(decision) = &wasm_decision {
            route.apply_wasm_overrides(decision);
        }

        self.track_delivery_id(envelope.delivery_id.clone());
        if self.inbox.len() >= self.config.policy.max_queue_depth {
            self.metrics.queue_rejected_total = self.metrics.queue_rejected_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
            self.metrics.last_error = Some("channel queue is full".to_string());
            self.persist_state()?;
            return Err(KelvinError::Backend(format!(
                "{} channel queue is full",
                self.config.kind.as_str()
            )));
        }

        self.metrics.queued_total = self.metrics.queued_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        let current_delivery_id = envelope.delivery_id.clone();
        self.inbox.push_back(QueuedEnvelope { envelope, route });
        self.persist_state()?;
        self.process_inbox(runtime, &current_delivery_id).await
    }

    fn verify_ingress_auth(&self, envelope: &ChannelEnvelope) -> KelvinErrorOr<()> {
        let Some(required) = &self.config.policy.ingress_token else {
            return Ok(());
        };
        let Some(provided) = envelope.auth_token.as_deref() else {
            return Err(KelvinError::NotFound(format!(
                "{} ingress auth token missing",
                self.config.kind.as_str()
            )));
        };
        if provided != required {
            return Err(KelvinError::NotFound(format!(
                "{} ingress auth token mismatch",
                self.config.kind.as_str()
            )));
        }
        Ok(())
    }

    fn is_duplicate_delivery(&self, delivery_id: &str) -> bool {
        self.seen_delivery_ids.contains(delivery_id)
    }

    fn track_delivery_id(&mut self, delivery_id: String) {
        if self.seen_delivery_ids.insert(delivery_id.clone()) {
            self.seen_delivery_order.push_back(delivery_id);
        }
        self.metrics.last_delivery_at_ms = Some(now_ms());
        while self.seen_delivery_order.len() > self.config.policy.max_seen_delivery_ids {
            if let Some(evicted) = self.seen_delivery_order.pop_front() {
                self.seen_delivery_ids.remove(&evicted);
            }
        }
    }

    fn enforce_policy(&mut self, envelope: &ChannelEnvelope) -> KelvinErrorOr<SenderTrustTier> {
        let sender_tier = SenderTrustTier::from_policy(&self.config.policy, &envelope.sender_id);

        if sender_tier == SenderTrustTier::Blocked {
            self.metrics.policy_denied_total = self.metrics.policy_denied_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
            return Err(KelvinError::NotFound(format!(
                "{} sender '{}' is blocked",
                self.config.kind.as_str(),
                envelope.sender_id
            )));
        }

        if !self.config.policy.allow_accounts.is_empty()
            && !self
                .config
                .policy
                .allow_accounts
                .contains(&envelope.account_id)
            && sender_tier != SenderTrustTier::Owner
        {
            self.metrics.policy_denied_total = self.metrics.policy_denied_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
            return Err(KelvinError::NotFound(format!(
                "{} account '{}' is not allowlisted",
                self.config.kind.as_str(),
                envelope.account_id
            )));
        }

        if !self.config.policy.allow_senders.is_empty()
            && !self
                .config
                .policy
                .allow_senders
                .contains(&envelope.sender_id)
            && sender_tier == SenderTrustTier::Standard
        {
            self.metrics.policy_denied_total = self.metrics.policy_denied_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
            return Err(KelvinError::NotFound(format!(
                "{} sender '{}' is not allowlisted",
                self.config.kind.as_str(),
                envelope.sender_id
            )));
        }

        self.enforce_rate_limit(&envelope.sender_id, sender_tier)?;

        if sender_tier == SenderTrustTier::Probation && self.config.policy.cooldown_probation_ms > 0 // THIS LINE CONTAINS CONSTANT(S)
        {
            let now = now_ms();
            if let Some(cooldown_until_ms) =
                self.cooldown_until_ms.get(&envelope.sender_id).copied()
            {
                if now < cooldown_until_ms {
                    return Err(KelvinError::Timeout(format!(
                        "{} sender '{}' is in cooldown",
                        self.config.kind.as_str(),
                        envelope.sender_id
                    )));
                }
            }
            self.cooldown_until_ms.insert(
                envelope.sender_id.clone(),
                now.saturating_add(u128::from(self.config.policy.cooldown_probation_ms)), // THIS LINE CONTAINS CONSTANT(S)
            );
        }

        self.persist_state()?;
        Ok(sender_tier)
    }

    fn enforce_rate_limit(
        &mut self,
        sender_id: &str,
        sender_tier: SenderTrustTier,
    ) -> KelvinErrorOr<()> {
        let now = now_ms();
        let window = self.rate_windows.entry(sender_id.to_string()).or_default();
        while let Some(ts) = window.front().copied() {
            if now.saturating_sub(ts) > 60_000 { // THIS LINE CONTAINS CONSTANT(S)
                let _ = window.pop_front();
            } else {
                break;
            }
        }

        let quota = match sender_tier {
            SenderTrustTier::Owner => self.config.policy.quota_owner_per_minute,
            SenderTrustTier::Trusted => self.config.policy.quota_trusted_per_minute,
            SenderTrustTier::Standard => self.config.policy.quota_standard_per_minute,
            SenderTrustTier::Probation => self.config.policy.quota_probation_per_minute,
            SenderTrustTier::Blocked => 0, // THIS LINE CONTAINS CONSTANT(S)
        };

        if window.len() >= quota {
            self.metrics.rate_limited_total = self.metrics.rate_limited_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
            return Err(KelvinError::Timeout(format!(
                "{} sender '{}' exceeded per-minute quota",
                self.config.kind.as_str(),
                sender_id
            )));
        }

        window.push_back(now);
        Ok(())
    }

    fn enforce_pairing_policy(&mut self, account_id: &str) -> KelvinErrorOr<Option<String>> {
        if !self.config.pairing_enabled {
            return Ok(None);
        }

        if self.config.policy.allow_accounts.contains(account_id)
            || self.paired_accounts.contains(account_id)
        {
            return Ok(None);
        }

        if let Some(existing) =
            self.pending_pair_codes
                .iter()
                .find_map(|(code, pending_account)| {
                    if pending_account == account_id {
                        Some(code.clone())
                    } else {
                        None
                    }
                })
        {
            return Ok(Some(existing));
        }

        let hash = now_ms() ^ simple_hash(account_id);
        let numeric = hash % 900_000 + 100_000; // THIS LINE CONTAINS CONSTANT(S)
        let code = format!("{numeric:06}"); // THIS LINE CONTAINS CONSTANT(S)
        self.pending_pair_codes
            .insert(code.clone(), account_id.to_string());
        if let Err(err) = self.persist_state() {
            self.pending_pair_codes.remove(&code);
            return Err(err);
        }
        Ok(Some(code))
    }

    async fn process_inbox(
        &mut self,
        runtime: &KelvinSdkRuntime,
        target_delivery_id: &str,
    ) -> KelvinErrorOr<Value> {
        let mut target_response: Option<Value> = None;
        while let Some(entry) = self.inbox.pop_front() {
            self.persist_state()?;
            let is_target = entry.envelope.delivery_id == target_delivery_id;
            let result = self.execute_entry(runtime, &entry).await;
            if is_target {
                target_response = Some(result?);
            } else if let Err(err) = result {
                self.metrics.last_error = Some(err.to_string());
            }
        }

        if let Some(payload) = target_response {
            return Ok(payload);
        }

        Ok(json!({
            "status": "queued", // THIS LINE CONTAINS CONSTANT(S)
            "delivery_id": target_delivery_id, // THIS LINE CONTAINS CONSTANT(S)
            "queue_depth": self.inbox.len(), // THIS LINE CONTAINS CONSTANT(S)
        }))
    }

    async fn execute_entry(
        &mut self,
        runtime: &KelvinSdkRuntime,
        entry: &QueuedEnvelope,
    ) -> KelvinErrorOr<Value> {
        let sender_context = format!(
            "[Channel: {} | Sender: {} | Tier: {}]",
            self.config.kind.as_str(),
            entry.envelope.sender_id,
            entry.route.sender_tier,
        );
        let system_prompt = match &entry.route.system_prompt {
            Some(existing) => Some(format!("{sender_context}\n{existing}")),
            None => Some(sender_context),
        };

        let accepted = runtime
            .submit(KelvinSdkRunRequest {
                prompt: entry.envelope.text.clone(),
                session_id: Some(entry.route.session_id.clone()),
                workspace_dir: entry
                    .route
                    .workspace_dir
                    .as_ref()
                    .map(std::path::PathBuf::from),
                timeout_ms: entry.envelope.timeout_ms,
                system_prompt,
                memory_query: None,
                run_id: None,
            })
            .await?;

        let timeout_ms = entry
            .envelope
            .timeout_ms
            .unwrap_or(30_000) // THIS LINE CONTAINS CONSTANT(S)
            .saturating_add(3_000); // THIS LINE CONTAINS CONSTANT(S)

        let outcome = runtime
            .wait_for_outcome(&accepted.run_id, timeout_ms)
            .await?;
        match outcome {
            RunOutcome::Completed(result) => {
                let response_text = result
                    .payloads
                    .iter()
                    .map(|payload| payload.text.clone())
                    .collect::<Vec<_>>()
                    .join("\n");
                let outbound_text = if response_text.trim().is_empty() {
                    "No response generated.".to_string()
                } else {
                    response_text
                };
                self.send_message_with_retry(&entry.envelope.account_id, &outbound_text)
                    .await?;
                self.metrics.completed_total = self.metrics.completed_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
                self.metrics.last_error = None;
                self.persist_state()?;

                Ok(json!({
                    "status": "completed", // THIS LINE CONTAINS CONSTANT(S)
                    "run_id": accepted.run_id, // THIS LINE CONTAINS CONSTANT(S)
                    "delivery_id": entry.envelope.delivery_id, // THIS LINE CONTAINS CONSTANT(S)
                    "response_text": outbound_text, // THIS LINE CONTAINS CONSTANT(S)
                    "route": entry.route, // THIS LINE CONTAINS CONSTANT(S)
                }))
            }
            RunOutcome::Failed(error) => {
                self.metrics.failed_total = self.metrics.failed_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
                self.metrics.last_error = Some(error.clone());
                let outbound_text = format!("Kelvin run failed: {error}");
                let _ = self
                    .send_message_with_retry(&entry.envelope.account_id, &outbound_text)
                    .await;
                self.persist_state()?;
                Ok(json!({
                    "status": "failed", // THIS LINE CONTAINS CONSTANT(S)
                    "run_id": accepted.run_id, // THIS LINE CONTAINS CONSTANT(S)
                    "delivery_id": entry.envelope.delivery_id, // THIS LINE CONTAINS CONSTANT(S)
                    "error": error, // THIS LINE CONTAINS CONSTANT(S)
                    "route": entry.route, // THIS LINE CONTAINS CONSTANT(S)
                }))
            }
            RunOutcome::Timeout => {
                self.metrics.timeout_total = self.metrics.timeout_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
                self.metrics.last_error = Some("run timed out".to_string());
                let _ = self
                    .send_message_with_retry(&entry.envelope.account_id, "Kelvin run timed out.")
                    .await;
                self.persist_state()?;
                Ok(json!({
                    "status": "timeout", // THIS LINE CONTAINS CONSTANT(S)
                    "run_id": accepted.run_id, // THIS LINE CONTAINS CONSTANT(S)
                    "delivery_id": entry.envelope.delivery_id, // THIS LINE CONTAINS CONSTANT(S)
                    "route": entry.route, // THIS LINE CONTAINS CONSTANT(S)
                }))
            }
        }
    }

    async fn send_message_with_retry(&mut self, account_id: &str, text: &str) -> KelvinErrorOr<()> {
        let Some(bot_token) = &self.config.transport.bot_token else {
            return Ok(());
        };

        let mut last_error = None;
        for attempt in 0..=self.config.transport.outbound_max_retries { // THIS LINE CONTAINS CONSTANT(S)
            self.metrics.outbound_attempt_total =
                self.metrics.outbound_attempt_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
            let call = self.send_outbound_once(bot_token, account_id, text).await;
            match call {
                Ok(()) => return Ok(()),
                Err(err) => {
                    last_error = Some(err.to_string());
                }
            }
            if attempt < self.config.transport.outbound_max_retries {
                self.metrics.outbound_retry_total =
                    self.metrics.outbound_retry_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
                sleep(Duration::from_millis(
                    self.config.transport.outbound_retry_backoff_ms.max(1), // THIS LINE CONTAINS CONSTANT(S)
                ))
                .await;
            }
        }

        self.metrics.outbound_failure_total = self.metrics.outbound_failure_total.saturating_add(1); // THIS LINE CONTAINS CONSTANT(S)
        self.metrics.last_error = Some(
            last_error
                .clone()
                .unwrap_or_else(|| "unknown outbound error".to_string()),
        );

        Err(KelvinError::Backend(format!(
            "{} outbound delivery failed after retries: {}",
            self.config.kind.as_str(),
            last_error.unwrap_or_else(|| "unknown error".to_string())
        )))
    }

    async fn deliver_outbound_message(
        &mut self,
        account_id: &str,
        text: &str,
    ) -> KelvinErrorOr<()> {
        self.send_message_with_retry(account_id, text).await?;
        self.persist_state()?;
        Ok(())
    }

    async fn send_outbound_once(
        &self,
        bot_token: &str,
        account_id: &str,
        text: &str,
    ) -> KelvinErrorOr<()> {
        let base = self.config.transport.api_base_url.trim_end_matches('/');
        let (endpoint, request) = match self.config.kind {
            ChannelKind::Telegram => {
                let endpoint = format!("{}/bot{}/sendMessage", base, bot_token);
                let request = self.client.post(endpoint.clone()).json(&json!({
                    "chat_id": account_id, // THIS LINE CONTAINS CONSTANT(S)
                    "text": text, // THIS LINE CONTAINS CONSTANT(S)
                }));
                (endpoint, request)
            }
            ChannelKind::Slack => {
                let endpoint = format!("{}/chat.postMessage", base);
                let request = self
                    .client
                    .post(endpoint.clone())
                    .bearer_auth(bot_token)
                    .json(&json!({
                        "channel": account_id, // THIS LINE CONTAINS CONSTANT(S)
                        "text": text, // THIS LINE CONTAINS CONSTANT(S)
                    }));
                (endpoint, request)
            }
            ChannelKind::Discord => {
                let endpoint = format!("{}/channels/{}/messages", base, account_id);
                let request = self
                    .client
                    .post(endpoint.clone())
                    .header("Authorization", format!("Bot {}", bot_token)) // THIS LINE CONTAINS CONSTANT(S)
                    .json(&json!({
                        "content": text, // THIS LINE CONTAINS CONSTANT(S)
                    }));
                (endpoint, request)
            }
            ChannelKind::WhatsApp => {
                // bot_token is the WhatsApp Cloud API access token.
                // account_id is the recipient phone number (user_phone).
                // We need the phone_number_id from env to construct the endpoint.
                let phone_number_id =
                    std::env::var("KELVIN_WHATSAPP_PHONE_NUMBER_ID").unwrap_or_default(); // THIS LINE CONTAINS CONSTANT(S)
                let endpoint = format!("{}/{}/messages", base, phone_number_id);
                let request = self
                    .client
                    .post(endpoint.clone())
                    .bearer_auth(bot_token)
                    .json(&json!({
                        "messaging_product": "whatsapp", // THIS LINE CONTAINS CONSTANT(S)
                        "to": account_id, // THIS LINE CONTAINS CONSTANT(S)
                        "type": "text", // THIS LINE CONTAINS CONSTANT(S)
                        "text": { "body": text }, // THIS LINE CONTAINS CONSTANT(S)
                    }));
                (endpoint, request)
            }
        };

        let response = request.send().await.map_err(|err| {
            KelvinError::Backend(format!(
                "{} outbound request failed: {}",
                self.config.kind.as_str(),
                err
            ))
        })?;

        if !response.status().is_success() {
            return Err(KelvinError::Backend(format!(
                "{} outbound endpoint '{}' returned status {}",
                self.config.kind.as_str(),
                endpoint,
                response.status()
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")] // THIS LINE CONTAINS CONSTANT(S)
enum SenderTrustTier { // THIS LINE CONTAINS CONSTANT(S)
    Owner,
    Trusted,
    Standard,
    Probation,
    Blocked,
}

impl SenderTrustTier {
    fn as_str(self) -> &'static str { // THIS LINE CONTAINS CONSTANT(S)
        match self {
            Self::Owner => "owner", // THIS LINE CONTAINS CONSTANT(S)
            Self::Trusted => "trusted", // THIS LINE CONTAINS CONSTANT(S)
            Self::Standard => "standard", // THIS LINE CONTAINS CONSTANT(S)
            Self::Probation => "probation", // THIS LINE CONTAINS CONSTANT(S)
            Self::Blocked => "blocked", // THIS LINE CONTAINS CONSTANT(S)
        }
    }

    fn parse(input: &str) -> Option<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "owner" => Some(Self::Owner), // THIS LINE CONTAINS CONSTANT(S)
            "trusted" => Some(Self::Trusted), // THIS LINE CONTAINS CONSTANT(S)
            "standard" => Some(Self::Standard), // THIS LINE CONTAINS CONSTANT(S)
            "probation" => Some(Self::Probation), // THIS LINE CONTAINS CONSTANT(S)
            "blocked" => Some(Self::Blocked), // THIS LINE CONTAINS CONSTANT(S)
            _ => None,
        }
    }

    fn from_policy(policy: &ChannelPolicy, sender_id: &str) -> Self {
        if policy.blocked_senders.contains(sender_id) {
            Self::Blocked
        } else if policy.owner_senders.contains(sender_id) {
            Self::Owner
        } else if policy.trusted_senders.contains(sender_id) {
            Self::Trusted
        } else if policy.probation_senders.contains(sender_id) {
            Self::Probation
        } else {
            Self::Standard
        }
    }
}

#[derive(Debug, Clone)]
struct ChannelRoutingTable {
    rules: Vec<RouteRule>,
}

impl ChannelRoutingTable {
    fn from_env() -> KelvinErrorOr<Self> {
        let Some(raw) = std::env::var("KELVIN_CHANNEL_ROUTING_RULES_JSON") // THIS LINE CONTAINS CONSTANT(S)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            return Ok(Self { rules: Vec::new() });
        };

        let mut parsed: Vec<RouteRule> = serde_json::from_str(&raw).map_err(|err| {
            KelvinError::InvalidInput(format!(
                "invalid KELVIN_CHANNEL_ROUTING_RULES_JSON: {}",
                err
            ))
        })?;
        let mut seen_ids = HashSet::new();
        for rule in &parsed {
            if rule.id.trim().is_empty() {
                return Err(KelvinError::InvalidInput(
                    "channel routing rule id must not be empty".to_string(),
                ));
            }
            if !seen_ids.insert(rule.id.clone()) {
                return Err(KelvinError::InvalidInput(format!(
                    "duplicate channel routing rule id '{}'",
                    rule.id
                )));
            }
        }

        parsed.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        });

        Ok(Self { rules: parsed })
    }

    fn decide(&self, input: RouteInput<'_>) -> RouteDecision {
        for rule in &self.rules {
            if !rule.matches(&input) {
                continue;
            }
            return RouteDecision {
                matched_rule_id: Some(rule.id.clone()),
                session_id: rule
                    .route_session_id
                    .clone()
                    .unwrap_or_else(|| default_session_id(input.channel, input.account_id)),
                workspace_dir: rule
                    .route_workspace_dir
                    .clone()
                    .or_else(|| input.requested_workspace_dir.map(ToString::to_string)),
                system_prompt: rule.route_system_prompt.clone(),
                sender_tier: input.sender_tier.as_str().to_string(),
            };
        }

        RouteDecision {
            matched_rule_id: None,
            session_id: input
                .requested_session_id
                .map(ToString::to_string)
                .unwrap_or_else(|| default_session_id(input.channel, input.account_id)),
            workspace_dir: input.requested_workspace_dir.map(ToString::to_string),
            system_prompt: None,
            sender_tier: input.sender_tier.as_str().to_string(),
        }
    }

    fn rule_count(&self) -> usize {
        self.rules.len()
    }

    fn status(&self) -> Value {
        json!({
            "rules_loaded": self.rules.len(), // THIS LINE CONTAINS CONSTANT(S)
            "rules": self // THIS LINE CONTAINS CONSTANT(S)
                .rules
                .iter()
                .map(|rule| {
                    json!({
                        "id": rule.id, // THIS LINE CONTAINS CONSTANT(S)
                        "priority": rule.priority, // THIS LINE CONTAINS CONSTANT(S)
                        "channel": rule.channel, // THIS LINE CONTAINS CONSTANT(S)
                        "account_id": rule.account_id, // THIS LINE CONTAINS CONSTANT(S)
                        "sender_tier": rule.sender_tier, // THIS LINE CONTAINS CONSTANT(S)
                        "session_id": rule.session_id, // THIS LINE CONTAINS CONSTANT(S)
                        "workspace_dir": rule.workspace_dir, // THIS LINE CONTAINS CONSTANT(S)
                    })
                })
                .collect::<Vec<_>>(),
        })
    }
}

fn default_session_id(channel: &str, account_id: &str) -> String {
    format!("{}:{}", channel.trim(), account_id.trim())
}

#[derive(Debug, Clone, Deserialize)]
struct RouteRule {
    id: String,
    #[serde(default)]
    priority: i64, // THIS LINE CONTAINS CONSTANT(S)
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    account_id: Option<String>,
    #[serde(default)]
    sender_tier: Option<SenderTrustTier>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    workspace_dir: Option<String>,
    #[serde(default)]
    route_session_id: Option<String>,
    #[serde(default)]
    route_workspace_dir: Option<String>,
    #[serde(default)]
    route_system_prompt: Option<String>,
}

impl RouteRule {
    fn matches(&self, input: &RouteInput<'_>) -> bool {
        if let Some(channel) = self.channel.as_deref() {
            let normalized = channel.trim().to_ascii_lowercase();
            if normalized != "*" && normalized != input.channel.to_ascii_lowercase() {
                return false;
            }
        }
        if let Some(account_id) = self.account_id.as_deref() {
            if account_id.trim() != input.account_id {
                return false;
            }
        }
        if let Some(sender_tier) = self.sender_tier {
            if sender_tier != input.sender_tier {
                return false;
            }
        }
        if let Some(session_id) = self.session_id.as_deref() {
            if input.requested_session_id != Some(session_id.trim()) {
                return false;
            }
        }
        if let Some(workspace_dir) = self.workspace_dir.as_deref() {
            if input.requested_workspace_dir != Some(workspace_dir.trim()) {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RouteDecision {
    matched_rule_id: Option<String>,
    session_id: String,
    workspace_dir: Option<String>,
    system_prompt: Option<String>,
    sender_tier: String,
}

impl RouteDecision {
    fn apply_wasm_overrides(&mut self, decision: &WasmPolicyDecision) {
        if let Some(session_id) = &decision.route_session_id {
            let trimmed = session_id.trim();
            if !trimmed.is_empty() {
                self.session_id = trimmed.to_string();
            }
        }
        if let Some(workspace_dir) = &decision.route_workspace_dir {
            let trimmed = workspace_dir.trim();
            if !trimmed.is_empty() {
                self.workspace_dir = Some(trimmed.to_string());
            }
        }
        if let Some(system_prompt) = &decision.route_system_prompt {
            let trimmed = system_prompt.trim();
            if !trimmed.is_empty() {
                self.system_prompt = Some(trimmed.to_string());
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RouteInput<'a> {
    channel: &'a str,
    account_id: &'a str,
    requested_session_id: Option<&'a str>,
    requested_workspace_dir: Option<&'a str>,
    sender_tier: SenderTrustTier,
}

fn normalize_identifier(label: &str, value: &str, max_len: usize) -> KelvinErrorOr<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(KelvinError::InvalidInput(format!(
            "{} must not be empty",
            label
        )));
    }
    if normalized.len() > max_len {
        return Err(KelvinError::InvalidInput(format!(
            "{} exceeds {} bytes",
            label, max_len
        )));
    }
    if normalized.chars().any(|ch| ch.is_control()) {
        return Err(KelvinError::InvalidInput(format!(
            "{} must not include control characters",
            label
        )));
    }
    Ok(normalized.to_string())
}

fn read_env_csv_set(name: &str) -> HashSet<String> {
    let Some(value) = std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return HashSet::new();
    };

    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn read_env_bool(name: &str, default: bool) -> KelvinErrorOr<bool> {
    let Ok(value) = std::env::var(name) else {
        return Ok(default);
    };
    match value.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true), // THIS LINE CONTAINS CONSTANT(S)
        "0" | "false" | "no" | "off" => Ok(false), // THIS LINE CONTAINS CONSTANT(S)
        _ => Err(KelvinError::InvalidInput(format!(
            "invalid boolean value for {}: {}",
            name, value
        ))),
    }
}

fn read_env_usize(name: &str, default: usize, min: usize, max: usize) -> KelvinErrorOr<usize> {
    let Ok(value) = std::env::var(name) else {
        return Ok(default);
    };
    let parsed = value.trim().parse::<usize>().map_err(|_| {
        KelvinError::InvalidInput(format!("invalid numeric value for {}: {}", name, value))
    })?;
    if parsed < min || parsed > max {
        return Err(KelvinError::InvalidInput(format!(
            "{} must be between {} and {}",
            name, min, max
        )));
    }
    Ok(parsed)
}

fn read_env_u8(name: &str, default: u8, min: u8, max: u8) -> KelvinErrorOr<u8> { // THIS LINE CONTAINS CONSTANT(S)
    let Ok(value) = std::env::var(name) else {
        return Ok(default);
    };
    let parsed = value.trim().parse::<u8>().map_err(|_| { // THIS LINE CONTAINS CONSTANT(S)
        KelvinError::InvalidInput(format!("invalid numeric value for {}: {}", name, value))
    })?;
    if parsed < min || parsed > max {
        return Err(KelvinError::InvalidInput(format!(
            "{} must be between {} and {}",
            name, min, max
        )));
    }
    Ok(parsed)
}

fn read_env_u64(name: &str, default: u64, min: u64, max: u64) -> KelvinErrorOr<u64> { // THIS LINE CONTAINS CONSTANT(S)
    let Ok(value) = std::env::var(name) else {
        return Ok(default);
    };
    let parsed = value.trim().parse::<u64>().map_err(|_| { // THIS LINE CONTAINS CONSTANT(S)
        KelvinError::InvalidInput(format!("invalid numeric value for {}: {}", name, value))
    })?;
    if parsed < min || parsed > max {
        return Err(KelvinError::InvalidInput(format!(
            "{} must be between {} and {}",
            name, min, max
        )));
    }
    Ok(parsed)
}

fn simple_hash(input: &str) -> u128 { // THIS LINE CONTAINS CONSTANT(S)
    let mut hash: u128 = 1469598103934665603; // THIS LINE CONTAINS CONSTANT(S)
    for byte in input.as_bytes() {
        hash ^= u128::from(*byte); // THIS LINE CONTAINS CONSTANT(S)
        hash = hash.wrapping_mul(1099511628211); // THIS LINE CONTAINS CONSTANT(S)
    }
    hash
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramIngressRequest {
    pub delivery_id: String,
    pub chat_id: i64, // THIS LINE CONTAINS CONSTANT(S)
    pub text: String,
    pub timeout_ms: Option<u64>, // THIS LINE CONTAINS CONSTANT(S)
    pub auth_token: Option<String>,
    pub session_id: Option<String>,
    pub workspace_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackIngressRequest {
    pub delivery_id: String,
    pub channel_id: String,
    pub user_id: String,
    pub text: String,
    pub timeout_ms: Option<u64>, // THIS LINE CONTAINS CONSTANT(S)
    pub auth_token: Option<String>,
    pub session_id: Option<String>,
    pub workspace_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordIngressRequest {
    pub delivery_id: String,
    pub channel_id: String,
    pub user_id: String,
    pub text: String,
    pub timeout_ms: Option<u64>, // THIS LINE CONTAINS CONSTANT(S)
    pub auth_token: Option<String>,
    pub session_id: Option<String>,
    pub workspace_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WhatsappIngressRequest {
    pub delivery_id: String,
    /// The WhatsApp Business phone number ID that received the message.
    #[allow(dead_code)]
    pub phone_number_id: String,
    pub user_phone: String,
    pub text: String,
    pub timeout_ms: Option<u64>, // THIS LINE CONTAINS CONSTANT(S)
    pub auth_token: Option<String>,
    pub session_id: Option<String>,
    pub workspace_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelegramPairApproveRequest {
    pub code: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelRouteInspectRequest {
    pub channel: String,
    pub account_id: String,
    pub sender_tier: Option<String>,
    pub session_id: Option<String>,
    pub workspace_dir: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_state_dir(prefix: &str) -> std::path::PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!("kelvin-channel-test-{prefix}-{millis}"));
        std::fs::create_dir_all(&path).expect("create state dir");
        path
    }

    #[test]
    fn routing_is_deterministic_and_priority_ordered() {
        let table = ChannelRoutingTable {
            rules: vec![
                RouteRule {
                    id: "b".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                    priority: 5, // THIS LINE CONTAINS CONSTANT(S)
                    channel: Some("telegram".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                    account_id: Some("42".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                    sender_tier: None,
                    session_id: None,
                    workspace_dir: None,
                    route_session_id: Some("session-b".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                    route_workspace_dir: None,
                    route_system_prompt: None,
                },
                RouteRule {
                    id: "a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                    priority: 5, // THIS LINE CONTAINS CONSTANT(S)
                    channel: Some("telegram".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                    account_id: Some("42".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                    sender_tier: None,
                    session_id: None,
                    workspace_dir: None,
                    route_session_id: Some("session-a".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                    route_workspace_dir: None,
                    route_system_prompt: None,
                },
            ],
        };

        let decision = table.decide(RouteInput {
            channel: "telegram", // THIS LINE CONTAINS CONSTANT(S)
            account_id: "42", // THIS LINE CONTAINS CONSTANT(S)
            requested_session_id: None,
            requested_workspace_dir: None,
            sender_tier: SenderTrustTier::Standard,
        });

        assert_eq!(decision.matched_rule_id.as_deref(), Some("b")); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(decision.session_id, "session-b"); // THIS LINE CONTAINS CONSTANT(S)
    }

    #[test]
    fn sender_tier_from_policy_respects_block_overrides() {
        let policy = ChannelPolicy {
            ingress_token: None,
            allow_accounts: HashSet::new(),
            allow_senders: HashSet::new(),
            owner_senders: HashSet::from(["dave".to_string()]), // THIS LINE CONTAINS CONSTANT(S)
            trusted_senders: HashSet::from(["alice".to_string()]), // THIS LINE CONTAINS CONSTANT(S)
            probation_senders: HashSet::from(["bob".to_string()]), // THIS LINE CONTAINS CONSTANT(S)
            blocked_senders: HashSet::from(["alice".to_string(), "dave".to_string()]), // THIS LINE CONTAINS CONSTANT(S)
            quota_owner_per_minute: 40, // THIS LINE CONTAINS CONSTANT(S)
            quota_standard_per_minute: 10, // THIS LINE CONTAINS CONSTANT(S)
            quota_trusted_per_minute: 20, // THIS LINE CONTAINS CONSTANT(S)
            quota_probation_per_minute: 5, // THIS LINE CONTAINS CONSTANT(S)
            cooldown_probation_ms: 100, // THIS LINE CONTAINS CONSTANT(S)
            max_seen_delivery_ids: 100, // THIS LINE CONTAINS CONSTANT(S)
            max_queue_depth: 100, // THIS LINE CONTAINS CONSTANT(S)
            max_text_bytes: 100, // THIS LINE CONTAINS CONSTANT(S)
        };

        // blocked overrides trusted
        assert_eq!(
            SenderTrustTier::from_policy(&policy, "alice"), // THIS LINE CONTAINS CONSTANT(S)
            SenderTrustTier::Blocked
        );
        // blocked overrides owner
        assert_eq!(
            SenderTrustTier::from_policy(&policy, "dave"), // THIS LINE CONTAINS CONSTANT(S)
            SenderTrustTier::Blocked
        );
        assert_eq!(
            SenderTrustTier::from_policy(&policy, "bob"), // THIS LINE CONTAINS CONSTANT(S)
            SenderTrustTier::Probation
        );
        assert_eq!(
            SenderTrustTier::from_policy(&policy, "carol"), // THIS LINE CONTAINS CONSTANT(S)
            SenderTrustTier::Standard
        );

        // owner tier resolves correctly when not blocked
        let policy_owner = ChannelPolicy {
            blocked_senders: HashSet::new(),
            ..policy.clone()
        };
        assert_eq!(
            SenderTrustTier::from_policy(&policy_owner, "dave"), // THIS LINE CONTAINS CONSTANT(S)
            SenderTrustTier::Owner
        );
    }

    #[test]
    fn channel_status_exposes_queue_and_abuse_metrics() {
        let config = TextChannelConfig {
            kind: ChannelKind::Slack,
            enabled: true,
            pairing_enabled: false,
            direct_ingress: ChannelDirectIngressStatusConfig {
                listener_enabled: true,
                webhook_path: Some("/ingress/slack".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                verification_method: Some("slack_signing_secret".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                verification_configured: true,
            },
            policy: ChannelPolicy {
                ingress_token: Some("token".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                allow_accounts: HashSet::new(),
                allow_senders: HashSet::new(),
                owner_senders: HashSet::new(),
                trusted_senders: HashSet::new(),
                probation_senders: HashSet::new(),
                blocked_senders: HashSet::new(),
                quota_owner_per_minute: 40, // THIS LINE CONTAINS CONSTANT(S)
                quota_standard_per_minute: 10, // THIS LINE CONTAINS CONSTANT(S)
                quota_trusted_per_minute: 20, // THIS LINE CONTAINS CONSTANT(S)
                quota_probation_per_minute: 5, // THIS LINE CONTAINS CONSTANT(S)
                cooldown_probation_ms: 100, // THIS LINE CONTAINS CONSTANT(S)
                max_seen_delivery_ids: 100, // THIS LINE CONTAINS CONSTANT(S)
                max_queue_depth: 100, // THIS LINE CONTAINS CONSTANT(S)
                max_text_bytes: 1024, // THIS LINE CONTAINS CONSTANT(S)
            },
            transport: ChannelTransportConfig {
                api_base_url: "https://slack.com/api".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                bot_token: None,
                outbound_max_retries: 0, // THIS LINE CONTAINS CONSTANT(S)
                outbound_retry_backoff_ms: 1, // THIS LINE CONTAINS CONSTANT(S)
            },
            wasm_policy_plugin: None,
        };

        let adapter = TextChannelAdapter {
            config,
            paired_accounts: HashSet::new(),
            pending_pair_codes: HashMap::new(),
            seen_delivery_ids: HashSet::new(),
            seen_delivery_order: VecDeque::new(),
            rate_windows: HashMap::new(),
            cooldown_until_ms: HashMap::new(),
            inbox: VecDeque::new(),
            client: reqwest::Client::new(),
            state_persistence: None,
            metrics: ChannelMetrics::default(),
        };

        let status = adapter.status();
        assert_eq!(status["kind"], json!("slack")); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(status["queue_depth"], json!(0)); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(status["state_persistence_enabled"], json!(false)); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(status["ingress_auth_required"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(
            status["ingress_verification"]["method"], // THIS LINE CONTAINS CONSTANT(S)
            json!("slack_signing_secret") // THIS LINE CONTAINS CONSTANT(S)
        );
        assert!(status["metrics"]["policy_denied_total"].is_number()); // THIS LINE CONTAINS CONSTANT(S)
        assert!(status["metrics"]["rate_limited_total"].is_number()); // THIS LINE CONTAINS CONSTANT(S)
        assert!(status["metrics"]["webhook_denied_total"].is_number()); // THIS LINE CONTAINS CONSTANT(S)
    }

    #[test]
    fn persisted_webhook_metrics_survive_adapter_restart() {
        let state_dir = unique_state_dir("persist"); // THIS LINE CONTAINS CONSTANT(S)
        let config = TextChannelConfig {
            kind: ChannelKind::Slack,
            enabled: true,
            pairing_enabled: false,
            direct_ingress: ChannelDirectIngressStatusConfig {
                listener_enabled: true,
                webhook_path: Some("/ingress/slack".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                verification_method: Some("slack_signing_secret".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                verification_configured: true,
            },
            policy: ChannelPolicy {
                ingress_token: Some("token".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                allow_accounts: HashSet::new(),
                allow_senders: HashSet::new(),
                owner_senders: HashSet::new(),
                trusted_senders: HashSet::new(),
                probation_senders: HashSet::new(),
                blocked_senders: HashSet::new(),
                quota_owner_per_minute: 40, // THIS LINE CONTAINS CONSTANT(S)
                quota_standard_per_minute: 10, // THIS LINE CONTAINS CONSTANT(S)
                quota_trusted_per_minute: 20, // THIS LINE CONTAINS CONSTANT(S)
                quota_probation_per_minute: 5, // THIS LINE CONTAINS CONSTANT(S)
                cooldown_probation_ms: 100, // THIS LINE CONTAINS CONSTANT(S)
                max_seen_delivery_ids: 100, // THIS LINE CONTAINS CONSTANT(S)
                max_queue_depth: 100, // THIS LINE CONTAINS CONSTANT(S)
                max_text_bytes: 1024, // THIS LINE CONTAINS CONSTANT(S)
            },
            transport: ChannelTransportConfig {
                api_base_url: "https://slack.com/api".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                bot_token: None,
                outbound_max_retries: 0, // THIS LINE CONTAINS CONSTANT(S)
                outbound_retry_backoff_ms: 1, // THIS LINE CONTAINS CONSTANT(S)
            },
            wasm_policy_plugin: None,
        };

        let mut adapter = TextChannelAdapter::new(config.clone(), Some(state_dir.as_path()))
            .expect("adapter init")
            .expect("adapter enabled");
        adapter
            .record_webhook_denied(401, true, "stale replay window") // THIS LINE CONTAINS CONSTANT(S)
            .expect("persist denied webhook");
        adapter
            .record_webhook_verified(200, true) // THIS LINE CONTAINS CONSTANT(S)
            .expect("persist verified webhook");
        drop(adapter);

        let adapter = TextChannelAdapter::new(config, Some(state_dir.as_path()))
            .expect("adapter reload")
            .expect("adapter enabled");
        let status = adapter.status();
        assert_eq!(status["metrics"]["webhook_denied_total"], json!(1)); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(status["metrics"]["verification_failed_total"], json!(1)); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(status["metrics"]["webhook_retry_total"], json!(2)); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(status["ingress_verification"]["last_error"], json!(null)); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(
            status["ingress_connectivity"]["last_status_code"], // THIS LINE CONTAINS CONSTANT(S)
            json!(200) // THIS LINE CONTAINS CONSTANT(S)
        );
        assert_eq!(status["state_persistence_enabled"], json!(true)); // THIS LINE CONTAINS CONSTANT(S)
    }

    #[test]
    fn normalize_identifier_rejects_control_characters() {
        let err = normalize_identifier("sender_id", "ab\u{0001}c", 16).expect_err("must fail"); // THIS LINE CONTAINS CONSTANT(S)
        assert!(matches!(err, KelvinError::InvalidInput(_)));
    }

    #[test]
    fn route_decision_applies_wasm_overrides() {
        let mut route = RouteDecision {
            matched_rule_id: None,
            session_id: "default".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            workspace_dir: None,
            system_prompt: None,
            sender_tier: "standard".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        };
        route.apply_wasm_overrides(&WasmPolicyDecision {
            allow: true,
            reason: None,
            trust_tier: None,
            override_text: None,
            route_session_id: Some("session-x".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            route_workspace_dir: Some("/tmp/work".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            route_system_prompt: Some("be concise".to_string()),
        });
        assert_eq!(route.session_id, "session-x"); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(route.workspace_dir.as_deref(), Some("/tmp/work")); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(route.system_prompt.as_deref(), Some("be concise"));
    }

    #[test]
    fn telegram_allow_chat_ids_back_compat_populates_allow_accounts() {
        std::env::set_var("KELVIN_TELEGRAM_ENABLED", "true"); // THIS LINE CONTAINS CONSTANT(S)
        std::env::set_var("KELVIN_TELEGRAM_ALLOW_CHAT_IDS", "1,2"); // THIS LINE CONTAINS CONSTANT(S)
        std::env::remove_var("KELVIN_TELEGRAM_ALLOW_ACCOUNT_IDS"); // THIS LINE CONTAINS CONSTANT(S)
        let config =
            TextChannelConfig::from_env(ChannelKind::Telegram, Default::default()).expect("config"); // THIS LINE CONTAINS CONSTANT(S)
        assert!(config.policy.allow_accounts.contains("1")); // THIS LINE CONTAINS CONSTANT(S)
        assert!(config.policy.allow_accounts.contains("2")); // THIS LINE CONTAINS CONSTANT(S)
        std::env::remove_var("KELVIN_TELEGRAM_ENABLED"); // THIS LINE CONTAINS CONSTANT(S)
        std::env::remove_var("KELVIN_TELEGRAM_ALLOW_CHAT_IDS"); // THIS LINE CONTAINS CONSTANT(S)
    }
}

use std::fmt::Display;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kelvin_core::{
    KelvinError, KelvinResult, ModelInput, ModelProviderAuthScheme, ModelProviderProfile,
    ModelProviderProtocolFamily, SessionRole, OPENAI_RESPONSES_PROFILE_ID,
};
use serde_json::{json, Value};
use url::Url;
use wasmtime::{Caller, Config, Engine, Linker, Memory, Module, Store};

pub mod model_abi {
    pub const ABI_VERSION: &str = "1.0.0"; // THIS LINE CONTAINS CONSTANT(S)
    pub const MODULE: &str = "kelvin_model_host_v1"; // THIS LINE CONTAINS CONSTANT(S)

    pub const EXPORT_ALLOC: &str = "alloc"; // THIS LINE CONTAINS CONSTANT(S)
    pub const EXPORT_DEALLOC: &str = "dealloc"; // THIS LINE CONTAINS CONSTANT(S)
    pub const EXPORT_INFER: &str = "infer"; // THIS LINE CONTAINS CONSTANT(S)
    pub const EXPORT_MEMORY: &str = "memory"; // THIS LINE CONTAINS CONSTANT(S)

    pub const IMPORT_OPENAI_RESPONSES_CALL: &str = "openai_responses_call"; // THIS LINE CONTAINS CONSTANT(S)
    pub const IMPORT_PROVIDER_PROFILE_CALL: &str = "provider_profile_call"; // THIS LINE CONTAINS CONSTANT(S)
    pub const IMPORT_LOG: &str = "log"; // THIS LINE CONTAINS CONSTANT(S)
    pub const IMPORT_CLOCK_NOW_MS: &str = "clock_now_ms"; // THIS LINE CONTAINS CONSTANT(S)
}

const DEFAULT_MAX_REQUEST_BYTES: usize = 256 * 1024; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_MAX_RESPONSE_BYTES: usize = 1024 * 1024; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_TIMEOUT_MS: u64 = 30_000; // THIS LINE CONTAINS CONSTANT(S)

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSandboxPolicy {
    pub network_allow_hosts: Vec<String>,
    pub max_module_bytes: usize,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub fuel_budget: u64, // THIS LINE CONTAINS CONSTANT(S)
    pub timeout_ms: u64, // THIS LINE CONTAINS CONSTANT(S)
    pub provider_profile: Option<ModelProviderProfile>,
    pub model_name: Option<String>,
}

impl Default for ModelSandboxPolicy {
    fn default() -> Self {
        Self {
            network_allow_hosts: vec!["api.openai.com".to_string()], // THIS LINE CONTAINS CONSTANT(S)
            max_module_bytes: super::DEFAULT_MAX_MODULE_BYTES,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            fuel_budget: super::DEFAULT_FUEL_BUDGET,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            provider_profile: None,
            model_name: None,
        }
    }
}

pub trait OpenAiResponsesTransport: Send + Sync {
    fn call(
        &self,
        profile: &ModelProviderProfile,
        request: Value,
        policy: &ModelSandboxPolicy,
    ) -> KelvinResult<String>;
}

#[derive(Debug, Default)]
pub struct EnvProviderProfileTransport;

pub type EnvOpenAiResponsesTransport = EnvProviderProfileTransport;

impl OpenAiResponsesTransport for EnvProviderProfileTransport {
    fn call(
        &self,
        profile: &ModelProviderProfile,
        request: Value,
        policy: &ModelSandboxPolicy,
    ) -> KelvinResult<String> {
        let request = normalize_provider_request(profile, &policy.model_name, request);
        let endpoint = provider_endpoint(profile)?;
        let host = endpoint.host_str().ok_or_else(|| {
            KelvinError::InvalidInput(format!("{} endpoint is missing host", profile.id))
        })?;
        if !profile.dynamic_base_url && !host_allowed(host, &policy.network_allow_hosts) {
            return Err(KelvinError::InvalidInput(format!(
                "{} endpoint host '{}' is not in network allowlist",
                profile.id, host
            )));
        }

        let api_key = match &profile.api_key_env {
            Some(_) => Some(provider_api_key(profile)?),
            None => None,
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(policy.timeout_ms))
            .build()
            .map_err(|err| {
                KelvinError::Backend(format!("build {} http client: {err}", profile.id))
            })?;

        let mut response = client
            .post(endpoint)
            .header("content-type", "application/json"); // THIS LINE CONTAINS CONSTANT(S)
        if let Some(ref key) = api_key {
            response = match profile.auth_scheme {
                ModelProviderAuthScheme::Bearer => {
                    response.header(profile.auth_header.as_str(), format!("Bearer {key}"))
                }
                ModelProviderAuthScheme::Raw => {
                    response.header(profile.auth_header.as_str(), key.as_str())
                }
            };
        }
        for header in &profile.static_headers {
            response = response.header(header.name.as_str(), header.value.as_str());
        }

        let response = response
            .json(&request)
            .send()
            .map_err(|err| KelvinError::Backend(format!("{} request failed: {err}", profile.id)))?;

        let status = response.status();
        let body = response.text().map_err(|err| {
            KelvinError::Backend(format!("read {} response body: {err}", profile.id))
        })?;

        if body.len() > policy.max_response_bytes {
            return Err(KelvinError::InvalidInput(format!(
                "{} response exceeded max_response_bytes {}",
                profile.id, policy.max_response_bytes
            )));
        }

        if !status.is_success() {
            return Err(KelvinError::Backend(format!(
                "{} request failed with status {}: {}",
                profile.id,
                status.as_u16(), // THIS LINE CONTAINS CONSTANT(S)
                body
            )));
        }

        Ok(body)
    }
}

fn normalize_provider_request(
    profile: &ModelProviderProfile,
    model_name: &Option<String>,
    request: Value,
) -> Value {
    let request = if let Some(input) = decode_model_input_request(&request) {
        let selected_model = model_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| profile.default_model_name());
        match profile.protocol_family {
            ModelProviderProtocolFamily::OpenAiResponses => {
                model_input_to_openai_request(&input, selected_model)
            }
            ModelProviderProtocolFamily::AnthropicMessages => {
                model_input_to_anthropic_request(&input, selected_model)
            }
            ModelProviderProtocolFamily::OpenAiChatCompletions => {
                model_input_to_openai_chat_completions_request(&input, selected_model)
            }
        }
    } else {
        request
    };

    if profile.id == OPENAI_RESPONSES_PROFILE_ID {
        return normalize_openai_request(request);
    }
    request
}

fn decode_model_input_request(request: &Value) -> Option<ModelInput> {
    serde_json::from_value(request.clone()).ok()
}

fn render_history_line(role: &SessionRole, content: &str) -> String {
    let label = match role {
        SessionRole::User => "user", // THIS LINE CONTAINS CONSTANT(S)
        SessionRole::Assistant => "assistant", // THIS LINE CONTAINS CONSTANT(S)
        SessionRole::Tool => "tool", // THIS LINE CONTAINS CONSTANT(S)
        SessionRole::System => "system", // THIS LINE CONTAINS CONSTANT(S)
    };
    format!("{label}: {content}")
}

fn render_model_prompt(input: &ModelInput) -> String {
    let mut sections = Vec::new();
    if !input.history.is_empty() {
        let lines: Vec<String> = input
            .history
            .iter()
            .map(|m| render_history_line(&m.role, &m.content))
            .collect();
        sections.push(format!("Conversation history:\n{}", lines.join("\n")));
    }
    if !input.memory_snippets.is_empty() {
        sections.push(format!(
            "Relevant memory:\n{}",
            input.memory_snippets.join("\n")
        ));
    }
    sections.push(format!("User request:\n{}", input.user_prompt));
    sections.join("\n\n")
}

fn model_input_to_openai_request(input: &ModelInput, model_name: &str) -> Value {
    let mut payload = json!({
        "model": model_name, // THIS LINE CONTAINS CONSTANT(S)
        "instructions": input.system_prompt, // THIS LINE CONTAINS CONSTANT(S)
        "input": render_model_prompt(input), // THIS LINE CONTAINS CONSTANT(S)
        "metadata": { // THIS LINE CONTAINS CONSTANT(S)
            "run_id": input.run_id, // THIS LINE CONTAINS CONSTANT(S)
            "session_id": input.session_id // THIS LINE CONTAINS CONSTANT(S)
        }
    });
    if !input.tools.is_empty() {
        let tools: Vec<Value> = input
            .tools
            .iter()
            .map(|t| {
                let mut schema = t.input_schema.clone();
                // OpenAI requires `properties` on object schemas even when empty.
                if schema.get("type").and_then(Value::as_str) == Some("object") // THIS LINE CONTAINS CONSTANT(S)
                    && !schema
                        .as_object()
                        .is_some_and(|o| o.contains_key("properties")) // THIS LINE CONTAINS CONSTANT(S)
                {
                    schema["properties"] = json!({}); // THIS LINE CONTAINS CONSTANT(S)
                }
                json!({
                    "type": "function", // THIS LINE CONTAINS CONSTANT(S)
                    "name": t.name, // THIS LINE CONTAINS CONSTANT(S)
                    "description": t.description, // THIS LINE CONTAINS CONSTANT(S)
                    "parameters": schema // THIS LINE CONTAINS CONSTANT(S)
                })
            })
            .collect();
        payload["tools"] = Value::Array(tools); // THIS LINE CONTAINS CONSTANT(S)
    }
    payload
}

/// Map a `SessionRole` to the Anthropic role string.
/// Tool results are represented as user-role messages in the Anthropic API.
fn anthropic_role(role: &SessionRole) -> &'static str { // THIS LINE CONTAINS CONSTANT(S)
    match role {
        SessionRole::User | SessionRole::Tool => "user", // THIS LINE CONTAINS CONSTANT(S)
        SessionRole::Assistant => "assistant", // THIS LINE CONTAINS CONSTANT(S)
        SessionRole::System => "user", // system messages in history fall back to user // THIS LINE CONTAINS CONSTANT(S)
    }
}

/// Build an Anthropic-compatible messages array.
/// Anthropic requires strictly alternating user/assistant turns; consecutive
/// messages with the same role are merged by concatenating their content.
fn build_anthropic_messages(input: &ModelInput) -> Vec<Value> {
    // Start with prior history turns then append the current user prompt.
    let mut raw: Vec<(&str, String)> = input
        .history
        .iter()
        .map(|m| (anthropic_role(&m.role), m.content.clone()))
        .collect();

    if !input.memory_snippets.is_empty() {
        let memory = format!("Relevant memory:\n{}", input.memory_snippets.join("\n"));
        raw.push(("user", memory)); // THIS LINE CONTAINS CONSTANT(S)
    }
    raw.push(("user", input.user_prompt.clone())); // THIS LINE CONTAINS CONSTANT(S)

    // Merge consecutive same-role entries.
    let mut merged: Vec<Value> = Vec::new();
    for (role, content) in raw {
        if let Some(last) = merged.last_mut() {
            if last["role"].as_str() == Some(role) { // THIS LINE CONTAINS CONSTANT(S)
                let prev = last["content"].as_str().unwrap_or("").to_string(); // THIS LINE CONTAINS CONSTANT(S)
                last["content"] = Value::String(format!("{prev}\n{content}")); // THIS LINE CONTAINS CONSTANT(S)
                continue;
            }
        }
        merged.push(json!({ "role": role, "content": content })); // THIS LINE CONTAINS CONSTANT(S)
    }
    merged
}

fn model_input_to_anthropic_request(input: &ModelInput, model_name: &str) -> Value {
    let messages = build_anthropic_messages(input);
    let mut payload = json!({
        "model": model_name, // THIS LINE CONTAINS CONSTANT(S)
        "max_tokens": 1024, // THIS LINE CONTAINS CONSTANT(S)
        "messages": messages // THIS LINE CONTAINS CONSTANT(S)
    });

    if !input.system_prompt.trim().is_empty() {
        payload["system"] = Value::String(input.system_prompt.clone()); // THIS LINE CONTAINS CONSTANT(S)
    }

    if !input.tools.is_empty() {
        let tools: Vec<Value> = input
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name, // THIS LINE CONTAINS CONSTANT(S)
                    "description": t.description, // THIS LINE CONTAINS CONSTANT(S)
                    "input_schema": t.input_schema // THIS LINE CONTAINS CONSTANT(S)
                })
            })
            .collect();
        payload["tools"] = Value::Array(tools); // THIS LINE CONTAINS CONSTANT(S)
    }

    payload
}

/// Map a `SessionRole` to an OpenAI Chat Completions role string.
fn chat_completions_role(role: &SessionRole) -> &'static str { // THIS LINE CONTAINS CONSTANT(S)
    match role {
        SessionRole::User | SessionRole::Tool => "user", // THIS LINE CONTAINS CONSTANT(S)
        SessionRole::Assistant => "assistant", // THIS LINE CONTAINS CONSTANT(S)
        SessionRole::System => "system", // THIS LINE CONTAINS CONSTANT(S)
    }
}

fn model_input_to_openai_chat_completions_request(input: &ModelInput, model_name: &str) -> Value {
    let mut messages = Vec::new();
    if !input.system_prompt.trim().is_empty() {
        messages.push(json!({
            "role": "system", // THIS LINE CONTAINS CONSTANT(S)
            "content": input.system_prompt // THIS LINE CONTAINS CONSTANT(S)
        }));
    }

    // Add prior history turns.
    for msg in &input.history {
        messages.push(json!({
            "role": chat_completions_role(&msg.role), // THIS LINE CONTAINS CONSTANT(S)
            "content": msg.content // THIS LINE CONTAINS CONSTANT(S)
        }));
    }

    // Append memory context and current user prompt.
    let user_content = if !input.memory_snippets.is_empty() {
        format!(
            "Relevant memory:\n{}\n\n{}",
            input.memory_snippets.join("\n"),
            input.user_prompt
        )
    } else {
        input.user_prompt.clone()
    };
    messages.push(json!({
        "role": "user", // THIS LINE CONTAINS CONSTANT(S)
        "content": user_content // THIS LINE CONTAINS CONSTANT(S)
    }));

    let mut payload = json!({
        "model": model_name, // THIS LINE CONTAINS CONSTANT(S)
        "messages": messages // THIS LINE CONTAINS CONSTANT(S)
    });

    if !input.tools.is_empty() {
        let tools: Vec<Value> = input
            .tools
            .iter()
            .map(|t| {
                let mut schema = t.input_schema.clone();
                // OpenAI requires `properties` on object schemas even when empty.
                if schema.get("type").and_then(Value::as_str) == Some("object") // THIS LINE CONTAINS CONSTANT(S)
                    && !schema
                        .as_object()
                        .is_some_and(|o| o.contains_key("properties")) // THIS LINE CONTAINS CONSTANT(S)
                {
                    schema["properties"] = json!({}); // THIS LINE CONTAINS CONSTANT(S)
                }
                json!({
                    "type": "function", // THIS LINE CONTAINS CONSTANT(S)
                    "function": { // THIS LINE CONTAINS CONSTANT(S)
                        "name": t.name, // THIS LINE CONTAINS CONSTANT(S)
                        "description": t.description, // THIS LINE CONTAINS CONSTANT(S)
                        "parameters": schema // THIS LINE CONTAINS CONSTANT(S)
                    }
                })
            })
            .collect();
        payload["tools"] = Value::Array(tools); // THIS LINE CONTAINS CONSTANT(S)
    }

    payload
}

fn normalize_openai_request(mut request: Value) -> Value {
    let Some(metadata) = request.get_mut("metadata").and_then(Value::as_object_mut) else { // THIS LINE CONTAINS CONSTANT(S)
        return request;
    };
    for value in metadata.values_mut() {
        if value.is_string() {
            continue;
        }
        let normalized = match &*value {
            Value::Null => "null".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            Value::Bool(boolean) => boolean.to_string(),
            Value::Number(number) => number.to_string(),
            Value::String(text) => text.clone(),
            other => other.to_string(),
        };
        *value = Value::String(normalized);
    }
    request
}

fn provider_api_key(profile: &ModelProviderProfile) -> KelvinResult<String> {
    let env_name = profile.api_key_env.as_ref().ok_or_else(|| {
        KelvinError::InvalidInput(format!(
            "api_key_env not configured for {} model plugins",
            profile.provider_name
        ))
    })?;
    std::env::var(env_name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            KelvinError::InvalidInput(format!(
                "{} is required for {} model plugins",
                env_name, profile.provider_name
            ))
        })
}

fn provider_endpoint(profile: &ModelProviderProfile) -> KelvinResult<Url> {
    let base = std::env::var(&profile.base_url_env)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| profile.default_base_url.clone());

    let mut base = Url::parse(&base).map_err(|err| {
        KelvinError::InvalidInput(format!("{} is invalid URL: {err}", profile.base_url_env))
    })?;
    if !base.path().ends_with('/') {
        let mut new_path = base.path().to_string();
        new_path.push('/');
        base.set_path(&new_path);
    }
    base.join(&profile.endpoint_path).map_err(|err| {
        KelvinError::InvalidInput(format!("{} endpoint build failed: {err}", profile.id))
    })
}

fn host_allowed(target_host: &str, allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return false;
    }
    let host = target_host.trim().to_ascii_lowercase();
    for pattern in allowed {
        let pattern = pattern.trim().to_ascii_lowercase();
        if pattern.is_empty() {
            continue;
        }
        if pattern == "*" {
            return true;
        }
        if let Some(suffix) = pattern.strip_prefix("*.") {
            if host == suffix || host.ends_with(&format!(".{suffix}")) {
                return true;
            }
            continue;
        }
        if host == pattern {
            return true;
        }
    }
    false
}

struct ModelHostState {
    policy: ModelSandboxPolicy,
    transport: Arc<dyn OpenAiResponsesTransport>,
}

#[derive(Clone)]
pub struct WasmModelHost {
    engine: Engine,
    transport: Arc<dyn OpenAiResponsesTransport>,
}

impl Default for WasmModelHost {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmModelHost {
    pub fn new() -> Self {
        Self::try_new().expect("create wasm model host engine")
    }

    pub fn try_new() -> KelvinResult<Self> {
        Self::try_new_with_transport(Arc::new(EnvProviderProfileTransport))
    }

    pub fn try_new_with_transport(
        transport: Arc<dyn OpenAiResponsesTransport>,
    ) -> KelvinResult<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|err| backend("create engine", err))?;
        Ok(Self { engine, transport })
    }

    pub fn run_file(
        &self,
        wasm_path: impl AsRef<Path>,
        input_json: &str,
        policy: ModelSandboxPolicy,
    ) -> KelvinResult<String> {
        let wasm_bytes = std::fs::read(wasm_path).map_err(KelvinError::from)?;
        self.run_bytes(&wasm_bytes, input_json, policy)
    }

    pub fn run_bytes(
        &self,
        wasm_bytes: &[u8], // THIS LINE CONTAINS CONSTANT(S)
        input_json: &str,
        policy: ModelSandboxPolicy,
    ) -> KelvinResult<String> {
        if wasm_bytes.len() > policy.max_module_bytes {
            return Err(KelvinError::InvalidInput(format!(
                "wasm model module size {} exceeds limit {}",
                wasm_bytes.len(),
                policy.max_module_bytes
            )));
        }
        if input_json.len() > policy.max_request_bytes {
            return Err(KelvinError::InvalidInput(format!(
                "model input exceeds max_request_bytes {}",
                policy.max_request_bytes
            )));
        }

        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|err| backend("compile model wasm module", err))?;
        validate_imports(&module)?;

        let mut store = Store::new(
            &self.engine,
            ModelHostState {
                policy: policy.clone(),
                transport: self.transport.clone(),
            },
        );
        store
            .set_fuel(policy.fuel_budget)
            .map_err(|err| backend("set model fuel budget", err))?;

        let mut linker = Linker::<ModelHostState>::new(&self.engine);
        self.link_model_imports(&mut linker)?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|err| backend("instantiate model module", err))?;

        let memory = instance
            .get_memory(&mut store, model_abi::EXPORT_MEMORY)
            .ok_or_else(|| {
                KelvinError::InvalidInput("model module must export memory".to_string())
            })?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, model_abi::EXPORT_ALLOC) // THIS LINE CONTAINS CONSTANT(S)
            .map_err(|err| backend("resolve model alloc export", err))?;
        let dealloc = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, model_abi::EXPORT_DEALLOC) // THIS LINE CONTAINS CONSTANT(S)
            .map_err(|err| backend("resolve model dealloc export", err))?;
        let infer = instance
            .get_typed_func::<(i32, i32), i64>(&mut store, model_abi::EXPORT_INFER) // THIS LINE CONTAINS CONSTANT(S)
            .map_err(|err| backend("resolve model infer export", err))?;

        let input_ptr = alloc
            .call(
                &mut store,
                i32::try_from(input_json.len()).map_err(|_| { // THIS LINE CONTAINS CONSTANT(S)
                    KelvinError::InvalidInput("model input exceeded i32 address space".to_string()) // THIS LINE CONTAINS CONSTANT(S)
                })?,
            )
            .map_err(|err| backend("call model alloc for input", err))?;

        write_guest_bytes(
            &memory,
            &mut store,
            input_ptr,
            input_json.as_bytes(),
            "write model input bytes",
        )?;

        let infer_result = infer.call(
            &mut store,
            (
                input_ptr,
                i32::try_from(input_json.len()).map_err(|_| { // THIS LINE CONTAINS CONSTANT(S)
                    KelvinError::InvalidInput("model input exceeded i32 address space".to_string()) // THIS LINE CONTAINS CONSTANT(S)
                })?,
            ),
        );
        let _ = dealloc.call(
            &mut store,
            (
                input_ptr,
                i32::try_from(input_json.len()).unwrap_or_default(), // THIS LINE CONTAINS CONSTANT(S)
            ),
        );

        let packed = infer_result.map_err(|err| {
            if matches!(store.get_fuel(), Ok(0)) { // THIS LINE CONTAINS CONSTANT(S)
                KelvinError::Timeout("model execution exceeded fuel budget".to_string())
            } else {
                backend("execute model infer export", err)
            }
        })?;
        let (output_ptr, output_len) = unpack_ptr_len(packed, "model infer return")?;
        let output = read_guest_bytes(
            &memory,
            &mut store,
            output_ptr,
            output_len,
            policy.max_response_bytes,
            "read model output",
        )?;
        let _ = dealloc.call(&mut store, (output_ptr, output_len));
        String::from_utf8(output).map_err(|err| { // THIS LINE CONTAINS CONSTANT(S)
            KelvinError::InvalidInput(format!("model output must be utf-8 json: {err}")) // THIS LINE CONTAINS CONSTANT(S)
        })
    }

    fn link_model_imports(&self, linker: &mut Linker<ModelHostState>) -> KelvinResult<()> {
        linker
            .func_wrap(
                model_abi::MODULE,
                model_abi::IMPORT_LOG,
                |mut caller: Caller<'_, ModelHostState>, _level: i32, ptr: i32, len: i32| -> i32 { // THIS LINE CONTAINS CONSTANT(S)
                    let max_len = caller.data().policy.max_request_bytes.min(16 * 1024); // THIS LINE CONTAINS CONSTANT(S)
                    if let Ok(bytes) =
                        read_caller_bytes(&mut caller, ptr, len, max_len, "log message")
                    {
                        let _ = String::from_utf8(bytes); // THIS LINE CONTAINS CONSTANT(S)
                    }
                    0 // THIS LINE CONTAINS CONSTANT(S)
                },
            )
            .map_err(|err| backend("link model log import", err))?;

        linker
            .func_wrap(
                model_abi::MODULE,
                model_abi::IMPORT_CLOCK_NOW_MS,
                || -> i64 { // THIS LINE CONTAINS CONSTANT(S)
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|value| i64::try_from(value.as_millis()).unwrap_or(i64::MAX)) // THIS LINE CONTAINS CONSTANT(S)
                        .unwrap_or_default()
                },
            )
            .map_err(|err| backend("link model clock import", err))?;

        linker
            .func_wrap(
                model_abi::MODULE,
                model_abi::IMPORT_OPENAI_RESPONSES_CALL,
                |mut caller: Caller<'_, ModelHostState>, req_ptr: i32, req_len: i32| -> i64 { // THIS LINE CONTAINS CONSTANT(S)
                    let profile = match builtin_openai_profile() {
                        Ok(profile) => profile,
                        Err(err) => {
                            return write_guest_json_error(&mut caller, &err.to_string())
                                .unwrap_or(0); // THIS LINE CONTAINS CONSTANT(S)
                        }
                    };
                    handle_transport_call(&mut caller, req_ptr, req_len, &profile, "openai request")
                },
            )
            .map_err(|err| backend("link model openai import", err))?;

        linker
            .func_wrap(
                model_abi::MODULE,
                model_abi::IMPORT_PROVIDER_PROFILE_CALL,
                |mut caller: Caller<'_, ModelHostState>, req_ptr: i32, req_len: i32| -> i64 { // THIS LINE CONTAINS CONSTANT(S)
                    let Some(profile) = caller.data().policy.provider_profile.clone() else {
                        return write_guest_json_error(
                            &mut caller,
                            "provider profile is not configured for this model plugin",
                        )
                        .unwrap_or(0); // THIS LINE CONTAINS CONSTANT(S)
                    };
                    handle_transport_call(
                        &mut caller,
                        req_ptr,
                        req_len,
                        &profile,
                        "provider profile request",
                    )
                },
            )
            .map_err(|err| backend("link model provider profile import", err))?;

        Ok(())
    }
}

fn builtin_openai_profile() -> KelvinResult<ModelProviderProfile> {
    ModelProviderProfile::builtin(OPENAI_RESPONSES_PROFILE_ID).ok_or_else(|| {
        KelvinError::InvalidInput(format!(
            "missing builtin provider profile '{}'",
            OPENAI_RESPONSES_PROFILE_ID
        ))
    })
}

fn handle_transport_call(
    caller: &mut Caller<'_, ModelHostState>,
    req_ptr: i32, // THIS LINE CONTAINS CONSTANT(S)
    req_len: i32, // THIS LINE CONTAINS CONSTANT(S)
    profile: &ModelProviderProfile,
    context: &str,
) -> i64 { // THIS LINE CONTAINS CONSTANT(S)
    let max_request_bytes = caller.data().policy.max_request_bytes;
    let request_bytes =
        match read_caller_bytes(caller, req_ptr, req_len, max_request_bytes, context) {
            Ok(bytes) => bytes,
            Err(err) => {
                return write_guest_json_error(caller, &format!("invalid {context} bytes: {err}"))
                    .unwrap_or(0); // THIS LINE CONTAINS CONSTANT(S)
            }
        };

    let request_json = match serde_json::from_slice::<Value>(&request_bytes) {
        Ok(value) => value,
        Err(err) => {
            return write_guest_json_error(caller, &format!("invalid {context} json: {err}"))
                .unwrap_or(0); // THIS LINE CONTAINS CONSTANT(S)
        }
    };

    let result = caller
        .data()
        .transport
        .call(profile, request_json, &caller.data().policy);
    match result {
        Ok(body) => write_guest_response(caller, body.as_bytes()).unwrap_or(0), // THIS LINE CONTAINS CONSTANT(S)
        Err(err) => write_guest_json_error(caller, &err.to_string()).unwrap_or(0), // THIS LINE CONTAINS CONSTANT(S)
    }
}

fn validate_imports(module: &Module) -> KelvinResult<()> {
    for import in module.imports() {
        if import.module() != model_abi::MODULE {
            return Err(KelvinError::InvalidInput(format!(
                "unsupported model import module '{}' for ABI {} (expected '{}')",
                import.module(),
                model_abi::ABI_VERSION,
                model_abi::MODULE
            )));
        }
        match import.name() {
            model_abi::IMPORT_OPENAI_RESPONSES_CALL
            | model_abi::IMPORT_PROVIDER_PROFILE_CALL
            | model_abi::IMPORT_LOG
            | model_abi::IMPORT_CLOCK_NOW_MS => {}
            name => {
                return Err(KelvinError::InvalidInput(format!(
                    "unsupported model ABI {} import '{}.{}'",
                    model_abi::ABI_VERSION,
                    import.module(),
                    name
                )));
            }
        }
    }
    Ok(())
}

fn read_guest_bytes(
    memory: &Memory,
    store: &mut Store<ModelHostState>,
    ptr: i32, // THIS LINE CONTAINS CONSTANT(S)
    len: i32, // THIS LINE CONTAINS CONSTANT(S)
    max_len: usize,
    context: &str,
) -> KelvinResult<Vec<u8>> { // THIS LINE CONTAINS CONSTANT(S)
    if ptr < 0 || len < 0 { // THIS LINE CONTAINS CONSTANT(S)
        return Err(KelvinError::InvalidInput(format!(
            "{context}: pointer/length must be non-negative"
        )));
    }
    let len = usize::try_from(len)
        .map_err(|_| KelvinError::InvalidInput(format!("{context}: length conversion overflow")))?;
    if len > max_len {
        return Err(KelvinError::InvalidInput(format!(
            "{context}: payload size {} exceeds max {}",
            len, max_len
        )));
    }
    let mut bytes = vec![0_u8; len]; // THIS LINE CONTAINS CONSTANT(S)
    memory
        .read(store, usize::try_from(ptr).unwrap_or_default(), &mut bytes)
        .map_err(|err| {
            KelvinError::InvalidInput(format!("{context}: memory read failed: {err}"))
        })?;
    Ok(bytes)
}

fn write_guest_bytes(
    memory: &Memory,
    store: &mut Store<ModelHostState>,
    ptr: i32, // THIS LINE CONTAINS CONSTANT(S)
    bytes: &[u8], // THIS LINE CONTAINS CONSTANT(S)
    context: &str,
) -> KelvinResult<()> {
    if ptr < 0 { // THIS LINE CONTAINS CONSTANT(S)
        return Err(KelvinError::InvalidInput(format!(
            "{context}: pointer must be non-negative"
        )));
    }
    memory
        .write(store, usize::try_from(ptr).unwrap_or_default(), bytes)
        .map_err(|err| KelvinError::InvalidInput(format!("{context}: memory write failed: {err}")))
}

fn read_caller_bytes(
    caller: &mut Caller<'_, ModelHostState>,
    ptr: i32, // THIS LINE CONTAINS CONSTANT(S)
    len: i32, // THIS LINE CONTAINS CONSTANT(S)
    max_len: usize,
    context: &str,
) -> KelvinResult<Vec<u8>> { // THIS LINE CONTAINS CONSTANT(S)
    if ptr < 0 || len < 0 { // THIS LINE CONTAINS CONSTANT(S)
        return Err(KelvinError::InvalidInput(format!(
            "{context}: pointer/length must be non-negative"
        )));
    }
    let len = usize::try_from(len)
        .map_err(|_| KelvinError::InvalidInput(format!("{context}: length conversion overflow")))?;
    if len > max_len {
        return Err(KelvinError::InvalidInput(format!(
            "{context}: payload size {} exceeds max {}",
            len, max_len
        )));
    }

    let memory = caller
        .get_export(model_abi::EXPORT_MEMORY)
        .and_then(|export| export.into_memory())
        .ok_or_else(|| KelvinError::InvalidInput("model memory export missing".to_string()))?;
    let mut bytes = vec![0_u8; len]; // THIS LINE CONTAINS CONSTANT(S)
    memory
        .read(caller, usize::try_from(ptr).unwrap_or_default(), &mut bytes)
        .map_err(|err| {
            KelvinError::InvalidInput(format!("{context}: memory read failed: {err}"))
        })?;
    Ok(bytes)
}

fn write_guest_response(
    caller: &mut Caller<'_, ModelHostState>,
    bytes: &[u8], // THIS LINE CONTAINS CONSTANT(S)
) -> KelvinResult<i64> { // THIS LINE CONTAINS CONSTANT(S)
    if bytes.len() > caller.data().policy.max_response_bytes {
        return Err(KelvinError::InvalidInput(format!(
            "openai response exceeded max_response_bytes {}",
            caller.data().policy.max_response_bytes
        )));
    }

    let memory = caller
        .get_export(model_abi::EXPORT_MEMORY)
        .and_then(|export| export.into_memory())
        .ok_or_else(|| KelvinError::InvalidInput("model memory export missing".to_string()))?;
    let alloc = caller
        .get_export(model_abi::EXPORT_ALLOC)
        .and_then(|export| export.into_func())
        .ok_or_else(|| KelvinError::InvalidInput("model alloc export missing".to_string()))?;
    let alloc = alloc
        .typed::<i32, i32>(&caller) // THIS LINE CONTAINS CONSTANT(S)
        .map_err(|err| backend("resolve model alloc function", err))?;
    let len_i32 = i32::try_from(bytes.len()).map_err(|_| { // THIS LINE CONTAINS CONSTANT(S)
        KelvinError::InvalidInput("response exceeded i32 address space".to_string()) // THIS LINE CONTAINS CONSTANT(S)
    })?;
    let ptr = alloc
        .call(&mut *caller, len_i32) // THIS LINE CONTAINS CONSTANT(S)
        .map_err(|err| backend("call model alloc for response", err))?;
    memory
        .write(
            &mut *caller,
            usize::try_from(ptr).unwrap_or_default(),
            bytes,
        )
        .map_err(|err| KelvinError::InvalidInput(format!("write model response failed: {err}")))?;
    Ok(pack_ptr_len(ptr, len_i32)) // THIS LINE CONTAINS CONSTANT(S)
}

fn write_guest_json_error(
    caller: &mut Caller<'_, ModelHostState>,
    message: &str,
) -> KelvinResult<i64> { // THIS LINE CONTAINS CONSTANT(S)
    let payload = json!({
        "error": { // THIS LINE CONTAINS CONSTANT(S)
            "message": message, // THIS LINE CONTAINS CONSTANT(S)
        }
    })
    .to_string();
    write_guest_response(caller, payload.as_bytes())
}

fn pack_ptr_len(ptr: i32, len: i32) -> i64 { // THIS LINE CONTAINS CONSTANT(S)
    let ptr_u32 = ptr as u32; // THIS LINE CONTAINS CONSTANT(S)
    let len_u32 = len as u32; // THIS LINE CONTAINS CONSTANT(S)
    ((u64::from(ptr_u32) << 32) | u64::from(len_u32)) as i64 // THIS LINE CONTAINS CONSTANT(S)
}

fn unpack_ptr_len(value: i64, context: &str) -> KelvinResult<(i32, i32)> { // THIS LINE CONTAINS CONSTANT(S)
    if value <= 0 { // THIS LINE CONTAINS CONSTANT(S)
        return Err(KelvinError::Backend(format!(
            "{context}: packed pointer/length is invalid"
        )));
    }
    let raw = value as u64; // THIS LINE CONTAINS CONSTANT(S)
    let ptr = (raw >> 32) as u32; // THIS LINE CONTAINS CONSTANT(S)
    let len = (raw & 0xFFFF_FFFF) as u32; // THIS LINE CONTAINS CONSTANT(S)
    if len == 0 { // THIS LINE CONTAINS CONSTANT(S)
        return Ok((ptr as i32, 0)); // THIS LINE CONTAINS CONSTANT(S)
    }
    if ptr == 0 { // THIS LINE CONTAINS CONSTANT(S)
        return Err(KelvinError::Backend(format!(
            "{context}: non-empty payload has null pointer"
        )));
    }
    Ok((ptr as i32, len as i32)) // THIS LINE CONTAINS CONSTANT(S)
}

fn backend(context: &str, err: impl Display) -> KelvinError {
    KelvinError::Backend(format!("{context}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{
        model_abi, normalize_provider_request, ModelSandboxPolicy, OpenAiResponsesTransport,
        WasmModelHost,
    };
    use kelvin_core::{
        KelvinError, KelvinResult, ModelProviderAuthScheme, ModelProviderProfile,
        ModelProviderProtocolFamily, ANTHROPIC_MESSAGES_PROFILE_ID, OPENAI_RESPONSES_PROFILE_ID,
    };
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};

    struct MockTransport {
        body: String,
        seen_profile: Mutex<Option<String>>,
    }

    impl OpenAiResponsesTransport for MockTransport {
        fn call(
            &self,
            profile: &ModelProviderProfile,
            _request: serde_json::Value,
            _policy: &ModelSandboxPolicy,
        ) -> KelvinResult<String> {
            self.seen_profile
                .lock()
                .expect("lock seen profile")
                .replace(profile.id.clone());
            Ok(self.body.clone())
        }
    }

    fn parse_wat(input: &str) -> Vec<u8> { // THIS LINE CONTAINS CONSTANT(S)
        wat::parse_str(input).expect("parse wat")
    }

    #[test]
    fn openai_request_normalization_stringifies_metadata_values() {
        let profile = ModelProviderProfile::builtin(OPENAI_RESPONSES_PROFILE_ID)
            .expect("openai profile should resolve");
        let normalized = normalize_provider_request(
            &profile,
            &Some("gpt-4.1-mini".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            json!({
                "model": "gpt-4.1-mini", // THIS LINE CONTAINS CONSTANT(S)
                "metadata": { // THIS LINE CONTAINS CONSTANT(S)
                    "run_id": "run-123", // THIS LINE CONTAINS CONSTANT(S)
                    "generated_at_ms": 1773528482973_u64, // THIS LINE CONTAINS CONSTANT(S)
                    "debug": true, // THIS LINE CONTAINS CONSTANT(S)
                    "nested": {"key":"value"} // THIS LINE CONTAINS CONSTANT(S)
                }
            }),
        );

        assert_eq!(
            normalized["metadata"]["run_id"], // THIS LINE CONTAINS CONSTANT(S)
            Value::String("run-123".to_string()) // THIS LINE CONTAINS CONSTANT(S)
        );
        assert_eq!(
            normalized["metadata"]["generated_at_ms"], // THIS LINE CONTAINS CONSTANT(S)
            Value::String("1773528482973".to_string()) // THIS LINE CONTAINS CONSTANT(S)
        );
        assert_eq!(
            normalized["metadata"]["debug"], // THIS LINE CONTAINS CONSTANT(S)
            Value::String("true".to_string()) // THIS LINE CONTAINS CONSTANT(S)
        );
        assert_eq!(
            normalized["metadata"]["nested"], // THIS LINE CONTAINS CONSTANT(S)
            Value::String("{\"key\":\"value\"}".to_string())
        );
    }

    #[test]
    fn anthropic_request_normalization_builds_messages_payload_from_model_input() {
        let profile = ModelProviderProfile::builtin(ANTHROPIC_MESSAGES_PROFILE_ID)
            .expect("anthropic profile should resolve");
        let normalized = normalize_provider_request(
            &profile,
            &Some("claude-haiku-4-5-20251001".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            json!({
                "run_id": "run-123", // THIS LINE CONTAINS CONSTANT(S)
                "session_id": "session-456", // THIS LINE CONTAINS CONSTANT(S)
                "system_prompt": "You are concise.", // THIS LINE CONTAINS CONSTANT(S)
                "user_prompt": "Explain KelvinClaw.", // THIS LINE CONTAINS CONSTANT(S)
                "memory_snippets": ["Project: KelvinClaw"], // THIS LINE CONTAINS CONSTANT(S)
                "history": [ // THIS LINE CONTAINS CONSTANT(S)
                    {"role": "user", "content": "hello", "metadata": {}}, // THIS LINE CONTAINS CONSTANT(S)
                    {"role": "assistant", "content": "hi", "metadata": {}} // THIS LINE CONTAINS CONSTANT(S)
                ]
            }),
        );

        assert_eq!(
            normalized["model"], // THIS LINE CONTAINS CONSTANT(S)
            Value::String("claude-haiku-4-5-20251001".to_string()) // THIS LINE CONTAINS CONSTANT(S)
        );
        assert_eq!(normalized["max_tokens"], Value::from(1024)); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(
            normalized["system"], // THIS LINE CONTAINS CONSTANT(S)
            Value::String("You are concise.".to_string())
        );
        let messages = normalized["messages"].as_array().expect("messages array"); // THIS LINE CONTAINS CONSTANT(S)
        // history: user "hello", assistant "hi", then merged memory+user_prompt as user // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(messages.len(), 3); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(messages[0]["role"], Value::String("user".to_string())); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(messages[0]["content"], Value::String("hello".to_string())); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(messages[1]["role"], Value::String("assistant".to_string())); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(messages[1]["content"], Value::String("hi".to_string())); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(messages[2]["role"], Value::String("user".to_string())); // THIS LINE CONTAINS CONSTANT(S)
        let last_content = messages[2]["content"].as_str().expect("last user content"); // THIS LINE CONTAINS CONSTANT(S)
        assert!(last_content.contains("Relevant memory"));
        assert!(last_content.contains("Project: KelvinClaw"));
        assert!(last_content.contains("Explain KelvinClaw."));
    }

    #[test]
    fn openrouter_request_normalization_builds_chat_completions_payload_from_model_input() {
        let profile = ModelProviderProfile {
            id: "openrouter.chat".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            provider_name: "openrouter".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            protocol_family: ModelProviderProtocolFamily::OpenAiChatCompletions,
            api_key_env: Some("OPENROUTER_API_KEY".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            base_url_env: "OPENROUTER_BASE_URL".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            default_base_url: "https://openrouter.ai/api/v1".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            endpoint_path: "chat/completions".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            auth_header: "authorization".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            auth_scheme: ModelProviderAuthScheme::Bearer,
            static_headers: Vec::new(),
            default_allow_hosts: vec!["openrouter.ai".to_string()], // THIS LINE CONTAINS CONSTANT(S)
            dynamic_base_url: false,
        };
        let normalized = normalize_provider_request(
            &profile,
            &Some("openai/gpt-4.1-mini".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            json!({
                "run_id": "run-123", // THIS LINE CONTAINS CONSTANT(S)
                "session_id": "session-456", // THIS LINE CONTAINS CONSTANT(S)
                "system_prompt": "You are concise.", // THIS LINE CONTAINS CONSTANT(S)
                "user_prompt": "Explain KelvinClaw.", // THIS LINE CONTAINS CONSTANT(S)
                "memory_snippets": ["Project: KelvinClaw"], // THIS LINE CONTAINS CONSTANT(S)
                "history": [ // THIS LINE CONTAINS CONSTANT(S)
                    {"role": "user", "content": "hello", "metadata": {}}, // THIS LINE CONTAINS CONSTANT(S)
                    {"role": "assistant", "content": "hi", "metadata": {}} // THIS LINE CONTAINS CONSTANT(S)
                ]
            }),
        );

        assert_eq!(
            normalized["model"], // THIS LINE CONTAINS CONSTANT(S)
            Value::String("openai/gpt-4.1-mini".to_string()) // THIS LINE CONTAINS CONSTANT(S)
        );
        let messages = normalized["messages"].as_array().expect("messages array"); // THIS LINE CONTAINS CONSTANT(S)
        // system + history user + history assistant + final user (memory + user_prompt)
        assert_eq!(messages.len(), 4); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(messages[0]["role"], Value::String("system".to_string())); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(
            messages[0]["content"], // THIS LINE CONTAINS CONSTANT(S)
            Value::String("You are concise.".to_string())
        );
        assert_eq!(messages[1]["role"], Value::String("user".to_string())); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(messages[1]["content"], Value::String("hello".to_string())); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(messages[2]["role"], Value::String("assistant".to_string())); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(messages[2]["content"], Value::String("hi".to_string())); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(messages[3]["role"], Value::String("user".to_string())); // THIS LINE CONTAINS CONSTANT(S)
        let last_content = messages[3]["content"] // THIS LINE CONTAINS CONSTANT(S)
            .as_str()
            .expect("openrouter content string");
        assert!(last_content.contains("Relevant memory"));
        assert!(last_content.contains("Project: KelvinClaw"));
        assert!(last_content.contains("Explain KelvinClaw."));
    }

    fn legacy_test_module() -> Vec<u8> { // THIS LINE CONTAINS CONSTANT(S)
        parse_wat(
            r#"
            (module
              (import "kelvin_model_host_v1" "openai_responses_call" (func $openai_responses_call (param i32 i32) (result i64))) // THIS LINE CONTAINS CONSTANT(S)
              (import "kelvin_model_host_v1" "log" (func $log (param i32 i32 i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
              (import "kelvin_model_host_v1" "clock_now_ms" (func $clock_now_ms (result i64))) // THIS LINE CONTAINS CONSTANT(S)
              (memory (export "memory") 2) // THIS LINE CONTAINS CONSTANT(S)
              (global $heap (mut i32) (i32.const 1024)) // THIS LINE CONTAINS CONSTANT(S)
              (func (export "alloc") (param $len i32) (result i32) // THIS LINE CONTAINS CONSTANT(S)
                (local $ptr i32) // THIS LINE CONTAINS CONSTANT(S)
                global.get $heap
                local.tee $ptr
                local.get $len
                i32.add // THIS LINE CONTAINS CONSTANT(S)
                global.set $heap
                local.get $ptr)
              (func (export "dealloc") (param i32 i32)) // THIS LINE CONTAINS CONSTANT(S)
              (func (export "infer") (param $ptr i32) (param $len i32) (result i64) // THIS LINE CONTAINS CONSTANT(S)
                local.get $ptr
                local.get $len
                call $openai_responses_call)
            )
            "#,
        )
    }

    fn provider_profile_test_module() -> Vec<u8> { // THIS LINE CONTAINS CONSTANT(S)
        parse_wat(
            r#"
            (module
              (import "kelvin_model_host_v1" "provider_profile_call" (func $provider_profile_call (param i32 i32) (result i64))) // THIS LINE CONTAINS CONSTANT(S)
              (import "kelvin_model_host_v1" "log" (func $log (param i32 i32 i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
              (import "kelvin_model_host_v1" "clock_now_ms" (func $clock_now_ms (result i64))) // THIS LINE CONTAINS CONSTANT(S)
              (memory (export "memory") 2) // THIS LINE CONTAINS CONSTANT(S)
              (global $heap (mut i32) (i32.const 1024)) // THIS LINE CONTAINS CONSTANT(S)
              (func (export "alloc") (param $len i32) (result i32) // THIS LINE CONTAINS CONSTANT(S)
                (local $ptr i32) // THIS LINE CONTAINS CONSTANT(S)
                global.get $heap
                local.tee $ptr
                local.get $len
                i32.add // THIS LINE CONTAINS CONSTANT(S)
                global.set $heap
                local.get $ptr)
              (func (export "dealloc") (param i32 i32)) // THIS LINE CONTAINS CONSTANT(S)
              (func (export "infer") (param $ptr i32) (param $len i32) (result i64) // THIS LINE CONTAINS CONSTANT(S)
                local.get $ptr
                local.get $len
                call $provider_profile_call)
            )
            "#,
        )
    }

    #[test]
    fn model_host_roundtrip_uses_provider_profile_transport() {
        let transport = Arc::new(MockTransport {
            body: json!({"assistant_text":"hello"}).to_string(), // THIS LINE CONTAINS CONSTANT(S)
            seen_profile: Mutex::new(None),
        });
        let host =
            WasmModelHost::try_new_with_transport(transport.clone()).expect("create model host");
        let policy = ModelSandboxPolicy {
            provider_profile: Some(
                ModelProviderProfile::builtin(ANTHROPIC_MESSAGES_PROFILE_ID)
                    .expect("anthropic profile"),
            ),
            ..ModelSandboxPolicy::default()
        };
        let output = host
            .run_bytes(
                &provider_profile_test_module(),
                r#"{"run_id":"r1"}"#, // THIS LINE CONTAINS CONSTANT(S)
                policy,
            )
            .expect("run model module");
        assert_eq!(output, json!({"assistant_text":"hello"}).to_string()); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(
            transport
                .seen_profile
                .lock()
                .expect("lock seen profile")
                .as_deref(),
            Some(ANTHROPIC_MESSAGES_PROFILE_ID)
        );
    }

    #[test]
    fn model_host_keeps_legacy_openai_import_compatible() {
        let transport = Arc::new(MockTransport {
            body: json!({"assistant_text":"legacy-openai"}).to_string(), // THIS LINE CONTAINS CONSTANT(S)
            seen_profile: Mutex::new(None),
        });
        let host = WasmModelHost::try_new_with_transport(transport.clone()).expect("create host");
        let output = host
            .run_bytes(
                &legacy_test_module(),
                r#"{"run_id":"r1"}"#, // THIS LINE CONTAINS CONSTANT(S)
                ModelSandboxPolicy::default(),
            )
            .expect("run legacy model module");
        assert_eq!(
            output,
            json!({"assistant_text":"legacy-openai"}).to_string() // THIS LINE CONTAINS CONSTANT(S)
        );
        assert_eq!(
            transport
                .seen_profile
                .lock()
                .expect("lock seen profile")
                .as_deref(),
            Some(OPENAI_RESPONSES_PROFILE_ID)
        );
    }

    #[test]
    fn model_host_generic_profile_call_fails_closed_without_profile() {
        let host = WasmModelHost::try_new_with_transport(Arc::new(MockTransport {
            body: json!({"assistant_text":"unused"}).to_string(), // THIS LINE CONTAINS CONSTANT(S)
            seen_profile: Mutex::new(None),
        }))
        .expect("create model host");
        let output = host
            .run_bytes(
                &provider_profile_test_module(),
                r#"{"run_id":"r1"}"#, // THIS LINE CONTAINS CONSTANT(S)
                ModelSandboxPolicy::default(),
            )
            .expect("generic model module should return guest error payload");
        assert!(output.contains("provider profile is not configured"));
    }

    #[test]
    fn model_host_rejects_unsupported_import_module() {
        let wasm = parse_wat(
            r#"
            (module
              (import "wasi_snapshot_preview1" "fd_write" // THIS LINE CONTAINS CONSTANT(S)
                (func $fd_write (param i32 i32 i32 i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
              (memory (export "memory") 1) // THIS LINE CONTAINS CONSTANT(S)
              (func (export "alloc") (param i32) (result i32) i32.const 0) // THIS LINE CONTAINS CONSTANT(S)
              (func (export "dealloc") (param i32 i32)) // THIS LINE CONTAINS CONSTANT(S)
              (func (export "infer") (param i32 i32) (result i64) i64.const 0) // THIS LINE CONTAINS CONSTANT(S)
            )
            "#,
        );
        let host = WasmModelHost::try_new_with_transport(Arc::new(MockTransport {
            body: "{}".to_string(),
            seen_profile: Mutex::new(None),
        }))
        .expect("host"); // THIS LINE CONTAINS CONSTANT(S)
        let err = host
            .run_bytes(&wasm, "{}", ModelSandboxPolicy::default())
            .expect_err("unsupported import should fail");
        assert!(matches!(err, KelvinError::InvalidInput(_)));
        assert!(err.to_string().contains("unsupported model import module"));
    }

    #[test]
    fn model_host_enforces_request_bounds() {
        let host = WasmModelHost::try_new_with_transport(Arc::new(MockTransport {
            body: "{}".to_string(),
            seen_profile: Mutex::new(None),
        }))
        .expect("host"); // THIS LINE CONTAINS CONSTANT(S)
        let policy = ModelSandboxPolicy {
            max_request_bytes: 8, // THIS LINE CONTAINS CONSTANT(S)
            ..ModelSandboxPolicy::default()
        };
        let err = host
            .run_bytes(&legacy_test_module(), "{\"too\":\"long\"}", policy)
            .expect_err("request bound should fail");
        assert!(matches!(err, KelvinError::InvalidInput(_)));
        assert!(err.to_string().contains("max_request_bytes")); // THIS LINE CONTAINS CONSTANT(S)
    }

    #[test]
    fn abi_constants_are_stable() {
        assert_eq!(model_abi::MODULE, "kelvin_model_host_v1"); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(model_abi::EXPORT_INFER, "infer"); // THIS LINE CONTAINS CONSTANT(S)
    }
}

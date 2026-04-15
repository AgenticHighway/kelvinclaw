use std::fmt::Display;
use std::path::Path;
use std::process::Command;

use kelvin_core::{KelvinError, KelvinResult};
use wasmtime::{Caller, Config, Engine, Linker, Module, Store};

pub mod consts;

pub mod model_host;
pub use model_host::{
    model_abi, EnvOpenAiResponsesTransport, EnvProviderProfileTransport, ModelSandboxPolicy,
    OpenAiResponsesTransport, WasmModelHost,
};

pub mod channel_host;
pub use channel_host::{channel_abi, ChannelSandboxPolicy, WasmChannelHost};

pub mod claw_abi {
    use crate::consts;

    pub const ABI_VERSION: &str = consts::CLAW_ABI_VERSION;
    pub const MODULE: &str = consts::CLAW_MODULE;
    pub const RUN_EXPORT: &str = consts::CLAW_RUN_EXPORT;
    pub const SEND_MESSAGE: &str = consts::CLAW_SEND_MESSAGE;
    pub const MOVE_SERVO: &str = consts::CLAW_MOVE_SERVO;
    pub const FS_READ: &str = consts::CLAW_FS_READ;
    pub const NETWORK_SEND: &str = consts::CLAW_NETWORK_SEND;
    // v2 shared-memory ABI exports
    pub const EXPORT_MEMORY: &str = consts::CLAW_EXPORT_MEMORY;
    pub const EXPORT_ALLOC: &str = consts::CLAW_EXPORT_ALLOC;
    pub const EXPORT_DEALLOC: &str = consts::CLAW_EXPORT_DEALLOC;
    pub const HANDLE_TOOL_CALL: &str = consts::CLAW_HANDLE_TOOL_CALL;
    // optional log import (always accepted)
    pub const IMPORT_LOG: &str = consts::CLAW_IMPORT_LOG;
    // real HTTP call: request/response JSON through shared memory
    pub const HTTP_CALL: &str = consts::CLAW_HTTP_CALL;
    // read an env var from the host (gated by env_allow in sandbox policy)
    pub const GET_ENV: &str = consts::CLAW_GET_ENV;
    // execute a shell command (gated by shell_allow_commands in sandbox policy)
    pub const SHELL_EXEC: &str = consts::CLAW_SHELL_EXEC;
}

pub const DEFAULT_MAX_MODULE_BYTES: usize = consts::DEFAULT_MAX_MODULE_BYTES;
pub const DEFAULT_FUEL_BUDGET: u64 = consts::DEFAULT_FUEL_BUDGET;
/// Hard upper bound on fuel_budget to prevent manifests from requesting
/// unbounded execution time (#69).
pub const MAX_FUEL_BUDGET: u64 = consts::MAX_FUEL_BUDGET;
pub const DEFAULT_MAX_REQUEST_BYTES: usize = consts::DEFAULT_MAX_REQUEST_BYTES;
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = consts::DEFAULT_MAX_RESPONSE_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClawCall {
    SendMessage { message_code: i32 },
    MoveServo { channel: i32, position: i32 },
    FsRead { handle: i32 },
    NetworkSend { packet: i32 },
    HttpCall { url: String },
    EnvAccess { key: String },
    ShellExec { command: String, args: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxPreset {
    LockedDown,
    DevLocal,
    HardwareControl,
}

impl SandboxPreset {
    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().to_lowercase().as_str() {
            "locked_down" | "locked-down" | "locked" => Some(Self::LockedDown),
            "dev_local" | "dev-local" | "dev" => Some(Self::DevLocal),
            "hardware_control" | "hardware-control" | "hardware" => Some(Self::HardwareControl),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::LockedDown => "locked_down",
            Self::DevLocal => "dev_local",
            Self::HardwareControl => "hardware_control",
        }
    }

    pub fn policy(self) -> SandboxPolicy {
        match self {
            Self::LockedDown => SandboxPolicy::locked_down(),
            Self::DevLocal => SandboxPolicy {
                allow_fs_read: true,
                ..SandboxPolicy::locked_down()
            },
            Self::HardwareControl => SandboxPolicy {
                allow_move_servo: true,
                ..SandboxPolicy::locked_down()
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub allow_move_servo: bool,
    pub allow_fs_read: bool,
    /// Hostnames allowed for `http_call`. Supports `"*"` (any host) and
    /// `"*.example.com"` (subdomain wildcard). Empty = no HTTP access.
    pub network_allow_hosts: Vec<String>,
    /// Environment variable names the plugin is allowed to read via `get_env`.
    /// Empty = no env access. Names are matched case-sensitively.
    pub env_allow: Vec<String>,
    /// Command basenames the plugin is allowed to execute via `shell_exec`.
    /// Empty = no shell access (fail-closed). Only the basename is checked;
    /// the host resolves the full path via `PATH`. Supports `"*"` as a
    /// wildcard to allow any command (use with extreme caution).
    pub shell_allow_commands: Vec<String>,
    pub max_module_bytes: usize,
    pub fuel_budget: u64,
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            allow_move_servo: false,
            allow_fs_read: false,
            network_allow_hosts: Vec::new(),
            env_allow: Vec::new(),
            shell_allow_commands: Vec::new(),
            max_module_bytes: DEFAULT_MAX_MODULE_BYTES,
            fuel_budget: DEFAULT_FUEL_BUDGET,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }
}

impl SandboxPolicy {
    pub fn locked_down() -> Self {
        Self::default()
    }

    pub fn allow_all() -> Self {
        Self {
            allow_move_servo: true,
            allow_fs_read: true,
            network_allow_hosts: vec!["*".to_string()],
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillExecution {
    pub exit_code: i32,
    pub calls: Vec<ClawCall>,
    /// Populated for v2 modules that export `handle_tool_call`; `None` for v1 modules.
    pub output_json: Option<String>,
}

#[derive(Debug, Default)]
struct HostState {
    calls: Vec<ClawCall>,
}

impl HostState {
    fn record(&mut self, call: ClawCall) {
        self.calls.push(call);
    }
}

#[derive(Clone)]
pub struct WasmSkillHost {
    engine: Engine,
}

impl Default for WasmSkillHost {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmSkillHost {
    pub fn new() -> Self {
        Self::try_new().expect("create wasm skill host engine")
    }

    pub fn try_new() -> KelvinResult<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|err| backend("create engine", err))?;
        Ok(Self { engine })
    }

    pub fn run_file(
        &self,
        wasm_path: impl AsRef<Path>,
        policy: SandboxPolicy,
    ) -> KelvinResult<SkillExecution> {
        let bytes = std::fs::read(wasm_path).map_err(KelvinError::from)?;
        self.run_bytes(&bytes, policy)
    }

    pub fn run_bytes(
        &self,
        wasm_bytes: &[u8],
        policy: SandboxPolicy,
    ) -> KelvinResult<SkillExecution> {
        if wasm_bytes.len() > policy.max_module_bytes {
            return Err(KelvinError::InvalidInput(format!(
                "wasm module size {} exceeds limit {}",
                wasm_bytes.len(),
                policy.max_module_bytes
            )));
        }

        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|err| backend("compile wasm module", err))?;
        validate_imports(&module, &policy)?;

        let mut store = Store::new(&self.engine, HostState::default());
        store
            .set_fuel(policy.fuel_budget)
            .map_err(|err| backend("set fuel budget", err))?;

        let mut linker = Linker::<HostState>::new(&self.engine);
        link_claw_imports(&mut linker, &policy)?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|err| backend("instantiate module", err))?;
        let run = instance
            .get_typed_func::<(), i32>(&mut store, claw_abi::RUN_EXPORT)
            .map_err(|err| backend("resolve run export", err))?;
        let exit_code = match run.call(&mut store, ()) {
            Ok(code) => code,
            Err(err) => {
                let remaining_fuel = store.get_fuel().ok();
                if matches!(remaining_fuel, Some(0)) {
                    return Err(KelvinError::Timeout(
                        "skill execution exceeded fuel budget".to_string(),
                    ));
                }
                return Err(backend("execute run export", err));
            }
        };

        Ok(SkillExecution {
            exit_code,
            calls: store.data().calls.clone(),
            output_json: None,
        })
    }

    pub fn run_file_with_input(
        &self,
        wasm_path: impl AsRef<Path>,
        input_json: &str,
        policy: SandboxPolicy,
    ) -> KelvinResult<SkillExecution> {
        let bytes = std::fs::read(wasm_path).map_err(KelvinError::from)?;
        self.run_bytes_with_input(&bytes, input_json, policy)
    }

    pub fn run_bytes_with_input(
        &self,
        wasm_bytes: &[u8],
        input_json: &str,
        policy: SandboxPolicy,
    ) -> KelvinResult<SkillExecution> {
        if wasm_bytes.len() > policy.max_module_bytes {
            return Err(KelvinError::InvalidInput(format!(
                "wasm module size {} exceeds limit {}",
                wasm_bytes.len(),
                policy.max_module_bytes
            )));
        }
        if input_json.len() > policy.max_request_bytes {
            return Err(KelvinError::InvalidInput(format!(
                "tool input exceeds max_request_bytes {}",
                policy.max_request_bytes
            )));
        }

        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|err| backend("compile wasm module", err))?;
        validate_imports(&module, &policy)?;

        let has_v2 = module
            .exports()
            .any(|exp| exp.name() == claw_abi::HANDLE_TOOL_CALL);

        let mut store = Store::new(&self.engine, HostState::default());
        store
            .set_fuel(policy.fuel_budget)
            .map_err(|err| backend("set fuel budget", err))?;

        let mut linker = Linker::<HostState>::new(&self.engine);
        link_claw_imports(&mut linker, &policy)?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|err| backend("instantiate module", err))?;

        if has_v2 {
            // --- v2 path: write input JSON into guest memory, call handle_tool_call ---
            let memory = instance
                .get_memory(&mut store, claw_abi::EXPORT_MEMORY)
                .ok_or_else(|| {
                    KelvinError::InvalidInput("v2 tool module must export memory".to_string())
                })?;
            let alloc_fn = instance
                .get_typed_func::<i32, i32>(&mut store, claw_abi::EXPORT_ALLOC)
                .map_err(|err| backend("resolve alloc export", err))?;
            let dealloc_fn = instance
                .get_typed_func::<(i32, i32), ()>(&mut store, claw_abi::EXPORT_DEALLOC)
                .map_err(|err| backend("resolve dealloc export", err))?;
            let handle_fn = instance
                .get_typed_func::<(i32, i32), i64>(&mut store, claw_abi::HANDLE_TOOL_CALL)
                .map_err(|err| backend("resolve handle_tool_call export", err))?;

            let input_bytes = input_json.as_bytes();
            let input_len = i32::try_from(input_bytes.len()).map_err(|_| {
                KelvinError::InvalidInput("tool input exceeded i32 address space".to_string())
            })?;

            let input_ptr = alloc_fn
                .call(&mut store, input_len)
                .map_err(|err| backend("call alloc for tool input", err))?;
            skill_write_guest_bytes(
                &memory,
                &mut store,
                input_ptr,
                input_bytes,
                "write tool input",
            )?;

            let call_result = handle_fn.call(&mut store, (input_ptr, input_len));
            let _ = dealloc_fn.call(&mut store, (input_ptr, input_len));

            let packed = call_result.map_err(|err| {
                if matches!(store.get_fuel(), Ok(0)) {
                    KelvinError::Timeout("tool execution exceeded fuel budget".to_string())
                } else {
                    backend("execute handle_tool_call export", err)
                }
            })?;

            let (output_ptr, output_len) = skill_unpack_ptr_len(packed, "handle_tool_call return")?;
            let output_bytes = skill_read_guest_bytes(
                &memory,
                &mut store,
                output_ptr,
                output_len,
                policy.max_response_bytes,
                "read tool output",
            )?;
            let _ = dealloc_fn.call(&mut store, (output_ptr, output_len));

            let text = String::from_utf8(output_bytes).map_err(|err| {
                KelvinError::InvalidInput(format!("tool output must be utf-8 json: {err}"))
            })?;
            serde_json::from_str::<serde_json::Value>(&text).map_err(|err| {
                KelvinError::InvalidInput(format!("tool output must be json: {err}"))
            })?;

            Ok(SkillExecution {
                exit_code: 0,
                calls: store.data().calls.clone(),
                output_json: Some(text),
            })
        } else {
            // --- v1 fallback: call run() ---
            let run = instance
                .get_typed_func::<(), i32>(&mut store, claw_abi::RUN_EXPORT)
                .map_err(|err| backend("resolve run export", err))?;
            let exit_code = match run.call(&mut store, ()) {
                Ok(code) => code,
                Err(err) => {
                    let remaining_fuel = store.get_fuel().ok();
                    if matches!(remaining_fuel, Some(0)) {
                        return Err(KelvinError::Timeout(
                            "skill execution exceeded fuel budget".to_string(),
                        ));
                    }
                    return Err(backend("execute run export", err));
                }
            };

            Ok(SkillExecution {
                exit_code,
                calls: store.data().calls.clone(),
                output_json: None,
            })
        }
    }
}

fn validate_imports(module: &Module, policy: &SandboxPolicy) -> KelvinResult<()> {
    for import in module.imports() {
        if import.module() != claw_abi::MODULE {
            return Err(KelvinError::InvalidInput(format!(
                "unsupported import module '{}' for ABI {} (expected '{}')",
                import.module(),
                claw_abi::ABI_VERSION,
                claw_abi::MODULE
            )));
        }

        let name = import.name();
        match name {
            claw_abi::SEND_MESSAGE => {}
            claw_abi::IMPORT_LOG => {}
            claw_abi::MOVE_SERVO if policy.allow_move_servo => {}
            claw_abi::FS_READ if policy.allow_fs_read => {}
            claw_abi::NETWORK_SEND if !policy.network_allow_hosts.is_empty() => {}
            claw_abi::HTTP_CALL if !policy.network_allow_hosts.is_empty() => {}
            claw_abi::GET_ENV if !policy.env_allow.is_empty() => {}
            claw_abi::SHELL_EXEC if !policy.shell_allow_commands.is_empty() => {}
            claw_abi::MOVE_SERVO
            | claw_abi::FS_READ
            | claw_abi::NETWORK_SEND
            | claw_abi::HTTP_CALL
            | claw_abi::GET_ENV
            | claw_abi::SHELL_EXEC => {
                return Err(KelvinError::InvalidInput(format!(
                    "capability import '{name}' denied by sandbox policy"
                )));
            }
            _ => {
                return Err(KelvinError::InvalidInput(format!(
                    "unsupported ABI {} import '{}.{}'",
                    claw_abi::ABI_VERSION,
                    import.module(),
                    name
                )));
            }
        }
    }
    Ok(())
}

fn link_claw_imports(linker: &mut Linker<HostState>, policy: &SandboxPolicy) -> KelvinResult<()> {
    linker
        .func_wrap(
            claw_abi::MODULE,
            claw_abi::SEND_MESSAGE,
            |mut caller: Caller<'_, HostState>, message_code: i32| -> i32 {
                caller
                    .data_mut()
                    .record(ClawCall::SendMessage { message_code });
                0
            },
        )
        .map_err(|err| backend("link claw.send_message", err))?;

    linker
        .func_wrap(
            claw_abi::MODULE,
            claw_abi::IMPORT_LOG,
            |_caller: Caller<'_, HostState>, _level: i32, _ptr: i32, _len: i32| -> i32 { 0 },
        )
        .map_err(|err| backend("link claw.log", err))?;

    if policy.allow_move_servo {
        linker
            .func_wrap(
                claw_abi::MODULE,
                claw_abi::MOVE_SERVO,
                |mut caller: Caller<'_, HostState>, channel: i32, position: i32| -> i32 {
                    caller
                        .data_mut()
                        .record(ClawCall::MoveServo { channel, position });
                    0
                },
            )
            .map_err(|err| backend("link claw.move_servo", err))?;
    }

    if policy.allow_fs_read {
        linker
            .func_wrap(
                claw_abi::MODULE,
                claw_abi::FS_READ,
                |mut caller: Caller<'_, HostState>, handle: i32| -> i32 {
                    caller.data_mut().record(ClawCall::FsRead { handle });
                    0
                },
            )
            .map_err(|err| backend("link claw.fs_read", err))?;
    }

    if !policy.network_allow_hosts.is_empty() {
        // Legacy stub — records the call, no real I/O.
        linker
            .func_wrap(
                claw_abi::MODULE,
                claw_abi::NETWORK_SEND,
                |mut caller: Caller<'_, HostState>, packet: i32| -> i32 {
                    caller.data_mut().record(ClawCall::NetworkSend { packet });
                    0
                },
            )
            .map_err(|err| backend("link claw.network_send", err))?;

        // Real HTTP: request/response JSON through guest-provided buffers.
        // Signature: http_call(req_ptr, req_len, resp_ptr, resp_max_len) -> i32
        // Request:   {"url":"...", "method":"GET"|"POST"|..., "body":"..."}
        // Response:  {"status":200, "body":"..."}  written directly to resp_ptr.
        // Returns actual bytes written, or 0 on error.
        // The guest pre-allocates the response buffer — no alloc re-entry needed.
        let allow_hosts = policy.network_allow_hosts.clone();
        linker
            .func_wrap(
                claw_abi::MODULE,
                claw_abi::HTTP_CALL,
                move |mut caller: Caller<'_, HostState>,
                      req_ptr: i32,
                      req_len: i32,
                      resp_ptr: i32,
                      resp_max_len: i32|
                      -> i32 {
                    if req_ptr < 0
                        || req_len <= 0
                        || resp_ptr < 0
                        || resp_max_len <= 0
                        || req_len as usize > DEFAULT_MAX_REQUEST_BYTES
                    {
                        return 0;
                    }

                    // --- read request from guest memory ---
                    let memory = match caller
                        .get_export(claw_abi::EXPORT_MEMORY)
                        .and_then(|e| e.into_memory())
                    {
                        Some(m) => m,
                        None => return 0,
                    };
                    let mut req_bytes = vec![0u8; req_len as usize];
                    if memory.read(&caller, req_ptr as usize, &mut req_bytes).is_err() {
                        return 0;
                    }

                    // --- parse request ---
                    let req: serde_json::Value = match serde_json::from_slice(&req_bytes) {
                        Ok(v) => v,
                        Err(_) => return 0,
                    };
                    let url_str = match req.get("url").and_then(|v| v.as_str()) {
                        Some(u) => u.to_string(),
                        None => return 0,
                    };

                    // --- enforce allowlist ---
                    let hostname = match url::Url::parse(&url_str)
                        .ok()
                        .and_then(|u| u.host_str().map(|h| h.to_string()))
                    {
                        Some(h) if !h.is_empty() => h,
                        _ => {
                            // Reject unparseable or host-less URLs (#71)
                            return write_resp_to_buf(
                                &mut caller,
                                &serde_json::json!({"status": 400, "body": "invalid or missing hostname in URL"}).to_string(),
                                resp_ptr,
                                resp_max_len,
                            );
                        }
                    };
                    caller.data_mut().record(ClawCall::HttpCall { url: url_str.clone() });

                    // --- build response JSON ---
                    let resp_json = if !claw_host_allowed(&hostname, &allow_hosts) {
                        serde_json::json!({
                            "status": 403,
                            "body": "host not allowed by sandbox policy"
                        })
                        .to_string()
                    } else {
                        let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
                        let body =
                            req.get("body").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let client = match reqwest::blocking::Client::builder()
                            .timeout(std::time::Duration::from_secs(consts::HTTP_CALL_TIMEOUT_SECS))
                            .build()
                        {
                            Ok(c) => c,
                            Err(e) => {
                                return write_resp_to_buf(
                                    &mut caller,
                                    &serde_json::json!({"status": 0, "body": format!("client error: {e}")}).to_string(),
                                    resp_ptr,
                                    resp_max_len,
                                );
                            }
                        };
                        let mut req_builder = match method.to_uppercase().as_str() {
                            "POST" => client.post(&url_str).body(body),
                            "PUT" => client.put(&url_str).body(body),
                            "DELETE" => client.delete(&url_str),
                            "PATCH" => client.patch(&url_str).body(body),
                            _ => client.get(&url_str),
                        };
                        // Optional headers object: {"Header-Name": "value", ...}
                        // Block security-sensitive headers to prevent injection (#70).
                        if let Some(hdrs) = req.get("headers").and_then(|v| v.as_object()) {
                            for (k, v) in hdrs {
                                if let Some(val) = v.as_str() {
                                    if !consts::BLOCKED_HEADERS.contains(&k.to_ascii_lowercase().as_str()) {
                                        req_builder = req_builder.header(k.as_str(), val);
                                    }
                                }
                            }
                        }
                        // Wrap blocking HTTP in block_in_place to avoid starving
                        // the tokio runtime (#66).
                        match tokio::task::block_in_place(|| req_builder.send()) {
                            Ok(resp) => {
                                let status = resp.status().as_u16();
                                let text = resp.text().unwrap_or_default();
                                let cap = resp_max_len as usize;
                                // Serialize, then shrink body if JSON exceeds the buffer.
                                // JSON escaping can double raw byte count (e.g. HTML with many '"'),
                                // so we re-encode with a smaller body rather than hard-truncating.
                                let mut json = serde_json::json!({"status": status, "body": &text}).to_string();
                                if json.len() > cap {
                                    let half = cap / 2;
                                    let trunc = format!("{}...[truncated]", &text[..text.len().min(half)]);
                                    json = serde_json::json!({"status": status, "body": trunc}).to_string();
                                    if json.len() > cap {
                                        json = serde_json::json!({"status": status, "body": "[response too large]"}).to_string();
                                    }
                                }
                                json
                            }
                            Err(e) => {
                                serde_json::json!({"status": 0, "body": format!("request error: {e}")})
                                    .to_string()
                            }
                        }
                    };

                    write_resp_to_buf(&mut caller, &resp_json, resp_ptr, resp_max_len)
                },
            )
            .map_err(|err| backend("link claw.http_call", err))?;
    }

    if !policy.env_allow.is_empty() {
        // Read an env var by name. Only vars listed in env_allow are accessible.
        // Signature: get_env(key_ptr, key_len, val_ptr, val_max) -> i32
        // Returns bytes written (0 = not allowed or not set).
        let env_allow = policy.env_allow.clone();
        linker
            .func_wrap(
                claw_abi::MODULE,
                claw_abi::GET_ENV,
                move |mut caller: Caller<'_, HostState>,
                      key_ptr: i32,
                      key_len: i32,
                      val_ptr: i32,
                      val_max: i32|
                      -> i32 {
                    if key_ptr < 0 || key_len <= 0 || val_ptr < 0 || val_max <= 0 {
                        return 0;
                    }
                    let memory = match caller
                        .get_export(claw_abi::EXPORT_MEMORY)
                        .and_then(|e| e.into_memory())
                    {
                        Some(m) => m,
                        None => return 0,
                    };
                    let mut key_bytes = vec![0u8; key_len as usize];
                    if memory
                        .read(&caller, key_ptr as usize, &mut key_bytes)
                        .is_err()
                    {
                        return 0;
                    }
                    let key = match std::str::from_utf8(&key_bytes) {
                        Ok(s) => s,
                        Err(_) => return 0,
                    };
                    if !env_allow.iter().any(|allowed| allowed == key) {
                        return 0;
                    }
                    caller.data_mut().record(ClawCall::EnvAccess {
                        key: key.to_string(),
                    });
                    let value = match std::env::var(key) {
                        Ok(v) => v,
                        Err(_) => return 0,
                    };
                    let val_bytes = value.as_bytes();
                    let write_len = val_bytes.len().min(val_max as usize);
                    if write_len == 0 {
                        return 0;
                    }
                    if memory
                        .write(&mut caller, val_ptr as usize, &val_bytes[..write_len])
                        .is_err()
                    {
                        return 0;
                    }
                    write_len as i32
                },
            )
            .map_err(|err| backend("link claw.get_env", err))?;
    }

    if !policy.shell_allow_commands.is_empty() {
        // Execute a shell command. Only commands whose basename is present in
        // `shell_allow_commands` are permitted. No shell expansion — the command
        // is invoked directly via `std::process::Command`.
        //
        // Signature: shell_exec(req_ptr, req_len, resp_ptr, resp_max_len) -> i32
        //   req_ptr/req_len  – JSON request  {"command":"...","args":["..."],"timeout_secs":10}
        //   resp_ptr/resp_max_len – buffer for JSON response
        //   returns bytes written into resp buffer (0 = error)
        //
        // Response JSON: {"exit_code":0,"stdout":"...","stderr":"..."}
        let allow_cmds = policy.shell_allow_commands.clone();
        let max_req = policy.max_request_bytes;
        linker
            .func_wrap(
                claw_abi::MODULE,
                claw_abi::SHELL_EXEC,
                move |mut caller: Caller<'_, HostState>,
                      req_ptr: i32,
                      req_len: i32,
                      resp_ptr: i32,
                      resp_max_len: i32|
                      -> i32 {
                    if req_ptr < 0 || req_len <= 0 || resp_ptr < 0 || resp_max_len <= 0 {
                        return 0;
                    }
                    let memory = match caller
                        .get_export(claw_abi::EXPORT_MEMORY)
                        .and_then(|e| e.into_memory())
                    {
                        Some(m) => m,
                        None => return 0,
                    };
                    if req_len as usize > max_req {
                        return write_resp_to_buf(
                            &mut caller,
                            &serde_json::json!({"exit_code": -1, "stdout": "", "stderr": "request too large"}).to_string(),
                            resp_ptr,
                            resp_max_len,
                        );
                    }
                    let mut req_bytes = vec![0u8; req_len as usize];
                    if memory
                        .read(&caller, req_ptr as usize, &mut req_bytes)
                        .is_err()
                    {
                        return 0;
                    }
                    let req: serde_json::Value = match serde_json::from_slice(&req_bytes) {
                        Ok(v) => v,
                        Err(_) => {
                            return write_resp_to_buf(
                                &mut caller,
                                &serde_json::json!({"exit_code": -1, "stdout": "", "stderr": "invalid JSON request"}).to_string(),
                                resp_ptr,
                                resp_max_len,
                            );
                        }
                    };

                    let command_raw = match req.get("command").and_then(|v| v.as_str()) {
                        Some(c) => c,
                        None => {
                            return write_resp_to_buf(
                                &mut caller,
                                &serde_json::json!({"exit_code": -1, "stdout": "", "stderr": "missing 'command' field"}).to_string(),
                                resp_ptr,
                                resp_max_len,
                            );
                        }
                    };

                    // Security: extract basename only — reject absolute paths and
                    // path traversals to prevent bypassing the allowlist.
                    let cmd_path = std::path::Path::new(command_raw);
                    let basename = match cmd_path.file_name().and_then(|n| n.to_str()) {
                        Some(b) => b,
                        None => {
                            return write_resp_to_buf(
                                &mut caller,
                                &serde_json::json!({"exit_code": -1, "stdout": "", "stderr": "invalid command name"}).to_string(),
                                resp_ptr,
                                resp_max_len,
                            );
                        }
                    };
                    if cmd_path.components().count() > 1 {
                        return write_resp_to_buf(
                            &mut caller,
                            &serde_json::json!({"exit_code": -1, "stdout": "", "stderr": "absolute paths and path separators are not allowed; use the command basename only"}).to_string(),
                            resp_ptr,
                            resp_max_len,
                        );
                    }

                    // Check against allowlist
                    if !shell_command_allowed(basename, &allow_cmds) {
                        return write_resp_to_buf(
                            &mut caller,
                            &serde_json::json!({"exit_code": -1, "stdout": "", "stderr": "command not allowed by sandbox policy"}).to_string(),
                            resp_ptr,
                            resp_max_len,
                        );
                    }

                    let args: Vec<String> = match req.get("args").and_then(|v| v.as_array()) {
                        Some(arr) => {
                            let mut out = Vec::with_capacity(arr.len());
                            for v in arr {
                                match v.as_str() {
                                    Some(s) => out.push(s.to_string()),
                                    None => {
                                        return write_resp_to_buf(
                                            &mut caller,
                                            &serde_json::json!({"exit_code": -1, "stdout": "", "stderr": "all entries in 'args' must be strings"}).to_string(),
                                            resp_ptr,
                                            resp_max_len,
                                        );
                                    }
                                }
                            }
                            out
                        }
                        None => Vec::new(),
                    };

                    // Interpreter guard: block inline code execution flags
                    // (e.g. `python -c "code"`, `node --eval "code"`) to prevent
                    // plugins from using allowed interpreters for arbitrary execution.
                    if is_interpreter_inline_exec(basename, &args) {
                        return write_resp_to_buf(
                            &mut caller,
                            &serde_json::json!({"exit_code": -1, "stdout": "", "stderr": "inline code execution via interpreter flags is not allowed"}).to_string(),
                            resp_ptr,
                            resp_max_len,
                        );
                    }

                    caller.data_mut().record(ClawCall::ShellExec {
                        command: basename.to_string(),
                        args: args.clone(),
                    });

                    let timeout_secs = req
                        .get("timeout_secs")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(consts::SHELL_EXEC_DEFAULT_TIMEOUT_SECS)
                        .min(consts::SHELL_EXEC_MAX_TIMEOUT_SECS);

                    // Execute the command with no shell expansion and a clean
                    // environment (only PATH is inherited so the command can be
                    // resolved).
                    let result = tokio::task::block_in_place(|| {
                        let mut child = match Command::new(basename)
                            .args(&args)
                            .env_clear()
                            .env("PATH", std::env::var("PATH").unwrap_or_default())
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::piped())
                            .spawn()
                        {
                            Ok(c) => c,
                            Err(e) => {
                                return serde_json::json!({
                                    "exit_code": -1,
                                    "stdout": "",
                                    "stderr": format!("spawn error: {e}")
                                })
                                .to_string();
                            }
                        };

                        // Take stdout/stderr handles and drain them on dedicated
                        // threads to avoid the classic pipe-buffer deadlock: if the
                        // child writes more than the OS pipe buffer (~64 KB) without
                        // anyone reading, its write() blocks and try_wait() would
                        // never see an exit.
                        //
                        // Each reader is capped at SHELL_EXEC_MAX_OUTPUT_BYTES to
                        // prevent a malicious command from exhausting host memory.
                        let stdout_handle = child.stdout.take();
                        let stderr_handle = child.stderr.take();
                        let read_limit = consts::SHELL_EXEC_MAX_OUTPUT_BYTES as u64;

                        let stdout_thread = std::thread::spawn(move || {
                            let mut buf = Vec::new();
                            if let Some(r) = stdout_handle {
                                let _ =
                                    std::io::Read::read_to_end(&mut std::io::Read::take(r, read_limit), &mut buf);
                            }
                            buf
                        });
                        let stderr_thread = std::thread::spawn(move || {
                            let mut buf = Vec::new();
                            if let Some(r) = stderr_handle {
                                let _ =
                                    std::io::Read::read_to_end(&mut std::io::Read::take(r, read_limit), &mut buf);
                            }
                            buf
                        });

                        let timeout = std::time::Duration::from_secs(timeout_secs);
                        let start = std::time::Instant::now();
                        let exit_status = loop {
                            match child.try_wait() {
                                Ok(Some(status)) => break Some(status),
                                Ok(None) => {
                                    if start.elapsed() >= timeout {
                                        let _ = child.kill();
                                        let _ = child.wait();
                                        break None;
                                    }
                                    std::thread::sleep(std::time::Duration::from_millis(50));
                                }
                                Err(e) => {
                                    // Clean up child and reader threads to avoid
                                    // orphaned processes and leaked threads.
                                    let _ = child.kill();
                                    let _ = child.wait();
                                    let _ = stdout_thread.join();
                                    let _ = stderr_thread.join();
                                    return serde_json::json!({
                                        "exit_code": -1,
                                        "stdout": "",
                                        "stderr": format!("wait error: {e}")
                                    })
                                    .to_string();
                                }
                            }
                        };

                        let stdout_bytes = stdout_thread.join().unwrap_or_default();
                        let stderr_bytes = stderr_thread.join().unwrap_or_default();

                        match exit_status {
                            None => {
                                serde_json::json!({
                                    "exit_code": -1,
                                    "stdout": "",
                                    "stderr": "command timed out"
                                })
                                .to_string()
                            }
                            Some(status) => {
                                // Enforce a *combined* output budget across both
                                // streams, splitting evenly when both exceed half.
                                let max_out = consts::SHELL_EXEC_MAX_OUTPUT_BYTES;
                                let stdout_raw = String::from_utf8_lossy(&stdout_bytes);
                                let stderr_raw = String::from_utf8_lossy(&stderr_bytes);

                                let half = max_out / 2;
                                let (stdout_budget, stderr_budget) =
                                    if stdout_raw.len() <= half {
                                        // stdout is small — give stderr the remainder
                                        (stdout_raw.len(), max_out.saturating_sub(stdout_raw.len()))
                                    } else if stderr_raw.len() <= half {
                                        // stderr is small — give stdout the remainder
                                        (max_out.saturating_sub(stderr_raw.len()), stderr_raw.len())
                                    } else {
                                        // both large — split evenly
                                        (half, half)
                                    };

                                let stdout = if stdout_raw.len() > stdout_budget {
                                    let mut end = stdout_budget;
                                    while end > 0 && !stdout_raw.is_char_boundary(end) {
                                        end -= 1;
                                    }
                                    format!("{}...[truncated]", &stdout_raw[..end])
                                } else {
                                    stdout_raw.into_owned()
                                };
                                let stderr = if stderr_raw.len() > stderr_budget {
                                    let mut end = stderr_budget;
                                    while end > 0 && !stderr_raw.is_char_boundary(end) {
                                        end -= 1;
                                    }
                                    format!("{}...[truncated]", &stderr_raw[..end])
                                } else {
                                    stderr_raw.into_owned()
                                };

                                serde_json::json!({
                                    "exit_code": status.code().unwrap_or(-1),
                                    "stdout": stdout,
                                    "stderr": stderr,
                                })
                                .to_string()
                            }
                        }
                    });

                    write_resp_to_buf(&mut caller, &result, resp_ptr, resp_max_len)
                },
            )
            .map_err(|err| backend("link claw.shell_exec", err))?;
    }

    Ok(())
}

fn skill_read_guest_bytes(
    memory: &wasmtime::Memory,
    store: &mut Store<HostState>,
    ptr: i32,
    len: i32,
    max_len: usize,
    context: &str,
) -> KelvinResult<Vec<u8>> {
    if ptr < 0 || len < 0 {
        return Err(KelvinError::InvalidInput(format!(
            "{context}: pointer/length must be non-negative"
        )));
    }
    let len = usize::try_from(len)
        .map_err(|_| KelvinError::InvalidInput(format!("{context}: length conversion overflow")))?;
    if len > max_len {
        return Err(KelvinError::InvalidInput(format!(
            "{context}: payload size {len} exceeds max {max_len}"
        )));
    }
    let mut bytes = vec![0_u8; len];
    memory
        .read(store, ptr as usize, &mut bytes)
        .map_err(|err| {
            KelvinError::InvalidInput(format!("{context}: memory read failed: {err}"))
        })?;
    Ok(bytes)
}

fn skill_write_guest_bytes(
    memory: &wasmtime::Memory,
    store: &mut Store<HostState>,
    ptr: i32,
    bytes: &[u8],
    context: &str,
) -> KelvinResult<()> {
    if ptr < 0 {
        return Err(KelvinError::InvalidInput(format!(
            "{context}: pointer must be non-negative"
        )));
    }
    memory
        .write(store, ptr as usize, bytes)
        .map_err(|err| KelvinError::InvalidInput(format!("{context}: memory write failed: {err}")))
}

fn skill_unpack_ptr_len(value: i64, context: &str) -> KelvinResult<(i32, i32)> {
    if value <= 0 {
        return Err(KelvinError::Backend(format!(
            "{context}: packed pointer/length is invalid"
        )));
    }
    let raw = value as u64;
    let ptr = (raw >> 32) as u32;
    let len = (raw & 0xFFFF_FFFF) as u32;
    if len == 0 {
        return Ok((ptr as i32, 0));
    }
    if ptr == 0 {
        return Err(KelvinError::Backend(format!(
            "{context}: non-empty payload has null pointer"
        )));
    }
    Ok((ptr as i32, len as i32))
}

fn backend(context: &str, err: impl Display) -> KelvinError {
    KelvinError::Backend(format!("{context}: {err}"))
}

/// Checks whether `hostname` is permitted by the `allowed` list.
/// Supports `"*"` (any host) and `"*.example.com"` (subdomain wildcard).
fn claw_host_allowed(hostname: &str, allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return false;
    }
    let host = hostname.trim().to_ascii_lowercase();
    for pattern in allowed {
        let p = pattern.trim().to_ascii_lowercase();
        if p.is_empty() {
            continue;
        }
        if p == "*" {
            return true;
        }
        if let Some(suffix) = p.strip_prefix("*.") {
            if host == suffix || host.ends_with(&format!(".{suffix}")) {
                return true;
            }
            continue;
        }
        if host == p {
            return true;
        }
    }
    false
}

/// Checks whether `command` basename is permitted by the `allowed` list.
/// Supports `"*"` (any command). Matching is case-sensitive.
fn shell_command_allowed(command: &str, allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return false;
    }
    let cmd = command.trim();
    if cmd.is_empty() {
        return false;
    }
    for pattern in allowed {
        let p = pattern.trim();
        if p.is_empty() {
            continue;
        }
        if p == "*" {
            return true;
        }
        if cmd == p {
            return true;
        }
    }
    false
}

/// Returns `true` if `command` is a known script interpreter and any of `args`
/// contains an inline-code execution flag (e.g. `-c`, `-e`, `--eval`).
///
/// This is a deterministic guard that prevents plugins from using allowed
/// interpreters to execute arbitrary code.  Without this, a plugin allowed to
/// run `python` could call `python -c "import os; os.system('...')"`.
///
/// Uses a per-interpreter mapping so that flags with different meanings across
/// interpreters do not cause false positives (e.g. `bash -r` is restricted
/// mode, not inline code; `bash -p` is privileged mode).
///
/// POSIX shells allow combining single-character flags, so `bash -xc 'code'`
/// is equivalent to `bash -x -c 'code'`.  We detect inline-code characters
/// inside any combined short-flag group (an arg matching `-[a-zA-Z]+`).
const INTERPRETER_LONG_FLAGS: &[&str] = &["--eval", "--print", "--command", "-Command"];

fn is_interpreter_inline_exec(command: &str, args: &[String]) -> bool {
    let cmd_lower = command.to_ascii_lowercase();
    // Look up the per-interpreter inline-char set; returns None if not
    // a known interpreter (→ allowed).
    let inline_chars: &[char] = match consts::INTERPRETER_INLINE_MAP
        .iter()
        .find(|(name, _)| cmd_lower == name.to_ascii_lowercase())
    {
        Some((_, chars)) => chars,
        None => return false,
    };

    for arg in args {
        let a = arg.trim();
        // Check long-form flags: case-insensitive exact match or --flag=value.
        // PowerShell accepts -Command, -COMMAND, -command, etc.
        for flag in INTERPRETER_LONG_FLAGS {
            if a.eq_ignore_ascii_case(flag)
                || a.to_ascii_lowercase()
                    .starts_with(&format!("{}=", flag.to_ascii_lowercase()))
            {
                return true;
            }
        }
        // Check short flags — including combined groups like "-xc".
        // Only check the chars relevant to THIS interpreter.
        if !inline_chars.is_empty() {
            if let Some(rest) = a.strip_prefix('-') {
                if !rest.is_empty()
                    && !rest.starts_with('-')
                    && rest.chars().all(|ch| ch.is_ascii_alphabetic())
                {
                    for &flag_char in inline_chars {
                        if rest.contains(flag_char) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Writes `resp_json` into the guest buffer at `resp_ptr` (up to `resp_max_len` bytes).
/// Returns actual bytes written, or 0 on error.
fn write_resp_to_buf(
    caller: &mut Caller<'_, HostState>,
    resp_json: &str,
    resp_ptr: i32,
    resp_max_len: i32,
) -> i32 {
    let resp_bytes = resp_json.as_bytes();
    let write_len = resp_bytes.len().min(resp_max_len as usize);
    if write_len == 0 {
        return 0;
    }
    let memory = match caller
        .get_export(claw_abi::EXPORT_MEMORY)
        .and_then(|e| e.into_memory())
    {
        Some(m) => m,
        None => return 0,
    };
    if memory
        .write(&mut *caller, resp_ptr as usize, &resp_bytes[..write_len])
        .is_err()
    {
        return 0;
    }
    write_len as i32
}

#[cfg(test)]
mod tests {
    use kelvin_core::KelvinError;

    use super::{ClawCall, SandboxPolicy, SandboxPreset, WasmSkillHost};

    fn parse_wat(input: &str) -> Vec<u8> {
        wat::parse_str(input).expect("parse wat")
    }

    #[test]
    fn preset_policies_match_expected_capabilities() {
        assert_eq!(
            SandboxPreset::LockedDown.policy(),
            SandboxPolicy::locked_down()
        );
        assert!(SandboxPreset::DevLocal.policy().allow_fs_read);
        assert!(SandboxPreset::DevLocal
            .policy()
            .network_allow_hosts
            .is_empty());
        assert!(SandboxPreset::HardwareControl.policy().allow_move_servo);
        assert!(!SandboxPreset::HardwareControl.policy().allow_fs_read);
    }

    #[test]
    fn runs_skill_with_allowed_claw_call() {
        let wasm = parse_wat(
            r#"
            (module
              (import "claw" "send_message" (func $send_message (param i32) (result i32)))
              (func (export "run") (result i32)
                i32.const 7
                call $send_message
                drop
                i32.const 0
              )
            )
            "#,
        );

        let host = WasmSkillHost::try_new().expect("host");
        let result = host
            .run_bytes(&wasm, SandboxPolicy::locked_down())
            .expect("run allowed skill");
        assert_eq!(result.exit_code, 0);
        assert_eq!(
            result.calls,
            vec![ClawCall::SendMessage { message_code: 7 }]
        );
    }

    #[test]
    fn rejects_skill_when_policy_blocks_fs_call() {
        let wasm = parse_wat(
            r#"
            (module
              (import "claw" "fs_read" (func $fs_read (param i32) (result i32)))
              (func (export "run") (result i32)
                i32.const 1
                call $fs_read
              )
            )
            "#,
        );

        let host = WasmSkillHost::try_new().expect("host");
        let err = host
            .run_bytes(&wasm, SandboxPolicy::locked_down())
            .expect_err("policy should reject fs import");
        assert!(matches!(err, KelvinError::InvalidInput(_)));
        assert!(err.to_string().contains("denied by sandbox policy"));
    }

    #[test]
    fn allows_skill_when_policy_enables_fs_call() {
        let wasm = parse_wat(
            r#"
            (module
              (import "claw" "fs_read" (func $fs_read (param i32) (result i32)))
              (func (export "run") (result i32)
                i32.const 9
                call $fs_read
              )
            )
            "#,
        );

        let host = WasmSkillHost::try_new().expect("host");
        let result = host
            .run_bytes(
                &wasm,
                SandboxPolicy {
                    allow_fs_read: true,
                    ..SandboxPolicy::locked_down()
                },
            )
            .expect("run allowed fs skill");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.calls, vec![ClawCall::FsRead { handle: 9 }]);
    }

    #[test]
    fn rejects_skill_that_requests_wasi_imports() {
        let wasm = parse_wat(
            r#"
            (module
              (import "wasi_snapshot_preview1" "fd_write"
                (func $fd_write (param i32 i32 i32 i32) (result i32)))
              (func (export "run") (result i32)
                i32.const 0
              )
            )
            "#,
        );

        let host = WasmSkillHost::try_new().expect("host");
        let err = host
            .run_bytes(&wasm, SandboxPolicy::allow_all())
            .expect_err("wasi import should be blocked");
        assert!(matches!(err, KelvinError::InvalidInput(_)));
        assert!(err.to_string().contains("unsupported import module"));
    }

    #[test]
    fn rejects_unknown_abi_import() {
        let wasm = parse_wat(
            r#"
            (module
              (import "claw" "exfiltrate" (func $exfiltrate (param i32) (result i32)))
              (func (export "run") (result i32)
                i32.const 0
                call $exfiltrate
              )
            )
            "#,
        );

        let host = WasmSkillHost::try_new().expect("host");
        let err = host
            .run_bytes(&wasm, SandboxPolicy::allow_all())
            .expect_err("unknown import should be rejected");
        assert!(matches!(err, KelvinError::InvalidInput(_)));
        assert!(err.to_string().contains("unsupported ABI"));
    }

    #[test]
    fn rejects_oversized_module_before_compile() {
        let host = WasmSkillHost::try_new().expect("host");
        let policy = SandboxPolicy {
            max_module_bytes: 8,
            ..SandboxPolicy::locked_down()
        };
        let err = host
            .run_bytes(&[0_u8; 9], policy)
            .expect_err("oversized module should fail");
        assert!(matches!(err, KelvinError::InvalidInput(_)));
        assert!(err.to_string().contains("exceeds limit"));
    }

    #[test]
    fn times_out_on_fuel_exhaustion() {
        let wasm = parse_wat(
            r#"
            (module
              (func (export "run") (result i32)
                (loop
                  br 0
                )
                i32.const 0
              )
            )
            "#,
        );

        let host = WasmSkillHost::try_new().expect("host");
        let err = host
            .run_bytes(
                &wasm,
                SandboxPolicy {
                    fuel_budget: 500,
                    ..SandboxPolicy::locked_down()
                },
            )
            .expect_err("fuel exhaustion expected");
        assert!(matches!(err, KelvinError::Timeout(_)));
    }

    // --- v2 handle_tool_call tests ---

    fn echo_v2_wat() -> Vec<u8> {
        // A minimal WAT echo module: handle_tool_call reads input from memory and
        // returns it unchanged via a bump-allocated output buffer.
        parse_wat(
            r#"
            (module
              (memory (export "memory") 2)
              (global $next_off (mut i32) (i32.const 1024))
              (func $alloc (export "alloc") (param $len i32) (result i32)
                (local $ptr i32)
                (local.set $ptr (global.get $next_off))
                (global.set $next_off (i32.add (global.get $next_off) (local.get $len)))
                (local.get $ptr)
              )
              (func (export "dealloc") (param i32 i32))
              (func (export "handle_tool_call") (param $ptr i32) (param $len i32) (result i64)
                (local $out_ptr i32)
                (local.set $out_ptr (call $alloc (local.get $len)))
                (memory.copy
                  (local.get $out_ptr)
                  (local.get $ptr)
                  (local.get $len)
                )
                (i64.or
                  (i64.shl (i64.extend_i32_u (local.get $out_ptr)) (i64.const 32))
                  (i64.extend_i32_u (local.get $len))
                )
              )
              (func (export "run") (result i32) i32.const 0)
            )
            "#,
        )
    }

    #[test]
    fn v2_echo_roundtrip() {
        let wasm = echo_v2_wat();
        let host = WasmSkillHost::try_new().expect("host");
        let input = r#"{"message":"hello world"}"#;
        let result = host
            .run_bytes_with_input(&wasm, input, SandboxPolicy::locked_down())
            .expect("v2 echo roundtrip");
        assert_eq!(result.output_json.as_deref(), Some(input));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn v1_fallback_when_no_handle_tool_call() {
        let wasm = parse_wat(
            r#"
            (module
              (import "claw" "send_message" (func $send_message (param i32) (result i32)))
              (func (export "run") (result i32)
                i32.const 42
                call $send_message
                drop
                i32.const 0
              )
            )
            "#,
        );
        let host = WasmSkillHost::try_new().expect("host");
        let result = host
            .run_bytes_with_input(&wasm, r#"{"x":1}"#, SandboxPolicy::locked_down())
            .expect("v1 fallback");
        assert_eq!(result.output_json, None);
        assert_eq!(result.exit_code, 0);
        assert_eq!(
            result.calls,
            vec![ClawCall::SendMessage { message_code: 42 }]
        );
    }

    #[test]
    fn v2_fuel_exhaustion_in_handle_tool_call() {
        let wasm = parse_wat(
            r#"
            (module
              (memory (export "memory") 1)
              (func (export "alloc") (param i32) (result i32) i32.const 0)
              (func (export "dealloc") (param i32 i32))
              (func (export "handle_tool_call") (param i32 i32) (result i64)
                (loop (br 0))
                i64.const 0
              )
              (func (export "run") (result i32) i32.const 0)
            )
            "#,
        );
        let host = WasmSkillHost::try_new().expect("host");
        let err = host
            .run_bytes_with_input(
                &wasm,
                r#"{}"#,
                SandboxPolicy {
                    fuel_budget: 500,
                    ..SandboxPolicy::locked_down()
                },
            )
            .expect_err("fuel exhaustion in v2");
        assert!(matches!(err, KelvinError::Timeout(_)));
    }

    #[test]
    fn v2_non_json_output_rejected() {
        // Module returns "not json" (valid utf-8 but not JSON)
        let wasm = parse_wat(
            r#"
            (module
              (memory (export "memory") 1)
              ;; Store the string "not json" at address 100
              (data (i32.const 100) "not json")
              (func (export "alloc") (param i32) (result i32) i32.const 0)
              (func (export "dealloc") (param i32 i32))
              (func (export "handle_tool_call") (param i32 i32) (result i64)
                ;; return ptr=100, len=8  =>  (100 << 32) | 8
                (i64.or
                  (i64.shl (i64.const 100) (i64.const 32))
                  (i64.const 8)
                )
              )
              (func (export "run") (result i32) i32.const 0)
            )
            "#,
        );
        let host = WasmSkillHost::try_new().expect("host");
        let err = host
            .run_bytes_with_input(&wasm, r#"{}"#, SandboxPolicy::locked_down())
            .expect_err("non-json output should fail");
        assert!(matches!(err, KelvinError::InvalidInput(_)));
        assert!(err.to_string().contains("json"));
    }

    #[test]
    fn v2_oversized_output_rejected() {
        // Module claims a 1-byte response but max_response_bytes is 0
        let wasm = parse_wat(
            r#"
            (module
              (memory (export "memory") 1)
              (data (i32.const 100) "{}")
              (func (export "alloc") (param i32) (result i32) i32.const 0)
              (func (export "dealloc") (param i32 i32))
              (func (export "handle_tool_call") (param i32 i32) (result i64)
                (i64.or
                  (i64.shl (i64.const 100) (i64.const 32))
                  (i64.const 2)
                )
              )
              (func (export "run") (result i32) i32.const 0)
            )
            "#,
        );
        let host = WasmSkillHost::try_new().expect("host");
        let err = host
            .run_bytes_with_input(
                &wasm,
                r#"{}"#,
                SandboxPolicy {
                    max_response_bytes: 1,
                    ..SandboxPolicy::locked_down()
                },
            )
            .expect_err("oversized response should fail");
        assert!(matches!(err, KelvinError::InvalidInput(_)));
        assert!(err.to_string().contains("exceeds max"));
    }

    #[test]
    fn v2_accepts_log_import() {
        let wasm = parse_wat(
            r#"
            (module
              (import "claw" "log" (func $log (param i32 i32 i32) (result i32)))
              (memory (export "memory") 2)
              (global $next_off (mut i32) (i32.const 1024))
              (func $alloc (export "alloc") (param $len i32) (result i32)
                (local $ptr i32)
                (local.set $ptr (global.get $next_off))
                (global.set $next_off (i32.add (global.get $next_off) (local.get $len)))
                (local.get $ptr)
              )
              (func (export "dealloc") (param i32 i32))
              (func (export "handle_tool_call") (param $ptr i32) (param $len i32) (result i64)
                (local $out_ptr i32)
                (local.set $out_ptr (call $alloc (local.get $len)))
                (memory.copy (local.get $out_ptr) (local.get $ptr) (local.get $len))
                (i64.or
                  (i64.shl (i64.extend_i32_u (local.get $out_ptr)) (i64.const 32))
                  (i64.extend_i32_u (local.get $len))
                )
              )
              (func (export "run") (result i32) i32.const 0)
            )
            "#,
        );
        let host = WasmSkillHost::try_new().expect("host");
        let input = r#"{"msg":"hi"}"#;
        let result = host
            .run_bytes_with_input(&wasm, input, SandboxPolicy::locked_down())
            .expect("v2 with log import");
        assert_eq!(result.output_json.as_deref(), Some(input));
    }

    #[test]
    fn v2_input_oversized_rejected() {
        let wasm = echo_v2_wat();
        let host = WasmSkillHost::try_new().expect("host");
        let err = host
            .run_bytes_with_input(
                &wasm,
                r#"{"x":1}"#,
                SandboxPolicy {
                    max_request_bytes: 3,
                    ..SandboxPolicy::locked_down()
                },
            )
            .expect_err("oversized input should fail");
        assert!(matches!(err, KelvinError::InvalidInput(_)));
        assert!(err.to_string().contains("max_request_bytes"));
    }

    // --- http_call tests ---

    /// WAT module that imports claw.http_call (4-arg ABI), calls it with a hardcoded
    /// URL, and returns the response from handle_tool_call.
    ///
    /// Memory layout:
    ///   offset 200  — request JSON (req_len bytes)
    ///   offset 4096 — response buffer (resp_max bytes)
    ///   offset 8192 — bump allocator base (for tool-call input written by host)
    fn http_call_wat(url: &str, method: &str) -> Vec<u8> {
        let req_json = format!(r#"{{"url":"{url}","method":"{method}"}}"#);
        let req_len = req_json.len();
        // Reserve generous response buffer so large bodies get truncated, not dropped.
        let resp_max: usize = 65536; // 64 KiB
        let req_data = format!("(data (i32.const 200) {req_json:?})");
        parse_wat(&format!(
            r#"
            (module
              (import "claw" "http_call" (func $http_call (param i32 i32 i32 i32) (result i32)))
              (memory (export "memory") 4)
              {req_data}
              (global $bump (mut i32) (i32.const 8192))
              (func $alloc (export "alloc") (param $n i32) (result i32)
                (local $p i32)
                (local.set $p (global.get $bump))
                (global.set $bump (i32.add (global.get $bump) (local.get $n)))
                (local.get $p)
              )
              (func (export "dealloc") (param i32 i32))
              (func (export "handle_tool_call") (param i32 i32) (result i64)
                (local $resp_len i32)
                (local.set $resp_len
                  (call $http_call
                    (i32.const 200) (i32.const {req_len})
                    (i32.const 4096) (i32.const {resp_max})
                  )
                )
                (i64.or
                  (i64.shl (i64.const 4096) (i64.const 32))
                  (i64.extend_i32_u (local.get $resp_len))
                )
              )
              (func (export "run") (result i32) i32.const 0)
            )
            "#
        ))
    }

    #[test]
    fn http_call_import_denied_when_no_hosts_allowed() {
        // A module that imports claw.http_call must be rejected if network_allow_hosts is empty.
        let wasm = http_call_wat("https://example.com", "GET");
        let host = WasmSkillHost::try_new().expect("host");
        let err = host
            .run_bytes_with_input(&wasm, r#"{}"#, SandboxPolicy::locked_down())
            .expect_err("http_call import should be denied with empty allow list");
        assert!(matches!(err, KelvinError::InvalidInput(_)));
        assert!(err.to_string().contains("denied by sandbox policy"));
    }

    #[test]
    fn http_call_specific_host_allowed_passes_validation() {
        // validate_imports accepts http_call when network_allow_hosts is non-empty.
        let wasm = http_call_wat("https://example.com", "GET");
        let host = WasmSkillHost::try_new().expect("host");
        let module = wasmtime::Module::new(&host.engine, &wasm).expect("compile");
        let policy = SandboxPolicy {
            network_allow_hosts: vec!["example.com".to_string()],
            ..SandboxPolicy::locked_down()
        };
        super::validate_imports(&module, &policy).expect("should pass with matching host");
    }

    #[test]
    fn http_call_wildcard_policy_passes_validation() {
        let wasm = http_call_wat("https://api.github.com/zen", "GET");
        let host = WasmSkillHost::try_new().expect("host");
        let module = wasmtime::Module::new(&host.engine, &wasm).expect("compile");
        let policy = SandboxPolicy {
            network_allow_hosts: vec!["*".to_string()],
            ..SandboxPolicy::locked_down()
        };
        super::validate_imports(&module, &policy).expect("wildcard should pass validation");
    }

    #[test]
    fn http_call_blocked_host_returns_403_in_response() {
        // Module calls http_call targeting "blocked.internal" which is not in the allow list.
        // The host writes a 403 JSON response into guest memory without making any network call.
        let wasm = http_call_wat("https://blocked.internal/data", "GET");
        let host = WasmSkillHost::try_new().expect("host");
        let policy = SandboxPolicy {
            network_allow_hosts: vec!["allowed.example.com".to_string()],
            ..SandboxPolicy::locked_down()
        };
        let result = host
            .run_bytes_with_input(&wasm, r#"{}"#, policy)
            .expect("blocked host should return 403 response, not trap");
        let output = result.output_json.expect("output_json should be set");
        let v: serde_json::Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(v["status"], 403);
        assert!(v["body"].as_str().unwrap_or("").contains("not allowed"));
    }

    #[test]
    fn http_call_subdomain_wildcard_allows_matching_host() {
        // "*.github.com" should allow "api.github.com" but not "github.com" or "evil.com".
        use super::claw_host_allowed;
        let pattern = vec!["*.github.com".to_string()];
        assert!(claw_host_allowed("api.github.com", &pattern));
        assert!(claw_host_allowed("raw.github.com", &pattern));
        // apex is also covered (consistent with model_host.rs host_allowed)
        assert!(claw_host_allowed("github.com", &pattern));
        assert!(!claw_host_allowed("evil.com", &pattern));
        assert!(!claw_host_allowed("notgithub.com", &pattern));
    }

    #[test]
    fn http_call_wildcard_star_allows_any_host() {
        use super::claw_host_allowed;
        let pattern = vec!["*".to_string()];
        assert!(claw_host_allowed("github.com", &pattern));
        assert!(claw_host_allowed("api.openai.com", &pattern));
        assert!(claw_host_allowed("localhost", &pattern));
    }

    /// Hits a real network endpoint — skipped in CI, run manually with:
    ///   cargo test -p kelvin-wasm http_call_real_github -- --ignored
    #[test]
    #[ignore]
    fn http_call_real_github() {
        let wasm = http_call_wat("https://github.com/", "GET");
        let host = WasmSkillHost::try_new().expect("host");
        let policy = SandboxPolicy {
            network_allow_hosts: vec!["github.com".to_string()],
            ..SandboxPolicy::locked_down()
        };
        let result = host
            .run_bytes_with_input(&wasm, r#"{}"#, policy)
            .expect("request should succeed — large body is truncated");
        let output = result.output_json.expect("output_json");
        let v: serde_json::Value = serde_json::from_str(&output).expect("valid json");
        // GitHub may redirect (301) or return 200; just verify a valid HTTP status came back
        let status = v["status"].as_u64().unwrap_or(0);
        assert!(
            status > 0 && status < 600,
            "expected valid HTTP status, got {status}"
        );
        assert!(!v["body"].as_str().unwrap_or("").is_empty());
    }

    // --- shell_exec tests ---

    #[test]
    fn rejects_shell_exec_import_when_policy_blocks_it() {
        let wasm = parse_wat(
            r#"
            (module
              (import "claw" "shell_exec" (func $shell_exec (param i32 i32 i32 i32) (result i32)))
              (func (export "run") (result i32)
                i32.const 0
              )
            )
            "#,
        );

        let host = WasmSkillHost::try_new().expect("host");
        let err = host
            .run_bytes(&wasm, SandboxPolicy::locked_down())
            .expect_err("policy should reject shell_exec import");
        assert!(matches!(err, KelvinError::InvalidInput(_)));
        assert!(err.to_string().contains("denied by sandbox policy"));
    }

    #[test]
    fn shell_command_allowed_checks() {
        use super::shell_command_allowed;
        let allow = vec!["echo".to_string(), "ls".to_string()];
        assert!(shell_command_allowed("echo", &allow));
        assert!(shell_command_allowed("ls", &allow));
        assert!(!shell_command_allowed("rm", &allow));
        assert!(!shell_command_allowed("", &allow));

        let wildcard = vec!["*".to_string()];
        assert!(shell_command_allowed("anything", &wildcard));

        let empty: Vec<String> = vec![];
        assert!(!shell_command_allowed("echo", &empty));
    }

    #[test]
    fn shell_exec_runs_allowed_command() {
        // Build a v2 WAT module that calls shell_exec with {"command":"echo","args":["hello"]}
        // and returns the response through handle_tool_call.
        let wasm = parse_wat(
            r#"
            (module
              (import "claw" "shell_exec" (func $shell_exec (param i32 i32 i32 i32) (result i32)))
              (memory (export "memory") 2)
              (data (i32.const 0) "{\"command\":\"echo\",\"args\":[\"hello\"]}")
              (global $bump (mut i32) (i32.const 4096))
              (func $alloc (export "alloc") (param $len i32) (result i32)
                (local $ptr i32)
                (local.set $ptr (global.get $bump))
                (global.set $bump (i32.add (global.get $bump) (local.get $len)))
                (local.get $ptr)
              )
              (func (export "dealloc") (param i32 i32))
              (func (export "handle_tool_call") (param $in_ptr i32) (param $in_len i32) (result i64)
                (local $written i32)
                ;; call shell_exec: req at 0..35, resp buffer at 8192 max 8192
                (local.set $written
                  (call $shell_exec (i32.const 0) (i32.const 35) (i32.const 8192) (i32.const 8192))
                )
                ;; return (resp_ptr << 32) | written
                (i64.or
                  (i64.shl (i64.extend_i32_u (i32.const 8192)) (i64.const 32))
                  (i64.extend_i32_u (local.get $written))
                )
              )
              (func (export "run") (result i32) i32.const 0)
            )
            "#,
        );

        let host = WasmSkillHost::try_new().expect("host");
        let policy = SandboxPolicy {
            shell_allow_commands: vec!["echo".to_string()],
            ..SandboxPolicy::locked_down()
        };
        let result = host
            .run_bytes_with_input(&wasm, r#"{}"#, policy)
            .expect("shell_exec should succeed");
        let output = result.output_json.expect("output_json should be set");
        let v: serde_json::Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(v["exit_code"], 0);
        assert!(v["stdout"].as_str().unwrap_or("").contains("hello"));

        // Verify the ClawCall was recorded
        assert_eq!(result.calls.len(), 1);
        assert_eq!(
            result.calls[0],
            ClawCall::ShellExec {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
            }
        );
    }

    #[test]
    fn shell_exec_rejects_disallowed_command() {
        let wasm = parse_wat(
            r#"
            (module
              (import "claw" "shell_exec" (func $shell_exec (param i32 i32 i32 i32) (result i32)))
              (memory (export "memory") 2)
              (data (i32.const 0) "{\"command\":\"rm\",\"args\":[\"-rf\",\"/\"]}")
              (global $bump (mut i32) (i32.const 4096))
              (func $alloc (export "alloc") (param $len i32) (result i32)
                (local $ptr i32)
                (local.set $ptr (global.get $bump))
                (global.set $bump (i32.add (global.get $bump) (local.get $len)))
                (local.get $ptr)
              )
              (func (export "dealloc") (param i32 i32))
              (func (export "handle_tool_call") (param $in_ptr i32) (param $in_len i32) (result i64)
                (local $written i32)
                (local.set $written
                  (call $shell_exec (i32.const 0) (i32.const 35) (i32.const 8192) (i32.const 8192))
                )
                (i64.or
                  (i64.shl (i64.extend_i32_u (i32.const 8192)) (i64.const 32))
                  (i64.extend_i32_u (local.get $written))
                )
              )
              (func (export "run") (result i32) i32.const 0)
            )
            "#,
        );

        let host = WasmSkillHost::try_new().expect("host");
        let policy = SandboxPolicy {
            shell_allow_commands: vec!["echo".to_string(), "ls".to_string()],
            ..SandboxPolicy::locked_down()
        };
        let result = host
            .run_bytes_with_input(&wasm, r#"{}"#, policy)
            .expect("should not trap, just return denial in response");
        let output = result.output_json.expect("output_json");
        let v: serde_json::Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(v["exit_code"], -1);
        assert!(v["stderr"].as_str().unwrap_or("").contains("not allowed"));
    }

    #[test]
    fn shell_exec_rejects_absolute_path_command() {
        let wasm = parse_wat(
            r#"
            (module
              (import "claw" "shell_exec" (func $shell_exec (param i32 i32 i32 i32) (result i32)))
              (memory (export "memory") 2)
              (data (i32.const 0) "{\"command\":\"/bin/echo\",\"args\":[\"hello\"]}")
              (global $bump (mut i32) (i32.const 4096))
              (func $alloc (export "alloc") (param $len i32) (result i32)
                (local $ptr i32)
                (local.set $ptr (global.get $bump))
                (global.set $bump (i32.add (global.get $bump) (local.get $len)))
                (local.get $ptr)
              )
              (func (export "dealloc") (param i32 i32))
              (func (export "handle_tool_call") (param $in_ptr i32) (param $in_len i32) (result i64)
                (local $written i32)
                (local.set $written
                  (call $shell_exec (i32.const 0) (i32.const 40) (i32.const 8192) (i32.const 8192))
                )
                (i64.or
                  (i64.shl (i64.extend_i32_u (i32.const 8192)) (i64.const 32))
                  (i64.extend_i32_u (local.get $written))
                )
              )
              (func (export "run") (result i32) i32.const 0)
            )
            "#,
        );

        let host = WasmSkillHost::try_new().expect("host");
        let policy = SandboxPolicy {
            shell_allow_commands: vec!["echo".to_string()],
            ..SandboxPolicy::locked_down()
        };
        let result = host
            .run_bytes_with_input(&wasm, r#"{}"#, policy)
            .expect("should not trap");
        let output = result.output_json.expect("output_json");
        let v: serde_json::Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(v["exit_code"], -1);
        assert!(v["stderr"]
            .as_str()
            .unwrap_or("")
            .contains("absolute paths"));
    }

    #[test]
    fn interpreter_inline_exec_checks() {
        use super::is_interpreter_inline_exec;

        // Known interpreter with inline flag → blocked
        assert!(is_interpreter_inline_exec(
            "python",
            &["-c".into(), "import os".into()]
        ));
        assert!(is_interpreter_inline_exec(
            "python3",
            &["-c".into(), "print('hi')".into()]
        ));
        assert!(is_interpreter_inline_exec(
            "node",
            &["--eval".into(), "console.log(1)".into()]
        ));
        assert!(is_interpreter_inline_exec(
            "bash",
            &["-c".into(), "echo hi".into()]
        ));
        assert!(is_interpreter_inline_exec(
            "ruby",
            &["-e".into(), "puts 1".into()]
        ));
        assert!(is_interpreter_inline_exec(
            "perl",
            &["-E".into(), "say 1".into()]
        ));
        assert!(is_interpreter_inline_exec(
            "php",
            &["-r".into(), "echo 1;".into()]
        ));
        // --eval=code style
        assert!(is_interpreter_inline_exec(
            "node",
            &["--eval=console.log(1)".into()]
        ));

        // Known interpreter WITHOUT inline flag → allowed
        assert!(!is_interpreter_inline_exec("python", &["script.py".into()]));
        assert!(!is_interpreter_inline_exec(
            "node",
            &["app.js".into(), "--port".into(), "3000".into()]
        ));

        // Non-interpreter with same flags → allowed (not our concern)
        assert!(!is_interpreter_inline_exec(
            "echo",
            &["-c".into(), "hello".into()]
        ));
        assert!(!is_interpreter_inline_exec(
            "grep",
            &["-e".into(), "pattern".into()]
        ));

        // Case-insensitive interpreter matching
        assert!(is_interpreter_inline_exec(
            "Python3",
            &["-c".into(), "code".into()]
        ));

        // Empty args → allowed
        assert!(!is_interpreter_inline_exec("python", &[]));

        // Combined POSIX short flags — e.g. bash -xc 'code' is
        // equivalent to bash -x -c 'code' and must be blocked.
        assert!(is_interpreter_inline_exec(
            "bash",
            &["-xc".into(), "echo pwned".into()]
        ));
        assert!(is_interpreter_inline_exec(
            "sh",
            &["-lc".into(), "id".into()]
        ));
        assert!(is_interpreter_inline_exec(
            "zsh",
            &["-ic".into(), "whoami".into()]
        ));
        // Combined flags with -e (ruby/perl/node)
        assert!(is_interpreter_inline_exec(
            "ruby",
            &["-we".into(), "puts 1".into()]
        ));
        // Combined flags without any inline-code char → allowed
        assert!(!is_interpreter_inline_exec(
            "bash",
            &["-x".into(), "script.sh".into()]
        ));
        assert!(!is_interpreter_inline_exec(
            "python",
            &["-u".into(), "script.py".into()]
        ));

        // Case-insensitive long flags — PowerShell accepts -Command,
        // -COMMAND, -command, etc.
        assert!(is_interpreter_inline_exec(
            "powershell",
            &["-COMMAND".into(), "Get-Process".into()]
        ));
        assert!(is_interpreter_inline_exec(
            "pwsh",
            &["--COMMAND".into(), "code".into()]
        ));
        assert!(is_interpreter_inline_exec(
            "powershell",
            &["-command".into(), "code".into()]
        ));
        assert!(is_interpreter_inline_exec(
            "node",
            &["--EVAL=console.log(1)".into()]
        ));

        // Per-interpreter mapping: flags that mean something else for other
        // interpreters must NOT be falsely blocked.
        // bash -r = restricted mode (not inline code)
        assert!(!is_interpreter_inline_exec(
            "bash",
            &["-r".into(), "script.sh".into()]
        ));
        // bash -p = privileged mode (not inline code)
        assert!(!is_interpreter_inline_exec(
            "bash",
            &["-p".into(), "script.sh".into()]
        ));
        // node -r = --require (preload module, not inline code)
        assert!(!is_interpreter_inline_exec(
            "node",
            &["-r".into(), "module".into(), "app.js".into()]
        ));
        // perl -p = auto-print loop (not inline code without -e)
        assert!(!is_interpreter_inline_exec(
            "perl",
            &["-p".into(), "script.pl".into()]
        ));
        // php -r IS inline code and must still be blocked
        assert!(is_interpreter_inline_exec(
            "php",
            &["-r".into(), "echo 1;".into()]
        ));
    }

    #[test]
    fn shell_exec_rejects_interpreter_inline_code() {
        let wasm = parse_wat(
            r#"
            (module
              (import "claw" "shell_exec" (func $shell_exec (param i32 i32 i32 i32) (result i32)))
              (memory (export "memory") 2)
              (data (i32.const 0) "{\"command\":\"python3\",\"args\":[\"-c\",\"import os\"]}")
              (global $bump (mut i32) (i32.const 4096))
              (func $alloc (export "alloc") (param $len i32) (result i32)
                (local $ptr i32)
                (local.set $ptr (global.get $bump))
                (global.set $bump (i32.add (global.get $bump) (local.get $len)))
                (local.get $ptr)
              )
              (func (export "dealloc") (param i32 i32))
              (func (export "handle_tool_call") (param $in_ptr i32) (param $in_len i32) (result i64)
                (local $written i32)
                (local.set $written
                  (call $shell_exec (i32.const 0) (i32.const 47) (i32.const 8192) (i32.const 8192))
                )
                (i64.or
                  (i64.shl (i64.extend_i32_u (i32.const 8192)) (i64.const 32))
                  (i64.extend_i32_u (local.get $written))
                )
              )
            )
            "#,
        );
        let host = WasmSkillHost::try_new().expect("host");
        let policy = SandboxPolicy {
            shell_allow_commands: vec!["python3".into()],
            ..SandboxPolicy::locked_down()
        };
        let result = host
            .run_bytes_with_input(&wasm, r#"{}"#, policy)
            .expect("should not trap");
        let output = result.output_json.expect("output_json");
        let v: serde_json::Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(v["exit_code"], -1);
        assert!(v["stderr"]
            .as_str()
            .unwrap_or("")
            .contains("inline code execution"));
    }
}

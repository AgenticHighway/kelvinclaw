use std::fmt::Display;
use std::path::Path;

use kelvin_core::{KelvinError, KelvinResult};
use wasmtime::{Caller, Config, Engine, Linker, Module, Store};

pub mod model_host;
pub use model_host::{
    model_abi, EnvOpenAiResponsesTransport, EnvProviderProfileTransport, ModelSandboxPolicy,
    OpenAiResponsesTransport, WasmModelHost,
};

pub mod channel_host;
pub use channel_host::{channel_abi, ChannelSandboxPolicy, WasmChannelHost};

pub mod claw_abi {
    pub const ABI_VERSION: &str = "1.0.0";
    pub const MODULE: &str = "claw";
    pub const RUN_EXPORT: &str = "run";
    pub const SEND_MESSAGE: &str = "send_message";
    pub const MOVE_SERVO: &str = "move_servo";
    pub const FS_READ: &str = "fs_read";
    pub const NETWORK_SEND: &str = "network_send";
    // v2 shared-memory ABI exports
    pub const EXPORT_MEMORY: &str = "memory";
    pub const EXPORT_ALLOC: &str = "alloc";
    pub const EXPORT_DEALLOC: &str = "dealloc";
    pub const HANDLE_TOOL_CALL: &str = "handle_tool_call";
    // optional log import (always accepted)
    pub const IMPORT_LOG: &str = "log";
}

pub const DEFAULT_MAX_MODULE_BYTES: usize = 512 * 1024;
pub const DEFAULT_FUEL_BUDGET: u64 = 1_000_000;
pub const DEFAULT_MAX_REQUEST_BYTES: usize = 256 * 1024;
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClawCall {
    SendMessage { message_code: i32 },
    MoveServo { channel: i32, position: i32 },
    FsRead { handle: i32 },
    NetworkSend { packet: i32 },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub allow_move_servo: bool,
    pub allow_fs_read: bool,
    pub allow_network_send: bool,
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
            allow_network_send: false,
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
            allow_network_send: true,
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
        validate_imports(&module, policy)?;

        let mut store = Store::new(&self.engine, HostState::default());
        store
            .set_fuel(policy.fuel_budget)
            .map_err(|err| backend("set fuel budget", err))?;

        let mut linker = Linker::<HostState>::new(&self.engine);
        link_claw_imports(&mut linker, policy)?;

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
        validate_imports(&module, policy)?;

        let has_v2 = module
            .exports()
            .any(|exp| exp.name() == claw_abi::HANDLE_TOOL_CALL);

        let mut store = Store::new(&self.engine, HostState::default());
        store
            .set_fuel(policy.fuel_budget)
            .map_err(|err| backend("set fuel budget", err))?;

        let mut linker = Linker::<HostState>::new(&self.engine);
        link_claw_imports(&mut linker, policy)?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|err| backend("instantiate module", err))?;

        if has_v2 {
            // --- v2 path: write input JSON into guest memory, call handle_tool_call ---
            let memory = instance
                .get_memory(&mut store, claw_abi::EXPORT_MEMORY)
                .ok_or_else(|| {
                    KelvinError::InvalidInput(
                        "v2 tool module must export memory".to_string(),
                    )
                })?;
            let alloc_fn = instance
                .get_typed_func::<i32, i32>(&mut store, claw_abi::EXPORT_ALLOC)
                .map_err(|err| backend("resolve alloc export", err))?;
            let dealloc_fn = instance
                .get_typed_func::<(i32, i32), ()>(&mut store, claw_abi::EXPORT_DEALLOC)
                .map_err(|err| backend("resolve dealloc export", err))?;
            let handle_fn = instance
                .get_typed_func::<(i32, i32), i64>(
                    &mut store,
                    claw_abi::HANDLE_TOOL_CALL,
                )
                .map_err(|err| backend("resolve handle_tool_call export", err))?;

            let input_bytes = input_json.as_bytes();
            let input_len = i32::try_from(input_bytes.len()).map_err(|_| {
                KelvinError::InvalidInput(
                    "tool input exceeded i32 address space".to_string(),
                )
            })?;

            let input_ptr = alloc_fn
                .call(&mut store, input_len)
                .map_err(|err| backend("call alloc for tool input", err))?;
            skill_write_guest_bytes(&memory, &mut store, input_ptr, input_bytes, "write tool input")?;

            let call_result = handle_fn.call(&mut store, (input_ptr, input_len));
            let _ = dealloc_fn.call(&mut store, (input_ptr, input_len));

            let packed = call_result.map_err(|err| {
                if matches!(store.get_fuel(), Ok(0)) {
                    KelvinError::Timeout("tool execution exceeded fuel budget".to_string())
                } else {
                    backend("execute handle_tool_call export", err)
                }
            })?;

            let (output_ptr, output_len) =
                skill_unpack_ptr_len(packed, "handle_tool_call return")?;
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

fn validate_imports(module: &Module, policy: SandboxPolicy) -> KelvinResult<()> {
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
            claw_abi::NETWORK_SEND if policy.allow_network_send => {}
            claw_abi::MOVE_SERVO | claw_abi::FS_READ | claw_abi::NETWORK_SEND => {
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

fn link_claw_imports(linker: &mut Linker<HostState>, policy: SandboxPolicy) -> KelvinResult<()> {
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

    if policy.allow_network_send {
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
        .read(store, usize::try_from(ptr).unwrap_or_default(), &mut bytes)
        .map_err(|err| KelvinError::InvalidInput(format!("{context}: memory read failed: {err}")))?;
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
        .write(store, usize::try_from(ptr).unwrap_or_default(), bytes)
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
        assert!(!SandboxPreset::DevLocal.policy().allow_network_send);
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
        assert_eq!(result.calls, vec![ClawCall::SendMessage { message_code: 42 }]);
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
}

use std::fmt::Display;
use std::path::Path;

use kelvin_core::{KelvinError, KelvinResult};
use wasmtime::{Caller, Engine, Linker, Module, Store};

pub mod claw_abi {
    pub const MODULE: &str = "claw";
    pub const RUN_EXPORT: &str = "run";
    pub const SEND_MESSAGE: &str = "send_message";
    pub const MOVE_SERVO: &str = "move_servo";
    pub const FS_READ: &str = "fs_read";
    pub const NETWORK_SEND: &str = "network_send";
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClawCall {
    SendMessage { message_code: i32 },
    MoveServo { channel: i32, position: i32 },
    FsRead { handle: i32 },
    NetworkSend { packet: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SandboxPolicy {
    pub allow_move_servo: bool,
    pub allow_fs_read: bool,
    pub allow_network_send: bool,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillExecution {
    pub exit_code: i32,
    pub calls: Vec<ClawCall>,
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
        Self {
            engine: Engine::default(),
        }
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
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|err| backend("compile wasm module", err))?;
        let mut store = Store::new(&self.engine, HostState::default());
        let mut linker = Linker::<HostState>::new(&self.engine);

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

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|err| backend("instantiate module", err))?;
        let run = instance
            .get_typed_func::<(), i32>(&mut store, claw_abi::RUN_EXPORT)
            .map_err(|err| backend("resolve run export", err))?;
        let exit_code = run
            .call(&mut store, ())
            .map_err(|err| backend("execute run export", err))?;

        Ok(SkillExecution {
            exit_code,
            calls: store.data().calls.clone(),
        })
    }
}

fn backend(context: &str, err: impl Display) -> KelvinError {
    KelvinError::Backend(format!("{context}: {err}"))
}

#[cfg(test)]
mod tests {
    use kelvin_core::KelvinError;

    use super::{ClawCall, SandboxPolicy, WasmSkillHost};

    fn parse_wat(input: &str) -> Vec<u8> {
        wat::parse_str(input).expect("parse wat")
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

        let host = WasmSkillHost::new();
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

        let host = WasmSkillHost::new();
        let err = host
            .run_bytes(&wasm, SandboxPolicy::locked_down())
            .expect_err("policy should reject fs import");
        assert!(matches!(err, KelvinError::Backend(_)));
        assert!(err.to_string().contains("instantiate module"));
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

        let host = WasmSkillHost::new();
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

        let host = WasmSkillHost::new();
        let err = host
            .run_bytes(&wasm, SandboxPolicy::allow_all())
            .expect_err("wasi import should be blocked without wasi bindings");
        assert!(matches!(err, KelvinError::Backend(_)));
        assert!(err.to_string().contains("instantiate module"));
    }
}

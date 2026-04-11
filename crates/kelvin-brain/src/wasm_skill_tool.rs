use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use kelvin_core::{
    KelvinError, KelvinResult, PluginCapability, PluginFactory, PluginManifest, Tool,
    ToolCallInput, ToolCallResult, KELVIN_CORE_API_VERSION,
};
use kelvin_wasm::{ClawCall, SandboxPolicy, SandboxPreset, WasmSkillHost};

use crate::consts;

/// ### Brief
///
/// engine-side wrapper for `WasmSkillHost`. handles tool execution and tracking
///
/// ### Description
///
/// this wrapper is used to execute WASM skills in a sandboxed environment. it also
/// tracks execution calls and logs results in workspace memory files
///
/// ### Fields
/// * `name` - tool name
/// * `host` - pointer to the `WasmSkillHost` for this wasm skill tool
/// * `default_policy` - fallback policy if
/// * `default_memory_append_path` - location to append execution tracking info
#[derive(Clone)]
pub struct WasmSkillTool {
    name: String,
    host: Arc<WasmSkillHost>,
    default_policy: SandboxPolicy,
    default_memory_append_path: String,
}

/// ### Brief
///
/// main implementation of `WasmSkillTool`
impl WasmSkillTool {
    pub fn new(
        name: impl Into<String>,
        host: Arc<WasmSkillHost>,
        default_policy: SandboxPolicy,
    ) -> Self {
        Self {
            name: name.into(),
            host,
            default_policy,
            default_memory_append_path: consts::DEFAULT_MEMORY_APPEND_PATH.to_string(),
        }
    }

    /// ### Brief
    ///
    /// helper for loading json value to serde_json `Map`; errors if `value` is empty or not a json object
    ///
    /// ### Arguments
    /// * `value` - serde_json value to convert
    ///
    /// ### Returns
    /// a serde_json `Map` type from subvalues in `value`
    ///
    /// ### Errors
    /// - JSON parse error
    fn require_args_object<'a>(
        &self,
        value: &'a Value,
    ) -> KelvinResult<&'a serde_json::Map<String, Value>> {
        value.as_object().ok_or_else(|| {
            KelvinError::InvalidInput(format!("{} tool expects JSON object arguments", self.name))
        })
    }

    /// ### Brief
    ///
    /// helper for loading a required string value by key from a serde_json `Map`
    ///
    /// ### Arguments
    /// * `args` - value map
    /// * `key` - which string value to retrieve
    ///
    /// ### Returns
    /// value from map as a `&str`
    ///
    /// ### Errors
    /// - JSON parse error if value is not a string
    /// - JSON parse error if value doesn't exist
    fn require_string(
        &self,
        args: &serde_json::Map<String, Value>,
        key: &str,
    ) -> KelvinResult<String> {
        let value = args.get(key).ok_or_else(|| {
            KelvinError::InvalidInput(format!("{} tool requires '{key}' argument", self.name))
        })?;
        value.as_str().map(str::to_string).ok_or_else(|| {
            KelvinError::InvalidInput(format!(
                "{} tool argument '{key}' must be a string",
                self.name
            ))
        })
    }

    /// ### Brief
    ///
    /// helper for loading an optional string value by key from a serde_json `Map`
    ///
    /// ### Arguments
    /// * `args` - value map
    /// * `key` - which string value to retrieve
    ///
    /// ### Returns
    /// optional value from map as a `&str`
    ///
    /// ### Errors
    /// - JSON parse error if value is not a string
    fn optional_string(
        &self,
        args: &serde_json::Map<String, Value>,
        key: &str,
    ) -> KelvinResult<Option<String>> {
        match args.get(key) {
            None => Ok(None),
            Some(value) => value.as_str().map(|v| Some(v.to_string())).ok_or_else(|| {
                KelvinError::InvalidInput(format!(
                    "{} tool argument '{key}' must be a string",
                    self.name
                ))
            }),
        }
    }

    /// ### Brief
    ///
    /// helper for loading an optional bool value by key from a serde_json `Map`
    ///
    /// ### Arguments
    /// * `args` - value map
    /// * `key` - which bool value to retrieve
    ///
    /// ### Returns
    /// optional value from map as a boolean
    ///
    /// ### Errors
    /// - JSON parse error if value is not a bool
    fn optional_bool(
        &self,
        args: &serde_json::Map<String, Value>,
        key: &str,
    ) -> KelvinResult<Option<bool>> {
        match args.get(key) {
            None => Ok(None),
            Some(value) => value
                .as_bool()
                .map(Some)
                .ok_or_else(|| KelvinError::InvalidInput(format!("'{key}' must be a boolean"))),
        }
    }

    /// ### Brief
    ///
    /// helper for loading an array of string values by key from a serde_json `Map`
    ///
    /// ### Arguments
    /// * `args` - value map
    /// * `key` - which array value to retrieve
    ///
    /// ### Returns
    /// optional vector of string values
    ///
    /// ### Errors
    /// - JSON parse error if value is not an array of strings
    fn optional_string_array(
        &self,
        args: &serde_json::Map<String, Value>,
        key: &str,
    ) -> KelvinResult<Option<Vec<String>>> {
        match args.get(key) {
            None => Ok(None),
            Some(Value::Array(arr)) => {
                let mut result = Vec::with_capacity(arr.len());
                for item in arr {
                    match item.as_str() {
                        Some(s) => result.push(s.to_string()),
                        None => {
                            return Err(KelvinError::InvalidInput(format!(
                                "'{key}' must be an array of strings"
                            )))
                        }
                    }
                }
                Ok(Some(result))
            }
            _ => Err(KelvinError::InvalidInput(format!(
                "'{key}' must be an array of strings"
            ))),
        }
    }

    /// ### Brief
    ///
    /// helper for loading an optional u64 value by key from a serde_json `Map`
    ///
    /// ### Arguments
    /// * `args` - value map
    /// * `key` - which u64 value to retrieve
    ///
    /// ### Returns
    /// optional value from map as a u64
    ///
    /// ### Errors
    /// - JSON parse error if value is not a u64
    fn optional_u64(
        &self,
        args: &serde_json::Map<String, Value>,
        key: &str,
    ) -> KelvinResult<Option<u64>> {
        match args.get(key) {
            None => Ok(None),
            Some(value) => value
                .as_u64()
                .map(Some)
                .ok_or_else(|| KelvinError::InvalidInput(format!("'{key}' must be a u64"))),
        }
    }

    /// ### Brief
    ///
    /// helper for loading an optional usize value by key from a serde_json `Map`
    ///
    /// ### Arguments
    /// * `args` - value map
    /// * `key` - which usize value to retrieve
    ///
    /// ### Returns
    /// optional value from map as a usize
    ///
    /// ### Errors
    /// - JSON parse error if value is not a usize
    fn optional_usize(
        &self,
        args: &serde_json::Map<String, Value>,
        key: &str,
    ) -> KelvinResult<Option<usize>> {
        match args.get(key) {
            None => Ok(None),
            Some(value) => {
                let Some(raw) = value.as_u64() else {
                    return Err(KelvinError::InvalidInput(format!(
                        "'{key}' must be a usize"
                    )));
                };
                usize::try_from(raw)
                    .map(Some)
                    .map_err(|_| KelvinError::InvalidInput(format!("'{key}' exceeds usize")))
            }
        }
    }

    /// ### Brief
    ///
    /// sanitizes a workspace path by checking that its relative and checking for traversals
    ///
    /// ### Description
    ///
    /// Optional longer description explaining the purpose, behavior, and any important details.
    ///
    /// ### Arguments
    /// * `raw` - path string
    /// * `field` - name of associated field
    ///
    /// ### Returns
    /// sanitized path as an owned String
    ///
    /// ### Errors
    /// - path is empty
    /// - path is absolute
    /// - path contains traversals
    ///
    /// ### Example
    /// ```ignore
    /// use kelvin_brain::wasm_skill_tool::WasmSkillTool;
    ///
    /// let wasm_skill_tool = WasmSkillTool::default();
    ///
    /// assert!(wasm_skill_tool.sanitize_rel_path("this/is/good", "test_dir").is_ok());
    /// assert!(wasm_skill_tool.sanitize_rel_path("", "test_dir").is_err());
    /// assert!(wasm_skill_tool.sanitize_rel_path("/home/username/this/is/bad", "test_dir").is_err());
    /// assert!(wasm_skill_tool.sanitize_rel_path("../this/is/bad", "test_dir").is_err());
    /// ```
    fn sanitize_rel_path(&self, raw: &str, field: &str) -> KelvinResult<String> {
        let normalized = raw.trim().replace('\\', "/");
        if normalized.is_empty() {
            return Err(KelvinError::InvalidInput(format!(
                "'{field}' must not be empty"
            )));
        }
        if Path::new(&normalized).is_absolute() || normalized.starts_with('/') {
            return Err(KelvinError::InvalidInput(format!(
                "'{field}' must be a relative path"
            )));
        }
        let path = Path::new(&normalized);
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(KelvinError::InvalidInput(format!(
                "'{field}' path traversal is not allowed"
            )));
        }
        Ok(normalized)
    }

    /// ### Brief
    ///
    /// validates that a memory path is `MEMORY.md` or `memory/*.md`
    ///
    /// ### Arguments
    /// * `memory_rel_path` - path to memory relative to workspace
    ///
    /// ### Returns
    /// none
    ///
    /// ### Errors
    /// - path is not a valid memory location
    /// - path doesnt resolve to a markdown file
    fn validate_memory_path_scope(&self, memory_rel_path: &str) -> KelvinResult<()> {
        let is_memory_root = memory_rel_path == consts::MEMORY_ROOT_FILE;
        let is_memory_daily =
            memory_rel_path.starts_with(consts::MEMORY_PREFIX) && memory_rel_path.ends_with(".md");
        if !is_memory_root && !is_memory_daily {
            return Err(KelvinError::InvalidInput(
                "memory append path must be MEMORY.md or memory/*.md".to_string(),
            ));
        }
        Ok(())
    }

    /// ### Brief
    ///
    /// resolves fields in args to `SandboxPolicy`
    ///
    /// ### Description
    ///
    /// extracts 5 fields from the args json map:
    ///
    /// - `allow_move_servo` - bool for whether to allow moving a servo
    /// - `allow_fs_read` - bool for whether to allow reading files in general
    /// - `network_allow_hosts` - string array of allowed hosts for connection requests
    /// - `max_module_bytes` - maximum size in bytes of the WASM module
    /// - `fuel_budget` - maximum allowed computation for the WASM module
    ///
    /// uses default values from `SandboxPolicy` if not present in args
    ///
    /// ### Arguments
    /// * `args` - value map
    /// * `default_policy` - fallback `SandboxPolicy`; used if args doesn't specify a policy
    ///
    /// ### Returns
    /// the resolved sandbox policy
    ///
    /// ### Errors
    /// - unknown policy_preset in args. valid options are "locked_down", "dev_local", or "hardware_control"
    fn resolve_policy(
        &self,
        args: &serde_json::Map<String, Value>,
        default_policy: SandboxPolicy,
    ) -> KelvinResult<SandboxPolicy> {
        let mut policy = if let Some(raw) =
            self.optional_string(args, consts::FIELD_POLICY_PRESET)?
        {
            SandboxPreset::parse(&raw)
                .ok_or_else(|| KelvinError::InvalidInput(format!("unknown policy preset: {raw}")))?
                .policy()
        } else {
            default_policy
        };

        if let Some(value) = self.optional_bool(args, consts::FIELD_ALLOW_MOVE_SERVO)? {
            policy.allow_move_servo = value;
        }
        if let Some(value) = self.optional_bool(args, consts::FIELD_ALLOW_FS_READ)? {
            policy.allow_fs_read = value;
        }
        if let Some(hosts) = self.optional_string_array(args, consts::FIELD_NETWORK_ALLOW_HOSTS)? {
            policy.network_allow_hosts = hosts;
        }
        if let Some(value) = self.optional_usize(args, consts::FIELD_MAX_MODULE_BYTES)? {
            policy.max_module_bytes = value;
        }
        if let Some(value) = self.optional_u64(args, consts::FIELD_FUEL_BUDGET)? {
            policy.fuel_budget = value;
        }

        Ok(policy)
    }
}

/// ### Brief
///
/// default fields for a `WasmSkillTool`
impl Default for WasmSkillTool {
    fn default() -> Self {
        Self::new(
            consts::WASM_SKILL_TOOL_DEFAULT_NAME,
            Arc::new(WasmSkillHost::new()),
            SandboxPolicy::locked_down(),
        )
    }
}

/// ### Brief
///
/// factory/discovery-side wrapper for `WasmSkillTool`
///
/// ### Description
///
/// holds manifest metadata for describing the plugin to the system. it also implements `PluginFactory` to
/// provide access tool. this is here to allow `WasmSkillTool` types to integrate with the rest of the
/// plugin system.
///
/// ### Fields
/// * `manifest` - typed copy of the plugin manifest
/// * `tool` - tool struct
#[derive(Clone)]
pub struct WasmSkillPlugin {
    manifest: PluginManifest,
    tool: Arc<WasmSkillTool>,
}

/// ### Brief
///
/// base implementation for `WasmSkillPlugin`
impl WasmSkillPlugin {
    pub fn new(tool: Arc<WasmSkillTool>) -> Self {
        Self {
            manifest: Self::default_manifest(),
            tool,
        }
    }

    /// ### Brief
    ///
    /// creates default plugin manifest using predefined constants
    ///
    /// ### Returns
    ///
    /// default plugin manifest as a `PluginManifest`
    pub fn default_manifest() -> PluginManifest {
        PluginManifest {
            id: consts::WASM_SKILL_PLUGIN_ID.to_string(),
            name: consts::WASM_SKILL_PLUGIN_NAME.to_string(),
            version: consts::WASM_SKILL_PLUGIN_VERSION.to_string(),
            api_version: KELVIN_CORE_API_VERSION.to_string(),
            description: Some(
                "Sandboxed WebAssembly skill execution with workspace-scoped memory append."
                    .to_string(),
            ),
            homepage: None,
            capabilities: vec![
                PluginCapability::ToolProvider,
                PluginCapability::FsRead,
                PluginCapability::FsWrite,
            ],
            experimental: false,
            min_core_version: Some(consts::WASM_SKILL_MIN_CORE_VERSION.to_string()),
            max_core_version: None,
        }
    }
}

/// ### Brief
///
/// defaults for a wasm skill plugin
impl Default for WasmSkillPlugin {
    fn default() -> Self {
        Self::new(Arc::new(WasmSkillTool::default()))
    }
}

/// ### Brief
///
/// implement `PluginFactory` so we can get tools out
impl PluginFactory for WasmSkillPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn tool(&self) -> Option<Arc<dyn Tool>> {
        Some(self.tool.clone())
    }
}

/// ### Brief
///
/// implement tool trait
#[async_trait]
impl Tool for WasmSkillTool {
    fn name(&self) -> &str {
        &self.name
    }

    async fn call(&self, input: ToolCallInput) -> KelvinResult<ToolCallResult> {
        let args = self.require_args_object(&input.arguments)?;
        let wasm_rel_path = self.sanitize_rel_path(
            &self.require_string(args, consts::FIELD_WASM_PATH)?,
            consts::FIELD_WASM_PATH,
        )?;
        let policy = self.resolve_policy(args, self.default_policy.clone())?;

        let workspace_dir = PathBuf::from(&input.workspace_dir);
        let wasm_path = workspace_dir.join(&wasm_rel_path);
        let execution = self.host.run_file(&wasm_path, policy)?;

        let memory_rel_path = self
            .optional_string(args, consts::FIELD_MEMORY_APPEND_PATH)?
            .unwrap_or_else(|| self.default_memory_append_path.clone());
        let memory_rel_path =
            self.sanitize_rel_path(&memory_rel_path, consts::FIELD_MEMORY_APPEND_PATH)?;
        self.validate_memory_path_scope(&memory_rel_path)?;

        let memory_entry = self
            .optional_string(args, consts::FIELD_MEMORY_ENTRY)?
            .unwrap_or_else(|| {
                format!(
                    "run_id={} exit_code={} calls={}",
                    input.run_id,
                    execution.exit_code,
                    execution
                        .calls
                        .iter()
                        .map(claw_call_label)
                        .collect::<Vec<_>>()
                        .join(",")
                )
            });

        let memory_abs_path = workspace_dir.join(&memory_rel_path);
        if let Some(parent) = memory_abs_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut memory_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&memory_abs_path)?;
        writeln!(memory_file, "{memory_entry}")?;

        let calls_json = execution
            .calls
            .iter()
            .map(claw_call_json)
            .collect::<Vec<_>>();
        let summary = format!(
            "wasm skill exit={} calls={}",
            execution.exit_code,
            calls_json.len()
        );
        let output = json!({
            consts::JSON_KEY_WASM_PATH: wasm_rel_path,
            consts::JSON_KEY_MEMORY_PATH: memory_rel_path,
            consts::JSON_KEY_EXIT_CODE: execution.exit_code,
            consts::JSON_KEY_CALLS: calls_json,
        });

        Ok(ToolCallResult {
            summary: summary.clone(),
            output: Some(output.to_string()),
            visible_text: Some(summary),
            is_error: false,
        })
    }
}

/// ### Brief
///
/// serialize a claw call to a human-readable label string
fn claw_call_label(call: &ClawCall) -> String {
    match call {
        ClawCall::SendMessage { message_code } => format!("send_message({message_code})"),
        ClawCall::MoveServo { channel, position } => format!("move_servo({channel},{position})"),
        ClawCall::FsRead { handle } => format!("fs_read({handle})"),
        ClawCall::NetworkSend { packet } => format!("network_send({packet})"),
        ClawCall::HttpCall { url } => format!("http_call({url})"),
        ClawCall::EnvAccess { key } => format!("env_access({key})"),
    }
}

/// ### Brief
///
/// serialize a claw call to JSON
fn claw_call_json(call: &ClawCall) -> Value {
    match call {
        ClawCall::SendMessage { message_code } => json!({
            consts::JSON_KEY_KIND: consts::CLAW_KIND_SEND_MESSAGE,
            "message_code": message_code,
        }),
        ClawCall::MoveServo { channel, position } => json!({
            consts::JSON_KEY_KIND: consts::CLAW_KIND_MOVE_SERVO,
            "channel": channel,
            "position": position,
        }),
        ClawCall::FsRead { handle } => json!({
            consts::JSON_KEY_KIND: consts::CLAW_KIND_FS_READ,
            "handle": handle,
        }),
        ClawCall::NetworkSend { packet } => json!({
            consts::JSON_KEY_KIND: consts::CLAW_KIND_NETWORK_SEND,
            "packet": packet,
        }),
        ClawCall::HttpCall { url } => json!({
            consts::JSON_KEY_KIND: consts::CLAW_KIND_HTTP_CALL,
            "url": url,
        }),
        ClawCall::EnvAccess { key } => json!({
            consts::JSON_KEY_KIND: consts::CLAW_KIND_ENV_ACCESS,
            "key": key,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use kelvin_core::Tool;

    use super::WasmSkillTool;

    fn unique_test_workspace() -> std::path::PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("kelvin-wasm-tool-{millis}"));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_wasm(workspace: &Path, rel_path: &str, wat_src: &str) {
        let bytes = wat::parse_str(wat_src).expect("parse wat");
        let abs = workspace.join(rel_path);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent).expect("create skill dir");
        }
        std::fs::write(abs, bytes).expect("write wasm file");
    }

    #[tokio::test]
    async fn runs_wasm_and_appends_memory_entry() {
        let workspace = unique_test_workspace();
        write_wasm(
            &workspace,
            "skills/echo.wasm",
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

        let tool = WasmSkillTool::default();
        let result = tool
            .call(kelvin_core::ToolCallInput {
                run_id: "run-1".to_string(),
                session_id: "session-1".to_string(),
                workspace_dir: workspace.to_string_lossy().to_string(),
                arguments: json!({
                    "wasm_path": "skills/echo.wasm",
                    "memory_append_path": "memory/mvp.md",
                    "memory_entry": "mvp skill executed",
                    "policy_preset": "locked_down"
                }),
            })
            .await
            .expect("tool call");

        assert!(!result.is_error);
        let memory_text =
            std::fs::read_to_string(workspace.join("memory/mvp.md")).expect("memory file");
        assert!(memory_text.contains("mvp skill executed"));
    }

    #[tokio::test]
    async fn rejects_path_traversal() {
        let workspace = unique_test_workspace();
        let tool = WasmSkillTool::new(
            "wasm_skill",
            Arc::new(kelvin_wasm::WasmSkillHost::new()),
            kelvin_wasm::SandboxPolicy::locked_down(),
        );

        let error = tool
            .call(kelvin_core::ToolCallInput {
                run_id: "run-1".to_string(),
                session_id: "session-1".to_string(),
                workspace_dir: workspace.to_string_lossy().to_string(),
                arguments: json!({
                    "wasm_path": "../escape.wasm"
                }),
            })
            .await
            .expect_err("path traversal should fail");
        assert!(error.to_string().contains("path traversal"));
    }
}

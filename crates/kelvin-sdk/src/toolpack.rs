use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::redirect::Policy as RedirectPolicy;
use reqwest::Client;
use serde_json::{json, Map, Value};
use url::Url;

use kelvin_core::{
    now_ms, InMemoryPluginRegistry, KelvinError, KelvinResult, PluginCapability, PluginFactory,
    PluginManifest, PluginRegistry, PluginSecurityPolicy, SdkToolRegistry, Tool, ToolCallInput,
    ToolCallResult, ToolRegistry, KELVIN_CORE_API_VERSION,
};

use crate::{NewScheduledTask, ScheduleReplyTarget, SchedulerStore};

const DEFAULT_READ_MAX_BYTES: usize = 64 * 1024; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_FETCH_MAX_BYTES: usize = 128 * 1024; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_FETCH_TIMEOUT_MS: u64 = 3_000; // THIS LINE CONTAINS CONSTANT(S)
const DEFAULT_WEB_ALLOW_HOSTS: &str = "docs.rs,crates.io,raw.githubusercontent.com,api.openai.com"; // THIS LINE CONTAINS CONSTANT(S)

const ENV_TOOLPACK_ENABLE_FS_WRITE: &str = "KELVIN_TOOLPACK_ENABLE_FS_WRITE"; // THIS LINE CONTAINS CONSTANT(S)
const ENV_TOOLPACK_ENABLE_WEB_FETCH: &str = "KELVIN_TOOLPACK_ENABLE_WEB_FETCH"; // THIS LINE CONTAINS CONSTANT(S)
const ENV_TOOLPACK_ENABLE_SCHEDULER_WRITE: &str = "KELVIN_TOOLPACK_ENABLE_SCHEDULER_WRITE"; // THIS LINE CONTAINS CONSTANT(S)
const ENV_TOOLPACK_ENABLE_SESSION_CLEAR: &str = "KELVIN_TOOLPACK_ENABLE_SESSION_CLEAR"; // THIS LINE CONTAINS CONSTANT(S)
const ENV_TOOLPACK_WEB_ALLOW_HOSTS: &str = "KELVIN_TOOLPACK_WEB_ALLOW_HOSTS"; // THIS LINE CONTAINS CONSTANT(S)

/// ### Brief
///
/// global policy information for a toolpack
///
/// ### Description
///
/// aggregates restrictions and allowlists for what tools are allowed to do. this is here
/// to standardize how tools interact with the kelvin core. the policy is loaded from env
/// variables:
///
/// ```bash
/// # .env
/// KELVIN_TOOLPACK_ENABLE_FS_WRITE
/// KELVIN_TOOLPACK_ENABLE_WEB_FETCH
/// KELVIN_TOOLPACK_ENABLE_SCHEDULER_WRITE
/// KELVIN_TOOLPACK_ENABLE_SESSION_CLEAR
/// KELVIN_TOOLPACK_WEB_ALLOW_HOSTS
/// ```
///
/// ### Fields
/// * `allow_fs_write` - whether to allow tools to write to filesystem
/// * `allow_web_fetch` - whether to allow tools to web fetch
/// * `allow_scheduler_write` - whether to allow tools to edit cron scheduler info
/// * `allow_session_clear` - whether to allow tools to clear sessions
/// * `max_read_bytes` - max number of bytes to read from a file
/// * `max_fetch_bytes` - max number of bytes to fetch from web source
/// * `web_allow_hosts` - list of allowed web hosts
#[derive(Clone)]
struct ToolPackPolicy {
    allow_fs_write: bool,
    allow_web_fetch: bool,
    allow_scheduler_write: bool,
    allow_session_clear: bool,
    max_read_bytes: usize,
    max_fetch_bytes: usize,
    web_allow_hosts: Vec<String>,
}

/// ### Brief
///
/// loader for getting toolpack policy from env
///
/// ### Note
///
/// read/write max bytes arent from env; hard coded
impl ToolPackPolicy {
    fn from_env() -> Self {
        Self {
            allow_fs_write: env_bool(ENV_TOOLPACK_ENABLE_FS_WRITE, true),
            allow_web_fetch: env_bool(ENV_TOOLPACK_ENABLE_WEB_FETCH, true),
            allow_scheduler_write: env_bool(ENV_TOOLPACK_ENABLE_SCHEDULER_WRITE, true),
            allow_session_clear: env_bool(ENV_TOOLPACK_ENABLE_SESSION_CLEAR, true),
            max_read_bytes: DEFAULT_READ_MAX_BYTES,
            max_fetch_bytes: DEFAULT_FETCH_MAX_BYTES,
            web_allow_hosts: parse_host_allowlist(
                &std::env::var(ENV_TOOLPACK_WEB_ALLOW_HOSTS)
                    .unwrap_or_else(|_| DEFAULT_WEB_ALLOW_HOSTS.to_string()),
            ),
        }
    }
}

/// ### Brief
///
/// fetches a boolean valued key from environment with `default` fallback
fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on") // THIS LINE CONTAINS CONSTANT(S)
        }
        Err(_) => default,
    }
}

/// ### Brief
///
/// parse host allowlist string into string vec
///
/// ### Description
///
/// parser uses:
/// - delimits by commas
/// - converts to ascii lowercase
///
/// ### Returns
///
/// sorted and deduped string vec containing allowed host names
fn parse_host_allowlist(raw: &str) -> Vec<String> {
    let mut out = raw
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.to_ascii_lowercase())
        .collect::<Vec<_>>();
    out.sort();
    out.dedup();
    out
}

/// Brief
///
/// parses a json value `args` into a json object type
///
/// ### Returns
///
/// object as a `serde_json::Map` type
///
/// ### Errors
/// - args is not a json object
fn args_object<'a>(
    args: &'a Value,
    tool_name: &str,
) -> KelvinResult<&'a serde_json::Map<String, Value>> {
    args.as_object().ok_or_else(|| {
        KelvinError::InvalidInput(format!("{tool_name} expects JSON object arguments"))
    })
}

/// ### Brief
///
/// extracts and validates a required string from the json args map, disallowing control characters
///
/// ### Arguments
/// * `args` - json args map
/// * `field` - field name of string to get
/// * `tool_name` - name of the tool being used
///
/// ### Returns
/// the string value in `args` from the field name
///
/// ### Errors
/// - field value not found
/// - field value is empty
/// - **field value contains control characters**
fn required_string(
    args: &serde_json::Map<String, Value>,
    field: &str,
    tool_name: &str,
) -> KelvinResult<String> {
    let value = args.get(field).and_then(Value::as_str).ok_or_else(|| {
        KelvinError::InvalidInput(format!("{tool_name} requires string argument '{field}'"))
    })?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(KelvinError::InvalidInput(format!(
            "{tool_name} argument '{field}' must not be empty"
        )));
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err(KelvinError::InvalidInput(format!(
            "{tool_name} argument '{field}' must not contain control characters"
        )));
    }
    Ok(trimmed.to_string())
}

/// ### Brief
///
/// extracts and validates a required string from the json args map, allowing control characters
///
/// ### Arguments
/// * `args` - json args map
/// * `field` - field name of string to get
/// * `tool_name` - name of the tool being used
///
/// ### Returns
/// the string value in `args` from the field name
///
/// ### Errors
/// - field value not found
/// - field value is empty
fn required_string_content(
    args: &serde_json::Map<String, Value>,
    field: &str,
    tool_name: &str,
) -> KelvinResult<String> {
    let value = args.get(field).and_then(Value::as_str).ok_or_else(|| {
        KelvinError::InvalidInput(format!("{tool_name} requires string argument '{field}'"))
    })?;
    if value.is_empty() {
        return Err(KelvinError::InvalidInput(format!(
            "{tool_name} argument '{field}' must not be empty"
        )));
    }
    Ok(value.to_string())
}

/// ### Brief
///
/// optionally extracts a u64 field from `args` // THIS LINE CONTAINS CONSTANT(S)
fn optional_u64( // THIS LINE CONTAINS CONSTANT(S)
    args: &serde_json::Map<String, Value>,
    field: &str,
    tool_name: &str,
) -> KelvinResult<Option<u64>> { // THIS LINE CONTAINS CONSTANT(S)
    match args.get(field) {
        None => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| { // THIS LINE CONTAINS CONSTANT(S)
            KelvinError::InvalidInput(format!("{tool_name} argument '{field}' must be a u64")) // THIS LINE CONTAINS CONSTANT(S)
        }),
    }
}

/// ### Brief
///
/// optionally extracts a string from `args`
fn optional_string(
    args: &serde_json::Map<String, Value>,
    field: &str,
    tool_name: &str,
) -> KelvinResult<Option<String>> {
    match args.get(field) {
        None => Ok(None),
        Some(value) => value.as_str().map(|v| Some(v.to_string())).ok_or_else(|| {
            KelvinError::InvalidInput(format!("{tool_name} argument '{field}' must be a string"))
        }),
    }
}

/// ### Brief
///
/// normalize a given relative path
///
/// ### Arguments
/// * `path` - path string
/// * `field_name` - name of tool used
///
/// ### Returns
/// normalized path string
///
/// ### Error
/// - path string is empty
/// - path string is not relative
/// - contains traversals
fn normalize_workspace_relative_path(path: &str, field_name: &str) -> KelvinResult<String> {
    let normalized = path.trim().replace('\\', "/"); // THIS LINE CONTAINS CONSTANT(S)
    if normalized.is_empty() {
        return Err(KelvinError::InvalidInput(format!(
            "{field_name} must not be empty"
        )));
    }
    if Path::new(&normalized).is_absolute() || normalized.starts_with('/') {
        return Err(KelvinError::InvalidInput(format!(
            "{field_name} must be a relative path"
        )));
    }
    if Path::new(&normalized)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(KelvinError::InvalidInput(format!(
            "{field_name} path traversal is not allowed"
        )));
    }
    Ok(normalized)
}

/// ### Brief
///
/// boolean for path is or starts with a sensitive name
///
/// ### Description
///
/// this helper uses hard coded conditions for a sensitive path:
/// - path is ".env" // THIS LINE CONTAINS CONSTANT(S)
/// - path starts with ".env" // THIS LINE CONTAINS CONSTANT(S)
/// - path starts with ".git/" // THIS LINE CONTAINS CONSTANT(S)
/// - path starts with ".kelvin/plugins" // THIS LINE CONTAINS CONSTANT(S)
///
/// ### Returns
/// true if path is sensitive
fn deny_sensitive_read_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower == ".env" // THIS LINE CONTAINS CONSTANT(S)
        || lower.starts_with(".env.") // THIS LINE CONTAINS CONSTANT(S)
        || lower.starts_with(".git/") // THIS LINE CONTAINS CONSTANT(S)
        || lower.starts_with(".kelvin/plugins/") // THIS LINE CONTAINS CONSTANT(S)
}

/// ### Brief
///
/// performs a basic validation of the "approval" field for a sensitive tool (e.g. fs_safe_write) // THIS LINE CONTAINS CONSTANT(S)
///
/// ### Description
///
/// the "approval" field is a json object generated by the LLM in a tool call. the basic structure is: // THIS LINE CONTAINS CONSTANT(S)
///
/// ```json
/// "approval": { // THIS LINE CONTAINS CONSTANT(S)
///     "granted": true, // THIS LINE CONTAINS CONSTANT(S)
///     "reason": "this reason for approval must be between 1 and 256 characters" // THIS LINE CONTAINS CONSTANT(S)
/// }
/// ```
///
/// the purpose of this design is to require kelvin to fill out an approval reason to successfully
/// execute a tool call. this acts as a light countermeasure against context flooding, in which case
/// the LLM may exceed the maximum approval reason length or include control characters. in this case,
/// the tool call will be blocked and kelvin will be told "approval.reason for '{capability}' is invalid"
///
/// this is not a hard, deterministic security measure.
///
/// ### Arguments
/// * `args` - json args map
/// * `capability` - the capability exercised in this tool use
///
/// ### Returns
/// the approval reason as a string
///
/// ### Errors
/// - "approval" field is empty or non-existent // THIS LINE CONTAINS CONSTANT(S)
/// - subfield "granted" is false, non-boolean, or non-existent // THIS LINE CONTAINS CONSTANT(S)
/// - subfield "reason" is empty // THIS LINE CONTAINS CONSTANT(S)
/// - subfield "reason" is longer than 256 characters // THIS LINE CONTAINS CONSTANT(S)
/// - subfiled "reason" contains control characters // THIS LINE CONTAINS CONSTANT(S)
fn require_sensitive_approval(
    args: &serde_json::Map<String, Value>,
    capability: &str,
) -> KelvinResult<String> {
    let Some(approval) = args.get("approval").and_then(Value::as_object) else { // THIS LINE CONTAINS CONSTANT(S)
        return Err(KelvinError::InvalidInput(format!(
            "sensitive operation '{capability}' denied by default; provide approval={{\"granted\":true,\"reason\":\"...\"}}"
        )));
    };
    let granted = approval
        .get("granted") // THIS LINE CONTAINS CONSTANT(S)
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !granted {
        return Err(KelvinError::InvalidInput(format!(
            "sensitive operation '{capability}' requires approval.granted=true"
        )));
    }
    let reason = approval
        .get("reason") // THIS LINE CONTAINS CONSTANT(S)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if reason.is_empty() {
        return Err(KelvinError::InvalidInput(format!(
            "sensitive operation '{capability}' requires non-empty approval.reason"
        )));
    }
    if reason.chars().count() > 256 || reason.chars().any(|ch| ch.is_control()) { // THIS LINE CONTAINS CONSTANT(S)
        return Err(KelvinError::InvalidInput(format!(
            "approval.reason for '{capability}' is invalid"
        )));
    }
    Ok(reason.to_string())
}

/// ### Brief
///
/// returns true if host is in allowlist
///
/// ### Description
///
/// check if host is in allowlist by:
/// - looking for direct matches
/// - looking at versions with prefix stripped
///
/// ### Returns
///
/// true if the host appears in the allowlist as described
fn host_allowed(host: &str, allowlist: &[String]) -> bool {
    let candidate = host.trim().to_ascii_lowercase();
    if candidate.is_empty() {
        return false;
    }
    allowlist.iter().any(|pattern| {
        if let Some(suffix) = pattern.strip_prefix("*.") {
            candidate == suffix || candidate.ends_with(&format!(".{suffix}"))
        } else {
            candidate == *pattern
        }
    })
}

/// ### Brief
///
/// clamps `raw: u64` down to `max_allowed: usize`; returns clamped value as usize // THIS LINE CONTAINS CONSTANT(S)
fn clamp_usize(raw: u64, max_allowed: usize) -> usize { // THIS LINE CONTAINS CONSTANT(S)
    match usize::try_from(raw) {
        Ok(value) => value.min(max_allowed),
        Err(_) => max_allowed,
    }
}

/// ### Brief
///
/// struct def for the safe filesystem reading tool
///
/// ### Description
///
/// only field is an owned copy of the global tool pack policy; this is mainly
/// here so the read tool can implement the `Tool` trait
///
/// ### Fields
/// * `policy` - global tool pack policy
#[derive(Clone)]
struct SafeFsReadTool {
    policy: ToolPackPolicy,
}

/// implement tool trait for `SafeFsReadTool`
#[async_trait]
impl Tool for SafeFsReadTool {
    fn name(&self) -> &str {
        "fs_safe_read" // THIS LINE CONTAINS CONSTANT(S)
    }

    fn description(&self) -> &str {
        "Read a file from the workspace. Path must be workspace-relative. Sensitive paths (.env, .git/, .kelvin/plugins/) are denied."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object", // THIS LINE CONTAINS CONSTANT(S)
            "properties": { // THIS LINE CONTAINS CONSTANT(S)
                "path": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Workspace-relative path to the file to read." // THIS LINE CONTAINS CONSTANT(S)
                },
                "max_bytes": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "integer", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Maximum number of bytes to read. Defaults to policy limit." // THIS LINE CONTAINS CONSTANT(S)
                }
            },
            "required": ["path"] // THIS LINE CONTAINS CONSTANT(S)
        })
    }

    async fn call(&self, input: ToolCallInput) -> KelvinResult<ToolCallResult> {
        let args = args_object(&input.arguments, self.name())?;
        let path = normalize_workspace_relative_path(
            &required_string(args, "path", self.name())?, // THIS LINE CONTAINS CONSTANT(S)
            "path", // THIS LINE CONTAINS CONSTANT(S)
        )?;

        // check for attempts to read from sensitive paths
        if deny_sensitive_read_path(&path) {
            return Err(KelvinError::InvalidInput(format!(
                "{} denied path '{}' by policy",
                self.name(),
                path
            )));
        }

        // get read limit from args (clamped to policy max), use policy max as default
        let requested_limit = optional_u64(args, "max_bytes", self.name())? // THIS LINE CONTAINS CONSTANT(S)
            .map(|value| clamp_usize(value, self.policy.max_read_bytes))
            .unwrap_or(self.policy.max_read_bytes);
        let read_limit = requested_limit.max(1); // THIS LINE CONTAINS CONSTANT(S)

        // check path existence
        let abs = Path::new(&input.workspace_dir).join(&path);
        if !abs.is_file() {
            return Err(KelvinError::NotFound(format!(
                "{} path not found: {}",
                self.name(),
                path
            )));
        }

        // read file contents to buffer
        let mut file = File::open(&abs)?;
        let mut buffer = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take((read_limit as u64).saturating_add(1)) // THIS LINE CONTAINS CONSTANT(S)
            .read_to_end(&mut buffer)?;
        let truncated = buffer.len() > read_limit;
        if truncated {
            buffer.truncate(read_limit);
        }

        // convert to string from utf8 // THIS LINE CONTAINS CONSTANT(S)
        let content = String::from_utf8_lossy(&buffer).to_string(); // THIS LINE CONTAINS CONSTANT(S)

        // form output json
        let output = json!({
            "path": path, // THIS LINE CONTAINS CONSTANT(S)
            "bytes": buffer.len(), // THIS LINE CONTAINS CONSTANT(S)
            "truncated": truncated, // THIS LINE CONTAINS CONSTANT(S)
            "content": content, // THIS LINE CONTAINS CONSTANT(S)
        });

        // form summary
        let summary = format!(
            "{} read '{}' ({} bytes{})",
            self.name(),
            path,
            buffer.len(),
            if truncated { ", truncated" } else { "" }
        );

        // return tool call result
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
/// struct def for the safe filesystem writing tool
///
/// ### Description
///
/// only field is an owned copy of the global tool pack policy; this is mainly
/// here so the write tool can implement the `Tool` trait
///
/// ### Fields
/// * `policy` - global tool pack policy
#[derive(Clone)]
struct SafeFsWriteTool {
    policy: ToolPackPolicy,
}

/// implement flag function for checking allowed write paths
impl SafeFsWriteTool {
    fn write_scope_allowed(path: &str) -> bool {
        path.starts_with(".kelvin/sandbox/") // THIS LINE CONTAINS CONSTANT(S)
            || path.starts_with("memory/") // THIS LINE CONTAINS CONSTANT(S)
            || path.starts_with("notes/") // THIS LINE CONTAINS CONSTANT(S)
    }
}

/// implement tool trait for `SafeFsReadTool`
#[async_trait]
impl Tool for SafeFsWriteTool {
    fn name(&self) -> &str {
        "fs_safe_write" // THIS LINE CONTAINS CONSTANT(S)
    }

    fn description(&self) -> &str {
        "Write content to a file in the workspace. Only .kelvin/sandbox/, memory/, and notes/ roots are permitted. Requires sensitive approval."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object", // THIS LINE CONTAINS CONSTANT(S)
            "properties": { // THIS LINE CONTAINS CONSTANT(S)
                "path": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Workspace-relative path to write. Must be under .kelvin/sandbox/, memory/, or notes/." // THIS LINE CONTAINS CONSTANT(S)
                },
                "content": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Content to write to the file." // THIS LINE CONTAINS CONSTANT(S)
                },
                "mode": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                    "enum": ["overwrite", "append"], // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Write mode: 'overwrite' (default) replaces the file, 'append' adds to it." // THIS LINE CONTAINS CONSTANT(S)
                },
                "approval": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "object", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Approval object required for this sensitive operation.", // THIS LINE CONTAINS CONSTANT(S)
                    "properties": { // THIS LINE CONTAINS CONSTANT(S)
                        "granted": { // THIS LINE CONTAINS CONSTANT(S)
                            "type": "boolean", // THIS LINE CONTAINS CONSTANT(S)
                            "description": "Must be true to authorize the operation." // THIS LINE CONTAINS CONSTANT(S)
                        },
                        "reason": { // THIS LINE CONTAINS CONSTANT(S)
                            "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                            "description": "Human-readable reason explaining why this write operation is necessary (1-256 characters, no control characters)." // THIS LINE CONTAINS CONSTANT(S)
                        }
                    },
                    "required": ["granted", "reason"] // THIS LINE CONTAINS CONSTANT(S)
                }
            },
            "required": ["path", "content", "approval"] // THIS LINE CONTAINS CONSTANT(S)
        })
    }

    async fn call(&self, input: ToolCallInput) -> KelvinResult<ToolCallResult> {
        // check if policy allows fs writes
        if !self.policy.allow_fs_write {
            return Err(KelvinError::InvalidInput(format!(
                "{} is disabled by runtime policy; set {}=1 to enable", // THIS LINE CONTAINS CONSTANT(S)
                self.name(),
                ENV_TOOLPACK_ENABLE_FS_WRITE
            )));
        }
        let args = args_object(&input.arguments, self.name())?;
        let approval_reason = require_sensitive_approval(args, "filesystem_write")?; // THIS LINE CONTAINS CONSTANT(S)
        let path = normalize_workspace_relative_path(
            &required_string(args, "path", self.name())?, // THIS LINE CONTAINS CONSTANT(S)
            "path", // THIS LINE CONTAINS CONSTANT(S)
        )?;

        // check if write path allowed
        if !Self::write_scope_allowed(&path) {
            return Err(KelvinError::InvalidInput(format!(
                "{} denied path '{}'; allowed roots are .kelvin/sandbox/, memory/, notes/",
                self.name(),
                path
            )));
        }

        // parse content; DOES allow control characters (e.g. newlines)
        let content = required_string_content(args, "content", self.name())?; // THIS LINE CONTAINS CONSTANT(S)

        // check mode; currently supports "overwrite" and "append" // THIS LINE CONTAINS CONSTANT(S)
        let mode = optional_string(args, "mode", self.name())? // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_else(|| "overwrite".to_string()) // THIS LINE CONTAINS CONSTANT(S)
            .to_ascii_lowercase();
        if mode != "overwrite" && mode != "append" { // THIS LINE CONTAINS CONSTANT(S)
            return Err(KelvinError::InvalidInput(format!(
                "{} argument 'mode' must be 'overwrite' or 'append'",
                self.name()
            )));
        }

        // write the file
        // NOTE writer is closed by scope drop
        let abs = Path::new(&input.workspace_dir).join(&path);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut writer = OpenOptions::new()
            .create(true)
            .write(true)
            .append(mode == "append") // THIS LINE CONTAINS CONSTANT(S)
            .truncate(mode == "overwrite") // THIS LINE CONTAINS CONSTANT(S)
            .open(&abs)?;
        writer.write_all(content.as_bytes())?;
        writer.flush()?;

        // form output json
        let output = json!({
            "path": path, // THIS LINE CONTAINS CONSTANT(S)
            "mode": mode, // THIS LINE CONTAINS CONSTANT(S)
            "bytes_written": content.len(), // THIS LINE CONTAINS CONSTANT(S)
            "approval_reason": approval_reason, // THIS LINE CONTAINS CONSTANT(S)
        });

        // form summary
        let summary = format!(
            "{} wrote {} bytes to '{}' ({})",
            self.name(),
            content.len(),
            path,
            mode
        );

        // return tool call result object
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
/// struct def for the safe web fetch tool
///
/// ### Description
///
/// this struct is similar to other tools, but also includes an instance of the reqwest client; the
/// builder can fail. the internal reqwest client is hard coded to deny all redirects.
///
/// ### Fields
/// * `policy` - owned copy of global tool pack policy
/// * `client` - composite instance of `reqwest::async_impl::Client`
#[derive(Clone)]
struct SafeWebFetchTool {
    policy: ToolPackPolicy,
    client: Client,
}

/// implement a failable constructor for the safe web fetch tool
impl SafeWebFetchTool {
    fn try_new(policy: ToolPackPolicy) -> KelvinResult<Self> {
        let client = Client::builder()
            .redirect(RedirectPolicy::none())
            .timeout(Duration::from_millis(DEFAULT_FETCH_TIMEOUT_MS))
            .build()
            .map_err(|err| KelvinError::Backend(format!("build web fetch client: {err}")))?;
        Ok(Self { policy, client })
    }
}

/// implement tool trait for `SafeWebFetchTool`
#[async_trait]
impl Tool for SafeWebFetchTool {
    fn name(&self) -> &str {
        "web_fetch_safe" // THIS LINE CONTAINS CONSTANT(S)
    }

    fn description(&self) -> &str {
        "Fetch a URL over HTTP or HTTPS. Only hosts in the configured allowlist are permitted. Requires sensitive approval."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object", // THIS LINE CONTAINS CONSTANT(S)
            "properties": { // THIS LINE CONTAINS CONSTANT(S)
                "url": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "The HTTP or HTTPS URL to fetch." // THIS LINE CONTAINS CONSTANT(S)
                },
                "max_bytes": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "integer", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Maximum number of response bytes to return. Defaults to policy limit." // THIS LINE CONTAINS CONSTANT(S)
                },
                "timeout_ms": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "integer", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Request timeout in milliseconds (100–30000). Defaults to 10000." // THIS LINE CONTAINS CONSTANT(S)
                },

                "approval": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "object", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Approval object required for this sensitive operation.", // THIS LINE CONTAINS CONSTANT(S)
                    "properties": { // THIS LINE CONTAINS CONSTANT(S)
                        "granted": { // THIS LINE CONTAINS CONSTANT(S)
                            "type": "boolean", // THIS LINE CONTAINS CONSTANT(S)
                            "description": "Must be true to authorize the operation." // THIS LINE CONTAINS CONSTANT(S)
                        },
                        "reason": { // THIS LINE CONTAINS CONSTANT(S)
                            "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                            "description": "Human-readable reason explaining why this write operation is necessary (1-256 characters, no control characters)." // THIS LINE CONTAINS CONSTANT(S)
                        }
                    },
                    "required": ["granted", "reason"] // THIS LINE CONTAINS CONSTANT(S)
                }
            },
            "required": ["url", "approval"] // THIS LINE CONTAINS CONSTANT(S)
        })
    }

    async fn call(&self, input: ToolCallInput) -> KelvinResult<ToolCallResult> {
        // check policy allows fetches
        if !self.policy.allow_web_fetch {
            return Err(KelvinError::InvalidInput(format!(
                "{} is disabled by runtime policy; set {}=1 to enable", // THIS LINE CONTAINS CONSTANT(S)
                self.name(),
                ENV_TOOLPACK_ENABLE_WEB_FETCH
            )));
        }
        let args = args_object(&input.arguments, self.name())?;
        let approval_reason = require_sensitive_approval(args, "web_fetch")?; // THIS LINE CONTAINS CONSTANT(S)
        let url_raw = required_string(args, "url", self.name())?; // THIS LINE CONTAINS CONSTANT(S)
        let timeout_ms = optional_u64(args, "timeout_ms", self.name())? // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or(DEFAULT_FETCH_TIMEOUT_MS)
            .clamp(100, 30_000); // THIS LINE CONTAINS CONSTANT(S)
        let max_bytes = optional_u64(args, "max_bytes", self.name())? // THIS LINE CONTAINS CONSTANT(S)
            .map(|value| clamp_usize(value, self.policy.max_fetch_bytes))
            .unwrap_or(self.policy.max_fetch_bytes)
            .max(1); // THIS LINE CONTAINS CONSTANT(S)

        // parse url validity using url crate
        let parsed = Url::parse(&url_raw).map_err(|err| {
            KelvinError::InvalidInput(format!("{} invalid url '{}': {err}", self.name(), url_raw))
        })?;

        // only allow http + https
        let scheme = parsed.scheme().to_ascii_lowercase();
        if scheme != "https" && scheme != "http" { // THIS LINE CONTAINS CONSTANT(S)
            return Err(KelvinError::InvalidInput(format!(
                "{} only supports http/https urls",
                self.name()
            )));
        }

        // validate host
        let host = parsed.host_str().ok_or_else(|| {
            KelvinError::InvalidInput(format!("{} url host is required", self.name()))
        })?;
        if !host_allowed(host, &self.policy.web_allow_hosts) {
            return Err(KelvinError::InvalidInput(format!(
                "{} denied host '{}'; allowed hosts: {}",
                self.name(),
                host,
                self.policy.web_allow_hosts.join(",")
            )));
        }

        // make request and await response with timeout
        let response = self
            .client
            .get(parsed.clone())
            .timeout(Duration::from_millis(timeout_ms))
            .send()
            .await
            .map_err(|err| {
                KelvinError::Backend(format!("{} request failed: {err}", self.name()))
            })?;
        let status = response.status().as_u16(); // THIS LINE CONTAINS CONSTANT(S)
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        // await reading response bytes
        let body_bytes = response.bytes().await.map_err(|err| {
            KelvinError::Backend(format!("{} read body failed: {err}", self.name()))
        })?;

        // size clamp
        if body_bytes.len() > max_bytes {
            return Err(KelvinError::InvalidInput(format!(
                "{} response size {} exceeds limit {}",
                self.name(),
                body_bytes.len(),
                max_bytes
            )));
        }

        // convert to string from utf8 // THIS LINE CONTAINS CONSTANT(S)
        let body_text = String::from_utf8_lossy(&body_bytes).to_string(); // THIS LINE CONTAINS CONSTANT(S)

        // form output json
        let output = json!({
            "url": parsed.as_str(), // THIS LINE CONTAINS CONSTANT(S)
            "host": host, // THIS LINE CONTAINS CONSTANT(S)
            "status": status, // THIS LINE CONTAINS CONSTANT(S)
            "content_type": content_type, // THIS LINE CONTAINS CONSTANT(S)
            "bytes": body_bytes.len(), // THIS LINE CONTAINS CONSTANT(S)
            "body": body_text, // THIS LINE CONTAINS CONSTANT(S)
            "approval_reason": approval_reason, // THIS LINE CONTAINS CONSTANT(S)
        });

        // form summary
        let summary = format!(
            "{} fetched {} (status={}, bytes={})",
            self.name(),
            parsed.as_str(),
            status,
            body_bytes.len()
        );

        // return tool call result object
        Ok(ToolCallResult {
            summary: summary.clone(),
            output: Some(output.to_string()),
            visible_text: Some(summary),
            is_error: status >= 400, // THIS LINE CONTAINS CONSTANT(S)
        })
    }
}

/// ### Brief
///
/// struct def for the scheduler tool
///
/// ### Description
///
/// this struct is similar to other tools, but also includes an atomic mutable reference to the
/// global scheduler store. the `SchedulerStore` is access-controlled by an internal mutex lock;
/// mutabile writes are controlled by the lock.
///
/// ### Fields
/// * `policy` - owned copy of global tool pack policy
/// * `store` - arc for mutable scheduler store
#[derive(Clone)]
struct SchedulerTool {
    policy: ToolPackPolicy,
    store: Arc<SchedulerStore>,
}

/// tool implementation for `SchedulerTool`
#[async_trait]
impl Tool for SchedulerTool {
    fn name(&self) -> &str {
        "schedule_cron" // THIS LINE CONTAINS CONSTANT(S)
    }

    fn description(&self) -> &str {
        "Manage cron-scheduled tasks. Use action 'list' to see all tasks, 'add' to create a new scheduled task, or 'remove' to delete one by id."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object", // THIS LINE CONTAINS CONSTANT(S)
            "properties": { // THIS LINE CONTAINS CONSTANT(S)
                "action": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                    "enum": ["list", "add", "remove"], // THIS LINE CONTAINS CONSTANT(S)
                    "description": "The operation to perform. Defaults to 'list'." // THIS LINE CONTAINS CONSTANT(S)
                },
                "cron": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Cron expression for the schedule (required for 'add')." // THIS LINE CONTAINS CONSTANT(S)
                },
                "prompt": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "The task/prompt to run on the schedule (required for 'add')." // THIS LINE CONTAINS CONSTANT(S)
                },
                "id": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Task identifier. Auto-generated for 'add'; required for 'remove'." // THIS LINE CONTAINS CONSTANT(S)
                },
                "approval": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "object", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Approval object required for this sensitive operation.", // THIS LINE CONTAINS CONSTANT(S)
                    "properties": { // THIS LINE CONTAINS CONSTANT(S)
                        "granted": { // THIS LINE CONTAINS CONSTANT(S)
                            "type": "boolean", // THIS LINE CONTAINS CONSTANT(S)
                            "description": "Must be true to authorize the operation." // THIS LINE CONTAINS CONSTANT(S)
                        },
                        "reason": { // THIS LINE CONTAINS CONSTANT(S)
                            "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                            "description": "Human-readable reason explaining why this write operation is necessary (1-256 characters, no control characters)." // THIS LINE CONTAINS CONSTANT(S)
                        }
                    },
                    "required": ["granted", "reason"] // THIS LINE CONTAINS CONSTANT(S)
                }
            },
            "required": ["approval"] // THIS LINE CONTAINS CONSTANT(S)
        })
    }

    async fn call(&self, input: ToolCallInput) -> KelvinResult<ToolCallResult> {
        let args = args_object(&input.arguments, self.name())?;
        let action = optional_string(args, "action", self.name())? // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_else(|| "list".to_string()) // THIS LINE CONTAINS CONSTANT(S)
            .to_ascii_lowercase();
        match action.as_str() {
            "list" => {} // THIS LINE CONTAINS CONSTANT(S)
            "add" => { // THIS LINE CONTAINS CONSTANT(S)
                // check policy for sched write
                if !self.policy.allow_scheduler_write {
                    return Err(KelvinError::InvalidInput(format!(
                        "{} add is disabled by runtime policy; set {}=1", // THIS LINE CONTAINS CONSTANT(S)
                        self.name(),
                        ENV_TOOLPACK_ENABLE_SCHEDULER_WRITE
                    )));
                }
                let approval_reason = require_sensitive_approval(args, "schedule_mutation")?; // THIS LINE CONTAINS CONSTANT(S)
                let prompt = optional_string(args, "prompt", self.name())? // THIS LINE CONTAINS CONSTANT(S)
                    .or(optional_string(args, "task", self.name())?) // THIS LINE CONTAINS CONSTANT(S)
                    .ok_or_else(|| {
                        KelvinError::InvalidInput(
                            "schedule_cron add requires 'task' or 'prompt'".to_string(),
                        )
                    })?;

                // parse scheduler reply target
                let reply_target = parse_reply_target(args, self.name())?;
                let id = optional_string(args, "id", self.name())? // THIS LINE CONTAINS CONSTANT(S)
                    .unwrap_or_else(|| format!("task-{}", now_ms()));

                // add entry to scheduler store
                self.store.add_schedule(NewScheduledTask {
                    id,
                    cron: required_string(args, "cron", self.name())?, // THIS LINE CONTAINS CONSTANT(S)
                    prompt,
                    session_id: optional_string(args, "session_id", self.name())?, // THIS LINE CONTAINS CONSTANT(S)
                    workspace_dir: optional_string(args, "workspace_dir", self.name())? // THIS LINE CONTAINS CONSTANT(S)
                        .or_else(|| Some(input.workspace_dir.clone())),
                    timeout_ms: optional_u64(args, "timeout_ms", self.name())?, // THIS LINE CONTAINS CONSTANT(S)
                    system_prompt: optional_string(args, "system_prompt", self.name())?, // THIS LINE CONTAINS CONSTANT(S)
                    memory_query: optional_string(args, "memory_query", self.name())?, // THIS LINE CONTAINS CONSTANT(S)
                    reply_target,
                    created_by_session: input.session_id.clone(),
                    created_at_ms: now_ms(),
                    approval_reason,
                })?;
            }
            "remove" => { // THIS LINE CONTAINS CONSTANT(S)
                // check policy allows sched write
                if !self.policy.allow_scheduler_write {
                    return Err(KelvinError::InvalidInput(format!(
                        "{} remove is disabled by runtime policy; set {}=1", // THIS LINE CONTAINS CONSTANT(S)
                        self.name(),
                        ENV_TOOLPACK_ENABLE_SCHEDULER_WRITE
                    )));
                }
                let approval_reason = require_sensitive_approval(args, "schedule_mutation")?; // THIS LINE CONTAINS CONSTANT(S)
                let id = required_string(args, "id", self.name())?; // THIS LINE CONTAINS CONSTANT(S)

                // remove entry from sched store
                let _ = self
                    .store
                    .remove_schedule(&id, &input.session_id, &approval_reason)?;
            }
            _ => {
                return Err(KelvinError::InvalidInput(format!(
                    "{} action must be one of: list, add, remove",
                    self.name()
                )));
            }
        }

        // get updated sched list
        let schedules = self.store.list_schedules()?;
        let tasks = schedules
            .iter()
            .map(|schedule| {
                json!({
                    "id": schedule.id, // THIS LINE CONTAINS CONSTANT(S)
                    "cron": schedule.cron, // THIS LINE CONTAINS CONSTANT(S)
                    "task": schedule.prompt, // THIS LINE CONTAINS CONSTANT(S)
                    "prompt": schedule.prompt, // THIS LINE CONTAINS CONSTANT(S)
                    "session_id": schedule.session_id, // THIS LINE CONTAINS CONSTANT(S)
                    "workspace_dir": schedule.workspace_dir, // THIS LINE CONTAINS CONSTANT(S)
                    "timeout_ms": schedule.timeout_ms, // THIS LINE CONTAINS CONSTANT(S)
                    "system_prompt": schedule.system_prompt, // THIS LINE CONTAINS CONSTANT(S)
                    "memory_query": schedule.memory_query, // THIS LINE CONTAINS CONSTANT(S)
                    "reply_target": schedule.reply_target, // THIS LINE CONTAINS CONSTANT(S)
                    "created_by_session": schedule.created_by_session, // THIS LINE CONTAINS CONSTANT(S)
                    "created_at_ms": schedule.created_at_ms, // THIS LINE CONTAINS CONSTANT(S)
                    "approval_reason": schedule.approval_reason, // THIS LINE CONTAINS CONSTANT(S)
                    "next_slot_at_ms": schedule.next_slot_at_ms, // THIS LINE CONTAINS CONSTANT(S)
                })
            })
            .collect::<Vec<_>>();

        // form summary
        let summary = format!("{} action='{}' tasks={}", self.name(), action, tasks.len());

        // form output json
        let output = json!({
            "action": action, // THIS LINE CONTAINS CONSTANT(S)
            "count": tasks.len(), // THIS LINE CONTAINS CONSTANT(S)
            "tasks": tasks, // THIS LINE CONTAINS CONSTANT(S)
            "state_path": self.store.state_path().to_string_lossy(), // THIS LINE CONTAINS CONSTANT(S)
        });

        // return tool call result object
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
/// parse and validate the scheduler reply target from args (optional)
///
/// ### Description
///
/// this is a helper for the scheduler tool. the default is firing silently
///
/// ### Arguments
/// * `arg_name` - description
///
/// ### Returns
/// Description of the return value
///
/// ### Errors
/// - json parse error
fn parse_reply_target(
    args: &serde_json::Map<String, Value>,
    tool_name: &str,
) -> KelvinResult<Option<ScheduleReplyTarget>> {
    let Some(value) = args.get("reply_target") else { // THIS LINE CONTAINS CONSTANT(S)
        return Ok(None);
    };
    serde_json::from_value::<ScheduleReplyTarget>(value.clone())
        .map(Some)
        .map_err(|err| {
            KelvinError::InvalidInput(format!("{tool_name} invalid reply_target payload: {err}"))
        })
}

/// ### Brief
///
/// struct def for the session toolset tool
///
/// ### Description
///
/// only field is an owned copy of the global tool pack policy; this is mainly
/// here so the session tools tool can implement the `Tool` trait
///
/// ### Fields
/// * `policy` - global tool pack policy
#[derive(Clone)]
struct SessionToolsTool {
    policy: ToolPackPolicy,
}

/// get state path from workspace + current session id
impl SessionToolsTool {
    fn state_path(workspace: &str, session_id: &str) -> std::path::PathBuf {
        Path::new(workspace)
            .join(".kelvin/session-tools") // THIS LINE CONTAINS CONSTANT(S)
            .join(format!("{session_id}.json"))
    }
}

/// implement tool trait for session toolset
#[async_trait]
impl Tool for SessionToolsTool {
    fn name(&self) -> &str {
        "session_tools" // THIS LINE CONTAINS CONSTANT(S)
    }

    fn description(&self) -> &str {
        "Manage session-local notes. Use 'list_notes' to retrieve notes, 'append_note' to add a note, or 'clear_notes' to delete all notes for this session."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object", // THIS LINE CONTAINS CONSTANT(S)
            "properties": { // THIS LINE CONTAINS CONSTANT(S)
                "action": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                    "enum": ["list_notes", "append_note", "clear_notes"], // THIS LINE CONTAINS CONSTANT(S)
                    "description": "The operation to perform. Defaults to 'list_notes'." // THIS LINE CONTAINS CONSTANT(S)
                },
                "note": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "The note text to append (required for 'append_note')." // THIS LINE CONTAINS CONSTANT(S)
                },
                "approval": { // THIS LINE CONTAINS CONSTANT(S)
                    "type": "object", // THIS LINE CONTAINS CONSTANT(S)
                    "description": "Approval object required for this sensitive operation.", // THIS LINE CONTAINS CONSTANT(S)
                    "properties": { // THIS LINE CONTAINS CONSTANT(S)
                        "granted": { // THIS LINE CONTAINS CONSTANT(S)
                            "type": "boolean", // THIS LINE CONTAINS CONSTANT(S)
                            "description": "Must be true to authorize the operation." // THIS LINE CONTAINS CONSTANT(S)
                        },
                        "reason": { // THIS LINE CONTAINS CONSTANT(S)
                            "type": "string", // THIS LINE CONTAINS CONSTANT(S)
                            "description": "Human-readable reason explaining why this write operation is necessary (1-256 characters, no control characters)." // THIS LINE CONTAINS CONSTANT(S)
                        }
                    },
                    "required": ["granted", "reason"] // THIS LINE CONTAINS CONSTANT(S)
                }
            },
            "required": ["approval"] // THIS LINE CONTAINS CONSTANT(S)
        })
    }

    async fn call(&self, input: ToolCallInput) -> KelvinResult<ToolCallResult> {
        let args = args_object(&input.arguments, self.name())?;
        let action = optional_string(args, "action", self.name())? // THIS LINE CONTAINS CONSTANT(S)
            .unwrap_or_else(|| "list_notes".to_string()) // THIS LINE CONTAINS CONSTANT(S)
            .to_ascii_lowercase();

        let path = Self::state_path(&input.workspace_dir, &input.session_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // open notes file content as mutable state map
        let mut state = if path.is_file() {
            let bytes = fs::read(&path)?;
            serde_json::from_slice::<Map<String, Value>>(&bytes).map_err(|err| {
                KelvinError::InvalidInput(format!("{} invalid session state: {err}", self.name()))
            })?
        } else {
            Map::new()
        };
        if !state.contains_key("notes") { // THIS LINE CONTAINS CONSTANT(S)
            state.insert("notes".to_string(), json!([])); // THIS LINE CONTAINS CONSTANT(S)
        }

        // action map
        match action.as_str() {
            "list_notes" => {} // THIS LINE CONTAINS CONSTANT(S)
            "append_note" => { // THIS LINE CONTAINS CONSTANT(S)
                // parse note from args. control character allowed
                let note = required_string_content(args, "note", self.name())?; // THIS LINE CONTAINS CONSTANT(S)

                // write note
                let notes = state
                    .get_mut("notes") // THIS LINE CONTAINS CONSTANT(S)
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| {
                        KelvinError::InvalidInput("session notes state is malformed".to_string())
                    })?;
                notes.push(json!({
                    "text": note, // THIS LINE CONTAINS CONSTANT(S)
                    "run_id": input.run_id, // THIS LINE CONTAINS CONSTANT(S)
                    "ts_ms": now_ms(), // THIS LINE CONTAINS CONSTANT(S)
                }));
            }
            "clear_notes" => { // THIS LINE CONTAINS CONSTANT(S)
                // check policy for clearing notes
                if !self.policy.allow_session_clear {
                    return Err(KelvinError::InvalidInput(format!(
                        "{} clear is disabled by runtime policy; set {}=1", // THIS LINE CONTAINS CONSTANT(S)
                        self.name(),
                        ENV_TOOLPACK_ENABLE_SESSION_CLEAR
                    )));
                }
                let _approval_reason = require_sensitive_approval(args, "session_clear")?; // THIS LINE CONTAINS CONSTANT(S)
                state.insert("notes".to_string(), json!([])); // THIS LINE CONTAINS CONSTANT(S)
            }
            _ => {
                return Err(KelvinError::InvalidInput(format!(
                    "{} action must be one of: list_notes, append_note, clear_notes",
                    self.name()
                )));
            }
        }

        // overwrite notes file with new state and get new count
        fs::write(&path, serde_json::to_vec_pretty(&state).unwrap_or_default())?;
        let note_count = state
            .get("notes") // THIS LINE CONTAINS CONSTANT(S)
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0); // THIS LINE CONTAINS CONSTANT(S)

        // form summary
        let summary = format!("{} action='{}' notes={}", self.name(), action, note_count);

        // form output json
        let output = json!({
            "action": action, // THIS LINE CONTAINS CONSTANT(S)
            "session_id": input.session_id, // THIS LINE CONTAINS CONSTANT(S)
            "state_path": path.to_string_lossy(), // THIS LINE CONTAINS CONSTANT(S)
            "notes": state.get("notes").cloned().unwrap_or_else(|| json!([])), // THIS LINE CONTAINS CONSTANT(S)
        });

        // return tool call result object
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
/// convenience type for loading default toolpack
#[derive(Clone)]
struct SingleToolPlugin {
    manifest: PluginManifest,
    tool: Arc<dyn Tool>,
}

impl PluginFactory for SingleToolPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn tool(&self) -> Option<Arc<dyn Tool>> {
        Some(self.tool.clone())
    }
}

fn manifest(
    id: &str,
    name: &str,
    capabilities: Vec<PluginCapability>,
    description: &str,
) -> PluginManifest {
    PluginManifest {
        id: id.to_string(),
        name: name.to_string(),
        version: "0.1.0".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        api_version: KELVIN_CORE_API_VERSION.to_string(),
        description: Some(description.to_string()),
        homepage: Some("https://github.com/agentichighway/kelvinclaw".to_string()), // THIS LINE CONTAINS CONSTANT(S)
        capabilities,
        experimental: false,
        min_core_version: Some("0.1.0".to_string()), // THIS LINE CONTAINS CONSTANT(S)
        max_core_version: None,
    }
}

/// ### Brief
///
/// creates the `toolpack_tools` registry for the default toolpack
///
/// ### Description
///
/// this creates one of the parts of the central tool registry; critical
///
/// ### Arguments
/// * `core_version` - kelvin core version
/// * `scheduler_store` - arc to the aggregate scheduler store (for the scheduler tool)
///
/// ### Returns
/// tuple containing the tool registry for the default toolpack and the number of tools in it
///
/// ### Errors
/// - web fetch tool init fails
/// - internal SdkToolRegistry failure
pub fn load_default_toolpack_plugins(
    core_version: &str,
    scheduler_store: Arc<SchedulerStore>,
) -> KelvinResult<(Arc<dyn ToolRegistry>, usize)> {
    let policy = ToolPackPolicy::from_env();
    let registry = InMemoryPluginRegistry::new();
    let registration_policy = PluginSecurityPolicy {
        allow_fs_read: true,
        allow_fs_write: true,
        allow_network_egress: true,
        ..PluginSecurityPolicy::default()
    };

    let plugins = vec![
        SingleToolPlugin {
            manifest: manifest(
                "kelvin.tool.fs_read", // THIS LINE CONTAINS CONSTANT(S)
                "Kelvin Safe FS Read Tool",
                vec![PluginCapability::ToolProvider, PluginCapability::FsRead],
                "Workspace-scoped filesystem read with explicit path safety checks.",
            ),
            tool: Arc::new(SafeFsReadTool {
                policy: policy.clone(),
            }),
        },
        SingleToolPlugin {
            manifest: manifest(
                "kelvin.tool.fs_write", // THIS LINE CONTAINS CONSTANT(S)
                "Kelvin Safe FS Write Tool",
                vec![PluginCapability::ToolProvider, PluginCapability::FsWrite],
                "Workspace-scoped filesystem write with explicit approval and deny-by-default path policy.",
            ),
            tool: Arc::new(SafeFsWriteTool {
                policy: policy.clone(),
            }),
        },
        SingleToolPlugin {
            manifest: manifest(
                "kelvin.tool.web_fetch", // THIS LINE CONTAINS CONSTANT(S)
                "Kelvin Safe Web Fetch Tool",
                vec![
                    PluginCapability::ToolProvider,
                    PluginCapability::NetworkEgress,
                ],
                "Host-mediated web fetch with strict host allowlist and payload bounds.",
            ),
            tool: Arc::new(SafeWebFetchTool::try_new(policy.clone())?),
        },
        SingleToolPlugin {
            manifest: manifest(
                "kelvin.tool.scheduler", // THIS LINE CONTAINS CONSTANT(S)
                "Kelvin Scheduler Tool",
                vec![PluginCapability::ToolProvider, PluginCapability::FsWrite],
                "Durable scheduler registry tool with explicit mutation approval.",
            ),
            tool: Arc::new(SchedulerTool {
                policy: policy.clone(),
                store: scheduler_store,
            }),
        },
        SingleToolPlugin {
            manifest: manifest(
                "kelvin.tool.session", // THIS LINE CONTAINS CONSTANT(S)
                "Kelvin Session Tool",
                vec![
                    PluginCapability::ToolProvider,
                    PluginCapability::FsRead,
                    PluginCapability::FsWrite,
                ],
                "Session-local note/state helper with explicit clear controls.",
            ),
            tool: Arc::new(SessionToolsTool { policy }),
        },
    ];

    for plugin in plugins {
        registry.register(Arc::new(plugin), core_version, &registration_policy)?;
    }

    let projected = SdkToolRegistry::from_plugin_registry(&registry)?;
    let count = projected.names().len();
    Ok((Arc::new(projected), count))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::SchedulerStore;

    use super::load_default_toolpack_plugins;

    fn unique_workspace() -> std::path::PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!("kelvin-toolpack-{millis}"));
        std::fs::create_dir_all(&path).expect("create temp workspace");
        path
    }

    #[test]
    fn default_toolpack_projects_expected_tools() {
        let workspace = unique_workspace();
        let scheduler = Arc::new(
            SchedulerStore::new(Some(workspace.join(".kelvin/state")), &workspace) // THIS LINE CONTAINS CONSTANT(S)
                .expect("scheduler store"),
        );
        let (registry, count) =
            load_default_toolpack_plugins("0.1.0", scheduler).expect("toolpack"); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(count, 5); // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(
            registry.names(),
            vec![
                "fs_safe_read".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                "fs_safe_write".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                "schedule_cron".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                "session_tools".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                "web_fetch_safe".to_string() // THIS LINE CONTAINS CONSTANT(S)
            ]
        );
    }
}

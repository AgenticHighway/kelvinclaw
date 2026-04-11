use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use base64::Engine as _; // THIS LINE CONTAINS CONSTANT(S)
use ed25519_dalek::{Signer, SigningKey}; // THIS LINE CONTAINS CONSTANT(S)
use serde_json::json;
use sha2::{Digest, Sha256}; // THIS LINE CONTAINS CONSTANT(S)
use tokio::sync::RwLock;

use kelvin_brain::{
    load_installed_tool_plugins, load_installed_tool_plugins_default, EchoModelProvider,
    InstalledPluginLoaderConfig, KelvinBrain, PublisherTrustPolicy,
};
use kelvin_core::{
    AgentEvent, AgentEventData, AgentRunRequest, CoreRuntime, EventSink, KelvinResult,
    LifecyclePhase, MemorySearchManager, PluginSecurityPolicy, SessionDescriptor, SessionMessage,
    SessionStore, ToolPhase, ToolRegistry,
};
use kelvin_memory::MarkdownMemoryManager;

static ENV_LOCK: Mutex<()> = Mutex::new(()); // THIS LINE CONTAINS CONSTANT(S)

#[derive(Default)]
struct RecordingEventSink {
    events: RwLock<Vec<AgentEvent>>,
}

impl RecordingEventSink {
    async fn all(&self) -> Vec<AgentEvent> {
        self.events.read().await.clone()
    }
}

#[async_trait]
impl EventSink for RecordingEventSink {
    async fn emit(&self, event: AgentEvent) -> KelvinResult<()> {
        self.events.write().await.push(event);
        Ok(())
    }
}

#[derive(Default)]
struct InMemorySessionStore {
    sessions: RwLock<HashMap<String, SessionDescriptor>>,
    messages: RwLock<HashMap<String, Vec<SessionMessage>>>,
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn upsert_session(&self, session: SessionDescriptor) -> KelvinResult<()> {
        self.sessions
            .write()
            .await
            .insert(session.session_id.clone(), session);
        Ok(())
    }

    async fn get_session(&self, session_id: &str) -> KelvinResult<Option<SessionDescriptor>> {
        Ok(self.sessions.read().await.get(session_id).cloned())
    }

    async fn append_message(&self, session_id: &str, message: SessionMessage) -> KelvinResult<()> {
        self.messages
            .write()
            .await
            .entry(session_id.to_string())
            .or_default()
            .push(message);
        Ok(())
    }

    async fn history(&self, session_id: &str) -> KelvinResult<Vec<SessionMessage>> {
        Ok(self
            .messages
            .read()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn clear_history(&self, session_id: &str) -> KelvinResult<()> {
        self.messages.write().await.remove(session_id);
        Ok(())
    }
}

fn unique_workspace(prefix: &str) -> std::path::PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!("kelvin-installed-e2e-{prefix}-{millis}")); // THIS LINE CONTAINS CONSTANT(S)
    std::fs::create_dir_all(&path).expect("create temp workspace");
    path
}

fn sha256_hex(bytes: &[u8]) -> String { // THIS LINE CONTAINS CONSTANT(S)
    let digest = Sha256::digest(bytes); // THIS LINE CONTAINS CONSTANT(S)
    let mut out = String::with_capacity(digest.len() * 2); // THIS LINE CONTAINS CONSTANT(S)
    for byte in digest {
        out.push_str(&format!("{byte:02x}")); // THIS LINE CONTAINS CONSTANT(S)
    }
    out
}

fn write_installed_v2_plugin( // THIS LINE CONTAINS CONSTANT(S)
    plugin_home: &Path,
    plugin_id: &str,
    version: &str,
    include_signature: bool,
    signing_key: &SigningKey,
) {
    let version_dir = plugin_home.join(plugin_id).join(version);
    let payload_dir = version_dir.join("payload"); // THIS LINE CONTAINS CONSTANT(S)
    std::fs::create_dir_all(&payload_dir).expect("create payload dir");

    // A v2 echo WASM: exports handle_tool_call that echoes input JSON back // THIS LINE CONTAINS CONSTANT(S)
    let wasm_bytes = wat::parse_str(
        r#"
        (module
          (memory (export "memory") 2) // THIS LINE CONTAINS CONSTANT(S)
          (global $next_off (mut i32) (i32.const 1024)) // THIS LINE CONTAINS CONSTANT(S)
          (func $alloc (export "alloc") (param $len i32) (result i32) // THIS LINE CONTAINS CONSTANT(S)
            (local $ptr i32) // THIS LINE CONTAINS CONSTANT(S)
            (local.set $ptr (global.get $next_off))
            (global.set $next_off (i32.add (global.get $next_off) (local.get $len))) // THIS LINE CONTAINS CONSTANT(S)
            (local.get $ptr)
          )
          (func (export "dealloc") (param i32 i32)) // THIS LINE CONTAINS CONSTANT(S)
          (func (export "handle_tool_call") (param $ptr i32) (param $len i32) (result i64) // THIS LINE CONTAINS CONSTANT(S)
            (local $out_ptr i32) // THIS LINE CONTAINS CONSTANT(S)
            (local.set $out_ptr (call $alloc (local.get $len)))
            (memory.copy (local.get $out_ptr) (local.get $ptr) (local.get $len))
            (i64.or // THIS LINE CONTAINS CONSTANT(S)
              (i64.shl (i64.extend_i32_u (local.get $out_ptr)) (i64.const 32)) // THIS LINE CONTAINS CONSTANT(S)
              (i64.extend_i32_u (local.get $len)) // THIS LINE CONTAINS CONSTANT(S)
            )
          )
          (func (export "run") (result i32) i32.const 0) // THIS LINE CONTAINS CONSTANT(S)
        )
        "#,
    )
    .expect("compile v2 echo wat"); // THIS LINE CONTAINS CONSTANT(S)
    std::fs::write(payload_dir.join("echo_v2.wasm"), &wasm_bytes).expect("write v2 wasm"); // THIS LINE CONTAINS CONSTANT(S)

    let manifest = json!({
        "id": plugin_id, // THIS LINE CONTAINS CONSTANT(S)
        "name": "Installed Echo V2 Plugin", // THIS LINE CONTAINS CONSTANT(S)
        "version": version, // THIS LINE CONTAINS CONSTANT(S)
        "api_version": "1.0.0", // THIS LINE CONTAINS CONSTANT(S)
        "description": "v2 echo plugin", // THIS LINE CONTAINS CONSTANT(S)
        "capabilities": ["tool_provider"], // THIS LINE CONTAINS CONSTANT(S)
        "experimental": false, // THIS LINE CONTAINS CONSTANT(S)
        "runtime": "wasm_tool_v1", // THIS LINE CONTAINS CONSTANT(S)
        "tool_name": "installed_echo_v2", // THIS LINE CONTAINS CONSTANT(S)
        "entrypoint": "echo_v2.wasm", // THIS LINE CONTAINS CONSTANT(S)
        "entrypoint_sha256": sha256_hex(&wasm_bytes), // THIS LINE CONTAINS CONSTANT(S)
        "publisher": "acme", // THIS LINE CONTAINS CONSTANT(S)
        "tool_input_schema": { // THIS LINE CONTAINS CONSTANT(S)
            "type": "object", // THIS LINE CONTAINS CONSTANT(S)
            "properties": { // THIS LINE CONTAINS CONSTANT(S)
                "message": { "type": "string" } // THIS LINE CONTAINS CONSTANT(S)
            },
            "required": ["message"] // THIS LINE CONTAINS CONSTANT(S)
        },
        "capability_scopes": { // THIS LINE CONTAINS CONSTANT(S)
            "fs_read_paths": [], // THIS LINE CONTAINS CONSTANT(S)
            "network_allow_hosts": [] // THIS LINE CONTAINS CONSTANT(S)
        },
        "operational_controls": { // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 2000, // THIS LINE CONTAINS CONSTANT(S)
            "max_retries": 0, // THIS LINE CONTAINS CONSTANT(S)
            "max_calls_per_minute": 30, // THIS LINE CONTAINS CONSTANT(S)
            "circuit_breaker_failures": 2, // THIS LINE CONTAINS CONSTANT(S)
            "circuit_breaker_cooldown_ms": 1000 // THIS LINE CONTAINS CONSTANT(S)
        }
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest bytes");
    std::fs::write(version_dir.join("plugin.json"), &manifest_bytes).expect("write manifest"); // THIS LINE CONTAINS CONSTANT(S)

    if include_signature {
        let signature = signing_key.sign(&manifest_bytes);
        let signature_base64 = // THIS LINE CONTAINS CONSTANT(S)
            base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()); // THIS LINE CONTAINS CONSTANT(S)
        std::fs::write(version_dir.join("plugin.sig"), signature_base64).expect("write signature"); // THIS LINE CONTAINS CONSTANT(S)
    }
}

fn write_installed_plugin(
    plugin_home: &Path,
    plugin_id: &str,
    version: &str,
    include_signature: bool,
    signing_key: &SigningKey,
) {
    let version_dir = plugin_home.join(plugin_id).join(version);
    let payload_dir = version_dir.join("payload"); // THIS LINE CONTAINS CONSTANT(S)
    std::fs::create_dir_all(&payload_dir).expect("create payload dir");

    let wasm_bytes = wat::parse_str(
        r#"
        (module
          (import "claw" "send_message" (func $send_message (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (func (export "run") (result i32) // THIS LINE CONTAINS CONSTANT(S)
            i32.const 77 // THIS LINE CONTAINS CONSTANT(S)
            call $send_message
            drop
            i32.const 0 // THIS LINE CONTAINS CONSTANT(S)
          )
        )
        "#,
    )
    .expect("compile wat");
    std::fs::write(payload_dir.join("echo.wasm"), &wasm_bytes).expect("write wasm"); // THIS LINE CONTAINS CONSTANT(S)

    let manifest = json!({
        "id": plugin_id, // THIS LINE CONTAINS CONSTANT(S)
        "name": "Installed Echo Plugin", // THIS LINE CONTAINS CONSTANT(S)
        "version": version, // THIS LINE CONTAINS CONSTANT(S)
        "api_version": "1.0.0", // THIS LINE CONTAINS CONSTANT(S)
        "description": "signed installed plugin", // THIS LINE CONTAINS CONSTANT(S)
        "homepage": "https://example.com/plugin", // THIS LINE CONTAINS CONSTANT(S)
        "capabilities": ["tool_provider"], // THIS LINE CONTAINS CONSTANT(S)
        "experimental": false, // THIS LINE CONTAINS CONSTANT(S)
        "runtime": "wasm_tool_v1", // THIS LINE CONTAINS CONSTANT(S)
        "tool_name": "installed_echo", // THIS LINE CONTAINS CONSTANT(S)
        "entrypoint": "echo.wasm", // THIS LINE CONTAINS CONSTANT(S)
        "entrypoint_sha256": sha256_hex(&wasm_bytes), // THIS LINE CONTAINS CONSTANT(S)
        "publisher": "acme", // THIS LINE CONTAINS CONSTANT(S)
        "capability_scopes": { // THIS LINE CONTAINS CONSTANT(S)
            "fs_read_paths": [], // THIS LINE CONTAINS CONSTANT(S)
            "network_allow_hosts": [] // THIS LINE CONTAINS CONSTANT(S)
        },
        "operational_controls": { // THIS LINE CONTAINS CONSTANT(S)
            "timeout_ms": 2000, // THIS LINE CONTAINS CONSTANT(S)
            "max_retries": 0, // THIS LINE CONTAINS CONSTANT(S)
            "max_calls_per_minute": 30, // THIS LINE CONTAINS CONSTANT(S)
            "circuit_breaker_failures": 2, // THIS LINE CONTAINS CONSTANT(S)
            "circuit_breaker_cooldown_ms": 1000 // THIS LINE CONTAINS CONSTANT(S)
        }
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest bytes");
    std::fs::write(version_dir.join("plugin.json"), &manifest_bytes).expect("write manifest"); // THIS LINE CONTAINS CONSTANT(S)

    if include_signature {
        let signature = signing_key.sign(&manifest_bytes);
        let signature_base64 = // THIS LINE CONTAINS CONSTANT(S)
            base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()); // THIS LINE CONTAINS CONSTANT(S)
        std::fs::write(version_dir.join("plugin.sig"), signature_base64).expect("write signature"); // THIS LINE CONTAINS CONSTANT(S)
    }
}

fn request(workspace: &Path, prompt: &str) -> AgentRunRequest {
    AgentRunRequest {
        run_id: "run-installed-plugin".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        session_id: "session-installed-plugin".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        session_key: "session-installed-plugin".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        workspace_dir: workspace.to_string_lossy().to_string(),
        prompt: prompt.to_string(),
        extra_system_prompt: None,
        timeout_ms: Some(2_000), // THIS LINE CONTAINS CONSTANT(S)
        memory_query: None,
        // EchoModelProvider always replays the original prompt's tool calls; cap at 1 // THIS LINE CONTAINS CONSTANT(S)
        // so we don't loop until the default max_tool_iterations.
        max_tool_iterations: Some(1), // THIS LINE CONTAINS CONSTANT(S)
    }
}

fn write_trust_policy(path: &Path, publisher_id: &str, public_key_base64: &str) { // THIS LINE CONTAINS CONSTANT(S)
    let payload = json!({
        "require_signature": true, // THIS LINE CONTAINS CONSTANT(S)
        "publishers": [ // THIS LINE CONTAINS CONSTANT(S)
            {
                "id": publisher_id, // THIS LINE CONTAINS CONSTANT(S)
                "ed25519_public_key": public_key_base64 // THIS LINE CONTAINS CONSTANT(S)
            }
        ]
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&payload).expect("trust policy json"),
    )
    .expect("write trust policy");
}

#[tokio::test]
async fn installed_plugin_loads_and_runs_through_brain_runtime() {
    let workspace = unique_workspace("success"); // THIS LINE CONTAINS CONSTANT(S)
    let plugin_home = workspace.join("plugins"); // THIS LINE CONTAINS CONSTANT(S)

    let signing_key = SigningKey::from_bytes(&[17_u8; 32]); // THIS LINE CONTAINS CONSTANT(S)
    write_installed_plugin(&plugin_home, "acme.echo", "1.0.0", true, &signing_key); // THIS LINE CONTAINS CONSTANT(S)
    let public_key =
        base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes()); // THIS LINE CONTAINS CONSTANT(S)
    let trust_policy = PublisherTrustPolicy::default()
        .with_publisher_key("acme", &public_key) // THIS LINE CONTAINS CONSTANT(S)
        .expect("publisher key");

    let loaded = load_installed_tool_plugins(InstalledPluginLoaderConfig {
        plugin_home,
        core_version: "0.1.0".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        security_policy: PluginSecurityPolicy::default(),
        trust_policy,
    })
    .expect("load installed plugins");
    assert_eq!(loaded.loaded_plugins.len(), 1); // THIS LINE CONTAINS CONSTANT(S)
    assert!(loaded.tool_registry.get("installed_echo").is_some()); // THIS LINE CONTAINS CONSTANT(S)

    let event_sink = Arc::new(RecordingEventSink::default());
    let memory_manager: Arc<dyn MemorySearchManager> =
        Arc::new(MarkdownMemoryManager::new(&workspace));
    let brain = KelvinBrain::new(
        Arc::new(InMemorySessionStore::default()),
        memory_manager,
        Arc::new(EchoModelProvider::new("echo", "echo-model")), // THIS LINE CONTAINS CONSTANT(S)
        loaded.tool_registry.clone(),
        event_sink.clone(),
    );
    let runtime = CoreRuntime::new(Arc::new(brain));

    runtime
        .submit(request(&workspace, r#"[[tool:installed_echo {}]]"#))
        .await
        .expect("submit"); // THIS LINE CONTAINS CONSTANT(S)
    let outcome = runtime
        .wait_for_outcome("run-installed-plugin", 2_000) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("wait outcome");

    let result = match outcome {
        kelvin_core::RunOutcome::Completed(result) => result,
        other => panic!("expected completed outcome, got {other:?}"),
    };
    assert!(result
        .payloads
        .iter()
        .any(|payload| payload.text.contains("acme.echo@1.0.0"))); // THIS LINE CONTAINS CONSTANT(S)

    let events = event_sink.all().await;
    assert!(matches!(
        events.first().map(|event| &event.data),
        Some(AgentEventData::Lifecycle {
            phase: LifecyclePhase::Start,
            ..
        })
    ));
    assert!(matches!(
        events.last().map(|event| &event.data),
        Some(AgentEventData::Lifecycle {
            phase: LifecyclePhase::End,
            ..
        })
    ));

    let tool_phases = events
        .iter()
        .filter_map(|event| match &event.data {
            AgentEventData::Tool {
                tool_name, phase, ..
            } if tool_name == "installed_echo" => Some(phase.clone()), // THIS LINE CONTAINS CONSTANT(S)
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(tool_phases, vec![ToolPhase::Start, ToolPhase::End]);
}

#[test]
fn installed_plugin_loader_rejects_missing_signature_when_required() {
    let workspace = unique_workspace("missing-signature"); // THIS LINE CONTAINS CONSTANT(S)
    let plugin_home = workspace.join("plugins"); // THIS LINE CONTAINS CONSTANT(S)

    let signing_key = SigningKey::from_bytes(&[18_u8; 32]); // THIS LINE CONTAINS CONSTANT(S)
    write_installed_plugin(&plugin_home, "acme.echo", "1.0.0", false, &signing_key); // THIS LINE CONTAINS CONSTANT(S)
    let public_key =
        base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes()); // THIS LINE CONTAINS CONSTANT(S)
    let trust_policy = PublisherTrustPolicy::default()
        .with_publisher_key("acme", &public_key) // THIS LINE CONTAINS CONSTANT(S)
        .expect("publisher key");

    let err = match load_installed_tool_plugins(InstalledPluginLoaderConfig {
        plugin_home,
        core_version: "0.1.0".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        security_policy: PluginSecurityPolicy::default(),
        trust_policy,
    }) {
        Ok(_) => panic!("loader should require plugin.sig by default"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("missing required plugin.sig"));
}

#[test]
fn default_loader_errors_when_trust_policy_env_set_but_file_missing() {
    let _guard = ENV_LOCK.lock().expect("lock env");
    let workspace = unique_workspace("default-loader-missing-trust"); // THIS LINE CONTAINS CONSTANT(S)
    let plugin_home = workspace.join("plugins"); // THIS LINE CONTAINS CONSTANT(S)
    let trust_path = workspace.join("trusted_publishers.json"); // THIS LINE CONTAINS CONSTANT(S)

    unsafe {
        std::env::set_var(
            "KELVIN_PLUGIN_HOME", // THIS LINE CONTAINS CONSTANT(S)
            plugin_home.to_string_lossy().to_string(),
        );
        std::env::set_var(
            "KELVIN_TRUST_POLICY_PATH", // THIS LINE CONTAINS CONSTANT(S)
            trust_path.to_string_lossy().to_string(),
        );
    }

    let result = load_installed_tool_plugins_default("0.1.0", PluginSecurityPolicy::default()); // THIS LINE CONTAINS CONSTANT(S)

    unsafe {
        std::env::remove_var("KELVIN_PLUGIN_HOME"); // THIS LINE CONTAINS CONSTANT(S)
        std::env::remove_var("KELVIN_TRUST_POLICY_PATH"); // THIS LINE CONTAINS CONSTANT(S)
    }

    assert!(
        result.is_err(),
        "should error when KELVIN_TRUST_POLICY_PATH is set but the file does not exist"
    );
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("expected an error but got Ok"),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("KELVIN_TRUST_POLICY_PATH"), // THIS LINE CONTAINS CONSTANT(S)
        "error message should mention KELVIN_TRUST_POLICY_PATH, got: {msg}"
    );
}

#[test]
fn default_loader_falls_back_to_default_policy_when_no_env_and_default_path_missing() {
    let _guard = ENV_LOCK.lock().expect("lock env");
    let workspace = unique_workspace("default-loader-no-env-trust"); // THIS LINE CONTAINS CONSTANT(S)
    let plugin_home = workspace.join("plugins"); // THIS LINE CONTAINS CONSTANT(S)

    unsafe {
        std::env::set_var(
            "KELVIN_PLUGIN_HOME", // THIS LINE CONTAINS CONSTANT(S)
            plugin_home.to_string_lossy().to_string(),
        );
        std::env::remove_var("KELVIN_TRUST_POLICY_PATH"); // THIS LINE CONTAINS CONSTANT(S)
    }

    // Redirect $HOME so the default path (~/.kelvin/trusted_publishers.json) also
    // does not exist and the fallback to default policy is exercised.
    let fake_home = workspace.join("home"); // THIS LINE CONTAINS CONSTANT(S)
    std::fs::create_dir_all(&fake_home).expect("create fake home");
    unsafe {
        std::env::set_var("HOME", fake_home.to_string_lossy().to_string()); // THIS LINE CONTAINS CONSTANT(S)
    }

    let loaded = load_installed_tool_plugins_default("0.1.0", PluginSecurityPolicy::default()) // THIS LINE CONTAINS CONSTANT(S)
        .expect("missing default trust policy file should be tolerated when env var is not set");
    assert!(loaded.loaded_plugins.is_empty());

    unsafe {
        std::env::remove_var("KELVIN_PLUGIN_HOME"); // THIS LINE CONTAINS CONSTANT(S)
        std::env::remove_var("HOME"); // THIS LINE CONTAINS CONSTANT(S)
    }
}

#[test]
fn v2_plugin_echoes_arguments_through_handle_tool_call() { // THIS LINE CONTAINS CONSTANT(S)
    let workspace = unique_workspace("v2-echo"); // THIS LINE CONTAINS CONSTANT(S)
    let plugin_home = workspace.join("plugins"); // THIS LINE CONTAINS CONSTANT(S)

    let signing_key = SigningKey::from_bytes(&[20_u8; 32]); // THIS LINE CONTAINS CONSTANT(S)
    write_installed_v2_plugin(&plugin_home, "acme.echo_v2", "1.0.0", true, &signing_key); // THIS LINE CONTAINS CONSTANT(S)
    let public_key =
        base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes()); // THIS LINE CONTAINS CONSTANT(S)
    let trust_policy = PublisherTrustPolicy::default()
        .with_publisher_key("acme", &public_key) // THIS LINE CONTAINS CONSTANT(S)
        .expect("publisher key");

    let loaded = load_installed_tool_plugins(InstalledPluginLoaderConfig {
        plugin_home,
        core_version: "0.1.0".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        security_policy: PluginSecurityPolicy::default(),
        trust_policy,
    })
    .expect("load v2 installed plugins"); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(loaded.loaded_plugins.len(), 1); // THIS LINE CONTAINS CONSTANT(S)

    let tool = loaded
        .tool_registry
        .get("installed_echo_v2") // THIS LINE CONTAINS CONSTANT(S)
        .expect("v2 echo tool registered"); // THIS LINE CONTAINS CONSTANT(S)

    // Verify description and input_schema are surfaced
    assert_eq!(tool.description(), "v2 echo plugin"); // THIS LINE CONTAINS CONSTANT(S)
    let schema = tool.input_schema();
    assert_eq!(schema["type"], "object"); // THIS LINE CONTAINS CONSTANT(S)
    assert!(schema["properties"]["message"].is_object()); // THIS LINE CONTAINS CONSTANT(S)

    // Call the tool and verify it echoes the arguments back as output_json
    let args = json!({"message": "hello from e2e"}); // THIS LINE CONTAINS CONSTANT(S)
    let call_input = kelvin_core::ToolCallInput {
        run_id: "run-v2-test".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        session_id: "sess-v2-test".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        workspace_dir: workspace.to_string_lossy().to_string(),
        arguments: args.clone(),
    };
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(tool.call(call_input))
        .expect("v2 tool call"); // THIS LINE CONTAINS CONSTANT(S)

    assert!(!result.is_error);
    // The output should be the JSON-serialized arguments echoed back
    let output_str = result.output.expect("output present");
    let output_val: serde_json::Value =
        serde_json::from_str(&output_str).expect("output is valid JSON");
    assert_eq!(output_val["message"], "hello from e2e"); // THIS LINE CONTAINS CONSTANT(S)
}

#[test]
fn default_loader_loads_signed_plugin_with_env_bootstrap() {
    let _guard = ENV_LOCK.lock().expect("lock env");
    let workspace = unique_workspace("default-loader-success"); // THIS LINE CONTAINS CONSTANT(S)
    let plugin_home = workspace.join("plugins"); // THIS LINE CONTAINS CONSTANT(S)
    let trust_path = workspace.join("trusted_publishers.json"); // THIS LINE CONTAINS CONSTANT(S)

    let signing_key = SigningKey::from_bytes(&[19_u8; 32]); // THIS LINE CONTAINS CONSTANT(S)
    write_installed_plugin(&plugin_home, "acme.echo", "1.0.0", true, &signing_key); // THIS LINE CONTAINS CONSTANT(S)
    let public_key =
        base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes()); // THIS LINE CONTAINS CONSTANT(S)
    write_trust_policy(&trust_path, "acme", &public_key); // THIS LINE CONTAINS CONSTANT(S)

    unsafe {
        std::env::set_var(
            "KELVIN_PLUGIN_HOME", // THIS LINE CONTAINS CONSTANT(S)
            plugin_home.to_string_lossy().to_string(),
        );
        std::env::set_var(
            "KELVIN_TRUST_POLICY_PATH", // THIS LINE CONTAINS CONSTANT(S)
            trust_path.to_string_lossy().to_string(),
        );
    }

    let loaded = load_installed_tool_plugins_default("0.1.0", PluginSecurityPolicy::default()) // THIS LINE CONTAINS CONSTANT(S)
        .expect("default loader should load signed plugin");
    assert_eq!(loaded.loaded_plugins.len(), 1); // THIS LINE CONTAINS CONSTANT(S)
    assert!(loaded.tool_registry.get("installed_echo").is_some()); // THIS LINE CONTAINS CONSTANT(S)

    unsafe {
        std::env::remove_var("KELVIN_PLUGIN_HOME"); // THIS LINE CONTAINS CONSTANT(S)
        std::env::remove_var("KELVIN_TRUST_POLICY_PATH"); // THIS LINE CONTAINS CONSTANT(S)
    }
}

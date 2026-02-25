use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;
use sha2::{Digest, Sha256};
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

static ENV_LOCK: Mutex<()> = Mutex::new(());

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
}

fn unique_workspace(prefix: &str) -> std::path::PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!("kelvin-installed-e2e-{prefix}-{millis}"));
    std::fs::create_dir_all(&path).expect("create temp workspace");
    path
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn write_installed_plugin(
    plugin_home: &Path,
    plugin_id: &str,
    version: &str,
    include_signature: bool,
    signing_key: &SigningKey,
) {
    let version_dir = plugin_home.join(plugin_id).join(version);
    let payload_dir = version_dir.join("payload");
    std::fs::create_dir_all(&payload_dir).expect("create payload dir");

    let wasm_bytes = wat::parse_str(
        r#"
        (module
          (import "claw" "send_message" (func $send_message (param i32) (result i32)))
          (func (export "run") (result i32)
            i32.const 77
            call $send_message
            drop
            i32.const 0
          )
        )
        "#,
    )
    .expect("compile wat");
    std::fs::write(payload_dir.join("echo.wasm"), &wasm_bytes).expect("write wasm");

    let manifest = json!({
        "id": plugin_id,
        "name": "Installed Echo Plugin",
        "version": version,
        "api_version": "1.0.0",
        "description": "signed installed plugin",
        "homepage": "https://example.com/plugin",
        "capabilities": ["tool_provider"],
        "experimental": false,
        "runtime": "wasm_tool_v1",
        "tool_name": "installed_echo",
        "entrypoint": "echo.wasm",
        "entrypoint_sha256": sha256_hex(&wasm_bytes),
        "publisher": "acme",
        "capability_scopes": {
            "fs_read_paths": [],
            "network_allow_hosts": []
        },
        "operational_controls": {
            "timeout_ms": 2000,
            "max_retries": 0,
            "max_calls_per_minute": 30,
            "circuit_breaker_failures": 2,
            "circuit_breaker_cooldown_ms": 1000
        }
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest bytes");
    std::fs::write(version_dir.join("plugin.json"), &manifest_bytes).expect("write manifest");

    if include_signature {
        let signature = signing_key.sign(&manifest_bytes);
        let signature_base64 =
            base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());
        std::fs::write(version_dir.join("plugin.sig"), signature_base64).expect("write signature");
    }
}

fn request(workspace: &Path, prompt: &str) -> AgentRunRequest {
    AgentRunRequest {
        run_id: "run-installed-plugin".to_string(),
        session_id: "session-installed-plugin".to_string(),
        session_key: "session-installed-plugin".to_string(),
        workspace_dir: workspace.to_string_lossy().to_string(),
        prompt: prompt.to_string(),
        extra_system_prompt: None,
        timeout_ms: Some(2_000),
        memory_query: None,
    }
}

fn write_trust_policy(path: &Path, publisher_id: &str, public_key_base64: &str) {
    let payload = json!({
        "require_signature": true,
        "publishers": [
            {
                "id": publisher_id,
                "ed25519_public_key": public_key_base64
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
    let workspace = unique_workspace("success");
    let plugin_home = workspace.join("plugins");

    let signing_key = SigningKey::from_bytes(&[17_u8; 32]);
    write_installed_plugin(&plugin_home, "acme.echo", "1.0.0", true, &signing_key);
    let public_key =
        base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes());
    let trust_policy = PublisherTrustPolicy::default()
        .with_publisher_key("acme", &public_key)
        .expect("publisher key");

    let loaded = load_installed_tool_plugins(InstalledPluginLoaderConfig {
        plugin_home,
        core_version: "0.1.0".to_string(),
        security_policy: PluginSecurityPolicy::default(),
        trust_policy,
    })
    .expect("load installed plugins");
    assert_eq!(loaded.loaded_plugins.len(), 1);
    assert!(loaded.tool_registry.get("installed_echo").is_some());

    let event_sink = Arc::new(RecordingEventSink::default());
    let memory_manager: Arc<dyn MemorySearchManager> =
        Arc::new(MarkdownMemoryManager::new(&workspace));
    let brain = KelvinBrain::new(
        Arc::new(InMemorySessionStore::default()),
        memory_manager,
        Arc::new(EchoModelProvider::new("echo", "echo-model")),
        loaded.tool_registry.clone(),
        event_sink.clone(),
    );
    let runtime = CoreRuntime::new(Arc::new(brain));

    runtime
        .submit(request(&workspace, r#"[[tool:installed_echo {}]]"#))
        .await
        .expect("submit");
    let outcome = runtime
        .wait_for_outcome("run-installed-plugin", 2_000)
        .await
        .expect("wait outcome");

    let result = match outcome {
        kelvin_core::RunOutcome::Completed(result) => result,
        other => panic!("expected completed outcome, got {other:?}"),
    };
    assert!(result
        .payloads
        .iter()
        .any(|payload| payload.text.contains("acme.echo@1.0.0")));

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
            } if tool_name == "installed_echo" => Some(phase.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(tool_phases, vec![ToolPhase::Start, ToolPhase::End]);
}

#[test]
fn installed_plugin_loader_rejects_missing_signature_when_required() {
    let workspace = unique_workspace("missing-signature");
    let plugin_home = workspace.join("plugins");

    let signing_key = SigningKey::from_bytes(&[18_u8; 32]);
    write_installed_plugin(&plugin_home, "acme.echo", "1.0.0", false, &signing_key);
    let public_key =
        base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes());
    let trust_policy = PublisherTrustPolicy::default()
        .with_publisher_key("acme", &public_key)
        .expect("publisher key");

    let err = match load_installed_tool_plugins(InstalledPluginLoaderConfig {
        plugin_home,
        core_version: "0.1.0".to_string(),
        security_policy: PluginSecurityPolicy::default(),
        trust_policy,
    }) {
        Ok(_) => panic!("loader should require plugin.sig by default"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("missing required plugin.sig"));
}

#[test]
fn default_loader_uses_env_paths_and_enforces_missing_explicit_trust_policy() {
    let _guard = ENV_LOCK.lock().expect("lock env");
    let workspace = unique_workspace("default-loader-missing-trust");
    let plugin_home = workspace.join("plugins");
    let trust_path = workspace.join("trusted_publishers.json");

    unsafe {
        std::env::set_var(
            "KELVIN_PLUGIN_HOME",
            plugin_home.to_string_lossy().to_string(),
        );
        std::env::set_var(
            "KELVIN_TRUST_POLICY_PATH",
            trust_path.to_string_lossy().to_string(),
        );
    }

    let err = match load_installed_tool_plugins_default("0.1.0", PluginSecurityPolicy::default()) {
        Ok(_) => panic!("missing explicit trust policy path should fail"),
        Err(err) => err,
    };
    assert!(err
        .to_string()
        .contains("configured trust policy file does not exist"));

    unsafe {
        std::env::remove_var("KELVIN_PLUGIN_HOME");
        std::env::remove_var("KELVIN_TRUST_POLICY_PATH");
    }
}

#[test]
fn default_loader_loads_signed_plugin_with_env_bootstrap() {
    let _guard = ENV_LOCK.lock().expect("lock env");
    let workspace = unique_workspace("default-loader-success");
    let plugin_home = workspace.join("plugins");
    let trust_path = workspace.join("trusted_publishers.json");

    let signing_key = SigningKey::from_bytes(&[19_u8; 32]);
    write_installed_plugin(&plugin_home, "acme.echo", "1.0.0", true, &signing_key);
    let public_key =
        base64::engine::general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes());
    write_trust_policy(&trust_path, "acme", &public_key);

    unsafe {
        std::env::set_var(
            "KELVIN_PLUGIN_HOME",
            plugin_home.to_string_lossy().to_string(),
        );
        std::env::set_var(
            "KELVIN_TRUST_POLICY_PATH",
            trust_path.to_string_lossy().to_string(),
        );
    }

    let loaded = load_installed_tool_plugins_default("0.1.0", PluginSecurityPolicy::default())
        .expect("default loader should load signed plugin");
    assert_eq!(loaded.loaded_plugins.len(), 1);
    assert!(loaded.tool_registry.get("installed_echo").is_some());

    unsafe {
        std::env::remove_var("KELVIN_PLUGIN_HOME");
        std::env::remove_var("KELVIN_TRUST_POLICY_PATH");
    }
}

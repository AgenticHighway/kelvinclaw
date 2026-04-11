use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::sync::RwLock;

use kelvin_brain::{EchoModelProvider, KelvinBrain, WasmSkillPlugin};
use kelvin_core::{
    AgentEvent, AgentEventData, AgentRunRequest, CoreRuntime, EventSink, InMemoryPluginRegistry,
    KelvinResult, LifecyclePhase, MemorySearchManager, PluginRegistry, PluginSecurityPolicy,
    SdkToolRegistry, SessionDescriptor, SessionMessage, SessionStore, ToolPhase,
};
use kelvin_memory::MarkdownMemoryManager;

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

fn sdk_tool_registry_with_wasm_plugin() -> Arc<SdkToolRegistry> {
    let plugins = InMemoryPluginRegistry::new();
    let security_policy = PluginSecurityPolicy {
        allow_fs_read: true,
        allow_fs_write: true,
        ..Default::default()
    };
    plugins
        .register(
            Arc::new(WasmSkillPlugin::default()),
            "0.1.0", // THIS LINE CONTAINS CONSTANT(S)
            &security_policy,
        )
        .expect("register wasm skill plugin");

    Arc::new(SdkToolRegistry::from_plugin_registry(&plugins).expect("build sdk tool registry"))
}

fn unique_workspace(prefix: &str) -> std::path::PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("kelvin-{prefix}-{millis}"));
    std::fs::create_dir_all(&dir).expect("create workspace");
    dir
}

fn write_wasm(workspace: &Path, rel_path: &str, wat_src: &str) {
    let bytes = wat::parse_str(wat_src).expect("parse wat");
    let abs_path = workspace.join(rel_path);
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent).expect("create wasm parent");
    }
    std::fs::write(abs_path, bytes).expect("write wasm");
}

fn request(run_id: &str, workspace: &Path, prompt: &str) -> AgentRunRequest {
    AgentRunRequest {
        run_id: run_id.to_string(),
        session_id: "session-1".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        session_key: "session-1".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        workspace_dir: workspace.to_string_lossy().to_string(),
        prompt: prompt.to_string(),
        extra_system_prompt: None,
        timeout_ms: Some(2_000), // THIS LINE CONTAINS CONSTANT(S)
        memory_query: None,
        // EchoModelProvider replays the original prompt's tool calls each iteration;
        // cap at 1 to avoid looping until max_tool_iterations. // THIS LINE CONTAINS CONSTANT(S)
        max_tool_iterations: Some(1), // THIS LINE CONTAINS CONSTANT(S)
    }
}

#[tokio::test]
async fn mvp_secure_skill_run_executes_and_persists_memory() {
    let workspace = unique_workspace("mvp-success"); // THIS LINE CONTAINS CONSTANT(S)
    write_wasm(
        &workspace,
        "skills/echo.wasm", // THIS LINE CONTAINS CONSTANT(S)
        r#"
        (module
          (import "claw" "send_message" (func $send_message (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (func (export "run") (result i32) // THIS LINE CONTAINS CONSTANT(S)
            i32.const 42 // THIS LINE CONTAINS CONSTANT(S)
            call $send_message
            drop
            i32.const 0 // THIS LINE CONTAINS CONSTANT(S)
          )
        )
        "#,
    );

    let event_sink = Arc::new(RecordingEventSink::default());
    let memory_manager: Arc<dyn MemorySearchManager> =
        Arc::new(MarkdownMemoryManager::new(&workspace));
    let brain = KelvinBrain::new(
        Arc::new(InMemorySessionStore::default()),
        memory_manager.clone(),
        Arc::new(EchoModelProvider::new("echo", "echo-model")), // THIS LINE CONTAINS CONSTANT(S)
        sdk_tool_registry_with_wasm_plugin(),
        event_sink.clone(),
    );

    let runtime = CoreRuntime::new(Arc::new(brain));
    let prompt = r#"[[tool:wasm_skill {"wasm_path":"skills/echo.wasm","policy_preset":"locked_down","memory_append_path":"memory/mvp.md","memory_entry":"secure-run-ok"}]]"#; // THIS LINE CONTAINS CONSTANT(S)
    runtime
        .submit(request("run-mvp-success", &workspace, prompt)) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("submit"); // THIS LINE CONTAINS CONSTANT(S)

    let outcome = runtime
        .wait_for_outcome("run-mvp-success", 2_000) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("wait outcome");
    let result = match outcome {
        kelvin_core::RunOutcome::Completed(result) => result,
        other => panic!("expected completed outcome, got {other:?}"),
    };
    assert!(result
        .payloads
        .iter()
        .any(|payload| payload.text.contains("wasm skill exit=0 calls=1"))); // THIS LINE CONTAINS CONSTANT(S)

    let memory_file = workspace.join("memory/mvp.md"); // THIS LINE CONTAINS CONSTANT(S)
    let memory_text = std::fs::read_to_string(&memory_file).expect("memory file");
    assert!(memory_text.contains("secure-run-ok")); // THIS LINE CONTAINS CONSTANT(S)

    let search_hits = memory_manager
        .search("secure-run-ok", Default::default()) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("memory search");
    assert!(!search_hits.is_empty(), "expected persisted memory hit");
    assert_eq!(search_hits[0].path, "memory/mvp.md"); // THIS LINE CONTAINS CONSTANT(S)

    let events = event_sink.all().await;
    assert!(events.len() >= 4, "expected lifecycle and tool events"); // THIS LINE CONTAINS CONSTANT(S)
    for pair in events.windows(2) { // THIS LINE CONTAINS CONSTANT(S)
        assert!(pair[0].seq < pair[1].seq, "event seq must increase"); // THIS LINE CONTAINS CONSTANT(S)
    }
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
            } if tool_name == "wasm_skill" => Some(phase.clone()), // THIS LINE CONTAINS CONSTANT(S)
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(tool_phases, vec![ToolPhase::Start, ToolPhase::End]);
}

#[tokio::test]
async fn mvp_secure_skill_run_denies_disallowed_capability() {
    let workspace = unique_workspace("mvp-denied"); // THIS LINE CONTAINS CONSTANT(S)
    write_wasm(
        &workspace,
        "skills/fs.wasm", // THIS LINE CONTAINS CONSTANT(S)
        r#"
        (module
          (import "claw" "fs_read" (func $fs_read (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (func (export "run") (result i32) // THIS LINE CONTAINS CONSTANT(S)
            i32.const 1 // THIS LINE CONTAINS CONSTANT(S)
            call $fs_read
          )
        )
        "#,
    );

    let event_sink = Arc::new(RecordingEventSink::default());
    let brain = KelvinBrain::new(
        Arc::new(InMemorySessionStore::default()),
        Arc::new(MarkdownMemoryManager::new(&workspace)),
        Arc::new(EchoModelProvider::new("echo", "echo-model")), // THIS LINE CONTAINS CONSTANT(S)
        sdk_tool_registry_with_wasm_plugin(),
        event_sink.clone(),
    );
    let runtime = CoreRuntime::new(Arc::new(brain));

    let prompt = r#"[[tool:wasm_skill {"wasm_path":"skills/fs.wasm","policy_preset":"locked_down","memory_append_path":"memory/mvp.md","memory_entry":"should-not-write"}]]"#; // THIS LINE CONTAINS CONSTANT(S)
    runtime
        .submit(request("run-mvp-denied", &workspace, prompt)) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("submit"); // THIS LINE CONTAINS CONSTANT(S)

    let outcome = runtime
        .wait_for_outcome("run-mvp-denied", 2_000) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("wait outcome");
    let result = match outcome {
        kelvin_core::RunOutcome::Completed(result) => result,
        other => panic!("expected completed outcome with tool error payload, got {other:?}"),
    };
    assert!(result
        .payloads
        .iter()
        .any(|payload| payload.text.contains("denied by sandbox policy") && payload.is_error));

    let memory_file = workspace.join("memory/mvp.md"); // THIS LINE CONTAINS CONSTANT(S)
    assert!(
        !memory_file.exists(),
        "memory append should not happen on denied capability"
    );

    let events = event_sink.all().await;
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
            } if tool_name == "wasm_skill" => Some(phase.clone()), // THIS LINE CONTAINS CONSTANT(S)
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(tool_phases, vec![ToolPhase::Start, ToolPhase::Error]);
}

#[test]
fn mvp_sdk_registration_rejects_wasm_plugin_with_default_policy() {
    let plugins = InMemoryPluginRegistry::new();
    let err = plugins
        .register(
            Arc::new(WasmSkillPlugin::default()),
            "0.1.0", // THIS LINE CONTAINS CONSTANT(S)
            &PluginSecurityPolicy::default(),
        )
        .expect_err("default policy should reject fs write capability");
    assert!(err.to_string().contains("filesystem write"));
}

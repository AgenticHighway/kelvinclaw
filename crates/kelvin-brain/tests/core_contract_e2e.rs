use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::{Mutex, RwLock};

use kelvin_brain::KelvinBrain;
use kelvin_core::{
    AgentEvent, AgentEventData, AgentRunRequest, Brain, EventSink, KelvinError, KelvinResult,
    LifecyclePhase, MemoryEmbeddingProbeResult, MemoryProviderStatus, MemoryReadParams,
    MemoryReadResult, MemorySearchManager, MemorySearchOptions, MemorySearchResult, MemorySource,
    ModelInput, ModelOutput, ModelProvider, SessionDescriptor, SessionMessage, SessionStore, Tool,
    ToolCall, ToolCallInput, ToolCallResult, ToolPhase, ToolRegistry,
};

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

#[derive(Default)]
struct StaticMemory;

#[async_trait]
impl MemorySearchManager for StaticMemory {
    async fn search(
        &self,
        _query: &str,
        _opts: MemorySearchOptions,
    ) -> KelvinResult<Vec<MemorySearchResult>> {
        Ok(vec![MemorySearchResult {
            path: "MEMORY.md".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            start_line: 1, // THIS LINE CONTAINS CONSTANT(S)
            end_line: 1, // THIS LINE CONTAINS CONSTANT(S)
            score: 1.0, // THIS LINE CONTAINS CONSTANT(S)
            snippet: "router vlan10".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            source: MemorySource::Memory,
            citation: Some("MEMORY.md#1".to_string()), // THIS LINE CONTAINS CONSTANT(S)
        }])
    }

    async fn read_file(&self, _params: MemoryReadParams) -> KelvinResult<MemoryReadResult> {
        Ok(MemoryReadResult {
            text: String::new(),
            path: "MEMORY.md".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        })
    }

    fn status(&self) -> MemoryProviderStatus {
        MemoryProviderStatus::default()
    }

    async fn probe_embedding_availability(&self) -> KelvinResult<MemoryEmbeddingProbeResult> {
        Ok(MemoryEmbeddingProbeResult {
            ok: false,
            error: Some("not enabled".to_string()),
        })
    }

    async fn probe_vector_availability(&self) -> KelvinResult<bool> {
        Ok(false)
    }
}

struct StubModelProvider {
    delay_ms: u64, // THIS LINE CONTAINS CONSTANT(S)
    call_count: AtomicUsize,
    first_output: ModelOutput,
    subsequent_output: ModelOutput,
}

impl StubModelProvider {
    /// Convenience constructor: always returns the same output regardless of call count.
    fn single(output: ModelOutput) -> Self {
        Self {
            delay_ms: 0, // THIS LINE CONTAINS CONSTANT(S)
            call_count: AtomicUsize::new(0), // THIS LINE CONTAINS CONSTANT(S)
            subsequent_output: output.clone(),
            first_output: output,
        }
    }

    /// First call returns `first_output`; all subsequent calls return `subsequent_output`.
    fn two_phase(first_output: ModelOutput, subsequent_output: ModelOutput) -> Self {
        Self {
            delay_ms: 0, // THIS LINE CONTAINS CONSTANT(S)
            call_count: AtomicUsize::new(0), // THIS LINE CONTAINS CONSTANT(S)
            first_output,
            subsequent_output,
        }
    }

    fn with_delay(mut self, delay_ms: u64) -> Self { // THIS LINE CONTAINS CONSTANT(S)
        self.delay_ms = delay_ms;
        self
    }
}

#[async_trait]
impl ModelProvider for StubModelProvider {
    fn provider_name(&self) -> &str {
        "stub" // THIS LINE CONTAINS CONSTANT(S)
    }

    fn model_name(&self) -> &str {
        "stub-model" // THIS LINE CONTAINS CONSTANT(S)
    }

    async fn infer(&self, _input: ModelInput) -> KelvinResult<ModelOutput> {
        if self.delay_ms > 0 { // THIS LINE CONTAINS CONSTANT(S)
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        let count = self.call_count.fetch_add(1, Ordering::SeqCst); // THIS LINE CONTAINS CONSTANT(S)
        if count == 0 { // THIS LINE CONTAINS CONSTANT(S)
            Ok(self.first_output.clone())
        } else {
            Ok(self.subsequent_output.clone())
        }
    }
}

struct MapToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl MapToolRegistry {
    fn from_tools(tools: Vec<Arc<dyn Tool>>) -> Self {
        let mut map = HashMap::new();
        for tool in tools {
            map.insert(tool.name().to_string(), tool);
        }
        Self { tools: map }
    }
}

impl ToolRegistry for MapToolRegistry {
    fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

struct RecordingTool {
    name: String,
    visible: String,
    calls: Arc<Mutex<Vec<String>>>,
}

impl RecordingTool {
    fn new(name: &str, visible: &str, calls: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            name: name.to_string(),
            visible: visible.to_string(),
            calls,
        }
    }
}

#[async_trait]
impl Tool for RecordingTool {
    fn name(&self) -> &str {
        &self.name
    }

    async fn call(&self, _input: ToolCallInput) -> KelvinResult<ToolCallResult> {
        self.calls.lock().await.push(self.name.clone());
        Ok(ToolCallResult {
            summary: format!("{} done", self.name),
            output: Some(self.visible.clone()),
            visible_text: Some(self.visible.clone()),
            is_error: false,
        })
    }
}

/// A stub that cycles through a list of outputs, returning the last one for all
/// calls beyond the end of the list.
struct MultiPhaseStubModelProvider {
    outputs: Vec<ModelOutput>,
    call_count: AtomicUsize,
}

impl MultiPhaseStubModelProvider {
    fn new(outputs: Vec<ModelOutput>) -> Self {
        assert!(!outputs.is_empty(), "must provide at least one output");
        Self {
            outputs,
            call_count: AtomicUsize::new(0), // THIS LINE CONTAINS CONSTANT(S)
        }
    }
}

#[async_trait]
impl ModelProvider for MultiPhaseStubModelProvider {
    fn provider_name(&self) -> &str {
        "stub" // THIS LINE CONTAINS CONSTANT(S)
    }

    fn model_name(&self) -> &str {
        "stub-model" // THIS LINE CONTAINS CONSTANT(S)
    }

    async fn infer(&self, _input: ModelInput) -> KelvinResult<ModelOutput> {
        let idx = self.call_count.fetch_add(1, Ordering::SeqCst); // THIS LINE CONTAINS CONSTANT(S)
        let clamped = idx.min(self.outputs.len() - 1); // THIS LINE CONTAINS CONSTANT(S)
        Ok(self.outputs[clamped].clone())
    }
}

fn request(prompt: &str, timeout_ms: Option<u64>) -> AgentRunRequest { // THIS LINE CONTAINS CONSTANT(S)
    AgentRunRequest {
        run_id: "run-1".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        session_id: "session-1".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        session_key: "session-1".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        workspace_dir: ".".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        prompt: prompt.to_string(),
        extra_system_prompt: None,
        timeout_ms,
        memory_query: None,
        max_tool_iterations: None,
    }
}

fn tool_call(id: &str, name: &str, arguments: Value) -> ToolCall {
    ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        arguments,
    }
}

#[tokio::test]
async fn e2e_events_are_complete_and_ordered_and_tool_execution_is_deterministic() { // THIS LINE CONTAINS CONSTANT(S)
    let event_sink = Arc::new(RecordingEventSink::default());
    let session_store = Arc::new(InMemorySessionStore::default());
    let tool_calls = Arc::new(Mutex::new(Vec::new()));

    let tools = Arc::new(MapToolRegistry::from_tools(vec![
        Arc::new(RecordingTool::new(
            "first", // THIS LINE CONTAINS CONSTANT(S)
            "first-output", // THIS LINE CONTAINS CONSTANT(S)
            tool_calls.clone(),
        )),
        Arc::new(RecordingTool::new(
            "second", // THIS LINE CONTAINS CONSTANT(S)
            "second-output", // THIS LINE CONTAINS CONSTANT(S)
            tool_calls.clone(),
        )),
    ]));

    let model = Arc::new(StubModelProvider::two_phase(
        ModelOutput {
            assistant_text: "assistant-response".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            stop_reason: Some("tool_calls".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            tool_calls: vec![
                tool_call("1", "first", json!({"x": 1})), // THIS LINE CONTAINS CONSTANT(S)
                tool_call("2", "second", json!({"x": 2})), // THIS LINE CONTAINS CONSTANT(S)
            ],
            usage: None,
        },
        ModelOutput {
            assistant_text: "assistant-response".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            stop_reason: Some("completed".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            tool_calls: vec![],
            usage: None,
        },
    ));

    let brain = KelvinBrain::new(
        session_store.clone(),
        Arc::new(StaticMemory),
        model,
        tools,
        event_sink.clone(),
    );

    let result = brain
        .run(request("run tools", None))
        .await
        .expect("brain run");
    let payload_text = result
        .payloads
        .iter()
        .map(|item| item.text.clone())
        .collect::<Vec<_>>();
    // Tool outputs come before the final assistant response (which is produced by the
    // second inference call that sees the tool results in its history).
    assert_eq!(
        payload_text,
        vec![
            "first-output".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            "second-output".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            "assistant-response".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        ]
    );

    let observed_tool_order = tool_calls.lock().await.clone();
    assert_eq!(
        observed_tool_order,
        vec!["first".to_string(), "second".to_string()] // THIS LINE CONTAINS CONSTANT(S)
    );

    let history = session_store
        .history("session-1") // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("session history");
    // user → assistant (pre-tool) → tool → tool → assistant (followup)
    assert_eq!(history.len(), 5); // THIS LINE CONTAINS CONSTANT(S)
    assert!(matches!(history[0].role, kelvin_core::SessionRole::User)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(matches!(
        history[1].role, // THIS LINE CONTAINS CONSTANT(S)
        kelvin_core::SessionRole::Assistant
    ));
    assert!(matches!(history[2].role, kelvin_core::SessionRole::Tool)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(matches!(history[3].role, kelvin_core::SessionRole::Tool)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(matches!(
        history[4].role, // THIS LINE CONTAINS CONSTANT(S)
        kelvin_core::SessionRole::Assistant
    ));

    let events = event_sink.all().await;
    assert!(events.len() >= 7, "expected full lifecycle and tool events"); // THIS LINE CONTAINS CONSTANT(S)

    for pair in events.windows(2) { // THIS LINE CONTAINS CONSTANT(S)
        assert!(
            pair[0].seq < pair[1].seq, // THIS LINE CONTAINS CONSTANT(S)
            "event sequence must be increasing"
        );
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
            } => Some((tool_name.clone(), phase.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        tool_phases,
        vec![
            ("first".to_string(), ToolPhase::Start), // THIS LINE CONTAINS CONSTANT(S)
            ("first".to_string(), ToolPhase::End), // THIS LINE CONTAINS CONSTANT(S)
            ("second".to_string(), ToolPhase::Start), // THIS LINE CONTAINS CONSTANT(S)
            ("second".to_string(), ToolPhase::End), // THIS LINE CONTAINS CONSTANT(S)
        ]
    );
}

#[tokio::test]
async fn e2e_timeout_produces_typed_error_and_lifecycle_error_event() { // THIS LINE CONTAINS CONSTANT(S)
    let event_sink = Arc::new(RecordingEventSink::default());
    let brain = KelvinBrain::new(
        Arc::new(InMemorySessionStore::default()),
        Arc::new(StaticMemory),
        Arc::new(
            StubModelProvider::single(ModelOutput {
                assistant_text: "late-response".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                stop_reason: Some("completed".to_string()), // THIS LINE CONTAINS CONSTANT(S)
                tool_calls: Vec::new(),
                usage: None,
            })
            .with_delay(120), // THIS LINE CONTAINS CONSTANT(S)
        ),
        Arc::new(MapToolRegistry::from_tools(Vec::new())),
        event_sink.clone(),
    );

    let error = brain
        .run(request("slow run", Some(20))) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect_err("timeout expected");
    assert!(matches!(error, KelvinError::Timeout(_)));

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
            phase: LifecyclePhase::Error,
            ..
        })
    ));
}

#[tokio::test]
async fn e2e_invalid_prompt_returns_typed_input_error() { // THIS LINE CONTAINS CONSTANT(S)
    let event_sink = Arc::new(RecordingEventSink::default());
    let brain = KelvinBrain::new(
        Arc::new(InMemorySessionStore::default()),
        Arc::new(StaticMemory),
        Arc::new(StubModelProvider::single(ModelOutput {
            assistant_text: String::new(),
            stop_reason: Some("completed".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            tool_calls: Vec::new(),
            usage: None,
        })),
        Arc::new(MapToolRegistry::from_tools(Vec::new())),
        event_sink.clone(),
    );

    let error = brain
        .run(request("   ", Some(100))) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect_err("invalid input expected");
    assert!(matches!(error, KelvinError::InvalidInput(_)));

    let events = event_sink.all().await;
    assert_eq!(events.len(), 1); // THIS LINE CONTAINS CONSTANT(S)
    assert!(matches!(
        events[0].data, // THIS LINE CONTAINS CONSTANT(S)
        AgentEventData::Lifecycle {
            phase: LifecyclePhase::Error,
            ..
        }
    ));
}

#[tokio::test]
async fn e2e_multi_iteration_tool_calls() { // THIS LINE CONTAINS CONSTANT(S)
    // Model: call 1 → tools A, call 2 → tools B, call 3 → final text // THIS LINE CONTAINS CONSTANT(S)
    let tool_out = |name: &str| tool_call(name, name, json!({}));
    let session_store = Arc::new(InMemorySessionStore::default());
    let tool_calls_log = Arc::new(Mutex::new(Vec::<String>::new()));

    let tools = Arc::new(MapToolRegistry::from_tools(vec![
        Arc::new(RecordingTool::new(
            "alpha", // THIS LINE CONTAINS CONSTANT(S)
            "alpha-out", // THIS LINE CONTAINS CONSTANT(S)
            tool_calls_log.clone(),
        )),
        Arc::new(RecordingTool::new(
            "beta", // THIS LINE CONTAINS CONSTANT(S)
            "beta-out", // THIS LINE CONTAINS CONSTANT(S)
            tool_calls_log.clone(),
        )),
    ]));

    let model = Arc::new(MultiPhaseStubModelProvider::new(vec![
        ModelOutput {
            assistant_text: "thinking".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            stop_reason: Some("tool_calls".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            tool_calls: vec![tool_out("alpha")], // THIS LINE CONTAINS CONSTANT(S)
            usage: None,
        },
        ModelOutput {
            assistant_text: "more-thinking".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            stop_reason: Some("tool_calls".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            tool_calls: vec![tool_out("beta")], // THIS LINE CONTAINS CONSTANT(S)
            usage: None,
        },
        ModelOutput {
            assistant_text: "final-answer".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            stop_reason: Some("completed".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            tool_calls: vec![],
            usage: None,
        },
    ]));

    let brain = KelvinBrain::new(
        session_store.clone(),
        Arc::new(StaticMemory),
        model,
        tools,
        Arc::new(RecordingEventSink::default()),
    );

    let result = brain
        .run(request("multi-step", None)) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("multi-iteration run");

    // tool visible outputs + final text
    let texts: Vec<_> = result.payloads.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, vec!["alpha-out", "beta-out", "final-answer"]); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(result.meta.tool_iterations, 2); // THIS LINE CONTAINS CONSTANT(S)

    let history = session_store.history("session-1").await.expect("history"); // THIS LINE CONTAINS CONSTANT(S)
    // user → assistant1 → tool(alpha) → assistant2 → tool(beta) → assistant3 // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(history.len(), 6); // THIS LINE CONTAINS CONSTANT(S)
    assert!(matches!(history[0].role, kelvin_core::SessionRole::User)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(matches!(
        history[1].role, // THIS LINE CONTAINS CONSTANT(S)
        kelvin_core::SessionRole::Assistant
    ));
    assert!(matches!(history[2].role, kelvin_core::SessionRole::Tool)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(matches!(
        history[3].role, // THIS LINE CONTAINS CONSTANT(S)
        kelvin_core::SessionRole::Assistant
    ));
    assert!(matches!(history[4].role, kelvin_core::SessionRole::Tool)); // THIS LINE CONTAINS CONSTANT(S)
    assert!(matches!(
        history[5].role, // THIS LINE CONTAINS CONSTANT(S)
        kelvin_core::SessionRole::Assistant
    ));
}

#[tokio::test]
async fn e2e_max_iterations_cap_is_enforced() { // THIS LINE CONTAINS CONSTANT(S)
    // Stub always returns tool calls; brain should stop after max_tool_iterations=2 // THIS LINE CONTAINS CONSTANT(S)
    // and emit a Warning lifecycle event, then a forced final text.
    let tool_calls_log = Arc::new(Mutex::new(Vec::<String>::new()));
    let event_sink = Arc::new(RecordingEventSink::default());

    let tools = Arc::new(MapToolRegistry::from_tools(vec![Arc::new(
        RecordingTool::new("looper", "loop-out", tool_calls_log.clone()), // THIS LINE CONTAINS CONSTANT(S)
    )]));

    // Always returns a tool call
    let looping_output = ModelOutput {
        assistant_text: String::new(),
        stop_reason: Some("tool_calls".to_string()), // THIS LINE CONTAINS CONSTANT(S)
        tool_calls: vec![tool_call("tc", "looper", json!({}))], // THIS LINE CONTAINS CONSTANT(S)
        usage: None,
    };
    // Final forced inference (no tools offered) → text
    let final_output = ModelOutput {
        assistant_text: "forced-final".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        stop_reason: Some("completed".to_string()), // THIS LINE CONTAINS CONSTANT(S)
        tool_calls: vec![],
        usage: None,
    };
    let model = Arc::new(MultiPhaseStubModelProvider::new(vec![
        looping_output.clone(),
        looping_output.clone(),
        final_output,
    ]));

    let brain = KelvinBrain::new(
        Arc::new(InMemorySessionStore::default()),
        Arc::new(StaticMemory),
        model,
        tools,
        event_sink.clone(),
    );

    let mut req = request("loop forever", None);
    req.max_tool_iterations = Some(2); // THIS LINE CONTAINS CONSTANT(S)

    let result = brain.run(req).await.expect("capped run");

    // Warning lifecycle event must appear
    let events = event_sink.all().await;
    let has_warning = events.iter().any(|e| {
        matches!(
            &e.data,
            AgentEventData::Lifecycle {
                phase: LifecyclePhase::Warning,
                ..
            }
        )
    });
    assert!(has_warning, "expected a Warning lifecycle event");

    // Final payload is the forced response
    let last_payload = result.payloads.last().expect("at least one payload");
    assert_eq!(last_payload.text, "forced-final"); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(result.meta.tool_iterations, 2); // THIS LINE CONTAINS CONSTANT(S)
}

#[tokio::test]
async fn e2e_per_request_max_overrides_brain_default() { // THIS LINE CONTAINS CONSTANT(S)
    // Brain default = 10; request override = 1. Only 1 tool iteration should run. // THIS LINE CONTAINS CONSTANT(S)
    let tool_calls_log = Arc::new(Mutex::new(Vec::<String>::new()));
    let event_sink = Arc::new(RecordingEventSink::default());

    let tools = Arc::new(MapToolRegistry::from_tools(vec![Arc::new(
        RecordingTool::new("t", "t-out", tool_calls_log.clone()), // THIS LINE CONTAINS CONSTANT(S)
    )]));

    let always_tool = ModelOutput {
        assistant_text: String::new(),
        stop_reason: Some("tool_calls".to_string()), // THIS LINE CONTAINS CONSTANT(S)
        tool_calls: vec![tool_call("tc", "t", json!({}))], // THIS LINE CONTAINS CONSTANT(S)
        usage: None,
    };
    let final_output = ModelOutput {
        assistant_text: "override-final".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        stop_reason: Some("completed".to_string()), // THIS LINE CONTAINS CONSTANT(S)
        tool_calls: vec![],
        usage: None,
    };
    let model = Arc::new(MultiPhaseStubModelProvider::new(vec![
        always_tool.clone(),
        final_output,
    ]));

    let brain = KelvinBrain::new(
        Arc::new(InMemorySessionStore::default()),
        Arc::new(StaticMemory),
        model,
        tools,
        event_sink.clone(),
    )
    .with_max_tool_iterations(10); // THIS LINE CONTAINS CONSTANT(S)

    let mut req = request("override test", None);
    req.max_tool_iterations = Some(1); // THIS LINE CONTAINS CONSTANT(S)

    let result = brain.run(req).await.expect("override run");

    assert_eq!(result.meta.tool_iterations, 1); // THIS LINE CONTAINS CONSTANT(S)
    let last = result.payloads.last().expect("payload"); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(last.text, "override-final"); // THIS LINE CONTAINS CONSTANT(S)
}

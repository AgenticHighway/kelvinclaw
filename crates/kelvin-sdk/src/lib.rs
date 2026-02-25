use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::json;
use tokio::sync::RwLock;

use kelvin_brain::{load_installed_tool_plugins_default, EchoModelProvider, KelvinBrain};
use kelvin_core::{
    now_ms, AgentEvent, AgentRunRequest, CoreRuntime, EventSink, KelvinError, KelvinResult,
    MemorySearchManager, PluginSecurityPolicy, RunOutcome, SessionDescriptor, SessionMessage,
    SessionStore, Tool, ToolCallInput, ToolCallResult, ToolRegistry,
};
#[cfg(any(not(feature = "memory_rpc"), feature = "memory_legacy_fallback"))]
use kelvin_memory::MemoryBackendKind;
#[cfg(any(not(feature = "memory_rpc"), feature = "memory_legacy_fallback"))]
use kelvin_memory::MemoryFactory;
#[cfg(feature = "memory_rpc")]
use kelvin_memory_client::{MemoryClientConfig, RpcMemoryManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KelvinCliMemoryMode {
    Markdown,
    InMemory,
    Fallback,
}

impl KelvinCliMemoryMode {
    pub fn parse(input: &str) -> Self {
        let normalized = input.trim().to_lowercase();
        match normalized.as_str() {
            "markdown" | "md" => Self::Markdown,
            "in-memory" | "in_memory" | "memory" | "inmemory" => Self::InMemory,
            "fallback" | "with-fallback" => Self::Fallback,
            _ => Self::Markdown,
        }
    }

    #[cfg(any(not(feature = "memory_rpc"), feature = "memory_legacy_fallback"))]
    fn as_backend_kind(self) -> MemoryBackendKind {
        match self {
            Self::Markdown => MemoryBackendKind::Markdown,
            Self::InMemory => MemoryBackendKind::InMemoryVector,
            Self::Fallback => MemoryBackendKind::InMemoryWithMarkdownFallback,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KelvinSdkConfig {
    pub prompt: String,
    pub session_id: String,
    pub workspace_dir: PathBuf,
    pub memory_mode: KelvinCliMemoryMode,
    pub timeout_ms: u64,
    pub system_prompt: Option<String>,
    pub core_version: String,
    pub plugin_security_policy: PluginSecurityPolicy,
    pub load_installed_plugins: bool,
}

impl KelvinSdkConfig {
    pub fn for_prompt(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            session_id: "main".to_string(),
            workspace_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            memory_mode: KelvinCliMemoryMode::Markdown,
            timeout_ms: 30_000,
            system_prompt: None,
            core_version: "0.1.0".to_string(),
            plugin_security_policy: PluginSecurityPolicy::default(),
            load_installed_plugins: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KelvinRunSummary {
    pub run_id: String,
    pub accepted_at_ms: u128,
    pub provider: String,
    pub model: String,
    pub duration_ms: u128,
    pub payloads: Vec<String>,
    pub loaded_installed_plugins: usize,
    pub cli_plugin_preflight: String,
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

#[derive(Debug, Default)]
struct StdoutEventSink;

#[async_trait]
impl EventSink for StdoutEventSink {
    async fn emit(&self, event: AgentEvent) -> KelvinResult<()> {
        println!("{}", serde_json::to_string(&event).unwrap_or_default());
        Ok(())
    }
}

#[derive(Default)]
struct HashMapToolRegistry {
    tools: std::sync::RwLock<HashMap<String, Arc<dyn Tool>>>,
}

impl HashMapToolRegistry {
    fn register<T>(&self, tool: T)
    where
        T: Tool + 'static,
    {
        let name = tool.name().to_string();
        self.tools
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(name, Arc::new(tool));
    }
}

impl ToolRegistry for HashMapToolRegistry {
    fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(name)
            .cloned()
    }

    fn names(&self) -> Vec<String> {
        self.tools
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .keys()
            .cloned()
            .collect()
    }
}

#[derive(Clone)]
struct CombinedToolRegistry {
    registries: Vec<Arc<dyn ToolRegistry>>,
}

impl CombinedToolRegistry {
    fn new(registries: Vec<Arc<dyn ToolRegistry>>) -> Self {
        Self { registries }
    }
}

impl ToolRegistry for CombinedToolRegistry {
    fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        for registry in &self.registries {
            if let Some(tool) = registry.get(name) {
                return Some(tool);
            }
        }
        None
    }

    fn names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for registry in &self.registries {
            names.extend(registry.names());
        }
        names.sort();
        names.dedup();
        names
    }
}

#[derive(Debug, Clone)]
struct TimeTool;

#[async_trait]
impl Tool for TimeTool {
    fn name(&self) -> &str {
        "time"
    }

    async fn call(&self, _input: ToolCallInput) -> KelvinResult<ToolCallResult> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        Ok(ToolCallResult {
            summary: "timestamp generated".to_string(),
            output: Some(now_ms.to_string()),
            visible_text: Some(format!("Current unix epoch millis: {now_ms}")),
            is_error: false,
        })
    }
}

#[derive(Debug, Clone)]
struct StaticTextTool {
    name: String,
    text: String,
}

impl StaticTextTool {
    fn new(name: &str, text: &str) -> Self {
        Self {
            name: name.to_string(),
            text: text.to_string(),
        }
    }
}

#[async_trait]
impl Tool for StaticTextTool {
    fn name(&self) -> &str {
        &self.name
    }

    async fn call(&self, _input: ToolCallInput) -> KelvinResult<ToolCallResult> {
        Ok(ToolCallResult {
            summary: format!("{} returned static text", self.name),
            output: Some(self.text.clone()),
            visible_text: Some(self.text.clone()),
            is_error: false,
        })
    }
}

pub async fn run_with_sdk(config: KelvinSdkConfig) -> KelvinResult<KelvinRunSummary> {
    let session_store = Arc::new(InMemorySessionStore::default());
    let event_sink = Arc::new(StdoutEventSink);

    let builtin_tools = Arc::new(HashMapToolRegistry::default());
    builtin_tools.register(TimeTool);
    builtin_tools.register(StaticTextTool::new(
        "hello_tool",
        "Hello from Kelvin SDK built-in tools.",
    ));

    let (installed_tools, loaded_installed_plugins): (Arc<dyn ToolRegistry>, usize) =
        if config.load_installed_plugins {
            let loaded = load_installed_tool_plugins_default(
                &config.core_version,
                config.plugin_security_policy.clone(),
            )?;
            println!("loaded installed plugins: {}", loaded.loaded_plugins.len());
            (loaded.tool_registry, loaded.loaded_plugins.len())
        } else {
            (Arc::new(HashMapToolRegistry::default()), 0)
        };

    let cli_plugin_tool = installed_tools.get("kelvin_cli").ok_or_else(|| {
        KelvinError::NotFound(
            "required plugin tool 'kelvin_cli' not found; install it with scripts/install-kelvin-cli-plugin.sh"
                .to_string(),
        )
    })?;

    let run_id = format!("run-{}", now_ms());
    let cli_preflight = cli_plugin_tool
        .call(ToolCallInput {
            run_id: run_id.clone(),
            session_id: config.session_id.clone(),
            workspace_dir: config.workspace_dir.to_string_lossy().to_string(),
            arguments: json!({"prompt": config.prompt}),
        })
        .await?
        .summary;

    let tools: Arc<dyn ToolRegistry> = Arc::new(CombinedToolRegistry::new(vec![
        installed_tools,
        builtin_tools,
    ]));

    #[cfg(feature = "memory_rpc")]
    let memory: Arc<dyn MemorySearchManager> = {
        let mut rpc_cfg = MemoryClientConfig::from_env();
        rpc_cfg.workspace_id = config.workspace_dir.to_string_lossy().to_string();
        rpc_cfg.session_id = config.session_id.clone();
        match RpcMemoryManager::connect(rpc_cfg).await {
            Ok(manager) => {
                println!("using rpc memory manager");
                Arc::new(manager)
            }
            Err(err) => {
                #[cfg(feature = "memory_legacy_fallback")]
                {
                    eprintln!(
                        "warning: rpc memory unavailable, falling back to legacy in-proc memory: {err}"
                    );
                    MemoryFactory::build(
                        &config.workspace_dir,
                        config.memory_mode.as_backend_kind(),
                    )
                }
                #[cfg(not(feature = "memory_legacy_fallback"))]
                {
                    return Err(KelvinError::Backend(format!(
                        "memory controller unavailable and legacy fallback disabled: {err}"
                    )));
                }
            }
        }
    };

    #[cfg(not(feature = "memory_rpc"))]
    let memory: Arc<dyn MemorySearchManager> =
        MemoryFactory::build(&config.workspace_dir, config.memory_mode.as_backend_kind());

    let model = Arc::new(EchoModelProvider::new("kelvin", "echo-v1"));
    let brain = Arc::new(KelvinBrain::new(
        session_store,
        memory,
        model,
        tools,
        event_sink,
    ));

    let runtime = CoreRuntime::new(brain);
    let request = AgentRunRequest {
        run_id: run_id.clone(),
        session_id: config.session_id.clone(),
        session_key: config.session_id,
        workspace_dir: config.workspace_dir.to_string_lossy().to_string(),
        prompt: config.prompt,
        extra_system_prompt: config.system_prompt,
        timeout_ms: Some(config.timeout_ms),
        memory_query: None,
    };

    let accepted = runtime.submit(request).await?;
    println!(
        "accepted run: {} at {}",
        accepted.run_id, accepted.accepted_at_ms
    );

    match runtime
        .wait_for_outcome(&run_id, config.timeout_ms.saturating_add(5_000))
        .await?
    {
        RunOutcome::Completed(result) => Ok(KelvinRunSummary {
            run_id,
            accepted_at_ms: accepted.accepted_at_ms,
            provider: result.meta.provider,
            model: result.meta.model,
            duration_ms: result.meta.duration_ms,
            payloads: result.payloads.into_iter().map(|item| item.text).collect(),
            loaded_installed_plugins,
            cli_plugin_preflight: cli_preflight,
        }),
        RunOutcome::Failed(error) => Err(KelvinError::Backend(format!("run failed: {error}"))),
        RunOutcome::Timeout => Err(KelvinError::Timeout(
            "timed out waiting for run result".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{run_with_sdk, KelvinCliMemoryMode, KelvinSdkConfig};

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .canonicalize()
            .expect("resolve repo root")
    }

    fn unique_workspace() -> std::path::PathBuf {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!("kelvin-sdk-test-{millis}"));
        std::fs::create_dir_all(&path).expect("create workspace");
        path
    }

    #[tokio::test]
    async fn run_with_sdk_executes_cli_plugin_and_returns_payload() {
        let root = repo_root();
        let plugin_home = root.join("fixtures/installed_plugins");
        let trust_policy = root.join("fixtures/trusted_publishers.kelvin.json");
        assert!(
            plugin_home.is_dir(),
            "missing plugin fixture directory: {}",
            plugin_home.to_string_lossy()
        );
        assert!(
            trust_policy.is_file(),
            "missing trust policy fixture: {}",
            trust_policy.to_string_lossy()
        );

        let previous_plugin_home = std::env::var("KELVIN_PLUGIN_HOME").ok();
        let previous_trust_policy = std::env::var("KELVIN_TRUST_POLICY_PATH").ok();
        std::env::set_var("KELVIN_PLUGIN_HOME", plugin_home.as_os_str());
        std::env::set_var("KELVIN_TRUST_POLICY_PATH", trust_policy.as_os_str());

        let workspace = unique_workspace();
        let mut config = KelvinSdkConfig::for_prompt("hello sdk");
        config.workspace_dir = workspace;
        config.timeout_ms = 5_000;
        config.memory_mode = KelvinCliMemoryMode::Fallback;
        config.load_installed_plugins = true;

        let result = run_with_sdk(config).await.expect("run with sdk");

        match previous_plugin_home {
            Some(value) => std::env::set_var("KELVIN_PLUGIN_HOME", value),
            None => std::env::remove_var("KELVIN_PLUGIN_HOME"),
        }
        match previous_trust_policy {
            Some(value) => std::env::set_var("KELVIN_TRUST_POLICY_PATH", value),
            None => std::env::remove_var("KELVIN_TRUST_POLICY_PATH"),
        }

        assert!(result.cli_plugin_preflight.contains("kelvin_cli executed"));
        assert!(result
            .payloads
            .iter()
            .any(|payload| payload.contains("Echo: hello sdk")));
    }
}

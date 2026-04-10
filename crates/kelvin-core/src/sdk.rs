use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use semver::Version;
use serde::{Deserialize, Serialize};

use serde_json::Value;

use crate::{
    EventSink, KelvinError, KelvinResult, MemorySearchManager, ModelProvider, SessionStore, Tool,
    ToolRegistry,
};

pub const KELVIN_CORE_SDK_NAME: &str = "Kelvin Core";
pub const KELVIN_CORE_API_VERSION: &str = "1.0.0";
pub const MAX_PLUGIN_ID_LEN: usize = 128;
pub const MAX_PLUGIN_NAME_LEN: usize = 128;
pub const MAX_PLUGIN_DESCRIPTION_LEN: usize = 4_096;
pub const MAX_PLUGIN_HOMEPAGE_LEN: usize = 2_048;
pub const MAX_PLUGIN_CAPABILITIES: usize = 32;

/// ### Brief
///
/// defines explicit capabilities for a plugin
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    /// ### Brief
    ///
    /// plugin provides an LLM model backend
    ///
    /// ### Description
    ///
    /// declares that the plugin implements `ModelProvider` and can serve inference requests.
    /// wasm model plugins that declare this are loaded into `SdkModelProviderRegistry` and
    /// selected via `KELVIN_MODEL_PROVIDER`. `FsRead`, `FsWrite`, and `CommandExecution` are
    /// rejected when combined with this capability for model-runtime plugins.
    ///
    /// ### Currently Used
    ///
    /// yes — active for installed WASM model plugins and built-in model providers
    ModelProvider,

    /// ### Brief
    ///
    /// plugin provides a memory search backend
    ///
    /// ### Description
    ///
    /// declares that the plugin implements `MemorySearchManager`. defined in `PluginFactory`
    /// as `memory_provider()` but no installed plugin loader currently wires this up at
    /// runtime; only appears in tests.
    ///
    /// ### Currently Used
    ///
    /// no — declared but not wired in any user-facing loading path
    MemoryProvider,

    /// ### Brief
    ///
    /// plugin provides a session store backend
    ///
    /// ### Description
    ///
    /// declares that the plugin implements `SessionStore`. defined in `PluginFactory` as
    /// `session_store()` but no installed plugin loader currently wires this up at runtime.
    ///
    /// ### Currently Used
    ///
    /// no — declared but not wired in any user-facing loading path
    SessionStore,

    /// ### Brief
    ///
    /// plugin provides an event sink backend
    ///
    /// ### Description
    ///
    /// declares that the plugin implements `EventSink` for consuming runtime events. defined
    /// in `PluginFactory` as `event_sink()` but no installed plugin loader currently wires
    /// this up at runtime.
    ///
    /// ### Currently Used
    ///
    /// no — declared but not wired in any user-facing loading path
    EventSink,

    /// ### Brief
    ///
    /// plugin exposes a tool callable by the LLM
    ///
    /// ### Description
    ///
    /// declares that the plugin implements `Tool` and should be registered in the tool
    /// registry. `SdkToolRegistry` enforces that this capability and a non-nil `tool()`
    /// return must agree — a mismatch is a hard registration error. Currently used by all toolpack
    /// tools and installed WASM skill plugins.
    ///
    /// ### Currently Used
    ///
    /// yes — required for all tool plugins, both built-in and installed
    ToolProvider,

    /// ### Brief
    ///
    /// plugin reads from the filesystem
    ///
    /// ### Description
    ///
    /// declares that the plugin needs filesystem read access. checked against
    /// `PluginSecurityPolicy::allow_fs_read` at registration time; plugins will be rejected
    /// if the host policy disallows it. for WASM tool plugins, this also sets
    /// `SandboxPolicy::allow_fs_read` on the wasm sandbox.
    ///
    /// ### Currently Used
    ///
    /// yes — declared by `kelvin.tool.fs_read` and any installed WASM tool that reads files
    FsRead,

    /// ### Brief
    ///
    /// plugin writes to the filesystem
    ///
    /// ### Description
    ///
    /// declares that the plugin needs filesystem write access. checked against
    /// `PluginSecurityPolicy::allow_fs_write` at registration time. currently rejected
    /// outright for installed WASM tool plugins — only the built-in toolpack
    /// `kelvin.tool.fs_write` uses this capability.
    ///
    /// ### Currently Used
    ///
    /// partially — active for the built-in toolpack only; blocked for user-installed WASM plugins
    FsWrite,

    /// ### Brief
    ///
    /// plugin makes outbound network requests
    ///
    /// ### Description
    ///
    /// declares that the plugin needs outbound network access. checked against
    /// `PluginSecurityPolicy::allow_network_egress` at registration time. for WASM plugins,
    /// the allowed hosts are further constrained by `capability_scopes.network_allow_hosts`
    /// in the manifest. Currently used by `kelvin.tool.web_fetch` and WASM model plugins.
    ///
    /// ### Currently Used
    ///
    /// yes — active for web fetch toolpack tool and installed WASM model plugins
    NetworkEgress,

    /// ### Brief
    ///
    /// plugin executes shell commands
    ///
    /// ### Description
    ///
    /// declares that the plugin needs to run arbitrary shell commands. checked against
    /// `PluginSecurityPolicy::allow_command_execution` at registration time. currently
    /// rejected for all installed WASM tool and model plugins — it exists in the enum and
    /// security policy but no runtime path permits it for user plugins. only appears in
    /// security tests as an expected-rejection case.
    ///
    /// ### Currently Used
    ///
    /// no — defined and policy-gated but blocked for all user-installed plugins
    CommandExecution,

    /// ### Brief
    ///
    /// plugin provides slash commands to the gateway
    ///
    /// ### Description
    ///
    /// declares that the plugin implements `CommandProvider` and exposes slash commands
    /// via `list_commands` / `execute_command`. defined in `PluginFactory` as
    /// `command_provider()` but no installed plugin loader currently wires this up at runtime.
    ///
    /// ### Currently Used
    ///
    /// no — declared but not wired in any user-facing loading path
    CommandProvider,

    /// ### Brief
    ///
    /// plugin reads specific environment variables
    ///
    /// ### Description
    ///
    /// declares that the plugin needs access to environment variables listed in
    /// `capability_scopes.env_allow` in the manifest. declaring `env_allow` scopes without
    /// this capability is a hard registration error. the allowed vars are passed into the
    /// WASM sandbox policy and are the only env vars the plugin can read.
    ///
    /// ### Currently Used
    ///
    /// yes — required for any installed WASM plugin that declares `env_allow` scopes
    EnvAccess,
}

/// ### Brief
///
/// metadata describing a single slash command provided by the gateway or a plugin.
///
/// ### Fields
/// * `name` -
///
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlashCommandMeta {
    pub name: String,
    pub description: String,
    pub usage: Option<String>,
    pub category: String,
}

/// ### Brief
///
/// context passed to a command handler during execution.
#[derive(Debug, Clone)]
pub struct CommandContext {
    pub session_id: String,
    pub workspace_dir: String,
}

/// ### Brief
///
/// the result returned by a command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub message: String,
    pub data: Option<Value>,
}

/// ### Brief
///
/// trait implemented by plugins that provide slash commands
///
/// ### Description
///
/// this is only for installed plugins; not used for internal slash commands
pub trait CommandProvider: Send + Sync {
    /// ### Brief
    ///
    /// list slash command(s) provided by a command provider
    fn list_commands(&self) -> Vec<SlashCommandMeta>;

    /// ### Brief
    ///
    /// execute a slash command
    ///
    /// ### Description
    ///
    /// ### Arguments
    /// * `name` - name of the command to execute
    /// * `args` - arguments provided to command
    /// * `ctx` - context in which to execute command
    ///
    /// ### Returns
    /// command output object
    ///
    /// ### Errors
    fn execute_command(
        &self,
        name: &str,
        args: Value,
        ctx: CommandContext,
    ) -> KelvinResult<CommandOutput>;
}

/// ### Brief
///
/// registry-level definition of a plugin manifest
///
/// ### Description
///
/// a plugin manifest is the static declaration a plugin provides to describe itself to the
/// kelvin core registry. it carries identity fields (`id`, `name`, `version`), an api
/// compatibility marker (`api_version`), optional metadata (`description`, `homepage`),
/// the set of capabilities the plugin claims (`capabilities`), a flag for experimental status,
/// and optional semver bounds that constrain which core versions the plugin is compatible with.
///
/// at registration time `PluginManifest::validate` checks the manifest for well-formedness
/// and `check_plugin_compatibility` cross-checks it against the running core version and the
/// active `PluginSecurityPolicy`. a manifest that fails either check causes registration to
/// be rejected with a `KelvinError::InvalidInput`.
///
/// ### Fields
/// * `id` - plugin id
/// * `name` - human-readable display name (max 128 chars)
/// * `version` - semver string for the plugin release
/// * `api_version` - semver string for the kelvin sdk api the plugin was built against
/// * `description` - optional short description of the plugin (max 4096 chars)
/// * `homepage` - optional url pointing to documentation or a project page (max 2048 chars)
/// * `capabilities` - explicit list of `PluginCapability` values the plugin declares (max 32)
/// * `experimental` - if true, the plugin is only accepted when the security policy permits experimental plugins
/// * `min_core_version` - optional lower bound on the core semver version required to run this plugin
/// * `max_core_version` - optional upper bound on the core semver version this plugin is compatible with
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub capabilities: Vec<PluginCapability>,
    pub experimental: bool,
    pub min_core_version: Option<String>,
    pub max_core_version: Option<String>,
}

/// top-level manifest validation routine
impl PluginManifest {
    pub fn validate(&self) -> KelvinResult<()> {
        validate_plugin_id(&self.id)?;
        validate_plugin_name(&self.name)?;
        validate_semver("plugin version", &self.version)?;
        validate_semver("api version", &self.api_version)?;
        validate_optional_text_field(
            "plugin description",
            self.description.as_deref(),
            MAX_PLUGIN_DESCRIPTION_LEN,
        )?;
        validate_homepage(self.homepage.as_deref())?;
        validate_capabilities(&self.capabilities)?;

        if let Some(min) = &self.min_core_version {
            validate_semver("min core version", min)?;
        }
        if let Some(max) = &self.max_core_version {
            validate_semver("max core version", max)?;
        }

        Ok(())
    }
}

/// ### Brief
///
/// validate a semver string
fn validate_semver(label: &str, value: &str) -> KelvinResult<()> {
    if value.trim().is_empty() {
        return Err(KelvinError::InvalidInput(format!(
            "{label} must not be empty"
        )));
    }
    Version::parse(value).map_err(|err| {
        let shown = preview(value, 64);
        KelvinError::InvalidInput(format!(
            "{label} must be valid semver, got '{shown}': {err}"
        ))
    })?;
    Ok(())
}

/// ### Brief
///
/// validate a plugin id string
///
/// ### Description
///
/// - the length must be in: 0 < length <= 128
/// - must be all alphanumeric, but allowing '_', '-', and '.'
fn validate_plugin_id(value: &str) -> KelvinResult<()> {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        return Err(KelvinError::InvalidInput(
            "plugin id must not be empty".to_string(),
        ));
    }
    if cleaned.chars().count() > MAX_PLUGIN_ID_LEN {
        return Err(KelvinError::InvalidInput(format!(
            "plugin id exceeds max length {MAX_PLUGIN_ID_LEN}"
        )));
    }
    if cleaned.chars().any(|ch| ch.is_control()) {
        return Err(KelvinError::InvalidInput(
            "plugin id must not include control characters".to_string(),
        ));
    }
    if !cleaned
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        let shown = preview(cleaned, 64);
        return Err(KelvinError::InvalidInput(format!(
            "plugin id has invalid characters: {shown}"
        )));
    }
    Ok(())
}

/// ### Brief
///
/// validate a plugin name string
///
/// ### Description
///
/// - the length must be in: 0 < length <= 128
/// - must be all alphanumeric
fn validate_plugin_name(value: &str) -> KelvinResult<()> {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        return Err(KelvinError::InvalidInput(
            "plugin name must not be empty".to_string(),
        ));
    }
    if cleaned.chars().count() > MAX_PLUGIN_NAME_LEN {
        return Err(KelvinError::InvalidInput(format!(
            "plugin name exceeds max length {MAX_PLUGIN_NAME_LEN}"
        )));
    }
    if cleaned.chars().any(|ch| ch.is_control()) {
        return Err(KelvinError::InvalidInput(
            "plugin name must not include control characters".to_string(),
        ));
    }
    Ok(())
}

/// ### Brief
///
/// validate an optional string field
///
/// ### Description
///
/// - the length must be in: 0 < length <= max_len
/// - must not have control characters
fn validate_optional_text_field(
    label: &str,
    value: Option<&str>,
    max_len: usize,
) -> KelvinResult<()> {
    let Some(raw) = value else {
        return Ok(());
    };
    let cleaned = raw.trim();
    if cleaned.is_empty() {
        return Err(KelvinError::InvalidInput(format!(
            "{label} must not be empty"
        )));
    }
    if cleaned.chars().count() > max_len {
        return Err(KelvinError::InvalidInput(format!(
            "{label} exceeds max length {max_len}"
        )));
    }
    if cleaned.chars().any(|ch| ch.is_control()) {
        return Err(KelvinError::InvalidInput(format!(
            "{label} must not include control characters"
        )));
    }
    Ok(())
}

/// ### Brief
///
/// validate an optional home URL
///
/// ### Description
///
/// - the length must be in: 0 < length <= 2048
/// - must not have control characters or whitespace
/// - must be an http URL
fn validate_homepage(value: Option<&str>) -> KelvinResult<()> {
    let Some(raw) = value else {
        return Ok(());
    };
    let cleaned = raw.trim();
    if cleaned.is_empty() {
        return Err(KelvinError::InvalidInput(
            "plugin homepage must not be empty".to_string(),
        ));
    }
    if cleaned.chars().count() > MAX_PLUGIN_HOMEPAGE_LEN {
        return Err(KelvinError::InvalidInput(format!(
            "plugin homepage exceeds max length {MAX_PLUGIN_HOMEPAGE_LEN}"
        )));
    }
    if cleaned
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(KelvinError::InvalidInput(
            "plugin homepage must not include whitespace/control characters".to_string(),
        ));
    }
    if !(cleaned.starts_with("https://") || cleaned.starts_with("http://")) {
        return Err(KelvinError::InvalidInput(
            "plugin homepage must use http:// or https://".to_string(),
        ));
    }
    Ok(())
}

/// ### Brief
///
/// validate the capabilities field
///
/// ### Description
///
/// - one plugin is allowed a maximum of 32 capabilities (hard coded)
fn validate_capabilities(capabilities: &[PluginCapability]) -> KelvinResult<()> {
    if capabilities.len() > MAX_PLUGIN_CAPABILITIES {
        return Err(KelvinError::InvalidInput(format!(
            "plugin capabilities exceed max length {MAX_PLUGIN_CAPABILITIES}"
        )));
    }

    let mut seen = HashSet::new();
    for capability in capabilities {
        if !seen.insert(*capability) {
            return Err(KelvinError::InvalidInput(format!(
                "plugin capabilities contain duplicate value: {capability:?}"
            )));
        }
    }
    Ok(())
}

/// ### Brief
///
/// generate preview string for a plugin manifest (as a JSON object)
fn preview(value: &str, max_chars: usize) -> String {
    let mut shown = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_chars {
            shown.push_str("...");
            return shown;
        }
        shown.push(ch);
    }
    shown
}

/// ### Brief
///
/// set of capability gates passed to the registry at plugin registration time
///
/// ### Description
///
/// constructed by the host per invocation and handed to `PluginRegistry::register`, which
/// forwards it to `check_plugin_compatibility`. any capability declared in the plugin's
/// manifest that is not permitted by the policy causes registration to be rejected.
///
/// ### Fields
/// * `allow_experimental` - if false, plugins with `experimental: true` in their manifest are rejected
/// * `allow_fs_read` - permits plugins that declare `PluginCapability::FsRead`
/// * `allow_network_egress` - permits plugins that declare `PluginCapability::NetworkEgress`
/// * `allow_fs_write` - permits plugins that declare `PluginCapability::FsWrite`
/// * `allow_command_execution` - permits plugins that declare `PluginCapability::CommandExecution`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PluginSecurityPolicy {
    pub allow_experimental: bool,
    pub allow_fs_read: bool,
    pub allow_network_egress: bool,
    pub allow_fs_write: bool,
    pub allow_command_execution: bool,
}

/// ### Brief
///
/// result of a plugin compatibility check against the running core and security policy
///
/// ### Description
///
/// returned by `check_plugin_compatibility`. if `compatible` is false, `reasons` contains
/// one entry per rejection — api version mismatches, semver range violations, policy-blocked
/// capabilities, and manifest validation failures all append to the same list.
///
/// ### Fields
/// * `compatible` - true if all checks passed and the plugin may be registered
/// * `reasons` - human-readable rejection messages; empty when `compatible` is true
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginCompatibilityReport {
    pub compatible: bool,
    pub reasons: Vec<String>,
}

/// success and failure constructors
impl PluginCompatibilityReport {
    pub fn success() -> Self {
        Self {
            compatible: true,
            reasons: Vec::new(),
        }
    }

    pub fn failure(reason: impl Into<String>) -> Self {
        Self {
            compatible: false,
            reasons: vec![reason.into()],
        }
    }
}

/// ### Brief
///
/// checks whether a plugin manifest is compatible with the running core and security policy
///
/// ### Description
///
/// validates the manifest, then checks api major version alignment, semver range bounds,
/// experimental flag, and each declared capability against the policy. does not short circuit.
///
/// ### Arguments
/// * `manifest` - the plugin's self-reported manifest to validate and check
/// * `core_version` - semver string of the running kelvin core
/// * `security_policy` - capability gates for this registration call
///
/// ### Returns
/// a `PluginCompatibilityReport` with `compatible: true` if all checks passed, or
/// `compatible: false` with one `reasons` entry per failure
pub fn check_plugin_compatibility(
    manifest: &PluginManifest,
    core_version: &str,
    security_policy: &PluginSecurityPolicy,
) -> PluginCompatibilityReport {
    if let Err(err) = manifest.validate() {
        return PluginCompatibilityReport::failure(err.to_string());
    }

    let Ok(core_version) = Version::parse(core_version) else {
        return PluginCompatibilityReport::failure(format!("invalid core version: {core_version}"));
    };

    let mut reasons = Vec::new();
    let plugin_api = Version::parse(&manifest.api_version);
    let core_api = Version::parse(KELVIN_CORE_API_VERSION);
    if let (Ok(plugin_api), Ok(core_api)) = (plugin_api, core_api) {
        if plugin_api.major != core_api.major {
            reasons.push(format!(
                "api version mismatch: plugin {} vs core {}",
                plugin_api, core_api
            ));
        }
    }

    if let Some(min) = &manifest.min_core_version {
        match Version::parse(min) {
            Ok(min_version) if core_version < min_version => reasons.push(format!(
                "core version {} is lower than required minimum {}",
                core_version, min_version
            )),
            Ok(_) => {}
            Err(err) => reasons.push(format!("invalid min_core_version '{min}': {err}")),
        }
    }

    if let Some(max) = &manifest.max_core_version {
        match Version::parse(max) {
            Ok(max_version) if core_version > max_version => reasons.push(format!(
                "core version {} exceeds plugin maximum {}",
                core_version, max_version
            )),
            Ok(_) => {}
            Err(err) => reasons.push(format!("invalid max_core_version '{max}': {err}")),
        }
    }

    if manifest.experimental && !security_policy.allow_experimental {
        reasons.push(format!(
            "plugin '{}' is experimental and policy disallows experimental plugins",
            manifest.id
        ));
    }

    if manifest.capabilities.contains(&PluginCapability::FsRead) && !security_policy.allow_fs_read {
        reasons.push(format!(
            "plugin '{}' requires filesystem read but policy disallows it",
            manifest.id
        ));
    }

    if manifest
        .capabilities
        .contains(&PluginCapability::NetworkEgress)
        && !security_policy.allow_network_egress
    {
        reasons.push(format!(
            "plugin '{}' requires network egress but policy disallows it",
            manifest.id
        ));
    }

    if manifest.capabilities.contains(&PluginCapability::FsWrite) && !security_policy.allow_fs_write
    {
        reasons.push(format!(
            "plugin '{}' requires filesystem write but policy disallows it",
            manifest.id
        ));
    }

    if manifest
        .capabilities
        .contains(&PluginCapability::CommandExecution)
        && !security_policy.allow_command_execution
    {
        reasons.push(format!(
            "plugin '{}' requires command execution but policy disallows it",
            manifest.id
        ));
    }

    if reasons.is_empty() {
        PluginCompatibilityReport::success()
    } else {
        PluginCompatibilityReport {
            compatible: false,
            reasons,
        }
    }
}

/// ### Brief
///
/// trait implemented by every plugin to expose its manifest and optional capability objects
///
/// ### Description
///
/// the primary interface between a plugin and the kelvin registry. `manifest()` is the only
/// required method; all capability accessors (`tool`, `model_provider`, `memory_provider`,
/// `session_store`, `event_sink`, `command_provider`) default to `None` and are overridden
/// only by plugins that declare the corresponding `PluginCapability`. the registry enforces
/// that declared capabilities and non-`None` accessors agree — a mismatch is a hard
/// registration error.
pub trait PluginFactory: Send + Sync {
    /// returns the plugin's manifest
    fn manifest(&self) -> &PluginManifest;

    /// returns the plugin's `Tool` implementation, if it declares `PluginCapability::ToolProvider`
    fn tool(&self) -> Option<Arc<dyn Tool>> {
        None
    }

    /// returns the plugin's `MemorySearchManager` implementation, if it declares `PluginCapability::MemoryProvider`
    fn memory_provider(&self) -> Option<Arc<dyn MemorySearchManager>> {
        None
    }

    /// returns the plugin's `ModelProvider` implementation, if it declares `PluginCapability::ModelProvider`
    fn model_provider(&self) -> Option<Arc<dyn ModelProvider>> {
        None
    }

    /// returns the plugin's `SessionStore` implementation, if it declares `PluginCapability::SessionStore`
    fn session_store(&self) -> Option<Arc<dyn SessionStore>> {
        None
    }

    /// returns the plugin's `EventSink` implementation, if it declares `PluginCapability::EventSink`
    fn event_sink(&self) -> Option<Arc<dyn EventSink>> {
        None
    }

    /// returns the plugin's `CommandProvider` implementation, if it declares `PluginCapability::CommandExecution`
    fn command_provider(&self) -> Option<Arc<dyn CommandProvider>> {
        None
    }
}

/// ### Brief
///
/// trait for storing and retrieving registered plugins
///
/// ### Description
///
/// implemented by `InMemoryPluginRegistry`. `register` runs compatibility and policy checks
/// before inserting; `get` retrieves a plugin by id; `manifests` returns a snapshot of all
/// registered manifests, used by `SdkToolRegistry` and `SdkModelProviderRegistry` during
/// their build phase.
pub trait PluginRegistry: Send + Sync {
    fn register(
        &self,
        plugin: Arc<dyn PluginFactory>,
        core_version: &str,
        security_policy: &PluginSecurityPolicy,
    ) -> KelvinResult<()>;

    fn get(&self, plugin_id: &str) -> Option<Arc<dyn PluginFactory>>;

    fn manifests(&self) -> Vec<PluginManifest>;
}

/// ### Brief
///
/// `RwLock`-backed, in-process implementation of `PluginRegistry`
///
/// ### Description
///
/// stores plugins in a `HashMap` keyed by plugin id. duplicate registration is rejected.
/// used as the default registry in the kelvin sdk runtime; not intended for cross-process
/// or persistent storage.
///
/// ### Fields
/// * `plugins` - internal map of plugin id to `PluginFactory` instance
pub struct InMemoryPluginRegistry {
    plugins: RwLock<HashMap<String, Arc<dyn PluginFactory>>>,
}

impl Default for InMemoryPluginRegistry {
    fn default() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
        }
    }
}

impl InMemoryPluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PluginRegistry for InMemoryPluginRegistry {
    fn register(
        &self,
        plugin: Arc<dyn PluginFactory>,
        core_version: &str,
        security_policy: &PluginSecurityPolicy,
    ) -> KelvinResult<()> {
        let manifest = plugin.manifest().clone();
        let report = check_plugin_compatibility(&manifest, core_version, security_policy);
        if !report.compatible {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' rejected: {}",
                manifest.id,
                report.reasons.join("; ")
            )));
        }

        let mut lock = self
            .plugins
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if lock.contains_key(&manifest.id) {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' is already registered",
                manifest.id
            )));
        }
        lock.insert(manifest.id.clone(), plugin);
        Ok(())
    }

    fn get(&self, plugin_id: &str) -> Option<Arc<dyn PluginFactory>> {
        self.plugins
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(plugin_id)
            .cloned()
    }

    fn manifests(&self) -> Vec<PluginManifest> {
        self.plugins
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .map(|plugin| plugin.manifest().clone())
            .collect()
    }
}

/// ### Brief
///
/// `ToolRegistry` built from the set of tool-capable plugins in a `PluginRegistry`
///
/// ### Description
///
/// constructed via `from_plugin_registry`, which iterates all registered manifests in
/// deterministic (sorted) order and collects tools from plugins that declare
/// `PluginCapability::ToolProvider`. it enforces that the capability declaration and the
/// `tool()` return value agree, and that no two plugins expose a tool with the same name.
/// implements `ToolRegistry` for use by the agent runtime.
///
/// ### Fields
/// * `tools` - map of tool name to `Tool` implementation
pub struct SdkToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl SdkToolRegistry {
    pub fn from_plugin_registry(registry: &dyn PluginRegistry) -> KelvinResult<Self> {
        let mut tools = HashMap::new();
        let mut manifests = registry.manifests();
        manifests.sort_by(|left, right| left.id.cmp(&right.id));

        for manifest in manifests {
            let plugin = registry.get(&manifest.id).ok_or_else(|| {
                KelvinError::NotFound(format!(
                    "plugin '{}' disappeared during tool registry build",
                    manifest.id
                ))
            })?;

            let declared_tool_provider = manifest
                .capabilities
                .contains(&PluginCapability::ToolProvider);
            let provided_tool = plugin.tool();

            match (declared_tool_provider, provided_tool) {
                (false, None) => {}
                (false, Some(_)) => {
                    return Err(KelvinError::InvalidInput(format!(
                        "plugin '{}' exposes a tool but is missing '{}' capability",
                        manifest.id, "tool_provider"
                    )));
                }
                (true, None) => {
                    return Err(KelvinError::InvalidInput(format!(
                        "plugin '{}' declares tool capability but returned no tool",
                        manifest.id
                    )));
                }
                (true, Some(tool)) => {
                    let tool_name = tool.name().trim();
                    if tool_name.is_empty() {
                        return Err(KelvinError::InvalidInput(format!(
                            "plugin '{}' returned a tool with empty name",
                            manifest.id
                        )));
                    }
                    if tools.contains_key(tool_name) {
                        return Err(KelvinError::InvalidInput(format!(
                            "duplicate tool name from plugins: {tool_name}"
                        )));
                    }
                    tools.insert(tool_name.to_string(), tool);
                }
            }
        }

        Ok(Self { tools })
    }
}

impl ToolRegistry for SdkToolRegistry {
    fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    fn names(&self) -> Vec<String> {
        let mut names = self.tools.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }
}

/// ### Brief
///
/// model provider registry built from the set of model-capable plugins in a `PluginRegistry`
///
/// ### Description
///
/// constructed via `from_plugin_registry`, which iterates all registered manifests in
/// deterministic (sorted) order and collects model providers from plugins that declare
/// `PluginCapability::ModelProvider`. it enforces that the capability declaration and the
/// `model_provider()` return value agree, and that no two plugins expose a provider with
/// the same `provider_name::model_name` key. providers are indexed both by plugin id
/// and by that composite key for flexible lookup.
///
/// ### Fields
/// * `by_plugin_id` - map of plugin id to `ModelProvider` implementation
/// * `by_provider_model` - map of `"provider_name::model_name"` key to `ModelProvider` implementation
pub struct SdkModelProviderRegistry {
    by_plugin_id: HashMap<String, Arc<dyn ModelProvider>>,
    by_provider_model: HashMap<String, Arc<dyn ModelProvider>>,
}

impl SdkModelProviderRegistry {
    pub fn from_plugin_registry(registry: &dyn PluginRegistry) -> KelvinResult<Self> {
        let mut by_plugin_id = HashMap::new();
        let mut by_provider_model = HashMap::new();
        let mut manifests = registry.manifests();
        manifests.sort_by(|left, right| left.id.cmp(&right.id));

        for manifest in manifests {
            let plugin = registry.get(&manifest.id).ok_or_else(|| {
                KelvinError::NotFound(format!(
                    "plugin '{}' disappeared during model registry build",
                    manifest.id
                ))
            })?;

            let declared_model_provider = manifest
                .capabilities
                .contains(&PluginCapability::ModelProvider);
            let provided_model = plugin.model_provider();

            match (declared_model_provider, provided_model) {
                (false, None) => {}
                (false, Some(_)) => {
                    return Err(KelvinError::InvalidInput(format!(
                        "plugin '{}' exposes a model provider but is missing '{}' capability",
                        manifest.id, "model_provider"
                    )));
                }
                (true, None) => {
                    return Err(KelvinError::InvalidInput(format!(
                        "plugin '{}' declares model provider capability but returned no model provider",
                        manifest.id
                    )));
                }
                (true, Some(model)) => {
                    let provider_name = model.provider_name().trim();
                    let model_name = model.model_name().trim();
                    if provider_name.is_empty() {
                        return Err(KelvinError::InvalidInput(format!(
                            "plugin '{}' returned a model provider with empty provider_name",
                            manifest.id
                        )));
                    }
                    if model_name.is_empty() {
                        return Err(KelvinError::InvalidInput(format!(
                            "plugin '{}' returned a model provider with empty model_name",
                            manifest.id
                        )));
                    }
                    let provider_model_key = format!("{provider_name}::{model_name}");
                    if by_provider_model.contains_key(&provider_model_key) {
                        return Err(KelvinError::InvalidInput(format!(
                            "duplicate model provider name from plugins: {provider_model_key}"
                        )));
                    }

                    by_plugin_id.insert(manifest.id.clone(), model.clone());
                    by_provider_model.insert(provider_model_key, model);
                }
            }
        }

        Ok(Self {
            by_plugin_id,
            by_provider_model,
        })
    }

    pub fn get_by_plugin_id(&self, plugin_id: &str) -> Option<Arc<dyn ModelProvider>> {
        self.by_plugin_id.get(plugin_id).cloned()
    }

    pub fn get_by_provider_model(
        &self,
        provider_name: &str,
        model_name: &str,
    ) -> Option<Arc<dyn ModelProvider>> {
        let key = format!("{}::{}", provider_name.trim(), model_name.trim());
        self.by_provider_model.get(&key).cloned()
    }

    pub fn plugin_ids(&self) -> Vec<String> {
        let mut ids = self.by_plugin_id.keys().cloned().collect::<Vec<_>>();
        ids.sort();
        ids
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use crate::{
        KelvinResult, ModelInput, ModelOutput, ModelProvider, Tool, ToolCallInput, ToolCallResult,
        ToolRegistry,
    };

    use super::{
        check_plugin_compatibility, InMemoryPluginRegistry, PluginCapability, PluginFactory,
        PluginManifest, PluginRegistry, PluginSecurityPolicy, SdkModelProviderRegistry,
        SdkToolRegistry, KELVIN_CORE_API_VERSION,
    };

    fn test_manifest() -> PluginManifest {
        PluginManifest {
            id: "acme.echo".to_string(),
            name: "Echo".to_string(),
            version: "1.2.3".to_string(),
            api_version: KELVIN_CORE_API_VERSION.to_string(),
            description: Some("Echo test plugin".to_string()),
            homepage: None,
            capabilities: vec![PluginCapability::ToolProvider],
            experimental: false,
            min_core_version: Some("0.1.0".to_string()),
            max_core_version: None,
        }
    }

    struct EchoTool;

    #[async_trait::async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        async fn call(&self, _input: ToolCallInput) -> KelvinResult<ToolCallResult> {
            Ok(ToolCallResult {
                summary: "ok".to_string(),
                output: Some("ok".to_string()),
                visible_text: Some("ok".to_string()),
                is_error: false,
            })
        }
    }

    struct EchoPlugin {
        manifest: PluginManifest,
    }

    impl PluginFactory for EchoPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }

        fn tool(&self) -> Option<Arc<dyn Tool>> {
            Some(Arc::new(EchoTool))
        }
    }

    struct EmptyToolPlugin {
        manifest: PluginManifest,
    }

    impl PluginFactory for EmptyToolPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }
    }

    struct ConflictingToolPlugin {
        manifest: PluginManifest,
    }

    impl PluginFactory for ConflictingToolPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }

        fn tool(&self) -> Option<Arc<dyn Tool>> {
            Some(Arc::new(EchoTool))
        }
    }

    #[derive(Clone)]
    struct StaticModelProvider {
        provider_name: String,
        model_name: String,
    }

    impl StaticModelProvider {
        fn new(provider_name: &str, model_name: &str) -> Self {
            Self {
                provider_name: provider_name.to_string(),
                model_name: model_name.to_string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl ModelProvider for StaticModelProvider {
        fn provider_name(&self) -> &str {
            &self.provider_name
        }

        fn model_name(&self) -> &str {
            &self.model_name
        }

        async fn infer(&self, _input: ModelInput) -> KelvinResult<ModelOutput> {
            Ok(ModelOutput {
                assistant_text: "ok".to_string(),
                stop_reason: Some("completed".to_string()),
                tool_calls: Vec::new(),
                usage: None,
            })
        }
    }

    struct StaticModelPlugin {
        manifest: PluginManifest,
        provider_name: String,
        model_name: String,
    }

    impl PluginFactory for StaticModelPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }

        fn model_provider(&self) -> Option<Arc<dyn ModelProvider>> {
            Some(Arc::new(StaticModelProvider::new(
                &self.provider_name,
                &self.model_name,
            )))
        }
    }

    struct EmptyModelPlugin {
        manifest: PluginManifest,
    }

    impl PluginFactory for EmptyModelPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }
    }

    #[test]
    fn manifest_validation_rejects_invalid_ids() {
        let mut manifest = test_manifest();
        manifest.id = "bad id".to_string();
        let err = manifest.validate().expect_err("invalid id");
        assert!(err.to_string().contains("invalid characters"));
    }

    #[test]
    fn compatibility_rejects_disallowed_capability() {
        let mut manifest = test_manifest();
        manifest.capabilities.push(PluginCapability::NetworkEgress);
        let policy = PluginSecurityPolicy::default();
        let report = check_plugin_compatibility(&manifest, "0.1.0", &policy);
        assert!(!report.compatible);
        assert!(report
            .reasons
            .iter()
            .any(|reason| reason.contains("network egress")));
    }

    #[test]
    fn compatibility_accepts_with_matching_policy() {
        let mut manifest = test_manifest();
        manifest.capabilities.push(PluginCapability::NetworkEgress);
        let policy = PluginSecurityPolicy {
            allow_network_egress: true,
            ..Default::default()
        };
        let report = check_plugin_compatibility(&manifest, "0.1.0", &policy);
        assert!(report.compatible, "{}", report.reasons.join("; "));
    }

    #[test]
    fn registry_registers_and_gets_plugin() {
        let registry = InMemoryPluginRegistry::new();
        let plugin = Arc::new(EchoPlugin {
            manifest: test_manifest(),
        });

        registry
            .register(plugin.clone(), "0.1.0", &PluginSecurityPolicy::default())
            .expect("register");

        let fetched = registry.get("acme.echo").expect("get");
        assert_eq!(fetched.manifest().id, "acme.echo");
        assert_eq!(registry.manifests().len(), 1);
        assert_eq!(fetched.tool().expect("tool").name(), "echo");
    }

    #[test]
    fn registry_rejects_duplicate_plugin_ids() {
        let registry = InMemoryPluginRegistry::new();
        let plugin = Arc::new(EchoPlugin {
            manifest: test_manifest(),
        });
        registry
            .register(plugin.clone(), "0.1.0", &PluginSecurityPolicy::default())
            .expect("first register");
        let err = registry
            .register(plugin, "0.1.0", &PluginSecurityPolicy::default())
            .expect_err("duplicate");
        assert!(err.to_string().contains("already registered"));
    }

    #[test]
    fn registry_rejects_version_out_of_range() {
        let registry = InMemoryPluginRegistry::new();
        let mut manifest = test_manifest();
        manifest.min_core_version = Some("9.0.0".to_string());
        manifest.max_core_version = Some("9.9.9".to_string());
        manifest.description = Some(json!({"note": "test"}).to_string());
        let plugin = Arc::new(EchoPlugin { manifest });
        let err = registry
            .register(plugin, "0.1.0", &PluginSecurityPolicy::default())
            .expect_err("range check");
        assert!(err.to_string().contains("lower than required minimum"));
    }

    #[test]
    fn sdk_tool_registry_builds_from_registered_plugins() {
        let registry = InMemoryPluginRegistry::new();
        let plugin = Arc::new(EchoPlugin {
            manifest: test_manifest(),
        });
        registry
            .register(plugin, "0.1.0", &PluginSecurityPolicy::default())
            .expect("register");

        let tools = SdkToolRegistry::from_plugin_registry(&registry).expect("tool registry");
        assert_eq!(tools.names(), vec!["echo".to_string()]);
        assert!(tools.get("echo").is_some());
    }

    #[test]
    fn sdk_tool_registry_rejects_missing_tool_implementation() {
        let registry = InMemoryPluginRegistry::new();
        let manifest = test_manifest();
        let plugin = Arc::new(EmptyToolPlugin { manifest });
        registry
            .register(plugin, "0.1.0", &PluginSecurityPolicy::default())
            .expect("register");

        let err = match SdkToolRegistry::from_plugin_registry(&registry) {
            Ok(_) => panic!("missing tool should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("returned no tool"));
    }

    #[test]
    fn sdk_tool_registry_rejects_duplicate_tool_names() {
        let registry = InMemoryPluginRegistry::new();
        let first = Arc::new(EchoPlugin {
            manifest: PluginManifest {
                id: "acme.echo.first".to_string(),
                ..test_manifest()
            },
        });
        let second = Arc::new(ConflictingToolPlugin {
            manifest: PluginManifest {
                id: "acme.echo.second".to_string(),
                ..test_manifest()
            },
        });
        registry
            .register(first, "0.1.0", &PluginSecurityPolicy::default())
            .expect("register first");
        registry
            .register(second, "0.1.0", &PluginSecurityPolicy::default())
            .expect("register second");

        let err = match SdkToolRegistry::from_plugin_registry(&registry) {
            Ok(_) => panic!("duplicate tools should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("duplicate tool name"));
    }

    #[test]
    fn sdk_model_registry_builds_from_registered_plugins() {
        let registry = InMemoryPluginRegistry::new();
        let plugin = Arc::new(StaticModelPlugin {
            manifest: PluginManifest {
                id: "acme.model".to_string(),
                capabilities: vec![PluginCapability::ModelProvider],
                ..test_manifest()
            },
            provider_name: "openai".to_string(),
            model_name: "gpt-4.1-mini".to_string(),
        });
        registry
            .register(plugin, "0.1.0", &PluginSecurityPolicy::default())
            .expect("register");

        let models = SdkModelProviderRegistry::from_plugin_registry(&registry).expect("build");
        assert_eq!(models.plugin_ids(), vec!["acme.model".to_string()]);
        assert!(models.get_by_plugin_id("acme.model").is_some());
        assert!(models
            .get_by_provider_model("openai", "gpt-4.1-mini")
            .is_some());
    }

    #[test]
    fn sdk_model_registry_rejects_missing_model_implementation() {
        let registry = InMemoryPluginRegistry::new();
        let plugin = Arc::new(EmptyModelPlugin {
            manifest: PluginManifest {
                id: "acme.model".to_string(),
                capabilities: vec![PluginCapability::ModelProvider],
                ..test_manifest()
            },
        });
        registry
            .register(plugin, "0.1.0", &PluginSecurityPolicy::default())
            .expect("register");

        let err = match SdkModelProviderRegistry::from_plugin_registry(&registry) {
            Ok(_) => panic!("missing model implementation should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("returned no model provider"));
    }

    #[test]
    fn sdk_model_registry_rejects_model_without_capability() {
        let registry = InMemoryPluginRegistry::new();
        let plugin = Arc::new(StaticModelPlugin {
            manifest: PluginManifest {
                id: "acme.model".to_string(),
                capabilities: vec![PluginCapability::ToolProvider],
                ..test_manifest()
            },
            provider_name: "openai".to_string(),
            model_name: "gpt-4.1-mini".to_string(),
        });
        registry
            .register(plugin, "0.1.0", &PluginSecurityPolicy::default())
            .expect("register");

        let err = match SdkModelProviderRegistry::from_plugin_registry(&registry) {
            Ok(_) => panic!("model provider without capability should fail"),
            Err(err) => err,
        };
        assert!(err
            .to_string()
            .contains("missing 'model_provider' capability"));
    }

    #[test]
    fn sdk_model_registry_rejects_duplicate_provider_model_names() {
        let registry = InMemoryPluginRegistry::new();
        let first = Arc::new(StaticModelPlugin {
            manifest: PluginManifest {
                id: "acme.model.first".to_string(),
                capabilities: vec![PluginCapability::ModelProvider],
                ..test_manifest()
            },
            provider_name: "openai".to_string(),
            model_name: "gpt-4.1-mini".to_string(),
        });
        let second = Arc::new(StaticModelPlugin {
            manifest: PluginManifest {
                id: "acme.model.second".to_string(),
                capabilities: vec![PluginCapability::ModelProvider],
                ..test_manifest()
            },
            provider_name: "openai".to_string(),
            model_name: "gpt-4.1-mini".to_string(),
        });

        registry
            .register(first, "0.1.0", &PluginSecurityPolicy::default())
            .expect("register first");
        registry
            .register(second, "0.1.0", &PluginSecurityPolicy::default())
            .expect("register second");

        let err = match SdkModelProviderRegistry::from_plugin_registry(&registry) {
            Ok(_) => panic!("duplicate model providers should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("duplicate model provider name"));
    }
}

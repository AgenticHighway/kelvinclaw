use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tokio::time;
use wasmparser::{Parser, Payload};

use kelvin_core::{
    InMemoryPluginRegistry, KelvinError, KelvinResult, ModelInput, ModelOutput, ModelProvider,
    ModelProviderProfile, ModelProviderProtocolFamily, PluginCapability, PluginFactory,
    PluginManifest, PluginRegistry, PluginSecurityPolicy, SdkModelProviderRegistry,
    SdkToolRegistry, Tool, ToolCall, ToolCallInput, ToolCallResult,
};
use kelvin_wasm::{
    model_abi, ClawCall, ModelSandboxPolicy, SandboxPolicy, WasmModelHost, WasmSkillHost,
};

use crate::consts;

/// ### Brief
///
/// represents a successfully loaded and instantiated plugin that has been registered in the plugin system
///
/// ### Description
///
/// captures a plugin's metadata and runtime identity, allowing the plugin system to track, manage,
/// and invoke plugins at runtime. a `LoadedInstalledPlugin` can represent any plugin type.
///
/// ### Note
///
/// plugins may be both a tool and model provider simultaneously.
///
/// ### Fields
/// * `id` - unique identifier for this plugin instance (typically stored as publisher.plugin)
/// * `version` - semantic version of the loaded plugin
/// * `tool_name` - the registered tool/skill name if this plugin provides tools, None otherwise
/// * `provider_name` - the registered model provider name if this plugin provides model providers, None otherwise
/// * `model_name` - the specific model name provided by this plugin if it's a model provider, None otherwise
/// * `provider_profile` - configuration profile name for the provider (e.g., "default", "gpt-4"), None if not applicable
/// * `runtime` - the runtime environment where this plugin executes (e.g., "wasm_tool_v1", "wasm_model_v1")
/// * `publisher` - the publisher/author of this plugin, None if unpublished or self-hosted
#[derive(Debug, Clone)]
pub struct LoadedInstalledPlugin {
    pub id: String,
    pub version: String,
    pub tool_name: Option<String>,
    pub provider_name: Option<String>,
    pub model_name: Option<String>,
    pub provider_profile: Option<String>,
    pub runtime: String,
    pub publisher: Option<String>,
}

/// ### Brief
///
/// aggregation wrapper for installed plugins' registries
///
/// ### Description
///
/// a wrapper the aggregates all loaded plugins and their respective registries, plus an ownership vector
/// of loaded plugins. this is here to provide unified access to all plugin metadata and runtime
/// ids.
///
/// ### Fields
/// * `plugin_registry` - generic plugin registry
/// * `tool_registry` - plugin registry for `tool_provider` type plugins
/// * `model_registry` - plugin registry for `model_provider` type plugins
/// * `loaded_plugins` - vector that holds plugin structs' ownership
#[derive(Clone)]
pub struct LoadedInstalledPlugins {
    pub plugin_registry: Arc<InMemoryPluginRegistry>,
    pub tool_registry: Arc<SdkToolRegistry>,
    pub model_registry: Arc<SdkModelProviderRegistry>,
    pub loaded_plugins: Vec<LoadedInstalledPlugin>,
}

/// ### Brief
///
/// config object for the plugin loaders
///
/// ### Description
///
/// specifies directory location, version, and security/trust policies. created (immut) before loading plugins. this
/// is here to standardize discovery and policy enforcement.
///
/// ### Fields
/// * `plugin_home` - directory location of plugin
/// * `core_version` - kelvin version
/// * `security_policy` - global security policy
/// * `trust_policy` - global trust policy
#[derive(Debug, Clone)]
pub struct InstalledPluginLoaderConfig {
    pub plugin_home: PathBuf,
    pub core_version: String,
    pub security_policy: PluginSecurityPolicy,
    pub trust_policy: PublisherTrustPolicy,
}

/// configuration construction for plugin loading
impl InstalledPluginLoaderConfig {
    /// ### Brief
    ///
    /// construct a new loader configuration with a plugin home directory
    pub fn new(plugin_home: impl Into<PathBuf>) -> Self {
        Self {
            plugin_home: plugin_home.into(),
            core_version: env!("CARGO_PKG_VERSION").to_string(),
            security_policy: PluginSecurityPolicy::default(),
            trust_policy: PublisherTrustPolicy::default(),
        }
    }
}

/// ### Brief
///
/// retrieves the KELVIN_PLUGIN_HOME env var
///
/// ### Returns
/// env var for KELVIN_PLUGIN_HOME as PathBuf
pub fn default_plugin_home() -> KelvinResult<PathBuf> {
    if let Some(path) = env_path(consts::ENV_KELVIN_PLUGIN_HOME) {
        return Ok(path);
    }
    Ok(resolve_home_dir()?.join(consts::DEFAULT_PLUGIN_HOME_RELATIVE))
}

/// ### Brief
///
/// retrieves the KELVIN_TRUST_POLICY_PATH env var
///
/// ### Returns
/// env var for KELVIN_TRUST_POLICY_PATH as PathBuf
pub fn default_trust_policy_path() -> KelvinResult<PathBuf> {
    if let Some(path) = env_path(consts::ENV_KELVIN_TRUST_POLICY_PATH) {
        return Ok(path);
    }
    Ok(resolve_home_dir()?.join(consts::DEFAULT_TRUST_POLICY_RELATIVE))
}

/// ### Brief
///
/// callback for `load_installed_plugins_default()`
///
/// ### Arguments
/// * `core_version` - kelvin version
/// * `security_policy` - security policy
///
/// ### Returns
/// a `LoadedInstalledPlugins` instance containing default plugins
pub fn load_installed_tool_plugins_default(
    core_version: impl Into<String>,
    security_policy: PluginSecurityPolicy,
) -> KelvinResult<LoadedInstalledPlugins> {
    load_installed_plugins_default(core_version, security_policy)
}

/// ### Brief
///
/// callback for `load_installed_plugins()` with default plugin home and auto-loaded trust policy
///
/// ### Arguments
/// * `core_version` - kelvin version
/// * `security_policy` - security policy
///
/// ### Returns
/// a `LoadedInstalledPlugins` instance containing default plugins
pub fn load_installed_plugins_default(
    core_version: impl Into<String>,
    security_policy: PluginSecurityPolicy,
) -> KelvinResult<LoadedInstalledPlugins> {
    let trust_policy_path = default_trust_policy_path()?;
    let trust_policy = if let Some(path) = maybe_load_trust_policy_path(&trust_policy_path)? {
        PublisherTrustPolicy::from_json_file(path)?
    } else {
        PublisherTrustPolicy::default()
    };

    load_installed_plugins(InstalledPluginLoaderConfig {
        plugin_home: default_plugin_home()?,
        core_version: core_version.into(),
        security_policy,
        trust_policy,
    })
}

/// ### Brief
///
/// defines per-plugin granular scopes for the plugin security sandbox
///
/// ### Description
///
/// generally contains whitelists for various access options. these are declared by plugin authors.
///
/// ### Fields
/// * `fs_read_paths` - what paths the plugin is allowed to read from
/// * `network_allow_hosts` - what hosts the plugin is allowed to fetch from
/// * `env_allow` - what env vars the plugin is allowed to access
#[derive(Debug, Clone, Default)]
pub struct CapabilityScopes {
    pub fs_read_paths: Vec<String>,
    pub network_allow_hosts: Vec<String>,
    pub env_allow: Vec<String>,
}

/// ### Brief
///
/// defines operational config items for a plugin
///
/// ### Description
///
/// generally consists of hard runtime limits. these are declared by plugin authors.
///
/// ### Fields
/// * `timeout_ms` - max time kelvin waits for a plugin to respond
/// * `max_retries` - max number of times kelvin core can retry a plugin if internal execution fails
/// * `max_calls_per_minute` - max calls to this plugin per minute
/// * `circuit_breaker_failures` - number of failed attempts to trip circuit breaker
/// * `circuit_breaker_cooldown_ms` - how long to block the plugin after the circuit breaker trips
#[derive(Debug, Clone)]
pub struct OperationalControls {
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub max_calls_per_minute: usize,
    pub circuit_breaker_failures: u32,
    pub circuit_breaker_cooldown_ms: u64,
}

impl Default for OperationalControls {
    fn default() -> Self {
        Self {
            timeout_ms: consts::DEFAULT_TIMEOUT_MS,
            max_retries: consts::DEFAULT_MAX_RETRIES,
            max_calls_per_minute: consts::DEFAULT_MAX_CALLS_PER_MINUTE,
            circuit_breaker_failures: consts::DEFAULT_CIRCUIT_BREAKER_FAILURES,
            circuit_breaker_cooldown_ms: consts::DEFAULT_CIRCUIT_BREAKER_COOLDOWN_MS,
        }
    }
}

/// ### Brief
///
/// defines the kelvin trust policy for all publishers
///
/// ### Description
///
/// manages trusted/revoked publisher public keys, signature requirements, and plugin-publisher mappings. this
/// is defined by the user (with restrictive defaults). revoked publisher ids
///
/// ### Fields
/// * `require_signature` - whether to require and check plugin signatures
/// * `trusted_publishers` - hashmap of trusted publishers' ids -> their public keys
/// * `revoked_publishers` - hashset of revoked publishers; ids
/// * `pinned_plugin_publishers` - hashmap of plugin ids -> publisher ids
#[derive(Debug, Clone)]
pub struct PublisherTrustPolicy {
    pub require_signature: bool,
    trusted_publishers: HashMap<String, VerifyingKey>,
    revoked_publishers: HashSet<String>,
    pinned_plugin_publishers: HashMap<String, String>,
}

/// ### Brief
///
/// default for trust policy: requires signatures, but trusted/revoked/map are empty
impl Default for PublisherTrustPolicy {
    fn default() -> Self {
        Self {
            require_signature: true,
            trusted_publishers: HashMap::new(),
            revoked_publishers: HashSet::new(),
            pinned_plugin_publishers: HashMap::new(),
        }
    }
}

/// builder methods and core trust verification for plugin publishers
impl PublisherTrustPolicy {
    /// ### Brief
    ///
    /// chainable consuming function that sets the signature requirement to true or false
    ///
    /// ### Arguments
    /// * `required` - whether to require a signature
    ///
    /// ### Returns
    /// the `PublisherTrustPolicy` instance
    ///
    /// ### Example
    /// ```
    /// use kelvin_brain::installed_plugins::PublisherTrustPolicy;
    ///
    /// let policy = PublisherTrustPolicy::default();
    ///
    /// assert_eq!(policy.require_signature, true);
    /// assert_eq!(policy.with_signature_requirement(false).require_signature, false);
    /// ```
    pub fn with_signature_requirement(mut self, required: bool) -> Self {
        self.require_signature = required;
        self
    }

    /// ### Brief
    ///
    /// chainable consuming function that inserts a new trusted publisher id/public key pair
    ///
    /// ### Arguments
    /// * `publisher_id` - publisher id
    /// * `ed25519_public_key_base64` - public key
    ///
    /// ### Returns
    /// the `PublisherTrustPolicy` instance
    ///
    /// ### Errors
    /// - invalid base64 encoding
    /// - invalid ed25519 public key format
    ///
    /// ### Example
    /// ```
    /// use kelvin_brain::installed_plugins::PublisherTrustPolicy;
    ///
    /// let policy = PublisherTrustPolicy::default();
    /// let new_policy = policy.with_publisher_key("acme", "DEADBEEF");
    /// ```
    pub fn with_publisher_key(
        mut self,
        publisher_id: &str,
        ed25519_public_key_base64: &str,
    ) -> KelvinResult<Self> {
        let key = parse_public_key(ed25519_public_key_base64)?;
        self.trusted_publishers
            .insert(publisher_id.to_string(), key);
        Ok(self)
    }

    /// ### Brief
    ///
    /// chainable consuming function that inserts a revoked publisher id
    ///
    /// ### Arguments
    /// * `publisher_id` - publisher id
    ///
    /// ### Returns
    /// the `PublisherTrustPolicy` instance
    ///
    /// ### Example
    /// ```
    /// use kelvin_brain::installed_plugins::PublisherTrustPolicy;
    ///
    /// let policy = PublisherTrustPolicy::default();
    /// let new_policy = policy.with_revoked_publisher("weyland-yutani-corp");
    /// ```
    pub fn with_revoked_publisher(mut self, publisher_id: &str) -> Self {
        self.revoked_publishers.insert(publisher_id.to_string());
        self
    }

    /// ### Brief
    ///
    /// chainable consuming function that inserts a plugin -> publisher mapping
    ///
    /// ### Arguments
    /// * `plugin_id` - plugin id
    /// * `publisher_id` - publisher id
    ///
    /// ### Returns
    /// the `PublisherTrustPolicy` instance
    ///
    /// ### Example
    /// ```
    /// use kelvin_brain::installed_plugins::PublisherTrustPolicy;
    ///
    /// let policy = PublisherTrustPolicy::default();
    /// let new_policy = policy.with_pinned_plugin_publisher("microsoft.backwards_compat", "microsoft-official");
    /// ```
    pub fn with_pinned_plugin_publisher(mut self, plugin_id: &str, publisher_id: &str) -> Self {
        self.pinned_plugin_publishers
            .insert(plugin_id.to_string(), publisher_id.to_string());
        self
    }

    /// ### Brief
    ///
    /// load a `PublisherTrustPolicy` from a json file
    ///
    /// ### Description
    ///
    /// parses a JSON file containing publisher trust configuration, including whether to require
    /// signatures, trusted publishers, revoked publishers, and plugin-to-publisher pinnings. returns
    /// an error if the file is missing, invalid JSON, or contains invalid ed25519 keys.
    ///
    /// ### Arguments
    /// * `path` - path to trust policy JSON file
    ///
    /// ### Returns
    /// a `PublisherTrustPolicy` instance with all configured publishers, revoked entries, and pinnings
    ///
    /// ### Errors
    /// - file I/O error
    /// - invalid JSON format
    /// - invalid ed25519 public keys in the file
    ///
    /// ### Example
    /// ```no_run
    /// use kelvin_brain::installed_plugins::PublisherTrustPolicy;
    /// use std::fs;
    ///
    /// // example trust policy JSON file:
    /// // {
    /// //   "require_signature": true,
    /// //   "publishers": [
    /// //     {
    /// //       "id": "acme-corp",
    /// //       "ed25519_public_key": "MCowBQYDK2VwAyEAu7..."
    /// //     }
    /// //   ],
    /// //   "revoked_publishers": ["malicious-pub"],
    /// //   "pinned_plugin_publishers": {
    /// //     "kelvin.echo": "acme-corp"
    /// //   }
    /// // }
    ///
    /// let policy = PublisherTrustPolicy::from_json_file("/home/user/.kelvinclaw/trusted_publishers.json")?;
    /// assert!(policy.require_signature);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_json_file(path: impl AsRef<Path>) -> KelvinResult<Self> {
        let text = fs::read_to_string(path.as_ref())?;
        let parsed: PublisherTrustPolicyFile = serde_json::from_str(&text).map_err(|err| {
            KelvinError::InvalidInput(format!("invalid publisher trust policy JSON: {err}"))
        })?;

        let mut policy = Self {
            require_signature: parsed.require_signature.unwrap_or(true),
            trusted_publishers: HashMap::new(),
            revoked_publishers: HashSet::new(),
            pinned_plugin_publishers: HashMap::new(),
        };
        for publisher in parsed.publishers {
            let key = parse_public_key(&publisher.ed25519_public_key)?;
            policy.trusted_publishers.insert(publisher.id, key);
        }
        for publisher_id in parsed.revoked_publishers {
            let cleaned = publisher_id.trim();
            if !cleaned.is_empty() {
                policy.revoked_publishers.insert(cleaned.to_string());
            }
        }
        for (plugin_id, publisher_id) in parsed.pinned_plugin_publishers {
            let plugin_id = plugin_id.trim();
            let publisher_id = publisher_id.trim();
            if !plugin_id.is_empty() && !publisher_id.is_empty() {
                policy
                    .pinned_plugin_publishers
                    .insert(plugin_id.to_string(), publisher_id.to_string());
            }
        }
        Ok(policy)
    }

    /// ### Brief
    ///
    /// verifies that a plugin manifest passes all trust policy checks and has a valid signature
    ///
    /// ### Description
    ///
    /// checks the manifest against pinned publishers, revoked publishers, and signature requirements.
    /// validates the manifest signature using the trusted publisher's ed25519 key if required by policy.
    /// returns an error if any trust policy is violated or signature verification fails.
    ///
    /// ### Arguments
    /// * `manifest` - the plugin package manifest to verify
    /// * `manifest_bytes` - the raw manifest bytes for signature verification
    /// * `version_dir` - the plugin version directory containing the optional signature file (plugin.sig)
    ///
    /// ### Returns
    /// none
    ///
    /// ### Errors
    /// - pinned publisher mismatch or missing when required
    /// - publisher is revoked
    /// - signature file missing when required
    /// - signature file empty or invalid format
    /// - invalid ed25519 signature
    /// - publisher is not trusted
    fn verify_manifest_signature(
        &self,
        manifest: &InstalledPluginPackageManifest,
        manifest_bytes: &[u8],
        version_dir: &Path,
    ) -> KelvinResult<()> {
        let signature_path = version_dir.join(consts::PLUGIN_SIGNATURE_FILENAME);
        let has_signature = signature_path.is_file();
        let quality_tier = manifest.quality_tier();

        if let Some(expected_publisher) = self.pinned_plugin_publishers.get(&manifest.id) {
            match manifest.publisher.as_deref() {
                Some(actual) if actual == expected_publisher => {}
                Some(actual) => {
                    return Err(KelvinError::InvalidInput(format!(
                        "plugin '{}' publisher '{}' does not match pinned publisher '{}'",
                        manifest.id, actual, expected_publisher
                    )));
                }
                None => {
                    return Err(KelvinError::InvalidInput(format!(
                        "plugin '{}' is missing publisher id required by pinning policy",
                        manifest.id
                    )));
                }
            }
        }

        if let Some(publisher) = manifest.publisher.as_deref() {
            if self.revoked_publishers.contains(publisher) {
                return Err(KelvinError::InvalidInput(format!(
                    "plugin '{}' publisher '{}' is revoked",
                    manifest.id, publisher
                )));
            }
        }

        if !self.require_signature && !has_signature {
            return Ok(());
        }

        if quality_tier == consts::QUALITY_TIER_UNSIGNED_LOCAL && !has_signature {
            if let Some(publisher) = manifest.publisher.as_deref() {
                if self.trusted_publishers.contains_key(publisher) {
                    return Err(KelvinError::InvalidInput(format!(
                        "plugin '{}' is missing required plugin.sig",
                        manifest.id
                    )));
                }
            }
            return Ok(());
        }

        let publisher = manifest.publisher.as_deref().ok_or_else(|| {
            KelvinError::InvalidInput(format!(
                "plugin '{}' is missing publisher id for signature verification",
                manifest.id
            ))
        })?;
        let verifier = self.trusted_publishers.get(publisher).ok_or_else(|| {
            KelvinError::InvalidInput(format!(
                "plugin '{}' publisher '{}' is not trusted",
                manifest.id, publisher
            ))
        })?;

        if !has_signature {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' is missing required plugin.sig",
                manifest.id
            )));
        }

        let signature_text = fs::read_to_string(&signature_path)?;
        let signature_base64 = signature_text.trim();
        if signature_base64.is_empty() {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' has empty plugin.sig",
                manifest.id
            )));
        }
        let signature_bytes = STANDARD.decode(signature_base64).map_err(|err| {
            KelvinError::InvalidInput(format!("invalid plugin.sig base64: {err}"))
        })?;
        let signature = Signature::from_slice(&signature_bytes).map_err(|err| {
            KelvinError::InvalidInput(format!("invalid ed25519 signature: {err}"))
        })?;

        verifier.verify(manifest_bytes, &signature).map_err(|err| {
            KelvinError::InvalidInput(format!(
                "plugin '{}' signature verification failed: {err}",
                manifest.id
            ))
        })?;
        Ok(())
    }
}

/// ### Brief
///
/// JSON deserialization format for publisher trust policy configuration
///
/// ### Description
///
/// internal struct for deserializing trust policy from JSON. all fields are optional with
/// sensible defaults. the loaded data is transformed into a `PublisherTrustPolicy` instance
/// which enforces trust and signature verification rules.
///
/// ### Fields
/// * `require_signature` - whether plugin signatures are mandatory; defaults to true if unspecified
/// * `publishers` - list of trusted publishers with their ed25519 public keys
/// * `revoked_publishers` - list of publisher IDs that are no longer trusted
/// * `pinned_plugin_publishers` - mapping of plugin IDs to their required publisher
#[derive(Debug, Deserialize)]
struct PublisherTrustPolicyFile {
    #[serde(default)]
    require_signature: Option<bool>,
    #[serde(default)]
    publishers: Vec<TrustedPublisherEntry>,
    #[serde(default)]
    revoked_publishers: Vec<String>,
    #[serde(default)]
    pinned_plugin_publishers: HashMap<String, String>,
}

/// ### Brief
///
/// JSON deserialization format for a trusted publisher entry
///
/// ### Fields
/// * `id` - publisher id
/// * `ed25519_public_key` - base64-encoded ed25519 public key
#[derive(Debug, Deserialize)]
struct TrustedPublisherEntry {
    id: String,
    ed25519_public_key: String,
}

/// ### Brief
///
/// JSON deserialization format for a plugin package manifest
///
/// ### Description
///
/// captures plugin metadata, capabilities, operational controls, and deployment configuration.
/// converted to a `PluginManifest` after validation. fields may have different defaults or
/// resolution logic (e.g., `tool_name` may be derived from `name` if not specified).
///
/// ### Fields
/// * `id` - unique plugin id (e.g., "publisher.plugin_name")
/// * `name` - human-readable name
/// * `version` - semantic version
/// * `api_version` - plugin API version
/// * `description` - optional description
/// * `homepage` - optional homepage URL
/// * `capabilities` - plugin capabilities (tool provider, model provider, etc.)
/// * `experimental` - whether this is an experimental plugin
/// * `min_core_version` - optional minimum kelvin core version required
/// * `max_core_version` - optional maximum kelvin core version allowed
/// * `runtime` - optional runtime kind override
/// * `tool_name` - optional registered tool/skill name
/// * `provider_name` - optional model provider name
/// * `provider_profile` - optional model provider profile configuration
/// * `model_name` - optional specific model name
/// * `entrypoint` - relative path to plugin entrypoint within payload/
/// * `entrypoint_sha256` - optional SHA-256 checksum of entrypoint for integrity verification
/// * `publisher` - optional publisher/author id
/// * `quality_tier` - optional quality tier (e.g., "unsigned_local", "signed_trusted")
/// * `capability_scopes` - scoped access rules for capabilities
/// * `operational_controls` - runtime limits (timeouts, retries, rate limiting)
/// * `tool_input_schema` - optional JSON schema for tool input validation
#[derive(Debug, Clone, Deserialize)]
struct InstalledPluginPackageManifest {
    id: String,
    name: String,
    version: String,
    api_version: String,
    description: Option<String>,
    homepage: Option<String>,
    #[serde(default)]
    capabilities: Vec<PluginCapability>,
    #[serde(default)]
    experimental: bool,
    min_core_version: Option<String>,
    max_core_version: Option<String>,
    runtime: Option<String>,
    tool_name: Option<String>,
    provider_name: Option<String>,
    provider_profile: Option<ModelProviderProfile>,
    model_name: Option<String>,
    entrypoint: String,
    entrypoint_sha256: Option<String>,
    publisher: Option<String>,
    quality_tier: Option<String>,
    #[serde(default)]
    capability_scopes: CapabilityScopesManifest,
    #[serde(default)]
    operational_controls: OperationalControlsManifest,
    #[serde(default)]
    tool_input_schema: Option<Value>,
}

/// ### Brief
///
/// JSON deserialization format for capability scopes within a plugin manifest
///
/// ### Fields
/// * `fs_read_paths` - allowed file system paths for reading
/// * `network_allow_hosts` - allowed network hosts (wildcard patterns supported)
/// * `env_allow` - allowed environment variables
#[derive(Debug, Clone, Default, Deserialize)]
struct CapabilityScopesManifest {
    #[serde(default)]
    fs_read_paths: Vec<String>,
    #[serde(default)]
    network_allow_hosts: Vec<String>,
    #[serde(default)]
    env_allow: Vec<String>,
}

/// ### Brief
///
/// JSON deserialization format for operational controls within a plugin manifest
///
/// ### Fields
/// * `timeout_ms` - execution timeout in milliseconds
/// * `max_retries` - maximum number of retries on failure
/// * `max_calls_per_minute` - rate limit for calls per minute
/// * `circuit_breaker_failures` - threshold number of failures before circuit breaker trips
/// * `circuit_breaker_cooldown_ms` - cooldown duration in milliseconds after circuit breaker trip
/// * `fuel_budget` - optional WASM fuel budget override; omit to use runtime default
#[derive(Debug, Clone, Deserialize)]
struct OperationalControlsManifest {
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
    #[serde(default = "default_max_retries")]
    max_retries: u32,
    #[serde(default = "default_max_calls_per_minute")]
    max_calls_per_minute: usize,
    #[serde(default = "default_circuit_breaker_failures")]
    circuit_breaker_failures: u32,
    #[serde(default = "default_circuit_breaker_cooldown_ms")]
    circuit_breaker_cooldown_ms: u64,
    #[serde(default)]
    fuel_budget: Option<u64>,
}

/// default operational controls with standard timeout and rate limitingconstruct default operational controls from constant defaults
impl Default for OperationalControlsManifest {
    fn default() -> Self {
        Self {
            timeout_ms: default_timeout_ms(),
            max_retries: default_max_retries(),
            max_calls_per_minute: default_max_calls_per_minute(),
            circuit_breaker_failures: default_circuit_breaker_failures(),
            circuit_breaker_cooldown_ms: default_circuit_breaker_cooldown_ms(),
            fuel_budget: None,
        }
    }
}

fn default_timeout_ms() -> u64 {
    consts::DEFAULT_TIMEOUT_MS
}

fn default_max_retries() -> u32 {
    consts::DEFAULT_MAX_RETRIES
}

fn default_max_calls_per_minute() -> usize {
    consts::DEFAULT_MAX_CALLS_PER_MINUTE
}

fn default_circuit_breaker_failures() -> u32 {
    consts::DEFAULT_CIRCUIT_BREAKER_FAILURES
}

fn default_circuit_breaker_cooldown_ms() -> u64 {
    consts::DEFAULT_CIRCUIT_BREAKER_COOLDOWN_MS
}

/// ### Brief
///
/// name and version resolution, format conversion, and field validation
impl InstalledPluginPackageManifest {
    /// ### Brief
    ///
    /// convert to a core plugin manifest
    fn to_core_manifest(&self) -> PluginManifest {
        PluginManifest {
            id: self.id.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
            api_version: self.api_version.clone(),
            description: self.description.clone(),
            homepage: self.homepage.clone(),
            capabilities: self.capabilities.clone(),
            experimental: self.experimental,
            min_core_version: self.min_core_version.clone(),
            max_core_version: self.max_core_version.clone(),
        }
    }

    /// ### Brief
    ///
    /// get the runtime kind, defaulting to "wasm_tool_v1"
    fn runtime_kind(&self) -> &str {
        self.runtime
            .as_deref()
            .unwrap_or(consts::DEFAULT_TOOL_RUNTIME_KIND)
            .trim()
    }

    /// ### Brief
    ///
    /// get the quality tier, defaulting to "unsigned_local"
    fn quality_tier(&self) -> &str {
        self.quality_tier
            .as_deref()
            .unwrap_or(consts::QUALITY_TIER_UNSIGNED_LOCAL)
            .trim()
    }

    /// ### Brief
    ///
    /// resolve the tool name with fallback logic and validation
    ///
    /// ### Description
    ///
    /// uses the explicit `tool_name` if provided, otherwise derives it from the plugin `id`
    /// by replacing dots with underscores. validates that the name is non-empty and contains
    /// only alphanumeric characters, underscores, hyphens, or dots.
    ///
    /// ### Returns
    /// resolved tool name
    ///
    /// ### Errors
    /// - resolved name is empty
    /// - resolved name contains invalid characters
    fn resolve_tool_name(&self) -> KelvinResult<String> {
        let fallback = self.id.replace('.', "_");
        let candidate = self
            .tool_name
            .as_deref()
            .unwrap_or(&fallback)
            .trim()
            .to_string();
        if candidate.is_empty() {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' has empty tool_name",
                self.id
            )));
        }
        if !candidate
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' has invalid tool_name '{}'",
                self.id, candidate
            )));
        }
        Ok(candidate)
    }

    /// ### Brief
    ///
    /// resolve the provider name with fallback and consistency validation
    ///
    /// ### Description
    ///
    /// uses the explicit `provider_name` if provided. if absent, derives from the provider profile name
    /// or falls back to the plugin `id` with dots replaced by underscores. validates non-emptiness,
    /// allowed characters, and consistency with the provider profile if present.
    ///
    /// ### Arguments
    /// * `provider_profile` - optional model provider profile for fallback and consistency checking
    ///
    /// ### Returns
    /// resolved provider name
    ///
    /// ### Errors
    /// - resolved name is empty
    /// - resolved name contains invalid characters
    /// - resolved name does not match provider profile name
    fn resolve_model_provider_name(
        &self,
        provider_profile: Option<&ModelProviderProfile>,
    ) -> KelvinResult<String> {
        let fallback = provider_profile
            .map(|profile| profile.provider_name.clone())
            .unwrap_or_else(|| self.id.replace('.', "_"));
        let candidate = self
            .provider_name
            .as_deref()
            .unwrap_or(&fallback)
            .trim()
            .to_string();
        if candidate.is_empty() {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' has empty provider_name",
                self.id
            )));
        }
        if !candidate
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' has invalid provider_name '{}'",
                self.id, candidate
            )));
        }
        if let Some(profile) = provider_profile {
            if candidate != profile.provider_name {
                return Err(KelvinError::InvalidInput(format!(
                    "plugin '{}' provider_name '{}' does not match provider_profile '{}'",
                    self.id, candidate, profile.id
                )));
            }
        }
        Ok(candidate)
    }

    /// ### Brief
    ///
    /// resolve the model name, defaulting to "default"
    ///
    /// ### Returns
    /// resolved model name
    fn resolved_model_name(&self) -> KelvinResult<String> {
        let fallback = "default";
        let candidate = self
            .model_name
            .as_deref()
            .unwrap_or(fallback)
            .trim()
            .to_string();
        if candidate.is_empty() {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' has empty model_name",
                self.id
            )));
        }
        Ok(candidate)
    }

    /// ### Brief
    ///
    /// get and validate the provider profile
    ///
    /// ### Returns
    /// model provider profile
    ///
    /// ### Errors
    /// - provider profile is missing
    /// - provider profile is invalid
    fn resolved_provider_profile(&self) -> KelvinResult<ModelProviderProfile> {
        let Some(profile) = self.provider_profile.clone() else {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' requires a structured provider_profile object",
                self.id
            )));
        };
        profile.validate()?;
        Ok(profile)
    }
}

/// ### Brief
///
/// tracks which ABI imports a model plugin uses
///
/// ### Fields
/// * `uses_openai_import` - whether the plugin imports the openai ABI module
/// * `uses_provider_profile_import` - whether the plugin imports the provider_profile ABI module
#[derive(Debug, Clone, Copy, Default)]
struct ModelPluginAbiUsage {
    uses_openai_import: bool,
    uses_provider_profile_import: bool,
}

fn resolve_model_provider_profile(
    manifest: &InstalledPluginPackageManifest,
    abi_usage: ModelPluginAbiUsage,
) -> KelvinResult<Option<ModelProviderProfile>> {
    let profile = manifest.resolved_provider_profile()?;
    if abi_usage.uses_openai_import
        && profile.protocol_family != ModelProviderProtocolFamily::OpenAiResponses
    {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' uses legacy openai import but provider_profile '{}' is not compatible",
            manifest.id, profile.id
        )));
    }
    Ok(Some(profile))
}

/// ### Brief
///
/// shared runtime state for a plugin's call history and circuit breaker
///
/// ### Fields
/// * `call_timestamps` - queue of recent call timestamps for rate limiting
/// * `consecutive_failures` - count of consecutive call failures
/// * `circuit_open_until` - optional deadline when circuit breaker will close, if currently open
#[derive(Debug, Default)]
struct OperationalState {
    call_timestamps: VecDeque<Instant>,
    consecutive_failures: u32,
    circuit_open_until: Option<Instant>,
}

/// ### Brief
///
/// a WASM tool plugin loaded and ready for execution
///
/// ### Description
///
/// wraps a WASM skill (tool) with its metadata, sandbox configuration, scopes, runtime controls,
/// and shared operational state (for rate limiting and circuit breaking). implements the `Tool` trait
/// to be called by the skill invocation system.
///
/// ### Fields
/// * `plugin_id` - unique plugin identifier
/// * `plugin_version` - semantic version
/// * `tool_name` - registered tool/skill name
/// * `tool_description` - human-readable description
/// * `tool_input_schema` - JSON schema for input validation
/// * `entrypoint_abs` - absolute path to the WASM module
/// * `host` - WASM runtime host for skill execution
/// * `sandbox_policy` - sandbox configuration (capabilities, imports allowed)
/// * `scopes` - capability scopes (fs read paths, network hosts, env vars)
/// * `controls` - operational limits (timeout, retries, rate limit)
/// * `state` - shared runtime state under lock (call timestamps, failures, circuit breaker)
#[derive(Clone)]
struct InstalledWasmTool {
    plugin_id: String,
    plugin_version: String,
    tool_name: String,
    tool_description: String,
    tool_input_schema: Value,
    entrypoint_abs: PathBuf,
    host: Arc<WasmSkillHost>,
    sandbox_policy: SandboxPolicy,
    scopes: CapabilityScopes,
    controls: OperationalControls,
    state: Arc<Mutex<OperationalState>>,
}

/// construction, execution control, and safety enforcement for installed tools
impl InstalledWasmTool {
    /// ### Brief
    ///
    /// construct a new installed WASM tool
    #[allow(clippy::too_many_arguments)]
    fn new(
        plugin_id: String,
        plugin_version: String,
        tool_name: String,
        tool_description: String,
        tool_input_schema: Value,
        entrypoint_abs: PathBuf,
        host: Arc<WasmSkillHost>,
        sandbox_policy: SandboxPolicy,
        scopes: CapabilityScopes,
        controls: OperationalControls,
    ) -> Self {
        Self {
            plugin_id,
            plugin_version,
            tool_name,
            tool_description,
            tool_input_schema,
            entrypoint_abs,
            host,
            sandbox_policy,
            scopes,
            controls,
            state: Arc::new(Mutex::new(OperationalState::default())),
        }
    }

    /// ### Brief
    ///
    /// validate tool arguments against capability scopes (e.g., fs_read paths)
    ///
    /// ### Arguments
    /// * `args` - tool arguments as JSON value
    ///
    /// ### Returns
    /// unit on success
    ///
    /// ### Errors
    /// - required argument missing or wrong type
    /// - argument value does not match scoped allowlist
    fn enforce_scoped_arguments(&self, args: &serde_json::Value) -> KelvinResult<()> {
        if self.sandbox_policy.allow_fs_read {
            let target_path = args
                .get("target_path")
                .and_then(|value| value.as_str())
                .ok_or_else(|| {
                    KelvinError::InvalidInput(format!(
                        "tool '{}' requires string argument 'target_path' when fs_read is enabled",
                        self.tool_name
                    ))
                })?;
            let normalized = normalize_safe_relative_path(target_path, "target_path")?;
            if !scope_match(&normalized, &self.scopes.fs_read_paths) {
                return Err(KelvinError::InvalidInput(format!(
                    "tool '{}' denied target_path '{}' (outside allowed fs_read scopes)",
                    self.tool_name, normalized
                )));
            }
        }

        // Network host enforcement is handled at the WASM sandbox level (kelvin-wasm).
        // wasm_tool_v1 plugins declare their fixed hosts in capability_scopes.network_allow_hosts
        // and the sandbox prevents any connection outside that list — no caller argument needed.

        Ok(())
    }

    /// ### Brief
    ///
    /// check circuit breaker and rate limit before allowing a call
    ///
    /// ### Description
    ///
    /// mutates shared state under lock: checks if circuit breaker is open, updates call timestamp queue,
    /// and verifies call rate is within limit.
    ///
    /// ### Errors
    /// - circuit breaker is open
    /// - call rate limit exceeded
    async fn reserve_call_budget(&self) -> KelvinResult<()> {
        let now = Instant::now();
        let mut state = self.state.lock().await;

        if let Some(open_until) = state.circuit_open_until {
            if now < open_until {
                return Err(KelvinError::Backend(format!(
                    "tool '{}' circuit breaker is open; retry later",
                    self.tool_name
                )));
            }
            state.circuit_open_until = None;
            state.consecutive_failures = 0;
        }

        let window = Duration::from_secs(consts::MEMORY_WINDOW_SECS);
        while let Some(ts) = state.call_timestamps.front() {
            if now.duration_since(*ts) > window {
                state.call_timestamps.pop_front();
            } else {
                break;
            }
        }

        if state.call_timestamps.len() >= self.controls.max_calls_per_minute {
            return Err(KelvinError::Timeout(format!(
                "tool '{}' exceeded call budget ({} calls/minute)",
                self.tool_name, self.controls.max_calls_per_minute
            )));
        }
        state.call_timestamps.push_back(now);
        Ok(())
    }

    /// ### Brief
    ///
    /// reset consecutive failures to zero on a successful call
    async fn mark_success(&self) {
        let mut state = self.state.lock().await;
        state.consecutive_failures = 0;
    }

    /// ### Brief
    ///
    /// increment consecutive failures and conditionally trip the circuit breaker
    ///
    /// ### Note
    ///
    /// when consecutive failures reach the circuit breaker threshold, the circuit opens
    /// and rejects subsequent calls until the cooldown period expires.
    async fn mark_failure(&self) {
        let mut state = self.state.lock().await;
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        if state.consecutive_failures >= self.controls.circuit_breaker_failures {
            state.circuit_open_until = Some(
                Instant::now() + Duration::from_millis(self.controls.circuit_breaker_cooldown_ms),
            );
            state.consecutive_failures = 0;
        }
    }

    /// ### Brief
    ///
    /// execute the tool once with the given arguments under timeout
    ///
    /// ### Arguments
    /// * `arguments` - tool arguments as JSON
    ///
    /// ### Returns
    /// skill execution result
    ///
    /// ### Errors
    /// - JSON serialization failure
    /// - WASM execution timeout
    /// - WASM execution error
    async fn execute_once(&self, arguments: &Value) -> KelvinResult<kelvin_wasm::SkillExecution> {
        let host = self.host.clone();
        let entrypoint = self.entrypoint_abs.clone();
        let policy = self.sandbox_policy.clone();
        let input_json = serde_json::to_string(arguments).map_err(|err| {
            KelvinError::InvalidInput(format!(
                "serialize tool arguments for plugin '{}': {err}",
                self.plugin_id
            ))
        })?;

        let mut task = tokio::task::spawn_blocking(move || {
            host.run_file_with_input(entrypoint, &input_json, policy)
        });
        match time::timeout(Duration::from_millis(self.controls.timeout_ms), &mut task).await {
            Ok(join_result) => join_result
                .map_err(|err| KelvinError::Backend(format!("tool task join failure: {err}")))?,
            Err(_) => {
                task.abort();
                Err(KelvinError::Timeout(format!(
                    "tool '{}' timed out after {}ms",
                    self.tool_name, self.controls.timeout_ms
                )))
            }
        }
    }
}

/// ### Brief
///
/// a WASM model provider plugin loaded and ready for inference
///
/// ### Description
///
/// wraps a WASM model (inference engine) with its metadata, provider profile, scopes, runtime controls,
/// and shared operational state (for rate limiting and circuit breaking). implements the `ModelProvider` trait
/// to be called by the model invocation system.
///
/// ### Fields
/// * `plugin_id` - unique plugin identifier
/// * `plugin_version` - semantic version
/// * `provider_name` - registered model provider name
/// * `model_name` - specific model name
/// * `provider_profile` - optional model provider profile (protocol family, API keys, etc.)
/// * `entrypoint_abs` - absolute path to the WASM module
/// * `host` - WASM runtime host for model execution
/// * `scopes` - capability scopes (fs read paths, network hosts, env vars)
/// * `controls` - operational limits (timeout, retries, rate limit)
/// * `state` - shared runtime state under lock (call timestamps, failures, circuit breaker)
#[derive(Clone)]
struct InstalledWasmModelProvider {
    plugin_id: String,
    plugin_version: String,
    provider_name: String,
    model_name: String,
    provider_profile: Option<ModelProviderProfile>,
    entrypoint_abs: PathBuf,
    host: Arc<WasmModelHost>,
    scopes: CapabilityScopes,
    controls: OperationalControls,
    state: Arc<Mutex<OperationalState>>,
}

/// construction, execution control, and profile management for installed model providers
impl InstalledWasmModelProvider {
    /// ### Brief
    ///
    /// construct a new installed WASM model provider
    #[allow(clippy::too_many_arguments)]
    fn new(
        plugin_id: String,
        plugin_version: String,
        provider_name: String,
        model_name: String,
        provider_profile: Option<ModelProviderProfile>,
        entrypoint_abs: PathBuf,
        host: Arc<WasmModelHost>,
        scopes: CapabilityScopes,
        controls: OperationalControls,
    ) -> Self {
        Self {
            plugin_id,
            plugin_version,
            provider_name,
            model_name,
            provider_profile,
            entrypoint_abs,
            host,
            scopes,
            controls,
            state: Arc::new(Mutex::new(OperationalState::default())),
        }
    }

    /// ### Brief
    ///
    /// build a sandbox policy for this model provider
    ///
    /// ### Returns
    /// model sandbox policy with network scopes and timeout
    fn sandbox_policy(&self) -> ModelSandboxPolicy {
        ModelSandboxPolicy {
            network_allow_hosts: self.scopes.network_allow_hosts.clone(),
            timeout_ms: self.controls.timeout_ms,
            provider_profile: self.provider_profile.clone(),
            model_name: Some(self.model_name.clone()),
            ..ModelSandboxPolicy::default()
        }
    }

    /// ### Brief
    ///
    /// check circuit breaker and rate limit before allowing a call
    ///
    /// ### Description
    ///
    /// mutates shared state under lock: checks if circuit breaker is open, updates call timestamp queue,
    /// and verifies call rate is within limit.
    ///
    /// ### Errors
    /// - circuit breaker is open
    /// - call rate limit exceeded
    async fn reserve_call_budget(&self) -> KelvinResult<()> {
        let now = Instant::now();
        let mut state = self.state.lock().await;

        if let Some(open_until) = state.circuit_open_until {
            if now < open_until {
                return Err(KelvinError::Backend(format!(
                    "model provider '{}:{}' circuit breaker is open; retry later",
                    self.provider_name, self.model_name
                )));
            }
            state.circuit_open_until = None;
            state.consecutive_failures = 0;
        }

        let window = Duration::from_secs(consts::MEMORY_WINDOW_SECS);
        while let Some(ts) = state.call_timestamps.front() {
            if now.duration_since(*ts) > window {
                state.call_timestamps.pop_front();
            } else {
                break;
            }
        }

        if state.call_timestamps.len() >= self.controls.max_calls_per_minute {
            return Err(KelvinError::Timeout(format!(
                "model provider '{}:{}' exceeded call budget ({} calls/minute)",
                self.provider_name, self.model_name, self.controls.max_calls_per_minute
            )));
        }
        state.call_timestamps.push_back(now);
        Ok(())
    }

    /// ### Brief
    ///
    /// reset consecutive failures to zero on a successful call
    async fn mark_success(&self) {
        let mut state = self.state.lock().await;
        state.consecutive_failures = 0;
    }

    /// ### Brief
    ///
    /// increment consecutive failures and conditionally trip the circuit breaker
    ///
    /// ### Note
    ///
    /// when consecutive failures reach the circuit breaker threshold, the circuit opens
    /// and rejects subsequent calls until the cooldown period expires.
    async fn mark_failure(&self) {
        let mut state = self.state.lock().await;
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        if state.consecutive_failures >= self.controls.circuit_breaker_failures {
            state.circuit_open_until = Some(
                Instant::now() + Duration::from_millis(self.controls.circuit_breaker_cooldown_ms),
            );
            state.consecutive_failures = 0;
        }
    }

    /// ### Brief
    ///
    /// execute the model provider once with the given input under timeout
    ///
    /// ### Arguments
    /// * `input_json` - input as JSON string
    ///
    /// ### Returns
    /// output as JSON string
    ///
    /// ### Errors
    /// - WASM execution timeout
    /// - WASM execution error
    async fn execute_once(&self, input_json: String) -> KelvinResult<String> {
        let host = self.host.clone();
        let entrypoint = self.entrypoint_abs.clone();
        let policy = self.sandbox_policy();

        let mut task =
            tokio::task::spawn_blocking(move || host.run_file(entrypoint, &input_json, policy));
        match time::timeout(Duration::from_millis(self.controls.timeout_ms), &mut task).await {
            Ok(join_result) => join_result.map_err(|err| {
                KelvinError::Backend(format!("model provider task join failure: {err}"))
            })?,
            Err(_) => {
                task.abort();
                Err(KelvinError::Timeout(format!(
                    "model provider '{}:{}' timed out after {}ms",
                    self.provider_name, self.model_name, self.controls.timeout_ms
                )))
            }
        }
    }

    /// ### Brief
    ///
    /// parse and validate model output, adapting provider-specific formats if needed
    ///
    /// ### Description
    ///
    /// parses JSON output from the model plugin. checks for error fields, validates against
    /// `ModelOutput` schema, and attempts provider-profile-aware format adaptation if the
    /// direct deserialization fails. returns the normalized `ModelOutput`.
    ///
    /// ### Arguments
    /// * `output_json` - model output as JSON string
    ///
    /// ### Returns
    /// normalized model output
    ///
    /// ### Errors
    /// - invalid JSON format
    /// - model returned an error message
    /// - output schema validation failed
    /// - no valid output format found
    fn decode_output_payload(&self, output_json: &str) -> KelvinResult<ModelOutput> {
        let value: Value = serde_json::from_str(output_json).map_err(|err| {
            KelvinError::InvalidInput(format!(
                "model plugin '{}' returned invalid json: {err}",
                self.plugin_id
            ))
        })?;

        if let Some(message) = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(|message| message.as_str())
        {
            return Err(KelvinError::Backend(format!(
                "model plugin '{}@{}' failed: {}",
                self.plugin_id, self.plugin_version, message
            )));
        }

        if let Ok(output) = serde_json::from_value::<ModelOutput>(value.clone()) {
            if let Err(validation_err) = validate_model_output_schema(&value) {
                return Err(KelvinError::InvalidInput(format!(
                    "model plugin '{}' returned invalid ModelOutput schema: {validation_err}",
                    self.plugin_id
                )));
            }
            return Ok(output);
        }

        if let Some(profile) = self.provider_profile.as_ref() {
            if let Some(output) = adapt_provider_response(profile, &value) {
                return Ok(output);
            }
            return Err(KelvinError::InvalidInput(format!(
                "model plugin '{}' returned response that doesn't match {:?} protocol format",
                self.plugin_id, profile.protocol_family
            )));
        }

        Err(KelvinError::InvalidInput(format!(
            "model plugin '{}' returned response that is neither valid ModelOutput nor mapped to a known protocol family",
            self.plugin_id
        )))
    }
}

/// ### Brief
///
/// get the JSON type name of a value for error messages
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// ### Brief
///
/// validate that a JSON value matches the expected `ModelOutput` schema
///
/// ### Description
///
/// checks that the value is a JSON object with required fields: `assistant_text` (string) and
/// `tool_calls` (array). each tool call must have `id`, `name`, and `arguments` fields with
/// proper types (`name` non-empty string, `arguments` JSON object).
///
/// ### Arguments
/// * `value` - the JSON value to validate
///
/// ### Returns
/// unit on success, or an error message describing the validation failure
fn validate_model_output_schema(value: &Value) -> Result<(), String> {
    let obj = value.as_object().ok_or("response is not a JSON object")?;

    let assistant_text = obj
        .get("assistant_text")
        .ok_or("missing required field: assistant_text")?;
    if !assistant_text.is_string() {
        return Err("field 'assistant_text' must be a string".to_string());
    }

    let tool_calls = obj
        .get("tool_calls")
        .ok_or("missing required field: tool_calls")?;
    let tool_calls_array = tool_calls
        .as_array()
        .ok_or("field 'tool_calls' must be an array")?;

    for (idx, tool_call) in tool_calls_array.iter().enumerate() {
        let tc_obj = tool_call
            .as_object()
            .ok_or_else(|| format!("tool_calls[{idx}] is not a JSON object"))?;

        let id = tc_obj
            .get("id")
            .ok_or_else(|| format!("tool_calls[{idx}] missing required field: id"))?;
        if !id.is_string() {
            return Err(format!("tool_calls[{idx}].id must be a string"));
        }

        let name = tc_obj
            .get("name")
            .ok_or_else(|| format!("tool_calls[{idx}] missing required field: name"))?;
        if !name.is_string() {
            return Err(format!("tool_calls[{idx}].name must be a string"));
        }

        let arguments = tc_obj
            .get("arguments")
            .ok_or_else(|| format!("tool_calls[{idx}] missing required field: arguments"))?;
        if !arguments.is_object() {
            return Err(format!(
                "tool_calls[{idx}].arguments must be a JSON object, got {}",
                json_type_name(arguments)
            ));
        }

        if name.as_str().unwrap_or("").trim().is_empty() {
            return Err(format!("tool_calls[{idx}].name must not be empty"));
        }
    }

    if let Some(stop_reason) = obj.get("stop_reason") {
        if !stop_reason.is_string() && !stop_reason.is_null() {
            return Err("field 'stop_reason' must be a string or null".to_string());
        }
    }

    if let Some(usage) = obj.get("usage") {
        if !usage.is_object() && !usage.is_null() {
            return Err("field 'usage' must be an object or null".to_string());
        }
    }

    Ok(())
}

/// ### Brief
///
/// validate that a JSON value matches the openai responses format
///
/// ### Description
///
/// checks that the value has either an `output_text` string field or an `output` array field.
/// if `output` exists, validates that each element has `message` (with `type` field) or `function_call` fields.
///
/// ### Arguments
/// * `value` - the JSON value to validate
///
/// ### Returns
/// unit on success, or an error message describing the validation failure
fn validate_openai_response_schema(value: &Value) -> Result<(), String> {
    let obj = value.as_object().ok_or("response is not a JSON object")?;

    let has_output_text = obj
        .get("output_text")
        .map(|v| v.is_string())
        .unwrap_or(false);
    let has_output = obj.get("output").map(|v| v.is_array()).unwrap_or(false);

    if !has_output_text && !has_output {
        return Err(
            "OpenAI Responses: must have 'output_text' (string) or 'output' (array)".to_string(),
        );
    }

    if let Some(output_array) = obj.get("output").and_then(Value::as_array) {
        for (idx, item) in output_array.iter().enumerate() {
            let item_obj = item
                .as_object()
                .ok_or_else(|| format!("OpenAI Responses output[{idx}] is not a JSON object"))?;

            let item_type = item_obj
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("OpenAI Responses output[{idx}] missing field: type"))?;

            match item_type {
                "message" => {
                    let content = item_obj.get("content").ok_or_else(|| {
                        format!(
                            "OpenAI Responses output[{idx}] (type=message) missing field: content"
                        )
                    })?;
                    if !content.is_array() {
                        return Err(format!(
                            "OpenAI Responses output[{idx}].content must be an array"
                        ));
                    }
                }
                "function_call" => {
                    if !item_obj.contains_key("call_id") && !item_obj.contains_key("name") {
                        return Err(format!(
                            "OpenAI Responses output[{idx}] (type=function_call) must have 'call_id' or 'name'"
                        ));
                    }
                }
                _ => {
                    return Err(format!(
                        "OpenAI Responses output[{idx}] has unknown type: {item_type}"
                    ))
                }
            }
        }
    }

    Ok(())
}

/// ### Brief
///
/// validate that a JSON value matches the anthropic messages format
///
/// ### Description
///
/// checks that the value has a `content` array field. each content block must have either a `type` field
/// for text blocks or `input` field for tool_use blocks.
///
/// ### Arguments
/// * `value` - the JSON value to validate
///
/// ### Returns
/// unit on success, or an error message describing the validation failure
fn validate_anthropic_response_schema(value: &Value) -> Result<(), String> {
    let obj = value.as_object().ok_or("response is not a JSON object")?;

    let content = obj
        .get("content")
        .ok_or("Anthropic Messages: missing required field: content")?;
    let content_array = content
        .as_array()
        .ok_or("Anthropic Messages: field 'content' must be an array")?;

    for (idx, block) in content_array.iter().enumerate() {
        let block_obj = block
            .as_object()
            .ok_or_else(|| format!("Anthropic Messages content[{idx}] is not a JSON object"))?;

        let block_type = block_obj
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Anthropic Messages content[{idx}] missing field: type"))?;

        match block_type {
            "text" => {
                if !block_obj.contains_key("text") {
                    return Err(format!(
                        "Anthropic Messages content[{idx}] (type=text) missing field: text"
                    ));
                }
            }
            "tool_use" => {
                for required_field in &["id", "name", "input"] {
                    if !block_obj.contains_key(*required_field) {
                        return Err(format!(
                            "Anthropic Messages content[{idx}] (type=tool_use) missing field: {required_field}"
                        ));
                    }
                }
            }
            _ => {
                return Err(format!(
                    "Anthropic Messages content[{idx}] has unknown type: {block_type}"
                ))
            }
        }
    }

    Ok(())
}

/// ### Brief
///
/// validate that a JSON value matches the openai chat completions format
///
/// ### Description
///
/// checks that the value has a non-empty `choices` array. the first element must have a `message`
/// object field with a `role` field present.
///
/// ### Arguments
/// * `value` - the JSON value to validate
///
/// ### Returns
/// unit on success, or an error message describing the validation failure
fn validate_openai_chat_completions_schema(value: &Value) -> Result<(), String> {
    let obj = value.as_object().ok_or("response is not a JSON object")?;

    let choices = obj
        .get("choices")
        .ok_or("OpenAI Chat Completions: missing required field: choices")?;
    let choices_array = choices
        .as_array()
        .ok_or("OpenAI Chat Completions: field 'choices' must be an array")?;

    if choices_array.is_empty() {
        return Err("OpenAI Chat Completions: choices array must not be empty".to_string());
    }

    let first_choice = choices_array[0]
        .as_object()
        .ok_or("OpenAI Chat Completions: choices[0] is not a JSON object")?;

    let message = first_choice
        .get("message")
        .ok_or("OpenAI Chat Completions: choices[0] missing field: message")?;
    let message_obj = message
        .as_object()
        .ok_or("OpenAI Chat Completions: choices[0].message must be a JSON object")?;

    if !message_obj.contains_key("role") {
        return Err("OpenAI Chat Completions: choices[0].message missing field: role".to_string());
    }

    if let Some(tool_calls) = message_obj.get("tool_calls") {
        if !tool_calls.is_array() && !tool_calls.is_null() {
            return Err(
                "OpenAI Chat Completions: choices[0].message.tool_calls must be an array or null"
                    .to_string(),
            );
        }
    }

    Ok(())
}

fn adapt_provider_response(profile: &ModelProviderProfile, value: &Value) -> Option<ModelOutput> {
    match profile.protocol_family {
        ModelProviderProtocolFamily::OpenAiResponses => {
            validate_openai_response_schema(value).ok()?;
            adapt_openai_response(value)
        }
        ModelProviderProtocolFamily::AnthropicMessages => {
            validate_anthropic_response_schema(value).ok()?;
            adapt_anthropic_response(value)
        }
        ModelProviderProtocolFamily::OpenAiChatCompletions => {
            validate_openai_chat_completions_schema(value).ok()?;
            adapt_openrouter_response(value)
        }
    }
}

fn adapt_openai_response(value: &Value) -> Option<ModelOutput> {
    let mut tool_calls = Vec::new();
    let output_text = value
        .get("output_text")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            let output = value.get("output")?.as_array()?;
            let mut parts = Vec::new();
            for item in output {
                let item_type = item.get("type").and_then(Value::as_str);
                if item_type == Some("function_call") {
                    let id = item
                        .get("call_id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let arguments = item
                        .get("arguments")
                        .and_then(Value::as_str)
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_else(|| serde_json::json!({}));
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments,
                    });
                    continue;
                }
                if item_type != Some("message") {
                    continue;
                }
                let content = match item.get("content")?.as_array() {
                    Some(c) => c,
                    None => continue,
                };
                for block in content {
                    if block.get("type").and_then(Value::as_str) == Some("output_text") {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            parts.push(text.to_string());
                        }
                    }
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        });

    let assistant_text = output_text.unwrap_or_default();
    if assistant_text.is_empty() && tool_calls.is_empty() {
        return None;
    }

    let usage = value.get("usage").and_then(adapt_usage);
    let stop_reason = value
        .get("status")
        .and_then(Value::as_str)
        .map(str::to_owned);

    Some(ModelOutput {
        assistant_text,
        stop_reason,
        tool_calls,
        usage,
    })
}

fn adapt_anthropic_response(value: &Value) -> Option<ModelOutput> {
    let content = value.get("content")?.as_array()?;
    let mut parts = Vec::new();
    let mut tool_calls = Vec::new();
    for block in content {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    parts.push(text.to_string());
                }
            }
            Some("tool_use") => {
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let arguments = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments,
                });
            }
            _ => {}
        }
    }
    if parts.is_empty() && tool_calls.is_empty() {
        return None;
    }

    let usage = value.get("usage").and_then(adapt_usage);
    let stop_reason = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(str::to_owned);

    Some(ModelOutput {
        assistant_text: parts.join("\n"),
        stop_reason,
        tool_calls,
        usage,
    })
}

fn adapt_openrouter_response(value: &Value) -> Option<ModelOutput> {
    let choices = value.get("choices")?.as_array()?;
    let first_choice = choices.first()?;
    let message = first_choice.get("message")?;

    let assistant_text = match message.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => {
            let mut text_parts = Vec::new();
            for part in parts {
                if part.get("type").and_then(Value::as_str) == Some("text") {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        text_parts.push(text.to_string());
                    }
                }
            }
            text_parts.join("\n")
        }
        // null content is normal when the response only contains tool calls
        Some(Value::Null) | None => String::new(),
        _ => return None,
    };

    let mut tool_calls = Vec::new();
    if let Some(tc_array) = message.get("tool_calls").and_then(Value::as_array) {
        for tc in tc_array {
            let id = tc
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let function = tc.get("function");
            let name = function
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let arguments = function
                .and_then(|f| f.get("arguments"))
                .and_then(Value::as_str)
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_else(|| serde_json::json!({}));
            tool_calls.push(ToolCall {
                id,
                name,
                arguments,
            });
        }
    }

    if assistant_text.is_empty() && tool_calls.is_empty() {
        return None;
    }

    let usage = value.get("usage").and_then(adapt_openrouter_usage);
    let stop_reason = first_choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(str::to_owned);

    Some(ModelOutput {
        assistant_text,
        stop_reason,
        tool_calls,
        usage,
    })
}

fn adapt_usage(usage: &Value) -> Option<kelvin_core::ModelUsage> {
    let input_tokens = usage.get("input_tokens").and_then(Value::as_u64);
    let output_tokens = usage.get("output_tokens").and_then(Value::as_u64);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .or_else(|| match (input_tokens, output_tokens) {
            (Some(input), Some(output)) => Some(input + output),
            _ => None,
        });

    if input_tokens.is_none() && output_tokens.is_none() && total_tokens.is_none() {
        return None;
    }

    Some(kelvin_core::ModelUsage {
        input_tokens,
        output_tokens,
        total_tokens,
    })
}

fn adapt_openrouter_usage(usage: &Value) -> Option<kelvin_core::ModelUsage> {
    let input_tokens = usage.get("prompt_tokens").and_then(Value::as_u64);
    let output_tokens = usage.get("completion_tokens").and_then(Value::as_u64);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .or_else(|| match (input_tokens, output_tokens) {
            (Some(input), Some(output)) => Some(input + output),
            _ => None,
        });

    if input_tokens.is_none() && output_tokens.is_none() && total_tokens.is_none() {
        return None;
    }

    Some(kelvin_core::ModelUsage {
        input_tokens,
        output_tokens,
        total_tokens,
    })
}

/// tool interface for executing WASM-based tools with budget and scoping
#[async_trait]
impl Tool for InstalledWasmTool {
    /// ### Brief
    ///
    /// get the tool name
    fn name(&self) -> &str {
        &self.tool_name
    }

    /// ### Brief
    ///
    /// get the tool description
    fn description(&self) -> &str {
        &self.tool_description
    }

    /// ### Brief
    ///
    /// get the tool input schema
    fn input_schema(&self) -> Value {
        self.tool_input_schema.clone()
    }

    /// ### Brief
    ///
    /// call the tool with scope enforcement, budget limits, and retry logic
    ///
    /// ### Description
    ///
    /// enforces scoped arguments, reserves call budget (rate limit, circuit breaker), and executes
    /// the tool with up to `max_retries` attempts. marks success or failure on the operational state.
    ///
    /// ### Arguments
    /// * `input` - tool call input with arguments
    ///
    /// ### Returns
    /// tool call result with output or error
    ///
    /// ### Errors
    /// - scoped arguments validation fails
    /// - circuit breaker is open
    /// - call rate limit exceeded
    /// - all retry attempts failed
    async fn call(&self, input: ToolCallInput) -> KelvinResult<ToolCallResult> {
        self.enforce_scoped_arguments(&input.arguments)?;
        self.reserve_call_budget().await?;

        let mut last_error = None;
        for attempt in 0..=self.controls.max_retries {
            match self.execute_once(&input.arguments).await {
                Ok(execution) => {
                    self.mark_success().await;
                    let summary = format!(
                        "{} executed exit={} calls={} plugin={}@{}",
                        self.tool_name,
                        execution.exit_code,
                        execution.calls.len(),
                        self.plugin_id,
                        self.plugin_version
                    );

                    // v2: use output_json directly if the module produced structured output
                    if let Some(ref output_json) = execution.output_json {
                        return Ok(ToolCallResult {
                            summary: summary.clone(),
                            output: Some(output_json.clone()),
                            visible_text: Some(summary),
                            is_error: false,
                        });
                    }

                    // v1 fallback: format the calls list
                    let calls = execution
                        .calls
                        .iter()
                        .map(claw_call_json)
                        .collect::<Vec<_>>();
                    let output = json!({
                        "plugin_id": self.plugin_id,
                        "plugin_version": self.plugin_version,
                        "entrypoint": self.entrypoint_abs.to_string_lossy(),
                        "exit_code": execution.exit_code,
                        "calls": calls,
                    });
                    return Ok(ToolCallResult {
                        summary: summary.clone(),
                        output: Some(output.to_string()),
                        visible_text: Some(summary),
                        is_error: false,
                    });
                }
                Err(err) => {
                    last_error = Some(err);
                    if attempt < self.controls.max_retries {
                        continue;
                    }
                }
            }
        }

        self.mark_failure().await;
        Err(last_error.unwrap_or_else(|| {
            KelvinError::Backend(format!(
                "tool '{}' failed without error detail",
                self.tool_name
            ))
        }))
    }
}

/// model provider interface for executing WASM-based models with budget and output validation
#[async_trait]
impl ModelProvider for InstalledWasmModelProvider {
    /// ### Brief
    ///
    /// get the provider name
    fn provider_name(&self) -> &str {
        &self.provider_name
    }

    /// ### Brief
    ///
    /// get the model name
    fn model_name(&self) -> &str {
        &self.model_name
    }

    /// ### Brief
    ///
    /// infer with the model using budget limits and retry logic
    ///
    /// ### Description
    ///
    /// reserves call budget (rate limit, circuit breaker), serializes input, and executes the model
    /// with up to `max_retries` attempts. parses and validates output, marks success or failure
    /// on the operational state.
    ///
    /// ### Arguments
    /// * `input` - model input with messages and configuration
    ///
    /// ### Returns
    /// model output with text and tool calls
    ///
    /// ### Errors
    /// - circuit breaker is open
    /// - call rate limit exceeded
    /// - all retry attempts failed
    async fn infer(&self, input: ModelInput) -> KelvinResult<ModelOutput> {
        self.reserve_call_budget().await?;
        let input_json = serde_json::to_string(&input).map_err(|err| {
            KelvinError::InvalidInput(format!(
                "serialize model input for plugin '{}': {err}",
                self.plugin_id
            ))
        })?;

        let mut last_error = None;
        for attempt in 0..=self.controls.max_retries {
            match self.execute_once(input_json.clone()).await {
                Ok(output_json) => {
                    let output = self.decode_output_payload(&output_json)?;
                    self.mark_success().await;
                    return Ok(output);
                }
                Err(err) => {
                    last_error = Some(err);
                    if attempt < self.controls.max_retries {
                        continue;
                    }
                }
            }
        }

        self.mark_failure().await;
        Err(last_error.unwrap_or_else(|| {
            KelvinError::Backend(format!(
                "model provider '{}:{}' failed without error detail",
                self.provider_name, self.model_name
            ))
        }))
    }
}

/// ### Brief
///
/// factory for creating tool and model provider instances from a loaded plugin
///
/// ### Fields
/// * `manifest` - plugin metadata and configuration
/// * `tool` - optional instantiated tool plugin
/// * `model_provider` - optional instantiated model provider plugin
struct InstalledWasmPluginFactory {
    manifest: PluginManifest,
    tool: Option<Arc<InstalledWasmTool>>,
    model_provider: Option<Arc<InstalledWasmModelProvider>>,
}

/// trait implementation for instantiated plugin access
impl PluginFactory for InstalledWasmPluginFactory {
    /// ### Brief
    ///
    /// get the plugin manifest
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// ### Brief
    ///
    /// get the tool if this plugin provides one
    fn tool(&self) -> Option<Arc<dyn Tool>> {
        self.tool.clone().map(|tool| tool as Arc<dyn Tool>)
    }

    /// ### Brief
    ///
    /// get the model provider if this plugin provides one
    fn model_provider(&self) -> Option<Arc<dyn ModelProvider>> {
        self.model_provider
            .clone()
            .map(|provider| provider as Arc<dyn ModelProvider>)
    }
}

/// ### Brief
///
/// load installed plugins from the configured directory
///
/// ### Arguments
/// * `config` - loader configuration with plugin home and policies
///
/// ### Returns
/// loaded plugins with tool and model registries
pub fn load_installed_tool_plugins(
    config: InstalledPluginLoaderConfig,
) -> KelvinResult<LoadedInstalledPlugins> {
    load_installed_plugins(config)
}

/// ### Brief
///
/// main entrypoint to discover, load, and register all installed plugins
///
/// ### Description
///
/// walks the plugin home directory, validates each plugin against trust policies, initializes
/// WASM hosts, populates tool and model registries, and returns all loaded plugins. enforces
/// security policies, signature verification, and capability scoping.
///
/// ### Arguments
/// * `config` - loader configuration with plugin home, core version, and policies
///
/// ### Returns
/// loaded plugins with aggregated tool and model provider registries
///
/// ### Errors
/// - plugin home directory not found or not readable
/// - trust policy file invalid
/// - plugin manifest missing or invalid JSON
/// - trust policy violation (revoked publisher, signature required, etc.)
/// - manifest validation failure (missing fields, invalid versions, etc.)
/// - entrypoint file not found or invalid
/// - WASM host initialization failure
pub fn load_installed_plugins(
    config: InstalledPluginLoaderConfig,
) -> KelvinResult<LoadedInstalledPlugins> {
    let plugin_registry = Arc::new(InMemoryPluginRegistry::new());
    let mut loaded_plugins = Vec::new();

    if !config.plugin_home.exists() {
        let tool_registry = Arc::new(SdkToolRegistry::from_plugin_registry(
            plugin_registry.as_ref(),
        )?);
        let model_registry = Arc::new(SdkModelProviderRegistry::from_plugin_registry(
            plugin_registry.as_ref(),
        )?);
        return Ok(LoadedInstalledPlugins {
            plugin_registry,
            tool_registry,
            model_registry,
            loaded_plugins,
        });
    }

    let plugin_dirs = collect_plugin_dirs(&config.plugin_home)?;
    let skill_host = Arc::new(WasmSkillHost::try_new()?);
    let model_host = Arc::new(WasmModelHost::try_new()?);
    for plugin_dir in plugin_dirs {
        let plugin = load_one_plugin(&plugin_dir, &config, skill_host.clone(), model_host.clone())?;

        // define plugin registry entry
        let loaded = LoadedInstalledPlugin {
            id: plugin.manifest.id.clone(),
            version: plugin.manifest.version.clone(),
            tool_name: plugin.tool.as_ref().map(|tool| tool.name().to_string()),
            provider_name: plugin
                .model_provider
                .as_ref()
                .map(|provider| provider.provider_name.clone()),
            model_name: plugin
                .model_provider
                .as_ref()
                .map(|provider| provider.model_name.clone()),
            provider_profile: plugin
                .model_provider
                .as_ref()
                .and_then(|provider| provider.provider_profile.as_ref())
                .map(|profile| profile.id.clone()),
            runtime: plugin.runtime.clone(),
            publisher: plugin.publisher.clone(),
        };

        // insert plugin to generic registry (Arc ref)
        plugin_registry.register(
            Arc::new(InstalledWasmPluginFactory {
                manifest: plugin.manifest,
                tool: plugin.tool,
                model_provider: plugin.model_provider,
            }),
            &config.core_version,
            &config.security_policy,
        )?;

        // insert to vector (owned)
        loaded_plugins.push(loaded);
    }

    loaded_plugins.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.version.cmp(&right.version))
            .then_with(|| left.runtime.cmp(&right.runtime))
            .then_with(|| left.tool_name.cmp(&right.tool_name))
            .then_with(|| left.provider_name.cmp(&right.provider_name))
            .then_with(|| left.model_name.cmp(&right.model_name))
            .then_with(|| left.provider_profile.cmp(&right.provider_profile))
    });

    let tool_registry = Arc::new(SdkToolRegistry::from_plugin_registry(
        plugin_registry.as_ref(),
    )?);
    let model_registry = Arc::new(SdkModelProviderRegistry::from_plugin_registry(
        plugin_registry.as_ref(),
    )?);
    Ok(LoadedInstalledPlugins {
        plugin_registry,
        tool_registry,
        model_registry,
        loaded_plugins,
    })
}

/// ### Brief
///
/// intermediate data for a loaded plugin before registry registration
///
/// ### Fields
/// * `manifest` - validated core plugin manifest
/// * `tool` - optional instantiated tool plugin
/// * `model_provider` - optional instantiated model provider plugin
/// * `runtime` - runtime kind (e.g., "wasm_tool_v1")
/// * `publisher` - optional publisher/author id
struct LoadedPluginFactoryData {
    manifest: PluginManifest,
    tool: Option<Arc<InstalledWasmTool>>,
    model_provider: Option<Arc<InstalledWasmModelProvider>>,
    runtime: String,
    publisher: Option<String>,
}

/// ### Brief
///
/// load and validate a single plugin from its directory
///
/// ### Description
///
/// reads and parses the plugin manifest, verifies trust policies and signatures, resolves the
/// version directory, validates the manifest, extracts scopes and controls, and instantiates
/// tool/model plugins as needed. returns intermediate factory data ready for registration.
///
/// ### Arguments
/// * `plugin_dir` - directory containing the plugin
/// * `config` - loader configuration
/// * `skill_host` - WASM runtime host for tools
/// * `model_host` - WASM runtime host for models
///
/// ### Returns
/// loaded plugin factory data with instantiated tool and/or model provider
///
/// ### Errors
/// - plugin id or manifest missing/invalid
/// - trust policy violation
/// - manifest validation failure
/// - entrypoint or payload missing
/// - scope/control validation failure
/// - WASM host instantiation error
fn load_one_plugin(
    plugin_dir: &Path,
    config: &InstalledPluginLoaderConfig,
    skill_host: Arc<WasmSkillHost>,
    model_host: Arc<WasmModelHost>,
) -> KelvinResult<LoadedPluginFactoryData> {
    let plugin_id = plugin_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| KelvinError::InvalidInput("invalid plugin directory name".to_string()))?;
    let version_dir = resolve_version_dir(plugin_dir)?;

    let manifest_path = version_dir.join("plugin.json");
    let manifest_bytes = fs::read(&manifest_path)?;
    let package_manifest: InstalledPluginPackageManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|err| {
            KelvinError::InvalidInput(format!(
                "invalid plugin manifest at {}: {err}",
                manifest_path.to_string_lossy()
            ))
        })?;

    if package_manifest.id != plugin_id {
        return Err(KelvinError::InvalidInput(format!(
            "plugin id mismatch: directory '{}' but manifest id '{}'",
            plugin_id, package_manifest.id
        )));
    }

    let runtime_kind = package_manifest.runtime_kind();
    if runtime_kind != consts::DEFAULT_TOOL_RUNTIME_KIND
        && runtime_kind != consts::DEFAULT_MODEL_RUNTIME_KIND
    {
        return Err(KelvinError::InvalidInput(format!(
            "unsupported plugin runtime '{}'; expected '{}' or '{}'",
            runtime_kind,
            consts::DEFAULT_TOOL_RUNTIME_KIND,
            consts::DEFAULT_MODEL_RUNTIME_KIND
        )));
    }

    config.trust_policy.verify_manifest_signature(
        &package_manifest,
        &manifest_bytes,
        &version_dir,
    )?;

    let core_manifest = package_manifest.to_core_manifest();
    core_manifest.validate()?;
    let entrypoint_rel = normalize_safe_relative_path(&package_manifest.entrypoint, "entrypoint")?;
    let entrypoint_abs = version_dir.join("payload").join(&entrypoint_rel);
    if !entrypoint_abs.is_file() {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' entrypoint file not found: payload/{}",
            package_manifest.id, entrypoint_rel
        )));
    }

    if let Some(expected_sha) = package_manifest.entrypoint_sha256.as_deref() {
        let entrypoint_bytes = fs::read(&entrypoint_abs)?;
        let actual_sha = sha256_hex(&entrypoint_bytes);
        if !actual_sha.eq_ignore_ascii_case(expected_sha.trim()) {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' entrypoint_sha256 mismatch",
                package_manifest.id
            )));
        }
    }

    let scopes = normalize_scopes(&package_manifest)?;
    let controls = normalize_controls(&package_manifest)?;
    let mut tool = None;
    let mut model_provider = None;

    if runtime_kind == consts::DEFAULT_TOOL_RUNTIME_KIND {
        if !package_manifest
            .capabilities
            .contains(&PluginCapability::ToolProvider)
        {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' runtime '{}' requires capability '{}'",
                package_manifest.id,
                consts::DEFAULT_TOOL_RUNTIME_KIND,
                "tool_provider"
            )));
        }

        if package_manifest
            .capabilities
            .contains(&PluginCapability::FsWrite)
        {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' declares unsupported capability 'fs_write' for runtime '{}'",
                package_manifest.id,
                consts::DEFAULT_TOOL_RUNTIME_KIND
            )));
        }

        if package_manifest
            .capabilities
            .contains(&PluginCapability::CommandExecution)
        {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' declares unsupported capability 'command_execution' for runtime '{}'",
                package_manifest.id,
                consts::DEFAULT_TOOL_RUNTIME_KIND
            )));
        }

        let tool_name = package_manifest.resolve_tool_name()?;
        let sandbox_policy = sandbox_from_manifest(&package_manifest)?;
        let tool_description = package_manifest.description.clone().unwrap_or_default();
        let tool_input_schema = package_manifest
            .tool_input_schema
            .clone()
            .unwrap_or_else(|| json!({"type": "object"}));
        tool = Some(Arc::new(InstalledWasmTool::new(
            package_manifest.id.clone(),
            package_manifest.version.clone(),
            tool_name,
            tool_description,
            tool_input_schema,
            entrypoint_abs.clone(),
            skill_host,
            sandbox_policy,
            scopes.clone(),
            controls.clone(),
        )));
    } else {
        if !package_manifest
            .capabilities
            .contains(&PluginCapability::ModelProvider)
        {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' runtime '{}' requires capability '{}'",
                package_manifest.id,
                consts::DEFAULT_MODEL_RUNTIME_KIND,
                "model_provider"
            )));
        }
        if package_manifest
            .capabilities
            .contains(&PluginCapability::FsRead)
            || package_manifest
                .capabilities
                .contains(&PluginCapability::FsWrite)
            || package_manifest
                .capabilities
                .contains(&PluginCapability::CommandExecution)
        {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' runtime '{}' only supports model_provider and optional network_egress capabilities",
                package_manifest.id, consts::DEFAULT_MODEL_RUNTIME_KIND
            )));
        }

        let entrypoint_bytes = fs::read(&entrypoint_abs)?;
        let abi_usage = validate_model_plugin_imports(&entrypoint_bytes, &package_manifest.id)?;
        let provider_profile = resolve_model_provider_profile(&package_manifest, abi_usage)?;
        let provider_name =
            package_manifest.resolve_model_provider_name(provider_profile.as_ref())?;
        let model_name = package_manifest.resolved_model_name()?;
        model_provider = Some(Arc::new(InstalledWasmModelProvider::new(
            package_manifest.id.clone(),
            package_manifest.version.clone(),
            provider_name,
            model_name,
            provider_profile,
            entrypoint_abs.clone(),
            model_host,
            scopes.clone(),
            controls.clone(),
        )));
    }

    Ok(LoadedPluginFactoryData {
        manifest: core_manifest,
        tool,
        model_provider,
        runtime: runtime_kind.to_string(),
        publisher: package_manifest.publisher.clone(),
    })
}

/// ### Brief
///
/// collect all plugin directories in the plugin home
///
/// ### Arguments
/// * `plugin_home` - plugin home directory
///
/// ### Returns
/// sorted list of subdirectories
///
/// ### Errors
/// - plugin home directory not readable
fn collect_plugin_dirs(plugin_home: &Path) -> KelvinResult<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    for entry in fs::read_dir(plugin_home)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        dirs.push(entry.path());
    }
    dirs.sort();
    Ok(dirs)
}

/// ### Brief
///
/// resolve the active plugin version directory via symlink or semver selection
///
/// ### Description
///
/// checks for a "current" symlink; if present, resolves its target. otherwise, scans subdirectories
/// for the highest semver version match. returns the active version directory.
///
/// ### Arguments
/// * `plugin_dir` - plugin home directory
///
/// ### Returns
/// absolute path to the active version directory
///
/// ### Errors
/// - symlink target is invalid or doesn't exist
/// - no version directories found
/// - version directory resolution failed
fn resolve_version_dir(plugin_dir: &Path) -> KelvinResult<PathBuf> {
    let current = plugin_dir.join("current");
    if current.is_symlink() {
        let target = fs::read_link(&current)?;
        let target_str = target.to_string_lossy().to_string();
        let normalized = normalize_safe_relative_path(&target_str, "current symlink target")?;
        let resolved = plugin_dir.join(&normalized);
        if resolved.is_dir() {
            return Ok(resolved);
        }
        return Err(KelvinError::InvalidInput(format!(
            "plugin current symlink points to missing directory: {}",
            current.to_string_lossy()
        )));
    }

    let mut best: Option<(semver::Version, PathBuf)> = None;
    for entry in fs::read_dir(plugin_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name();
        let dir_name = dir_name.to_string_lossy();
        if dir_name == "current" {
            continue;
        }
        let Ok(version) = semver::Version::parse(&dir_name) else {
            continue;
        };
        match &best {
            Some((best_version, _)) if version <= *best_version => {}
            _ => best = Some((version, entry.path())),
        }
    }

    best.map(|(_, path)| path).ok_or_else(|| {
        KelvinError::InvalidInput(format!(
            "plugin '{}' has no version directories",
            plugin_dir.to_string_lossy()
        ))
    })
}

/// ### Brief
///
/// normalize and validate a safe relative path (reject traversal and absolute paths)
///
/// ### Arguments
/// * `raw` - raw path string
/// * `field_name` - field name for error messages
///
/// ### Returns
/// normalized relative path
///
/// ### Errors
/// - path is empty or whitespace-only
/// - path is absolute
/// - path contains parent directory (..) or root components
fn normalize_safe_relative_path(raw: &str, field_name: &str) -> KelvinResult<String> {
    let normalized = raw.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err(KelvinError::InvalidInput(format!(
            "{field_name} must not be empty"
        )));
    }
    if Path::new(&normalized).is_absolute() || normalized.starts_with('/') {
        return Err(KelvinError::InvalidInput(format!(
            "{field_name} must be relative path"
        )));
    }
    let path = Path::new(&normalized);
    if path
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
/// normalize and validate capability scopes against declared plugin capabilities
///
/// ### Description
///
/// cross-validates that declared capabilities match their scope declarations. for example, if
/// `fs_read` is declared, `fs_read_paths` must be provided; if `network_egress` or model runtime
/// is declared, `network_allow_hosts` must be provided. normalizes and validates host patterns.
///
/// ### Arguments
/// * `manifest` - plugin manifest with capabilities and scopes
///
/// ### Returns
/// normalized capability scopes
///
/// ### Errors
/// - declared capability missing required scope
/// - invalid scope values (empty paths, bad host patterns)
fn normalize_scopes(manifest: &InstalledPluginPackageManifest) -> KelvinResult<CapabilityScopes> {
    let has_fs_read = manifest.capabilities.contains(&PluginCapability::FsRead);
    let has_network = manifest
        .capabilities
        .contains(&PluginCapability::NetworkEgress);
    let runtime_requires_network_scope =
        manifest.runtime_kind() == consts::DEFAULT_MODEL_RUNTIME_KIND;
    let network_scope_required = has_network || runtime_requires_network_scope;

    let mut fs_read_paths = Vec::new();
    for path in &manifest.capability_scopes.fs_read_paths {
        fs_read_paths.push(normalize_safe_relative_path(
            path,
            "capability_scopes.fs_read_paths",
        )?);
    }
    if has_fs_read && fs_read_paths.is_empty() {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' declares fs_read but has no fs_read scope paths",
            manifest.id
        )));
    }
    if !has_fs_read && !fs_read_paths.is_empty() {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' has fs_read scope paths but does not declare fs_read capability",
            manifest.id
        )));
    }

    let mut network_allow_hosts = Vec::new();
    for host in &manifest.capability_scopes.network_allow_hosts {
        network_allow_hosts.push(normalize_host_pattern(host)?);
    }
    let has_dynamic_base_url = manifest
        .provider_profile
        .as_ref()
        .is_some_and(|p| p.dynamic_base_url);
    if network_scope_required && network_allow_hosts.is_empty() && !has_dynamic_base_url {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' requires network allowlist but has no network allow hosts",
            manifest.id
        )));
    }
    if !has_network && !runtime_requires_network_scope && !network_allow_hosts.is_empty() {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' has network allowlist but does not declare network_egress capability",
            manifest.id
        )));
    }

    Ok(CapabilityScopes {
        fs_read_paths,
        network_allow_hosts,
        env_allow: manifest.capability_scopes.env_allow.clone(),
    })
}

/// ### Brief
///
/// normalize and validate operational control bounds (timeout, retries, rate limits)
///
/// ### Arguments
/// * `manifest` - plugin manifest with operational controls
///
/// ### Returns
/// validated operational controls
///
/// ### Errors
/// - timeout_ms is 0 or exceeds maximum
/// - max_retries exceeds maximum
/// - max_calls_per_minute is 0
/// - circuit breaker values are invalid
fn normalize_controls(
    manifest: &InstalledPluginPackageManifest,
) -> KelvinResult<OperationalControls> {
    let controls = &manifest.operational_controls;
    if controls.timeout_ms == 0 || controls.timeout_ms > consts::OPERATIONAL_MAX_TIMEOUT {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' timeout_ms must be between 1 and {}",
            manifest.id,
            consts::OPERATIONAL_MAX_TIMEOUT
        )));
    }
    if controls.max_retries > consts::OPERATIONAL_MAX_RETRIES {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' max_retries must be <= {}",
            manifest.id,
            consts::OPERATIONAL_MAX_RETRIES
        )));
    }
    if controls.max_calls_per_minute == 0
        || controls.max_calls_per_minute > consts::OPERATIONAL_MAX_CALLS_PER_MINUTE
    {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' max_calls_per_minute must be between 1 and {}",
            manifest.id,
            consts::OPERATIONAL_MAX_CALLS_PER_MINUTE
        )));
    }
    if controls.circuit_breaker_failures == 0
        || controls.circuit_breaker_failures > consts::OPERATIONAL_MAX_CIRCUIT_BREAKER_FAILURES
    {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' circuit_breaker_failures must be between 1 and {}",
            manifest.id,
            consts::OPERATIONAL_MAX_CIRCUIT_BREAKER_FAILURES
        )));
    }
    if controls.circuit_breaker_cooldown_ms < 100
        || controls.circuit_breaker_cooldown_ms > consts::OPERATIONAL_MAX_CIRCUIT_BREAKER_COOLDOWN
    {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' circuit_breaker_cooldown_ms must be between 100 and {}",
            manifest.id,
            consts::OPERATIONAL_MAX_CIRCUIT_BREAKER_COOLDOWN
        )));
    }

    Ok(OperationalControls {
        timeout_ms: controls.timeout_ms,
        max_retries: controls.max_retries,
        max_calls_per_minute: controls.max_calls_per_minute,
        circuit_breaker_failures: controls.circuit_breaker_failures,
        circuit_breaker_cooldown_ms: controls.circuit_breaker_cooldown_ms,
    })
}

/// ### Brief
///
/// build a sandbox policy from the manifest capabilities and scopes
///
/// ### Description
///
/// initializes a locked-down sandbox policy and selectively enables capabilities (fs_read, network_egress,
/// env_access, command_execution) based on what the manifest declares. validates that env_access and
/// command_execution are only enabled when fs_write is also enabled.
///
/// ### Arguments
/// * `manifest` - plugin manifest with capabilities
///
/// ### Returns
/// sandbox policy with capabilities enabled as declared
///
/// ### Errors
/// - env_access or command_execution enabled without fs_write
fn sandbox_from_manifest(manifest: &InstalledPluginPackageManifest) -> KelvinResult<SandboxPolicy> {
    let mut policy = SandboxPolicy::locked_down();
    if manifest.capabilities.contains(&PluginCapability::FsRead) {
        policy.allow_fs_read = true;
    }
    if manifest
        .capabilities
        .contains(&PluginCapability::NetworkEgress)
    {
        policy.network_allow_hosts = manifest.capability_scopes.network_allow_hosts.clone();
    }
    if !manifest.capability_scopes.env_allow.is_empty() {
        // Require EnvAccess capability for env_allow scopes (#67)
        if !manifest.capabilities.contains(&PluginCapability::EnvAccess) {
            return Err(KelvinError::InvalidInput(format!(
                "plugin '{}' declares env_allow scopes but lacks 'env_access' capability",
                manifest.id
            )));
        }
        policy.env_allow = manifest.capability_scopes.env_allow.clone();
    }
    if let Some(budget) = manifest.operational_controls.fuel_budget {
        // Clamp fuel_budget to the hard upper bound (#69)
        policy.fuel_budget = budget.min(kelvin_wasm::MAX_FUEL_BUDGET);
    }
    if manifest.capabilities.contains(&PluginCapability::FsWrite)
        || manifest
            .capabilities
            .contains(&PluginCapability::CommandExecution)
    {
        return Err(KelvinError::InvalidInput(format!(
            "plugin '{}' requests unsupported runtime capability",
            manifest.id
        )));
    }
    Ok(policy)
}

/// ### Brief
///
/// parse and validate WASM model plugin imports against the allowed ABI
///
/// ### Description
///
/// inspects the WASM binary's import section to detect which ABI modules the plugin uses
/// (e.g., openai, provider_profile). validates that only allowed module/function combinations
/// are imported. tracks which ABI imports are used for later profile compatibility checks.
///
/// ### Arguments
/// * `wasm_bytes` - WASM binary bytes
/// * `plugin_id` - plugin id for error messages
///
/// ### Returns
/// record of which ABI modules are imported
///
/// ### Errors
/// - invalid WASM binary format
/// - disallowed ABI import
fn validate_model_plugin_imports(
    wasm_bytes: &[u8],
    plugin_id: &str,
) -> KelvinResult<ModelPluginAbiUsage> {
    let mut usage = ModelPluginAbiUsage::default();
    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload
            .map_err(|err| KelvinError::InvalidInput(format!("invalid model wasm: {err}")))?;
        if let Payload::ImportSection(section) = payload {
            for import in section {
                let import = import.map_err(|err| {
                    KelvinError::InvalidInput(format!("invalid model wasm import section: {err}"))
                })?;
                if import.module != model_abi::MODULE {
                    return Err(KelvinError::InvalidInput(format!(
                        "model plugin '{}' has forbidden import module '{}'",
                        plugin_id, import.module
                    )));
                }
                match import.name {
                    model_abi::IMPORT_OPENAI_RESPONSES_CALL => {
                        usage.uses_openai_import = true;
                    }
                    model_abi::IMPORT_PROVIDER_PROFILE_CALL => {
                        usage.uses_provider_profile_import = true;
                    }
                    model_abi::IMPORT_LOG | model_abi::IMPORT_CLOCK_NOW_MS => {}
                    name => {
                        return Err(KelvinError::InvalidInput(format!(
                            "model plugin '{}' has forbidden import '{}.{}'",
                            plugin_id, import.module, name
                        )));
                    }
                }
            }
        }
    }
    if usage.uses_openai_import && usage.uses_provider_profile_import {
        return Err(KelvinError::InvalidInput(format!(
            "model plugin '{}' mixes legacy and provider-profile imports",
            plugin_id
        )));
    }
    Ok(usage)
}

/// ### Brief
///
/// compute sha-256 hash and return as lowercase hex string
///
/// ### Arguments
/// * `bytes` - data to hash
///
/// ### Returns
/// hex-encoded sha-256 digest
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// ### Brief
///
/// parse a base64-encoded ed25519 public key
///
/// ### Arguments
/// * `key_base64` - base64-encoded ed25519 public key
///
/// ### Returns
/// parsed ed25519 verifying key
///
/// ### Errors
/// - invalid base64 encoding
/// - decoded key is not 32 bytes
/// - invalid ed25519 key format
fn parse_public_key(key_base64: &str) -> KelvinResult<VerifyingKey> {
    let bytes = STANDARD
        .decode(key_base64.trim())
        .map_err(|err| KelvinError::InvalidInput(format!("invalid base64 public key: {err}")))?;
    if bytes.len() != 32 {
        return Err(KelvinError::InvalidInput(
            "ed25519 public key must be 32 bytes".to_string(),
        ));
    }
    let mut key = [0_u8; 32];
    key.copy_from_slice(&bytes);
    VerifyingKey::from_bytes(&key)
        .map_err(|err| KelvinError::InvalidInput(format!("invalid ed25519 public key: {err}")))
}

/// ### Brief
///
/// normalize and validate a network host pattern for the allowlist
///
/// ### Description
///
/// lowercases and trims the input. validates that the pattern contains only alphanumeric
/// characters, dots, hyphens, asterisks (for wildcards), and colons (for ports).
/// rejects empty strings and control/whitespace characters.
///
/// ### Arguments
/// * `input` - host pattern (e.g., "*.example.com", "api.example.com:8080")
///
/// ### Returns
/// normalized lowercase host pattern
///
/// ### Errors
/// - pattern is empty or whitespace-only
/// - pattern contains control or whitespace characters
/// - pattern contains invalid characters
fn normalize_host_pattern(input: &str) -> KelvinResult<String> {
    let cleaned = input.trim().to_ascii_lowercase();
    if cleaned.is_empty() {
        return Err(KelvinError::InvalidInput(
            "network allowlist host must not be empty".to_string(),
        ));
    }
    if cleaned
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(KelvinError::InvalidInput(
            "network allowlist host must not contain whitespace/control characters".to_string(),
        ));
    }
    if !cleaned
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '*' | ':'))
    {
        return Err(KelvinError::InvalidInput(format!(
            "invalid network allowlist host pattern: {cleaned}"
        )));
    }
    Ok(cleaned)
}

/// ### Brief
///
/// get an environment variable as a trimmed path, returning None if unset or empty
///
/// ### Arguments
/// * `key` - environment variable name
///
/// ### Returns
/// path from environment variable if set and non-empty
fn env_path(key: &str) -> Option<PathBuf> {
    let value = env::var(key).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

/// ### Brief
///
/// resolve the user's home directory from HOME or USERPROFILE environment variables
///
/// ### Returns
/// home directory path
///
/// ### Errors
/// - HOME and USERPROFILE are both unset or empty
fn resolve_home_dir() -> KelvinResult<PathBuf> {
    if let Some(path) = env_path("HOME") {
        return Ok(path);
    }
    if let Some(path) = env_path("USERPROFILE") {
        return Ok(path);
    }
    Err(KelvinError::InvalidInput(
        "HOME is not set; configure KELVIN_PLUGIN_HOME and KELVIN_TRUST_POLICY_PATH explicitly"
            .to_string(),
    ))
}

/// ### Brief
///
/// check if a trust policy file exists; error if env var explicitly points to a missing file
///
/// ### Description
///
/// returns the path if it exists. if the path doesn't exist but KELVIN_TRUST_POLICY_PATH
/// environment variable is set to this exact path, returns an error (to avoid silently
/// falling back when the user explicitly configured the path). otherwise returns None.
///
/// ### Arguments
/// * `path` - path to check
///
/// ### Returns
/// the path if it exists, or None if it doesn't exist and wasn't explicitly configured
///
/// ### Errors
/// - KELVIN_TRUST_POLICY_PATH is set to this path but the file doesn't exist
fn maybe_load_trust_policy_path(path: &Path) -> KelvinResult<Option<&Path>> {
    if path.exists() {
        return Ok(Some(path));
    }

    // If KELVIN_TRUST_POLICY_PATH is explicitly set to this path but the file
    // does not exist, treat it as a configuration error instead of silently
    // falling back to the default permissive policy.
    if let Ok(env_value) = env::var("KELVIN_TRUST_POLICY_PATH") {
        let trimmed = env_value.trim();
        if !trimmed.is_empty() && Path::new(trimmed) == path {
            return Err(KelvinError::InvalidInput(format!(
                "KELVIN_TRUST_POLICY_PATH is set to '{}' but the file does not exist",
                trimmed
            )));
        }
    }

    Ok(None)
}

/// ### Brief
///
/// check if a host matches any pattern in the allowlist (supports wildcard patterns)
///
/// ### Arguments
/// * `target` - host to check
/// * `allowlist` - list of allowed host patterns (e.g., "*.example.com", "api.example.com")
///
/// ### Returns
/// true if the target matches any pattern in the allowlist
#[allow(dead_code)]
fn host_allowed(target: &str, allowlist: &[String]) -> bool {
    let candidate = target.trim().to_ascii_lowercase();
    allowlist.iter().any(|pattern| {
        if let Some(rest) = pattern.strip_prefix("*.") {
            candidate.ends_with(rest)
                && candidate.len() > rest.len()
                && candidate.as_bytes()[candidate.len() - rest.len() - 1] == b'.'
        } else {
            candidate == *pattern
        }
    })
}

/// ### Brief
///
/// check if a path matches a scope allowlist (exact match or prefix with /)
///
/// ### Arguments
/// * `target` - path to check
/// * `allowlist` - list of allowed scopes
///
/// ### Returns
/// true if target matches a scope exactly or is a path under a scope
fn scope_match(target: &str, allowlist: &[String]) -> bool {
    allowlist.iter().any(|scope| {
        target == scope
            || target
                .strip_prefix(scope)
                .map(|rest| rest.starts_with('/'))
                .unwrap_or(false)
    })
}

/// ### Brief
///
/// serialize a claw system call to JSON
///
/// ### Arguments
/// * `call` - claw call enum variant
///
/// ### Returns
/// JSON object with "kind" field and call-specific fields
fn claw_call_json(call: &ClawCall) -> serde_json::Value {
    match call {
        ClawCall::SendMessage { message_code } => json!({
            "kind": "send_message",
            "message_code": message_code,
        }),
        ClawCall::MoveServo { channel, position } => json!({
            "kind": "move_servo",
            "channel": channel,
            "position": position,
        }),
        ClawCall::FsRead { handle } => json!({
            "kind": "fs_read",
            "handle": handle,
        }),
        ClawCall::NetworkSend { packet } => json!({
            "kind": "network_send",
            "packet": packet,
        }),
        ClawCall::HttpCall { url } => json!({
            "kind": "http_call",
            "url": url,
        }),
        ClawCall::EnvAccess { key } => json!({
            "kind": "env_access",
            "key": key,
        }),
    }
}

#[cfg(test)]
mod tests {
    use crate::consts;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use base64::Engine as _;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::json;

    use super::{
        adapt_anthropic_response, adapt_openai_response, adapt_openrouter_response,
        load_installed_plugins, load_installed_tool_plugins, sha256_hex,
        InstalledPluginLoaderConfig, PublisherTrustPolicy,
    };
    use kelvin_core::{
        ModelOutput, ModelProviderAuthScheme, ModelProviderProfile, ModelProviderProtocolFamily,
        PluginSecurityPolicy, ToolCallInput, ToolRegistry, OPENAI_RESPONSES_PROFILE_ID,
    };

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!("kelvin-installed-{name}-{millis}"));
        std::fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    fn write_installed_plugin(
        plugin_home: &Path,
        plugin_id: &str,
        version: &str,
        manifest_value: serde_json::Value,
        wat_source: &str,
        signing_key: Option<&SigningKey>,
    ) {
        let version_dir = plugin_home.join(plugin_id).join(version);
        let payload_dir = version_dir.join("payload");
        std::fs::create_dir_all(&payload_dir).expect("create payload dir");

        let entrypoint_rel = manifest_value["entrypoint"]
            .as_str()
            .expect("entrypoint string");
        let wasm_bytes = wat::parse_str(wat_source).expect("compile wat");
        let entrypoint_abs = payload_dir.join(entrypoint_rel);
        std::fs::write(&entrypoint_abs, &wasm_bytes).expect("write wasm entrypoint");

        let mut manifest = manifest_value;
        if manifest["entrypoint_sha256"].is_null() {
            manifest["entrypoint_sha256"] = json!(sha256_hex(&wasm_bytes));
        }

        let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest bytes");
        std::fs::write(version_dir.join("plugin.json"), &manifest_bytes).expect("write manifest");

        if let Some(key) = signing_key {
            let signature = key.sign(&manifest_bytes);
            let signature_base64 =
                base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());
            std::fs::write(
                version_dir.join(consts::PLUGIN_SIGNATURE_FILENAME),
                signature_base64,
            )
            .expect("write signature");
        }
    }

    fn default_manifest(plugin_id: &str, version: &str) -> serde_json::Value {
        json!({
            "id": plugin_id,
            "name": "Installed Plugin",
            "version": version,
            "api_version": "1.0.0",
            "description": "installed runtime plugin",
            "homepage": "https://example.com/plugin",
            "capabilities": ["tool_provider"],
            "experimental": false,
            "runtime": "wasm_tool_v1",
            "tool_name": "installed_echo",
            "entrypoint": "echo.wasm",
            "entrypoint_sha256": null,
            "publisher": "acme",
            "capability_scopes": {
                "fs_read_paths": [],
                "network_allow_hosts": []
            },
            "operational_controls": {
                "timeout_ms": 2000,
                "max_retries": 0,
                "max_calls_per_minute": 100,
                "circuit_breaker_failures": 2,
                "circuit_breaker_cooldown_ms": 1000
            }
        })
    }

    fn default_model_manifest(plugin_id: &str, version: &str) -> serde_json::Value {
        let profile = ModelProviderProfile::builtin(OPENAI_RESPONSES_PROFILE_ID)
            .expect("openai builtin profile");
        json!({
            "id": plugin_id,
            "name": "Installed Model Plugin",
            "version": version,
            "api_version": "1.0.0",
            "description": "installed runtime model plugin",
            "homepage": "https://example.com/plugin",
            "capabilities": ["model_provider"],
            "experimental": false,
            "runtime": "wasm_model_v1",
            "provider_name": "openai",
            "provider_profile": profile,
            "model_name": "gpt-4.1-mini",
            "entrypoint": "model.wasm",
            "entrypoint_sha256": null,
            "publisher": "acme",
            "capability_scopes": {
                "fs_read_paths": [],
                "network_allow_hosts": ["api.openai.com"]
            },
            "operational_controls": {
                "timeout_ms": 2000,
                "max_retries": 0,
                "max_calls_per_minute": 100,
                "circuit_breaker_failures": 2,
                "circuit_breaker_cooldown_ms": 1000
            }
        })
    }

    #[test]
    fn adapts_anthropic_provider_response_into_model_output() {
        let output = adapt_anthropic_response(&json!({
            "type": "message",
            "content": [
                {"type": "text", "text": "KelvinClaw is a plugin-driven runtime."}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 11,
                "output_tokens": 9
            }
        }))
        .expect("anthropic provider response should adapt");

        assert_eq!(
            output,
            ModelOutput {
                assistant_text: "KelvinClaw is a plugin-driven runtime.".to_string(),
                stop_reason: Some("end_turn".to_string()),
                tool_calls: Vec::new(),
                usage: Some(kelvin_core::ModelUsage {
                    input_tokens: Some(11),
                    output_tokens: Some(9),
                    total_tokens: Some(20),
                }),
            }
        );
    }

    #[test]
    fn adapts_openai_provider_response_into_model_output() {
        let output = adapt_openai_response(&json!({
            "output_text": "KelvinClaw is a plugin-driven runtime.",
            "status": "completed",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 8,
                "total_tokens": 18
            }
        }))
        .expect("openai provider response should adapt");

        assert_eq!(
            output,
            ModelOutput {
                assistant_text: "KelvinClaw is a plugin-driven runtime.".to_string(),
                stop_reason: Some("completed".to_string()),
                tool_calls: Vec::new(),
                usage: Some(kelvin_core::ModelUsage {
                    input_tokens: Some(10),
                    output_tokens: Some(8),
                    total_tokens: Some(18),
                }),
            }
        );
    }

    #[test]
    fn adapts_openrouter_provider_response_into_model_output() {
        let output = adapt_openrouter_response(&json!({
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": "KelvinClaw can route model calls through OpenRouter."
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 9,
                "completion_tokens": 8,
                "total_tokens": 17
            }
        }))
        .expect("openrouter provider response should adapt");

        assert_eq!(
            output,
            ModelOutput {
                assistant_text: "KelvinClaw can route model calls through OpenRouter.".to_string(),
                stop_reason: Some("stop".to_string()),
                tool_calls: Vec::new(),
                usage: Some(kelvin_core::ModelUsage {
                    input_tokens: Some(9),
                    output_tokens: Some(8),
                    total_tokens: Some(17),
                }),
            }
        );
    }

    fn openrouter_profile() -> ModelProviderProfile {
        ModelProviderProfile {
            id: "openrouter.chat".to_string(),
            provider_name: "openrouter".to_string(),
            protocol_family: ModelProviderProtocolFamily::OpenAiChatCompletions,
            api_key_env: Some("OPENROUTER_API_KEY".to_string()),
            base_url_env: "OPENROUTER_BASE_URL".to_string(),
            default_base_url: "https://openrouter.ai/api/v1".to_string(),
            endpoint_path: "chat/completions".to_string(),
            auth_header: "authorization".to_string(),
            auth_scheme: ModelProviderAuthScheme::Bearer,
            static_headers: Vec::new(),
            default_allow_hosts: vec!["openrouter.ai".to_string()],
            dynamic_base_url: false,
        }
    }

    #[tokio::test]
    async fn loads_signed_plugin_and_executes_tool() {
        let plugin_home = unique_temp_dir("load-exec");
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let public_key = base64::engine::general_purpose::STANDARD
            .encode(signing_key.verifying_key().to_bytes());

        write_installed_plugin(
            &plugin_home,
            "acme.echo",
            "1.0.0",
            default_manifest("acme.echo", "1.0.0"),
            r#"
            (module
              (import "claw" "send_message" (func $send_message (param i32) (result i32)))
              (func (export "run") (result i32)
                i32.const 55
                call $send_message
                drop
                i32.const 0
              )
            )
            "#,
            Some(&signing_key),
        );

        let trust_policy = PublisherTrustPolicy::default()
            .with_publisher_key("acme", &public_key)
            .expect("publisher key");
        let loaded = load_installed_tool_plugins(InstalledPluginLoaderConfig {
            plugin_home: plugin_home.clone(),
            core_version: "0.1.0".to_string(),
            security_policy: PluginSecurityPolicy::default(),
            trust_policy,
        })
        .expect("load installed plugin");

        assert_eq!(loaded.loaded_plugins.len(), 1);
        let tool = loaded
            .tool_registry
            .get("installed_echo")
            .expect("tool should be registered");
        let result = tool
            .call(ToolCallInput {
                run_id: "run-1".to_string(),
                session_id: "session-1".to_string(),
                workspace_dir: plugin_home.to_string_lossy().to_string(),
                arguments: json!({}),
            })
            .await
            .expect("tool call");
        assert!(!result.is_error);
        assert!(result.summary.contains("acme.echo@1.0.0"));
    }

    #[test]
    fn rejects_missing_signature_when_required() {
        let plugin_home = unique_temp_dir("missing-signature");
        let signing_key = SigningKey::from_bytes(&[8_u8; 32]);
        let public_key = base64::engine::general_purpose::STANDARD
            .encode(signing_key.verifying_key().to_bytes());
        write_installed_plugin(
            &plugin_home,
            "acme.echo",
            "1.0.0",
            default_manifest("acme.echo", "1.0.0"),
            r#"
            (module
              (func (export "run") (result i32)
                i32.const 0
              )
            )
            "#,
            None,
        );

        let trust_policy = PublisherTrustPolicy::default()
            .with_publisher_key("acme", &public_key)
            .expect("publisher key");
        let err = match load_installed_tool_plugins(InstalledPluginLoaderConfig {
            plugin_home: plugin_home.clone(),
            core_version: "0.1.0".to_string(),
            security_policy: PluginSecurityPolicy::default(),
            trust_policy,
        }) {
            Ok(_) => panic!("signature should be required"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("missing required plugin.sig"));
    }

    #[test]
    fn unsigned_local_model_plugin_without_signature_still_loads() {
        let plugin_home = unique_temp_dir("unsigned-local-model");
        let mut manifest = default_model_manifest("acme.local-openrouter", "1.0.0");
        manifest["provider_name"] = json!("openrouter");
        manifest["provider_profile"] =
            serde_json::to_value(openrouter_profile()).expect("serialize openrouter profile");
        manifest["model_name"] = json!("openai/gpt-4.1-mini");
        manifest["publisher"] = serde_json::Value::Null;
        manifest["quality_tier"] = json!(consts::QUALITY_TIER_UNSIGNED_LOCAL);
        manifest["capability_scopes"]["network_allow_hosts"] = json!(["openrouter.ai"]);

        write_installed_plugin(
            &plugin_home,
            "acme.local-openrouter",
            "1.0.0",
            manifest,
            r#"
            (module
              (import "kelvin_model_host_v1" "provider_profile_call" (func $provider_profile_call (param i32 i32) (result i64)))
              (import "kelvin_model_host_v1" "log" (func $log (param i32 i32 i32) (result i32)))
              (import "kelvin_model_host_v1" "clock_now_ms" (func $clock_now_ms (result i64)))
              (memory (export "memory") 2)
              (global $heap (mut i32) (i32.const 1024))
              (func (export "alloc") (param $len i32) (result i32)
                (local $ptr i32)
                global.get $heap
                local.tee $ptr
                local.get $len
                i32.add
                global.set $heap
                local.get $ptr)
              (func (export "dealloc") (param i32 i32))
              (func (export "infer") (param $ptr i32) (param $len i32) (result i64)
                local.get $ptr
                local.get $len
                call $provider_profile_call)
            )
            "#,
            None,
        );

        let loaded = load_installed_plugins(InstalledPluginLoaderConfig {
            plugin_home,
            core_version: "0.1.0".to_string(),
            security_policy: PluginSecurityPolicy::default(),
            trust_policy: PublisherTrustPolicy::default(),
        })
        .expect("unsigned_local plugin should load without signature");

        assert_eq!(
            loaded.loaded_plugins[0].provider_profile.as_deref(),
            Some("openrouter.chat")
        );
    }

    #[tokio::test]
    async fn enforces_scopes_and_operational_controls() {
        let plugin_home = unique_temp_dir("scopes-controls");
        let signing_key = SigningKey::from_bytes(&[9_u8; 32]);
        let public_key = base64::engine::general_purpose::STANDARD
            .encode(signing_key.verifying_key().to_bytes());

        let mut manifest = default_manifest("acme.scoped", "1.0.0");
        manifest["capabilities"] = json!(["tool_provider", "fs_read", "network_egress"]);
        manifest["capability_scopes"] = json!({
            "fs_read_paths": ["memory/allowed"],
            "network_allow_hosts": ["api.example.com"]
        });
        manifest["operational_controls"] = json!({
            "timeout_ms": 2000,
            "max_retries": 0,
            "max_calls_per_minute": 1,
            "circuit_breaker_failures": 1,
            "circuit_breaker_cooldown_ms": 5000
        });

        write_installed_plugin(
            &plugin_home,
            "acme.scoped",
            "1.0.0",
            manifest,
            r#"
            (module
              (import "claw" "fs_read" (func $fs_read (param i32) (result i32)))
              (import "claw" "network_send" (func $network_send (param i32) (result i32)))
              (func (export "run") (result i32)
                i32.const 1
                call $fs_read
                drop
                i32.const 2
                call $network_send
                drop
                i32.const 0
              )
            )
            "#,
            Some(&signing_key),
        );

        let trust_policy = PublisherTrustPolicy::default()
            .with_publisher_key("acme", &public_key)
            .expect("publisher key");
        let loaded = load_installed_tool_plugins(InstalledPluginLoaderConfig {
            plugin_home: plugin_home.clone(),
            core_version: "0.1.0".to_string(),
            security_policy: PluginSecurityPolicy {
                allow_fs_read: true,
                allow_network_egress: true,
                ..Default::default()
            },
            trust_policy,
        })
        .expect("load installed plugin");

        let tool = loaded
            .tool_registry
            .get("installed_echo")
            .expect("tool should be registered");

        let scope_err = tool
            .call(ToolCallInput {
                run_id: "run-scope".to_string(),
                session_id: "session-scope".to_string(),
                workspace_dir: plugin_home.to_string_lossy().to_string(),
                arguments: json!({
                    "target_path": "memory/blocked/file.md",
                    "target_host": "api.example.com"
                }),
            })
            .await
            .expect_err("scope should deny path");
        assert!(scope_err
            .to_string()
            .contains("outside allowed fs_read scopes"));

        let ok = tool
            .call(ToolCallInput {
                run_id: "run-ok".to_string(),
                session_id: "session-ok".to_string(),
                workspace_dir: plugin_home.to_string_lossy().to_string(),
                arguments: json!({
                    "target_path": "memory/allowed/file.md",
                    "target_host": "api.example.com"
                }),
            })
            .await
            .expect("first allowed call");
        assert!(!ok.is_error);

        let rate_err = tool
            .call(ToolCallInput {
                run_id: "run-rate".to_string(),
                session_id: "session-rate".to_string(),
                workspace_dir: plugin_home.to_string_lossy().to_string(),
                arguments: json!({
                    "target_path": "memory/allowed/file.md",
                    "target_host": "api.example.com"
                }),
            })
            .await
            .expect_err("rate limit should apply");
        assert!(rate_err.to_string().contains("exceeded call budget"));
    }

    #[test]
    fn loads_signed_model_plugin_and_projects_model_registry() {
        let plugin_home = unique_temp_dir("load-model");
        let signing_key = SigningKey::from_bytes(&[10_u8; 32]);
        let public_key = base64::engine::general_purpose::STANDARD
            .encode(signing_key.verifying_key().to_bytes());

        write_installed_plugin(
            &plugin_home,
            "acme.openai",
            "1.0.0",
            default_model_manifest("acme.openai", "1.0.0"),
            r#"
            (module
              (import "kelvin_model_host_v1" "provider_profile_call" (func $provider_profile_call (param i32 i32) (result i64)))
              (import "kelvin_model_host_v1" "log" (func $log (param i32 i32 i32) (result i32)))
              (import "kelvin_model_host_v1" "clock_now_ms" (func $clock_now_ms (result i64)))
              (memory (export "memory") 2)
              (global $heap (mut i32) (i32.const 1024))
              (func (export "alloc") (param $len i32) (result i32)
                (local $ptr i32)
                global.get $heap
                local.tee $ptr
                local.get $len
                i32.add
                global.set $heap
                local.get $ptr)
              (func (export "dealloc") (param i32 i32))
              (func (export "infer") (param $ptr i32) (param $len i32) (result i64)
                local.get $ptr
                local.get $len
                call $provider_profile_call)
            )
            "#,
            Some(&signing_key),
        );

        let trust_policy = PublisherTrustPolicy::default()
            .with_publisher_key("acme", &public_key)
            .expect("publisher key");
        let loaded = load_installed_plugins(InstalledPluginLoaderConfig {
            plugin_home,
            core_version: "0.1.0".to_string(),
            security_policy: PluginSecurityPolicy::default(),
            trust_policy,
        })
        .expect("load installed model plugin");

        assert_eq!(loaded.loaded_plugins.len(), 1);
        assert_eq!(
            loaded.loaded_plugins[0].provider_name.as_deref(),
            Some("openai")
        );
        assert_eq!(
            loaded.loaded_plugins[0].model_name.as_deref(),
            Some("gpt-4.1-mini")
        );
        assert_eq!(
            loaded.loaded_plugins[0].provider_profile.as_deref(),
            Some(OPENAI_RESPONSES_PROFILE_ID)
        );
        let provider = loaded
            .model_registry
            .get_by_plugin_id("acme.openai")
            .expect("model registry entry");
        assert_eq!(provider.provider_name(), "openai");
        assert_eq!(provider.model_name(), "gpt-4.1-mini");
    }

    #[test]
    fn rejects_model_plugin_without_provider_profile() {
        let plugin_home = unique_temp_dir("missing-provider-profile");
        let signing_key = SigningKey::from_bytes(&[13_u8; 32]);
        let public_key = base64::engine::general_purpose::STANDARD
            .encode(signing_key.verifying_key().to_bytes());

        let mut manifest = default_model_manifest("acme.legacy-openai", "1.0.0");
        manifest
            .as_object_mut()
            .expect("manifest object")
            .remove("provider_profile");

        write_installed_plugin(
            &plugin_home,
            "acme.legacy-openai",
            "1.0.0",
            manifest,
            r#"
            (module
              (import "kelvin_model_host_v1" "openai_responses_call" (func $openai_responses_call (param i32 i32) (result i64)))
              (import "kelvin_model_host_v1" "log" (func $log (param i32 i32 i32) (result i32)))
              (import "kelvin_model_host_v1" "clock_now_ms" (func $clock_now_ms (result i64)))
              (memory (export "memory") 2)
              (global $heap (mut i32) (i32.const 1024))
              (func (export "alloc") (param $len i32) (result i32)
                (local $ptr i32)
                global.get $heap
                local.tee $ptr
                local.get $len
                i32.add
                global.set $heap
                local.get $ptr)
              (func (export "dealloc") (param i32 i32))
              (func (export "infer") (param $ptr i32) (param $len i32) (result i64)
                local.get $ptr
                local.get $len
                call $openai_responses_call)
            )
            "#,
            Some(&signing_key),
        );

        let trust_policy = PublisherTrustPolicy::default()
            .with_publisher_key("acme", &public_key)
            .expect("publisher key");
        let err = match load_installed_plugins(InstalledPluginLoaderConfig {
            plugin_home,
            core_version: "0.1.0".to_string(),
            security_policy: PluginSecurityPolicy::default(),
            trust_policy,
        }) {
            Ok(_) => panic!("missing provider_profile should fail"),
            Err(err) => err,
        };
        assert!(err
            .to_string()
            .contains("requires a structured provider_profile object"));
    }

    #[test]
    fn loads_openrouter_model_plugin_with_structured_provider_profile() {
        let plugin_home = unique_temp_dir("load-openrouter-model");
        let signing_key = SigningKey::from_bytes(&[15_u8; 32]);
        let public_key = base64::engine::general_purpose::STANDARD
            .encode(signing_key.verifying_key().to_bytes());

        let mut manifest = default_model_manifest("acme.openrouter", "1.0.0");
        manifest["provider_name"] = json!("openrouter");
        manifest["provider_profile"] =
            serde_json::to_value(openrouter_profile()).expect("serialize openrouter profile");
        manifest["model_name"] = json!("openai/gpt-4.1-mini");
        manifest["capability_scopes"]["network_allow_hosts"] = json!(["openrouter.ai"]);

        write_installed_plugin(
            &plugin_home,
            "acme.openrouter",
            "1.0.0",
            manifest,
            r#"
            (module
              (import "kelvin_model_host_v1" "provider_profile_call" (func $provider_profile_call (param i32 i32) (result i64)))
              (import "kelvin_model_host_v1" "log" (func $log (param i32 i32 i32) (result i32)))
              (import "kelvin_model_host_v1" "clock_now_ms" (func $clock_now_ms (result i64)))
              (memory (export "memory") 2)
              (global $heap (mut i32) (i32.const 1024))
              (func (export "alloc") (param $len i32) (result i32)
                (local $ptr i32)
                global.get $heap
                local.tee $ptr
                local.get $len
                i32.add
                global.set $heap
                local.get $ptr)
              (func (export "dealloc") (param i32 i32))
              (func (export "infer") (param $ptr i32) (param $len i32) (result i64)
                local.get $ptr
                local.get $len
                call $provider_profile_call)
            )
            "#,
            Some(&signing_key),
        );

        let trust_policy = PublisherTrustPolicy::default()
            .with_publisher_key("acme", &public_key)
            .expect("publisher key");
        let loaded = load_installed_plugins(InstalledPluginLoaderConfig {
            plugin_home,
            core_version: "0.1.0".to_string(),
            security_policy: PluginSecurityPolicy::default(),
            trust_policy,
        })
        .expect("load openrouter model plugin");

        assert_eq!(
            loaded.loaded_plugins[0].provider_profile.as_deref(),
            Some("openrouter.chat")
        );
    }

    #[test]
    fn rejects_revoked_publisher_even_with_valid_signature() {
        let plugin_home = unique_temp_dir("revoked-publisher");
        let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
        let public_key = base64::engine::general_purpose::STANDARD
            .encode(signing_key.verifying_key().to_bytes());

        write_installed_plugin(
            &plugin_home,
            "acme.echo",
            "1.0.0",
            default_manifest("acme.echo", "1.0.0"),
            r#"
            (module
              (func (export "run") (result i32)
                i32.const 0
              )
            )
            "#,
            Some(&signing_key),
        );

        let trust_policy = PublisherTrustPolicy::default()
            .with_publisher_key("acme", &public_key)
            .expect("publisher key")
            .with_revoked_publisher("acme");
        let err = match load_installed_tool_plugins(InstalledPluginLoaderConfig {
            plugin_home,
            core_version: "0.1.0".to_string(),
            security_policy: PluginSecurityPolicy::default(),
            trust_policy,
        }) {
            Ok(_) => panic!("revoked publisher should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("is revoked"));
    }

    #[test]
    fn rejects_publisher_that_does_not_match_pin_policy() {
        let plugin_home = unique_temp_dir("pinned-publisher");
        let signing_key = SigningKey::from_bytes(&[12_u8; 32]);
        let public_key = base64::engine::general_purpose::STANDARD
            .encode(signing_key.verifying_key().to_bytes());

        write_installed_plugin(
            &plugin_home,
            "acme.echo",
            "1.0.0",
            default_manifest("acme.echo", "1.0.0"),
            r#"
            (module
              (func (export "run") (result i32)
                i32.const 0
              )
            )
            "#,
            Some(&signing_key),
        );

        let trust_policy = PublisherTrustPolicy::default()
            .with_publisher_key("acme", &public_key)
            .expect("publisher key")
            .with_pinned_plugin_publisher("acme.echo", "kelvin");
        let err = match load_installed_tool_plugins(InstalledPluginLoaderConfig {
            plugin_home,
            core_version: "0.1.0".to_string(),
            security_policy: PluginSecurityPolicy::default(),
            trust_policy,
        }) {
            Ok(_) => panic!("pinned publisher mismatch should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("does not match pinned publisher"));
    }
}

#[cfg(test)]
mod schema_validation_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_valid_model_output_schema() {
        let valid_output = json!({
            "assistant_text": "Hello, world!",
            "tool_calls": [],
            "stop_reason": "completed",
            "usage": null
        });
        assert!(validate_model_output_schema(&valid_output).is_ok());
    }

    #[test]
    fn validates_model_output_with_tool_calls() {
        let output_with_tools = json!({
            "assistant_text": "I'll help you schedule a task.",
            "tool_calls": [
                {
                    "id": "call-1",
                    "name": "schedule_cron",
                    "arguments": {
                        "cron": "0 9 * * *",
                        "prompt": "daily reminder"
                    }
                }
            ],
            "stop_reason": "tool_calls",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50
            }
        });
        assert!(validate_model_output_schema(&output_with_tools).is_ok());
    }

    #[test]
    fn rejects_missing_assistant_text() {
        let invalid = json!({
            "tool_calls": [],
            "stop_reason": "completed"
        });
        let result = validate_model_output_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("assistant_text"));
    }

    #[test]
    fn rejects_non_string_assistant_text() {
        let invalid = json!({
            "assistant_text": 123,
            "tool_calls": [],
            "stop_reason": "completed"
        });
        let result = validate_model_output_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("assistant_text"));
    }

    #[test]
    fn rejects_missing_tool_calls_array() {
        let invalid = json!({
            "assistant_text": "Hello",
            "stop_reason": "completed"
        });
        let result = validate_model_output_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("tool_calls"));
    }

    #[test]
    fn rejects_tool_calls_not_array() {
        let invalid = json!({
            "assistant_text": "Hello",
            "tool_calls": "not an array",
            "stop_reason": "completed"
        });
        let result = validate_model_output_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("tool_calls"));
    }

    #[test]
    fn rejects_tool_call_missing_id() {
        let invalid = json!({
            "assistant_text": "Hello",
            "tool_calls": [
                {
                    "name": "schedule_cron",
                    "arguments": {}
                }
            ],
            "stop_reason": "completed"
        });
        let err = validate_model_output_schema(&invalid).unwrap_err();
        assert!(err.contains("tool_calls[0]"));
        assert!(err.contains("id"));
    }

    #[test]
    fn rejects_tool_call_missing_name() {
        let invalid = json!({
            "assistant_text": "Hello",
            "tool_calls": [
                {
                    "id": "call-1",
                    "arguments": {}
                }
            ],
            "stop_reason": "completed"
        });
        let err = validate_model_output_schema(&invalid).unwrap_err();
        assert!(err.contains("tool_calls[0]"));
        assert!(err.contains("name"));
    }

    #[test]
    fn rejects_tool_call_with_empty_name() {
        let invalid = json!({
            "assistant_text": "Hello",
            "tool_calls": [
                {
                    "id": "call-1",
                    "name": "   ",
                    "arguments": {}
                }
            ],
            "stop_reason": "completed"
        });
        let result = validate_model_output_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("name"));
    }

    #[test]
    fn rejects_tool_call_missing_arguments() {
        let invalid = json!({
            "assistant_text": "Hello",
            "tool_calls": [
                {
                    "id": "call-1",
                    "name": "schedule_cron"
                }
            ],
            "stop_reason": "completed"
        });
        let err = validate_model_output_schema(&invalid).unwrap_err();
        assert!(err.contains("tool_calls[0]"));
        assert!(err.contains("arguments"));
    }

    #[test]
    fn rejects_tool_call_arguments_not_object() {
        let invalid = json!({
            "assistant_text": "Hello",
            "tool_calls": [
                {
                    "id": "call-1",
                    "name": "schedule_cron",
                    "arguments": "string instead of object"
                }
            ],
            "stop_reason": "completed"
        });
        let err = validate_model_output_schema(&invalid).unwrap_err();
        assert!(err.contains("arguments"));
        assert!(err.contains("object"));
    }

    #[test]
    fn validates_valid_anthropic_response() {
        let valid_anthropic = json!({
            "content": [
                {
                    "type": "text",
                    "text": "I'll schedule that for you."
                },
                {
                    "type": "tool_use",
                    "id": "call-1",
                    "name": "schedule_cron",
                    "input": {
                        "cron": "0 9 * * *",
                        "prompt": "reminder"
                    }
                }
            ]
        });
        assert!(validate_anthropic_response_schema(&valid_anthropic).is_ok());
    }

    #[test]
    fn rejects_anthropic_missing_content() {
        let invalid = json!({});
        let result = validate_anthropic_response_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("content"));
    }

    #[test]
    fn rejects_anthropic_content_not_array() {
        let invalid = json!({
            "content": "not an array"
        });
        let result = validate_anthropic_response_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("array"));
    }

    #[test]
    fn rejects_anthropic_tool_use_missing_input() {
        let invalid = json!({
            "content": [
                {
                    "type": "tool_use",
                    "id": "call-1",
                    "name": "schedule_cron"
                }
            ]
        });
        let result = validate_anthropic_response_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("input"));
    }

    #[test]
    fn validates_valid_openai_responses_format() {
        let valid_openai = json!({
            "output": [
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "I'll help you schedule."
                        }
                    ]
                },
                {
                    "type": "function_call",
                    "call_id": "call-1",
                    "name": "schedule_cron",
                    "arguments": "{\"cron\":\"0 9 * * *\"}"
                }
            ]
        });
        assert!(validate_openai_response_schema(&valid_openai).is_ok());
    }

    #[test]
    fn validates_openai_responses_with_output_text() {
        let valid = json!({
            "output_text": "Direct text output"
        });
        assert!(validate_openai_response_schema(&valid).is_ok());
    }

    #[test]
    fn rejects_openai_responses_missing_both_output_forms() {
        let invalid = json!({
            "status": "success"
        });
        let err = validate_openai_response_schema(&invalid).unwrap_err();
        assert!(err.contains("output_text") || err.contains("output"));
    }

    #[test]
    fn rejects_openai_responses_output_not_array() {
        let invalid = json!({
            "output": "not an array"
        });
        let result = validate_openai_response_schema(&invalid);
        assert!(result.is_err());
    }

    #[test]
    fn validates_valid_openai_chat_completions() {
        let valid_chat = json!({
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": "I'll schedule that for you.",
                        "tool_calls": [
                            {
                                "id": "call-1",
                                "function": {
                                    "name": "schedule_cron",
                                    "arguments": "{\"cron\":\"0 9 * * *\"}"
                                }
                            }
                        ]
                    }
                }
            ]
        });
        assert!(validate_openai_chat_completions_schema(&valid_chat).is_ok());
    }

    #[test]
    fn rejects_openai_chat_completions_missing_choices() {
        let invalid = json!({
            "id": "chatcmpl-123"
        });
        let result = validate_openai_chat_completions_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("choices"));
    }

    #[test]
    fn rejects_openai_chat_completions_empty_choices() {
        let invalid = json!({
            "choices": []
        });
        let result = validate_openai_chat_completions_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("choices"));
    }

    #[test]
    fn rejects_openai_chat_completions_missing_message() {
        let invalid = json!({
            "choices": [
                {
                    "finish_reason": "stop"
                }
            ]
        });
        let result = validate_openai_chat_completions_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("message"));
    }

    #[test]
    fn rejects_openai_chat_completions_message_missing_role() {
        let invalid = json!({
            "choices": [
                {
                    "message": {
                        "content": "Hello"
                    }
                }
            ]
        });
        let result = validate_openai_chat_completions_schema(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("role"));
    }

    #[test]
    fn validates_json_type_names() {
        assert_eq!(json_type_name(&json!(null)), "null");
        assert_eq!(json_type_name(&json!(true)), "boolean");
        assert_eq!(json_type_name(&json!(42)), "number");
        assert_eq!(json_type_name(&json!("text")), "string");
        assert_eq!(json_type_name(&json!([])), "array");
        assert_eq!(json_type_name(&json!({})), "object");
    }
}

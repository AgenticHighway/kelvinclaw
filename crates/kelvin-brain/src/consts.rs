// --- Plugin System ---
pub const DEFAULT_TOOL_RUNTIME_KIND: &str = "wasm_tool_v1";
pub const DEFAULT_MODEL_RUNTIME_KIND: &str = "wasm_model_v1";
pub const DEFAULT_PLUGIN_HOME_RELATIVE: &str = ".kelvinclaw/plugins";
pub const DEFAULT_TRUST_POLICY_RELATIVE: &str = ".kelvinclaw/trusted_publishers.json";

// --- Timeouts and Limits ---
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_MAX_RETRIES: u32 = 0;
pub const DEFAULT_MAX_CALLS_PER_MINUTE: usize = 120;
pub const DEFAULT_CIRCUIT_BREAKER_FAILURES: u32 = 3;
pub const DEFAULT_CIRCUIT_BREAKER_COOLDOWN_MS: u64 = 30_000;

// --- WASM Skill Tool ---
pub const DEFAULT_MEMORY_APPEND_PATH: &str = "memory/skill-events.md";
pub const WASM_SKILL_PLUGIN_ID: &str = "kelvin.wasm_skill";
pub const WASM_SKILL_PLUGIN_NAME: &str = "Kelvin WASM Skill Tool";
pub const WASM_SKILL_TOOL_DEFAULT_NAME: &str = "wasm_skill";
pub const WASM_SKILL_PLUGIN_VERSION: &str = "0.1.0";
pub const WASM_SKILL_MIN_CORE_VERSION: &str = "0.1.0";

// --- WASM Skill Tool Field Names ---
pub const FIELD_WASM_PATH: &str = "wasm_path";
pub const FIELD_MEMORY_APPEND_PATH: &str = "memory_append_path";
pub const FIELD_MEMORY_ENTRY: &str = "memory_entry";
pub const FIELD_POLICY_PRESET: &str = "policy_preset";
pub const FIELD_ALLOW_MOVE_SERVO: &str = "allow_move_servo";
pub const FIELD_ALLOW_FS_READ: &str = "allow_fs_read";
pub const FIELD_NETWORK_ALLOW_HOSTS: &str = "network_allow_hosts";
pub const FIELD_MAX_MODULE_BYTES: &str = "max_module_bytes";
pub const FIELD_FUEL_BUDGET: &str = "fuel_budget";

// --- Tool Loop Detection ---
pub const TOOL_LOOP_DETECTOR_THRESHOLD: usize = 3;

// --- Model Output ---
pub const STOP_REASON_COMPLETED: &str = "completed";
pub const STOP_REASON_TOOL_CALLS: &str = "tool_calls";
pub const NO_REPLY_SIGNAL: &str = "NO_REPLY";

// --- Tool Execution ---
pub const MAX_TOOL_ITERATIONS: usize = 10;
pub const RECEIPT_REASON_MAX_LENGTH: usize = 512;

// --- Token Estimation ---
pub const TOKEN_ESTIMATION_DIVISOR: u64 = 4;

// --- Memory Path Validation ---
pub const MEMORY_ROOT_FILE: &str = "MEMORY.md";
pub const MEMORY_PREFIX: &str = "memory/";

// --- Plugin JSON Keys ---
pub const JSON_KEY_WASM_PATH: &str = "wasm_path";
pub const JSON_KEY_MEMORY_PATH: &str = "memory_path";
pub const JSON_KEY_EXIT_CODE: &str = "exit_code";
pub const JSON_KEY_CALLS: &str = "calls";
pub const JSON_KEY_KIND: &str = "kind";
pub const JSON_KEY_STREAM: &str = "stream";
pub const JSON_KEY_TOOL_RECEIPT: &str = "tool_receipt";
pub const JSON_KEY_RUN_ID: &str = "run_id";
pub const JSON_KEY_WHO: &str = "who";
pub const JSON_KEY_SESSION_ID: &str = "session_id";
pub const JSON_KEY_WHAT: &str = "what";
pub const JSON_KEY_TOOL_NAME: &str = "tool_name";
pub const JSON_KEY_TOOL_CALL_ID: &str = "tool_call_id";
pub const JSON_KEY_WHY: &str = "why";
pub const JSON_KEY_RESULT_CLASS: &str = "result_class";
pub const JSON_KEY_LATENCY_MS: &str = "latency_ms";
pub const JSON_KEY_TOOL: &str = "tool";
pub const JSON_KEY_IS_ERROR: &str = "is_error";
pub const JSON_KEY_ERROR: &str = "error";
pub const JSON_KEY_OUTPUT: &str = "output";

// --- Tool Receipt Result Classes ---
pub const RESULT_CLASS_DENIED: &str = "denied";
pub const RESULT_CLASS_ERROR: &str = "error";
pub const RESULT_CLASS_TOOL_ERROR: &str = "tool_error";
pub const RESULT_CLASS_SUCCESS: &str = "success";

// --- Claw Call Kinds ---
pub const CLAW_KIND_SEND_MESSAGE: &str = "send_message";
pub const CLAW_KIND_MOVE_SERVO: &str = "move_servo";
pub const CLAW_KIND_FS_READ: &str = "fs_read";
pub const CLAW_KIND_NETWORK_SEND: &str = "network_send";
pub const CLAW_KIND_HTTP_CALL: &str = "http_call";
pub const CLAW_KIND_ENV_ACCESS: &str = "env_access";

// --- Ed25519 Key Constants ---
pub const ED25519_KEY_SIZE_BYTES: usize = 32;

// --- Environment Variable Names ---
pub const ENV_KELVIN_PLUGIN_HOME: &str = "KELVIN_PLUGIN_HOME";
pub const ENV_KELVIN_TRUST_POLICY_PATH: &str = "KELVIN_TRUST_POLICY_PATH";
pub const ENV_HOME: &str = "HOME";
pub const ENV_USERPROFILE: &str = "USERPROFILE";

// --- Plugin Validation Limits ---
pub const MAX_TIMEOUT_MS: u64 = 600_000;
pub const MAX_RETRIES_LIMIT: u32 = 5;
pub const MAX_CALLS_PER_MINUTE_LIMIT: usize = 10_000;
pub const MAX_CIRCUIT_BREAKER_FAILURES: u32 = 100;
pub const MAX_CIRCUIT_BREAKER_COOLDOWN_MS: u64 = 600_000;
pub const MIN_CIRCUIT_BREAKER_COOLDOWN_MS: u64 = 100;
pub const MIN_TIMEOUT_MS: u64 = 1;
pub const MIN_CALLS_PER_MINUTE: usize = 1;
pub const MIN_CIRCUIT_BREAKER_FAILURES: u32 = 1;

// --- Memory Search ---
pub const MEMORY_PREVIEW_LIMIT: usize = 2;

// --- Tool Call Separator ---
pub const TOOL_CALL_OPEN_TAG: &str = "[[tool:";
pub const TOOL_CALL_CLOSE_TAG: &str = "]]";
pub const TOOL_CALL_CLOSE_TAG_LEN: usize = 2;
pub const TOOL_CALL_SPLITN_LIMIT: usize = 2;
pub const CHAR_WHITESPACE: &str = " ";

// --- Plugin Manifest ---
pub const PLUGIN_MANIFEST_FILENAME: &str = "plugin.json";
pub const PLUGIN_SIGNATURE_FILENAME: &str = "plugin.sig";
pub const PLUGIN_PAYLOAD_DIR: &str = "payload";
pub const PLUGIN_CURRENT_SYMLINK: &str = "current";
pub const APPROVAL_FIELD_NAME: &str = "approval";

// --- Plugin Quality Tiers ---
pub const QUALITY_TIER_UNSIGNED_LOCAL: &str = "unsigned_local";

// --- Core Version ---
pub const KELVIN_CORE_DEFAULT_VERSION: &str = "0.1.0";

// --- UTF-8 Conversion ---
pub const UTF8_FORMAT_STRING: &str = "{:02x}";

// --- Control Character Handling ---
pub const MEMORY_WINDOW_SECS: u64 = 60;

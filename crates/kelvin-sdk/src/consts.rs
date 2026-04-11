// --- Timeouts ---
pub const MIN_DEFAULT_TIMEOUT_MS: u64 = 100;
pub const MAX_DEFAULT_TIMEOUT_MS: u64 = 300_000;

// --- Configuration ---
pub const MAX_CONFIG_ID_LEN: usize = 128;

// --- File Operations ---
pub const DEFAULT_READ_MAX_BYTES: usize = 64 * 1024;
pub const DEFAULT_FETCH_MAX_BYTES: usize = 128 * 1024;
pub const DEFAULT_FETCH_TIMEOUT_MS: u64 = 3_000;

// --- Environment Variables ---
pub const ENV_TOOLPACK_ENABLE_FS_WRITE: &str = "KELVIN_TOOLPACK_ENABLE_FS_WRITE";
pub const ENV_TOOLPACK_ENABLE_WEB_FETCH: &str = "KELVIN_TOOLPACK_ENABLE_WEB_FETCH";
pub const ENV_TOOLPACK_ENABLE_SCHEDULER_WRITE: &str = "KELVIN_TOOLPACK_ENABLE_SCHEDULER_WRITE";
pub const ENV_TOOLPACK_ENABLE_SESSION_CLEAR: &str = "KELVIN_TOOLPACK_ENABLE_SESSION_CLEAR";
pub const ENV_TOOLPACK_WEB_ALLOW_HOSTS: &str = "KELVIN_TOOLPACK_WEB_ALLOW_HOSTS";

// --- Scheduler ---
pub const SCHEDULER_SLOT_ERROR_TRUNCATE: usize = 512;
pub const SCHEDULER_SLOT_PREVIEW_TRUNCATE: usize = 512;
pub const MAX_SCHEDULE_ID_BYTES: usize = 128;
pub const MAX_CRON_SCAN_MINUTES: usize = 1_051_200;
pub const MAX_AUDIT_ENTRIES: usize = 4_096;
pub const MAX_SLOT_ENTRIES: usize = 4_096;

// --- Validation ---
pub const MAX_APPROVAL_REASON_LEN: usize = 256;

// --- Paths and Directories ---
pub const STATE_DIR_NAME: &str = "state";
pub const KELVIN_DIR_NAME: &str = ".kelvin";
pub const STATE_SESSIONS_SUBDIR: &str = "sessions";
pub const STATE_RUNS_SUBDIR: &str = "runs";
pub const TEMP_FILE_EXTENSION: &str = "tmp";

// --- Sensitive Paths and Directories ---
// NOTE probably make more sense to regex these
pub const SENSITIVE_PATHS_COMPARE: &[&str] = &[".env"];
pub const SENSITIVE_PATHS_PREFIX: &[&str] =
    &[".env.", ".git/", ".kelvin/plugins", ".kelvinclaw/plugins"];

// --- Default Tool Scopes
// why tf is this a comma-sep string
pub const DEFAULT_WEB_ALLOW_HOSTS: &str =
    "docs.rs,crates.io,raw.githubusercontent.com,api.openai.com";
pub const DEFAULT_FS_WRITE_SCOPE: &[&str] = &["sandbox/", "memory/", "notes/"];

// --- Session Defaults ---
pub const DEFAULT_SESSION_ID: &str = "main";
pub const DEFAULT_MAX_SESSION_HISTORY_MESSAGES: usize = 128;
pub const DEFAULT_COMPACT_TO_MESSAGES: usize = 64;
pub const DEFAULT_MAX_TOOL_ITERATIONS: usize = 10;

// --- Role Names ---
pub const ROLE_USER: &str = "user";
pub const ROLE_ASSISTANT: &str = "assistant";
pub const ROLE_TOOL: &str = "tool";
pub const ROLE_SYSTEM: &str = "system";

// --- Built-in Tools ---
pub const BUILTIN_TOOL_TIME: &str = "time";
pub const PLUGIN_TOOL_KELVIN_CLI: &str = "kelvin_cli";

// --- Model Provider Identifiers ---
pub const MODEL_PROVIDER_KELVIN: &str = "kelvin";
pub const MODEL_VERSION_ECHO_V1: &str = "echo-v1";

// --- JSON Keys ---
pub const JSON_KEY_RUN_ID: &str = "run_id";
pub const JSON_KEY_UPDATED_AT_MS: &str = "updated_at_ms";
pub const JSON_KEY_LAST_STATE: &str = "last_state";
pub const JSON_KEY_LAST_WAIT: &str = "last_wait";
pub const JSON_KEY_LAST_OUTCOME: &str = "last_outcome";
pub const JSON_KEY_STATUS: &str = "status";
pub const JSON_KEY_RESULT: &str = "result";
pub const JSON_KEY_ERROR: &str = "error";
pub const JSON_KEY_TIMEOUT: &str = "timeout";
pub const JSON_KEY_COMPLETED: &str = "completed";
pub const JSON_KEY_FAILED: &str = "failed";
pub const JSON_KEY_COMPACTED: &str = "compacted";
pub const JSON_KEY_DROPPED_MESSAGES: &str = "dropped_messages";
pub const JSON_KEY_ACCEPTED_AT_MS: &str = "accepted_at_ms";
pub const JSON_KEY_SESSION_ID: &str = "session_id";
pub const JSON_KEY_WORKSPACE_DIR: &str = "workspace_dir";
pub const JSON_KEY_PROMPT_LENGTH: &str = "prompt_length";
pub const JSON_KEY_TIME: &str = "time";
pub const JSON_KEY_HUMAN: &str = "human";
pub const JSON_KEY_ISO8601: &str = "iso8601";

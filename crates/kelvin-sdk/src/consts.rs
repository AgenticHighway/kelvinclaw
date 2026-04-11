// --- Timeouts ---
pub const MIN_DEFAULT_TIMEOUT_MS: u64 = 100;
pub const MAX_DEFAULT_TIMEOUT_MS: u64 = 300_000;

// --- Configuration ---
pub const MAX_CONFIG_ID_LEN: usize = 128;

// --- File Operations ---
pub const DEFAULT_READ_MAX_BYTES: usize = 64 * 1024;
pub const DEFAULT_FETCH_MAX_BYTES: usize = 128 * 1024;
pub const DEFAULT_FETCH_TIMEOUT_MS: u64 = 3_000;
pub const DEFAULT_WEB_ALLOW_HOSTS: &str =
    "docs.rs,crates.io,raw.githubusercontent.com,api.openai.com";

// --- Environment Variables ---
pub const ENV_TOOLPACK_ENABLE_FS_WRITE: &str = "KELVIN_TOOLPACK_ENABLE_FS_WRITE";
pub const ENV_TOOLPACK_ENABLE_WEB_FETCH: &str = "KELVIN_TOOLPACK_ENABLE_WEB_FETCH";
pub const ENV_TOOLPACK_ENABLE_SCHEDULER_WRITE: &str = "KELVIN_TOOLPACK_ENABLE_SCHEDULER_WRITE";
pub const ENV_TOOLPACK_ENABLE_SESSION_CLEAR: &str = "KELVIN_TOOLPACK_ENABLE_SESSION_CLEAR";
pub const ENV_TOOLPACK_WEB_ALLOW_HOSTS: &str = "KELVIN_TOOLPACK_WEB_ALLOW_HOSTS";

// --- Scheduler ---
pub const MAX_CRON_SCAN_MINUTES: usize = 1_051_200;
pub const MAX_AUDIT_ENTRIES: usize = 4_096;
pub const MAX_SLOT_ENTRIES: usize = 4_096;

// --- Validation ---
pub const MAX_APPROVAL_REASON_LEN: usize = 256;

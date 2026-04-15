// --- Default Configuration ---
pub const DEFAULT_SESSION_ID: &str = "main";
pub const DEFAULT_TIMEOUT_MS: u64 = 300_000;
pub const DEFAULT_MAX_SESSION_HISTORY_MESSAGES: usize = 128;
pub const DEFAULT_COMPACT_TO_MESSAGES: usize = 64;

// --- Boolean Literals ---
pub const BOOL_TRUE_VALUES: &[&str] = &["1", "true", "yes", "on"];
pub const BOOL_FALSE_VALUES: &[&str] = &["0", "false", "no", "off"];

// --- Configuration Paths ---
pub const DEFAULT_STATE_DIR_PATH: &str = ".kelvin/state";

// --- CLI Arguments ---
pub const ARG_HELP_SHORT: &str = "-h";
pub const ARG_HELP_LONG: &str = "--help";
pub const ARG_INTERACTIVE: &str = "--interactive";
pub const ARG_PROMPT: &str = "--prompt";
pub const ARG_SESSION: &str = "--session";
pub const ARG_WORKSPACE: &str = "--workspace";
pub const ARG_MEMORY: &str = "--memory";
pub const ARG_TIMEOUT_MS: &str = "--timeout-ms";
pub const ARG_SYSTEM: &str = "--system";
pub const ARG_MODEL_PROVIDER: &str = "--model-provider";
pub const ARG_STATE_DIR: &str = "--state-dir";
pub const ARG_PERSIST_RUNS: &str = "--persist-runs";
pub const ARG_MAX_SESSION_HISTORY: &str = "--max-session-history";
pub const ARG_COMPACT_TO: &str = "--compact-to";

// --- Interactive Mode ---
pub const HELP_COMMAND: &str = "/help";
pub const EXIT_COMMAND_LOWERCASE: &str = "/exit";
pub const EXIT_COMMAND_QUIT: &str = "/quit";
pub const INTERACTIVE_PROMPT: &str = "kelvin> ";
pub const BYTES_EOF: usize = 0;

// --- Plugin IDs ---
pub const KELVIN_CLI_PLUGIN_ID: &str = "kelvin_cli";
pub const OPENAI_API_KEY_VAR: &str = "OPENAI_API_KEY";
pub const ANTHROPIC_API_KEY_VAR: &str = "ANTHROPIC_API_KEY";

// --- Timeouts ---
pub const TIMEOUT_BUFFER_MS: u64 = 5_000;

// --- Runtime Configuration ---
pub const MAX_TOOL_ITERATIONS: usize = 10;

// --- Exit Codes ---
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_FAILURE: i32 = 1;

// --- String Patterns ---
pub const USAGE_PREFIX: &str = "Usage:";

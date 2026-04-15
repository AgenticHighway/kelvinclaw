// --- ABI: Claw (Skill Host) ---
pub const CLAW_ABI_VERSION: &str = "1.0.0";
pub const CLAW_MODULE: &str = "claw";
pub const CLAW_RUN_EXPORT: &str = "run";
pub const CLAW_SEND_MESSAGE: &str = "send_message";
pub const CLAW_MOVE_SERVO: &str = "move_servo";
pub const CLAW_FS_READ: &str = "fs_read";
pub const CLAW_NETWORK_SEND: &str = "network_send";
pub const CLAW_EXPORT_MEMORY: &str = "memory";
pub const CLAW_EXPORT_ALLOC: &str = "alloc";
pub const CLAW_EXPORT_DEALLOC: &str = "dealloc";
pub const CLAW_HANDLE_TOOL_CALL: &str = "handle_tool_call";
pub const CLAW_IMPORT_LOG: &str = "log";
pub const CLAW_HTTP_CALL: &str = "http_call";
pub const CLAW_GET_ENV: &str = "get_env";
pub const CLAW_SHELL_EXEC: &str = "shell_exec";

// --- ABI: Model Host ---
pub const MODEL_ABI_VERSION: &str = "1.0.0";
pub const MODEL_MODULE: &str = "kelvin_model_host_v1";
pub const MODEL_EXPORT_ALLOC: &str = "alloc";
pub const MODEL_EXPORT_DEALLOC: &str = "dealloc";
pub const MODEL_EXPORT_INFER: &str = "infer";
pub const MODEL_EXPORT_MEMORY: &str = "memory";
pub const MODEL_IMPORT_OPENAI_RESPONSES_CALL: &str = "openai_responses_call";
pub const MODEL_IMPORT_PROVIDER_PROFILE_CALL: &str = "provider_profile_call";
pub const MODEL_IMPORT_LOG: &str = "log";
pub const MODEL_IMPORT_CLOCK_NOW_MS: &str = "clock_now_ms";
pub const MODEL_PAYLOAD_MAX_TOKENS: usize = 1024;

// --- ABI: Channel Host ---
pub const CHANNEL_ABI_VERSION: &str = "1.0.0";
pub const CHANNEL_MODULE: &str = "kelvin_channel_host_v1";
pub const CHANNEL_EXPORT_ALLOC: &str = "alloc";
pub const CHANNEL_EXPORT_DEALLOC: &str = "dealloc";
pub const CHANNEL_EXPORT_HANDLE_INGEST: &str = "handle_ingest";
pub const CHANNEL_EXPORT_MEMORY: &str = "memory";
pub const CHANNEL_IMPORT_LOG: &str = "log";
pub const CHANNEL_IMPORT_CLOCK_NOW_MS: &str = "clock_now_ms";

// --- Buffer Sizes ---
pub const DEFAULT_MAX_MODULE_BYTES: usize = 512 * 1024;
pub const DEFAULT_MAX_REQUEST_BYTES: usize = 256 * 1024;
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 256 * 1024;
pub const MODEL_DEFAULT_MAX_REQUEST_BYTES: usize = 256 * 1024;
pub const MODEL_DEFAULT_MAX_RESPONSE_BYTES: usize = 1024 * 1024;
pub const CHANNEL_DEFAULT_MAX_REQUEST_BYTES: usize = 256 * 1024;
pub const CHANNEL_DEFAULT_MAX_RESPONSE_BYTES: usize = 256 * 1024;
/// Maximum bytes allowed for a single log message emitted by a model WASM plugin.
/// Kept deliberately small to prevent untrusted modules from flooding logs or causing DoS.
pub const MODEL_LOG_MAX_BYTES: usize = 16 * 1024;
/// Maximum bytes allowed for a single log message emitted by a channel WASM plugin.
/// Kept deliberately small to prevent untrusted modules from flooding logs or causing DoS.
pub const CHANNEL_LOG_MAX_BYTES: usize = 16 * 1024;

// --- Fuel/Execution ---
pub const DEFAULT_FUEL_BUDGET: u64 = 1_000_000;
pub const MAX_FUEL_BUDGET: u64 = 100_000_000;

// --- Timeouts ---
pub const MODEL_DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub const HTTP_CALL_TIMEOUT_SECS: u64 = 30;
/// Hard upper bound on shell_exec timeout to prevent runaway processes.
pub const SHELL_EXEC_MAX_TIMEOUT_SECS: u64 = 30;
/// Default timeout applied when the guest does not specify one.
pub const SHELL_EXEC_DEFAULT_TIMEOUT_SECS: u64 = 10;
/// Maximum combined stdout + stderr bytes returned to the guest.
pub const SHELL_EXEC_MAX_OUTPUT_BYTES: usize = 64 * 1024;

// --- Interpreter Guard ---
/// Known script interpreters that can execute arbitrary code.
/// When one of these is the command basename, inline-code flags are blocked.
pub const KNOWN_INTERPRETERS: &[&str] = &[
    "python",
    "python3",
    "python2",
    "node",
    "nodejs",
    "bash",
    "sh",
    "zsh",
    "dash",
    "ksh",
    "csh",
    "tcsh",
    "fish",
    "ruby",
    "irb",
    "perl",
    "perl5",
    "php",
    "lua",
    "luajit",
    "Rscript",
    "julia",
    "powershell",
    "pwsh",
];

/// Argument flags that enable inline code execution for interpreters.
/// Any argument starting with one of these (exact or `=`-suffixed) is rejected
/// when the command is a known interpreter.
pub const INTERPRETER_INLINE_FLAGS: &[&str] = &[
    "-c",        // python, bash, sh, zsh, etc.
    "-e",        // ruby, perl, node (--eval shorthand)
    "-E",        // perl (like -e but with extra features)
    "-r",        // php (-r 'code')
    "-p",        // node (--print shorthand — eval + print)
    "--eval",    // node
    "--print",   // node
    "--command", // powershell
    "-Command",  // powershell
];

// --- Network/Hosts ---
pub const DEFAULT_OPENAI_HOST: &str = "api.openai.com";

// --- Security ---
pub const BLOCKED_HEADERS: &[&str] = &[
    "host",
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
    "transfer-encoding",
    "te",
    "connection",
    "upgrade",
];

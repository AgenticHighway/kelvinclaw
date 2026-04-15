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
/// Per-interpreter mapping of single-character flags that enable inline code
/// execution.  Each interpreter only checks the chars relevant to *its own*
/// semantics, avoiding false positives (e.g. `bash -r` means restricted mode,
/// not inline code; `bash -p` means privileged mode).
///
/// An empty char slice means the interpreter has no short inline-exec flags
/// (e.g. PowerShell uses long-form `-Command` / `--command` only).
///
/// This table doubles as the known-interpreter list: any command whose
/// lowercase basename matches an entry is subject to the inline-code guard.
pub const INTERPRETER_INLINE_MAP: &[(&str, &[char])] = &[
    // POSIX shells: only -c is inline-exec
    ("bash", &['c']),
    ("sh", &['c']),
    ("zsh", &['c']),
    ("dash", &['c']),
    ("ksh", &['c']),
    ("csh", &['c']),
    ("tcsh", &['c']),
    ("fish", &['c']),
    // Python: -c 'code'
    ("python", &['c']),
    ("python3", &['c']),
    ("python2", &['c']),
    // Node.js: -e (--eval), -p (--print)
    ("node", &['e', 'p']),
    ("nodejs", &['e', 'p']),
    // Ruby: -e 'code'
    ("ruby", &['e']),
    ("irb", &['e']),
    // Perl: -e 'code', -E 'code' (with extra features)
    ("perl", &['e', 'E']),
    ("perl5", &['e', 'E']),
    // PHP: -r 'code'
    ("php", &['r']),
    // Lua: -e 'code'
    ("lua", &['e']),
    ("luajit", &['e']),
    // R: -e 'code'
    ("Rscript", &['e']),
    // Julia: -e 'code'
    ("julia", &['e']),
    // PowerShell: handled via dedicated prefix-of-"-Command" check in
    // is_interpreter_inline_exec (case-insensitive, covers -c, -C, -co,
    // -Com, -COMMAND, etc.).  No short inline_chars needed.
    ("powershell", &[]),
    ("pwsh", &[]),
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

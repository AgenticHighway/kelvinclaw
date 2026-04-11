// --- SDK Constants ---
pub const KELVIN_CORE_SDK_NAME: &str = "Kelvin Core";
pub const KELVIN_CORE_API_VERSION: &str = "1.0.0";

// --- Plugin Limits ---
pub const MAX_PLUGIN_ID_LEN: usize = 128;
pub const MAX_PLUGIN_NAME_LEN: usize = 128;
pub const MAX_PLUGIN_DESCRIPTION_LEN: usize = 4_096;
pub const MAX_PLUGIN_HOMEPAGE_LEN: usize = 2_048;
pub const MAX_PLUGIN_CAPABILITIES: usize = 32;

// --- Display Limits ---
pub const DISPLAY_PREVIEW_MAX_LEN: usize = 64;

// --- Model Provider Profiles ---
pub const OPENAI_RESPONSES_PROFILE_ID: &str = "openai.responses";
pub const ANTHROPIC_MESSAGES_PROFILE_ID: &str = "anthropic.messages";

// --- OpenAI Provider Configuration ---
pub const OPENAI_PROVIDER_NAME: &str = "openai";
pub const OPENAI_API_KEY_ENV: &str = "OPENAI_API_KEY";
pub const OPENAI_BASE_URL_ENV: &str = "OPENAI_BASE_URL";
pub const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com";
pub const OPENAI_ENDPOINT_PATH: &str = "v1/responses";
pub const OPENAI_AUTH_HEADER: &str = "authorization";
pub const OPENAI_ALLOW_HOST: &str = "api.openai.com";
pub const OPENAI_DEFAULT_MODEL: &str = "gpt-4.1-mini";

// --- Anthropic Provider Configuration ---
pub const ANTHROPIC_PROVIDER_NAME: &str = "anthropic";
pub const ANTHROPIC_API_KEY_ENV: &str = "ANTHROPIC_API_KEY";
pub const ANTHROPIC_BASE_URL_ENV: &str = "ANTHROPIC_BASE_URL";
pub const ANTHROPIC_DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub const ANTHROPIC_ENDPOINT_PATH: &str = "v1/messages";
pub const ANTHROPIC_AUTH_HEADER: &str = "x-api-key";
pub const ANTHROPIC_VERSION_HEADER_NAME: &str = "anthropic-version";
pub const ANTHROPIC_VERSION_HEADER_VALUE: &str = "2023-06-01";
pub const ANTHROPIC_ALLOW_HOST: &str = "api.anthropic.com";
pub const ANTHROPIC_DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001";

// --- OpenRouter Configuration ---
pub const OPENROUTER_PROVIDER_NAME: &str = "openrouter";
pub const OPENROUTER_DEFAULT_MODEL: &str = "openai/gpt-4.1-mini";

// --- Memory Defaults ---
pub const MEMORY_DEFAULT_BACKEND: &str = "builtin";
pub const MEMORY_DEFAULT_PROVIDER: &str = "unknown";
pub const MEMORY_DEFAULT_MAX_RESULTS: usize = 6;
pub const MEMORY_DEFAULT_MIN_SCORE_MILLI: u16 = 0;

// --- Validation & URLs ---
pub const HTTPS_SCHEME: &str = "https://";
pub const HTTP_SCHEME: &str = "http://";

// --- Timeout & Duration ---
pub const MIN_TIMEOUT_MS: u64 = 1;

//! Constants for the kelvin-memory crate.

// --- Backend and Provider Identifiers ---
pub const BACKEND_BUILTIN: &str = "builtin";
pub const PROVIDER_MARKDOWN: &str = "markdown";
pub const PROVIDER_IN_MEMORY_VECTOR: &str = "in_memory_vector";

// --- File Paths and Extensions ---
pub const MEMORY_FILE: &str = "MEMORY.md";
pub const MEMORY_DIR: &str = "memory";
pub const MARKDOWN_EXTENSION: &str = "md";

// --- Model Names ---
pub const MODEL_TOKEN_OVERLAP_V1: &str = "token-overlap-v1";

// --- JSON Keys and Structures ---
pub const JSON_KEY_FALLBACK: &str = "fallback";
pub const JSON_KEY_ENABLED: &str = "enabled";
pub const JSON_KEY_REASON: &str = "reason";
pub const JSON_KEY_SOURCE_OF_TRUTH: &str = "source_of_truth";
pub const JSON_VALUE_WORKSPACE_MARKDOWN: &str = "workspace_markdown";
pub const JSON_KEY_INDEX: &str = "index";
pub const JSON_VALUE_VOLATILE: &str = "volatile";

// --- Path Traversal Protection ---
pub const PATH_TRAVERSAL_PATTERN: &str = "..";

// --- Special Provider Names for Parsing ---
pub const PARSE_MARKDOWN: &str = "markdown";
pub const PARSE_IN_MEMORY: &str = "in-memory";
pub const PARSE_IN_MEMORY_ALT: &str = "in_memory";
pub const PARSE_VECTOR: &str = "vector";
pub const PARSE_FALLBACK: &str = "fallback";
pub const PARSE_IN_MEMORY_FALLBACK: &str = "in-memory-fallback";
pub const PARSE_IN_MEMORY_FALLBACK_ALT: &str = "in_memory_fallback";

// --- Search and Display Constants ---
pub const SEARCH_QUERY_ROUTER: &str = "router";
pub const SEARCH_SNIPPET_CONTEXT_LINE_DELTA: usize = 1;

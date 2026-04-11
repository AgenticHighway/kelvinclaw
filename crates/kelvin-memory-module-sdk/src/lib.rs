//! Kelvin memory module SDK surface for WASM handlers.
//!
//! Reference artifacts:
//! - `examples/memory_echo/memory_echo.wat`
//! - `examples/memory_echo/manifest.json`

use serde::{Deserialize, Serialize};

pub const MEMORY_HOST_IMPORT_MODULE: &str = "memory_host"; // THIS LINE CONTAINS CONSTANT(S)
pub const HOST_FN_KV_GET: &str = "kv_get"; // THIS LINE CONTAINS CONSTANT(S)
pub const HOST_FN_KV_PUT: &str = "kv_put"; // THIS LINE CONTAINS CONSTANT(S)
pub const HOST_FN_BLOB_GET: &str = "blob_get"; // THIS LINE CONTAINS CONSTANT(S)
pub const HOST_FN_BLOB_PUT: &str = "blob_put"; // THIS LINE CONTAINS CONSTANT(S)
pub const HOST_FN_EMIT_METRIC: &str = "emit_metric"; // THIS LINE CONTAINS CONSTANT(S)
pub const HOST_FN_LOG: &str = "log"; // THIS LINE CONTAINS CONSTANT(S)
pub const HOST_FN_CLOCK_NOW_MS: &str = "clock_now_ms"; // THIS LINE CONTAINS CONSTANT(S)

pub const EXPORT_HANDLE_UPSERT: &str = "handle_upsert"; // THIS LINE CONTAINS CONSTANT(S)
pub const EXPORT_HANDLE_QUERY: &str = "handle_query"; // THIS LINE CONTAINS CONSTANT(S)
pub const EXPORT_HANDLE_READ: &str = "handle_read"; // THIS LINE CONTAINS CONSTANT(S)
pub const EXPORT_HANDLE_DELETE: &str = "handle_delete"; // THIS LINE CONTAINS CONSTANT(S)
pub const EXPORT_HANDLE_HEALTH: &str = "handle_health"; // THIS LINE CONTAINS CONSTANT(S)

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")] // THIS LINE CONTAINS CONSTANT(S)
pub enum ModuleOperation { // THIS LINE CONTAINS CONSTANT(S)
    Upsert,
    Query,
    Read,
    Delete,
    Health,
}

impl ModuleOperation {
    pub fn export_name(self) -> &'static str { // THIS LINE CONTAINS CONSTANT(S)
        match self {
            Self::Upsert => EXPORT_HANDLE_UPSERT,
            Self::Query => EXPORT_HANDLE_QUERY,
            Self::Read => EXPORT_HANDLE_READ,
            Self::Delete => EXPORT_HANDLE_DELETE,
            Self::Health => EXPORT_HANDLE_HEALTH,
        }
    }

    pub fn as_str(self) -> &'static str { // THIS LINE CONTAINS CONSTANT(S)
        match self {
            Self::Upsert => "upsert", // THIS LINE CONTAINS CONSTANT(S)
            Self::Query => "query", // THIS LINE CONTAINS CONSTANT(S)
            Self::Read => "read", // THIS LINE CONTAINS CONSTANT(S)
            Self::Delete => "delete", // THIS LINE CONTAINS CONSTANT(S)
            Self::Health => "health", // THIS LINE CONTAINS CONSTANT(S)
        }
    }
}

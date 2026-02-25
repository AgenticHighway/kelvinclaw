//! Kelvin memory module SDK surface for WASM handlers.
//!
//! Reference artifacts:
//! - `examples/memory_echo/memory_echo.wat`
//! - `examples/memory_echo/manifest.json`

use serde::{Deserialize, Serialize};

pub const MEMORY_HOST_IMPORT_MODULE: &str = "memory_host";
pub const HOST_FN_KV_GET: &str = "kv_get";
pub const HOST_FN_KV_PUT: &str = "kv_put";
pub const HOST_FN_BLOB_GET: &str = "blob_get";
pub const HOST_FN_BLOB_PUT: &str = "blob_put";
pub const HOST_FN_EMIT_METRIC: &str = "emit_metric";
pub const HOST_FN_LOG: &str = "log";
pub const HOST_FN_CLOCK_NOW_MS: &str = "clock_now_ms";

pub const EXPORT_HANDLE_UPSERT: &str = "handle_upsert";
pub const EXPORT_HANDLE_QUERY: &str = "handle_query";
pub const EXPORT_HANDLE_READ: &str = "handle_read";
pub const EXPORT_HANDLE_DELETE: &str = "handle_delete";
pub const EXPORT_HANDLE_HEALTH: &str = "handle_health";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleOperation {
    Upsert,
    Query,
    Read,
    Delete,
    Health,
}

impl ModuleOperation {
    pub fn export_name(self) -> &'static str {
        match self {
            Self::Upsert => EXPORT_HANDLE_UPSERT,
            Self::Query => EXPORT_HANDLE_QUERY,
            Self::Read => EXPORT_HANDLE_READ,
            Self::Delete => EXPORT_HANDLE_DELETE,
            Self::Health => EXPORT_HANDLE_HEALTH,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Query => "query",
            Self::Read => "read",
            Self::Delete => "delete",
            Self::Health => "health",
        }
    }
}

// --- Host Import Module ---
pub const MEMORY_HOST_IMPORT_MODULE: &str = "memory_host";

// --- Host Functions ---
pub const HOST_FN_KV_GET: &str = "kv_get";
pub const HOST_FN_KV_PUT: &str = "kv_put";
pub const HOST_FN_BLOB_GET: &str = "blob_get";
pub const HOST_FN_BLOB_PUT: &str = "blob_put";
pub const HOST_FN_EMIT_METRIC: &str = "emit_metric";
pub const HOST_FN_LOG: &str = "log";
pub const HOST_FN_CLOCK_NOW_MS: &str = "clock_now_ms";

// --- Export Handlers ---
pub const EXPORT_HANDLE_UPSERT: &str = "handle_upsert";
pub const EXPORT_HANDLE_QUERY: &str = "handle_query";
pub const EXPORT_HANDLE_READ: &str = "handle_read";
pub const EXPORT_HANDLE_DELETE: &str = "handle_delete";
pub const EXPORT_HANDLE_HEALTH: &str = "handle_health";

// --- Operation String Representations ---
pub const OP_UPSERT_STR: &str = "upsert";
pub const OP_QUERY_STR: &str = "query";
pub const OP_READ_STR: &str = "read";
pub const OP_DELETE_STR: &str = "delete";
pub const OP_HEALTH_STR: &str = "health";

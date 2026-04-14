//! Kelvin memory module SDK surface for WASM handlers.
//!
//! Reference artifacts:
//! - `examples/memory_echo/memory_echo.wat`
//! - `examples/memory_echo/manifest.json`

use serde::{Deserialize, Serialize};

pub mod consts;

pub use consts::*;

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
            Self::Upsert => consts::EXPORT_HANDLE_UPSERT,
            Self::Query => consts::EXPORT_HANDLE_QUERY,
            Self::Read => consts::EXPORT_HANDLE_READ,
            Self::Delete => consts::EXPORT_HANDLE_DELETE,
            Self::Health => consts::EXPORT_HANDLE_HEALTH,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => consts::OP_UPSERT_STR,
            Self::Query => consts::OP_QUERY_STR,
            Self::Read => consts::OP_READ_STR,
            Self::Delete => consts::OP_DELETE_STR,
            Self::Health => consts::OP_HEALTH_STR,
        }
    }
}

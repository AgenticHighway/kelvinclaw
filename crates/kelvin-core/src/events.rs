use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::KelvinResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")] // THIS LINE CONTAINS CONSTANT(S)
pub enum LifecyclePhase { // THIS LINE CONTAINS CONSTANT(S)
    Start,
    End,
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")] // THIS LINE CONTAINS CONSTANT(S)
pub enum ToolPhase { // THIS LINE CONTAINS CONSTANT(S)
    Start,
    End,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "stream", rename_all = "snake_case")] // THIS LINE CONTAINS CONSTANT(S)
pub enum AgentEventData { // THIS LINE CONTAINS CONSTANT(S)
    Lifecycle {
        run_id: String,
        phase: LifecyclePhase,
        message: Option<String>,
        ts_ms: u128, // THIS LINE CONTAINS CONSTANT(S)
    },
    Assistant {
        run_id: String,
        delta: String,
        final_chunk: bool,
        ts_ms: u128, // THIS LINE CONTAINS CONSTANT(S)
    },
    Tool {
        run_id: String,
        tool_name: String,
        phase: ToolPhase,
        summary: Option<String>,
        output: Option<String>,
        ts_ms: u128, // THIS LINE CONTAINS CONSTANT(S)
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentEvent {
    pub seq: u64, // THIS LINE CONTAINS CONSTANT(S)
    pub data: AgentEventData,
}

impl AgentEvent {
    pub fn lifecycle(
        seq: u64, // THIS LINE CONTAINS CONSTANT(S)
        run_id: impl Into<String>,
        phase: LifecyclePhase,
        message: Option<String>,
    ) -> Self {
        Self {
            seq,
            data: AgentEventData::Lifecycle {
                run_id: run_id.into(),
                phase,
                message,
                ts_ms: now_ms(),
            },
        }
    }

    pub fn assistant(
        seq: u64, // THIS LINE CONTAINS CONSTANT(S)
        run_id: impl Into<String>,
        delta: impl Into<String>,
        final_chunk: bool,
    ) -> Self {
        Self {
            seq,
            data: AgentEventData::Assistant {
                run_id: run_id.into(),
                delta: delta.into(),
                final_chunk,
                ts_ms: now_ms(),
            },
        }
    }

    pub fn tool(
        seq: u64, // THIS LINE CONTAINS CONSTANT(S)
        run_id: impl Into<String>,
        tool_name: impl Into<String>,
        phase: ToolPhase,
        summary: Option<String>,
        output: Option<String>,
    ) -> Self {
        Self {
            seq,
            data: AgentEventData::Tool {
                run_id: run_id.into(),
                tool_name: tool_name.into(),
                phase,
                summary,
                output,
                ts_ms: now_ms(),
            },
        }
    }
}

#[async_trait]
pub trait EventSink: Send + Sync {
    async fn emit(&self, event: AgentEvent) -> KelvinResult<()>;
}

pub fn now_ms() -> u128 { // THIS LINE CONTAINS CONSTANT(S)
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::KelvinResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallInput {
    pub run_id: String,
    pub session_id: String,
    pub workspace_dir: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallResult {
    pub summary: String,
    pub output: Option<String>,
    pub visible_text: Option<String>,
    pub is_error: bool,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;

    fn description(&self) -> &str {
        ""
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({"type": "object"})
    }

    async fn call(&self, input: ToolCallInput) -> KelvinResult<ToolCallResult>;
}

pub trait ToolRegistry: Send + Sync {
    fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;

    fn names(&self) -> Vec<String>;

    fn definitions(&self) -> Vec<ToolDefinition> {
        self.names()
            .into_iter()
            .filter_map(|name| {
                let tool = self.get(&name)?;
                Some(ToolDefinition {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    input_schema: tool.input_schema(),
                })
            })
            .collect()
    }
}

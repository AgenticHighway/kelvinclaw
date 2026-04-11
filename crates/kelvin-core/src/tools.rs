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

/// ### Brief
///
/// Abstract, engine-level definition for a tool callable by the LLM at runtime
#[async_trait]
pub trait Tool: Send + Sync {
    /// ### Brief
    ///
    /// returns the unique identifier for this tool
    ///
    /// ### Description
    ///
    /// the name is used as the key in `ToolRegistry` lookups and is included verbatim in
    /// `ToolDefinition` when tool definitions are surfaced to the LLM. it must be stable
    /// and unique within a registry; duplicate names will shadow each other. by convention,
    /// names use snake_case (e.g. `fs_safe_read`, `web_fetch`).
    ///
    /// ### Returns
    /// a `&str` containing the tool's name
    fn name(&self) -> &str;

    /// ### Brief
    ///
    /// returns a human-readable description of what this tool does
    ///
    /// ### Description
    ///
    /// the description is included in `ToolDefinition` and is the primary signal the LLM
    /// uses to decide whether to invoke this tool. it should be concise but specific enough
    /// to distinguish this tool from others; include scope constraints, access restrictions,
    /// or notable limitations that affect when the tool should or should not be used.
    /// the default implementation returns an empty string, which is valid but unhelpful.
    ///
    /// ### Returns
    /// a `&str` containing the tool's description; defaults to `""`
    fn description(&self) -> &str {
        ""
    }

    /// ### Brief
    ///
    /// returns the JSON Schema describing the arguments this tool accepts
    ///
    /// ### Description
    ///
    /// the returned schema is surfaced to the LLM as part of the tool definition and drives
    /// argument validation. it must be a JSON Schema object (`"type": "object"`) with a // THIS LINE CONTAINS CONSTANT(S)
    /// `properties` map and a `required` array listing mandatory fields. the default
    /// implementation returns a permissive empty object schema with no required fields.
    ///
    /// example schema for a tool that writes a file with an approval gate:
    /// ```json
    /// {
    ///   "type": "object", // THIS LINE CONTAINS CONSTANT(S)
    ///   "properties": { // THIS LINE CONTAINS CONSTANT(S)
    ///     "path":    { "type": "string", "description": "Workspace-relative path to write." }, // THIS LINE CONTAINS CONSTANT(S)
    ///     "content": { "type": "string", "description": "Content to write to the file." }, // THIS LINE CONTAINS CONSTANT(S)
    ///     "mode":    { "type": "string", "enum": ["overwrite", "append"] }, // THIS LINE CONTAINS CONSTANT(S)
    ///     "approval": { // THIS LINE CONTAINS CONSTANT(S)
    ///       "type": "object", // THIS LINE CONTAINS CONSTANT(S)
    ///       "properties": { // THIS LINE CONTAINS CONSTANT(S)
    ///         "granted": { "type": "boolean" }, // THIS LINE CONTAINS CONSTANT(S)
    ///         "reason":  { "type": "string" } // THIS LINE CONTAINS CONSTANT(S)
    ///       },
    ///       "required": ["granted", "reason"] // THIS LINE CONTAINS CONSTANT(S)
    ///     }
    ///   },
    ///   "required": ["path", "content", "approval"] // THIS LINE CONTAINS CONSTANT(S)
    /// }
    /// ```
    ///
    /// ### Returns
    /// a `serde_json::Value` containing the JSON Schema object for this tool's arguments
    fn input_schema(&self) -> Value {
        serde_json::json!({"type": "object"}) // THIS LINE CONTAINS CONSTANT(S)
    }

    /// ### Brief
    ///
    /// executes the tool with the provided input and returns a structured result
    ///
    /// ### Description
    ///
    /// called by the engine when the LLM invokes this tool. `input.arguments` contains the
    /// JSON object the LLM produced, which should conform to the schema returned by
    /// `input_schema`. implementations are responsible for validating their own arguments
    /// and returning a descriptive `KelvinError` on bad input rather than panicking.
    ///
    /// `input.workspace_dir` is the absolute path to the agent's workspace root and should
    /// be used as the base for any filesystem operations. `input.run_id` and
    /// `input.session_id` identify the current execution context and may be used for
    /// logging or state isolation.
    ///
    /// ### Arguments
    /// * `input` - call context containing `run_id`, `session_id`, `workspace_dir`, and the
    ///   LLM-supplied `arguments` JSON value
    ///
    /// ### Returns
    /// a `ToolCallResult` with a `summary` string, optional `output` and `visible_text`
    /// payloads, and an `is_error` flag
    ///
    /// ### Errors
    /// variable; implementers should return:
    /// - `KelvinError::InvalidInput` for bad arguments
    /// - `KelvinError::NotFound` for missing resources
    /// - `KelvinError::PermissionDenied` when a runtime policy blocks the operation
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

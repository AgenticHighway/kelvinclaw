use chrono::Utc;
use kelvin_core::ToolDefinition;

/// ### Brief
///
/// input parameters for system prompt assembly
///
/// ### Description
///
/// carries all runtime context needed to build a structured system prompt. passed to `build()`
/// once per agent run, before the first model call. fields that are optional produce no section
/// in the output when absent.
///
/// ### Fields
/// * `run_id` - identifier for the current agent run; injected into the runtime info section
/// * `session_id` - identifier for the current session; injected into the runtime info section
/// * `model_provider` - name of the active model provider (e.g. "anthropic", "openai")
/// * `model_name` - name of the active model (e.g. "claude-sonnet-4-6")
/// * `workspace_dir` - absolute path to the agent's working directory; always injected into the prompt
/// * `tools` - tool definitions registered in the current run; an empty slice omits the tools section
/// * `extra_system_prompt` - caller-supplied prompt text appended as the final section; `None` omits it
pub struct SystemPromptParams<'a> {
    pub run_id: &'a str,
    pub session_id: &'a str,
    pub model_provider: &'a str,
    pub model_name: &'a str,
    pub workspace_dir: &'a str,
    pub tools: &'a [ToolDefinition],
    pub extra_system_prompt: Option<&'a str>,
}

/// ### Brief
///
/// a single named section of the assembled system prompt
///
/// ### Description
///
/// each variant represents one logical block of the prompt. `build()` constructs a list of
/// sections, renders each via `render()`, and joins them with double newlines. variants that
/// carry data are only constructed when that data is present — the caller is responsible for
/// the conditional; `render()` has no awareness of whether a section should exist.
enum Section {
    /// fixed agent identity and behavioral preamble; always first
    Identity,
    /// fixed tool use policy explaining the approval gate; always present
    Safety,
    /// human-readable tool listing as `(name, description)` pairs; omitted when the registry is empty
    Tools(Vec<(String, String)>),
    /// working directory injected at runtime; always present
    Workspace(String),
    /// current UTC date and time as a formatted string; always present
    DateTime(String),
    /// run_id, session_id, model provider, and model name for this run; always present
    RuntimeInfo {
        run_id: String,
        session_id: String,
        model_provider: String,
        model_name: String,
    },
    /// verbatim caller-supplied text; appended last when provided
    Extra(String),
}

impl Section {
    /// ### Brief
    ///
    /// renders this section to its string representation
    ///
    /// ### Description
    ///
    /// each variant produces a self-contained block of text. `Tools` renders one bullet per
    /// entry; entries with an empty description omit the colon separator. `RuntimeInfo` renders
    /// as a labeled list. `Extra` is returned verbatim. all other variants produce a fixed block.
    ///
    /// ### Returns
    /// the rendered section as a `String`
    fn render(&self) -> String {
        match self {
            Section::Identity => {
                "You are Kelvin, an AI assistant. You help users accomplish tasks by reasoning carefully and using the tools available to you.".to_string()
            }

            Section::Safety => {
                concat!(
                    "Tool use policy:\n",
                    "Some tools are marked sensitive and require an explicit approval object in their arguments before they will execute. ",
                    "When calling a sensitive tool, you must provide the following field:\n\n",
                    "  \"approval\": { \"granted\": true, \"reason\": \"your reason here\" }\n\n",
                    "Rules for the approval field:\n",
                    "- granted must be the boolean true (not a string)\n",
                    "- reason must be a non-empty string, at most 256 characters, with no control characters\n",
                    "- if approval is absent, granted is not true, or reason is invalid, the tool call is blocked\n",
                    "Only invoke sensitive tools when the current task clearly warrants it."
                ).to_string()
            }

            Section::Tools(tools) => {
                let mut lines = vec!["Available tools:".to_string()];
                for (name, description) in tools {
                    if description.is_empty() {
                        lines.push(format!("- {name}"));
                    } else {
                        lines.push(format!("- {name}: {description}"));
                    }
                }
                lines.join("\n")
            }

            Section::Workspace(dir) => {
                let resolved = std::fs::canonicalize(dir)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| dir.clone());
                format!("Working directory: {resolved}")
            }

            Section::DateTime(dt) => {
                format!("Current date and time (UTC): {dt}")
            }

            Section::RuntimeInfo {
                run_id,
                session_id,
                model_provider,
                model_name,
            } => {
                format!(
                    "Runtime:\n- run_id: {run_id}\n- session_id: {session_id}\n- model: {model_provider} / {model_name}"
                )
            }

            Section::Extra(text) => text.clone(),
        }
    }
}

/// ### Brief
///
/// assembles a structured system prompt from runtime context
///
/// ### Description
///
/// constructs the ordered list of sections from `params`, renders each, and joins them
/// with double newlines. the section order is fixed: identity, safety, tools (if any),
/// workspace, datetime, runtime info, extra (if provided). tool definitions are summarized
/// by name and description only — full input schemas are passed separately via the model
/// API's native tool-calling mechanism and are not duplicated here. the current UTC
/// timestamp is captured at call time.
///
/// ### Arguments
/// * `params` - runtime context used to populate each section
///
/// ### Returns
/// the fully assembled system prompt as a `String`
pub fn build(params: SystemPromptParams) -> String {
    let tool_summaries: Vec<(String, String)> = params
        .tools
        .iter()
        .map(|t| (t.name.clone(), t.description.clone()))
        .collect();

    let now = Utc::now().to_rfc3339();

    [
        Some(Section::Identity),
        Some(Section::Safety),
        if tool_summaries.is_empty() {
            None
        } else {
            Some(Section::Tools(tool_summaries))
        },
        Some(Section::Workspace(params.workspace_dir.to_string())),
        Some(Section::DateTime(now)),
        Some(Section::RuntimeInfo {
            run_id: params.run_id.to_string(),
            session_id: params.session_id.to_string(),
            model_provider: params.model_provider.to_string(),
            model_name: params.model_name.to_string(),
        }),
        params
            .extra_system_prompt
            .map(|s| Section::Extra(s.to_string())),
    ]
    .into_iter()
    .flatten()
    .map(|s| s.render())
    .collect::<Vec<_>>()
    .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use kelvin_core::ToolDefinition;
    use serde_json::json;

    fn tool(name: &str, description: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: json!({"type": "object"}),
        }
    }

    fn base_params<'a>(tools: &'a [ToolDefinition]) -> SystemPromptParams<'a> {
        SystemPromptParams {
            run_id: "run-test",
            session_id: "session-test",
            model_provider: "anthropic",
            model_name: "claude-test",
            workspace_dir: "/tmp",
            tools,
            extra_system_prompt: None,
        }
    }

    #[test]
    fn identity_always_present() {
        let prompt = build(base_params(&[]));
        assert!(prompt.contains("You are Kelvin"));
    }

    #[test]
    fn safety_section_always_present() {
        let prompt = build(base_params(&[]));
        assert!(prompt.contains("Tool use policy:"));
        assert!(prompt.contains("\"granted\": true"));
        assert!(prompt.contains("256 characters"));
    }

    #[test]
    fn tools_section_omitted_when_empty() {
        let prompt = build(base_params(&[]));
        assert!(!prompt.contains("Available tools"));
    }

    #[test]
    fn tools_section_present_with_name_and_description() {
        let tools = vec![
            tool("fs_read", "Read a file from the workspace"),
            tool("web_fetch", "Fetch a URL"),
        ];
        let prompt = build(base_params(&tools));
        assert!(prompt.contains("Available tools:"));
        assert!(prompt.contains("- fs_read: Read a file from the workspace"));
        assert!(prompt.contains("- web_fetch: Fetch a URL"));
    }

    #[test]
    fn tool_with_empty_description_renders_without_colon() {
        let tools = vec![tool("noop", "")];
        let prompt = build(base_params(&tools));
        assert!(prompt.contains("- noop\n") || prompt.ends_with("- noop"));
        assert!(!prompt.contains("- noop:"));
    }

    #[test]
    fn workspace_always_present() {
        let prompt = build(SystemPromptParams {
            workspace_dir: "/home/user/project",
            ..base_params(&[])
        });
        assert!(prompt.contains("Working directory: /home/user/project"));
    }

    #[test]
    fn datetime_section_always_present() {
        let prompt = build(base_params(&[]));
        assert!(prompt.contains("Current date and time (UTC):"));
    }

    #[test]
    fn runtime_info_always_present() {
        let prompt = build(SystemPromptParams {
            run_id: "run-abc",
            session_id: "session-xyz",
            model_provider: "openai",
            model_name: "gpt-4.1",
            ..base_params(&[])
        });
        assert!(prompt.contains("run_id: run-abc"));
        assert!(prompt.contains("session_id: session-xyz"));
        assert!(prompt.contains("model: openai / gpt-4.1"));
    }

    #[test]
    fn extra_system_prompt_appended_when_provided() {
        let prompt = build(SystemPromptParams {
            extra_system_prompt: Some("Always respond in haiku."),
            ..base_params(&[])
        });
        assert!(prompt.contains("Always respond in haiku."));
    }

    #[test]
    fn extra_system_prompt_absent_when_none() {
        let prompt = build(base_params(&[]));
        assert!(!prompt.contains("None"));
    }

    #[test]
    fn sections_separated_by_double_newline_without_tools_or_extra() {
        // Identity, Safety, Workspace, DateTime, RuntimeInfo = 5 sections, 4 inter-section separators
        // Safety also contains 2 internal \n\n (after the field example and before the rules list)
        let prompt = build(base_params(&[]));
        assert_eq!(prompt.matches("\n\n").count(), 6);
    }

    #[test]
    #[ignore]
    fn realistic_full_prompt_snapshot() {
        let tools = vec![
            tool("fs_safe_read", "Read a file from the workspace (sandbox/, memory/, notes/)."),
            tool("fs_safe_write", "Write content to a file in the workspace. Only sandbox/, memory/, and notes/ roots are permitted. Requires sensitive approval."),
            tool("web_fetch_safe", "Fetch a URL over HTTP or HTTPS. Only hosts in the configured allowlist are permitted. Requires sensitive approval."),
            tool("schedule_cron", "Manage scheduled tasks. Requires sensitive approval for mutations."),
        ];
        let prompt = build(SystemPromptParams {
            run_id: "run-a1b2c3",
            session_id: "session-u7x9z2",
            model_provider: "anthropic",
            model_name: "claude-sonnet-4-6",
            workspace_dir: "./",
            tools: &tools,
            extra_system_prompt: Some("You are assisting a senior software engineer. Be concise and precise. Prefer code over prose."),
        });
        println!("\n{prompt}");
    }

    #[test]
    fn sections_separated_by_double_newline_with_tools_and_extra() {
        // Identity, Safety, Tools, Workspace, DateTime, RuntimeInfo, Extra = 7 sections, 6 inter-section separators
        // Safety also contains 2 internal \n\n
        let tools = vec![tool("fs_read", "reads files")];
        let prompt = build(SystemPromptParams {
            extra_system_prompt: Some("extra"),
            ..base_params(&tools)
        });
        assert_eq!(prompt.matches("\n\n").count(), 8);
    }
}

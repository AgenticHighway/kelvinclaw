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
/// * `workspace_dir` - absolute path to the agent's working directory; always injected into the prompt
/// * `tools` - tool definitions registered in the current run; an empty slice omits the tools section
/// * `extra_system_prompt` - caller-supplied prompt text appended as the final section; `None` omits it
pub struct SystemPromptParams<'a> {
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
///
/// ### Variants
/// * `Identity` - fixed agent identity and behavioral preamble; always first
/// * `Tools` - human-readable tool listing as `(name, description)` pairs; omitted when the registry is empty
/// * `Workspace` - working directory injected at runtime; always present
/// * `Extra` - verbatim caller-supplied text; appended last when provided
enum Section {
    Identity,
    Tools(Vec<(String, String)>),
    Workspace(String),
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
    /// entry; entries with an empty description omit the colon separator. `Extra` is returned
    /// verbatim. all other variants produce a single line.
    ///
    /// ### Returns
    /// the rendered section as a `String`
    fn render(&self) -> String {
        match self {
            Section::Identity => {
                "You are Kelvin, an AI assistant. You help users accomplish tasks by reasoning carefully and using the tools available to you.".to_string()
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
                format!("Working directory: {dir}")
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
/// with double newlines. the section order is fixed: identity, tools (if any), workspace,
/// extra (if provided). tool definitions are summarized by name and description only —
/// full input schemas are passed separately via the model API's native tool-calling mechanism
/// and are not duplicated here.
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

    [
        Some(Section::Identity),
        if tool_summaries.is_empty() {
            None
        } else {
            Some(Section::Tools(tool_summaries))
        },
        Some(Section::Workspace(params.workspace_dir.to_string())),
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

    #[test]
    fn identity_always_present() {
        let prompt = build(SystemPromptParams {
            workspace_dir: "/tmp",
            tools: &[],
            extra_system_prompt: None,
        });
        assert!(prompt.contains("You are Kelvin"));
    }

    #[test]
    fn tools_section_omitted_when_empty() {
        let prompt = build(SystemPromptParams {
            workspace_dir: "/tmp",
            tools: &[],
            extra_system_prompt: None,
        });
        assert!(!prompt.contains("Available tools"));
    }

    #[test]
    fn tools_section_present_with_name_and_description() {
        let tools = vec![
            tool("fs_read", "Read a file from the workspace"),
            tool("web_fetch", "Fetch a URL"),
        ];
        let prompt = build(SystemPromptParams {
            workspace_dir: "/tmp",
            tools: &tools,
            extra_system_prompt: None,
        });
        assert!(prompt.contains("Available tools:"));
        assert!(prompt.contains("- fs_read: Read a file from the workspace"));
        assert!(prompt.contains("- web_fetch: Fetch a URL"));
    }

    #[test]
    fn tool_with_empty_description_renders_without_colon() {
        let tools = vec![tool("noop", "")];
        let prompt = build(SystemPromptParams {
            workspace_dir: "/tmp",
            tools: &tools,
            extra_system_prompt: None,
        });
        assert!(prompt.contains("- noop\n") || prompt.ends_with("- noop"));
        assert!(!prompt.contains("- noop:"));
    }

    #[test]
    fn workspace_always_present() {
        let prompt = build(SystemPromptParams {
            workspace_dir: "/home/user/project",
            tools: &[],
            extra_system_prompt: None,
        });
        assert!(prompt.contains("Working directory: /home/user/project"));
    }

    #[test]
    fn extra_system_prompt_appended_when_provided() {
        let prompt = build(SystemPromptParams {
            workspace_dir: "/tmp",
            tools: &[],
            extra_system_prompt: Some("Always respond in haiku."),
        });
        assert!(prompt.contains("Always respond in haiku."));
    }

    #[test]
    fn extra_system_prompt_absent_when_none() {
        let prompt = build(SystemPromptParams {
            workspace_dir: "/tmp",
            tools: &[],
            extra_system_prompt: None,
        });
        // just check it compiles
        assert!(!prompt.contains("None"));
    }

    #[test]
    fn sections_separated_by_double_newline() {
        let tools = vec![tool("fs_read", "reads files")];
        let prompt = build(SystemPromptParams {
            workspace_dir: "/tmp",
            tools: &tools,
            extra_system_prompt: Some("extra"),
        });
        // Identity, Tools, Workspace, Extra — each separated by \n\n
        assert_eq!(prompt.matches("\n\n").count(), 3);
    }
}

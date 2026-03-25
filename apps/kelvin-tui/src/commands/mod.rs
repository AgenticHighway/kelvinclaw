use serde_json::Value;

/// A completion item shown in the autocomplete popup.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// The command name without leading slash, e.g. "tools".
    pub name: String,
    pub description: String,
    pub usage: Option<String>,
    #[allow(dead_code)] // reserved for grouped autocomplete UI
    pub category: String,
}

impl CompletionItem {
    /// Display label shown in the popup: "/name – description".
    #[allow(dead_code)] // reserved for autocomplete rendering
    pub fn label(&self) -> String {
        format!("/{} – {}", self.name, self.description)
    }
}

/// Commands that are handled entirely within the TUI, without a gateway call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalCommand {
    Quit,
    Clear,
    Help,
    New,
    Session,
}

/// A resolved command ready for dispatch.
#[derive(Debug, Clone)]
pub enum SlashCommand {
    Local(LocalCommand),
    Remote { name: String },
}

/// Metadata for a remote (gateway) command.
#[derive(Debug, Clone)]
pub struct RemoteCommandMeta {
    pub name: String,
    pub description: String,
    pub usage: Option<String>,
    pub category: String,
}

impl RemoteCommandMeta {
    pub fn from_json(value: &Value) -> Option<Self> {
        Some(RemoteCommandMeta {
            name: value.get("name")?.as_str()?.to_string(),
            description: value
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            usage: value
                .get("usage")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            category: value
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("system")
                .to_string(),
        })
    }
}

/// Merged registry of local + remote commands.
pub struct MergedCommandRegistry {
    local: Vec<(LocalCommand, CompletionItem)>,
    remote: Vec<RemoteCommandMeta>,
}

impl Default for MergedCommandRegistry {
    fn default() -> Self {
        let local = vec![
            (
                LocalCommand::Help,
                CompletionItem {
                    name: "help".to_string(),
                    description: "Show available commands".to_string(),
                    usage: None,
                    category: "system".to_string(),
                },
            ),
            (
                LocalCommand::Clear,
                CompletionItem {
                    name: "clear".to_string(),
                    description: "Clear session history and chat display".to_string(),
                    usage: None,
                    category: "system".to_string(),
                },
            ),
            (
                LocalCommand::New,
                CompletionItem {
                    name: "new".to_string(),
                    description: "Create a new session".to_string(),
                    usage: Some("[name]".to_string()),
                    category: "session".to_string(),
                },
            ),
            (
                LocalCommand::Session,
                CompletionItem {
                    name: "session".to_string(),
                    description: "List or switch sessions".to_string(),
                    usage: Some("[id]".to_string()),
                    category: "session".to_string(),
                },
            ),
            (
                LocalCommand::Quit,
                CompletionItem {
                    name: "quit".to_string(),
                    description: "Exit the TUI".to_string(),
                    usage: None,
                    category: "system".to_string(),
                },
            ),
        ];
        Self {
            local,
            remote: Vec::new(),
        }
    }
}

impl MergedCommandRegistry {
    /// Replace remote commands with newly fetched list from the gateway.
    /// Commands already registered as local are skipped to avoid duplicates.
    pub fn set_remote(&mut self, commands: &Value) {
        self.remote.clear();
        let local_names: std::collections::HashSet<&str> = self
            .local
            .iter()
            .map(|(_, item)| item.name.as_str())
            .collect();
        if let Some(arr) = commands.get("commands").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(meta) = RemoteCommandMeta::from_json(item) {
                    if !local_names.contains(meta.name.as_str()) {
                        self.remote.push(meta);
                    }
                }
            }
        }
    }

    /// Return all commands whose name starts with `prefix` (case-insensitive).
    pub fn completions(&self, prefix: &str) -> Vec<CompletionItem> {
        let prefix = prefix.to_ascii_lowercase();
        let mut items: Vec<CompletionItem> = self
            .local
            .iter()
            .filter(|(_, item)| item.name.starts_with(&prefix))
            .map(|(_, item)| item.clone())
            .collect();
        for r in &self.remote {
            if r.name.to_ascii_lowercase().starts_with(&prefix) {
                items.push(CompletionItem {
                    name: r.name.clone(),
                    description: r.description.clone(),
                    usage: r.usage.clone(),
                    category: r.category.clone(),
                });
            }
        }
        items
    }

    /// Resolve an exact command name to a dispatchable `SlashCommand`.
    pub fn resolve(&self, name: &str) -> Option<SlashCommand> {
        let lower = name.to_ascii_lowercase();
        if let Some((cmd, _)) = self.local.iter().find(|(_, item)| item.name == lower) {
            return Some(SlashCommand::Local(cmd.clone()));
        }
        if self
            .remote
            .iter()
            .any(|r| r.name.to_ascii_lowercase() == lower)
        {
            return Some(SlashCommand::Remote { name: lower });
        }
        None
    }

    /// Format a `/help` message listing all available commands.
    pub fn help_text(&self) -> String {
        let mut lines = vec!["Available commands:".to_string()];
        for (_, item) in &self.local {
            let usage = item
                .usage
                .as_deref()
                .map(|u| format!(" {u}"))
                .unwrap_or_default();
            lines.push(format!(
                "  • /{}{} — {}",
                item.name, usage, item.description
            ));
        }
        for r in &self.remote {
            let usage = r
                .usage
                .as_deref()
                .map(|u| format!(" {u}"))
                .unwrap_or_default();
            lines.push(format!("  • /{}{} — {}", r.name, usage, r.description));
        }
        lines.join("\n")
    }
}

/// Parse a slash-command input into (command_name, args_string).
/// Input must start with '/'. Returns None if it doesn't.
pub fn parse_slash_input(input: &str) -> Option<(String, String)> {
    let body = input.strip_prefix('/')?;
    let mut parts = body.splitn(2, char::is_whitespace);
    let name = parts.next()?.trim().to_ascii_lowercase();
    if name.is_empty() {
        return None;
    }
    let args = parts.next().unwrap_or("").trim().to_string();
    Some((name, args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn completions_filters_by_prefix() {
        let reg = MergedCommandRegistry::default();
        let items = reg.completions("q");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "quit");
    }

    #[test]
    fn completions_empty_prefix_returns_all_local() {
        let reg = MergedCommandRegistry::default();
        assert_eq!(reg.completions("").len(), 5); // help, clear, new, session, quit
    }

    #[test]
    fn resolve_local_quit() {
        let reg = MergedCommandRegistry::default();
        let cmd = reg.resolve("quit");
        assert!(matches!(cmd, Some(SlashCommand::Local(LocalCommand::Quit))));
    }

    #[test]
    fn resolve_local_new() {
        let reg = MergedCommandRegistry::default();
        let cmd = reg.resolve("new");
        assert!(matches!(cmd, Some(SlashCommand::Local(LocalCommand::New))));
    }

    #[test]
    fn resolve_local_session() {
        let reg = MergedCommandRegistry::default();
        let cmd = reg.resolve("session");
        assert!(matches!(
            cmd,
            Some(SlashCommand::Local(LocalCommand::Session))
        ));
    }

    #[test]
    fn resolve_unknown_returns_none() {
        let reg = MergedCommandRegistry::default();
        assert!(reg.resolve("nonexistent").is_none());
    }

    #[test]
    fn set_remote_populates_registry() {
        let mut reg = MergedCommandRegistry::default();
        reg.set_remote(&json!({
            "commands": [
                { "name": "tools", "description": "List tools", "category": "tools" }
            ]
        }));
        let items = reg.completions("to");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "tools");
    }

    #[test]
    fn resolve_remote_after_set_remote() {
        let mut reg = MergedCommandRegistry::default();
        reg.set_remote(&json!({
            "commands": [
                { "name": "tools", "description": "List tools", "category": "tools" }
            ]
        }));
        assert!(matches!(
            reg.resolve("tools"),
            Some(SlashCommand::Remote { name }) if name == "tools"
        ));
    }

    #[test]
    fn parse_slash_input_name_only() {
        let (name, args) = parse_slash_input("/tools").unwrap();
        assert_eq!(name, "tools");
        assert_eq!(args, "");
    }

    #[test]
    fn parse_slash_input_with_args() {
        let (name, args) = parse_slash_input("/model anthropic").unwrap();
        assert_eq!(name, "model");
        assert_eq!(args, "anthropic");
    }

    #[test]
    fn parse_slash_input_non_slash_returns_none() {
        assert!(parse_slash_input("hello").is_none());
    }
}

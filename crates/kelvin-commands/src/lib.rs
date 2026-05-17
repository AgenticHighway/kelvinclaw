use kelvin_core::{CommandSurface, SlashCommandMeta};
use serde_json::Value;

/// A completion item shown in the autocomplete popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub name: String,
    pub description: String,
    pub usage: Option<String>,
    #[allow(dead_code)]
    pub category: String,
}

impl CompletionItem {
    #[allow(dead_code)]
    pub fn label(&self) -> String {
        format!("/{} – {}", self.name, self.description)
    }
}

/// Commands handled entirely within the TUI without a gateway call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalCommand {
    Quit,
    Help,
}

/// A resolved command ready for dispatch.
#[derive(Debug, Clone)]
pub enum SlashCommand {
    Local(LocalCommand),
    Remote { name: String },
}

#[derive(Debug)]
struct LocalEntry {
    cmd: LocalCommand,
    item: CompletionItem,
}

/// Merged registry of local (TUI-only) and remote (gateway/plugin) commands.
///
/// Remote commands are stored as full `SlashCommandMeta` so that dispatch
/// sites can inspect `surfaces`, `min_tier`, and `bypass_queue`.
#[derive(Debug)]
pub struct CommandRegistry {
    local: Vec<LocalEntry>,
    remote: Vec<SlashCommandMeta>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self {
            local: vec![
                LocalEntry {
                    cmd: LocalCommand::Help,
                    item: CompletionItem {
                        name: "help".to_string(),
                        description: "Show available commands".to_string(),
                        usage: None,
                        category: "system".to_string(),
                    },
                },
                LocalEntry {
                    cmd: LocalCommand::Quit,
                    item: CompletionItem {
                        name: "quit".to_string(),
                        description: "Exit the TUI".to_string(),
                        usage: None,
                        category: "system".to_string(),
                    },
                },
            ],
            remote: Vec::new(),
        }
    }
}

impl CommandRegistry {
    /// Return a registry pre-seeded with the known gateway commands so that
    /// TUI slash commands work before the first `commands.list` response arrives.
    pub fn tui_default() -> Self {
        let mut r = Self::default();
        for meta in Self::tui_gateway_stubs() {
            r.remote.push(meta);
        }
        r
    }

    fn tui_gateway_stubs() -> Vec<SlashCommandMeta> {
        vec![
            SlashCommandMeta {
                name: "clear".to_string(),
                description: "Clear session history and chat display".to_string(),
                usage: None,
                category: "session".to_string(),
                surfaces: std::collections::HashSet::new(),
                min_tier: None,
                bypass_queue: false,
            },
            SlashCommandMeta {
                name: "new".to_string(),
                description: "Create a new session".to_string(),
                usage: Some("[name]".to_string()),
                category: "session".to_string(),
                surfaces: std::collections::HashSet::new(),
                min_tier: None,
                bypass_queue: false,
            },
            SlashCommandMeta {
                name: "session".to_string(),
                description: "List or switch sessions".to_string(),
                usage: Some("[id]".to_string()),
                category: "session".to_string(),
                surfaces: std::collections::HashSet::new(),
                min_tier: None,
                bypass_queue: false,
            },
        ]
    }

    /// Build a `CommandRegistry` from a list of `SlashCommandMeta`.
    ///
    /// When `surface` is provided, only commands that include that surface in
    /// their `surfaces` set are registered.  When `surface` is `None` all
    /// commands are included (useful for channel-dispatch registries where
    /// filtering happens at lookup time).
    pub fn from_slash_meta(commands: &[SlashCommandMeta], surface: Option<CommandSurface>) -> Self {
        let mut r = Self::default();
        for meta in commands {
            if let Some(s) = surface {
                if !meta.surfaces.contains(&s) {
                    continue;
                }
            }
            if !r.local_names().contains(meta.name.as_str()) {
                r.remote.push(meta.clone());
            }
        }
        r
    }

    fn local_names(&self) -> std::collections::HashSet<&str> {
        self.local.iter().map(|e| e.item.name.as_str()).collect()
    }

    /// Replace the remote command list with the response from `commands.list`.
    pub fn set_gateway_commands(&mut self, commands: &Value) {
        self.remote.clear();
        let local_names: std::collections::HashSet<String> =
            self.local.iter().map(|e| e.item.name.clone()).collect();
        if let Some(arr) = commands.get("commands").and_then(|v| v.as_array()) {
            for item in arr {
                let Some(name) = item.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                if local_names.contains(name) {
                    continue;
                }
                self.remote.push(SlashCommandMeta {
                    name: name.to_string(),
                    description: item
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    usage: item
                        .get("usage")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    category: item
                        .get("category")
                        .and_then(|v| v.as_str())
                        .unwrap_or("system")
                        .to_string(),
                    // Surfaces/tier/bypass_queue default to the safe values
                    // when the JSON omits them (older gateway versions).
                    surfaces: std::collections::HashSet::new(),
                    min_tier: None,
                    bypass_queue: false,
                });
            }
        }
    }

    /// Return the full `SlashCommandMeta` for a command by name, if present in
    /// the remote list.  Used by channel dispatch to check surfaces and tier.
    pub fn lookup_meta(&self, name: &str) -> Option<&SlashCommandMeta> {
        let lower = name.to_ascii_lowercase();
        self.remote.iter().find(|m| m.name == lower)
    }

    /// Return all commands whose name starts with `prefix` (case-insensitive).
    pub fn completions(&self, prefix: &str) -> Vec<CompletionItem> {
        let prefix = prefix.to_ascii_lowercase();
        let mut items: Vec<CompletionItem> = self
            .local
            .iter()
            .filter(|e| e.item.name.starts_with(&prefix))
            .map(|e| e.item.clone())
            .collect();
        for meta in &self.remote {
            if meta.name.to_ascii_lowercase().starts_with(&prefix) {
                items.push(CompletionItem {
                    name: meta.name.clone(),
                    description: meta.description.clone(),
                    usage: meta.usage.clone(),
                    category: meta.category.clone(),
                });
            }
        }
        items
    }

    /// Resolve an exact command name to a dispatchable `SlashCommand`.
    pub fn resolve(&self, name: &str) -> Option<SlashCommand> {
        let lower = name.to_ascii_lowercase();
        if let Some(entry) = self.local.iter().find(|e| e.item.name == lower) {
            return Some(SlashCommand::Local(entry.cmd.clone()));
        }
        if self.remote.iter().any(|m| m.name == lower) {
            return Some(SlashCommand::Remote { name: lower });
        }
        None
    }

    /// Format a `/help` listing of all available commands.
    pub fn help_text(&self) -> String {
        let mut lines = vec!["Available commands:".to_string()];
        for entry in &self.local {
            let usage = entry
                .item
                .usage
                .as_deref()
                .map(|u| format!(" {u}"))
                .unwrap_or_default();
            lines.push(format!(
                "  • /{}{} — {}",
                entry.item.name, usage, entry.item.description
            ));
        }
        for meta in &self.remote {
            let usage = meta
                .usage
                .as_deref()
                .map(|u| format!(" {u}"))
                .unwrap_or_default();
            lines.push(format!(
                "  • /{}{} — {}",
                meta.name, usage, meta.description
            ));
        }
        lines.join("\n")
    }
}

/// Parse a slash-command input into (command_name, args_string).
/// Returns `None` if the input does not start with `/`.
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
        let reg = CommandRegistry::default();
        let items = reg.completions("q");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "quit");
    }

    #[test]
    fn completions_empty_prefix_returns_all_local() {
        let reg = CommandRegistry::default();
        assert_eq!(reg.completions("").len(), 2); // help, quit
    }

    #[test]
    fn resolve_local_quit() {
        let reg = CommandRegistry::default();
        assert!(matches!(
            reg.resolve("quit"),
            Some(SlashCommand::Local(LocalCommand::Quit))
        ));
    }

    #[test]
    fn resolve_local_help() {
        let reg = CommandRegistry::default();
        assert!(matches!(
            reg.resolve("help"),
            Some(SlashCommand::Local(LocalCommand::Help))
        ));
    }

    #[test]
    fn resolve_unknown_returns_none() {
        let reg = CommandRegistry::default();
        assert!(reg.resolve("nonexistent").is_none());
    }

    #[test]
    fn set_gateway_commands_populates_registry() {
        let mut reg = CommandRegistry::default();
        reg.set_gateway_commands(&json!({
            "commands": [
                { "name": "tools", "description": "List tools", "category": "tools" }
            ]
        }));
        let items = reg.completions("to");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "tools");
    }

    #[test]
    fn resolve_remote_after_set_gateway_commands() {
        let mut reg = CommandRegistry::default();
        reg.set_gateway_commands(&json!({
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
    fn lookup_meta_returns_full_meta() {
        let mut reg = CommandRegistry::default();
        reg.set_gateway_commands(&json!({
            "commands": [
                { "name": "tools", "description": "List tools", "category": "tools" }
            ]
        }));
        let meta = reg.lookup_meta("tools").unwrap();
        assert_eq!(meta.name, "tools");
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

    #[test]
    fn tui_default_pre_seeds_gateway_stubs() {
        let reg = CommandRegistry::tui_default();
        assert!(reg.resolve("clear").is_some());
        assert!(reg.resolve("new").is_some());
        assert!(reg.resolve("session").is_some());
        assert!(reg.resolve("quit").is_some());
        assert!(reg.resolve("help").is_some());
    }
}

use std::path::Path;

use anyhow::{Context, Result};

pub const INTERFACE_MODE_ENV_VAR: &str = "KELVIN_INTERFACE_MODE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceMode {
    Cli,
    Tui,
}

impl InterfaceMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Tui => "tui",
        }
    }
}

pub fn normalize_interface_mode(value: &str) -> Option<InterfaceMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cli" => Some(InterfaceMode::Cli),
        "tui" => Some(InterfaceMode::Tui),
        _ => None,
    }
}

pub fn load_env_value(path: &Path, key: &str) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    for line in content.lines() {
        let stripped = line.split('#').next().unwrap_or("").trim();
        if stripped.is_empty() {
            continue;
        }

        let stripped = stripped.strip_prefix("export ").unwrap_or(stripped);
        let Some((found_key, raw_value)) = stripped.split_once('=') else {
            continue;
        };

        if found_key.trim() != key {
            continue;
        }

        return Ok(Some(strip_wrapping_quotes(raw_value.trim()).to_string()));
    }

    Ok(None)
}

pub fn save_env_value(path: &Path, key: &str, value: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let existing = if path.exists() {
        std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?
    } else {
        String::new()
    };

    let mut lines = Vec::new();
    let mut replaced = false;
    for line in existing.lines() {
        let stripped = line.trim_start();
        let stripped = stripped.strip_prefix("export ").unwrap_or(stripped);
        if let Some((found_key, _)) = stripped.split_once('=') {
            if found_key.trim() == key {
                lines.push(format!("{key}={value}"));
                replaced = true;
                continue;
            }
        }
        lines.push(line.to_string());
    }

    if !replaced {
        lines.push(format!("{key}={value}"));
    }

    let output = if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    };
    std::fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn save_interface_mode(path: &Path, mode: InterfaceMode) -> Result<()> {
    save_env_value(path, INTERFACE_MODE_ENV_VAR, mode.as_str())
}

fn strip_wrapping_quotes(value: &str) -> &str {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        return &value[1..value.len() - 1];
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file_path(label: &str) -> std::path::PathBuf {
        let base =
            std::env::temp_dir().join(format!("kelvin-cli-prefs-{}-{}", label, std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("create temp prefs dir");
        base.join("preferences.env")
    }

    #[test]
    fn normalize_interface_mode_accepts_cli_and_tui() {
        assert_eq!(normalize_interface_mode("cli"), Some(InterfaceMode::Cli));
        assert_eq!(normalize_interface_mode("TUI"), Some(InterfaceMode::Tui));
        assert_eq!(normalize_interface_mode("unknown"), None);
    }

    #[test]
    fn load_env_value_reads_saved_value() {
        let path = temp_file_path("load");
        std::fs::write(&path, "KELVIN_INTERFACE_MODE=tui\n").expect("write prefs");
        let value = load_env_value(&path, INTERFACE_MODE_ENV_VAR).expect("load prefs");
        assert_eq!(value.as_deref(), Some("tui"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_env_value_replaces_existing_key() {
        let path = temp_file_path("replace");
        std::fs::write(&path, "KELVIN_INTERFACE_MODE=cli\nOTHER_KEY=value\n").expect("write prefs");
        save_env_value(&path, INTERFACE_MODE_ENV_VAR, "tui").expect("save prefs");
        let content = std::fs::read_to_string(&path).expect("read prefs");
        assert!(content.contains("KELVIN_INTERFACE_MODE=tui"));
        assert!(content.contains("OTHER_KEY=value"));
        let _ = std::fs::remove_file(&path);
    }
}

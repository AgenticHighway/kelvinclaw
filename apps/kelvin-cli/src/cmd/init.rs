use anyhow::{Context, Result};

use crate::cli::InitArgs;
use crate::paths;

pub fn run(args: InitArgs) -> Result<()> {
    let dot_env = paths::dotenv_path();
    let home = paths::kelvin_home();

    if dot_env.exists() && !args.force {
        if !crate::tty::is_interactive() {
            anyhow::bail!(
                "{} already exists. Use --force to overwrite.",
                dot_env.display()
            );
        }
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!("{} already exists. Overwrite?", dot_env.display()))
            .default(false)
            .interact()
            .context("prompt failed")?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    std::fs::create_dir_all(&home)
        .with_context(|| format!("failed to create {}", home.display()))?;

    // Write permissive trust policy so plugins load before the user runs `kelvin start`.
    super::start::ensure_trust_policy()?;

    // Generate gateway token.
    let token_bytes: [u8; 32] = rand::random();
    let gateway_token = hex::encode(token_bytes);

    // Generate memory keys.
    crate::keys::ensure_memory_keys(&home)?;
    let pub_key_path = paths::memory_public_key_path();
    let priv_key_path = paths::memory_private_key_path();

    // Provider selection.
    let (provider_id, api_key_entry, extra_env) = select_provider(args.provider.as_deref())?;

    // Build .env content.
    let plugin_index_url = std::env::var("KELVIN_PLUGIN_INDEX_URL").unwrap_or_else(|_| {
        "https://raw.githubusercontent.com/AgenticHighway/kelvinclaw-plugins/main/index.json"
            .to_string()
    });

    let mut env_lines = vec![
        format!("KELVIN_GATEWAY_TOKEN={}", gateway_token),
        format!("KELVIN_MODEL_PROVIDER={}", provider_id),
        format!("KELVIN_MEMORY_PUBLIC_KEY_PATH={}", pub_key_path.display()),
        format!("KELVIN_MEMORY_PRIVATE_KEY_PATH={}", priv_key_path.display()),
        format!("KELVIN_PLUGIN_INDEX_URL={}", plugin_index_url),
    ];

    if let Some((key_name, key_value)) = api_key_entry {
        env_lines.push(format!("{}={}", key_name, key_value));
    }

    for (key_name, key_value) in extra_env {
        env_lines.push(format!("{}={}", key_name, key_value));
    }

    let env_content = env_lines.join("\n") + "\n";
    std::fs::write(&dot_env, env_content)
        .with_context(|| format!("failed to write {}", dot_env.display()))?;

    println!("\n[kelvin] configuration written to: {}", dot_env.display());

    // Shell completions (opt-in).
    let install_completions = if args.with_completions {
        true
    } else if crate::tty::is_interactive() {
        dialoguer::Confirm::new()
            .with_prompt("Install shell completions?")
            .default(false)
            .interact()
            .unwrap_or(false)
    } else {
        false
    };

    if install_completions {
        if let Err(e) = super::completions::install_for_current_shell() {
            eprintln!("[kelvin] warning: could not install completions: {}", e);
        }
    }

    println!("\nNext steps: run `kelvin` to start.");
    Ok(())
}

/// Returns `(provider_id, api_key_entry, extra_env_pairs)`.
///
/// `provider_hint` is the value of `--provider` if supplied. Accepted values
/// (case-insensitive, with or without the `kelvin.` prefix):
///   echo, openai, anthropic, openrouter, ollama
fn select_provider(
    provider_hint: Option<&str>,
) -> Result<(String, Option<(String, String)>, Vec<(String, String)>)> {
    // If a provider was given on the CLI, resolve it without prompting.
    if let Some(hint) = provider_hint {
        return resolve_provider_hint(hint);
    }

    if !crate::tty::is_interactive() {
        // Non-interactive with no --provider: default to echo.
        return Ok(("kelvin.echo".to_string(), None, vec![]));
    }

    let choices = &[
        "Echo (no API key — good for testing)",
        "OpenAI",
        "Anthropic",
        "OpenRouter",
        "Ollama (local — ensure `ollama serve` is running)",
    ];

    let selection = dialoguer::Select::new()
        .with_prompt("Select model provider")
        .items(choices)
        .default(0)
        .interact()
        .context("provider selection failed")?;

    match selection {
        0 => Ok(("kelvin.echo".to_string(), None, vec![])),
        1 => {
            let key = prompt_api_key("OPENAI_API_KEY")?;
            Ok(("kelvin.openai".to_string(), Some(("OPENAI_API_KEY".to_string(), key)), vec![]))
        }
        2 => {
            let key = prompt_api_key("ANTHROPIC_API_KEY")?;
            Ok(("kelvin.anthropic".to_string(), Some(("ANTHROPIC_API_KEY".to_string(), key)), vec![]))
        }
        3 => {
            let key = prompt_api_key("OPENROUTER_API_KEY")?;
            Ok(("kelvin.openrouter".to_string(), Some(("OPENROUTER_API_KEY".to_string(), key)), vec![]))
        }
        4 => {
            println!("Note: ensure `ollama serve` is running before starting kelvin.");
            let base_url = prompt_ollama_base_url()?;
            Ok(("kelvin.ollama".to_string(), None, vec![("OLLAMA_BASE_URL".to_string(), base_url)]))
        }
        _ => unreachable!(),
    }
}

/// Resolve a `--provider` flag value to `(provider_id, api_key_entry, extra_env)`.
fn resolve_provider_hint(
    hint: &str,
) -> Result<(String, Option<(String, String)>, Vec<(String, String)>)> {
    let normalized = hint.trim_start_matches("kelvin.").to_ascii_lowercase();
    match normalized.as_str() {
        "echo" => Ok(("kelvin.echo".to_string(), None, vec![])),
        "openai" => {
            let key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
            if key.is_empty() {
                anyhow::bail!("--provider openai requires OPENAI_API_KEY to be set in the environment");
            }
            Ok(("kelvin.openai".to_string(), Some(("OPENAI_API_KEY".to_string(), key)), vec![]))
        }
        "anthropic" => {
            let key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
            if key.is_empty() {
                anyhow::bail!("--provider anthropic requires ANTHROPIC_API_KEY to be set in the environment");
            }
            Ok(("kelvin.anthropic".to_string(), Some(("ANTHROPIC_API_KEY".to_string(), key)), vec![]))
        }
        "openrouter" => {
            let key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
            if key.is_empty() {
                anyhow::bail!("--provider openrouter requires OPENROUTER_API_KEY to be set in the environment");
            }
            Ok(("kelvin.openrouter".to_string(), Some(("OPENROUTER_API_KEY".to_string(), key)), vec![]))
        }
        "ollama" => {
            let base_url = std::env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string());
            Ok(("kelvin.ollama".to_string(), None, vec![("OLLAMA_BASE_URL".to_string(), base_url)]))
        }
        other => anyhow::bail!(
            "unknown provider {:?}. Valid values: echo, openai, anthropic, openrouter, ollama",
            other
        ),
    }
}

fn prompt_ollama_base_url() -> Result<String> {
    let default = std::env::var("OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    if !crate::tty::is_interactive() {
        return Ok(default);
    }
    let input = dialoguer::Input::<String>::new()
        .with_prompt("Ollama base URL")
        .default(default)
        .interact_text()
        .context("failed to read OLLAMA_BASE_URL")?;
    Ok(input)
}

fn prompt_api_key(name: &str) -> Result<String> {
    rpassword::prompt_password(format!("Paste {} (input hidden): ", name))
        .with_context(|| format!("failed to read {}", name))
}

// hex encoding without an extra dep
mod hex {
    pub fn encode(bytes: [u8; 32]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

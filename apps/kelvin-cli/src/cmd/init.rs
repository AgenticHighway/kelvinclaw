use anyhow::{Context, Result};

use crate::cli::InitArgs;
use crate::paths;

pub fn run(args: InitArgs) -> Result<()> {
    let dot_env = paths::dotenv_path();
    let home = paths::kelvin_home();

    if dot_env.exists() {
        if !crate::tty::is_interactive() {
            anyhow::bail!(
                "{} already exists. Remove it first or run interactively to confirm overwrite.",
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
    let (provider_id, api_key_entry) = select_provider()?;

    // Build .env content.
    let mut env_lines = vec![
        format!("KELVIN_GATEWAY_TOKEN={}", gateway_token),
        format!("KELVIN_MODEL_PROVIDER={}", provider_id),
        format!("KELVIN_MEMORY_PUBLIC_KEY_PATH={}", pub_key_path.display()),
        format!("KELVIN_MEMORY_PRIVATE_KEY_PATH={}", priv_key_path.display()),
    ];

    if let Some((key_name, key_value)) = api_key_entry {
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

fn select_provider() -> Result<(String, Option<(String, String)>)> {
    if !crate::tty::is_interactive() {
        // Non-interactive: default to echo.
        return Ok(("kelvin.echo".to_string(), None));
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
        0 => Ok(("kelvin.echo".to_string(), None)),
        1 => {
            let key = prompt_api_key("OPENAI_API_KEY")?;
            Ok((
                "kelvin.openai".to_string(),
                Some(("OPENAI_API_KEY".to_string(), key)),
            ))
        }
        2 => {
            let key = prompt_api_key("ANTHROPIC_API_KEY")?;
            Ok((
                "kelvin.anthropic".to_string(),
                Some(("ANTHROPIC_API_KEY".to_string(), key)),
            ))
        }
        3 => {
            let key = prompt_api_key("OPENROUTER_API_KEY")?;
            Ok((
                "kelvin.openrouter".to_string(),
                Some(("OPENROUTER_API_KEY".to_string(), key)),
            ))
        }
        4 => {
            println!("Note: ensure `ollama serve` is running before starting kelvin.");
            Ok(("kelvin.ollama".to_string(), None))
        }
        _ => unreachable!(),
    }
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

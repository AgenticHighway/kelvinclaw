use anyhow::{Context, Result};

use crate::cli::StartArgs;
use crate::paths;
use crate::proc;

pub fn run(args: StartArgs) -> Result<()> {
    ensure_config()?;
    ensure_trust_policy()?;
    ensure_plugin()?;

    if !args.no_memory {
        start_memory_daemon()?;
    }
    start_gateway_daemon()?;

    println!("[kelvin] stack started. run `kelvin tui` to open the terminal UI.");
    println!("[kelvin] run `kelvin stop` to shut everything down.");
    Ok(())
}

/// Ensures the .env exists; directs the user to `kelvin init` if not.
pub fn ensure_config() -> Result<()> {
    let dot_env = paths::dotenv_path();
    if !dot_env.exists() {
        anyhow::bail!(
            "no config found at {}\nRun `kelvin init` to set up, or `kelvin medkit` to diagnose.",
            dot_env.display()
        );
    }
    Ok(())
}

/// Ensures the trust policy file exists; writes a permissive default if missing.
pub fn ensure_trust_policy() -> Result<()> {
    let trust_path = paths::trust_policy_path();
    if !trust_path.exists() {
        if let Some(parent) = trust_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &trust_path,
            r#"{"require_signature":false,"publishers":[]}"#,
        )
        .with_context(|| format!("failed to write trust policy to {}", trust_path.display()))?;
        println!("[kelvin] wrote permissive trust policy: {}", trust_path.display());
    }
    Ok(())
}

/// Ensures the required model provider plugin is installed.
pub fn ensure_plugin() -> Result<()> {
    let provider =
        std::env::var("KELVIN_MODEL_PROVIDER").unwrap_or_else(|_| "kelvin.echo".to_string());
    // Echo doesn't need a plugin.
    if provider == "kelvin.echo" {
        return Ok(());
    }

    let plugin_home = paths::plugin_home();
    let current = plugin_home.join(&provider).join("current");
    if current.exists() {
        return Ok(());
    }

    // Attempt to install it.
    let index_url = std::env::var("KELVIN_PLUGIN_INDEX_URL").unwrap_or_else(|_| {
        "https://raw.githubusercontent.com/AgenticHighway/kelvinclaw-plugins/main/index.json"
            .to_string()
    });

    println!("[kelvin] installing model provider: {}", provider);
    std::fs::create_dir_all(&plugin_home)?;
    super::plugin_ops::install_from_index(&provider, None, &plugin_home, &index_url, false)
        .with_context(|| {
            format!(
                "failed to install model provider plugin '{}'. \
                Set KELVIN_PLUGIN_INDEX_URL or choose a different KELVIN_MODEL_PROVIDER.",
                provider
            )
        })
}

pub fn start_memory_daemon() -> Result<()> {
    use crate::cmd::memory::{cmd_start, memory_binary};
    use crate::cli::MemoryStartArgs;

    if !memory_binary().exists() {
        eprintln!(
            "[kelvin] warning: kelvin-memory-controller not found, skipping memory controller"
        );
        return Ok(());
    }

    let pid_file = paths::memory_pid_path();
    if let Some(pid) = proc::read_pid_file(&pid_file) {
        if proc::is_running(pid) {
            println!("[kelvin] memory controller already running (pid={})", pid);
            return Ok(());
        }
    }

    cmd_start(MemoryStartArgs { foreground: false })
}

pub fn start_gateway_daemon() -> Result<()> {
    use crate::cli::GatewayStartArgs;
    use crate::cmd::gateway::run;
    use crate::cli::GatewayCmd;

    let home = paths::kelvin_home();
    crate::keys::ensure_memory_keys(&home)?;

    run(GatewayCmd::Start(GatewayStartArgs {
        foreground: false,
        gateway_args: vec![],
    }))
}

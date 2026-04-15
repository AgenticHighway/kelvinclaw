/// Bare `kelvin` invocation: choose CLI chat vs TUI, remember that preference,
/// then launch the selected experience.
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::cmd::start;
use crate::paths;
use crate::prefs::{self, InterfaceMode};

const GATEWAY_URL: &str = "ws://127.0.0.1:34617";
const READY_POLL_MS: u64 = 500;
const READY_MAX_MS: u64 = 10_000;

pub async fn run() -> Result<()> {
    start::ensure_config()?;

    match select_launch_interface_mode()? {
        InterfaceMode::Cli => launch_cli_mode(),
        InterfaceMode::Tui => launch_tui_mode().await,
    }
}

pub fn run_shell_help() -> Result<()> {
    println!("{}", shell_help_banner());
    Ok(())
}

fn shell_help_banner() -> String {
    let provider = configured_model_provider()
        .unwrap_or_else(|| "echo (no model provider configured)".to_string());
    let mut banner = format!(
        "Kelvin interactive quickstart (provider={})\n\n\
Try asking:\n\
  - What can you help me do here?\n\
  - Summarize the files in this folder\n\
  - Help me make a small change safely\n\
\n\
Commands:\n\
  /help  show this guide again\n\
  /exit  quit\n\
\n\
Run `kelvin` and choose CLI chat to start interactive mode.",
        provider
    );
    if provider == "echo (no model provider configured)" {
        banner.push_str(
            "\n\nTip: echo mode is useful for smoke tests. Configure a real model provider with `kelvin init` before relying on non-echo responses.",
        );
    }
    banner
}

fn select_launch_interface_mode() -> Result<InterfaceMode> {
    if let Ok(value) = std::env::var(prefs::INTERFACE_MODE_ENV_VAR) {
        if let Some(mode) = prefs::normalize_interface_mode(&value) {
            return Ok(mode);
        }
        eprintln!(
            "[kelvin] warning: ignoring invalid {}='{}' (expected cli or tui)",
            prefs::INTERFACE_MODE_ENV_VAR,
            value
        );
    }

    let preferences_path = paths::preferences_path();
    if let Some(saved_value) =
        prefs::load_env_value(&preferences_path, prefs::INTERFACE_MODE_ENV_VAR)?
    {
        if let Some(mode) = prefs::normalize_interface_mode(&saved_value) {
            return Ok(mode);
        }
        eprintln!(
            "[kelvin] warning: ignoring invalid saved interface mode '{}' in {}",
            saved_value,
            preferences_path.display()
        );
    }

    if !crate::tty::is_interactive() {
        return Ok(InterfaceMode::Tui);
    }

    let selection = dialoguer::Select::new()
        .with_prompt("Choose how to use Kelvin")
        .items(&[
            "CLI chat (Recommended)",
            "TUI app (Starts gateway + opens terminal UI)",
        ])
        .default(0)
        .interact()
        .context("failed to choose Kelvin interface")?;
    let mode = match selection {
        0 => InterfaceMode::Cli,
        1 => InterfaceMode::Tui,
        _ => unreachable!(),
    };

    prefs::save_interface_mode(&preferences_path, mode)?;
    println!(
        "[kelvin] saved interface preference ({}) in {}",
        mode.as_str(),
        preferences_path.display()
    );
    Ok(mode)
}

fn launch_cli_mode() -> Result<()> {
    start::ensure_trust_policy()?;
    start::ensure_cli_plugin()?;
    start::ensure_plugin()?;

    let host_bin = paths::binary_dir().join(format!("kelvin-host{}", std::env::consts::EXE_SUFFIX));
    if !host_bin.exists() {
        bail!("kelvin-host binary not found at {}", host_bin.display());
    }

    let workspace_dir = std::env::current_dir().context("failed to determine current directory")?;
    let state_dir = paths::state_dir();
    let model_provider = configured_model_provider();
    let args = host_args(&workspace_dir, &state_dir, model_provider.as_deref());

    exec_host(&host_bin, &args);
}

async fn launch_tui_mode() -> Result<()> {
    start::ensure_trust_policy()?;
    start::ensure_plugin()?;

    // Start memory controller.
    start::start_memory_daemon()?;

    // Start gateway.
    start::start_gateway_daemon()?;

    // Poll for gateway readiness.
    println!("[kelvin] waiting for gateway...");
    let gateway_ready = poll_gateway_ready().await;

    if !gateway_ready {
        eprintln!(
            "\n[kelvin] ERROR: gateway did not become ready within {}s.",
            READY_MAX_MS / 1000
        );
        eprintln!("[kelvin] stopping daemons...");
        crate::cmd::stop::run().ok();
        eprintln!(
            "[kelvin] run `kelvin medkit` to diagnose, or check the log at: {}",
            paths::gateway_log_path().display()
        );
        std::process::exit(1);
    }

    let tui_bin = paths::binary_dir().join(format!("kelvin-tui{}", std::env::consts::EXE_SUFFIX));
    if !tui_bin.exists() {
        eprintln!(
            "[kelvin] ERROR: kelvin-tui binary not found at {}",
            tui_bin.display()
        );
        eprintln!("[kelvin] stopping daemons...");
        crate::cmd::stop::run().ok();
        std::process::exit(1);
    }

    let gateway_url =
        std::env::var("KELVIN_GATEWAY_URL").unwrap_or_else(|_| GATEWAY_URL.to_string());
    let mut tui_args = vec!["--gateway-url".to_string(), gateway_url];

    if let Ok(token) = std::env::var("KELVIN_GATEWAY_TOKEN") {
        if !token.is_empty() {
            tui_args.push("--auth-token".to_string());
            tui_args.push(token);
        }
    }

    exec_tui(&tui_bin, &tui_args);
}

fn configured_model_provider() -> Option<String> {
    std::env::var("KELVIN_MODEL_PROVIDER")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn host_args(workspace_dir: &Path, state_dir: &Path, model_provider: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--interactive".to_string(),
        "--workspace".to_string(),
        workspace_dir.display().to_string(),
        "--state-dir".to_string(),
        state_dir.display().to_string(),
    ];
    if let Some(model_provider) = model_provider {
        args.push("--model-provider".to_string());
        args.push(model_provider.to_string());
    }
    args
}

async fn poll_gateway_ready() -> bool {
    use tokio_tungstenite::connect_async;

    let gateway_url =
        std::env::var("KELVIN_GATEWAY_URL").unwrap_or_else(|_| GATEWAY_URL.to_string());

    let steps = READY_MAX_MS / READY_POLL_MS;
    for _ in 0..steps {
        tokio::time::sleep(tokio::time::Duration::from_millis(READY_POLL_MS)).await;
        if connect_async(&gateway_url).await.is_ok() {
            return true;
        }
    }
    false
}

#[cfg(unix)]
fn exec_host(bin: &Path, args: &[String]) -> ! {
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(bin).args(args).exec();
    eprintln!("[kelvin] ERROR: failed to exec kelvin-host: {}", err);
    std::process::exit(1);
}

#[cfg(not(unix))]
fn exec_host(bin: &Path, args: &[String]) -> ! {
    match std::process::Command::new(bin).args(args).status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("[kelvin] ERROR: failed to launch kelvin-host: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(unix)]
fn exec_tui(bin: &Path, args: &[String]) -> ! {
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(bin).args(args).exec();
    eprintln!("[kelvin] ERROR: failed to exec kelvin-tui: {}", err);
    eprintln!("[kelvin] stopping daemons...");
    crate::cmd::stop::run().ok();
    std::process::exit(1);
}

#[cfg(not(unix))]
fn exec_tui(bin: &Path, args: &[String]) -> ! {
    let result = std::process::Command::new(bin).args(args).status();
    match result {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("[kelvin] ERROR: failed to launch kelvin-tui: {}", e);
            crate::cmd::stop::run().ok();
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_args_include_provider_when_configured() {
        let args = host_args(
            Path::new("/tmp/workspace"),
            Path::new("/tmp/state"),
            Some("kelvin.openai"),
        );
        assert_eq!(
            args,
            vec![
                "--interactive",
                "--workspace",
                "/tmp/workspace",
                "--state-dir",
                "/tmp/state",
                "--model-provider",
                "kelvin.openai",
            ]
        );
    }

    #[test]
    fn host_args_omit_provider_when_not_configured() {
        let args = host_args(Path::new("/tmp/workspace"), Path::new("/tmp/state"), None);
        assert_eq!(
            args,
            vec![
                "--interactive",
                "--workspace",
                "/tmp/workspace",
                "--state-dir",
                "/tmp/state",
            ]
        );
    }

    #[test]
    fn shell_help_banner_mentions_cli_chat() {
        let banner = shell_help_banner();
        assert!(banner.contains("Kelvin interactive quickstart"));
        assert!(banner.contains("choose CLI chat"));
        assert!(banner.contains("/help"));
    }
}

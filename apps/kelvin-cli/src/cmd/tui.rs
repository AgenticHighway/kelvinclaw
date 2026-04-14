use anyhow::{bail, Result};

use crate::cli::TuiArgs;
use crate::paths;
use crate::proc;

const DEFAULT_GATEWAY_URL: &str = "ws://127.0.0.1:34617";

pub fn run(args: TuiArgs) -> Result<()> {
    // Hard-fail if gateway isn't running.
    let pid_file = paths::gateway_pid_path();
    let gateway_up = proc::read_pid_file(&pid_file)
        .map(proc::is_running)
        .unwrap_or(false);

    if !gateway_up {
        bail!(
            "gateway is not running.\nRun `kelvin start` to start the full stack, or `kelvin gateway start` to start just the gateway."
        );
    }

    let tui_bin = paths::binary_dir().join("kelvin-tui");
    if !tui_bin.exists() {
        bail!(
            "kelvin-tui binary not found at {}\nRun `cargo build -p kelvin-tui` first.",
            tui_bin.display()
        );
    }

    let gateway_url = args
        .gateway_url
        .unwrap_or_else(|| {
            std::env::var("KELVIN_GATEWAY_URL").unwrap_or_else(|_| DEFAULT_GATEWAY_URL.to_string())
        });

    let mut cmd_args = vec!["--gateway-url".to_string(), gateway_url];

    if let Ok(token) = std::env::var("KELVIN_GATEWAY_TOKEN") {
        if !token.is_empty() {
            cmd_args.push("--auth-token".to_string());
            cmd_args.push(token);
        }
    }

    if let Some(session) = args.session {
        cmd_args.push("--session".to_string());
        cmd_args.push(session);
    }

    // Replace current process with the TUI (exec-style).
    exec_replace(&tui_bin, &cmd_args)
}

#[cfg(unix)]
fn exec_replace(bin: &std::path::Path, args: &[String]) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(bin).args(args).exec();
    Err(err.into())
}

#[cfg(not(unix))]
fn exec_replace(bin: &std::path::Path, args: &[String]) -> Result<()> {
    let status = std::process::Command::new(bin).args(args).status()?;
    std::process::exit(status.code().unwrap_or(1));
}

use anyhow::{bail, Result};

use crate::paths;
use crate::proc;

pub fn run() -> Result<()> {
    // Hard-fail if gateway isn't running.
    let pid_file = paths::gateway_pid_path();
    let gateway_pid = proc::read_pid_file(&pid_file);
    let gateway_up = gateway_pid.map(proc::is_running).unwrap_or(false);

    if !gateway_up {
        bail!("gateway is not running.\nRun `kelvin start` to start the full stack.");
    }

    let gateway_bin =
        paths::binary_dir().join(format!("kelvin-gateway{}", std::env::consts::EXE_SUFFIX));
    if !gateway_bin.exists() {
        bail!(
            "kelvin-gateway binary not found at {}",
            gateway_bin.display()
        );
    }

    let endpoint = std::env::var("KELVIN_GATEWAY_BIND")
        .map(|bind| {
            if bind.starts_with("ws://") || bind.starts_with("wss://") {
                bind
            } else {
                format!("ws://{}", bind)
            }
        })
        .unwrap_or_else(|_| "ws://127.0.0.1:34617".to_string());

    let mut cmd_args = vec!["--doctor".to_string(), "--endpoint".to_string(), endpoint];

    if let Ok(token) = std::env::var("KELVIN_GATEWAY_TOKEN") {
        if !token.is_empty() {
            cmd_args.push("--token".to_string());
            cmd_args.push(token);
        }
    }

    let status = std::process::Command::new(&gateway_bin)
        .args(&cmd_args)
        .status()?;

    std::process::exit(status.code().unwrap_or(1));
}

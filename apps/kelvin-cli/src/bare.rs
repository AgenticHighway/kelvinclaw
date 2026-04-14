/// Bare `kelvin` invocation (#104): start full stack then exec into TUI.
///
/// Flow:
/// 1. Check ~/.kelvinclaw/.env exists; if not, print guidance and exit 1.
/// 2. Ensure trust policy exists.
/// 3. Ensure model provider plugin is installed.
/// 4. Start memory controller and gateway daemons.
/// 5. Poll gateway readiness (up to 10 seconds).
/// 6. exec(kelvin-tui). If exec fails: stop daemons, print loud error, exit 1.
use anyhow::{bail, Result};

use crate::cmd::start;
use crate::paths;
use crate::proc;

const GATEWAY_URL: &str = "ws://127.0.0.1:34617";
const READY_POLL_MS: u64 = 500;
const READY_MAX_MS: u64 = 10_000;

pub async fn run() -> Result<()> {
    // Check config exists.
    let dot_env = paths::dotenv_path();
    if !dot_env.exists() {
        eprintln!(
            "no config found — run `kelvin init` to set up, or `kelvin medkit` to diagnose"
        );
        std::process::exit(1);
    }

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
        // Gateway didn't come up — stop what we started and fail loudly.
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

    // exec into kelvin-tui.
    let tui_bin = paths::binary_dir().join("kelvin-tui");
    if !tui_bin.exists() {
        eprintln!("[kelvin] ERROR: kelvin-tui binary not found at {}", tui_bin.display());
        eprintln!("[kelvin] stopping daemons...");
        crate::cmd::stop::run().ok();
        std::process::exit(1);
    }

    let gateway_url = std::env::var("KELVIN_GATEWAY_URL")
        .unwrap_or_else(|_| GATEWAY_URL.to_string());
    let mut tui_args = vec!["--gateway-url".to_string(), gateway_url];

    if let Ok(token) = std::env::var("KELVIN_GATEWAY_TOKEN") {
        if !token.is_empty() {
            tui_args.push("--auth-token".to_string());
            tui_args.push(token);
        }
    }

    exec_tui(&tui_bin, &tui_args);
}

async fn poll_gateway_ready() -> bool {
    use tokio_tungstenite::connect_async;

    let gateway_url = std::env::var("KELVIN_GATEWAY_URL")
        .unwrap_or_else(|_| GATEWAY_URL.to_string());

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
fn exec_tui(bin: &std::path::Path, args: &[String]) -> ! {
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(bin).args(args).exec();
    eprintln!("[kelvin] ERROR: failed to exec kelvin-tui: {}", err);
    eprintln!("[kelvin] stopping daemons...");
    crate::cmd::stop::run().ok();
    std::process::exit(1);
}

#[cfg(not(unix))]
fn exec_tui(bin: &std::path::Path, args: &[String]) -> ! {
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

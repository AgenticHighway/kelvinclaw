use anyhow::{bail, Result};

use crate::cli::{GatewayCmd, GatewayStartArgs};
use crate::paths;
use crate::proc;

const GRACE_MS: u64 = 3000;

pub fn run(sub: GatewayCmd) -> Result<()> {
    match sub {
        GatewayCmd::Start(args) => cmd_start(args),
        GatewayCmd::Stop => cmd_stop(),
        GatewayCmd::Restart(args) => cmd_restart(args),
        GatewayCmd::Status => cmd_status(),
        GatewayCmd::ApprovePairing { code } => cmd_approve_pairing(&code),
    }
}

fn gateway_binary() -> std::path::PathBuf {
    paths::binary_dir().join(format!("kelvin-gateway{}", std::env::consts::EXE_SUFFIX))
}

fn cmd_start(args: GatewayStartArgs) -> Result<()> {
    let bin = gateway_binary();
    if !bin.exists() {
        bail!(
            "kelvin-gateway binary not found at {}\nRun `cargo build -p kelvin-gateway` first.",
            bin.display()
        );
    }

    // Ensure required files/dirs exist (mirrors what kelvin-gateway.sh::ensure_trust_policy did).
    std::fs::create_dir_all(paths::plugin_home())?;
    crate::cmd::start::ensure_trust_policy()?;

    let state_dir = paths::state_dir();
    std::fs::create_dir_all(&state_dir)?;

    let model_provider =
        std::env::var("KELVIN_MODEL_PROVIDER").unwrap_or_else(|_| "kelvin.echo".to_string());

    let mut gateway_args = vec![
        "--model-provider".to_string(),
        model_provider,
        "--state-dir".to_string(),
        state_dir.to_string_lossy().to_string(),
    ];

    if let Ok(token) = std::env::var("KELVIN_GATEWAY_TOKEN") {
        if !token.is_empty() {
            gateway_args.push("--token".to_string());
            gateway_args.push(token);
        }
    }

    gateway_args.extend(args.gateway_args.clone());

    // Inject tilde-expanded paths so the gateway subprocess never sees a literal
    // `~` inherited from a dotenv file — the OS does not expand tildes in env vars.
    let resolved_env = resolved_path_env(&state_dir);

    if args.foreground {
        let status = std::process::Command::new(&bin)
            .args(&gateway_args)
            .envs(resolved_env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .status()?;
        std::process::exit(status.code().unwrap_or(1));
    }

    let pid_file = paths::gateway_pid_path();
    if let Some(existing_pid) = proc::read_pid_file(&pid_file) {
        if proc::is_running(existing_pid) {
            bail!(
                "gateway is already running (pid={})\nlog: {}",
                existing_pid,
                paths::gateway_log_path().display()
            );
        }
        eprintln!(
            "[kelvin-gateway] removing stale PID file (pid={})",
            existing_pid
        );
        let _ = std::fs::remove_file(&pid_file);
    }

    std::fs::create_dir_all(paths::log_dir())?;
    let log = paths::gateway_log_path();
    let pid = proc::spawn_detached(&bin, &gateway_args, &resolved_env, &log, &pid_file)?;
    println!("[kelvin-gateway] started (pid={})", pid);
    println!("[kelvin-gateway] log: {}", log.display());
    println!("[kelvin-gateway] pid: {}", pid_file.display());
    Ok(())
}

fn cmd_stop() -> Result<()> {
    let pid_file = paths::gateway_pid_path();
    let Some(pid) = proc::read_pid_file(&pid_file) else {
        bail!("gateway is not running (no PID file)");
    };

    if !proc::is_running(pid) {
        eprintln!(
            "[kelvin-gateway] not running (stale PID {}); removing PID file",
            pid
        );
        let _ = std::fs::remove_file(&pid_file);
        return Ok(());
    }

    println!("[kelvin-gateway] stopping (pid={})", pid);
    proc::stop(pid, GRACE_MS)?;
    let _ = std::fs::remove_file(&pid_file);
    println!("[kelvin-gateway] stopped");
    Ok(())
}

fn cmd_restart(args: GatewayStartArgs) -> Result<()> {
    // Stop if running, ignoring "not running" errors.
    let pid_file = paths::gateway_pid_path();
    if let Some(pid) = proc::read_pid_file(&pid_file) {
        if proc::is_running(pid) {
            println!("[kelvin-gateway] stopping (pid={})", pid);
            proc::stop(pid, GRACE_MS)?;
            let _ = std::fs::remove_file(&pid_file);
            println!("[kelvin-gateway] stopped");
        }
    }
    cmd_start(args)
}

fn cmd_status() -> Result<()> {
    println!("KELVIN_HOME={}", paths::kelvin_home().display());
    println!(
        "KELVIN_MODEL_PROVIDER={}",
        std::env::var("KELVIN_MODEL_PROVIDER").unwrap_or_else(|_| "kelvin.echo".to_string())
    );
    println!(
        "KELVIN_PLUGIN_INDEX_URL={}",
        std::env::var("KELVIN_PLUGIN_INDEX_URL").unwrap_or_else(|_| "(not set)".to_string())
    );
    println!("log: {}", paths::gateway_log_path().display());
    println!();

    let pid_file = paths::gateway_pid_path();
    let Some(pid) = proc::read_pid_file(&pid_file) else {
        println!("status: stopped");
        return Ok(());
    };

    if !proc::is_running(pid) {
        println!("status: stopped (stale PID file: {})", pid);
        return Ok(());
    }

    let uptime = pid_file
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| std::time::SystemTime::now().duration_since(t).ok())
        .map(|d| format_uptime(d.as_secs()))
        .map(|u| format!(" (up {})", u))
        .unwrap_or_default();

    println!("status: running{}", uptime);
    println!("pid:    {}", pid);
    Ok(())
}

fn cmd_approve_pairing(code: &str) -> Result<()> {
    let bin = gateway_binary();
    let mut cmd_args = vec!["--approve-pairing".to_string(), code.to_string()];
    if let Ok(token) = std::env::var("KELVIN_GATEWAY_TOKEN") {
        if !token.is_empty() {
            cmd_args.push("--token".to_string());
            cmd_args.push(token);
        }
    }
    let status = std::process::Command::new(&bin).args(&cmd_args).status()?;
    std::process::exit(status.code().unwrap_or(1));
}

/// Returns env var overrides with tilde-expanded, absolute paths for all
/// path-type vars the gateway reads. This prevents the subprocess from
/// inheriting literal `~` values loaded from a dotenv file.
fn resolved_path_env(state_dir: &std::path::Path) -> Vec<(String, String)> {
    let mut vars = vec![
        (
            "KELVIN_HOME".to_string(),
            paths::kelvin_home().to_string_lossy().to_string(),
        ),
        (
            "KELVIN_PLUGIN_HOME".to_string(),
            paths::plugin_home().to_string_lossy().to_string(),
        ),
        (
            "KELVIN_TRUST_POLICY_PATH".to_string(),
            paths::trust_policy_path().to_string_lossy().to_string(),
        ),
        (
            "KELVIN_STATE_DIR".to_string(),
            state_dir.to_string_lossy().to_string(),
        ),
    ];
    // Forward memory controller address if set.
    if let Ok(addr) = std::env::var("KELVIN_MEMORY_CONTROLLER_ADDR") {
        vars.push(("KELVIN_MEMORY_CONTROLLER_ADDR".to_string(), addr));
    }
    vars
}

fn format_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if d > 0 {
        return format!("{}d {}h {}m", d, h, m);
    }
    if h > 0 {
        return format!("{}h {}m {}s", h, m, s);
    }
    if m > 0 {
        return format!("{}m {}s", m, s);
    }
    format!("{}s", s)
}

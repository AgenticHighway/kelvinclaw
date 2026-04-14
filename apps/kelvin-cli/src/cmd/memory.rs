use anyhow::{bail, Result};

use crate::cli::{MemoryCmd, MemoryStartArgs};
use crate::paths;
use crate::proc;

const MEMORY_BINARY: &str = "kelvin-memory-controller";
const GRACE_MS: u64 = 3000;

pub fn run(sub: MemoryCmd) -> Result<()> {
    match sub {
        MemoryCmd::Start(args) => cmd_start(args),
        MemoryCmd::Stop => cmd_stop(),
        MemoryCmd::Restart(args) => cmd_restart(args),
        MemoryCmd::Status => cmd_status(),
    }
}

pub fn memory_binary() -> std::path::PathBuf {
    paths::binary_dir().join(MEMORY_BINARY)
}

pub fn build_memory_env() -> anyhow::Result<Vec<(String, String)>> {
    let home = paths::kelvin_home();
    let (_, pub_path) = crate::keys::ensure_memory_keys(&home)?;

    let mut env_vars = vec![(
        "KELVIN_MEMORY_PUBLIC_KEY_PATH".to_string(),
        pub_path.to_string_lossy().to_string(),
    )];

    // Forward optional module env vars if set.
    for key in &[
        "KELVIN_MEMORY_CONTROLLER_ADDR",
        "KELVIN_MEMORY_MODULE_MANIFEST",
        "KELVIN_MEMORY_MODULE_WASM",
        "KELVIN_MEMORY_MODULE_WAT",
    ] {
        if let Ok(val) = std::env::var(key) {
            env_vars.push((key.to_string(), val));
        }
    }
    Ok(env_vars)
}

pub fn cmd_start(args: MemoryStartArgs) -> Result<()> {
    let bin = memory_binary();
    if !bin.exists() {
        bail!(
            "kelvin-memory-controller binary not found at {}\nRun `cargo build -p kelvin-memory-controller` first.",
            bin.display()
        );
    }

    let env_vars = build_memory_env()?;

    if args.foreground {
        let mut cmd = std::process::Command::new(&bin);
        cmd.envs(env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        let status = cmd.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }

    let pid_file = paths::memory_pid_path();
    if let Some(existing_pid) = proc::read_pid_file(&pid_file) {
        if proc::is_running(existing_pid) {
            bail!(
                "memory controller is already running (pid={})\nlog: {}",
                existing_pid,
                paths::memory_log_path().display()
            );
        }
        eprintln!(
            "[kelvin-memory] removing stale PID file (pid={})",
            existing_pid
        );
        let _ = std::fs::remove_file(&pid_file);
    }

    std::fs::create_dir_all(paths::log_dir())?;
    let log = paths::memory_log_path();
    let pid = proc::spawn_detached(&bin, &[], &env_vars, &log, &pid_file)?;
    println!("[kelvin-memory] started (pid={})", pid);
    println!("[kelvin-memory] log: {}", log.display());
    Ok(())
}

pub fn cmd_stop() -> Result<()> {
    let pid_file = paths::memory_pid_path();
    let Some(pid) = proc::read_pid_file(&pid_file) else {
        bail!("memory controller is not running (no PID file)");
    };

    if !proc::is_running(pid) {
        eprintln!(
            "[kelvin-memory] not running (stale PID {}); removing PID file",
            pid
        );
        let _ = std::fs::remove_file(&pid_file);
        return Ok(());
    }

    println!("[kelvin-memory] stopping (pid={})", pid);
    proc::stop(pid, GRACE_MS)?;
    let _ = std::fs::remove_file(&pid_file);
    println!("[kelvin-memory] stopped");
    Ok(())
}

fn cmd_restart(args: MemoryStartArgs) -> Result<()> {
    let pid_file = paths::memory_pid_path();
    if let Some(pid) = proc::read_pid_file(&pid_file) {
        if proc::is_running(pid) {
            println!("[kelvin-memory] stopping (pid={})", pid);
            proc::stop(pid, GRACE_MS)?;
            let _ = std::fs::remove_file(&pid_file);
            println!("[kelvin-memory] stopped");
        }
    }
    cmd_start(args)
}

fn cmd_status() -> Result<()> {
    println!("KELVIN_HOME={}", paths::kelvin_home().display());
    println!("log: {}", paths::memory_log_path().display());
    println!();

    let pid_file = paths::memory_pid_path();
    let Some(pid) = proc::read_pid_file(&pid_file) else {
        println!("status: stopped");
        return Ok(());
    };

    if !proc::is_running(pid) {
        println!("status: stopped (stale PID file: {})", pid);
        return Ok(());
    }

    println!("status: running");
    println!("pid:    {}", pid);
    Ok(())
}

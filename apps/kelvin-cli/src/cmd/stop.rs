use anyhow::Result;

use crate::paths;
use crate::proc;

const GRACE_MS: u64 = 3000;

pub fn run() -> Result<()> {
    stop_daemon("kelvin-memory", paths::memory_pid_path());
    stop_daemon("kelvin-gateway", paths::gateway_pid_path());
    Ok(())
}

fn stop_daemon(name: &str, pid_file: std::path::PathBuf) {
    let Some(pid) = proc::read_pid_file(&pid_file) else {
        println!("[{}] not running", name);
        return;
    };

    if !proc::is_running(pid) {
        println!("[{}] not running (stale PID {})", name, pid);
        let _ = std::fs::remove_file(&pid_file);
        return;
    }

    println!("[{}] stopping (pid={})", name, pid);
    if let Err(e) = proc::stop(pid, GRACE_MS) {
        eprintln!("[{}] warning: stop failed: {}", name, e);
    }
    let _ = std::fs::remove_file(&pid_file);
    println!("[{}] stopped", name);
}

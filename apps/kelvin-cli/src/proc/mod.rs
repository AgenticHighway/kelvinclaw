use std::path::Path;

use anyhow::Result;

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

/// Spawn a process detached from the terminal (daemon mode).
/// Stdout and stderr are redirected to `log_file`.
/// The PID of the spawned process is written to `pid_file` and returned.
pub fn spawn_detached(
    exe: &Path,
    args: &[String],
    env_vars: &[(String, String)],
    log_file: &Path,
    pid_file: &Path,
) -> Result<u32> {
    #[cfg(unix)]
    return unix::spawn_detached(exe, args, env_vars, log_file, pid_file);
    #[cfg(windows)]
    return windows::spawn_detached(exe, args, env_vars, log_file, pid_file);
    #[cfg(not(any(unix, windows)))]
    compile_error!("unsupported platform");
}

/// Returns true if the process with the given PID is currently running.
pub fn is_running(pid: u32) -> bool {
    #[cfg(unix)]
    return unix::is_running(pid);
    #[cfg(windows)]
    return windows::is_running(pid);
    #[cfg(not(any(unix, windows)))]
    false
}

/// Stops the process with the given PID gracefully (SIGTERM → poll → SIGKILL / TerminateProcess).
pub fn stop(pid: u32, grace_ms: u64) -> Result<()> {
    #[cfg(unix)]
    return unix::stop(pid, grace_ms);
    #[cfg(windows)]
    return windows::stop(pid, grace_ms);
    #[cfg(not(any(unix, windows)))]
    Ok(())
}

/// Reads a PID from a PID file. Returns None if the file doesn't exist or is unreadable.
pub fn read_pid_file(pid_file: &Path) -> Option<u32> {
    std::fs::read_to_string(pid_file)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
}

/// Writes a PID to a PID file.
pub fn write_pid_file(pid_file: &Path, pid: u32) -> Result<()> {
    if let Some(parent) = pid_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(pid_file, pid.to_string())?;
    Ok(())
}

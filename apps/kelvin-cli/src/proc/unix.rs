use std::fs;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

pub fn spawn_detached(
    exe: &Path,
    args: &[String],
    env_vars: &[(String, String)],
    log_file: &Path,
    pid_file: &Path,
) -> Result<u32> {
    if let Some(parent) = log_file.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create log directory {}", parent.display()))?;
    }

    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .with_context(|| format!("failed to open log file {}", log_file.display()))?;
    let log_err = log.try_clone().context("failed to clone log file handle")?;

    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    cmd.envs(env_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::from(log));
    cmd.stderr(Stdio::from(log_err));

    // setsid() in the child to detach from the controlling terminal.
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            Ok(())
        });
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn {}", exe.display()))?;

    let pid = child.id();

    // Forget the child handle so we don't wait for it.
    std::mem::forget(child);

    super::write_pid_file(pid_file, pid)?;
    Ok(pid)
}

pub fn is_running(pid: u32) -> bool {
    let nix_pid = Pid::from_raw(pid as i32);
    match signal::kill(nix_pid, None) {
        Ok(()) => true,
        Err(nix::errno::Errno::ESRCH) => false,
        Err(nix::errno::Errno::EPERM) => true, // process exists, we just don't have permission
        Err(_) => false,
    }
}

pub fn stop(pid: u32, grace_ms: u64) -> Result<()> {
    let nix_pid = Pid::from_raw(pid as i32);

    // Send SIGTERM.
    signal::kill(nix_pid, Signal::SIGTERM).ok();

    // Poll until dead or grace period expires.
    let steps = (grace_ms / 500).max(1);
    for _ in 0..steps {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if !is_running(pid) {
            return Ok(());
        }
    }

    // Escalate to SIGKILL.
    signal::kill(nix_pid, Signal::SIGKILL).ok();
    Ok(())
}

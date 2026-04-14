#![allow(dead_code)]

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};

// Windows process creation flags.
const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

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
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);

    let child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn {}", exe.display()))?;

    let pid = child.id();
    std::mem::forget(child);

    super::write_pid_file(pid_file, pid)?;
    Ok(pid)
}

pub fn is_running(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), false);
    sys.process(Pid::from_u32(pid)).is_some()
}

pub fn stop(pid: u32, grace_ms: u64) -> Result<()> {
    // Windows has no SIGTERM. We wait for the grace period then TerminateProcess.
    // Note: this is a hard kill — the gateway binary must tolerate unclean shutdown.
    let steps = (grace_ms / 500).max(1);
    for _ in 0..steps {
        if !is_running(pid) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    if is_running(pid) {
        unsafe {
            use windows_sys::Win32::System::Threading::{
                OpenProcess, TerminateProcess, PROCESS_TERMINATE,
            };
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if handle != 0 {
                TerminateProcess(handle, 1);
                windows_sys::Win32::Foundation::CloseHandle(handle);
            }
        }
    }
    Ok(())
}

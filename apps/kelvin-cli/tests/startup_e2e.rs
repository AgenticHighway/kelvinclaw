use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn configure_command(cmd: &mut Command, home: &Path, gateway_port: u16, memory_port: u16) {
    cmd.env("KELVIN_HOME", home)
        .env("KELVIN_GATEWAY_BIND", format!("127.0.0.1:{gateway_port}"))
        .env(
            "KELVIN_GATEWAY_URL",
            format!("ws://127.0.0.1:{gateway_port}"),
        )
        .env(
            "KELVIN_MEMORY_CONTROLLER_ADDR",
            format!("127.0.0.1:{memory_port}"),
        );
}

fn run_kelvin(home: &Path, gateway_port: u16, memory_port: u16, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_kelvin"));
    configure_command(&mut cmd, home, gateway_port, memory_port);
    cmd.args(args)
        .output()
        .unwrap_or_else(|err| panic!("failed to run kelvin {:?}: {err}", args))
}

fn run_kelvin_success(home: &Path, gateway_port: u16, memory_port: u16, args: &[&str]) -> Output {
    let output = run_kelvin(home, gateway_port, memory_port, args);
    assert!(
        output.status.success(),
        "kelvin {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

struct StackGuard {
    home: tempfile::TempDir,
    gateway_port: u16,
    memory_port: u16,
}

impl Drop for StackGuard {
    fn drop(&mut self) {
        let _ = run_kelvin(
            self.home.path(),
            self.gateway_port,
            self.memory_port,
            &["stop"],
        );
    }
}

#[test]
fn e2e_start_restarts_gateway_when_init_rewrites_env() {
    let _guard = test_lock().lock().expect("lock e2e");
    let home = tempfile::tempdir().expect("tempdir");
    let gateway_port = free_port();
    let memory_port = free_port();
    let cleanup = StackGuard {
        home,
        gateway_port,
        memory_port,
    };

    run_kelvin_success(
        cleanup.home.path(),
        cleanup.gateway_port,
        cleanup.memory_port,
        &["init", "--provider", "echo", "--force"],
    );
    run_kelvin_success(
        cleanup.home.path(),
        cleanup.gateway_port,
        cleanup.memory_port,
        &["start"],
    );

    let first_pid = std::fs::read_to_string(cleanup.home.path().join("gateway.pid"))
        .expect("read first gateway pid");

    std::thread::sleep(Duration::from_millis(20));
    run_kelvin_success(
        cleanup.home.path(),
        cleanup.gateway_port,
        cleanup.memory_port,
        &["init", "--provider", "echo", "--force"],
    );
    std::thread::sleep(Duration::from_millis(20));

    let restart_output = run_kelvin_success(
        cleanup.home.path(),
        cleanup.gateway_port,
        cleanup.memory_port,
        &["start"],
    );
    let second_pid = std::fs::read_to_string(cleanup.home.path().join("gateway.pid"))
        .expect("read second gateway pid");

    let stdout = String::from_utf8_lossy(&restart_output.stdout);
    assert!(
        stdout.contains("[kelvin] restarting gateway to apply updated configuration"),
        "missing restart message in stdout:\n{stdout}"
    );
    assert_ne!(
        first_pid.trim(),
        second_pid.trim(),
        "gateway pid should change after config rewrite"
    );
}

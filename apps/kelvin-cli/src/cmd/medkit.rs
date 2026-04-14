use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::cli::MedkitArgs;
use crate::paths;
use crate::proc;

const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

#[derive(Debug, Serialize)]
struct CheckResult {
    status: &'static str,
    message: String,
    hint: Option<String>,
}

struct Medkit {
    results: Vec<CheckResult>,
    pass: u32,
    warn: u32,
    fail: u32,
    fix: bool,
    json: bool,
}

impl Medkit {
    fn new(fix: bool, json: bool) -> Self {
        Self {
            results: vec![],
            pass: 0,
            warn: 0,
            fail: 0,
            fix,
            json,
        }
    }

    fn pass(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if !self.json {
            println!(" {}✔{} {}", GREEN, RESET, msg);
        }
        self.results.push(CheckResult {
            status: "pass",
            message: msg,
            hint: None,
        });
        self.pass += 1;
    }

    fn warn(&mut self, msg: impl Into<String>, hint: Option<&str>) {
        let msg = msg.into();
        if !self.json {
            println!(" {}⚠{} {}", YELLOW, RESET, msg);
            if let Some(h) = hint {
                println!("    hint: {}", h);
            }
        }
        self.results.push(CheckResult {
            status: "warn",
            message: msg,
            hint: hint.map(|s| s.to_string()),
        });
        self.warn += 1;
    }

    fn fail(&mut self, msg: impl Into<String>, hint: Option<&str>) {
        let msg = msg.into();
        if !self.json {
            println!(" {}✘{} {}", RED, RESET, msg);
            if let Some(h) = hint {
                println!("    hint: {}", h);
            }
        }
        self.results.push(CheckResult {
            status: "fail",
            message: msg,
            hint: hint.map(|s| s.to_string()),
        });
        self.fail += 1;
    }
}

pub fn run(args: MedkitArgs) -> Result<()> {
    let mut mk = Medkit::new(args.fix, args.json);

    if !args.json {
        println!("\n=== kelvin medkit ===\n");
    }

    check_prerequisites(&mut mk);
    check_directories(&mut mk);
    check_env_files(&mut mk);
    check_api_keys(&mut mk);
    check_trust_policy(&mut mk);
    check_plugins(&mut mk);
    check_plugin_index(&mut mk);
    check_processes(&mut mk);
    check_binaries(&mut mk);
    check_security(&mut mk);

    if args.json {
        #[derive(Serialize)]
        struct Report {
            pass: u32,
            warn: u32,
            fail: u32,
            checks: Vec<CheckResult>,
        }
        let report = Report {
            pass: mk.pass,
            warn: mk.warn,
            fail: mk.fail,
            checks: mk.results,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!();
        println!(
            "Summary: {}✔ {} passed{} | {}⚠ {} warnings{} | {}✘ {} failed{}",
            GREEN, mk.pass, RESET,
            YELLOW, mk.warn, RESET,
            RED, mk.fail, RESET,
        );
    }

    if mk.fail > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn check_command_exists(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or_else(|_| {
            // Fallback: try with --help for commands that don't support --version
            std::process::Command::new(name)
                .arg("--help")
                .output()
                .map(|_| true)
                .unwrap_or(false)
        })
}

fn which_version(name: &str) -> Option<String> {
    std::process::Command::new(name)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s.lines().next().unwrap_or(&s).to_string())
            }
        })
}

fn check_prerequisites(mk: &mut Medkit) {
    if !mk.json {
        println!("── prerequisites ─────────────────────────────────────");
    }

    for (cmd, hint) in &[
        ("jq", Some("brew install jq / apt install jq")),
        ("curl", None),
        ("tar", None),
        ("openssl", None),
    ] {
        if check_command_exists(cmd) {
            let ver = which_version(cmd)
                .map(|v| format!(" ({})", v))
                .unwrap_or_default();
            mk.pass(format!("{}{}", cmd, ver));
        } else {
            mk.fail(format!("{} not found", cmd), *hint);
        }
    }

    for (cmd, hint) in &[
        ("cargo", Some("Install from https://rustup.rs")),
        ("rustc", Some("Install from https://rustup.rs")),
    ] {
        if check_command_exists(cmd) {
            let ver = which_version(cmd)
                .map(|v| format!(" ({})", v))
                .unwrap_or_default();
            mk.pass(format!("{}{}", cmd, ver));
        } else {
            mk.warn(format!("{} not found (optional for dev builds)", cmd), *hint);
        }
    }
}

fn check_directories(mk: &mut Medkit) {
    if !mk.json {
        println!("\n── directories ───────────────────────────────────────");
    }

    let dirs = vec![
        (paths::kelvin_home(), format!("KELVIN_HOME ({})", paths::kelvin_home().display())),
        (paths::plugin_home(), format!("Plugin home ({})", paths::plugin_home().display())),
        (paths::state_dir(), format!("State directory ({})", paths::state_dir().display())),
    ];

    for (dir, label) in dirs {
        if dir.exists() {
            mk.pass(&label);
        } else if mk.fix {
            match std::fs::create_dir_all(&dir) {
                Ok(_) => mk.pass(format!("{} (created)", label)),
                Err(e) => mk.fail(format!("{} missing (create failed: {})", label, e), None),
            }
        } else {
            mk.fail(
                format!("{} missing", label),
                Some(&format!("Run: mkdir -p {}", dir.display())),
            );
        }
    }
}

fn check_env_files(mk: &mut Medkit) {
    if !mk.json {
        println!("\n── .env files ────────────────────────────────────────");
    }

    let home = paths::kelvin_home();
    let candidates = [
        home.join(".env.local"),
        home.join(".env"),
        std::path::PathBuf::from(".env.local"),
        std::path::PathBuf::from(".env"),
    ];

    let mut found = false;
    for path in &candidates {
        if path.exists() {
            mk.pass(format!(".env found: {}", path.display()));
            found = true;
        }
    }
    if !found {
        mk.warn(
            "No .env file found",
            Some(&format!("Create {}", paths::dotenv_path().display())),
        );
    }
}

fn check_api_keys(mk: &mut Medkit) {
    if !mk.json {
        println!("\n── API keys ──────────────────────────────────────────");
    }

    let mut any_key = false;

    let keys = [
        ("OPENAI_API_KEY", "sk-"),
        ("ANTHROPIC_API_KEY", "sk-ant-"),
        ("OPENROUTER_API_KEY", "sk-or-"),
    ];

    for (var, prefix) in &keys {
        if let Ok(val) = std::env::var(var) {
            if val.starts_with(prefix) {
                mk.pass(format!("{} set ({}...)", var, prefix));
            } else {
                mk.warn(
                    format!("{} set but unusual format", var),
                    Some(&format!("Expected prefix: {}", prefix)),
                );
            }
            any_key = true;
        }
    }

    if !any_key {
        mk.warn(
            "No model provider API keys detected",
            Some("Set OPENAI_API_KEY, ANTHROPIC_API_KEY, or OPENROUTER_API_KEY. Echo mode works without keys."),
        );
    }

    if let Ok(provider) = std::env::var("KELVIN_MODEL_PROVIDER") {
        mk.pass(format!("KELVIN_MODEL_PROVIDER={}", provider));
    } else {
        mk.warn(
            "KELVIN_MODEL_PROVIDER not set",
            Some("Will auto-detect from API keys or default to echo"),
        );
    }
}

fn check_trust_policy(mk: &mut Medkit) {
    if !mk.json {
        println!("\n── trust policy ──────────────────────────────────────");
    }

    let trust_path = paths::trust_policy_path();
    if trust_path.exists() {
        match std::fs::read(&trust_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        {
            Some(v) => {
                let req_sig = v.get("require_signature").and_then(|v| v.as_bool()).unwrap_or(false);
                let pub_count = v.get("publishers").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                mk.pass(format!(
                    "Trust policy: {} (require_signature={}, {} publishers)",
                    trust_path.display(), req_sig, pub_count
                ));
            }
            None => {
                mk.fail(
                    "Trust policy is invalid JSON",
                    Some(&format!("Delete and re-create: rm {}", trust_path.display())),
                );
            }
        }
    } else if mk.fix {
        let _ = std::fs::create_dir_all(trust_path.parent().unwrap_or(Path::new(".")));
        match std::fs::write(&trust_path, r#"{"require_signature":false,"publishers":[]}"#) {
            Ok(_) => mk.pass(format!("Trust policy created: {}", trust_path.display())),
            Err(e) => mk.fail(format!("Trust policy missing (create failed: {})", e), None),
        }
    } else {
        mk.fail(
            format!("Trust policy missing: {}", trust_path.display()),
            Some("Run: kelvin medkit --fix"),
        );
    }
}

fn check_plugins(mk: &mut Medkit) {
    if !mk.json {
        println!("\n── plugins ───────────────────────────────────────────");
    }

    let plugin_home = paths::plugin_home();
    if !plugin_home.exists() {
        mk.warn("No plugins installed", Some("Run: kelvin plugin install kelvin.cli"));
        return;
    }

    let installed = match super::plugin::list_installed_plugins() {
        Ok(v) => v,
        Err(e) => {
            mk.fail(format!("Failed to list plugins: {}", e), None);
            return;
        }
    };

    if installed.is_empty() {
        mk.warn("No plugins installed", Some("Run: kelvin plugin install kelvin.cli"));
    }

    let mut cli_installed = false;
    for (id, version) in &installed {
        let current = plugin_home.join(id).join("current");
        if id == "kelvin.cli" {
            cli_installed = true;
        }

        // Try to read plugin.json for entrypoint/runtime info.
        let manifest = current.join("plugin.json");
        if let Ok(bytes) = std::fs::read(&manifest) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let runtime = v.get("runtime").and_then(|r| r.as_str()).unwrap_or("wasm");
                let entrypoint = v
                    .get("entrypoint")
                    .and_then(|e| e.as_str())
                    .unwrap_or("unknown");
                let wasm_present = current.join("payload").join(entrypoint).exists();
                if wasm_present {
                    mk.pass(format!("{}@{} ({})", id, version, runtime));
                } else {
                    mk.fail(
                        format!("{}: missing WASM payload ({})", id, entrypoint),
                        Some("Reinstall plugin"),
                    );
                }
            } else {
                mk.fail(
                    format!("{}@{}: corrupt plugin.json", id, version),
                    Some(&format!("Reinstall: kelvin plugin install {}", id)),
                );
            }
        } else {
            mk.warn(format!("{}@{}: no plugin.json manifest", id, version), None);
        }
    }

    if cli_installed {
        mk.pass("Required plugin kelvin.cli: installed");
    } else {
        mk.fail(
            "Required plugin kelvin.cli: missing",
            Some("Run: kelvin plugin install kelvin.cli"),
        );
    }
}

fn check_plugin_index(mk: &mut Medkit) {
    if !mk.json {
        println!("\n── plugin index ──────────────────────────────────────");
    }

    let index_url = std::env::var("KELVIN_PLUGIN_INDEX_URL").unwrap_or_default();
    if index_url.is_empty() {
        mk.warn(
            "Plugin index not configured",
            Some("Set KELVIN_PLUGIN_INDEX_URL"),
        );
        return;
    }

    let resolved = super::plugin_ops::download::resolve_index_url(&index_url);
    match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .and_then(|c| c.get(&resolved).send())
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json::<serde_json::Value>())
    {
        Ok(index) => {
            let count = index
                .get("plugins")
                .and_then(|p| p.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            mk.pass(format!("Plugin index reachable ({}, {} plugins)", index_url, count));
        }
        Err(e) => {
            mk.warn(
                format!("Plugin index unreachable: {}", e),
                Some("Check network or KELVIN_PLUGIN_INDEX_URL"),
            );
        }
    }
}

fn check_processes(mk: &mut Medkit) {
    if !mk.json {
        println!("\n── processes ─────────────────────────────────────────");
    }

    check_pid_file(mk, "Memory controller", paths::memory_pid_path());
    check_pid_file(mk, "Gateway", paths::gateway_pid_path());
}

fn check_pid_file(mk: &mut Medkit, name: &str, pid_file: std::path::PathBuf) {
    if let Some(pid) = proc::read_pid_file(&pid_file) {
        if proc::is_running(pid) {
            mk.pass(format!("{}: running (pid {})", name, pid));
        } else {
            mk.warn(
                format!("{}: stale PID file (pid {} not running)", name, pid),
                Some(&format!("Remove: rm {}", pid_file.display())),
            );
        }
    } else {
        mk.warn(
            format!("{}: not running", name),
            Some("Start with: kelvin start"),
        );
    }
}

fn check_binaries(mk: &mut Medkit) {
    if !mk.json {
        println!("\n── binaries ──────────────────────────────────────────");
    }

    let bin_dir = paths::binary_dir();
    for (name, build_hint) in &[
        ("kelvin-gateway", "cargo build -p kelvin-gateway"),
        ("kelvin-memory-controller", "cargo build -p kelvin-memory-controller"),
        ("kelvin-tui", "cargo build -p kelvin-tui"),
    ] {
        let bin = bin_dir.join(name);
        if bin.exists() {
            mk.pass(format!("{} binary: built", name));
        } else {
            mk.warn(
                format!("{} binary: not found", name),
                Some(&format!("Run: {}", build_hint)),
            );
        }
    }
}

fn check_security(mk: &mut Medkit) {
    if !mk.json {
        println!("\n── security ──────────────────────────────────────────");
    }

    let allow_insecure = std::env::var("KELVIN_GATEWAY_ALLOW_INSECURE_PUBLIC_BIND")
        .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);

    if allow_insecure {
        mk.warn(
            "KELVIN_GATEWAY_ALLOW_INSECURE_PUBLIC_BIND is enabled",
            Some("Disable for production use"),
        );
    }

    let bind = std::env::var("KELVIN_GATEWAY_BIND")
        .or_else(|_| std::env::var("KELVIN_GATEWAY_ADDR"))
        .unwrap_or_else(|_| "127.0.0.1:34617".to_string());

    let is_loopback = bind.starts_with("127.") || bind.starts_with("::1") || bind.starts_with("localhost");
    if is_loopback {
        mk.pass(format!("Gateway bound to loopback ({})", bind));
    } else {
        let token = std::env::var("KELVIN_GATEWAY_TOKEN").unwrap_or_default();
        if token.is_empty() {
            mk.fail(
                "Gateway bound to non-loopback address without token",
                Some("Set KELVIN_GATEWAY_TOKEN for non-local binds"),
            );
        } else if token.len() < 32 {
            mk.fail(
                "Gateway token is weak or default",
                Some("Use a strong random token (32+ characters)"),
            );
        } else {
            mk.pass("Gateway token set for non-local bind");
        }
    }

    // Check .env is in .gitignore.
    let gitignore = std::path::PathBuf::from(".gitignore");
    if gitignore.exists() {
        let content = std::fs::read_to_string(&gitignore).unwrap_or_default();
        if content.contains(".env") {
            mk.pass(".env is in .gitignore");
        } else {
            mk.warn(".env not found in .gitignore", Some("Add .env to .gitignore to avoid leaking secrets"));
        }
    }
}

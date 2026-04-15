use anyhow::{Context, Result};

use crate::cli::StartArgs;
use crate::paths;
use crate::proc;

const GATEWAY_RESTART_GRACE_MS: u64 = 3000;
const DEFAULT_PLUGIN_INDEX_URL: &str =
    "https://raw.githubusercontent.com/AgenticHighway/kelvinclaw-plugins/main/index.json";
const KELVIN_CLI_PLUGIN_ID: &str = "kelvin.cli";

pub fn run(args: StartArgs) -> Result<()> {
    ensure_config()?;
    ensure_trust_policy()?;
    ensure_plugin()?;

    if !args.no_memory {
        start_memory_daemon()?;
    }
    start_gateway_daemon()?;

    println!("[kelvin] stack started. run `kelvin tui` to open the terminal UI.");
    println!("[kelvin] run `kelvin stop` to shut everything down.");
    Ok(())
}

/// Ensures the .env exists; directs the user to `kelvin init` if not.
pub fn ensure_config() -> Result<()> {
    let dot_env = paths::dotenv_path();
    if !dot_env.exists() {
        anyhow::bail!(
            "no config found at {}\nRun `kelvin init` to set up, or `kelvin medkit` to diagnose.",
            dot_env.display()
        );
    }
    Ok(())
}

/// Ensures the trust policy file exists; writes a permissive default if missing.
pub fn ensure_trust_policy() -> Result<()> {
    let trust_path = paths::trust_policy_path();
    if !trust_path.exists() {
        if let Some(parent) = trust_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &trust_path,
            r#"{"require_signature":false,"publishers":[]}"#,
        )
        .with_context(|| format!("failed to write trust policy to {}", trust_path.display()))?;
        println!(
            "[kelvin] wrote permissive trust policy: {}",
            trust_path.display()
        );
    }
    Ok(())
}

/// Ensures the required model provider plugin is installed.
pub fn ensure_plugin() -> Result<()> {
    let provider =
        std::env::var("KELVIN_MODEL_PROVIDER").unwrap_or_else(|_| "kelvin.echo".to_string());
    ensure_plugin_installed(&provider)
}

pub fn ensure_cli_plugin() -> Result<()> {
    ensure_plugin_installed(KELVIN_CLI_PLUGIN_ID)
}

fn ensure_plugin_installed(plugin_id: &str) -> Result<()> {
    if plugin_id == "kelvin.echo" {
        return Ok(());
    }

    let plugin_home = paths::plugin_home();
    let current = plugin_home.join(plugin_id).join("current");
    if current.exists() {
        return Ok(());
    }

    let index_url = std::env::var("KELVIN_PLUGIN_INDEX_URL")
        .unwrap_or_else(|_| DEFAULT_PLUGIN_INDEX_URL.to_string());

    println!("[kelvin] installing plugin: {}", plugin_id);
    std::fs::create_dir_all(&plugin_home)?;
    super::plugin_ops::install_from_index(plugin_id, None, &plugin_home, &index_url, false)
        .with_context(|| match plugin_id {
            KELVIN_CLI_PLUGIN_ID => "failed to install required CLI plugin 'kelvin.cli'. \
                Set KELVIN_PLUGIN_INDEX_URL or install the plugin manually."
                .to_string(),
            other => format!(
                "failed to install model provider plugin '{}'. \
                Set KELVIN_PLUGIN_INDEX_URL or choose a different KELVIN_MODEL_PROVIDER.",
                other
            ),
        })
}

pub fn start_memory_daemon() -> Result<()> {
    use crate::cli::MemoryStartArgs;
    use crate::cmd::memory::{cmd_start, memory_binary};

    if !memory_binary().exists() {
        eprintln!(
            "[kelvin] warning: kelvin-memory-controller not found, skipping memory controller"
        );
        return Ok(());
    }

    let pid_file = paths::memory_pid_path();
    if let Some(pid) = proc::read_pid_file(&pid_file) {
        if proc::is_running(pid) {
            println!("[kelvin] memory controller already running (pid={})", pid);
            return Ok(());
        }
    }

    cmd_start(MemoryStartArgs { foreground: false })
}

pub fn start_gateway_daemon() -> Result<()> {
    use crate::cli::GatewayCmd;
    use crate::cli::GatewayStartArgs;
    use crate::cmd::gateway::run;

    let home = paths::kelvin_home();
    crate::keys::ensure_memory_keys(&home)?;
    let model_provider =
        std::env::var("KELVIN_MODEL_PROVIDER").unwrap_or_else(|_| "kelvin.echo".to_string());
    let gateway_args = gateway_start_args_from_env();

    let pid_file = paths::gateway_pid_path();
    if let Some(pid) = proc::read_pid_file(&pid_file) {
        if proc::is_running(pid) {
            if gateway_requires_restart(&pid_file, &model_provider) {
                println!("[kelvin] restarting gateway to apply updated configuration");
                proc::stop(pid, GATEWAY_RESTART_GRACE_MS)?;
                let _ = std::fs::remove_file(&pid_file);
            } else {
                println!("[kelvin] gateway already running (pid={})", pid);
                return Ok(());
            }
        }
    }

    run(GatewayCmd::Start(GatewayStartArgs {
        foreground: false,
        gateway_args,
    }))
}

fn gateway_start_args_from_env() -> Vec<String> {
    let mut args = Vec::new();
    if let Ok(bind_addr) = std::env::var("KELVIN_GATEWAY_BIND") {
        let trimmed = bind_addr.trim();
        if !trimmed.is_empty() {
            args.push("--bind".to_string());
            args.push(trimmed.to_string());
        }
    }
    args
}

fn gateway_requires_restart(pid_file: &std::path::Path, model_provider: &str) -> bool {
    dependency_changed_after(pid_file, &gateway_restart_dependency_paths(model_provider))
}

fn gateway_restart_dependency_paths(model_provider: &str) -> Vec<std::path::PathBuf> {
    let mut paths_to_watch = vec![paths::dotenv_path(), paths::trust_policy_path()];
    let trimmed_provider = model_provider.trim();
    if !trimmed_provider.is_empty() && trimmed_provider != "kelvin.echo" {
        paths_to_watch.push(paths::plugin_home().join(trimmed_provider).join("current"));
    }
    paths_to_watch
}

fn dependency_changed_after(
    reference_path: &std::path::Path,
    dependency_paths: &[std::path::PathBuf],
) -> bool {
    let Some(reference_modified) = file_modified_time(reference_path) else {
        return false;
    };
    dependency_paths
        .iter()
        .filter_map(|path| file_modified_time(path))
        .any(|modified| modified > reference_modified)
}

fn file_modified_time(path: &std::path::Path) -> Option<std::time::SystemTime> {
    path.metadata().ok()?.modified().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn write_file(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, b"test").expect("write file");
    }

    fn tick_filesystem_clock() {
        std::thread::sleep(Duration::from_millis(20));
    }

    #[test]
    fn gateway_start_args_from_env_includes_bind_override() {
        let _guard = env_lock().lock().expect("lock env");
        unsafe {
            std::env::set_var("KELVIN_GATEWAY_BIND", "127.0.0.1:43117");
        }
        assert_eq!(
            gateway_start_args_from_env(),
            vec!["--bind".to_string(), "127.0.0.1:43117".to_string()]
        );
        unsafe {
            std::env::remove_var("KELVIN_GATEWAY_BIND");
        }
    }

    #[test]
    fn gateway_requires_restart_when_dotenv_is_newer_than_pid_file() {
        let _guard = env_lock().lock().expect("lock env");
        let home = tempfile::tempdir().expect("tempdir");
        unsafe {
            std::env::set_var("KELVIN_HOME", home.path());
        }

        let pid_file = paths::gateway_pid_path();
        write_file(&pid_file);
        tick_filesystem_clock();
        write_file(&paths::dotenv_path());

        assert!(gateway_requires_restart(&pid_file, "kelvin.echo"));

        unsafe {
            std::env::remove_var("KELVIN_HOME");
        }
    }

    #[test]
    fn gateway_requires_restart_when_provider_plugin_changes() {
        let _guard = env_lock().lock().expect("lock env");
        let home = tempfile::tempdir().expect("tempdir");
        unsafe {
            std::env::set_var("KELVIN_HOME", home.path());
        }

        let pid_file = paths::gateway_pid_path();
        write_file(&pid_file);
        tick_filesystem_clock();
        write_file(&paths::plugin_home().join("kelvin.openai").join("current"));

        assert!(gateway_requires_restart(&pid_file, "kelvin.openai"));

        unsafe {
            std::env::remove_var("KELVIN_HOME");
        }
    }

    #[test]
    fn gateway_requires_no_restart_when_pid_file_is_newest() {
        let _guard = env_lock().lock().expect("lock env");
        let home = tempfile::tempdir().expect("tempdir");
        unsafe {
            std::env::set_var("KELVIN_HOME", home.path());
        }

        write_file(&paths::dotenv_path());
        write_file(&paths::trust_policy_path());
        tick_filesystem_clock();
        let pid_file = paths::gateway_pid_path();
        write_file(&pid_file);

        assert!(!gateway_requires_restart(&pid_file, "kelvin.echo"));

        unsafe {
            std::env::remove_var("KELVIN_HOME");
        }
    }
}

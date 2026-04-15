use std::path::PathBuf;

/// Returns the kelvin home directory: $KELVIN_HOME or ~/.kelvinclaw
pub fn kelvin_home() -> PathBuf {
    if let Ok(h) = std::env::var("KELVIN_HOME") {
        let h = h.trim().to_string();
        if !h.is_empty() {
            let p = PathBuf::from(h);
            if let Ok(expanded) = expand_tilde(&p) {
                return expanded;
            }
            return p;
        }
    }
    dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".kelvinclaw")
}

/// Returns the plugin home: $KELVIN_PLUGIN_HOME or {kelvin_home}/plugins
pub fn plugin_home() -> PathBuf {
    if let Ok(p) = std::env::var("KELVIN_PLUGIN_HOME") {
        let p = p.trim().to_string();
        if !p.is_empty() {
            let pb = PathBuf::from(p);
            if let Ok(expanded) = expand_tilde(&pb) {
                return expanded;
            }
            return pb;
        }
    }
    kelvin_home().join("plugins")
}

/// Returns the trust policy path: $KELVIN_TRUST_POLICY_PATH or {kelvin_home}/trusted_publishers.json
pub fn trust_policy_path() -> PathBuf {
    if let Ok(p) = std::env::var("KELVIN_TRUST_POLICY_PATH") {
        let p = p.trim().to_string();
        if !p.is_empty() {
            let pb = PathBuf::from(p);
            if let Ok(expanded) = expand_tilde(&pb) {
                return expanded;
            }
            return pb;
        }
    }
    kelvin_home().join("trusted_publishers.json")
}

/// Returns the state dir: $KELVIN_STATE_DIR or {kelvin_home}/state
pub fn state_dir() -> PathBuf {
    if let Ok(p) = std::env::var("KELVIN_STATE_DIR") {
        let p = p.trim().to_string();
        if !p.is_empty() {
            let pb = PathBuf::from(p);
            if let Ok(expanded) = expand_tilde(&pb) {
                return expanded;
            }
            return pb;
        }
    }
    kelvin_home().join("state")
}

/// Returns the log dir: {kelvin_home}/logs
pub fn log_dir() -> PathBuf {
    kelvin_home().join("logs")
}

/// Returns the gateway PID file path: {kelvin_home}/gateway.pid
pub fn gateway_pid_path() -> PathBuf {
    kelvin_home().join("gateway.pid")
}

/// Returns the memory controller PID file path: {kelvin_home}/memory.pid
pub fn memory_pid_path() -> PathBuf {
    kelvin_home().join("memory.pid")
}

/// Returns the gateway log path: {kelvin_home}/logs/gateway.log
pub fn gateway_log_path() -> PathBuf {
    log_dir().join("gateway.log")
}

/// Returns the memory controller log path: {kelvin_home}/logs/memory.log
pub fn memory_log_path() -> PathBuf {
    log_dir().join("memory.log")
}

/// Returns the memory private key path: {kelvin_home}/memory-private.pem
pub fn memory_private_key_path() -> PathBuf {
    kelvin_home().join("memory-private.pem")
}

/// Returns the memory public key path: {kelvin_home}/memory-public.pem
pub fn memory_public_key_path() -> PathBuf {
    kelvin_home().join("memory-public.pem")
}

/// Returns the canonical dotenv path: {kelvin_home}/.env
pub fn dotenv_path() -> PathBuf {
    kelvin_home().join(".env")
}

/// Returns the launcher-managed preferences path: {kelvin_home}/preferences.env
pub fn preferences_path() -> PathBuf {
    kelvin_home().join("preferences.env")
}

/// Returns the directory containing the current executable (sibling of bin/)
pub fn binary_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Expands a leading `~` to the home directory.
pub fn expand_tilde(path: &std::path::Path) -> anyhow::Result<PathBuf> {
    let s = path.to_string_lossy();
    if s.starts_with("~/") || s == "~" {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home dir"))?;
        if s == "~" {
            return Ok(home);
        }
        return Ok(home.join(&s[2..]));
    }
    Ok(path.to_path_buf())
}

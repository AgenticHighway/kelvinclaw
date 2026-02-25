use jsonwebtoken::DecodingKey;
use kelvin_core::{KelvinError, KelvinResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderProfile {
    Minimal,
    IPhone,
    LinuxGpu,
}

#[derive(Debug, Clone)]
pub struct MemoryControllerConfig {
    pub issuer: String,
    pub audience: String,
    pub decoding_key_pem: String,
    pub clock_skew_secs: u64,
    pub max_module_bytes: usize,
    pub max_memory_pages: u32,
    pub default_fuel: u64,
    pub default_timeout_ms: u64,
    pub default_max_response_bytes: usize,
    pub replay_window_secs: u64,
    pub profile: ProviderProfile,
}

impl Default for MemoryControllerConfig {
    fn default() -> Self {
        Self {
            issuer: "kelvin-root".to_string(),
            audience: "kelvin-memory-controller".to_string(),
            decoding_key_pem: String::new(),
            clock_skew_secs: 30,
            max_module_bytes: 2 * 1024 * 1024,
            max_memory_pages: 64,
            default_fuel: 100_000,
            default_timeout_ms: 2_000,
            default_max_response_bytes: 1024 * 1024,
            replay_window_secs: 120,
            profile: ProviderProfile::Minimal,
        }
    }
}

impl MemoryControllerConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(value) = std::env::var("KELVIN_MEMORY_ISSUER") {
            if !value.trim().is_empty() {
                cfg.issuer = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_AUDIENCE") {
            if !value.trim().is_empty() {
                cfg.audience = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_PUBLIC_KEY_PEM") {
            if !value.trim().is_empty() {
                cfg.decoding_key_pem = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_CLOCK_SKEW_SECS") {
            if let Ok(parsed) = value.parse::<u64>() {
                cfg.clock_skew_secs = parsed;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_MAX_MODULE_BYTES") {
            if let Ok(parsed) = value.parse::<usize>() {
                cfg.max_module_bytes = parsed;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_MAX_MEMORY_PAGES") {
            if let Ok(parsed) = value.parse::<u32>() {
                cfg.max_memory_pages = parsed;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_DEFAULT_FUEL") {
            if let Ok(parsed) = value.parse::<u64>() {
                cfg.default_fuel = parsed;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_DEFAULT_TIMEOUT_MS") {
            if let Ok(parsed) = value.parse::<u64>() {
                cfg.default_timeout_ms = parsed;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_DEFAULT_MAX_RESPONSE_BYTES") {
            if let Ok(parsed) = value.parse::<usize>() {
                cfg.default_max_response_bytes = parsed;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_REPLAY_WINDOW_SECS") {
            if let Ok(parsed) = value.parse::<u64>() {
                cfg.replay_window_secs = parsed;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_PROFILE") {
            let normalized = value.trim().to_ascii_lowercase();
            cfg.profile = match normalized.as_str() {
                "iphone" => ProviderProfile::IPhone,
                "linux-gpu" | "linux_gpu" => ProviderProfile::LinuxGpu,
                _ => ProviderProfile::Minimal,
            };
        }
        cfg
    }

    pub fn decoding_key(&self) -> KelvinResult<DecodingKey> {
        DecodingKey::from_ed_pem(self.decoding_key_pem.as_bytes())
            .map_err(|err| KelvinError::InvalidInput(format!("invalid decoding key pem: {err}")))
    }
}

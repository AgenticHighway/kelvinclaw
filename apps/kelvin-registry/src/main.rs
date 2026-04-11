use std::net::SocketAddr;
use std::path::PathBuf;

use kelvin_registry::{consts, run_registry, RegistryConfig};

fn usage() -> &'static str {
    "Usage: kelvin-registry --index <path> [--bind <host:port>] [--trust-policy <path>]"
}

fn parse_args() -> Result<RegistryConfig, String> {
    let mut bind_addr = std::env::var(consts::ENV_BIND_ADDR)
        .ok()
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(consts::DEFAULT_BIND_ADDR)
        .parse::<SocketAddr>()
        .map_err(|err| format!("invalid {} value: {err}", consts::ENV_BIND_ADDR))?;
    let mut index_path = std::env::var(consts::ENV_INDEX_PATH)
        .ok()
        .map(PathBuf::from);
    let mut trust_policy_path = std::env::var(consts::ENV_TRUST_POLICY_PATH)
        .ok()
        .map(PathBuf::from);

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            consts::FLAG_BIND => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing value for {}", consts::FLAG_BIND))?;
                bind_addr = value.parse::<SocketAddr>().map_err(|err| {
                    format!("invalid {} value '{value}': {err}", consts::FLAG_BIND)
                })?;
            }
            consts::FLAG_INDEX => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing value for {}", consts::FLAG_INDEX))?;
                index_path = Some(PathBuf::from(value));
            }
            consts::FLAG_TRUST_POLICY => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing value for {}", consts::FLAG_TRUST_POLICY))?;
                trust_policy_path = Some(PathBuf::from(value));
            }
            consts::FLAG_HELP_SHORT | consts::FLAG_HELP_LONG => {
                println!("{}", usage());
                std::process::exit(0);
            }
            _ => {
                return Err(format!("unknown argument: {arg}\n{}", usage()));
            }
        }
    }

    let index_path = index_path.ok_or_else(|| {
        format!(
            "missing registry index path; set --index <path> or KELVIN_PLUGIN_REGISTRY_INDEX\n{}",
            usage()
        )
    })?;
    Ok(RegistryConfig {
        bind_addr,
        index_path,
        trust_policy_path,
    })
}

#[tokio::main]
async fn main() {
    match parse_args() {
        Ok(config) => {
            if let Err(err) = run_registry(config).await {
                eprintln!("registry error: {err}");
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

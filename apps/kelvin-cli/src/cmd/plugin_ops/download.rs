use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

/// Runs a closure on a dedicated OS thread so that `reqwest::blocking` calls
/// never panic when invoked from inside a tokio runtime context.
/// (`reqwest::blocking` creates its own runtime internally; nesting panics.)
fn run_blocking_http<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    std::thread::spawn(f)
        .join()
        .map_err(|_| anyhow::anyhow!("HTTP worker thread panicked"))?
}

/// Downloads a URL to `dest`, verifying the sha256 checksum.
pub fn download_and_verify(url: &str, expected_sha256: &str, dest: &Path) -> Result<()> {
    eprintln!("[kelvin] downloading: {}", url);

    let url = url.to_string();
    let expected_sha256 = expected_sha256.to_string();
    let dest = dest.to_path_buf();

    run_blocking_http(move || {
        let response = reqwest::blocking::get(&url)
            .with_context(|| format!("failed to GET {}", url))?
            .error_for_status()
            .with_context(|| format!("HTTP error fetching {}", url))?;

        let bytes = response
            .bytes()
            .with_context(|| format!("failed to read response body from {}", url))?;

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = format!("{:x}", hasher.finalize());

        if actual != expected_sha256 {
            bail!(
                "checksum mismatch for {}\n  expected: {}\n  actual:   {}",
                url,
                expected_sha256,
                actual
            );
        }

        let mut f = std::fs::File::create(&dest)
            .with_context(|| format!("failed to create {}", dest.display()))?;
        f.write_all(&bytes)
            .with_context(|| format!("failed to write to {}", dest.display()))?;

        Ok(())
    })
}

/// Fetches a plugin index from a URL and returns the parsed JSON value.
pub fn fetch_index(index_url: &str) -> Result<serde_json::Value> {
    let url = resolve_index_url(index_url);
    eprintln!("[kelvin] fetching index: {}", url);

    let value: serde_json::Value = run_blocking_http(move || {
        let resp = reqwest::blocking::get(&url)
            .with_context(|| format!("failed to fetch index from {}", url))?
            .error_for_status()
            .with_context(|| format!("HTTP error fetching index from {}", url))?;

        resp.json().with_context(|| "failed to parse index JSON")
    })?;

    if value.get("schema_version").and_then(|v| v.as_str()) != Some("v1") {
        bail!("invalid index: expected schema_version=v1");
    }

    Ok(value)
}

/// Resolves an index URL: appends /v1/index.json if the URL doesn't end in .json.
pub fn resolve_index_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.ends_with(".json") {
        trimmed.to_string()
    } else {
        format!("{}/v1/index.json", trimmed)
    }
}

/// Selects the best matching plugin entry from an index value.
/// Returns the selected entry as a JSON object.
pub fn select_plugin_entry<'a>(
    index: &'a serde_json::Value,
    id: &str,
    version: Option<&str>,
) -> Result<&'a serde_json::Value> {
    let plugins = index
        .get("plugins")
        .and_then(|p| p.as_array())
        .ok_or_else(|| anyhow::anyhow!("index missing 'plugins' array"))?;

    let candidates: Vec<&serde_json::Value> = plugins
        .iter()
        .filter(|p| p.get("id").and_then(|v| v.as_str()) == Some(id))
        .filter(|p| {
            if let Some(v) = version {
                p.get("version").and_then(|vv| vv.as_str()) == Some(v)
            } else {
                true
            }
        })
        .collect();

    if candidates.is_empty() {
        if let Some(v) = version {
            bail!("plugin not found in index: id={} version={}", id, v);
        } else {
            bail!("plugin not found in index: id={}", id);
        }
    }

    // Select the highest semver version.
    let best = candidates
        .into_iter()
        .max_by(|a, b| {
            let va = a.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0");
            let vb = b.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0");
            compare_semver(va, vb)
        })
        .unwrap();

    Ok(best)
}

pub fn compare_semver_pub(a: &str, b: &str) -> std::cmp::Ordering {
    compare_semver(a, b)
}

fn compare_semver(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| {
        let base = s.split('+').next().unwrap_or(s);
        let base = base.split('-').next().unwrap_or(base);
        let parts: Vec<u64> = base.split('.').map(|p| p.parse().unwrap_or(0)).collect();
        parts
    };
    parse(a).cmp(&parse(b))
}

/// Downloads a plugin tarball to `dest_path`, verifying sha256.
pub fn download_tarball(package_url: &str, expected_sha256: &str, dest_path: &Path) -> Result<()> {
    download_and_verify(package_url, expected_sha256, dest_path)
}

/// Fetches a trust policy JSON document from a URL and returns the parsed value.
pub fn fetch_trust_policy(url: &str) -> Result<serde_json::Value> {
    let url = url.to_string();
    run_blocking_http(move || {
        let resp = reqwest::blocking::get(&url)
            .with_context(|| format!("failed to fetch trust policy from {}", url))?
            .error_for_status()
            .with_context(|| format!("HTTP error fetching trust policy from {}", url))?;
        resp.json()
            .with_context(|| "failed to parse trust policy JSON")
    })
}

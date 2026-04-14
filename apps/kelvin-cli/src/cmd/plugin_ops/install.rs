use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use super::download;

/// Installs a plugin from a local tarball. Mirrors plugin-install.sh.
pub fn install_package(tarball: &Path, plugin_home: &Path, force: bool) -> Result<()> {
    let work_dir = tempdir()?;
    extract_tarball(tarball, &work_dir)?;

    let manifest_path = work_dir.join("plugin.json");
    let payload_dir = work_dir.join("payload");

    if !manifest_path.exists() {
        bail!("invalid package: missing plugin.json");
    }
    if !payload_dir.exists() {
        bail!("invalid package: missing payload/ directory");
    }

    let manifest_bytes = std::fs::read(&manifest_path).context("failed to read plugin.json")?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).context("failed to parse plugin.json")?;

    let plugin_id = manifest
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("plugin.json missing 'id'"))?;
    let plugin_name = manifest
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(plugin_id);
    let plugin_version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("plugin.json missing 'version'"))?;
    let api_version = manifest
        .get("api_version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let entrypoint_rel = manifest
        .get("entrypoint")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("plugin.json missing 'entrypoint'"))?;
    let capabilities = manifest
        .get("capabilities")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("plugin.json missing 'capabilities'"))?;

    if capabilities.is_empty() {
        bail!("invalid plugin.json: capabilities must contain at least one entry");
    }

    // Validate entrypoint path — no absolute paths, no `..` components.
    if entrypoint_rel.starts_with('/') || entrypoint_rel.contains("..") {
        bail!("invalid plugin.json: entrypoint must be a safe relative path");
    }

    let entrypoint_abs = payload_dir.join(entrypoint_rel);
    if !entrypoint_abs.exists() {
        bail!(
            "invalid package: entrypoint file not found: payload/{}",
            entrypoint_rel
        );
    }

    // Verify sha256 if present.
    if let Some(expected_sha) = manifest
        .get("entrypoint_sha256")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let actual_sha = sha256_file(&entrypoint_abs)?;
        if actual_sha != expected_sha {
            bail!(
                "checksum mismatch for entrypoint. expected={} actual={}",
                expected_sha,
                actual_sha
            );
        }
    }

    // Quality tier warnings.
    let quality_tier = manifest
        .get("quality_tier")
        .and_then(|v| v.as_str())
        .unwrap_or("unsigned_local");
    let publisher = manifest
        .get("publisher")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    match quality_tier {
        "unsigned_local" => {
            eprintln!(
                "WARNING: installing unsigned_local plugin '{}@{}'.",
                plugin_id, plugin_version
            );
            eprintln!("         This package is not signature-verified and should be treated as local development content.");
        }
        "signed_community" => {
            eprintln!(
                "WARNING: installing signed_community plugin '{}@{}' from publisher '{}'.",
                plugin_id, plugin_version, publisher
            );
            eprintln!("         Signature trust is enforced later by Kelvin runtime policy, not by this installer alone.");
        }
        _ => {}
    }

    let install_dir = plugin_home.join(plugin_id).join(plugin_version);
    let current_link = plugin_home.join(plugin_id).join("current");

    if install_dir.exists() && !force {
        bail!(
            "plugin already installed at {}. Use --force to overwrite.",
            install_dir.display()
        );
    }

    std::fs::create_dir_all(plugin_home.join(plugin_id))?;
    if install_dir.exists() {
        std::fs::remove_dir_all(&install_dir)?;
    }
    std::fs::create_dir_all(&install_dir)?;

    // Copy plugin.json and payload/.
    std::fs::copy(&manifest_path, install_dir.join("plugin.json"))?;
    copy_dir_all(&payload_dir, &install_dir.join("payload"))?;

    // Copy signature if present.
    let sig_path = work_dir.join("plugin.sig");
    if sig_path.exists() {
        std::fs::copy(&sig_path, install_dir.join("plugin.sig"))?;
    }

    // Atomic symlink (or dir copy on Windows fallback).
    update_current_link(&current_link, plugin_version)?;

    println!("Installed plugin:");
    println!("  id:          {}", plugin_id);
    println!("  name:        {}", plugin_name);
    println!("  version:     {}", plugin_version);
    println!("  api_version: {}", api_version);
    println!("  path:        {}", install_dir.display());
    println!(
        "  current:     {} -> {}",
        current_link.display(),
        plugin_version
    );

    Ok(())
}

/// Installs a plugin from the remote index. Mirrors plugin-index-install.sh.
pub fn install_from_index(
    plugin_id: &str,
    version: Option<&str>,
    plugin_home: &Path,
    index_url: &str,
    force: bool,
) -> Result<()> {
    let index = download::fetch_index(index_url)?;
    let entry = download::select_plugin_entry(&index, plugin_id, version)?;

    let package_url = entry
        .get("package_url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("index entry missing 'package_url'"))?;
    let expected_sha = entry
        .get("sha256")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("index entry missing 'sha256'"))?;
    let selected_version = entry
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("index entry missing 'version'"))?;

    eprintln!(
        "[kelvin] installing plugin id={} version={}",
        plugin_id, selected_version
    );

    let work_dir = tempdir()?;
    let tarball_path = work_dir.join("plugin.tar.gz");
    download::download_tarball(package_url, expected_sha, &tarball_path)?;

    install_package(&tarball_path, plugin_home, force)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn tempdir() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "kelvin-plugin-{}",
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("tmp")
    ));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn extract_tarball(tarball: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(tarball)
        .with_context(|| format!("failed to open tarball {}", tarball.display()))?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    archive.set_preserve_permissions(false);
    archive.set_ignore_zeros(true);

    for entry in archive.entries().context("failed to read tarball entries")? {
        let mut entry = entry.context("failed to read tarball entry")?;
        let entry_path = entry.path().context("invalid entry path")?;

        // Skip AppleDouble files (._*) and .DS_Store.
        let file_name = entry_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if file_name.starts_with("._") || file_name == ".DS_Store" {
            continue;
        }

        let out_path = dest.join(&entry_path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry
            .unpack(&out_path)
            .with_context(|| format!("failed to extract {}", out_path.display()))?;
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("failed to open {} for hashing", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn update_current_link(current_link: &Path, version: &str) -> Result<()> {
    // Remove existing symlink/dir if present.
    if current_link.exists() || current_link.is_symlink() {
        if current_link.is_dir() && !current_link.is_symlink() {
            std::fs::remove_dir_all(current_link)?;
        } else {
            std::fs::remove_file(current_link)?;
        }
    }
    std::os::unix::fs::symlink(version, current_link)
        .with_context(|| format!("failed to create symlink {}", current_link.display()))?;
    Ok(())
}

#[cfg(windows)]
fn update_current_link(current_link: &Path, version: &str) -> Result<()> {
    let parent = current_link
        .parent()
        .ok_or_else(|| anyhow::anyhow!("current_link has no parent"))?;
    let version_dir = parent.join(version);

    if current_link.exists() {
        std::fs::remove_dir_all(current_link)?;
    }

    // Try junction first; fall back to copy.
    if std::os::windows::fs::symlink_dir(&version_dir, current_link).is_err() {
        copy_dir_all(&version_dir, current_link)?;
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn update_current_link(current_link: &Path, version: &str) -> Result<()> {
    let parent = current_link.parent().unwrap();
    let version_dir = parent.join(version);
    if current_link.exists() {
        std::fs::remove_dir_all(current_link)?;
    }
    copy_dir_all(&version_dir, current_link)?;
    Ok(())
}

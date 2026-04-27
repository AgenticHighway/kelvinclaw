use std::io::Read;
use std::path::Path;

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use super::download;

/// Installs a plugin from a local tarball. Mirrors plugin-install.sh.
pub fn install_package(
    tarball: &Path,
    plugin_home: &Path,
    force: bool,
    strict: bool,
) -> Result<()> {
    let work_dir = tempdir()?;
    extract_tarball(tarball, &work_dir)?;
    install_from_extracted_dir(&work_dir, plugin_home, force, strict)
}

/// Installs a plugin from an already-extracted plugin directory.
///
/// The directory must contain `plugin.json` and `payload/` at its root — the
/// same layout that results from unpacking a plugin tarball. This is used by
/// the Docker init container to install locally-built plugins baked into the
/// image without re-packaging them.
pub fn install_from_dir(src: &Path, plugin_home: &Path, force: bool, strict: bool) -> Result<()> {
    if !src.is_dir() {
        bail!("not a directory: {}", src.display());
    }
    install_from_extracted_dir(src, plugin_home, force, strict)
}

/// Core install logic operating on an already-extracted plugin directory.
fn install_from_extracted_dir(
    src: &Path,
    plugin_home: &Path,
    force: bool,
    strict: bool,
) -> Result<()> {
    let manifest_path = src.join("plugin.json");
    let payload_dir = src.join("payload");

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

    if strict {
        verify_strict_signature(src, &manifest_bytes, plugin_id, plugin_version)?;
    }

    let install_dir = plugin_home.join(plugin_id).join(plugin_version);
    let current_link = plugin_home.join(plugin_id).join("current");

    if install_dir.exists() && !force {
        eprintln!(
            "[kelvin] plugin {}@{} already installed, skipping (use --force to reinstall)",
            plugin_id, plugin_version
        );
        return Ok(());
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
    let sig_path = src.join("plugin.sig");
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
    strict: bool,
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

    let install_dir = plugin_home.join(plugin_id).join(selected_version);
    if install_dir.exists() && !force {
        eprintln!(
            "[kelvin] plugin {}@{} already installed, skipping (use --force to reinstall)",
            plugin_id, selected_version
        );
        return Ok(());
    }

    eprintln!(
        "[kelvin] installing plugin id={} version={}",
        plugin_id, selected_version
    );

    let work_dir = tempdir()?;
    let tarball_path = work_dir.join("plugin.tar.gz");
    download::download_tarball(package_url, expected_sha, &tarball_path)?;

    install_package(&tarball_path, plugin_home, force, strict)?;

    // Merge publisher trust entries from the index if a trust_policy_url is present.
    if let Some(trust_url) = entry.get("trust_policy_url").and_then(|v| v.as_str()) {
        if !trust_url.is_empty() {
            if let Err(e) = merge_trust_policy(trust_url) {
                eprintln!(
                    "[kelvin] warning: could not merge trust policy from {}: {}",
                    trust_url, e
                );
            }
        }
    }

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

struct TempInstallDir(tempfile::TempDir);

impl std::ops::Deref for TempInstallDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.0.path()
    }
}

impl AsRef<Path> for TempInstallDir {
    fn as_ref(&self) -> &Path {
        self.0.path()
    }
}

fn tempdir() -> Result<TempInstallDir> {
    let dir = tempfile::Builder::new()
        .prefix("kelvin-plugin-")
        .tempdir()?;
    Ok(TempInstallDir(dir))
}
fn extract_tarball(tarball: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(tarball)
        .with_context(|| format!("failed to open tarball {}", tarball.display()))?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    archive.set_preserve_permissions(false);
    archive.set_ignore_zeros(true);

    for entry in archive
        .entries()
        .context("failed to read tarball entries")?
    {
        let mut entry = entry.context("failed to read tarball entry")?;
        let entry_path = entry.path().context("invalid entry path")?.into_owned();

        // Skip AppleDouble files (._*) and .DS_Store.
        let file_name = entry_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if file_name.starts_with("._") || file_name == ".DS_Store" {
            continue;
        }

        let unpacked = entry
            .unpack_in(dest)
            .with_context(|| format!("failed to extract {}", entry_path.display()))?;
        if !unpacked {
            bail!(
                "refusing to extract tar entry outside destination: {}",
                entry_path.display()
            );
        }
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

/// Fetches a trust policy URL and merges its publishers into the local trust policy file.
///
/// Merge rules (per index schema):
/// - `require_signature` = base && incoming (stays strict if either side is strict)
/// - `publishers` merged by `id` (incoming entry wins for duplicates)
fn merge_trust_policy(trust_url: &str) -> Result<()> {
    let incoming: serde_json::Value = download::fetch_trust_policy(trust_url)?;

    let trust_path = crate::paths::trust_policy_path();

    // Read existing policy, or start with a permissive default.
    let mut base: serde_json::Value = if trust_path.exists() {
        let bytes = std::fs::read(&trust_path)
            .with_context(|| format!("failed to read {}", trust_path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", trust_path.display()))?
    } else {
        serde_json::json!({"require_signature": false, "publishers": []})
    };

    // Merge require_signature: base && incoming.
    // A plugin's index trust policy cannot escalate the user's local setting;
    // strict is only preserved when both sides are strict.
    let base_strict = base
        .get("require_signature")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let incoming_strict = incoming
        .get("require_signature")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    base["require_signature"] = serde_json::Value::Bool(base_strict && incoming_strict);

    // Merge publishers by id (incoming wins for duplicates).
    let base_pubs = base
        .get_mut("publishers")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow::anyhow!("trust policy missing 'publishers' array"))?;

    if let Some(incoming_pubs) = incoming.get("publishers").and_then(|v| v.as_array()) {
        for incoming_pub in incoming_pubs {
            let id = incoming_pub
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // Replace existing entry with same id, or append.
            if let Some(pos) = base_pubs
                .iter()
                .position(|p| p.get("id").and_then(|v| v.as_str()) == Some(id))
            {
                base_pubs[pos] = incoming_pub.clone();
            } else {
                base_pubs.push(incoming_pub.clone());
            }
        }
    }

    if let Some(parent) = trust_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let merged = serde_json::to_string_pretty(&base).context("failed to serialize trust policy")?;
    std::fs::write(&trust_path, merged)
        .with_context(|| format!("failed to write {}", trust_path.display()))?;

    eprintln!("[kelvin] merged trust policy: {}", trust_path.display());
    Ok(())
}

/// Strict-mode signature verification for `kelvin plugin install --strict`.
///
/// Checks that plugin.sig exists, the publisher is trusted, and the signature
/// is a valid Ed25519 signature over the manifest bytes.
fn verify_strict_signature(
    src: &Path,
    manifest_bytes: &[u8],
    plugin_id: &str,
    plugin_version: &str,
) -> Result<()> {
    let sig_path = src.join("plugin.sig");
    if !sig_path.exists() {
        bail!(
            "strict install rejected: plugin '{}' is missing plugin.sig",
            plugin_id
        );
    }

    let signature_text = std::fs::read_to_string(&sig_path)
        .with_context(|| format!("failed to read plugin.sig for {}", plugin_id))?;
    let signature_base64 = signature_text.trim();
    if signature_base64.is_empty() {
        bail!(
            "strict install rejected: plugin '{}' has empty plugin.sig",
            plugin_id
        );
    }
    let signature_bytes = STANDARD
        .decode(signature_base64)
        .with_context(|| format!("invalid plugin.sig base64 for {}", plugin_id))?;
    let signature = Signature::from_slice(&signature_bytes)
        .with_context(|| format!("invalid ed25519 signature for {}", plugin_id))?;

    // Load trust policy.
    let trust_path = crate::paths::trust_policy_path();
    let trust_policy: serde_json::Value = if trust_path.exists() {
        let bytes = std::fs::read(&trust_path)
            .with_context(|| format!("failed to read trust policy {}", trust_path.display()))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse trust policy {}", trust_path.display()))?
    } else {
        bail!(
            "strict install rejected: no trust policy found at {}. \
             Run 'kelvin init' or add the publisher's key manually.",
            trust_path.display()
        );
    };

    // Get publisher from manifest.
    let manifest: serde_json::Value = serde_json::from_slice(manifest_bytes)
        .context("failed to re-parse plugin.json for signature verification")?;
    let publisher = manifest
        .get("publisher")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "strict install rejected: plugin '{}' missing 'publisher' field in manifest",
                plugin_id
            )
        })?;

    // Find publisher's public key.
    let publishers = trust_policy
        .get("publishers")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("trust policy missing 'publishers' array"))?;

    let pub_key_b64 = publishers
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(publisher))
        .and_then(|p| p.get("ed25519_public_key").and_then(|v| v.as_str()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "strict install rejected: publisher '{}' is not in the trust policy. \
                 Add the key with 'kelvin plugin trust add {} --key <path>'.",
                publisher,
                publisher
            )
        })?;

    let pub_key_bytes = STANDARD
        .decode(pub_key_b64)
        .with_context(|| format!("invalid base64 public key for publisher {}", publisher))?;
    let pub_key_array: [u8; 32] = pub_key_bytes.try_into().map_err(|_| {
        anyhow::anyhow!(
            "invalid ed25519 public key length for publisher {} (expected 32 bytes)",
            publisher
        )
    })?;
    let verifying_key = VerifyingKey::from_bytes(&pub_key_array)
        .with_context(|| format!("invalid ed25519 public key for publisher {}", publisher))?;

    verifying_key
        .verify(manifest_bytes, &signature)
        .with_context(|| {
            format!(
                "strict install rejected: signature verification failed for {}@{} from publisher '{}'",
                plugin_id, plugin_version, publisher
            )
        })?;

    eprintln!(
        "[kelvin] strict-mode signature verified: {}@{} from publisher '{}'",
        plugin_id, plugin_version, publisher
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn temp_home() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("kelvin-cli-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn make_plugin_dir(
        base: &Path,
        manifest: &serde_json::Value,
        signing_key: Option<&SigningKey>,
    ) -> std::path::PathBuf {
        let dir = base.join("plugin");
        let payload = dir.join("payload");
        std::fs::create_dir_all(&payload).expect("create payload");
        std::fs::write(payload.join("entry.wasm"), b"wasm").expect("write wasm");

        let manifest_bytes = serde_json::to_vec_pretty(manifest).expect("serialize manifest");
        std::fs::write(dir.join("plugin.json"), &manifest_bytes).expect("write manifest");

        if let Some(key) = signing_key {
            let sig = key.sign(&manifest_bytes);
            let sig_b64 = STANDARD.encode(sig.to_bytes());
            std::fs::write(dir.join("plugin.sig"), sig_b64).expect("write sig");
        }
        dir
    }

    fn make_trust_policy(
        home: &Path,
        publisher_id: &str,
        public_key_b64: &str,
    ) -> std::path::PathBuf {
        let policy = serde_json::json!({
            "require_signature": false,
            "publishers": [
                { "id": publisher_id, "ed25519_public_key": public_key_b64 }
            ]
        });
        let path = home.join("trusted_publishers.json");
        std::fs::write(&path, serde_json::to_vec_pretty(&policy).unwrap()).expect("write policy");
        path
    }

    #[test]
    fn strict_verify_accepts_valid_signature() {
        let _guard = env_lock().lock().expect("lock env");
        let home = temp_home();
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let pub_key_b64 = STANDARD.encode(signing_key.verifying_key().as_bytes());
        let policy_path = make_trust_policy(&home, "acme", &pub_key_b64);
        unsafe { std::env::set_var("KELVIN_TRUST_POLICY_PATH", &policy_path) };

        let manifest = serde_json::json!({
            "id": "acme.echo",
            "name": "Echo",
            "version": "1.0.0",
            "api_version": "1.0.0",
            "capabilities": ["tool_provider"],
            "runtime": "wasm_tool_v1",
            "entrypoint": "entry.wasm",
            "publisher": "acme",
        });
        let dir = make_plugin_dir(&home, &manifest, Some(&signing_key));
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();

        assert!(verify_strict_signature(&dir, &manifest_bytes, "acme.echo", "1.0.0").is_ok());
    }

    #[test]
    fn strict_verify_rejects_missing_sig() {
        let _guard = env_lock().lock().expect("lock env");
        let home = temp_home();
        let signing_key = SigningKey::from_bytes(&[2u8; 32]);
        let pub_key_b64 = STANDARD.encode(signing_key.verifying_key().as_bytes());
        let policy_path = make_trust_policy(&home, "acme", &pub_key_b64);
        unsafe { std::env::set_var("KELVIN_TRUST_POLICY_PATH", &policy_path) };

        let manifest = serde_json::json!({
            "id": "acme.echo",
            "name": "Echo",
            "version": "1.0.0",
            "api_version": "1.0.0",
            "capabilities": ["tool_provider"],
            "runtime": "wasm_tool_v1",
            "entrypoint": "entry.wasm",
            "publisher": "acme",
        });
        let dir = make_plugin_dir(&home, &manifest, None);
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();

        let err = verify_strict_signature(&dir, &manifest_bytes, "acme.echo", "1.0.0")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("missing plugin.sig"),
            "expected missing sig error, got: {err}"
        );
    }

    #[test]
    fn strict_verify_rejects_untrusted_publisher() {
        let _guard = env_lock().lock().expect("lock env");
        let home = temp_home();
        let signing_key = SigningKey::from_bytes(&[3u8; 32]);
        let pub_key_b64 = STANDARD.encode(signing_key.verifying_key().as_bytes());
        let policy_path = make_trust_policy(&home, "acme", &pub_key_b64);
        unsafe { std::env::set_var("KELVIN_TRUST_POLICY_PATH", &policy_path) };

        let manifest = serde_json::json!({
            "id": "evil.echo",
            "name": "Evil",
            "version": "1.0.0",
            "api_version": "1.0.0",
            "capabilities": ["tool_provider"],
            "runtime": "wasm_tool_v1",
            "entrypoint": "entry.wasm",
            "publisher": "evilcorp",
        });
        let dir = make_plugin_dir(&home, &manifest, Some(&signing_key));
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();

        let err = verify_strict_signature(&dir, &manifest_bytes, "evil.echo", "1.0.0")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("not in the trust policy"),
            "expected untrusted publisher error, got: {err}"
        );
    }

    #[test]
    fn strict_verify_rejects_invalid_signature() {
        let _guard = env_lock().lock().expect("lock env");
        let home = temp_home();
        let signing_key = SigningKey::from_bytes(&[4u8; 32]);
        let wrong_key = SigningKey::from_bytes(&[5u8; 32]);
        let pub_key_b64 = STANDARD.encode(signing_key.verifying_key().as_bytes());
        let policy_path = make_trust_policy(&home, "acme", &pub_key_b64);
        unsafe { std::env::set_var("KELVIN_TRUST_POLICY_PATH", &policy_path) };

        let manifest = serde_json::json!({
            "id": "acme.echo",
            "name": "Echo",
            "version": "1.0.0",
            "api_version": "1.0.0",
            "capabilities": ["tool_provider"],
            "runtime": "wasm_tool_v1",
            "entrypoint": "entry.wasm",
            "publisher": "acme",
        });
        // Sign with wrong key
        let dir = make_plugin_dir(&home, &manifest, Some(&wrong_key));
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();

        let err = verify_strict_signature(&dir, &manifest_bytes, "acme.echo", "1.0.0")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("signature verification failed"),
            "expected invalid sig error, got: {err}"
        );
    }

    #[test]
    fn strict_verify_rejects_missing_publisher() {
        let _guard = env_lock().lock().expect("lock env");
        let home = temp_home();
        let signing_key = SigningKey::from_bytes(&[6u8; 32]);
        let pub_key_b64 = STANDARD.encode(signing_key.verifying_key().as_bytes());
        let policy_path = make_trust_policy(&home, "acme", &pub_key_b64);
        unsafe { std::env::set_var("KELVIN_TRUST_POLICY_PATH", &policy_path) };

        let manifest = serde_json::json!({
            "id": "acme.echo",
            "name": "Echo",
            "version": "1.0.0",
            "api_version": "1.0.0",
            "capabilities": ["tool_provider"],
            "runtime": "wasm_tool_v1",
            "entrypoint": "entry.wasm",
        });
        let dir = make_plugin_dir(&home, &manifest, Some(&signing_key));
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();

        let err = verify_strict_signature(&dir, &manifest_bytes, "acme.echo", "1.0.0")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("missing 'publisher' field"),
            "expected missing publisher error, got: {err}"
        );
    }

    #[test]
    fn strict_verify_rejects_missing_trust_policy() {
        let _guard = env_lock().lock().expect("lock env");
        let home = temp_home();
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);

        // Point to a non-existent trust policy file.
        let policy_path = home.join("no-such-policy.json");
        let _ = std::fs::remove_file(&policy_path);
        unsafe { std::env::set_var("KELVIN_TRUST_POLICY_PATH", &policy_path) };

        let manifest = serde_json::json!({
            "id": "acme.echo",
            "name": "Echo",
            "version": "1.0.0",
            "api_version": "1.0.0",
            "capabilities": ["tool_provider"],
            "runtime": "wasm_tool_v1",
            "entrypoint": "entry.wasm",
            "publisher": "acme",
        });
        let dir = make_plugin_dir(&home, &manifest, Some(&signing_key));
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();

        let err = verify_strict_signature(&dir, &manifest_bytes, "acme.echo", "1.0.0")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("no trust policy found"),
            "expected missing policy error, got: {err}"
        );
    }
}

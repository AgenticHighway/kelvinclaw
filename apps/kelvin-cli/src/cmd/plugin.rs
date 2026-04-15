use anyhow::{bail, Context, Result};

use crate::cli::{PluginCmd, PluginInstallArgs, PluginUninstallArgs, PluginUpdateArgs};
use crate::paths;

const DEFAULT_INDEX_URL: &str =
    "https://raw.githubusercontent.com/AgenticHighway/kelvinclaw-plugins/main/index.json";

pub fn run(sub: PluginCmd) -> Result<()> {
    match sub {
        PluginCmd::Install(args) => cmd_install(args),
        PluginCmd::Uninstall(args) => cmd_uninstall(args),
        PluginCmd::Update(args) => cmd_update(args),
        PluginCmd::Search { query } => cmd_search(query.as_deref()),
        PluginCmd::Info { id } => cmd_info(&id),
        PluginCmd::List => cmd_list(),
        PluginCmd::Status => cmd_status(),
    }
}

fn index_url() -> String {
    std::env::var("KELVIN_PLUGIN_INDEX_URL").unwrap_or_else(|_| DEFAULT_INDEX_URL.to_string())
}

fn cmd_install(args: PluginInstallArgs) -> Result<()> {
    let plugin_home = paths::plugin_home();
    std::fs::create_dir_all(&plugin_home)?;

    if let Some(dir) = args.from_dir {
        if !dir.exists() {
            bail!("directory not found: {}", dir.display());
        }
        super::plugin_ops::install_from_dir(&dir, &plugin_home, args.force)?;
        return Ok(());
    }

    if let Some(pkg) = args.package {
        // Local package install.
        if !pkg.exists() {
            bail!("package not found: {}", pkg.display());
        }
        super::plugin_ops::install_package(&pkg, &plugin_home, args.force)?;
        return Ok(());
    }

    let id = args.id.ok_or_else(|| {
        anyhow::anyhow!("provide a plugin <id>, --package <path>, or --from-dir <path>")
    })?;
    let url = index_url();
    super::plugin_ops::install_from_index(
        &id,
        args.version.as_deref(),
        &plugin_home,
        &url,
        args.force,
    )
}

fn cmd_uninstall(args: PluginUninstallArgs) -> Result<()> {
    let plugin_dir = paths::plugin_home().join(&args.id);
    if !plugin_dir.exists() {
        bail!("plugin not installed: {}", args.id);
    }

    if !args.yes {
        if !crate::tty::is_interactive() {
            bail!("pass --yes to confirm uninstall in non-interactive mode");
        }
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!("Uninstall plugin '{}'?", args.id))
            .default(false)
            .interact()
            .context("prompt failed")?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    std::fs::remove_dir_all(&plugin_dir)
        .with_context(|| format!("failed to remove {}", plugin_dir.display()))?;
    println!("Uninstalled plugin: {}", args.id);
    Ok(())
}

fn cmd_update(args: PluginUpdateArgs) -> Result<()> {
    let plugin_home = paths::plugin_home();
    let url = index_url();
    let index = super::plugin_ops::download::fetch_index(&url)?;
    let plugins = index
        .get("plugins")
        .and_then(|p| p.as_array())
        .ok_or_else(|| anyhow::anyhow!("index missing 'plugins' array"))?;

    let installed = list_installed_plugins()?;
    if installed.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }

    let mut updated = 0u32;
    for (id, current_version) in &installed {
        if let Some(filter) = &args.id {
            if id != filter {
                continue;
            }
        }

        // Find latest in index.
        let best = plugins
            .iter()
            .filter(|p| p.get("id").and_then(|v| v.as_str()) == Some(id.as_str()))
            .max_by(|a, b| {
                let va = a.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0");
                let vb = b.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0");
                super::plugin_ops::download::compare_semver_pub(va, vb)
            });

        let Some(entry) = best else { continue };
        let latest_version = match entry.get("version").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => continue,
        };

        if latest_version == current_version {
            println!("{}: up to date ({})", id, current_version);
            continue;
        }

        println!("{}: {} → {}", id, current_version, latest_version);
        if args.dry_run {
            continue;
        }

        let package_url = match entry.get("package_url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => {
                eprintln!("  skipping: index entry missing package_url");
                continue;
            }
        };
        let expected_sha = match entry.get("sha256").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                eprintln!("  skipping: index entry missing sha256");
                continue;
            }
        };

        let tmp = std::env::temp_dir().join(format!(
            "kelvin-update-{}",
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("tmp")
        ));
        std::fs::create_dir_all(&tmp)?;
        let tarball = tmp.join("plugin.tar.gz");
        super::plugin_ops::download::download_tarball(package_url, expected_sha, &tarball)?;
        super::plugin_ops::install_package(&tarball, &plugin_home, true)?;
        let _ = std::fs::remove_dir_all(&tmp);
        updated += 1;
    }

    if args.dry_run {
        println!("[dry-run] no changes made");
    } else {
        println!("Updated {} plugin(s).", updated);
    }
    Ok(())
}

fn cmd_search(query: Option<&str>) -> Result<()> {
    let url = index_url();
    let index = super::plugin_ops::download::fetch_index(&url)?;
    let plugins = index
        .get("plugins")
        .and_then(|p| p.as_array())
        .ok_or_else(|| anyhow::anyhow!("index missing 'plugins'"))?;

    let q = query.unwrap_or("").to_lowercase();
    let mut found = false;
    for p in plugins {
        let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let version = p.get("version").and_then(|v| v.as_str()).unwrap_or("");
        let desc = p.get("description").and_then(|v| v.as_str()).unwrap_or("");

        if !q.is_empty()
            && !id.to_lowercase().contains(&q)
            && !name.to_lowercase().contains(&q)
            && !desc.to_lowercase().contains(&q)
        {
            continue;
        }
        println!(
            "{} {} — {}",
            id,
            version,
            if desc.is_empty() { name } else { desc }
        );
        found = true;
    }
    if !found {
        println!(
            "No plugins found{}",
            query
                .map(|q| format!(" matching '{}'", q))
                .unwrap_or_default()
        );
    }
    Ok(())
}

fn cmd_info(id: &str) -> Result<()> {
    let url = index_url();
    let index = super::plugin_ops::download::fetch_index(&url)?;
    let plugins = index
        .get("plugins")
        .and_then(|p| p.as_array())
        .ok_or_else(|| anyhow::anyhow!("index missing 'plugins'"))?;

    let matching: Vec<&serde_json::Value> = plugins
        .iter()
        .filter(|p| p.get("id").and_then(|v| v.as_str()) == Some(id))
        .collect();

    if matching.is_empty() {
        bail!("plugin not found in index: {}", id);
    }

    for entry in matching {
        println!("{}", serde_json::to_string_pretty(entry)?);
    }
    Ok(())
}

fn cmd_list() -> Result<()> {
    let installed = list_installed_plugins()?;
    if installed.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }
    for (id, version) in &installed {
        println!("{} {}", id, version);
    }
    Ok(())
}

fn cmd_status() -> Result<()> {
    let plugin_home = paths::plugin_home();
    let installed = list_installed_plugins()?;
    if installed.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }
    for (id, version) in &installed {
        let current = plugin_home.join(id).join("current");
        let wasm_present = current.join("payload").exists();
        let status = if wasm_present {
            "ok"
        } else {
            "missing payload"
        };
        println!("{} {} [{}]", id, version, status);
    }
    Ok(())
}

/// Returns (plugin_id, installed_version) pairs for all currently-installed plugins.
pub fn list_installed_plugins() -> Result<Vec<(String, String)>> {
    let plugin_home = paths::plugin_home();
    if !plugin_home.exists() {
        return Ok(vec![]);
    }
    let mut result = Vec::new();
    for entry in std::fs::read_dir(&plugin_home)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        let current = entry.path().join("current");
        if !current.exists() {
            continue;
        }
        // Read version from plugin.json inside current/.
        let manifest = current.join("plugin.json");
        if let Ok(bytes) = std::fs::read(&manifest) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(ver) = v.get("version").and_then(|v| v.as_str()) {
                    result.push((id, ver.to_string()));
                    continue;
                }
            }
        }
        // Fall back to reading the symlink target name.
        #[cfg(unix)]
        if let Ok(target) = std::fs::read_link(&current) {
            if let Some(name) = target.file_name() {
                result.push((id, name.to_string_lossy().to_string()));
                continue;
            }
        }
        result.push((id, "unknown".to_string()));
    }
    Ok(result)
}

use anyhow::Result;

use crate::cli::ServiceCmd;

pub fn run(sub: ServiceCmd) -> Result<()> {
    match sub {
        ServiceCmd::RenderSystemd => render_systemd(),
        ServiceCmd::RenderLaunchd => render_launchd(),
        ServiceCmd::InstallSystemd { unit } => install_systemd(&unit),
        ServiceCmd::InstallLaunchd { label } => install_launchd(&label),
    }
}

fn kelvin_bin() -> String {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "kelvin".to_string())
}

fn systemd_unit(_unit_name: &str) -> String {
    let bin = kelvin_bin();
    format!(
        r#"[Unit]
Description=KelvinClaw Gateway
After=network.target

[Service]
Type=simple
ExecStart={bin} gateway start --foreground
Restart=on-failure
RestartSec=5s
Environment=KELVIN_HOME=%h/.kelvinclaw

[Install]
WantedBy=default.target
"#,
        bin = bin
    )
}

#[cfg(target_os = "macos")]
fn launchd_plist(label: &str) -> String {
    let bin = kelvin_bin();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
        <string>gateway</string>
        <string>start</string>
        <string>--foreground</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>~/.kelvinclaw/logs/gateway.log</string>
    <key>StandardErrorPath</key>
    <string>~/.kelvinclaw/logs/gateway.log</string>
</dict>
</plist>
"#,
        label = label,
        bin = bin
    )
}

fn render_systemd() -> Result<()> {
    #[cfg(windows)]
    {
        eprintln!("systemd is not available on Windows. Use `kelvin gateway start` for daemon mode.");
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        print!("{}", systemd_unit("kelvin-gateway"));
        Ok(())
    }
}

fn render_launchd() -> Result<()> {
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("launchd is only available on macOS. Use `kelvin gateway start` for daemon mode.");
        std::process::exit(1);
    }
    #[cfg(target_os = "macos")]
    {
        print!("{}", launchd_plist("dev.kelvinclaw.gateway"));
        Ok(())
    }
}

fn install_systemd(unit_name: &str) -> Result<()> {
    #[cfg(windows)]
    {
        eprintln!("systemd is not available on Windows. Use `kelvin gateway start` for daemon mode.");
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        let xdg_config = std::env::var("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .expect("cannot determine home dir")
                    .join(".config")
            });

        let unit_dir = xdg_config.join("systemd").join("user");
        std::fs::create_dir_all(&unit_dir)?;

        let unit_file = unit_dir.join(format!("{}.service", unit_name));
        let content = systemd_unit(unit_name);
        std::fs::write(&unit_file, content)?;

        println!("Installed systemd unit: {}", unit_file.display());
        println!("Enable with:");
        println!("  systemctl --user daemon-reload");
        println!("  systemctl --user enable --now {}.service", unit_name);
        Ok(())
    }
}

#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn install_launchd(label: &str) -> Result<()> {
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("launchd is only available on macOS. Use `kelvin gateway start` for daemon mode.");
        std::process::exit(1);
    }
    #[cfg(target_os = "macos")]
    {
        let launch_agents = dirs::home_dir()
            .expect("cannot determine home dir")
            .join("Library")
            .join("LaunchAgents");
        std::fs::create_dir_all(&launch_agents)?;

        let plist_file = launch_agents.join(format!("{}.plist", label));
        std::fs::write(&plist_file, launchd_plist(label))?;

        println!("Installed launchd plist: {}", plist_file.display());
        println!("Load with:");
        println!("  launchctl load {}", plist_file.display());
        println!();
        println!("Tip: if you installed via Homebrew, `brew services start kelvin` is also available.");
        Ok(())
    }
}

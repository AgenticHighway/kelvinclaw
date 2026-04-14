use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "kelvin",
    about = "KelvinClaw — AI agent gateway CLI",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the full stack (memory + gateway) in the background, then open the TUI
    Start(StartArgs),
    /// Stop all background daemons (gateway + memory)
    Stop,
    /// Open the TUI (gateway must already be running)
    Tui(TuiArgs),
    /// Manage the gateway daemon
    Gateway {
        #[command(subcommand)]
        sub: GatewayCmd,
    },
    /// Manage the memory controller daemon
    Memory {
        #[command(subcommand)]
        sub: MemoryCmd,
    },
    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        sub: PluginCmd,
    },
    /// Manage plugins (alias for `plugin`)
    #[command(name = "kpm")]
    Kpm {
        #[command(subcommand)]
        sub: PluginCmd,
    },
    /// Interactive first-run setup
    Init(InitArgs),
    /// Offline diagnostics
    Medkit(MedkitArgs),
    /// WebSocket probe of running gateway
    Doctor,
    /// Install or render system service files
    Service {
        #[command(subcommand)]
        sub: ServiceCmd,
    },
    /// Print shell completion scripts
    Completions(CompletionsArgs),
}

// ── start ─────────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct StartArgs {
    /// Start gateway without the memory controller
    #[arg(long)]
    pub no_memory: bool,
}

// ── tui ───────────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct TuiArgs {
    /// Gateway WebSocket URL (default: ws://127.0.0.1:34617)
    #[arg(long, env = "KELVIN_GATEWAY_URL")]
    pub gateway_url: Option<String>,
    /// Session ID to join
    #[arg(long)]
    pub session: Option<String>,
}

// ── gateway ───────────────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum GatewayCmd {
    /// Start the gateway daemon
    Start(GatewayStartArgs),
    /// Stop the gateway daemon
    Stop,
    /// Restart the gateway daemon
    Restart(GatewayStartArgs),
    /// Show gateway status
    Status,
    /// Approve a Telegram pairing request
    ApprovePairing { code: String },
}

#[derive(Args, Debug)]
pub struct GatewayStartArgs {
    /// Run attached to the terminal instead of in the background
    #[arg(long)]
    pub foreground: bool,
    /// Additional arguments to pass to kelvin-gateway
    #[arg(last = true)]
    pub gateway_args: Vec<String>,
}

// ── memory ────────────────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum MemoryCmd {
    /// Start the memory controller daemon
    Start(MemoryStartArgs),
    /// Stop the memory controller daemon
    Stop,
    /// Restart the memory controller daemon
    Restart(MemoryStartArgs),
    /// Show memory controller status
    Status,
}

#[derive(Args, Debug)]
pub struct MemoryStartArgs {
    /// Run attached to the terminal instead of in the background
    #[arg(long)]
    pub foreground: bool,
}

// ── plugin ────────────────────────────────────────────────────────────────────

#[derive(Subcommand, Debug, Clone)]
pub enum PluginCmd {
    /// Install a plugin
    #[command(disable_version_flag = true)]
    Install(PluginInstallArgs),
    /// Uninstall a plugin
    Uninstall(PluginUninstallArgs),
    /// Update installed plugins
    Update(PluginUpdateArgs),
    /// Search the plugin index
    Search { query: Option<String> },
    /// Show plugin details
    Info { id: String },
    /// List installed plugins
    List,
    /// Show plugin runtime status
    Status,
}

#[derive(Args, Debug, Clone)]
pub struct PluginInstallArgs {
    /// Plugin ID from the index (omit when using --package)
    pub id: Option<String>,
    /// Install a local tarball instead of downloading from index
    #[arg(long)]
    pub package: Option<std::path::PathBuf>,
    /// Specific version to install
    #[arg(long)]
    pub version: Option<String>,
    /// Overwrite an existing install
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug, Clone)]
pub struct PluginUninstallArgs {
    /// Plugin ID to remove
    pub id: String,
    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct PluginUpdateArgs {
    /// Update only this plugin (default: all)
    pub id: Option<String>,
    /// Show what would be updated without installing
    #[arg(long)]
    pub dry_run: bool,
}

// ── init ──────────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Also install shell completions after setup
    #[arg(long)]
    pub with_completions: bool,
}

// ── medkit ────────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct MedkitArgs {
    /// Output results as JSON
    #[arg(long)]
    pub json: bool,
    /// Attempt to fix problems automatically
    #[arg(long)]
    pub fix: bool,
}

// ── service ───────────────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum ServiceCmd {
    /// Install a systemd user unit
    InstallSystemd {
        #[arg(long, default_value = "kelvin-gateway")]
        unit: String,
    },
    /// Install a launchd agent plist (macOS)
    InstallLaunchd {
        #[arg(long, default_value = "dev.kelvinclaw.gateway")]
        label: String,
    },
    /// Print the systemd unit to stdout
    RenderSystemd,
    /// Print the launchd plist to stdout
    RenderLaunchd,
}

// ── completions ───────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    pub shell: clap_complete::Shell,
    /// Write completions to the default location instead of stdout
    #[arg(long)]
    pub write: bool,
}

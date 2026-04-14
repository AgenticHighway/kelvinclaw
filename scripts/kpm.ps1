$ErrorActionPreference = "Stop"

if (Test-Path (Join-Path $PSScriptRoot "bin\kelvin-gateway.exe")) {
    $RootDir = $PSScriptRoot
} else {
    $RootDir = Split-Path -Parent $PSScriptRoot
}
$DefaultKelvinHome = if ($env:KELVIN_HOME) { $env:KELVIN_HOME } else { Join-Path $HOME ".kelvinclaw" }

# ── dotenv loader ─────────────────────────────────────────────────────────────
$_KpmEnvPaths = @(
    (Join-Path $DefaultKelvinHome ".env.local"),
    (Join-Path $DefaultKelvinHome ".env"),
    (Join-Path (Get-Location).Path ".env.local"),
    (Join-Path (Get-Location).Path ".env")
)
function _KpmLoadDotenv {
    $Dotenv = @{}
    foreach ($F in $_KpmEnvPaths) {
        if (-not (Test-Path $F)) { continue }
        foreach ($Line in Get-Content $F) {
            $S = $Line.Split("#")[0].Trim()
            if ([string]::IsNullOrWhiteSpace($S)) { continue }
            if ($S -match '^export\s+') { $S = $S -replace '^export\s+', '' }
            if ($S -match '^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$') {
                $K = $Matches[1]; $V = $Matches[2].Trim()
                if ($V.Length -ge 2 -and (($V[0] -eq '"' -and $V[-1] -eq '"') -or ($V[0] -eq "'" -and $V[-1] -eq "'"))) { $V = $V.Substring(1, $V.Length - 2) }
                if (-not $Dotenv.ContainsKey($K)) { $Dotenv[$K] = $V }
            }
        }
    }
    return $Dotenv
}
$_KpmDotenv = _KpmLoadDotenv
function _KpmEnv([string]$Key, [string]$Default = "") {
    $V = [System.Environment]::GetEnvironmentVariable($Key)
    if ($V) { return $V }
    if ($_KpmDotenv.ContainsKey($Key)) { return $_KpmDotenv[$Key] }
    return $Default
}
# ──────────────────────────────────────────────────────────────────────────────

$KelvinHome      = _KpmEnv "KELVIN_HOME"              (Join-Path $HOME ".kelvinclaw")
$PluginHome      = _KpmEnv "KELVIN_PLUGIN_HOME"       (Join-Path $KelvinHome "plugins")
$TrustPolicyPath = _KpmEnv "KELVIN_TRUST_POLICY_PATH" (Join-Path $KelvinHome "trusted_publishers.json")
$IndexUrl        = _KpmEnv "KELVIN_PLUGIN_INDEX_URL"  ""

# ── helpers ───────────────────────────────────────────────────────────────────

function Show-Usage {
    @"
Usage: kpm <subcommand> [options]

Kelvin Plugin Manager — install and manage KelvinClaw plugins.

Subcommands:
  install <plugin-id> [--version <ver>] [--force]
                         Install a plugin from the index
  uninstall <plugin-id> [--yes]
                         Remove an installed plugin
  update [<plugin-id>] [--dry-run]
                         Update installed plugins to the latest index version
  search [<query>]       List available plugins from the index
  info <plugin-id>       Show detailed metadata for a plugin from the index
  list                   List installed plugins
  status                 Show current configuration and installed plugins

Options:
  -h, --help   Show this help

Environment:
  KELVIN_PLUGIN_INDEX_URL    Plugin index URL (required for install, search, info, update)
  KELVIN_MODEL_PROVIDER      Active model provider (informational in status output)
  KELVIN_HOME                State root (default: ~\.kelvinclaw)
  KELVIN_PLUGIN_HOME         Override plugin install root
  KELVIN_TRUST_POLICY_PATH   Override trust policy path

Config precedence:
  1. ~\.kelvinclaw\.env.local
  2. ~\.kelvinclaw\.env
  3. .\.env.local
  4. .\.env
"@
}

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command: $Name"
    }
}

function Require-IndexUrl {
    if (-not $IndexUrl) {
        throw "KELVIN_PLUGIN_INDEX_URL is required for this command"
    }
}

function Fetch-Index {
    Invoke-RestMethod -Uri $IndexUrl -TimeoutSec 15
}

function Plugin-CurrentVersion([string]$PluginId) {
    $CurrentDir = Join-Path (Join-Path $PluginHome $PluginId) "current"
    $ManifestPath = Join-Path $CurrentDir "plugin.json"
    if (-not (Test-Path $ManifestPath)) {
        return $null
    }
    $Manifest = Get-Content $ManifestPath -Raw | ConvertFrom-Json
    return $Manifest.version
}

function Install-PluginFromEntry {
    param(
        [object]$Entry,
        [switch]$Force
    )

    Require-Command "tar"

    $PluginId   = $Entry.id
    $Version    = $Entry.version
    $PackageUrl = $Entry.package_url
    $ExpectedSha = $Entry.sha256

    if (-not $PackageUrl) {
        throw "Plugin '$PluginId' has no package_url in index (build from source required)"
    }

    $CurrentVersion = Plugin-CurrentVersion $PluginId
    if ($CurrentVersion -eq $Version -and -not $Force) {
        Write-Host "$PluginId@$Version is already installed"
        return
    }

    Write-Host "Installing $PluginId@$Version"

    New-Item -ItemType Directory -Force -Path (Join-Path $PluginHome $PluginId) | Out-Null
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $TrustPolicyPath) | Out-Null

    $WorkDir    = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
    $PkgPath    = Join-Path $WorkDir "plugin.tar.gz"
    $ExtractDir = Join-Path $WorkDir "extract"
    New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

    try {
        Invoke-WebRequest -Uri $PackageUrl -OutFile $PkgPath
        if ($ExpectedSha) {
            $ActualSha = (Get-FileHash -Algorithm SHA256 -Path $PkgPath).Hash.ToLowerInvariant()
            if ($ActualSha -ne $ExpectedSha.ToLowerInvariant()) {
                throw "Checksum mismatch for $PluginId@$Version (got $ActualSha)"
            }
        }

        New-Item -ItemType Directory -Force -Path $ExtractDir | Out-Null
        & tar -xzf $PkgPath -C $ExtractDir

        $VersionDir = Join-Path (Join-Path $PluginHome $PluginId) $Version
        $CurrentDir = Join-Path (Join-Path $PluginHome $PluginId) "current"

        if (Test-Path $VersionDir) { Remove-Item -Recurse -Force $VersionDir }
        New-Item -ItemType Directory -Force -Path $VersionDir | Out-Null
        Copy-Item -Recurse -Force (Join-Path $ExtractDir "*") $VersionDir

        if (Test-Path $CurrentDir) { Remove-Item -Recurse -Force $CurrentDir }
        New-Item -ItemType Directory -Force -Path $CurrentDir | Out-Null
        Copy-Item -Recurse -Force (Join-Path $VersionDir "*") $CurrentDir

        Write-Host "Installed $PluginId@$Version"
    } finally {
        Remove-Item -Recurse -Force $WorkDir -ErrorAction SilentlyContinue
    }
}

# ── subcommands ───────────────────────────────────────────────────────────────

function Cmd-Install([string[]]$CmdArgs) {
    $PluginId = ""
    $PluginVersion = ""
    $Force = $false

    $i = 0
    if ($CmdArgs.Length -gt 0 -and -not $CmdArgs[0].StartsWith("-")) {
        $PluginId = $CmdArgs[0]
        $i = 1
    }
    while ($i -lt $CmdArgs.Length) {
        switch ($CmdArgs[$i]) {
            "--version" { $PluginVersion = $CmdArgs[++$i]; $i++ }
            "--force"   { $Force = $true; $i++ }
            default     { throw "Unknown argument: $($CmdArgs[$i])" }
        }
    }

    Require-IndexUrl

    # Interactive selection when no plugin id provided
    if (-not $PluginId) {
        if (-not [Environment]::UserInteractive) {
            throw "Plugin id is required in non-interactive mode`n  Usage: kpm install <plugin-id>"
        }
        Write-Host "Fetching available plugins from index..."
        $Index = Fetch-Index
        Write-Host ""
        Write-Host "Available plugins:"
        foreach ($P in $Index.plugins) {
            $Desc = if ($P.description) { $P.description } else { "" }
            Write-Host "  $($P.id)  $($P.version)  $Desc"
        }
        Write-Host ""
        $PluginId = (Read-Host "Enter plugin id").Trim()
    }

    if (-not $PluginId) {
        throw "No plugin id specified"
    }

    $Index = Fetch-Index
    $Entries = @($Index.plugins | Where-Object { $_.id -eq $PluginId })
    if ($Entries.Count -eq 0) {
        throw "Plugin not found in index: $PluginId"
    }

    if ($PluginVersion) {
        # When a version is requested, require an exact match in the index
        $MatchingEntries = @($Entries | Where-Object { $_.version -eq $PluginVersion })
        if ($MatchingEntries.Count -eq 0) {
            throw "Version not found in index: $PluginId@$PluginVersion"
        }
        if ($MatchingEntries.Count -gt 1) {
            throw "Multiple index entries found for $PluginId@$PluginVersion; refusing to choose arbitrarily"
        }
        # Use the exact matching entry from the index (preserving package_url and sha256)
        $Entry = $MatchingEntries[0]
    } else {
        # Pick the highest semver when multiple versions exist in the index
        $Entry = $Entries | Sort-Object {
            $v = $_.version -replace '[+\-].*$', ''
            try { [System.Version]$v } catch { [System.Version]"0.0.0" }
        } | Select-Object -Last 1
    }

    $env:KELVIN_PLUGIN_HOME       = $PluginHome
    $env:KELVIN_TRUST_POLICY_PATH = $TrustPolicyPath
    Install-PluginFromEntry -Entry $Entry -Force:$Force
}

function Cmd-Uninstall([string[]]$CmdArgs) {
    $PluginId = ""
    $Yes = $false

    $i = 0
    if ($CmdArgs.Length -gt 0 -and -not $CmdArgs[0].StartsWith("-")) {
        $PluginId = $CmdArgs[0]
        $i = 1
    }
    while ($i -lt $CmdArgs.Length) {
        switch ($CmdArgs[$i]) {
            { $_ -eq "--yes" -or $_ -eq "-y" } { $Yes = $true; $i++ }
            default { throw "Unknown argument: $($CmdArgs[$i])" }
        }
    }

    if (-not $PluginId) {
        throw "Plugin id is required`n  Usage: kpm uninstall <plugin-id>"
    }

    $PluginDir = Join-Path $PluginHome $PluginId
    if (-not (Test-Path $PluginDir)) {
        throw "Plugin not installed: $PluginId"
    }

    if (-not $Yes -and [Environment]::UserInteractive) {
        $Answer = Read-Host "Remove $PluginId from $PluginDir? [y/N]"
        if ($Answer -ne "y" -and $Answer -ne "Y") {
            Write-Host "Aborted."
            exit 0
        }
    }

    Remove-Item -Recurse -Force $PluginDir
    Write-Host "Removed $PluginId"
}

function Cmd-Update([string[]]$CmdArgs) {
    $PluginId = ""
    $DryRun = $false

    $i = 0
    if ($CmdArgs.Length -gt 0 -and -not $CmdArgs[0].StartsWith("-")) {
        $PluginId = $CmdArgs[0]
        $i = 1
    }
    while ($i -lt $CmdArgs.Length) {
        switch ($CmdArgs[$i]) {
            "--dry-run" { $DryRun = $true; $i++ }
            default     { throw "Unknown argument: $($CmdArgs[$i])" }
        }
    }

    Require-IndexUrl

    if (-not (Test-Path $PluginHome)) {
        Write-Host "No plugins installed."
        return
    }

    $Index = Fetch-Index
    $Updated = 0

    function Check-AndUpdate([string]$Id) {
        $InstalledVersion = Plugin-CurrentVersion $Id
        if (-not $InstalledVersion) { return }

        $MatchedEntries = @($Index.plugins | Where-Object { $_.id -eq $Id })
        if ($MatchedEntries.Count -eq 0) {
            Write-Host "  ${Id}: not found in index (skipping)"
            return
        }
        $Entry = $MatchedEntries | Sort-Object {
            $v = $_.version -replace '[+\-].*$', ''
            try { [System.Version]$v } catch { [System.Version]"0.0.0" }
        } | Select-Object -Last 1

        if ($InstalledVersion -eq $Entry.version) {
            Write-Host "  ${Id}: up to date ($InstalledVersion)"
            return
        }

        Write-Host "  ${Id}: $InstalledVersion -> $($Entry.version)"
        if ($DryRun) { return }

        $env:KELVIN_PLUGIN_HOME       = $PluginHome
        $env:KELVIN_TRUST_POLICY_PATH = $TrustPolicyPath
        Install-PluginFromEntry -Entry $Entry -Force
        $script:Updated++
    }

    if ($PluginId) {
        Check-AndUpdate $PluginId
    } else {
        foreach ($Dir in (Get-ChildItem -Path $PluginHome -Directory -ErrorAction SilentlyContinue)) {
            Check-AndUpdate $Dir.Name
        }
    }

    if ($DryRun) {
        Write-Host "(dry run — no changes made)"
    } elseif ($Updated -gt 0) {
        Write-Host "$Updated plugin(s) updated."
    } else {
        Write-Host "All plugins up to date."
    }
}

function Cmd-Search([string]$Query) {
    Require-IndexUrl
    $Index = Fetch-Index

    $Plugins = if ($Query) {
        $Index.plugins | Where-Object { $_.id -match [regex]::Escape($Query) -or ($_.name -and $_.name -match [regex]::Escape($Query)) }
    } else {
        $Index.plugins
    }

    if (-not $Plugins) {
        if ($Query) { Write-Host "No plugins matching: $Query" } else { Write-Host "No plugins found in index." }
        return
    }

    "{0,-30}  {1,-10}  {2}" -f "ID", "VERSION", "DESCRIPTION"
    "{0,-30}  {1,-10}  {2}" -f ("─" * 30), ("─" * 10), ("─" * 38)
    foreach ($P in $Plugins) {
        $Desc = if ($P.description) { $P.description } else { "(no description)" }
        if ($Desc.Length -gt 60) { $Desc = $Desc.Substring(0, 57) + "..." }
        "{0,-30}  {1,-10}  {2}" -f $P.id, $P.version, $Desc
    }
}

function Cmd-Info([string]$PluginId) {
    if (-not $PluginId) {
        throw "Plugin id is required`n  Usage: kpm info <plugin-id>"
    }
    Require-IndexUrl
    $Index = Fetch-Index
    $Entry = $Index.plugins | Where-Object { $_.id -eq $PluginId }
    if (-not $Entry) {
        throw "Plugin not found in index: $PluginId"
    }

    $InstalledVersion = Plugin-CurrentVersion $PluginId

    "id:           $($Entry.id)"
    "name:         $(if ($Entry.name) { $Entry.name } else { '(none)' })"
    "version:      $($Entry.version)"
    "installed:    $(if ($InstalledVersion) { $InstalledVersion } else { '(not installed)' })"
    "description:  $(if ($Entry.description) { $Entry.description } else { '(none)' })"
    "homepage:     $(if ($Entry.homepage) { $Entry.homepage } else { '(none)' })"
    "capabilities: $(if ($Entry.capabilities) { $Entry.capabilities -join ', ' } else { '(none)' })"
    "runtime:      $(if ($Entry.runtime) { $Entry.runtime } else { '(none)' })"
    "quality_tier: $(if ($Entry.quality_tier) { $Entry.quality_tier } else { '(none)' })"
    "sha256:       $(if ($Entry.sha256) { $Entry.sha256 } else { '(none)' })"
}

function Cmd-List {
    if (-not (Test-Path $PluginHome)) {
        Write-Host "No plugins installed (KELVIN_PLUGIN_HOME=$PluginHome)"
        return
    }
    $Found = $false
    foreach ($Dir in (Get-ChildItem -Path $PluginHome -Directory -ErrorAction SilentlyContinue)) {
        $Version = Plugin-CurrentVersion $Dir.Name
        if (-not $Version) { $Version = "(unknown)" }
        Write-Host "  $($Dir.Name)@$Version"
        $Found = $true
    }
    if (-not $Found) {
        Write-Host "No plugins installed (KELVIN_PLUGIN_HOME=$PluginHome)"
    }
}

function Cmd-Status {
    "KELVIN_HOME=$KelvinHome"
    "KELVIN_PLUGIN_HOME=$PluginHome"
    "KELVIN_TRUST_POLICY_PATH=$TrustPolicyPath"
    "KELVIN_MODEL_PROVIDER=$(if ($env:KELVIN_MODEL_PROVIDER) { $env:KELVIN_MODEL_PROVIDER } else { 'kelvin.echo' })"
    "KELVIN_PLUGIN_INDEX_URL=$(if ($IndexUrl) { $IndexUrl } else { '(not set)' })"
    ""
    Write-Host "Installed plugins:"
    Cmd-List
}

# ── dispatch ──────────────────────────────────────────────────────────────────

$AllArgs = $args
if ($AllArgs.Length -eq 0 -or $AllArgs[0] -eq "-h" -or $AllArgs[0] -eq "--help") {
    Show-Usage
    exit 0
}

$Subcommand = $AllArgs[0]
$SubArgs    = if ($AllArgs.Length -gt 1) { $AllArgs[1..($AllArgs.Length - 1)] } else { @() }

switch ($Subcommand) {
    "install"   { Cmd-Install $SubArgs }
    "uninstall" { Cmd-Uninstall $SubArgs }
    "update"    { Cmd-Update $SubArgs }
    "search"    { Cmd-Search ($SubArgs | Select-Object -First 1) }
    "info"      { Cmd-Info ($SubArgs | Select-Object -First 1) }
    "list"      { Cmd-List }
    "status"    { Cmd-Status }
    default     {
        Write-Error "Unknown subcommand: $Subcommand"
        Write-Host ""
        Show-Usage
        exit 1
    }
}

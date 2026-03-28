$ErrorActionPreference = "Stop"

if (Test-Path (Join-Path $PSScriptRoot "bin\kelvin-gateway.exe")) {
    $RootDir = $PSScriptRoot
} else {
    $RootDir = Split-Path -Parent $PSScriptRoot
}

# ── dotenv loader ─────────────────────────────────────────────────────────────
$_KgwEnvPaths = @(
    (Join-Path (Get-Location).Path ".env.local"),
    (Join-Path (Get-Location).Path ".env"),
    (Join-Path $HOME ".kelvinclaw\.env.local"),
    (Join-Path $HOME ".kelvinclaw\.env")
)
function _KgwLoadDotenv {
    $Dotenv = @{}
    foreach ($F in $_KgwEnvPaths) {
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
$_KgwDotenv = _KgwLoadDotenv
function _KgwEnv([string]$Key, [string]$Default = "") {
    $V = [System.Environment]::GetEnvironmentVariable($Key)
    if ($V) { return $V }
    if ($_KgwDotenv.ContainsKey($Key)) { return $_KgwDotenv[$Key] }
    return $Default
}
# ──────────────────────────────────────────────────────────────────────────────

$KelvinHome      = _KgwEnv "KELVIN_HOME"              (Join-Path $HOME ".kelvinclaw")
$PluginHome      = _KgwEnv "KELVIN_PLUGIN_HOME"       (Join-Path $KelvinHome "plugins")
$TrustPolicyPath = _KgwEnv "KELVIN_TRUST_POLICY_PATH" (Join-Path $KelvinHome "trusted_publishers.json")
$IndexUrl        = _KgwEnv "KELVIN_PLUGIN_INDEX_URL"  ""
$ModelProvider   = _KgwEnv "KELVIN_MODEL_PROVIDER"    "kelvin.echo"
$LogDir          = Join-Path $KelvinHome "logs"
$LogFile         = Join-Path $LogDir "gateway.log"
$ErrFile         = Join-Path $LogDir "gateway.err"
$GwPidFile         = Join-Path $KelvinHome "gateway.pid"
$GatewayBinary   = Join-Path $RootDir "bin\kelvin-gateway.exe"

# ── helpers ───────────────────────────────────────────────────────────────────

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command: $Name"
    }
}

function Get-GatewayProcess {
    if (-not (Test-Path $GwPidFile)) { return $null }
    $GwPid = [int](Get-Content $GwPidFile -Raw).Trim()
    $Proc = Get-Process -Id $GwPid -ErrorAction SilentlyContinue
    if ($Proc -and -not $Proc.HasExited) { return $Proc }
    return $null
}

function Format-Uptime([TimeSpan]$Span) {
    if ($Span.TotalDays -ge 1)  { return "$([int]$Span.TotalDays)d $($Span.Hours)h $($Span.Minutes)m" }
    if ($Span.TotalHours -ge 1) { return "$([int]$Span.TotalHours)h $($Span.Minutes)m $($Span.Seconds)s" }
    if ($Span.TotalMinutes -ge 1) { return "$([int]$Span.TotalMinutes)m $($Span.Seconds)s" }
    return "$([int]$Span.TotalSeconds)s"
}

function Plugin-IsInstalled([string]$PluginId) {
    $CurrentDir = Join-Path (Join-Path $PluginHome $PluginId) "current"
    return (Test-Path (Join-Path $CurrentDir "plugin.json"))
}

function Ensure-TrustPolicy {
    New-Item -ItemType Directory -Force -Path $PluginHome | Out-Null
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $TrustPolicyPath) | Out-Null
    $env:KELVIN_PLUGIN_HOME       = $PluginHome
    $env:KELVIN_TRUST_POLICY_PATH = $TrustPolicyPath
    if (-not (Test-Path $TrustPolicyPath)) {
        '{"require_signature":false,"publishers":[]}' | Set-Content -NoNewline $TrustPolicyPath
        Write-Host "[kelvin-gateway] wrote permissive trust policy: $TrustPolicyPath"
    }
}

function Ensure-Plugin {
    if (Plugin-IsInstalled $ModelProvider) { return }
    if (-not $IndexUrl) {
        throw "KELVIN_PLUGIN_INDEX_URL must be set to install '$ModelProvider'"
    }
    Require-Command "tar"
    Write-Host "[kelvin-gateway] fetching plugin index"
    $IndexJson = Invoke-RestMethod -Uri $IndexUrl -TimeoutSec 15
    $Entry = $IndexJson.plugins | Where-Object { $_.id -eq $ModelProvider }
    if (-not $Entry) { throw "Plugin not found in index: $ModelProvider" }
    if (-not $Entry.package_url) { throw "Plugin '$ModelProvider' has no package_url (build from source required)" }

    Write-Host "[kelvin-gateway] installing $ModelProvider@$($Entry.version)"
    New-Item -ItemType Directory -Force -Path (Join-Path $PluginHome $ModelProvider) | Out-Null

    $WorkDir    = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
    $PkgPath    = Join-Path $WorkDir "plugin.tar.gz"
    $ExtractDir = Join-Path $WorkDir "extract"
    New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null
    try {
        Invoke-WebRequest -Uri $Entry.package_url -OutFile $PkgPath
        if ($Entry.sha256) {
            $ActualSha = (Get-FileHash -Algorithm SHA256 -Path $PkgPath).Hash.ToLowerInvariant()
            if ($ActualSha -ne $Entry.sha256.ToLowerInvariant()) { throw "Checksum mismatch for $ModelProvider@$($Entry.version)" }
        }
        New-Item -ItemType Directory -Force -Path $ExtractDir | Out-Null
        & tar -xzf $PkgPath -C $ExtractDir
        $VersionDir = Join-Path (Join-Path $PluginHome $ModelProvider) $Entry.version
        $CurrentDir = Join-Path (Join-Path $PluginHome $ModelProvider) "current"
        if (Test-Path $VersionDir) { Remove-Item -Recurse -Force $VersionDir }
        New-Item -ItemType Directory -Force -Path $VersionDir | Out-Null
        Copy-Item -Recurse -Force (Join-Path $ExtractDir "*") $VersionDir
        if (Test-Path $CurrentDir) { Remove-Item -Recurse -Force $CurrentDir }
        New-Item -ItemType Directory -Force -Path $CurrentDir | Out-Null
        Copy-Item -Recurse -Force (Join-Path $VersionDir "*") $CurrentDir

        if ($Entry.trust_policy_url) {
            Write-Host "[kelvin-gateway] fetching trust policy: $($Entry.trust_policy_url)"
            $TrustTmp = Join-Path $WorkDir "trust-policy.json"
            Invoke-WebRequest -Uri $Entry.trust_policy_url -OutFile $TrustTmp
            if (-not (Test-Path $TrustPolicyPath)) {
                Copy-Item $TrustTmp $TrustPolicyPath
            } else {
                $Base     = Get-Content $TrustPolicyPath -Raw | ConvertFrom-Json
                $Incoming = Get-Content $TrustTmp        -Raw | ConvertFrom-Json
                $MergedPublishers = @(($Base.publishers + $Incoming.publishers) |
                    Group-Object id | ForEach-Object { $_.Group[-1] })
                $Merged = [ordered]@{
                    require_signature = ($Base.require_signature -and $Incoming.require_signature)
                    publishers        = $MergedPublishers
                }
                $Merged | ConvertTo-Json -Depth 10 | Set-Content -NoNewline $TrustPolicyPath
            }
        }
    } finally {
        Remove-Item -Recurse -Force $WorkDir -ErrorAction SilentlyContinue
    }
}

# ── usage ─────────────────────────────────────────────────────────────────────

function Show-Usage {
    @"
Usage: .\kelvin-gateway.cmd <subcommand> [options]

Lifecycle manager for the kelvin-gateway daemon.

Subcommands:
  start [--foreground] [-- <gateway-args>]
                   Start the gateway.
                   Default: daemon mode (background, PID file, log file).
                   --foreground: run attached to the terminal.
                   Pass gateway binary flags after --.
  stop             Stop the running gateway daemon.
  restart [-- <gateway-args>]
                   Stop (if running) and start the gateway.
  status           Show gateway status, PID, model provider, log path, uptime.
  -h, --help       Show this help.

State files:
  %KELVIN_HOME%\gateway.pid        PID of the running daemon
  %KELVIN_HOME%\logs\gateway.log   Daemon stdout (overwritten each run)
  %KELVIN_HOME%\logs\gateway.err   Daemon stderr (overwritten each run)

Environment:
  KELVIN_MODEL_PROVIDER      Model provider plugin id (default: kelvin.echo)
  KELVIN_PLUGIN_INDEX_URL    Plugin index URL (required to install model provider plugin)
  KELVIN_GATEWAY_TOKEN       Auth token for the gateway
  KELVIN_HOME                State root (default: ~\.kelvinclaw)
  KELVIN_PLUGIN_HOME         Override plugin install root
  KELVIN_TRUST_POLICY_PATH   Override trust policy path
"@
}

# ── subcommands ───────────────────────────────────────────────────────────────

function Cmd-Start([string[]]$CmdArgs) {
    $Foreground   = $false
    $GatewayArgs  = @()

    $i = 0
    while ($i -lt $CmdArgs.Length) {
        if ($CmdArgs[$i] -eq "--foreground") { $Foreground = $true; $i++ }
        elseif ($CmdArgs[$i] -eq "--")       { $GatewayArgs = $CmdArgs[($i+1)..($CmdArgs.Length-1)]; break }
        else { throw "Unknown option: $($CmdArgs[$i])" }
    }

    Ensure-TrustPolicy
    Ensure-Plugin

    $FullArgs = @("--model-provider", $ModelProvider) + $GatewayArgs

    if ($Foreground) {
        & $GatewayBinary @FullArgs
        exit $LASTEXITCODE
    }

    # Daemon mode
    if (Test-Path $GwPidFile) {
        $ExistingPid = [int](Get-Content $GwPidFile -Raw).Trim()
        $ExistingProc = Get-Process -Id $ExistingPid -ErrorAction SilentlyContinue
        if ($ExistingProc -and -not $ExistingProc.HasExited) {
            Write-Error "gateway is already running (pid=$ExistingPid)"
            Write-Host "log: $LogFile"
            exit 1
        }
        Write-Host "[kelvin-gateway] removing stale PID file (pid=$ExistingPid)"
        Remove-Item -Force $GwPidFile
    }

    New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
    $Process = Start-Process -FilePath $GatewayBinary -ArgumentList $FullArgs `
        -RedirectStandardOutput $LogFile -RedirectStandardError $ErrFile `
        -WindowStyle Hidden -PassThru
    [string]$Process.Id | Set-Content -NoNewline $GwPidFile
    Write-Host "[kelvin-gateway] started (pid=$($Process.Id))"
    Write-Host "[kelvin-gateway] log: $LogFile"
    Write-Host "[kelvin-gateway] pid: $GwPidFile"
}

function Cmd-Stop {
    if (-not (Test-Path $GwPidFile)) {
        Write-Error "gateway is not running (no PID file)"
        exit 1
    }

    $GwPid = [int](Get-Content $GwPidFile -Raw).Trim()
    $Proc = Get-Process -Id $GwPid -ErrorAction SilentlyContinue

    if (-not $Proc -or $Proc.HasExited) {
        Write-Host "[kelvin-gateway] not running (stale PID $GwPid); removing PID file"
        Remove-Item -Force $GwPidFile
        exit 0
    }

    Write-Host "[kelvin-gateway] stopping (pid=$GwPid)"
    Stop-Process -Id $GwPid -ErrorAction SilentlyContinue

    $Elapsed = 0
    while ($true) {
        Start-Sleep -Milliseconds 500
        $Elapsed += 500
        $Check = Get-Process -Id $GwPid -ErrorAction SilentlyContinue
        if (-not $Check -or $Check.HasExited) { break }
        if ($Elapsed -ge 3000) {
            Write-Host "[kelvin-gateway] process did not stop; sending SIGKILL"
            Stop-Process -Id $GwPid -Force -ErrorAction SilentlyContinue
            break
        }
    }

    Remove-Item -Force $GwPidFile -ErrorAction SilentlyContinue
    Write-Host "[kelvin-gateway] stopped"
}

function Cmd-Restart([string[]]$CmdArgs) {
    $Proc = Get-GatewayProcess
    if ($Proc) { Cmd-Stop }
    Cmd-Start $CmdArgs
}

function Cmd-Status {
    "KELVIN_HOME=$KelvinHome"
    "KELVIN_MODEL_PROVIDER=$ModelProvider"
    "KELVIN_PLUGIN_INDEX_URL=$(if ($IndexUrl) { $IndexUrl } else { '(not set)' })"
    "log: $LogFile"
    ""

    if (-not (Test-Path $GwPidFile)) {
        "status: stopped"
        return
    }

    $GwPid = [int](Get-Content $GwPidFile -Raw).Trim()
    $Proc = Get-Process -Id $GwPid -ErrorAction SilentlyContinue

    if (-not $Proc -or $Proc.HasExited) {
        "status: stopped (stale PID file: $GwPid)"
        return
    }

    $Uptime = (Get-Date) - $Proc.StartTime
    $UptimeStr = Format-Uptime $Uptime
    "status: running (up $UptimeStr)"
    "pid:    $GwPid"
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
    "start"   { Cmd-Start $SubArgs }
    "stop"    { Cmd-Stop }
    "restart" { Cmd-Restart $SubArgs }
    "status"  { Cmd-Status }
    default {
        Write-Error "Unknown subcommand: $Subcommand"
        Write-Host ""
        Show-Usage
        exit 1
    }
}

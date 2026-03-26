$ErrorActionPreference = "Stop"

if (Test-Path (Join-Path $PSScriptRoot "bin\kelvin-gateway.exe")) {
    $RootDir = $PSScriptRoot
} else {
    $RootDir = Split-Path -Parent $PSScriptRoot
}

# ── dotenv loader ─────────────────────────────────────────────────────────────
$_SgEnvPaths = @(
    (Join-Path (Get-Location).Path ".env.local"),
    (Join-Path (Get-Location).Path ".env"),
    (Join-Path $HOME ".kelvinclaw\.env.local"),
    (Join-Path $HOME ".kelvinclaw\.env")
)
function _SgLoadDotenv {
    foreach ($F in $_SgEnvPaths) {
        if (-not (Test-Path $F)) { continue }
        foreach ($Line in Get-Content $F) {
            $S = $Line.Split("#")[0].Trim()
            if ([string]::IsNullOrWhiteSpace($S)) { continue }
            if ($S -match '^export\s+') { $S = $S -replace '^export\s+', '' }
            if ($S -match '^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$') {
                $K = $Matches[1]; $V = $Matches[2].Trim()
                if ($V.Length -ge 2 -and (($V[0] -eq '"' -and $V[-1] -eq '"') -or ($V[0] -eq "'" -and $V[-1] -eq "'"))) { $V = $V.Substring(1, $V.Length - 2) }
                if (-not [System.Environment]::GetEnvironmentVariable($K)) { Set-Item -Path "Env:$K" -Value $V }
            }
        }
    }
}
_SgLoadDotenv
# ──────────────────────────────────────────────────────────────────────────────

$KelvinHome      = if ($env:KELVIN_HOME)             { $env:KELVIN_HOME }             else { Join-Path $HOME ".kelvinclaw" }
$PluginHome      = if ($env:KELVIN_PLUGIN_HOME)      { $env:KELVIN_PLUGIN_HOME }      else { Join-Path $KelvinHome "plugins" }
$TrustPolicyPath = if ($env:KELVIN_TRUST_POLICY_PATH){ $env:KELVIN_TRUST_POLICY_PATH } else { Join-Path $KelvinHome "trusted_publishers.json" }
$IndexUrl        = if ($env:KELVIN_PLUGIN_INDEX_URL) { $env:KELVIN_PLUGIN_INDEX_URL } else { "" }
$ModelProvider   = if ($env:KELVIN_MODEL_PROVIDER)   { $env:KELVIN_MODEL_PROVIDER }   else { "kelvin.echo" }

function Show-Usage {
    @"
Usage: .\start-gateway.cmd [kelvin-gateway args]

Gateway launcher for KelvinClaw on Windows.
Automatically installs the configured model provider plugin on first run.

Environment:
  KELVIN_MODEL_PROVIDER        Model provider plugin id (default: kelvin.echo)
  KELVIN_PLUGIN_INDEX_URL      Plugin index URL (required for auto-install)
  KELVIN_GATEWAY_TOKEN         Gateway auth token (required)
  KELVIN_HOME                  State root (default: ~\.kelvinclaw)
  KELVIN_PLUGIN_HOME           Plugin install root
  KELVIN_TRUST_POLICY_PATH     Trust policy path
"@
}

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command: $Name"
    }
}

function Plugin-IsInstalled([string]$PluginId) {
    $CurrentDir = Join-Path (Join-Path $PluginHome $PluginId) "current"
    return (Test-Path (Join-Path $CurrentDir "plugin.json"))
}

function Install-PluginFromIndex([string]$PluginId) {
    if (-not $IndexUrl) {
        throw "KELVIN_PLUGIN_INDEX_URL is required to auto-install plugin: $PluginId"
    }
    Require-Command "tar"

    Write-Host "[start-gateway] fetching plugin index"
    $IndexJson = Invoke-RestMethod -Uri $IndexUrl -TimeoutSec 15
    $Entry = $IndexJson.plugins | Where-Object { $_.id -eq $PluginId }
    if (-not $Entry) {
        throw "Plugin not found in index: $PluginId"
    }
    if (-not $Entry.package_url) {
        throw "Plugin '$PluginId' has no package_url in index (build from source required)"
    }

    $Version    = $Entry.version
    $PackageUrl = $Entry.package_url
    $ExpectedSha = $Entry.sha256

    Write-Host "[start-gateway] installing $PluginId@$Version"

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
    } finally {
        Remove-Item -Recurse -Force $WorkDir -ErrorAction SilentlyContinue
    }
}

$CliArgs = $args
if ($CliArgs.Length -gt 0 -and ($CliArgs[0] -eq "-h" -or $CliArgs[0] -eq "--help")) {
    Show-Usage
    exit 0
}

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $TrustPolicyPath) | Out-Null
$env:KELVIN_PLUGIN_HOME      = $PluginHome
$env:KELVIN_TRUST_POLICY_PATH = $TrustPolicyPath

if ($ModelProvider -ne "kelvin.echo") {
    if (-not (Plugin-IsInstalled $ModelProvider)) {
        Install-PluginFromIndex $ModelProvider
    }
}

$GatewayBinary = Join-Path $RootDir "bin\kelvin-gateway.exe"
& $GatewayBinary --model-provider $ModelProvider @CliArgs
exit $LASTEXITCODE

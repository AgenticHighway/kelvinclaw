$ErrorActionPreference = "Stop"

if (Test-Path (Join-Path $PSScriptRoot "bin\\kelvin-host.exe")) {
    $RootDir = $PSScriptRoot
} else {
    $RootDir = Split-Path -Parent $PSScriptRoot
}

$PluginManifestPath = Join-Path $RootDir "share\\official-first-party-plugins.env"
$DefaultPluginIndexUrl = "https://raw.githubusercontent.com/AgenticHighway/kelvinclaw-plugins/main/index.json"
$DefaultOllamaBaseUrl = "http://localhost:11434"
$DefaultKelvinHome = if ($env:KELVIN_HOME) { $env:KELVIN_HOME } else { Join-Path $HOME ".kelvinclaw" }
$_LaunchEnvPaths = @(
    (Join-Path $DefaultKelvinHome ".env.local"),
    (Join-Path $DefaultKelvinHome ".env"),
    (Join-Path (Get-Location).Path ".env.local"),
    (Join-Path (Get-Location).Path ".env")
)

function Show-Usage {
@"
Usage: .\kelvin.cmd [init [options] | kelvin-host args]

Release-bundle launcher for KelvinClaw on Windows.

Behavior:
  - kelvin init writes ~/.kelvinclaw/.env for first-run setup
  - with no args, installs required official plugins on first run
  - starts interactive mode in a terminal
  - falls back to a default prompt when not attached to a console

Environment:
  KELVIN_HOME
  KELVIN_PLUGIN_HOME
  KELVIN_TRUST_POLICY_PATH
  KELVIN_STATE_DIR
  KELVIN_DEFAULT_PROMPT
  OPENAI_API_KEY
"@
}

function Show-InitUsage {
@"
Usage: .\kelvin.cmd init [--provider <echo|openai|anthropic|openrouter|ollama>] [--force]

Initialize KelvinClaw's user config in ~/.kelvinclaw/.env.
"@
}

function Require-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command: $Name"
    }
}

function Trim-Value([string]$Value) {
    return $Value.Trim()
}

function Strip-WrappingQuotes([string]$Value) {
    if ($Value.Length -ge 2) {
        if (($Value.StartsWith('"') -and $Value.EndsWith('"')) -or ($Value.StartsWith("'") -and $Value.EndsWith("'"))) {
            return $Value.Substring(1, $Value.Length - 2)
        }
    }
    return $Value
}

function Get-ConfigTemplatePath {
    $ReleaseTemplate = Join-Path $RootDir "release\env.example"
    if (Test-Path $ReleaseTemplate) {
        return $ReleaseTemplate
    }

    $RootTemplate = Join-Path $RootDir ".env.example"
    if (Test-Path $RootTemplate) {
        return $RootTemplate
    }

    throw "Missing KelvinClaw config template (.env.example)"
}

function New-GatewayToken {
    $Bytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($Bytes)
    return ([System.BitConverter]::ToString($Bytes)).Replace("-", "").ToLowerInvariant()
}

function Set-EnvValueInFile([string]$Path, [string]$Key, [string]$Value) {
    $Lines = if (Test-Path $Path) { Get-Content $Path } else { @() }
    $Pattern = "^\s*" + [regex]::Escape($Key) + "\s*="
    $Replacement = "$Key=$Value"
    $Replaced = $false

    for ($Index = 0; $Index -lt $Lines.Count; $Index++) {
        if ($Lines[$Index] -match $Pattern) {
            $Lines[$Index] = $Replacement
            $Replaced = $true
            break
        }
    }

    if (-not $Replaced) {
        $Lines += $Replacement
    }

    Set-Content -Path $Path -Value $Lines
}

function Resolve-InitProvider([string]$Provider) {
    switch ($Provider.ToLowerInvariant()) {
        "echo" { return "echo" }
        "kelvin.echo" { return "echo" }
        "openai" { return "openai" }
        "kelvin.openai" { return "openai" }
        "anthropic" { return "anthropic" }
        "kelvin.anthropic" { return "anthropic" }
        "openrouter" { return "openrouter" }
        "kelvin.openrouter" { return "openrouter" }
        "ollama" { return "ollama" }
        "kelvin.ollama" { return "ollama" }
        default {
            throw "Unsupported provider: $Provider. Expected one of: echo, openai, anthropic, openrouter, ollama"
        }
    }
}

function Select-InitProvider {
    Write-Host "[kelvin init] Choose a provider:"
    Write-Host "  1) kelvin.echo (Recommended)"
    Write-Host "  2) kelvin.openai"
    Write-Host "  3) kelvin.anthropic"
    Write-Host "  4) kelvin.openrouter"
    Write-Host "  5) kelvin.ollama"
    $Selection = (Read-Host "[kelvin init] Provider [1]").Trim()

    switch ($Selection) {
        "" { return "echo" }
        "1" { return "echo" }
        "2" { return "openai" }
        "3" { return "anthropic" }
        "4" { return "openrouter" }
        "5" { return "ollama" }
        default { return (Resolve-InitProvider $Selection) }
    }
}

function Read-SecretValue([string]$Prompt) {
    $SecureValue = Read-Host $Prompt -AsSecureString
    $Ptr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($SecureValue)
    try {
        return [Runtime.InteropServices.Marshal]::PtrToStringBSTR($Ptr)
    }
    finally {
        if ($Ptr -ne [IntPtr]::Zero) {
            [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($Ptr)
        }
    }
}

function Read-ValueWithDefault([string]$Prompt, [string]$Default) {
    $Value = (Read-Host "$Prompt [$Default]").Trim()
    if ([string]::IsNullOrWhiteSpace($Value)) {
        return $Default
    }
    return $Value
}

function Load-EnvVarFromFile([string]$Key, [string]$FilePath) {
    if (-not (Test-Path $FilePath)) {
        return $null
    }

    foreach ($Line in Get-Content $FilePath) {
        $Stripped = $Line.Split("#")[0].Trim()
        if ([string]::IsNullOrWhiteSpace($Stripped)) {
            continue
        }
        if ($Stripped -match '^export\s+') {
            $Stripped = $Stripped -replace '^export\s+', ''
        }
        if ($Stripped -match "^$Key\s*=\s*(.*)$") {
            return (Strip-WrappingQuotes (Trim-Value $Matches[1]))
        }
    }

    return $null
}

function Load-Dotenv {
    $Dotenv = @{}
    foreach ($EnvFile in $_LaunchEnvPaths) {
        if (-not (Test-Path $EnvFile)) { continue }
        foreach ($Line in Get-Content $EnvFile) {
            $Stripped = $Line.Split("#")[0].Trim()
            if ([string]::IsNullOrWhiteSpace($Stripped)) { continue }
            if ($Stripped -match '^export\s+') { $Stripped = $Stripped -replace '^export\s+', '' }
            if ($Stripped -match '^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$') {
                $Key   = $Matches[1]
                $Value = Strip-WrappingQuotes (Trim-Value $Matches[2])
                if (-not $Dotenv.ContainsKey($Key)) { $Dotenv[$Key] = $Value }
            }
        }
    }
    return $Dotenv
}
function _LaunchEnv([string]$Key, [string]$Default = "") {
    $V = [System.Environment]::GetEnvironmentVariable($Key)
    if ($V) { return $V }
    if ($_LaunchDotenv.ContainsKey($Key)) { return $_LaunchDotenv[$Key] }
    return $Default
}

function Prompt-ForOpenAIKey([string[]]$CliArgs) {
    if ($env:OPENAI_API_KEY -or $CliArgs.Length -gt 0) {
        return
    }
    if (-not [Environment]::UserInteractive) {
        return
    }

    Write-Host "[kelvin] OPENAI_API_KEY not found in the environment or .env files."
    $Value = Read-Host "[kelvin] Paste your OpenAI API key for this run, or press Enter to continue with echo mode"
    $Value = Trim-Value $Value
    if ($Value) {
        $env:OPENAI_API_KEY = $Value
    }
}

function Resolve-LaunchModelProvider {
    if ($env:KELVIN_MODEL_PROVIDER) {
        return $env:KELVIN_MODEL_PROVIDER
    }
    if ($env:OPENAI_API_KEY) {
        return "kelvin.openai"
    }
    return ""
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

function Ensure-TrustPolicy([string]$TrustPolicyUrl) {
    if (Test-Path $TrustPolicyPath) {
        return
    }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $TrustPolicyPath) | Out-Null
    if ($TrustPolicyUrl) {
        Write-Host "[kelvin] fetching official trust policy"
        Invoke-WebRequest -Uri $TrustPolicyUrl -OutFile $TrustPolicyPath
        return
    }
    '{"require_signature":false,"publishers":[]}' | Set-Content -NoNewline $TrustPolicyPath
    Write-Host "[kelvin] wrote permissive trust policy: $TrustPolicyPath"
}

function Extract-PackageCleanly([string]$TarballPath, [string]$ExtractDir) {
    New-Item -ItemType Directory -Force -Path $ExtractDir | Out-Null
    & tar -xzf $TarballPath -C $ExtractDir
}

function Install-OfficialPlugin([string]$PluginId, [string]$Version, [string]$PackageUrl, [string]$ExpectedSha, [string]$TrustPolicyUrl) {
    $CurrentVersion = Plugin-CurrentVersion $PluginId
    $VersionDir = Join-Path (Join-Path $PluginHome $PluginId) $Version
    if ($CurrentVersion -eq $Version -and (Test-Path (Join-Path $VersionDir "plugin.json"))) {
        return
    }

    Write-Host "[kelvin] installing official plugin: $PluginId@$Version"
    Ensure-TrustPolicy $TrustPolicyUrl
    New-Item -ItemType Directory -Force -Path (Join-Path $PluginHome $PluginId) | Out-Null

    $WorkDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
    $PackagePath = Join-Path $WorkDir "plugin.tar.gz"
    $ExtractDir = Join-Path $WorkDir "extract"
    New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

    Invoke-WebRequest -Uri $PackageUrl -OutFile $PackagePath
    $ActualSha = (Get-FileHash -Algorithm SHA256 -Path $PackagePath).Hash.ToLowerInvariant()
    if ($ActualSha -ne $ExpectedSha.ToLowerInvariant()) {
        throw "Checksum mismatch for $PluginId@$Version"
    }

    Extract-PackageCleanly -TarballPath $PackagePath -ExtractDir $ExtractDir

    if (Test-Path $VersionDir) {
        Remove-Item -Recurse -Force $VersionDir
    }
    New-Item -ItemType Directory -Force -Path $VersionDir | Out-Null
    Copy-Item -Recurse -Force (Join-Path $ExtractDir "*") $VersionDir

    $CurrentDir = Join-Path (Join-Path $PluginHome $PluginId) "current"
    if (Test-Path $CurrentDir) {
        Remove-Item -Recurse -Force $CurrentDir
    }
    New-Item -ItemType Directory -Force -Path $CurrentDir | Out-Null
    Copy-Item -Recurse -Force (Join-Path $VersionDir "*") $CurrentDir

    Remove-Item -Recurse -Force $WorkDir
}

function Load-PluginManifest {
    if (-not (Test-Path $PluginManifestPath)) {
        throw "Release bundle is missing $PluginManifestPath"
    }

    $Values = @{}
    foreach ($Line in Get-Content $PluginManifestPath) {
        $Stripped = $Line.Trim()
        if ([string]::IsNullOrWhiteSpace($Stripped) -or $Stripped.StartsWith("#")) {
            continue
        }
        if ($Stripped -match '^([A-Z0-9_]+)="(.*)"$') {
            $Values[$Matches[1]] = $Matches[2]
        }
    }

    return $Values
}

function Resolve-PluginIndexEntry([string]$PluginId, [string]$IndexUrl) {
    if (-not $IndexUrl) {
        throw "KELVIN_PLUGIN_INDEX_URL must be set to install '$PluginId'"
    }

    Write-Host "[kelvin] fetching plugin index"
    $IndexJson = Invoke-RestMethod -Uri $IndexUrl -TimeoutSec 15
    $Entries = @($IndexJson.plugins | Where-Object { $_.id -eq $PluginId })
    if ($Entries.Count -eq 0) {
        throw "Plugin not found in index: $PluginId"
    }

    return $Entries | Sort-Object {
        $v = $_.version -replace '[+\-].*$', ''
        try { [System.Version]$v } catch { [System.Version]"0.0.0" }
    } | Select-Object -Last 1
}

function Bootstrap-OfficialPlugins {
    Require-Command "tar"
    $Manifest = Load-PluginManifest

    if (-not [string]::IsNullOrWhiteSpace($Manifest["KELVIN_CLI_VERSION"])) {
        Install-OfficialPlugin `
            -PluginId "kelvin.cli" `
            -Version $Manifest["KELVIN_CLI_VERSION"] `
            -PackageUrl $Manifest["KELVIN_CLI_PACKAGE_URL"] `
            -ExpectedSha $Manifest["KELVIN_CLI_SHA256"] `
            -TrustPolicyUrl $Manifest["OFFICIAL_TRUST_POLICY_URL"]
    }

    if ($env:OPENAI_API_KEY -and -not [string]::IsNullOrWhiteSpace($Manifest["KELVIN_OPENAI_VERSION"])) {
        Install-OfficialPlugin `
            -PluginId "kelvin.openai" `
            -Version $Manifest["KELVIN_OPENAI_VERSION"] `
            -PackageUrl $Manifest["KELVIN_OPENAI_PACKAGE_URL"] `
            -ExpectedSha $Manifest["KELVIN_OPENAI_SHA256"] `
            -TrustPolicyUrl $Manifest["OFFICIAL_TRUST_POLICY_URL"]
    }
}

function Ensure-PluginInstalled([string]$PluginId) {
    $CurrentVersion = Plugin-CurrentVersion $PluginId
    if ($CurrentVersion) {
        Ensure-TrustPolicy $null
        return
    }

    Require-Command "tar"
    $IndexUrl = if ($env:KELVIN_PLUGIN_INDEX_URL) { $env:KELVIN_PLUGIN_INDEX_URL } else { $DefaultPluginIndexUrl }
    $Entry = Resolve-PluginIndexEntry -PluginId $PluginId -IndexUrl $IndexUrl
    Write-Host "[kelvin] bootstrapping plugin: $PluginId"
    Install-OfficialPlugin `
        -PluginId $PluginId `
        -Version $Entry.version `
        -PackageUrl $Entry.package_url `
        -ExpectedSha $Entry.sha256 `
        -TrustPolicyUrl $Entry.trust_policy_url
}

function Ensure-RequiredPlugins([string]$PluginId) {
    Ensure-PluginInstalled "kelvin.cli"

    if ([string]::IsNullOrWhiteSpace($PluginId) -or $PluginId -eq "kelvin.echo") {
        Ensure-TrustPolicy $null
        return
    }

    Ensure-PluginInstalled $PluginId
}

function Invoke-KelvinInit([string[]]$InitArgs) {
    $Force = $false
    $Provider = $null

    for ($Index = 0; $Index -lt $InitArgs.Length; $Index++) {
        switch ($InitArgs[$Index]) {
            "--force" {
                $Force = $true
            }
            "--provider" {
                if ($Index + 1 -ge $InitArgs.Length) {
                    throw "missing value for --provider"
                }
                $Provider = Resolve-InitProvider $InitArgs[$Index + 1]
                $Index++
            }
            "-h" {
                Show-InitUsage
                return
            }
            "--help" {
                Show-InitUsage
                return
            }
            default {
                throw "unknown init argument: $($InitArgs[$Index])"
            }
        }
    }

    $KelvinHome = if ($env:KELVIN_HOME) { $env:KELVIN_HOME } else { Join-Path $HOME ".kelvinclaw" }
    $ConfigEnvPath = Join-Path $KelvinHome ".env"
    if (-not $Provider) {
        $Provider = if ([Environment]::UserInteractive) { Select-InitProvider } else { "echo" }
    }

    if ((Test-Path $ConfigEnvPath) -and -not $Force) {
        throw "$ConfigEnvPath already exists. Re-run with 'kelvin init --force' to overwrite it."
    }

    New-Item -ItemType Directory -Force -Path $KelvinHome | Out-Null
    Copy-Item (Get-ConfigTemplatePath) $ConfigEnvPath -Force

    Set-EnvValueInFile -Path $ConfigEnvPath -Key "KELVIN_GATEWAY_TOKEN" -Value (New-GatewayToken)
    $PluginIndexUrl = if ($env:KELVIN_PLUGIN_INDEX_URL) { $env:KELVIN_PLUGIN_INDEX_URL } else { $DefaultPluginIndexUrl }
    Set-EnvValueInFile -Path $ConfigEnvPath -Key "KELVIN_PLUGIN_INDEX_URL" -Value $PluginIndexUrl

    switch ($Provider) {
        "echo" {
            Set-EnvValueInFile -Path $ConfigEnvPath -Key "KELVIN_MODEL_PROVIDER" -Value "kelvin.echo"
        }
        "openai" {
            $OpenAIKey = if ($env:OPENAI_API_KEY) { $env:OPENAI_API_KEY } elseif ([Environment]::UserInteractive) { Read-SecretValue "[kelvin init] OpenAI API key" } else { throw "OPENAI_API_KEY must be set for non-interactive openai init" }
            if ([string]::IsNullOrWhiteSpace($OpenAIKey)) {
                throw "OPENAI_API_KEY cannot be empty"
            }
            Set-EnvValueInFile -Path $ConfigEnvPath -Key "KELVIN_MODEL_PROVIDER" -Value "kelvin.openai"
            Set-EnvValueInFile -Path $ConfigEnvPath -Key "OPENAI_API_KEY" -Value $OpenAIKey.Trim()
        }
        "anthropic" {
            $AnthropicKey = if ($env:ANTHROPIC_API_KEY) { $env:ANTHROPIC_API_KEY } elseif ([Environment]::UserInteractive) { Read-SecretValue "[kelvin init] Anthropic API key" } else { throw "ANTHROPIC_API_KEY must be set for non-interactive anthropic init" }
            if ([string]::IsNullOrWhiteSpace($AnthropicKey)) {
                throw "ANTHROPIC_API_KEY cannot be empty"
            }
            Set-EnvValueInFile -Path $ConfigEnvPath -Key "KELVIN_MODEL_PROVIDER" -Value "kelvin.anthropic"
            Set-EnvValueInFile -Path $ConfigEnvPath -Key "ANTHROPIC_API_KEY" -Value $AnthropicKey.Trim()
        }
        "openrouter" {
            $OpenRouterKey = if ($env:OPENROUTER_API_KEY) { $env:OPENROUTER_API_KEY } elseif ([Environment]::UserInteractive) { Read-SecretValue "[kelvin init] OpenRouter API key" } else { throw "OPENROUTER_API_KEY must be set for non-interactive openrouter init" }
            if ([string]::IsNullOrWhiteSpace($OpenRouterKey)) {
                throw "OPENROUTER_API_KEY cannot be empty"
            }
            Set-EnvValueInFile -Path $ConfigEnvPath -Key "KELVIN_MODEL_PROVIDER" -Value "kelvin.openrouter"
            Set-EnvValueInFile -Path $ConfigEnvPath -Key "OPENROUTER_API_KEY" -Value $OpenRouterKey.Trim()
        }
        "ollama" {
            $OllamaBaseUrl = if ($env:OLLAMA_BASE_URL) { $env:OLLAMA_BASE_URL } elseif ([Environment]::UserInteractive) { Read-ValueWithDefault "[kelvin init] Ollama base URL" $DefaultOllamaBaseUrl } else { $DefaultOllamaBaseUrl }
            if ([string]::IsNullOrWhiteSpace($OllamaBaseUrl)) {
                throw "OLLAMA_BASE_URL cannot be empty"
            }
            Set-EnvValueInFile -Path $ConfigEnvPath -Key "KELVIN_MODEL_PROVIDER" -Value "kelvin.ollama"
            Set-EnvValueInFile -Path $ConfigEnvPath -Key "OLLAMA_BASE_URL" -Value $OllamaBaseUrl.Trim()
        }
    }

    Write-Host "[kelvin init] Wrote $ConfigEnvPath"
    Write-Host "[kelvin init] Next step: kelvin"
}

$CliArgs = $args
if ($CliArgs.Length -gt 0) {
    if ($CliArgs[0] -eq "-h" -or $CliArgs[0] -eq "--help") {
        Show-Usage
        exit 0
    }
    if ($CliArgs[0] -eq "init") {
        $InitArgs = if ($CliArgs.Length -gt 1) { $CliArgs[1..($CliArgs.Length - 1)] } else { @() }
        Invoke-KelvinInit $InitArgs
        exit 0
    }
}

$_LaunchDotenv = Load-Dotenv

$KelvinHome      = _LaunchEnv "KELVIN_HOME"              (Join-Path $HOME ".kelvinclaw")
$PluginHome      = _LaunchEnv "KELVIN_PLUGIN_HOME"       (Join-Path $KelvinHome "plugins")
$TrustPolicyPath = _LaunchEnv "KELVIN_TRUST_POLICY_PATH" (Join-Path $KelvinHome "trusted_publishers.json")
$StateDir        = _LaunchEnv "KELVIN_STATE_DIR"         (Join-Path $KelvinHome "state")
$DefaultPrompt   = _LaunchEnv "KELVIN_DEFAULT_PROMPT"    "What is KelvinClaw?"

# Push dotenv values into process env so downstream functions and kelvin-host inherit them.
foreach ($KV in $_LaunchDotenv.GetEnumerator()) {
    if (-not [System.Environment]::GetEnvironmentVariable($KV.Key)) {
        [System.Environment]::SetEnvironmentVariable($KV.Key, $KV.Value, "Process")
    }
}

Prompt-ForOpenAIKey $CliArgs
$LaunchModelProvider = Resolve-LaunchModelProvider
Bootstrap-OfficialPlugins
Ensure-RequiredPlugins $LaunchModelProvider

New-Item -ItemType Directory -Force -Path $StateDir | Out-Null
$env:KELVIN_PLUGIN_HOME = $PluginHome
$env:KELVIN_TRUST_POLICY_PATH = $TrustPolicyPath

$DefaultHostArgs = @()
if ($LaunchModelProvider) {
    $DefaultHostArgs += @("--model-provider", $LaunchModelProvider)
}

$HostBinary = Join-Path $RootDir "bin\\kelvin-host.exe"
if ($CliArgs.Length -eq 0) {
    if ([Environment]::UserInteractive) {
        & $HostBinary @DefaultHostArgs --interactive --workspace (Get-Location).Path --state-dir $StateDir
        exit $LASTEXITCODE
    }

    & $HostBinary @DefaultHostArgs --prompt $DefaultPrompt --workspace (Get-Location).Path --state-dir $StateDir
    exit $LASTEXITCODE
}

& $HostBinary @CliArgs
exit $LASTEXITCODE

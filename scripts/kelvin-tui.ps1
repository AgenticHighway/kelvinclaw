$ErrorActionPreference = "Stop"

if (Test-Path (Join-Path $PSScriptRoot "bin\kelvin-tui.exe")) {
    $RootDir = $PSScriptRoot
} else {
    $RootDir = Split-Path -Parent $PSScriptRoot
}

# ── dotenv loader ─────────────────────────────────────────────────────────────
$_TuiEnvPaths = @(
    (Join-Path (Get-Location).Path ".env.local"),
    (Join-Path (Get-Location).Path ".env"),
    (Join-Path $HOME ".kelvinclaw\.env.local"),
    (Join-Path $HOME ".kelvinclaw\.env")
)
function _TuiLoadDotenv {
    foreach ($F in $_TuiEnvPaths) {
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
_TuiLoadDotenv
# ──────────────────────────────────────────────────────────────────────────────

$CliArgs = $args
if ($CliArgs.Length -gt 0 -and ($CliArgs[0] -eq "-h" -or $CliArgs[0] -eq "--help")) {
    @"
Usage: .\kelvin-tui.cmd [kelvin-tui args]

Release-bundle launcher for kelvin-tui on Windows.
Loads .env files automatically — no manual export needed.

Environment:
  KELVIN_GATEWAY_TOKEN   Auth token for the gateway (required)

The launcher reads all variables from:
  - .\.env.local / .\.env
  - ~\.kelvinclaw\.env.local / ~\.kelvinclaw\.env

Pass --help to see kelvin-tui's full option list.
"@
    exit 0
}

$TuiBinary = Join-Path $RootDir "bin\kelvin-tui.exe"
& $TuiBinary @CliArgs
exit $LASTEXITCODE

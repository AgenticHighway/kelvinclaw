$ErrorActionPreference = "Stop"

if (Test-Path (Join-Path $PSScriptRoot "bin\kelvin-tui.exe")) {
    $RootDir = $PSScriptRoot
} else {
    $RootDir = Split-Path -Parent $PSScriptRoot
}
$DefaultKelvinHome = if ($env:KELVIN_HOME) { $env:KELVIN_HOME } else { Join-Path $HOME ".kelvinclaw" }

# ── dotenv loader ─────────────────────────────────────────────────────────────
$_TuiEnvPaths = @(
    (Join-Path $DefaultKelvinHome ".env.local"),
    (Join-Path $DefaultKelvinHome ".env"),
    (Join-Path (Get-Location).Path ".env.local"),
    (Join-Path (Get-Location).Path ".env")
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
                [System.Environment]::SetEnvironmentVariable($K, $V, "Process")
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
  - ~\.kelvinclaw\.env.local / ~\.kelvinclaw\.env
  - .\.env.local / .\.env

Pass --help to see kelvin-tui's full option list.
"@
    exit 0
}

$TuiBinary = Join-Path $RootDir "bin\kelvin-tui.exe"
& $TuiBinary @CliArgs
exit $LASTEXITCODE

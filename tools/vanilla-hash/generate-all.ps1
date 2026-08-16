# generate-all.ps1 — Generate golden data for multiple seeds and server types.
#
# Usage: .\generate-all.ps1 [seed1 seed2 ...]
# If no seeds are given, uses default set: 12345 67890 11111 99999 42
#
# Requires: cargo, java

param(
    [int[]]$Seeds = @(12345, 67890, 11111, 99999, 42)
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent (Split-Path -Parent $ScriptDir)
$OutputDir = Join-Path $ScriptDir "hashes"
$ServersDir = Join-Path $RepoRoot "bench" "servers"

# Determine available servers
$Servers = @("vanilla")
if (Test-Path (Join-Path $ServersDir "paper" "server.jar")) { $Servers += "paper" }
if (Test-Path (Join-Path $ServersDir "folia" "server.jar")) { $Servers += "folia" }

# Check prerequisites
if (-not (Get-Command java -ErrorAction SilentlyContinue)) {
    Write-Error "java not found in PATH"
    exit 1
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo not found in PATH"
    exit 1
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

Write-Host "=== Golden Data Generation ===" -ForegroundColor Cyan
Write-Host "Seeds: $($Seeds -join ', ')"
Write-Host "Servers: $($Servers -join ', ')"
Write-Host "Output: $OutputDir"
Write-Host ""

Set-Location $RepoRoot

# Build once
Write-Host "--- Building golden-data ---" -ForegroundColor Yellow
cargo build -p golden-data --release
Write-Host ""

$Failed = 0
foreach ($server in $Servers) {
    foreach ($seed in $Seeds) {
        $OutputFile = Join-Path $OutputDir "$server-$seed.json"
        Write-Host "--- Generating: server=$server seed=$seed ---" -ForegroundColor Yellow

        try {
            cargo run -p golden-data --release -- `
                --seed $seed `
                --server $server `
                --servers-dir $ServersDir `
                --output $OutputFile
            Write-Host "  OK: $OutputFile" -ForegroundColor Green
        } catch {
            Write-Host "  FAILED: server=$server seed=$seed" -ForegroundColor Red
            $Failed++
        }
        Write-Host ""
    }
}

Write-Host "=== Done ===" -ForegroundColor Cyan
Write-Host "Generated files in $OutputDir:"
Get-ChildItem "$OutputDir\*.json" -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host "  $($_.Name) ($($_.Length) bytes)"
}

if ($Failed -gt 0) {
    Write-Warning "$Failed extraction(s) failed"
    exit 1
}

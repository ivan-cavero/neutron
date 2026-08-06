<#
  run_minimal.ps1 — Baseline B0 barebones
  Levanta Vanilla 26.2, mide startup, ejecuta bots, produce JSON.
#>
param(
    [string]$Server = "vanilla",
    [int]$N = 10,
    [string]$SeedStr = "1234567890123456789",
    [int]$WarmupSec = 30
)

$ErrorActionPreference = "Stop"
$BenchDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$ResultsDir = Join-Path $BenchDir "results"
$LogDir = Join-Path $BenchDir "logs"

$dateStamp = Get-Date -Format "yyyyMMdd-HHmmss"
$runId = "${Server}-${N}j-baseline"
$runLogDir = Join-Path $LogDir $runId
$null = New-Item -ItemType Directory -Path $runLogDir -Force
$null = New-Item -ItemType Directory -Path $ResultsDir -Force

$LogFilePath = Join-Path $runLogDir "server.log"
$BotLogDir = Join-Path $runLogDir "bots"
$null = New-Item -ItemType Directory -Path $BotLogDir -Force

Write-Host "=== Baseline B0: Vanilla 26.2 ==="
Write-Host "Server: $Server | N=$N | Warmup=${WarmupSec}s | Seed=$SeedStr"
Write-Host "Results: $ResultsDir"
Write-Host "Logs: $runLogDir"

# ── Start server ────────────────────────────────────────────────────────────
$ServerDir = Join-Path (Join-Path $BenchDir "servers") $Server
$JarPath = Join-Path $ServerDir "server.jar"
if (-not (Test-Path $JarPath)) { throw "Not found: $JarPath" }

# Create temp world
$WorldDir = Join-Path $runLogDir "world"
$null = New-Item -ItemType Directory -Path $WorldDir -Force

# Build server.properties (forward slashes to avoid Java properties backslash escaping)
$WorldDirFwd = $WorldDir -replace '\\', '/'
$Props = @"
eula=true
online-mode=false
level-seed=$SeedStr
view-distance=10
simulation-distance=10
server-port=25565
max-players=20
gamemode=survival
difficulty=peaceful
spawn-animals=false
spawn-monsters=false
spawn-npcs=false
level-type=minecraft:normal
allow-nether=false
allow-end=false
sync-chunk-writes=true
enforce-secure-profile=false
level-name=$WorldDirFwd
"@
$PropsPath = Join-Path $runLogDir "server.properties"
$Props | Set-Content -Path $PropsPath -Encoding UTF8

# Accept EULA and place properties in working directory (no BOM)
[System.IO.File]::WriteAllText((Join-Path $runLogDir "eula.txt"), "eula=true")
$Props | Out-File -FilePath (Join-Path $runLogDir "server.properties") -Encoding ASCII

$javaArgs = @("-Xms2G", "-Xmx2G", "-XX:+AlwaysPreTouch", "-jar", $JarPath, "nogui")
$errPath = "$LogFilePath.err"
Write-Host "Starting server..."
$proc = Start-Process -FilePath "java" `
    -ArgumentList $javaArgs `
    -WorkingDirectory $runLogDir `
    -RedirectStandardOutput $LogFilePath `
    -RedirectStandardError $errPath `
    -NoNewWindow -PassThru

Start-Sleep -Seconds 3
Write-Host "Server PID: $($proc.Id)"

# Wait for "Done"
$timeoutSec = 90
$startTime = Get-Date
$done = $false
while ((Get-Date) -lt $startTime.AddSeconds($timeoutSec)) {
    Start-Sleep -Milliseconds 500
    if (Test-Path $LogFilePath) {
        $content = Get-Content -Path $LogFilePath -Raw
        if ($content -match "Done \(([\d.]+)s\)!") {
            $startupMs = [double]::Parse($matches[1]) * 1000
            $done = $true
            Write-Host "Server ready! Startup: ${startupMs}ms"
            break
        }
    }
    if ($proc.HasExited) {
        throw "Server exited prematurely. Check $LogFilePath"
    }
}

if (-not $done) { throw "Server did not start within ${timeoutSec}s" }

# ── Warmup ──────────────────────────────────────────────────────────────────
Write-Host "Warmup ${WarmupSec}s..."
Start-Sleep -Seconds $WarmupSec

# ── Start bots ──────────────────────────────────────────────────────────────
$BotScript = Join-Path $BenchDir "bots\join-bench\index.js"
if (-not (Test-Path $BotScript)) { throw "Bot script not found: $BotScript" }

$OutputPath = Join-Path $runLogDir "latency.json"
$nodeArgs = @("$BotScript", "--count", "$N", "--version", "26.2", "--output", "$OutputPath")

Write-Host "Launching $N bots..."
$botStart = Get-Date
$botProc = Start-Process -FilePath "node" `
    -ArgumentList $nodeArgs `
    -RedirectStandardOutput (Join-Path $runLogDir "bot.out.log") `
    -RedirectStandardError (Join-Path $runLogDir "bot.err.log") `
    -NoNewWindow -PassThru

# Wait for bots
$botTimeoutSec = 30
$botDone = $false
$botWaitStart = Get-Date
while ((Get-Date) -lt $botWaitStart.AddSeconds($botTimeoutSec)) {
    Start-Sleep -Milliseconds 200
    if ($botProc.HasExited) {
        $botDone = $true
        break
    }
}
if (-not $botDone) {
    Write-Warning "Bot timeout, killing..."
    $botProc.Kill()
}
$botElapsed = [math]::Round(((Get-Date) - $botStart).TotalMilliseconds, 1)

# ── Read results ────────────────────────────────────────────────────────────
$joinP50Ms = 0; $joinP95Ms = 0; $joinP99Ms = 0; $botSuccess = 0; $botFailed = 0; $totalBots = 0
if (Test-Path $OutputPath) {
    $json = Get-Content -Path $OutputPath -Raw | ConvertFrom-Json
    $joinP50Ms = [math]::Round($json.p50Ms, 1)
    $joinP95Ms = [math]::Round($json.p95Ms, 1)
    $joinP99Ms = [math]::Round($json.p99Ms, 1)
    $botSuccess = $json.successful
    $botFailed = $json.failed
    $totalBots = $json.totalBots
    Write-Host "Bots: $botSuccess/$totalBots connected | p50=${joinP50Ms}ms p95=${joinP95Ms}ms p99=${joinP99Ms}ms"
} else {
    Write-Warning "No bot output found at $OutputPath"
}

# ── Stop server ─────────────────────────────────────────────────────────────
Write-Host "Stopping server..."
$proc.Kill()
Start-Sleep -Seconds 2

# ── Write baseline JSON ─────────────────────────────────────────────────────
$result = @{
    benchmarkId = $runId
    date = $dateStamp
    server = $Server
    version = "26.2"
    seed = $SeedStr
    botCount = $N
    warmupSec = $WarmupSec
    startupMs = [math]::Round($startupMs, 1)
    bots = @{
        totalBots = $N
        successful = $botSuccess
        failed = $botFailed
        joinP50Ms = $joinP50Ms
        joinP95Ms = $joinP95Ms
        joinP99Ms = $joinP99Ms
        botElapsedMs = $botElapsed
    }
    tps = $null
    cps = $null
    notes = "Baseline B0 - Vanilla 26.2 - single run"
}
} else {
    Write-Warning "No bot output found at $OutputPath"
}

# ── Stop server ─────────────────────────────────────────────────────────────
Write-Host "Stopping server..."
$proc.Kill()
Start-Sleep -Seconds 2

# ── Write baseline JSON ─────────────────────────────────────────────────────
$result = @{
    benchmarkId = $runId
    date = $dateStamp
    server = $Server
    version = "26.2"
    seed = $SeedStr
    botCount = $N
    warmupSec = $WarmupSec
    startupMs = [math]::Round($startupMs, 1)
    bots = @{
        totalBots = $N
        successful = $botSuccess
        failed = $botFailed
        joinP50Ms = $joinP50Ms
        joinP95Ms = $joinP95Ms
        joinP99Ms = $joinP99Ms
        botElapsedMs = $botElapsed
    }
    tps = $null
    cps = $null
    notes = "Baseline B0 - Vanilla 26.2 - single run"
}

$jsonOut = Join-Path $ResultsDir "baseline-B0-${dateStamp}.json"
$result | ConvertTo-Json -Depth 5 | Set-Content -Path $jsonOut -Encoding UTF8
Write-Host "=== RESULT ==="
Write-Host "JSON: $jsonOut"
Get-Content -Path $jsonOut
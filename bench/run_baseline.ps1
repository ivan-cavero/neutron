<#
  run_baseline.ps1 — Baseline B0: Vanilla 26.2
  Bare minimum: start server, wait Done, warmup, run bots, output JSON.
#>
param([string]$Server="vanilla", [int]$N=10, [int]$WarmupSec=15)
$ErrorActionPreference="Stop"
$BenchDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$ResultsDir = Join-Path $BenchDir "results"
$dateStamp = Get-Date -Format "yyyyMMdd-HHmmss"
$runId = "${Server}-${N}j-baseline"
$RunDir = Join-Path (Join-Path $BenchDir "logs") $runId
New-Item -ItemType Directory -Path $RunDir -Force | Out-Null
New-Item -ItemType Directory -Path $ResultsDir -Force | Out-Null
$LogFile = Join-Path $RunDir "server.log"
$ErrFile = "$LogFile.err"

Write-Host "=== Baseline B0: Vanilla 26.2 ==="
Write-Host "Run: $runId | Warmup: ${WarmupSec}s"

# EULA
[System.IO.File]::WriteAllText((Join-Path $RunDir "eula.txt"), "eula=true")
$WorldDir = Join-Path $RunDir "world"
New-Item -ItemType Directory -Path $WorldDir -Force | Out-Null

# server.properties (forward slashes to avoid Java backslash escaping)
$props = @"
eula=true
online-mode=false
level-seed=1234567890123456789
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
level-name=$($WorldDir -replace '\\','/')
"@
[System.IO.File]::WriteAllText((Join-Path $RunDir "server.properties"), $props)

# Start server
$javaArgs = @("-Xms2G","-Xmx2G","-XX:+AlwaysPreTouch","-jar",(Join-Path $BenchDir "servers/vanilla/server.jar"),"nogui")
$proc = Start-Process -FilePath "java" -ArgumentList $javaArgs -WorkingDirectory $RunDir -RedirectStandardOutput $LogFile -RedirectStandardError $ErrFile -NoNewWindow -PassThru
Start-Sleep -Seconds 2

# Wait for "Done"
$found=$false; $startupMs=0
$timeout=[datetime]::Now.AddSeconds(90)
while([datetime]::Now -lt $timeout){
    Start-Sleep -Milliseconds 300
    if(Test-Path $LogFile){
        $c=Get-Content $LogFile -Raw
        if($c -match "Done \((\d+\.?\d*)s\)!"){
            $startupMs=[double]::Parse($matches[1])*1000
            $found=$true; break
        }
    }
    if($proc.HasExited){break}
}
if(!$found){try{$proc.Kill()}catch{}; throw "Server did not start in 90s"}
Write-Host "Server ready! Startup: ${startupMs}ms"

# Warmup
Write-Host "Warmup ${WarmupSec}s..."
Start-Sleep -Seconds $WarmupSec

# Run bots
$outPathJS = (Join-Path $RunDir 'latency.json') -replace '\\', '/'
$BotScript = "$($BenchDir -replace '\\','/')/bots/join-bench/index.js --count $N --version 26.2 --output $outPathJS"
$botOut = Join-Path $RunDir "bot_out.log"
$botErr = Join-Path $RunDir "bot_err.log"
Write-Host "Starting $N bots..."
$botProc = Start-Process -FilePath "node" -ArgumentList @($BotScript) -WorkingDirectory $RunDir -RedirectStandardOutput $botOut -RedirectStandardError $botErr -NoNewWindow -PassThru

$botWait=[datetime]::Now.AddSeconds(30)
while([datetime]::Now -lt $botWait){
    Start-Sleep -Milliseconds 200
    if($botProc.HasExited){break}
}
if(!$botProc.HasExited){$botProc.Kill(); Write-Warning "Bot timeout"}
$botElapsed = $null

# Read results
$j50=0;$j95=0;$j99=0;$suc=0;$fail=0
$latFile = Join-Path $RunDir "latency.json"
if(Test-Path $latFile){
    $json = Get-Content $latFile -Raw | ConvertFrom-Json
    $r = $json.results
    $j50=[math]::Round($r.p50Ms,1); $j95=[math]::Round($r.p95Ms,1); $j99=[math]::Round($r.p99Ms,1)
    $suc=$r.successful; $fail=$r.failed; $total=$r.totalBots
    Write-Host "Bots: $suc/$total ok | p50=${j50}ms p95=${j95}ms p99=${j99}ms"
} else { Write-Warning "No latency.json" }

# Stop server
$proc.Kill(); Start-Sleep -Seconds 1
try { taskkill /F /IM java.exe 2>&1 | Out-Null } catch {}

# Write JSON
$result = @{
    benchmarkId = $runId; date = $dateStamp; server = $Server; version = "26.2"
    seed = "1234567890123456789"; botCount = $N; warmupSec = $WarmupSec
    startupMs = [math]::Round($startupMs,1)
    bots = @{ totalBots = $N; successful=$suc; failed=$fail; joinP50Ms=$j50; joinP95Ms=$j95; joinP99Ms=$j99 }
    notes = "Baseline B0 - Vanilla 26.2 - Windows"
}
$jsonOut = Join-Path $ResultsDir "baseline-B0-${dateStamp}.json"
$result | ConvertTo-Json -Depth 5 | Set-Content $jsonOut -Encoding UTF8
Write-Host "=== DONE ==="
Write-Host "JSON: $jsonOut"
Get-Content $jsonOut
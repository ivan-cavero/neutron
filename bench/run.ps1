<#
  run.ps1 — Neutron benchmark harness (PowerShell 7 / Windows)
  Multi-platform: Windows (PowerShell) and Linux/macOS (bash via run.sh)

  Usage:
    .\bench\run.ps1 -Server vanilla [-N 10] [-Runs 5] [-Seed 1234567890123456789]
    .\bench\run.ps1 -Server paper   -N 10 -Runs 5
    .\bench\run.ps1 -Server folia   -N 10 -Runs 5
    .\bench\run.ps1 -Server pumpkin -N 10 -Runs 5
    .\bench\run.ps1 -Server neutron -N 10 -Runs 5

  Requirements:
    - Node.js (for join-bench bots, up to 1.21.11)
    - Rust (for azalea bots, 26.x)
    - Java 25 (for vanilla/paper/folia servers)
    - Server binary in bench/servers/<type>/
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet("vanilla", "paper", "folia", "pumpkin", "neutron")]
    [string]$Server,

    [Parameter(Mandatory = $false, Position = 1)]
    [int]$N = 10,

    [Parameter(Mandatory = $false)]
    [int]$Runs = 5,

    [Parameter(Mandatory = $false)]
    [string]$SeedStr = "1234567890123456789",

    [Parameter(Mandatory = $false)]
    [string]$WorldDir = "",

    [Parameter(Mandatory = $false)]
    [string]$ResultsDir = "",

    [Parameter(Mandatory = $false)]
    [string]$LogDirPath = "",

    [Parameter(Mandatory = $false)]
    [int]$WarmupSec = 60,

    [Parameter(Mandatory = $false)]
    [int]$MemWatchSec = 90
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

# Seed as long
$Seed = [long]::Parse($SeedStr.Trim())

# Paths
$ScriptRoot  = Split-Path -Parent $MyInvocation.MyCommand.Definition
$BaseDir     = (Get-Item $ScriptRoot\..).FullName
$BenchDir    = $ScriptRoot
$serversDir  = Join-Path $BenchDir "servers"
$botsDir     = Join-Path $BenchDir "bots"
$benchJoin   = Join-Path $botsDir "join-bench"
$azaleaBin   = Join-Path $botsDir "azalea-join-bench\target\release\azalea-join-bench.exe"

$defaultResultsDir = Join-Path $BenchDir "results"
$defaultLogDir     = Join-Path $BenchDir "logs"

# Use results/log dirs
if (-not $ResultsDir) { $ResultsDir = $defaultResultsDir }
if (-not $LogDirPath) { $LogDirPath = $defaultLogDir }

# Ensure dirs exist
foreach ($d in @($ResultsDir, $LogDirPath)) {
    if (-not (Test-Path $d)) { New-Item -ItemType Directory -Path $d -Force | Out-Null }
}

# Status logger
function Write-Status { Write-Host "[$(Get-Date -Format 'yyyy-MM-ddTHH:mm:ss.fff')] $($args -join ' ')" }

# ── Utility functions ───────────────────────────────────────────────────────

function Get-ServerVersion {
    param([string]$ServerType)
    switch ($ServerType) {
        "vanilla"  { return "26.2" }
        "paper"    { return "26.2" }
        "folia"    { return "26.2" }
        "pumpkin"  { return "26.2" }
        "neutron"  {
            $cargoToml = Join-Path $BaseDir "Cargo.toml"
            if (Test-Path $cargoToml) {
                $ver = Select-String -Path $cargoToml -Pattern '^version = "(.+)"' | ForEach-Object { $_.Matches.Groups[1].Value }
                if ($ver) { return $ver }
            }
            return "dev"
        }
    }
}

function Get-BotPath {
    param([string]$ServerType)
    # For 26.x, prefer azalea; fallback to mineflayer for older versions
    $azaleaExists = Test-Path $azaleaBin
    if ($azaleaExists) {
        return $azaleaBin
    }
    # Fallback to mineflayer
    $jsPath = Join-Path $benchJoin "index.js"
    if (Test-Path $jsPath) {
        return "node `"$jsPath`""
    }
    throw "No bot found. Build azalea first: cd bench/bots/azalea-join-bench && cargo build --release"
}

function Get-Percentile {
    param([double[]]$SortedValues, [double]$Percentile)
    if ($SortedValues.Count -eq 0) { return 0 }
    $idx = ($Percentile / 100.0) * ($SortedValues.Count - 1)
    $lo = [math]::Floor($idx)
    $hi = [math]::Ceiling($idx)
    if ($lo -eq $hi -or $hi -ge $SortedValues.Count) { return $SortedValues[$lo] }
    $frac = $idx - $lo
    return $SortedValues[$lo] * (1.0 - $frac) + $SortedValues[$hi] * $frac
}

function Read-Latencies {
    param([string]$FilePath)
    if (-not (Test-Path $FilePath)) {
        Write-Status "Warning: Latency file not found: $FilePath"
        return $null
    }
    try {
        $json = Get-Content -Path $FilePath -Raw | ConvertFrom-Json
        $results = $json.results
        if (-not $results) { return $null }

        $perBot = @()
        foreach ($bot in $results.perBot) {
            $perBot += @{
                index    = [int]$bot.index
                loginMs  = [double]$bot.loginMs
                spawnMs  = [double]$bot.spawnMs
                success  = [bool]$bot.success
                error    = if ($bot.error) { [string]$bot.error } else { $null }
            }
        }

        $totalLatencies = @($results.joinLatencies)
        $totalLatencies = $totalLatencies | Sort-Object

        return @{
            perBot          = $perBot
            totalLatencies  = $totalLatencies
            p50Ms           = [double]$results.p50Ms
            p95Ms           = [double]$results.p95Ms
            p99Ms           = [double]$results.p99Ms
            successful      = [int]$results.successful
            failed          = [int]$results.failed
            totalBots       = [int]$results.totalBots
            totalTimeMs     = [double]$results.totalTimeMs
        }
    } catch {
        Write-Status "Warning: Could not parse latency file: $_"
        return $null
    }
}

function Get-HardwareInfo {
    $os = (Get-CimInstance Win32_OperatingSystem).Caption
    $cpu = (Get-CimInstance Win32_Processor).Name
    $ram = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB, 1)
    return @{ os = $os; cpu = $cpu; ram_gb = $ram }
}

# ── Memory watcher (background job) ─────────────────────────────────────────
function Start-MemoryWatcher {
    param([string]$StatsFile, [int]$DurationSec = 90, [int]$IntervalMs = 1000)
    $scriptBlock = {
        param($ParentPid, $OutFile, $Duration, $Interval)
        $end = (Get-Date).AddSeconds($Duration)
        $samples = @()
        while ((Get-Date) -lt $end) {
            $proc = Get-Process -Id $ParentPid -ErrorAction SilentlyContinue
            if (-not $proc) { break }
            $sample = @{
                timestamp = (Get-Date -Format "yyyy-MM-ddTHH:mm:ss.fff")
                workingSetMB = [math]::Round($proc.WorkingSet64 / 1MB, 1)
                privateMB   = [math]::Round($proc.PrivateMemorySize64 / 1MB, 1)
                cpuPercent  = [math]::Round($proc.CPU, 1)
            }
            $samples += $sample
            Start-Sleep -Milliseconds $Interval
        }
        $samples | ConvertTo-Json | Set-Content -Path $OutFile -Encoding UTF8
    }
    $job = Start-Job -ScriptBlock $scriptBlock -ArgumentList $Pid, $StatsFile, $DurationSec, $IntervalMs
    return $job
}

# ── Server control ──────────────────────────────────────────────────────────

function Start-Server {
    param([string]$ServerType, [string]$RunDir, [string]$Seed, [int]$WarmupSec)
    $LogFilePath = Join-Path $RunDir "server.log"
    $ErrFilePath = Join-Path $RunDir "server.err"

    # Write EULA
    [System.IO.File]::WriteAllText((Join-Path $RunDir "eula.txt"), "eula=true")

    $worldPath = Join-Path $RunDir "world"
    New-Item -ItemType Directory -Path $worldPath -Force | Out-Null

    # Write server.properties
    $props = @"
eula=true
online-mode=false
level-seed=$Seed
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
level-name=$($worldPath -replace '\\','/')
"@
    [System.IO.File]::WriteAllText((Join-Path $RunDir "server.properties"), $props)

    switch ($ServerType) {
        "vanilla" {
            $jar = Join-Path $serversDir "vanilla\server.jar"
            if (-not (Test-Path $jar)) { throw "Not found: $jar" }
            $javaArgs = @("-Xms2G", "-Xmx2G", "-XX:+AlwaysPreTouch", "-jar", $jar, "nogui")
            $proc = Start-Process -FilePath "java" -ArgumentList $javaArgs -WorkingDirectory $RunDir `
                -RedirectStandardOutput $LogFilePath -RedirectStandardError $ErrFilePath -NoNewWindow -PassThru
        }
        "paper" {
            $jar = Join-Path $serversDir "paper\server.jar"
            if (-not (Test-Path $jar)) { throw "Not found: $jar" }
            $javaArgs = @("-Xms2G", "-Xmx2G", "-XX:+AlwaysPreTouch", "-jar", $jar, "nogui")
            $proc = Start-Process -FilePath "java" -ArgumentList $javaArgs -WorkingDirectory $RunDir `
                -RedirectStandardOutput $LogFilePath -RedirectStandardError $ErrFilePath -NoNewWindow -PassThru
        }
        "folia" {
            $jar = Join-Path $serversDir "folia\server.jar"
            if (-not (Test-Path $jar)) { throw "Not found: $jar" }
            $javaArgs = @("-Xms2G", "-Xmx2G", "-XX:+AlwaysPreTouch", "-jar", $jar, "nogui")
            $proc = Start-Process -FilePath "java" -ArgumentList $javaArgs -WorkingDirectory $RunDir `
                -RedirectStandardOutput $LogFilePath -RedirectStandardError $ErrFilePath -NoNewWindow -PassThru
        }
        "pumpkin" {
            $pumpkinExe = if ($IsWindows -or $env:OS -match "Windows") {
                Join-Path $serversDir "pumpkin\pumpkin.exe"
            } else {
                Join-Path $serversDir "pumpkin\pumpkin"
            }
            if (-not (Test-Path $pumpkinExe)) { throw "Not found: $pumpkinExe" }
            # Write pumpkin config
            $cfg = @"
online_mode = false
max_players = 20
seed = $Seed
"@
            [System.IO.File]::WriteAllText((Join-Path $RunDir "config.toml"), $cfg)
            $proc = Start-Process -FilePath $pumpkinExe -WorkingDirectory $RunDir `
                -RedirectStandardOutput $LogFilePath -RedirectStandardError $ErrFilePath -NoNewWindow -PassThru
        }
        "neutron" {
            $cargoArgs = @("run", "--release", "-p", "neutron-cli")
            $proc = Start-Process -FilePath "cargo" -ArgumentList $cargoArgs -WorkingDirectory $BaseDir `
                -RedirectStandardOutput $LogFilePath -RedirectStandardError $ErrFilePath -NoNewWindow -PassThru
        }
    }

    # Wait for "Done" line
    Write-Status "Waiting for server to start (looking for 'Done' line)..."
    $timeoutSec = 120
    $deadline = (Get-Date).AddSeconds($timeoutSec)
    $found = $false
    $lastDone = ""

    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 500
        if (Test-Path $LogFilePath) {
            $content = Get-Content -Path $LogFilePath -Raw
            if ($content -match "Done \(([\d.]+)s\)!") {
                $doneSeconds = [double]::Parse($matches[1])
                $startupMs = $doneSeconds * 1000
                $found = $true
                Write-Status "Server ready! Startup: ${startupMs}ms"
                return @{ proc = $proc; startupMs = $startupMs; log = $LogFilePath }
            }
        }
        if ($proc.HasExited) {
            Write-Status "Server exited prematurely. Log tail:"
            if (Test-Path $LogFilePath) { Get-Content -Path $LogFilePath -Tail 5 }
            throw "Server exited before 'Done' line"
        }
    }

    if (-not $found) { throw "Server did not start within ${timeoutSec}s" }
}

function Stop-All {
    param([bool]$Graceful = $true)
    if ($serverProc -and -not $serverProc.HasExited) {
        Write-Status "Stopping server (PID: $($serverProc.Id))..."
        try { $serverProc.Kill() } catch {}
    }
    if ($memJob) { Stop-Job $memJob 2>$null; Remove-Job $memJob 2>$null }
    # Kill any lingering java/cargo processes
    try { taskkill /F /IM java.exe 2>&1 | Out-Null } catch {}
    try { taskkill /F /IM pumpkin.exe 2>&1 | Out-Null } catch {}
}

# ── Main ────────────────────────────────────────────────────────────────────
try {
    Write-Status "=== Neutron Benchmark Harness (PowerShell) ==="
    Write-Status "Server: $Server | N=$N | Runs=$Runs | Seed=$Seed | Warmup=${WarmupSec}s"

    $dateStamp  = Get-Date -Format "yyyyMMdd-HHmmss"
    $runId      = "${Server}-${N}j"

    $botVersion = Get-ServerVersion -ServerType $Server
    $verStr     = $botVersion
    $hardware   = Get-HardwareInfo

    # Per-run results
    $runDetails = @()
    $allStartup = @()

    for ($runIdx = 0; $runIdx -lt $Runs; $runIdx++) {
        $runLogDir = Join-Path $LogDirPath "${runId}-${dateStamp}-run$($runIdx+1)"
        New-Item -ItemType Directory -Path $runLogDir -Force | Out-Null

        Write-Status "--- Run $($runIdx+1)/$Runs ---"

        # Start memory watcher
        $statsFile = Join-Path $runLogDir "memory.json"
        $memWatchSec = if ($MemWatchSec -gt 0) { $MemWatchSec } else { $WarmupSec + 30 }
        $memJob = Start-MemoryWatcher -StatsFile $statsFile -DurationSec $memWatchSec

        # Start server
        $serverResult = Start-Server -ServerType $Server -RunDir $runLogDir -Seed $Seed -WarmupSec $WarmupSec
        $serverProc    = $serverResult.proc
        $startupMs     = $serverResult.startupMs
        $serverLog     = $serverResult.log
        $allStartup   += $startupMs

        # Warmup
        Write-Status "Warmup ${WarmupSec}s..."
        Start-Sleep -Seconds $WarmupSec

        # Launch bots
        $outputPath = Join-Path $runLogDir "latency.json"
        $botVersion = Get-ServerVersion -ServerType $Server

        $botCmd = Get-BotPath -ServerType $Server
        Write-Status "Bot command: $botCmd --host 127.0.0.1 --port 25565 --count $N --version $botVersion --output $outputPath"

        # Actually run the bot
        $useAzalea = Test-Path $azaleaBin
        if ($useAzalea) {
            $botProc = Start-Process -FilePath $azaleaBin -ArgumentList @(
                "--host", "127.0.0.1", "--port", "25565", "--count", "$N",
                "--version", $botVersion, "--output", $outputPath
            ) -WorkingDirectory $runLogDir -NoNewWindow -PassThru
        } else {
            # Use mineflayer (Node.js)
            $botScript = Join-Path $benchJoin "index.js"
            $botProc = Start-Process -FilePath "node" -ArgumentList @(
                "`"$botScript`"", "--host", "127.0.0.1", "--port", "25565",
                "--count", "$N", "--version", $botVersion, "--output", "`"$outputPath`""
            ) -WorkingDirectory $runLogDir -NoNewWindow -PassThru
        }

        # Wait for bots
        $botTimeout = 30
        $botDeadline = (Get-Date).AddSeconds($botTimeout)
        while ((Get-Date) -lt $botDeadline) {
            Start-Sleep -Milliseconds 200
            if ($botProc.HasExited) { break }
        }
        if (-not $botProc.HasExited) { $botProc.Kill(); Write-Status "Bot timeout after ${botTimeout}s" }

        # Read latency results
        $latData = Read-Latencies -FilePath $outputPath
        if ($latData) {
            Write-Status "Bots: $($latData.successful)/$($latData.totalBots) connected | p50=$($latData.p50Ms)ms p95=$($latData.p95Ms)ms p99=$($latData.p99Ms)ms"
        } else {
            Write-Status "No bot latency data"
        }

        # Stop server for this run
        Write-Status "Stopping server..."
        try { $serverProc.Kill() } catch {}
        try { taskkill /F /IM java.exe 2>&1 | Out-Null } catch {}
        try { taskkill /F /IM pumpkin.exe 2>&1 | Out-Null } catch {}

        # Collect run details (use PSCustomObject for property access)
        $runDetails += [PSCustomObject]@{
            run         = $runIdx + 1
            startup_ms  = [math]::Round($startupMs, 1)
            p50_ms      = if ($latData) { $latData.p50Ms } else { $null }
            p95_ms      = if ($latData) { $latData.p95Ms } else { $null }
            p99_ms      = if ($latData) { $latData.p99Ms } else { $null }
            peak_ram_mb = if (Test-Path $statsFile) {
                $memData = Get-Content $statsFile -Raw | ConvertFrom-Json
                ($memData | Measure-Object -Property workingSetMB -Maximum).Maximum
            } else { $null }
            bot_success = if ($latData) { $latData.successful } else { 0 }
            bot_failed  = if ($latData) { $latData.failed } else { 0 }
        }
    }

    # ── Aggregate results ───────────────────────────────────────────────────
    $startupMedian = if ($allStartup.Count -gt 0) {
        $sorted = $allStartup | Sort-Object
        Get-Percentile -SortedValues $sorted -Percentile 50
    } else { $null }

    $nonNullP50 = $runDetails | Where-Object { $null -ne $_.p50_ms }
    $nonNullP95 = $runDetails | Where-Object { $null -ne $_.p95_ms }
    $nonNullP99 = $runDetails | Where-Object { $null -ne $_.p99_ms }
    $overallStats = @{
        p50 = if ($nonNullP50.Count -gt 0) { ($nonNullP50 | Measure-Object -Property p50_ms -Average).Average } else { $null }
        p95 = if ($nonNullP95.Count -gt 0) { ($nonNullP95 | Measure-Object -Property p95_ms -Average).Average } else { $null }
        p99 = if ($nonNullP99.Count -gt 0) { ($nonNullP99 | Measure-Object -Property p99_ms -Average).Average } else { $null }
    }

    # ── Write JSON output ──────────────────────────────────────────────────
    $jsonResult = @{
        benchmarkId  = $runId
        date         = $dateStamp
        server       = $Server
        version      = $verStr
        seed         = $SeedStr
        botCount     = $N
        runs         = $Runs
        warmupSec    = $WarmupSec
        hardware     = $hardware
        aggregate    = @{
            startup_ms = [math]::Round($startupMedian, 1)
            p50_ms     = if ($overallStats.p50) { [math]::Round($overallStats.p50, 1) } else { $null }
            p95_ms     = if ($overallStats.p95) { [math]::Round($overallStats.p95, 1) } else { $null }
            p99_ms     = if ($overallStats.p99) { [math]::Round($overallStats.p99, 1) } else { $null }
        }
        runs_detail  = $runDetails
        notes        = "Baseline B0 - ${Server} 26.2 - Windows - ${Runs} runs"
    }

    $jsonOut = Join-Path $ResultsDir "${runId}-${dateStamp}.json"
    $jsonResult | ConvertTo-Json -Depth 5 | Set-Content -Path $jsonOut -Encoding UTF8
    Write-Status "JSON written: $jsonOut"

    # ── Write Markdown summary ─────────────────────────────────────────────
    $mdLines = @(
        "# Benchmark ${Server} - ${runId} - ${dateStamp}",
        "",
        "OS: $($hardware.os) - CPU: $($hardware.cpu) - RAM: $($hardware.ram_gb)GB - Seed: $Seed",
        "View: 10 - Sim: 10 - online-mode: false",
        "Warmup: ${WarmupSec}s - Runs: $Runs (median)",
        "",
        "| Metric | Value |",
        "|---|---|",
        "| Server | ${Server} |",
        "| Version | ${verStr} |",
        "| Startup (median) | $([math]::Round($startupMedian, 1)) ms |",
        "| Join p50 | $(if ($overallStats.p50) { [math]::Round($overallStats.p50, 1) } else { 'TBD' }) ms |",
        "| Join p95 | $(if ($overallStats.p95) { [math]::Round($overallStats.p95, 1) } else { 'TBD' }) ms |",
        "| Join p99 | $(if ($overallStats.p99) { [math]::Round($overallStats.p99, 1) } else { 'TBD' }) ms |",
        "| Bot success | $(($runDetails | Measure-Object -Property bot_success -Sum).Sum)/$($Runs * $N) |",
        "",
        "## Per-Run Detail",
        "",
        "| Run | Startup (ms) | p50 (ms) | p95 (ms) | p99 (ms) | Bot success | Bot failed |",
        "|---|---|---|---|---|---|---|"
    )

    foreach ($r in $runDetails) {
        $mdLines += "| $($r.run) | $($r.startup_ms) | $($r.p50_ms) | $($r.p95_ms) | $($r.p99_ms) | $($r.bot_success) | $($r.bot_failed) |"
    }

    $mdOut = Join-Path $ResultsDir "${runId}-${dateStamp}.md"
    $mdLines | Set-Content -Path $mdOut -Encoding UTF8
    Write-Status "Markdown written: $mdOut"

    Write-Status "=== Done ==="
    Write-Status "JSON:  $jsonOut"
    Write-Status "MD:    $mdOut"
    Write-Status "Logs:  $LogDirPath"

} catch {
    Write-Error "Benchmark failed: $_"
    Write-Status "Stopping server on error..."
    Stop-All -Graceful $false
    throw
} finally {
    Stop-All -Graceful $false
}
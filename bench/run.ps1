<#
  run.ps1 — Neutron benchmark harness (PowerShell 7 / Windows)

  Usage:
    .\bench\run.ps1 -Server vanilla [-N 10] [-Runs 5] [-Seed 1234567890123456789]
    .\bench\run.ps1 -Server paper   -N 10 -Runs 5
    .\bench\run.ps1 -Server pumpkin -N 10 -Runs 5
    .\bench\run.ps1 -Server neutron -N 10 -Runs 5

  Requirements:
    - Node.js (for join-bench bots)
    - Java 25 (for vanilla/paper)
    - Server binary in bench/servers/<type>/
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet("vanilla", "paper", "pumpkin", "neutron")]
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
    [int]$MemWatchSec = 90  # 60 warmup + 30 post-warmup (fix: was 30, now 90)
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

# Seed as long (preserves precision for large values)
$Seed = [long]::Parse($SeedStr.Trim())

# ── Paths ──────────────────────────────────────────────────────────────────────
$ScriptRoot  = Split-Path -Parent $MyInvocation.MyCommand.Definition
$BaseDir     = (Get-Item $ScriptRoot\..).FullName
$BenchDir    = Join-Path $ScriptRoot ""
$serversDir  = Join-Path $BenchDir "servers"
$botsDir     = Join-Path $BenchDir "bots"
$benchJoin   = Join-Path $botsDir "join-bench"

$defaultResultsDir = Join-Path $BenchDir "results"
$defaultLogDir     = Join-Path $BenchDir "logs"
$resultsDir = if ($ResultsDir -and $ResultsDir.Trim()) { $ResultsDir } else { $defaultResultsDir }
$logDir     = if ($LogDirPath -and $LogDirPath.Trim()) { $LogDirPath } else { $defaultLogDir }

New-Item -ItemType Directory -Force -Path $resultsDir | Out-Null
New-Item -ItemType Directory -Force -Path $logDir | Out-Null

# ── Globals for cleanup ───────────────────────────────────────────────────────
$serverPid = $null
$botPids = @()
$memJob = $null

# ── Helpers ────────────────────────────────────────────────────────────────────
function Write-Status {
    param([string]$Msg)
    $ts = Get-Date -Format "yyyy-MM-ddTHH:mm:ss.fff"
    $line = "[$ts] $Msg"
    Write-Host $line -ForegroundColor Cyan
    try { Add-Content -Path "$logDir\.harness.log" -Value "$line`n" -Encoding UTF8 } catch {}
}

function Test-Exists {
    param([string]$Cmd)
    $null = Get-Command $Cmd -ErrorAction SilentlyContinue
    return $true
}

function Get-Percentile {
    param([double[]]$Sorted, [double]$Pct)
    if ($Sorted.Count -eq 0) { return 0 }
    $idx = [Math]::Floor(($Pct / 100) * ($Sorted.Count - 1))
    if ($idx -lt 0) { $idx = 0 }
    if ($idx -ge $Sorted.Count) { $idx = $Sorted.Count - 1 }
    return $Sorted[$idx]
}

function Get-LatencyStats {
    param([double[]]$Data)
    if ($Data.Count -eq 0) {
        return @{ p50 = 0.0; p95 = 0.0; p99 = 0.0; avg = 0.0 }
    }
    $sorted = $Data | Sort-Object
    @{
        p50  = [math]::Round((Get-Percentile $sorted 50), 3)
        p95  = [math]::Round((Get-Percentile $sorted 95), 3)
        p99  = [math]::Round((Get-Percentile $sorted 99), 3)
        avg  = [math]::Round(($sorted | Measure-Object -Sum).Sum / $sorted.Count, 3)
    }
}

# ── Latency reading (extract joinLatencies from bot's nested JSON) ───────────
# The bot writes: { results: { joinLatencies: [234.5, ...] } }
# joinLatencies values are in MILLISECONDS (already computed by the bot)
# We extract them as-is — no conversion needed
function Read-Latencies {
    param([string]$FilePath)
    $latencies = [System.Collections.Generic.List[double]]::new()

    if (-not (Test-Path $FilePath)) { return [double[]]$latencies.ToArray() }

    $raw = Get-Content $FilePath -Raw -ErrorAction SilentlyContinue
    if (-not $raw) { return [double[]]$latencies.ToArray() }

    try {
        $parsed = $raw | ConvertFrom-Json
        $values = @()

        # Primary: bot's nested structure { results: { joinLatencies: [...] } }
        if ($parsed.PSObject.Properties['results'] -and
            $parsed.results.PSObject.Properties['joinLatencies']) {
            $values = @($parsed.results.joinLatencies)
        }
        # Fallback: top-level array
        elseif ($parsed -is [array]) {
            $values = $parsed
        }
        # Fallback: top-level object with known property names
        elseif ($parsed -is [PSCustomObject]) {
            foreach ($p in $parsed.PSObject.Properties) {
                if ($p.Name -match 'joinLatency|latency|lat\b' -and $p.Value -is [array]) {
                    $values = @($p.Value)
                    break
                }
            }
            if ($values.Count -eq 0) {
                $values = @($parsed)
            }
        }
        else { return [double[]]$latencies.ToArray() }

        # Convert each value from SECONDS to MILLISECONDS
        foreach ($v in $values) {
            $val = $null
            if ($v -is [double] -or $v -is [int] -or $v -is [single] -or $v -is [long]) {
                $val = [double]$v
            }
            elseif ($v -is [string]) {
                try { $val = [double]::Parse($v) } catch {}
            }
            elseif ($v -is [PSCustomObject]) {
                # Extract from nested object (e.g. { join_latency: 0.234 })
                foreach ($pname in 'join_latency', 'latency', 'lat', 't') {
                    if ($v.PSObject.Properties[$pname] -ne $null) {
                        try { $val = [double]$v.PSObject.Properties[$pname].Value } catch {}
                        break
                    }
                }
            }
            if ($val -ne $null -and [double]::IsNaN($val) -eq $false) {
                # Bot already outputs ms, no conversion needed
                $latencies.Add([math]::Round($val, 3))
            }
        }
    }
    catch {
        Write-Warning "Read-Latencies: failed to parse $FilePath : $_"
    }

    return [double[]]$latencies.ToArray()
}

# ── TPS measurement (Paper: spark plugin) ──────────────────────────────────────
# TBD: Full TPS measurement requires RCON or bot-based command execution.
# Paper ships spark; others need server-specific metrics endpoints.
# TODO: implement when RCON is configured or metrics are available.
function Measure-TPS {
    param(
        [string]$ServerType,
        [string]$ServerLog,
        [string]$OutputDir,
        [int]$DurationSec = 30
    )
    $tpsFile = Join-Path $OutputDir "tps.json"
    $result = @{
        tps_p99_ms = $null
        notes      = "TBD — TPS measurement not yet implemented"
    }

    if ($ServerType -eq "paper") {
        # Paper has spark included. Probe spark HTTP endpoint if available.
        # spark runs on :8181 by default; enable spark http via spark config.
        # For now: TBD — requires spark config or RCON command execution.
        $result.notes = "spark TBD — Paper has spark but probe not yet implemented"
    }
    else {
        switch ($ServerType) {
            "vanilla"  { $result.notes = "TBD — vanilla TPS requires paper spark or custom metrics" }
            "pumpkin"  { $result.notes = "TBD — pumpkin TPS requires server metrics endpoint" }
            "neutron"  { $result.notes = "TBD — neutron TPS requires metrics endpoint / bench mode" }
        }
    }

    $result | ConvertTo-Json | Set-Content -Path $tpsFile -Encoding UTF8
    return $result
}

# ── CPS measurement placeholder ───────────────────────────────────────────────
# cps = chunks generated per second (sustained).
# Vanilla/Paper: Chunky plugin (chunky radius N, chunky start, chunky progress).
# Pumpkin/Neutron: counter from server metrics or equivalent load method.
# TODO: implement once Chunky/server metrics are integrated.
function Measure-CPS {
    param(
        [string]$ServerType,
        [int]$Radius = 64,  # radius in chunks (≈4096 blocks)
        [string]$OutputDir
    )
    $cpsFile = Join-Path $OutputDir "cps.json"
    $result = @{
        cps    = $null
        radius = $Radius
        notes  = "TBD — Chunky for Vanilla/Paper, server counter for Pumpkin/Neutron"
    }

    $result | ConvertTo-Json | Set-Content -Path $cpsFile -Encoding UTF8
    return $result
}

# ── Memory watcher (background, samples RSS every second) ────────────────────
function Start-MemoryWatcher {
    param(
        [int]$Pid,
        [string]$StatsFile,
        [int]$DurationSec = 90,  # FIX: was 30, now 90 (60 warmup + 30 post-warmup)
        [int]$IntervalMs = 1000
    )
    $scriptBlock = {
        param($Pid, $StatsFile, $DurationSec, $IntervalMs)
        $endSec = (Get-Date -UFormat %s) + $DurationSec
        $samples = @()
        while ((Get-Date -UFormat %s) -lt $endSec) {
            try {
                $proc = Get-Process -Id $Pid -ErrorAction SilentlyContinue
                if ($proc) {
                    $rssMB = [math]::Round($proc.WorkingSet64 / 1MB, 2)
                    $samples += @{ ts = (Get-Date -Format "o"); rss_mb = $rssMB }
                }
            } catch { break }
            Start-Sleep -Milliseconds $IntervalMs
        }
        $samples | ConvertTo-Json -Depth 2 | Set-Content -Path $StatsFile -Encoding UTF8
    }
    $job = Start-Job -ScriptBlock $scriptBlock -ArgumentList $Pid, $StatsFile, $DurationSec, $IntervalMs
    return $job
}

# ── Get peak RSS from stats file ─────────────────────────────────────────────
function Get-PeakRSS {
    param([string]$StatsFile)
    if (-not (Test-Path $StatsFile)) { return 0 }
    try {
        $data = Get-Content $StatsFile -Raw | ConvertFrom-Json -ErrorAction SilentlyContinue
        if ($data -and $data.Count -gt 0) {
            $peak = ($data | ForEach-Object { [double]$_.rss_mb }) | Measure-Object -Maximum
            if ($peak) { return [math]::Round($peak.Maximum, 2) }
        }
    } catch {}
    return 0
}

# ── Server config builder ─────────────────────────────────────────────────────
function Build-ServerConfig {
    param([string]$ServerType, [long]$Seed, [string]$WorldPath)
    if ($ServerType -in @("vanilla", "paper")) {
        $propsPath = Join-Path $BenchDir "server.properties"
@"
eula=true
online-mode=false
level-seed=$Seed
view-distance=10
simulation-distance=10
level-name=$WorldPath
max-players=$N
white-list=false
"@ | Set-Content -Path $propsPath -Encoding UTF8
    }
}

# ── Server start helpers ──────────────────────────────────────────────────────
function Start-Server {
    param([string]$ServerType, [string]$ServerDir, [string]$LogFilePath, [long]$Seed, [string]$WorldPath)
    $startTime = Get-Date

    if ($ServerType -in @("vanilla", "paper")) {
        $jar = Join-Path $ServerDir "server.jar"
        if (-not (Test-Path $jar)) { throw "Server jar not found: $jar" }

        $javaArgs = @("-Xms2G", "-Xmx2G", "-XX:+AlwaysPreTouch", "-jar", $jar, "nogui")
        $proc = Start-Process -FilePath "java" `
            -ArgumentList $javaArgs `
            -RedirectStandardOutput $LogFilePath `
            -RedirectStandardError $LogFilePath `
            -NoNewWindow -PassThru

        Write-Status "Waiting for server to start (looking for 'Done' line)..."
        $timeoutSec = 120
        $startSec = Get-Date -UFormat %s
        $found = $false
        $lastDone = ""

        while ((Get-Date -UFormat %s) -lt ($startSec + $timeoutSec) -and -not $found) {
            Start-Sleep -Milliseconds 500
            if (Test-Path $LogFilePath) {
                $content = Get-Content $LogFilePath -Raw -ErrorAction SilentlyContinue
                if ($content) {
                    $matches = [regex]::Matches($content, "Done \([\d.]+s\)")
                    if ($matches.Count -gt 0) {
                        $lastDone = $matches[$matches.Count - 1].Value
                        $found = $true
                    }
                }
            }
        }
        if (-not $found) { throw "Server did not start within ${timeoutSec}s (no 'Done' line in log)" }

        $match = [regex]::Match($lastDone, "\(([\d.]+)s\)")
        if ($match.Success) {
            Write-Status "Server started in $($match.Groups[1].Value)s"
        }

        return @{
            Pid       = $proc.Id
            StartupMs = [math]::Round((Get-Date - $startTime).TotalMilliseconds, 1)
        }
    }
    elseif ($ServerType -eq "pumpkin") {
        $binary = Join-Path $ServerDir "pumpkin"
        if (-not (Test-Path $binary)) { $binary = Join-Path $ServerDir "pumpkin.exe" }
        if (-not (Test-Path $binary)) { throw "Pumpkin binary not found in $ServerDir" }

        $toml = @"
[general]
online_mode = false
seed = $Seed
view_distance = 10
simulation_distance = 10
level_name = "$WorldPath"
max_players = $N
[server]
port = 25565
address = "127.0.0.1"
[motd]
single = "Neutron Benchmark Server"
"@
        $configPath = Join-Path $ServerDir "config.toml"
        Set-Content -Path $configPath -Value $toml -Encoding UTF8

        $proc = Start-Process -FilePath $binary `
            -ArgumentList "--config", "`"$configPath`"", "--world-dir", "`"$WorldPath`"" `
            -RedirectStandardOutput $LogFilePath `
            -RedirectStandardError $LogFilePath `
            -NoNewWindow -PassThru

        Write-Status "Waiting for Pumpkin server to start..."
        $timeoutSec = 60
        $startSec = Get-Date -UFormat %s
        $found = $false

        while ((Get-Date -UFormat %s) -lt ($startSec + $timeoutSec) -and -not $found) {
            Start-Sleep -Milliseconds 500
            if (Test-Path $LogFilePath) {
                $content = Get-Content $LogFilePath -Raw -ErrorAction SilentlyContinue
                if ($content -match 'Done \([\d.]+s\)|started') { $found = $true }
            }
        }
        if (-not $found) { throw "Pumpkin server did not start within ${timeoutSec}s" }

        return @{
            Pid       = $proc.Id
            StartupMs = [math]::Round((Get-Date - $startTime).TotalMilliseconds, 1)
        }
    }
    elseif ($ServerType -eq "neutron") {
        if (-not (Test-Path (Join-Path $BaseDir "Cargo.toml"))) {
            throw "Cargo.toml not found at $BaseDir (must run from neutron repo root)"
        }

        Write-Status "Building neutron..."
        $buildLog = Join-Path $logDir "neutron-build.log"
        $buildProc = Start-Process -FilePath "cargo" `
            -ArgumentList "build", "--release", "-p", "neutron-cli" `
            -RedirectStandardOutput $buildLog `
            -Wait -PassThru

        if ($buildProc.ExitCode -ne 0) {
            throw "Neutron build failed with exit code $($buildProc.ExitCode)"
        }

        $toml = @"
[general]
online_mode = false
seed = $Seed
view_distance = 10
simulation_distance = 10
level_name = "$WorldPath"
max_players = $N
[server]
port = 25565
address = "127.0.0.1"
[motd]
single = "Neutron Benchmark Server"
"@
        $configPath = Join-Path $BenchDir "server.toml"
        Set-Content -Path $configPath -Value $toml -Encoding UTF8

        $binary = Join-Path (Join-Path $BaseDir "target/release") "neutron"
        if (-not (Test-Path $binary)) {
            $binary = Join-Path (Join-Path $BaseDir "target/release") "neutron.exe"
        }
        if (-not (Test-Path $binary)) { throw "Neutron binary not found" }

        $proc = Start-Process -FilePath $binary `
            -ArgumentList "--config", "`"$configPath`"" `
            -RedirectStandardOutput $LogFilePath `
            -RedirectStandardError $LogFilePath `
            -NoNewWindow -PassThru

        Write-Status "Waiting for Neutron server to start..."
        $timeoutSec = 60
        $startSec = Get-Date -UFormat %s
        $found = $false

        while ((Get-Date -UFormat %s) -lt ($startSec + $timeoutSec) -and -not $found) {
            Start-Sleep -Milliseconds 500
            if (Test-Path $LogFilePath) {
                $content = Get-Content $LogFilePath -Raw -ErrorAction SilentlyContinue
                if ($content -match 'Done \([\d.]+s\)|started') { $found = $true }
            }
        }
        if (-not $found) { throw "Neutron server did not start within ${timeoutSec}s" }

        return @{
            Pid       = $proc.Id
            StartupMs = [math]::Round((Get-Date - $startTime).TotalMilliseconds, 1)
        }
    }

    throw "Unknown server type: $ServerType"
}

# ── Server version ────────────────────────────────────────────────────────────
function Get-ServerVersion {
    param([string]$ServerType)
    switch ($ServerType) {
        "vanilla"  { return "26.2" }
        "paper"    { return "paper-latest" }
        "pumpkin"  { return "pumpkin-nightly" }
        "neutron" {
            $cargoToml = Join-Path $BaseDir "Cargo.toml"
            if (Test-Path $cargoToml) {
                $ver = Select-String -Path $cargoToml -Pattern '^version = "(.+)"' |
                    ForEach-Object { $_.Matches[0].Groups[1].Value }
                if ($ver) { return "neutron-$ver" }
            }
            return "neutron-dev"
        }
        default    { return "unknown" }
    }
}

# ── System info ───────────────────────────────────────────────────────────────
function Get-SystemInfo {
    $os  = (Get-CimInstance Win32_OperatingSystem).Caption
    $cpu = (Get-CimInstance Win32_Processor).Name[0]
    $ram = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB, 1)
    return @{ os = $os; cpu = $cpu; ram_gb = $ram }
}

# ── Bot launcher ──────────────────────────────────────────────────────────────
function Start-Bots {
    param([string]$BotLogDir, [string]$OutputPath, [int]$Count, [string]$ServerType)

    $nodePath = if (Test-Exists "node") { "node" } elseif (Test-Exists "nodejs") { "nodejs" } else { $null }
    if (-not $nodePath) { throw "Node.js not found — required for join-bench bots" }

    $botScript = Join-Path $benchJoin "index.js"
    if (-not (Test-Path $botScript)) { throw "Bot script not found: $botScript" }

    Write-Status "Launching $Count join-bench bots..."

    # Pass --version so the bot uses the correct Minecraft protocol for this server.
    $botVersion = if ($ServerType -eq "vanilla" -or $ServerType -eq "paper") { "26.2" } elseif ($ServerType -eq "pumpkin") { "26.2" } else { "26.2" }
    $proc = Start-Process -FilePath $nodePath `
        -ArgumentList "`"$botScript`" --host 127.0.0.1 --port 25565 --count $Count --version $botVersion --output `"$OutputPath`" --log-dir `"$BotLogDir`" --server-type $ServerType" `
        -NoNewWindow -PassThru

    Write-Status "Launched bot worker (PID: $($proc.Id), version: $botVersion)"
    return @{ Pid = $proc.Id }
}

# ── Cleanup ───────────────────────────────────────────────────────────────────
function Stop-All {
    param([bool]$Graceful = $true)

    if ($serverPid) {
        try {
            $proc = Get-Process -Id $serverPid -ErrorAction SilentlyContinue
            if ($proc) {
                if ($Graceful) {
                    $proc.CloseMainWindow() | Out-Null 2>&1
                    Start-Sleep -Milliseconds 2000
                }
                Stop-Process -Id $serverPid -Force -ErrorAction SilentlyContinue
            }
        } catch {
            try { Stop-Process -Id $serverPid -Force -ErrorAction SilentlyContinue } catch {}
        }
        Write-Status "Stopped server (PID: $serverPid)"
    }

    foreach ($pid in $botPids) {
        try { Stop-Process -Id $pid -Force -ErrorAction SilentlyContinue } catch {}
    }

    if ($memJob) { Stop-Job $memJob 2>$null; Remove-Job $memJob 2>$null }
    $serverPid = $null; $botPids = @(); $memJob = $null
}

# ── Signal handler ─────────────────────────────────────────────────────────────
$handler = [ConsoleCancelEventHandler]{
    param($sender, $e)
    Write-Status "`n=== Interrupted (Ctrl+C) — Cleaning up ==="
    Stop-All -Graceful $false
    $e.Cancel = $true
}
[Console]::CancelKeyPress += $handler

# ── Main ──────────────────────────────────────────────────────────────────────
try {
    Write-Status "=== Neutron Benchmark Harness (PowerShell) ==="
    Write-Status "Server: $Server | N=$N | Runs=$Runs | Seed=$Seed | Warmup=${WarmupSec}s"

    $dateStamp  = Get-Date -Format "yyyyMMdd-HHmmss"
    $runId      = "${Server}-${N}j"
    $runLogDir  = Join-Path $logDir $runId
    New-Item -ItemType Directory -Force -Path $runLogDir | Out-Null

    $allStartup  = @()
    $allFlatLat  = [System.Collections.Generic.List[double]]::new()
    $runDetails  = @()

    for ($runIdx = 0; $runIdx -lt $Runs; $runIdx++) {
        Write-Status "--- Run $($runIdx + 1)/$Runs ---"

        $runLogPath  = Join-Path $runLogDir "run-${runIdx}.log"
        $botLogPath  = Join-Path $runLogDir "bots"
        $outputPath  = Join-Path $runLogDir "latency-${runIdx}.json"
        $statsFile   = Join-Path $runLogDir "stats-${runIdx}.json"
        $worldPath   = Join-Path $runLogDir "world-run-${runIdx}"

        if (Test-Path $worldPath) { Remove-Item -Recurse -Force $worldPath }
        New-Item -ItemType Directory -Force -Path $worldPath | Out-Null

        # Build server config
        Build-ServerConfig -ServerType $Server -Seed $Seed -WorldPath $worldPath

        # Start server
        $serverResult = Start-Server `
            -ServerType $Server `
            -ServerDir (Join-Path $serversDir $Server) `
            -LogFilePath $runLogPath `
            -Seed $Seed `
            -WorldPath $worldPath
        $serverPid = $serverResult.Pid

        # Warmup: idle before bots
        Write-Status "Warmup: ${WarmupSec}s idle..."
        Start-Sleep -Seconds $WarmupSec

        # Memory watcher: run during warmup + post-warmup (DurationSec >= WarmupSec)
        $memWatchSec = if ($MemWatchSec -gt 0) { $MemWatchSec } else { $WarmupSec + 30 }
        $memJob = Start-MemoryWatcher `
            -Pid $serverPid `
            -StatsFile $statsFile `
            -DurationSec $memWatchSec

        # Launch bots (using --output, not --latency-file)
        $botResult = Start-Bots `
            -BotLogDir $botLogPath `
            -OutputPath $outputPath `
            -Count $N `
            -ServerType $Server
        $botPids = @($botResult.Pid)

        # Wait for bots to finish
        Write-Status "Waiting for bots to connect..."
        $waitedSec = 0
        $botDone = $false
        while ($waitedSec -lt 60 -and -not $botDone) {
            Start-Sleep -Milliseconds 1000
            $waitedSec++
            try {
                $proc = Get-Process -Id $botResult.Pid -ErrorAction SilentlyContinue
                if (-not $proc) { $botDone = $true }
            } catch { $botDone = $true }
        }

        # Stop memory watcher
        Stop-Job $memJob 2>$null
        Remove-Job $memJob 2>$null
        $memJob = $null

        # Read latencies (FIX: handles nested structure, converts s→ms)
        $latencies = Read-Latencies $outputPath

        if ($latencies.Count -eq 0) { Write-Warning "No latency data for run $($runIdx + 1)" }

        # Compute percentiles
        $stats = Get-LatencyStats ([double[]]$latencies)

        # Peak RAM from memory watcher
        $peakRam = Get-PeakRSS -StatsFile $statsFile

        # TPS measurement (Paper: spark, others: TBD)
        $tpsResult = Measure-TPS -ServerType $Server -ServerLog $runLogPath -OutputDir $runLogDir -DurationSec 10

        # CPS measurement (placeholder — Chunky TBD)
        $cpsResult = Measure-CPS -ServerType $Server -OutputDir $runLogDir

        # Store run detail
        $runDetail = @{
            run         = $runIdx + 1
            startup_ms  = $serverResult.StartupMs
            p50_ms      = $stats.p50
            p95_ms      = $stats.p95
            p99_ms      = $stats.p99
            avg_ms      = $stats.avg
            peak_ram_mb = $peakRam
            n_bots      = $latencies.Count
            latencies   = [double[]]$latencies
            tps_p99_ms  = $tpsResult.tps_p99_ms
            cps         = $cpsResult.cps
        }
        $runDetails += $runDetail
        $allStartup += $serverResult.StartupMs

        foreach ($l in $latencies) { $allFlatLat.Add([double]$l) }

        Write-Status "Run $($runIdx + 1): startup=$($serverResult.StartupMs)ms p50=$($stats.p50)ms p95=$($stats.p95)ms p99=$($stats.p99)ms peakRAM=${peakRam}MB"

        # Kill server for next run
        Stop-All -Graceful $true
    }

    # ── Aggregate ──────────────────────────────────────────────────────────
    Write-Status "=== Aggregating results ==="

    $sortedStartup = $allStartup | Sort-Object
    $midIdx = [Math]::Floor($Runs / 2)
    if ($midIdx -ge $sortedStartup.Count) { $midIdx = $sortedStartup.Count - 1 }
    $startupMedian = [math]::Round($sortedStartup[$midIdx], 2)

    $overallStats = Get-LatencyStats ([double[]]$allFlatLat.ToArray())

    $version  = Get-ServerVersion -ServerType $Server
    $hwInfo   = Get-SystemInfo

    # TPS / CPS aggregated (use first run's values as representative)
    $tpsAgg = $null
    $cpsAgg = $null
    if ($runDetails.Count -gt 0) {
        if ($runDetails[0].tps_p99_ms -ne $null) { $tpsAgg = $runDetails[0].tps_p99_ms }
        if ($runDetails[0].cps -ne $null) { $cpsAgg = $runDetails[0].cps }
    }

    # RAM idle ≈ average of first 3 samples from first run's memory watcher
    $ramIdle = 0
    $ramIdleStats = Join-Path $runLogDir "stats-0.json"
    if (Test-Path $ramIdleStats) {
        try {
            $firstSamples = Get-Content $ramIdleStats -Raw | ConvertFrom-Json -ErrorAction SilentlyContinue
            if ($firstSamples -and $firstSamples.Count -gt 0) {
                $avgRam = (($firstSamples | Select-Object -First 3 | ForEach-Object { [double]$_.rss_mb }) | Measure-Object -Average).Average
                $ramIdle = [math]::Round($avgRam, 2)
            }
        } catch {}
    }

    # RAM 100j: TBD (would require 100-concurrent-bots stress test)
    $ram100j = $null
    # CPU idle: TBD (requires OS-level CPU monitoring)
    $cpuIdle = $null

    # ── Write JSON ─────────────────────────────────────────────────────────
    $jsonResult = @{
        test_name   = "join-bench"
        server_type = $Server
        version     = $version
        date        = $dateStamp
        seed        = "$Seed"               # string to preserve precision for large seeds
        n_bots      = $N
        runs        = $Runs
        aggregate = [ordered]@{
            startup_ms    = $startupMedian
            join_p50_ms   = $overallStats.p50
            join_p95_ms   = $overallStats.p95
            join_p99_ms   = $overallStats.p99
            all_latencies = @([double[]]$allFlatLat.ToArray() | ForEach-Object { [math]::Round($_, 3) })
            tps_p99_ms    = $tpsAgg
            cps           = $cpsAgg
            ram_idle_mb   = $ramIdle
            ram_100j_mb   = $ram100j
            cpu_idle_pct  = $cpuIdle
        }
        runs_detail = $runDetails | ForEach-Object {
            $r = $_
            [ordered]@{
                run         = $r.run
                startup_ms  = $r.startup_ms
                p50_ms      = $r.p50_ms
                p95_ms      = $r.p95_ms
                p99_ms      = $r.p99_ms
                avg_ms      = $r.avg_ms
                peak_ram_mb = $r.peak_ram_mb
                n_bots      = $r.n_bots
                tps_p99_ms  = $r.tps_p99_ms
                cps         = $r.cps
                latencies   = $r.latencies | ForEach-Object { [math]::Round($_, 3) }
            }
        }
        hardware = $hwInfo
    }

    $jsonOut = Join-Path $resultsDir "${runId}-${dateStamp}.json"
    $jsonResult | ConvertTo-Json -Depth 5 | Set-Content -Path $jsonOut -Encoding UTF8
    Write-Status "JSON written: $jsonOut"

    # ── Write Markdown ─────────────────────────────────────────────────────
    # BENCHMARKS.md §8 template columns:
    # | Server | Version | Startup | RAM idle | RAM 100j | CPU idle | cps | TPS p99 | Join p50 | Join p95 |
    $tpsStr = if ($tpsAgg -ne $null) { "$tpsAgg ms" } else { "TBD" }
    $cpsStr = if ($cpsAgg -ne $null) { "$cpsAgg" } else { "TBD" }

    $mdLines = @(
        "# Benchmark ${Server} — ${runId} — ${dateStamp}",
        "",
        "OS: ${hwInfo.os} · CPU: ${hwInfo.cpu} · RAM: ${hwInfo.ram_gb}GB · Seed: ${Seed}",
        "View: 10 · Sim: 10 · online-mode: false",
        "Warmup: ${WarmupSec}s · Runs: ${Runs} (median)",
        "",
        "| Metric | Value |",
        "|---|---|",
        "| Server | ${Server} |",
        "| Version | ${version} |",
        "| Startup (median) | ${startupMedian} ms |",
        "| RAM idle | ${ramIdle} MB |",
        "| RAM 100j | TBD |",
        "| CPU idle | TBD |",
        "| cps | ${cpsStr} |",
        "| TPS p99 | ${tpsStr} |",
        "| Join p50 | ${overallStats.p50} ms |",
        "| Join p95 | ${overallStats.p95} ms |",
        "",
        "## Per-Run Detail",
        "",
        "| Run | Startup (ms) | p50 (ms) | p95 (ms) | p99 (ms) | Peak RAM (MB) |",
        "|---|---|---|---|---|---|"
    )

    foreach ($r in $runDetails) {
        $mdLines += "| $($r.run) | $($r.startup_ms) | $($r.p50_ms) | $($r.p95_ms) | $($r.p99_ms) | $($r.peak_ram_mb) |"
    }

    $mdOut = Join-Path $resultsDir "${runId}-${dateStamp}.md"
    $mdLines | Set-Content -Path $mdOut -Encoding UTF8
    Write-Status "Markdown written: $mdOut"

    Write-Status "=== Done ==="
    Write-Status "JSON:  $jsonOut"
    Write-Status "MD:    $mdOut"
    Write-Status "Logs:  $runLogDir/"

} catch {
    Write-Error "Benchmark failed: $_"
    Write-Status "Stopping server on error..."
    Stop-All -Graceful $false
    throw
} finally {
    Stop-All -Graceful $false
}
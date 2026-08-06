# Benchmark Harness

Automated benchmarking tool for Neutron Minecraft server comparisons. Runs **vanilla**, **Paper**, **Pumpkin**, and **Neutron** servers under identical conditions (same seed, same bot load, same warmup) and outputs structured JSON + markdown reports.

## Table of contents

- [Quick start](#quick-start)
- [Prerequisites](#prerequisites)
- [Arguments](#arguments)
- [How it works](#how-it-works)
- [Output format](#output-format)
- [Server types](#server-types)
- [Bots](#bots)
- [Interpreting results](#interpreting-results)
- [Baselines](#baselines)
- [Troubleshooting](#troubleshooting)
- [Extending](#extending)
- [CI integration](#ci-integration)

---

## Quick start

**Windows (PowerShell 7):**

```powershell
.\bench\run.ps1 -Server vanilla -N 10 -Runs 5
```

**Linux (bash):**

```bash
./bench/run.sh vanilla -n 10 --runs 5
```

Both commands: start a vanilla 26.2 server → wait for "Done" → 60 s warmup → spawn 10 bots → collect metrics → write `bench/results/<ID>-<date>.json` + `.md` report.

---

## Prerequisites

| Dependency | Required for | Minimum version | Notes |
|---|---|---|---|
| **Node.js** | Bot script (`join-bench`) | 18 | Check `node --version` |
| **Java 25** | Vanilla / Paper servers | 25 | Required by Minecraft 26.x |
| **Server binary** | Each server type | — | See [Server types](#server-types) |
| **jq** (Linux only) | JSON output parsing | Any | Falls back to `grep` if absent |
| **Rust toolchain** | Neutron server | stable | `cargo build --release -p neutron-cli` runs at bench start |

> **Note:** Run `npm install` in `bench/bots/join-bench/` to install bot dependencies (`node_modules/` is ~100 MB, git-ignored).

### Server binaries

Place binaries in `bench/servers/<type>/` (relative to `bench/`):

| Type | Binary | Expected path |
|---|---|---|
| `vanilla` | `server.jar` | `bench/servers/vanilla/server.jar` |
| `paper` | `server.jar` | `bench/servers/paper/server.jar` |
| `pumpkin` | `pumpkin` (Linux) / `pumpkin.exe` (Windows) | `bench/servers/pumpkin/pumpkin` |
| `neutron` | Built from source | Built at bench start via `cargo run --release -p neutron-cli` from repo root |

For Java servers (vanilla, paper), a `server.properties` is auto-generated at bench start with these values:

```
eula=true
online-mode=false
level-seed=<fixed>
view-distance=10
simulation-distance=10
level-name=<run-id>
max-players=<N>
white-list=false
```

For Pumpkin, a `config.toml` is auto-generated:

```toml
[general]
online_mode = false
seed = <seed>
view_distance = 10
simulation_distance = 10
level_name = <run-id>
max_players = <N>
[server]
port = 25565
address = "127.0.0.1"
[motd]
single = "Neutron Benchmark Server"
```

---

## Arguments

### Harness script (`run.ps1` / `run.sh`)

Both scripts accept the same logical parameters but with different CLI syntax.

| Parameter | PowerShell (`run.ps1`) | Bash (`run.sh`) | Default | Description |
|---|---|---|---|---|
| **Server type** (positional) | `-Server vanilla` | `run.sh vanilla` | *(required)* | `vanilla` · `paper` · `pumpkin` · `neutron` |
| **Bot count** | `-N 20` | `-n 20` or `--bots 20` | `10` | Number of concurrent bot connections |
| **Runs** | `-Runs 10` | `--runs 10` | `5` | Number of iterations (each restarts the server) |
| **Seed** | `-Seed 1234567890123456789` | `--seed 1234567890123456789` | `1234567890123456789` | Fixed world seed (string, preserved with full precision) |
| **Warmup seconds** | `-WarmupSec 60` | `--warmup 60` | `60` | Idle time before spawning bots (JIT/caches) |
| **Memory watch seconds** | `-MemWatchSec 90` | `--mem-watch 90` | `90` | Duration for RSS sampling (warmup + post-warmup) |
| **Results dir** | `-ResultsDir C:\out` | `--results-dir /tmp/out` | `bench/results/` | Where JSON + markdown output is written |
| **Log dir** | `-LogDirPath C:\logs` | `--log-dir /tmp/logs` | `bench/logs/` | Where raw per-run logs are stored |
| **World dir** | `-WorldDir C:\worlds` | `--world /tmp/worlds` | Auto-created per run | Override world directory |

**Example (Windows):**

```powershell
.\bench\run.ps1 -Server paper -N 50 -Runs 10 -Seed 9876543210 -WarmupSec 90
```

**Example (Linux):**

```bash
./bench/run.sh paper -n 50 --runs 10 --seed 9876543210 --warmup 90
```

**Performance tip (Linux):** For maximum performance, store the world on a tmpfs to reduce disk I/O:

```bash
./bench/run.sh vanilla -n 10 --runs 5 --world /dev/shm/neutron-world
```

### Bot script (`bench/bots/join-bench/index.js`)

Invoked automatically by the harness. Direct usage:

| Flag | Default | Description |
|---|---|---|
| `--host` | `localhost` | Server hostname |
| `--port` | `25565` | Server port |
| `--count` | `10` | Number of bots to spawn |
| `--version` | `1.21.11` | Minecraft protocol version (target: `26.2`) |
| `--output` | stdout | Write results JSON to file path |
| `-h, --help` | — | Show help |

```bash
node bench/bots/join-bench/index.js --count 20 --output results/join-bench.json
```

---

## How it works

The harness runs each iteration as follows:

```
┌─────────────────────────────────────────────────────────────┐
│ Iteration N                                                 │
├─────────────────────────────────────────────────────────────┤
│ 1. Create clean world dir (empty each run)                  │
│ 2. Generate server.properties / config.toml                 │
│ 3. Start server process → redirect stdout/stderr to log     │
│ 4. Poll log for "Done (Xs)!" regex match (timeout: 120s)    │
│ 5. Record startup_ms (process spawn → "Done" line)          │
│ 6. Warmup: idle for --warmup seconds (default 60)           │
│    └─ Memory watcher runs in background: samples RSS every  │
│       1s during warmup + 30s after warmup (total 90s)       │
│ 7. Launch join-bench bots (N simultaneous, ~2ms stagger)    │
│ 8. Wait for bots to complete (timeout: 60s)                 │
│ 9. Stop memory watcher                                      │
│ 10. Read bot results → compute p50/p95/p99/avg latencies   │
│ 11. Measure TPS (Paper: spark TBD; others: TBD)             │
│ 12. Measure CPS (Chunky: TBD)                               │
│ 13. Record per-run metrics                                  │
│ 14. Kill server → next iteration                          │
└─────────────────────────────────────────────────────────────┘
         ↓ (after all iterations)
┌─────────────────────────────────────────────────────────────┐
│ Aggregation                                                 │
├─────────────────────────────────────────────────────────────┤
│ • Startup: median across all runs                           │
│ • Join latencies: merge all runs → p50/p95/p99             │
│ • RAM idle: avg of first 3 RSS samples from run 1          │
│ • Write JSON (full report) + Markdown (summary table)       │
└─────────────────────────────────────────────────────────────┘
```

### Signal handling

The harness catches `SIGINT` / `SIGTERM` (Ctrl+C / kill):

1. Stops the server and all bot processes
2. Writes any accumulated results to disk
3. Exits cleanly (no orphaned processes)

---

## Output format

### JSON structure

Written to `bench/results/<server>-<Nj>-<date>.json`:

```json
{
  "test_name": "join-bench",
  "server_type": "vanilla",
  "version": "26.2",
  "date": "20260805-143022",
  "seed": "1234567890123456789",
  "n_bots": 10,
  "runs": 5,
  "aggregate": {
    "startup_ms": 8.42,
    "join_p50_ms": 234.5,
    "join_p95_ms": 567.8,
    "join_p99_ms": 890.1,
    "all_latencies": [210.3, 234.5, 256.7, 345.2, 567.8, 890.1, ...],
    "tps_p99_ms": null,
    "cps": null,
    "ram_idle_mb": 1250.5,
    "ram_100j_mb": null,
    "cpu_idle_pct": null
  },
  "runs_detail": [
    {
      "run": 1,
      "startup_ms": 8.2,
      "p50_ms": 220.1,
      "p95_ms": 540.3,
      "p99_ms": 870.5,
      "avg_ms": 280.4,
      "peak_ram_mb": 1300.2,
      "n_bots": 10,
      "tps_p99_ms": null,
      "cps": null,
      "latencies": [210.3, 234.5, 256.7, 345.2, 567.8, 890.1, 102.3, 175.4, 290.6, 310.8]
    },
    ...
  ],
  "hardware": {
    "os": "Windows 11 Pro 23H2",
    "cpu": "Intel Core i7-13700K",
    "ram_gb": 32
  }
}
```

**Key fields:**

| Field | Type | Description |
|---|---|---|
| `aggregate.join_p50_ms` | number | Median join latency across all runs (ms) |
| `aggregate.join_p95_ms` | number | 95th percentile join latency (ms) |
| `aggregate.join_p99_ms` | number | 99th percentile join latency (ms) |
| `aggregate.startup_ms` | number | Median server startup time across runs (ms) |
| `aggregate.ram_idle_mb` | number | Average RSS from first 3 memory samples (MB) |
| `aggregate.all_latencies` | array | All bot latencies merged across all runs (ms) |
| `runs_detail[].latencies` | array | Per-run latency values (ms) |
| `hardware` | object | Detected CPU/RAM/OS from the machine |
| `seed` | string | World seed (string preserves full 64-bit precision) |

### Markdown table

Written to `bench/results/<server>-<Nj>-<date>.md`:

```markdown
# Benchmark vanilla — vanilla-10j — 20260805-143022

OS: Linux · CPU: Intel Core i7-13700K · RAM: 32GB · Seed: 1234567890123456789
View: 10 · Sim: 10 · online-mode: false
Warmup: 60s · Runs: 5 (median)

| Metric | Value |
|---|---|
| Server | vanilla |
| Version | 26.2 |
| Startup (median) | 8420 ms |
| RAM idle | 1250.5 MB |
| RAM 100j | TBD |
| CPU idle | TBD |
| cps | TBD |
| TPS p99 | TBD |
| Join p50 | 234.5 ms |
| Join p95 | 567.8 ms |

## Per-Run Detail

| Run | Startup (ms) | p50 (ms) | p95 (ms) | p99 (ms) | Peak RAM (MB) |
|---|---|---|---|---|---|
| 1 | 8200 | 220.1 | 540.3 | 870.5 | 1300.2 |
| 2 | 8600 | 240.3 | 580.1 | 900.2 | 1310.5 |
| ... | ... | ... | ... | ... | ... |
```

### File layout per run

Each run creates a subdirectory under `bench/logs/<server>-<Nj>/`:

```
bench/logs/vanilla-10j/
├── run-0.log          # Server stdout/stderr for run 1
├── run-1.log          # Server stdout/stderr for run 2
├── bots/
│   └── bot.log        # Bot stdout
├── latency-0.json     # Raw bot results for run 1
├── latency-1.json     # Raw bot results for run 2
├── stats-0.json       # RSS samples for run 1
├── stats-1.json       # RSS samples for run 2
└── per_run.txt        # Pipe-delimited per-run summary (Linux only)
                         # Windows: use `run-n.log` timestamps as fallback
```

---

## Server types

### vanilla

- **Binary:** `bench/servers/vanilla/server.jar`
- **Runtime:** Java 25 with `-Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui`
- **Version:** Reported as `26.2`
- **Config:** `server.properties` auto-generated
- **Notes:** Unmodified Minecraft server. Highest baseline for RAM and startup.

### paper

- **Binary:** `bench/servers/paper/server.jar` (latest build from [papermc.io](https://papermc.io))
- **Runtime:** Same JVM args as vanilla
- **Version:** Reported as `paper-latest`
- **Config:** `server.properties` auto-generated
- **Notes:** Ships [spark](https://spark.lucko.me/) for TPS/health diagnostics. Command rate limit ≈ 15/s — bots should throttle.

### pumpkin

- **Binary:** `bench/servers/pumpkin/pumpkin` (Linux) or `pumpkin.exe` (Windows)
- **Runtime:** Native binary, no JVM
- **Version:** Reported as `pumpkin-nightly`
- **Config:** `config.toml` auto-generated
- **Notes:** Rust server. Self-reported benchmarks show ~5-8 ms startup and ~100 MB idle RAM. Does not support Chunky plugin → CPS measurement TBD.

### neutron

- **Binary:** Built from source at bench start via `cargo build --release -p neutron-cli`
  - First build takes 5–15 minutes (dependency compilation). Subsequent runs reuse the cached binary.
  - **Recommended:** Run `cargo build --release -p neutron-cli` manually before the benchmark to avoid the wait.
- **Runtime:** Built binary from `target/release/neutron` (or `.exe`)
- **Version:** Read from `Cargo.toml` version field, reported as `neutron-<ver>`
- **Config:** `server.toml` auto-generated
- **Notes:** Requires Rust toolchain. Built fresh from repo root on each run. Targets: startup < 2 s, RAM idle < 150 MB, TPS 20.0 @ 500 players.

---

## Bots

### join-bench

Location: `bench/bots/join-bench/`

**What it does:** Spawns N simultaneous mineflayer bot connections to a running server and measures join latency per bot (t0 → login → spawn).

**Implementation:**

- **Library:** mineflayer v4.x (Node.js)
- **Connection:** Bots stagger by ~2 ms each (near-simultaneous test of N=10 bots spans ~18 ms total)
- **Safety timeout:** 15 s per bot — if no event fires, the bot is marked as failed
- **Version:** Default protocol version is `1.21.11` (broad mineflayer compatibility). Target Neutron is 26.2 — use `--version 26.2` if your mineflayer install supports it.

**Output structure:**

```json
{
  "test": "join-bench",
  "timestamp": "2026-08-05T14:30:22.456Z",
  "config": { "host": "localhost", "port": 25565, "version": "1.21.11", "count": 10, "t0Ms": 1722873022456 },
  "results": {
    "allConnected": true,
    "totalBots": 10,
    "successful": 10,
    "failed": 0,
    "joinLatencies": [210.3, 234.5, 256.7, 345.2, 567.8, 890.1, 102.3, 175.4, 290.6, 310.8],
    "p50Ms": 273.65,
    "p95Ms": 567.8,
    "p99Ms": 890.1,
    "totalTimeMs": 3200,
    "perBot": [
      { "index": 0, "loginMs": 150.2, "spawnMs": 210.3, "latencyMs": 60.1, "success": true, "error": null },
      ...
    ]
  }
}
```

**Per-bot fields:**

| Field | Type | Description |
|---|---|---|
| `index` | number | Bot index (0-based) |
| `loginMs` | number | Time from t0 to `login` event (ms) |
| `spawnMs` | number | Time from t0 to `spawn` event (ms) |
| `latencyMs` | number | `spawnMs - loginMs` (time to load world after accepted connection, ms) |
| `success` | boolean | Bot reached spawn without error |
| `error` | string or null | Error message if bot failed |

### Physics quirk

Mineflayer ≥ 1.20.2 requires `physicsEnabled: false` in bot options to prevent kicks during the physics tick simulation before the spawn event. This is enabled by default in `join-bench/index.js`:

```js
const botOpts = {
  host, port, version,
  username: `bench-bot-${index}-${Date.now()}`,
  physicsEnabled: false,  // Required for 1.20.2+
  timeout: 10000,
};
```

### Graceful shutdown

The bot script catches `SIGINT`, `SIGTERM`, and `unhandledRejection` — it writes accumulated results before exiting, ensuring partial data is captured if interrupted mid-run.

---

## Interpreting results

### Metrics

| Metric | What it measures | What to look for |
|---|---|---|
| **Startup (median)** | Server process spawn → `Done (Xs)!` line | Lower is better. Paper/vanilla typically 7-15 s; target for Neutron: < 2 s |
| **Join p50** | Median time from bot creation → spawn | Lower is better. This is the "typical" client experience |
| **Join p95** | 95th percentile join latency | How slow the worst 5% of players experience. Critical for server health perception |
| **Join p99** | 99th percentile join latency | Tail latency. Outliers indicate GC pauses, thread contention, or network issues |
| **RAM idle (MB)** | Average RSS of server process during warmup | Lower is better. Vanilla: ~1-2 GB; target for Neutron: < 150 MB |
| **Peak RAM (MB)** | Maximum RSS observed per run | Watch for memory leaks — peak should stabilize after warmup |

### Comparing servers

Run all four server types on the **same machine** with **the same seed** and **same bot count**:

```bash
# Example: compare all four on the same machine
./bench/run.sh vanilla  -n 10 --runs 5 --seed 1234567890123456789
./bench/run.sh paper    -n 10 --runs 5 --seed 1234567890123456789
./bench/run.sh pumpkin  -n 10 --runs 5 --seed 1234567890123456789
./bench/run.sh neutron  -n 10 --runs 5 --seed 1234567890123456789
```

Then compare `aggregate` values across the JSON outputs. Use the median (`aggregate.startup_ms`, merged `all_latencies`) for cross-run comparison — individual runs can vary due to GC, CPU scheduler, or background processes.

### What good looks like

From [BENCHMARKS.md](../BENCHMARKS.md) baselines:

| Server | Startup | RAM idle | Join p50 (est.) |
|---|---|---|---|
| Vanilla 26.2 | 7-15 s | 0.9-1.8 GB | TBD |
| Paper | 7-10 s | 1.1-2.2 GB | TBD |
| Pumpkin | 5-8 s (self-reported) | ~100 MB | TBD |
| Neutron (target) | < 2 s | < 150 MB | TBD |

---

## Baselines

Self-reported baselines from the community (from [BENCHMARKS.md §6](../BENCHMARKS.md#6-baselines-verificados-agosto-2026)):

| Metric | Vanilla | Paper | Pumpkin (self-reported) | Neutron (target) |
|---|---|---|---|---|
| Startup | 7-15 s | 7-10 s | ~5-8 ms (no preload) | < 2 s |
| RAM idle | 0.9-1.8 GB | 1.1-2.2 GB | ~100 MB | < 150 MB |
| CPU idle | ~24% | ~20% | ~1.5% | TBD |
| cps (gen) | 10.6-14.2 | 17.4-84.8 | not published | > 250 |
| TPS @500 | 20 | 20 | 20 (target) | 20.0, p99 < 25 ms |
| Join p95 @100 | TBD | TBD | TBD | < 2 s |

*Note: Pumpkin numbers are self-reported from [docs.pumpkinmc.org](https://docs.pumpkinmc.org/about/benchmarks) and their own docs note fair comparison is difficult (fewer features). C2ME is a vanilla optimization mod (not a standalone server) — see the [ishland gist](https://gist.github.com/ishland) for the most rigorous published methodology.*

All baselines should be reproduced on the test machine before using as a reference point.

---

## Troubleshooting

### Server doesn't start

**Symptom:** Harness prints `ERROR: Server did not start within Xs (no 'Done' line)`

**Fixes:**
1. Verify the binary exists at the expected path (`bench/servers/<type>/server.jar`, etc.)
2. Check `bench/logs/<run-id>/run-0.log` for Java/server errors
3. For Java servers: ensure Java 25 is installed (`java -version`)
4. For Neutron: ensure you're running from the repo root (`Cargo.toml` must be present)
5. Check for port conflicts: `netstat -ano | findstr :25565` (Windows) or `ss -tlnp | grep 25565` (Linux)

### Bot timeouts / failures

**Symptom:** `allConnected: false` in bot output, or `failed: X` in JSON

**Fixes:**
1. Verify the server is actually running and accepting connections (`telnet localhost 25565`)
2. Check protocol version mismatch: `--version` must match the server's supported protocol
3. Check `bench/logs/<run-id>/bots/bot.log` for mineflayer error messages
4. If bots connect but disconnect immediately, check for anti-cheat or version enforcement on the server

### `jq: command not found` (Linux)

The harness falls back to `grep` for JSON parsing but produces slightly less accurate output. Install it:

```bash
# Ubuntu/Debian
sudo apt install jq

# Fedora/RHEL
sudo dnf install jq

# macOS
brew install jq
```

### Out of memory

**Symptom:** Server crashes or OOM kills the process

**Fixes:**
1. Reduce `--bots N` (fewer concurrent connections = less RAM)
2. For Java servers, JVM args are fixed at `-Xms2G -Xmx2G` — adjust in `run.sh`/`run.ps1` if needed
3. Ensure you have enough free RAM. With `view-distance=10`, a vanilla server typically uses 1-2 GB

### No latency data in results

**Symptom:** `WARNING: No latency data for run X` or `all_latencies: []`

**Fixes:**
1. **Protocol version mismatch (most common):** The bot default is `1.21.11`; the Neutron server is 26.2. If mineflayer's data files don't support 26.2, bots fail silently. Verify with `node bench/bots/join-bench/index.js --count 1 --output test.json` and check the output for `allConnected: true`.
2. Verify bots actually connected (check `bots/bot.log`)
3. Ensure the server log contains `Done` line before bots start
4. Check that `--warmup` gives the server enough time to be fully ready

### Permission denied (Linux)

**Symptom:** `Permission denied` when running `run.sh` or starting Pumpkin

```bash
chmod +x bench/run.sh
chmod +x bench/servers/pumpkin/pumpkin  # Pumpkin binary
```

---

## Extending

### Adding a new scenario

The harness is iteration-driven: each loop runs one scenario. To add a new scenario (e.g., sustained load), create a new launcher function:

**PowerShell (`run.ps1`):**

```powershell
function Start-LoadBots {
    param([string]$BotLogDir, [string]$OutputPath, [int]$Count, [string]$ServerType)
    # Launch your load test script (e.g., a script that keeps bots connected and doing things)
    $proc = Start-Process -FilePath "node" `
        -ArgumentList "`"$BotLogDir\load-test.js`", "--count $Count", "--output `"$OutputPath`"" `
        -NoNewWindow -PassThru
    return @{ Pid = $proc.Id }
}
```

**Bash (`run.sh`):**

```bash
start_load_bots() {
    local bot_log_dir="$1" output_path="$2" count="$3" server_type="$4"
    node "$bot_log_dir/load-test.js" --count "$count" --output "$output_path" &
    echo $!
}
```

Then integrate it into the main loop similar to how `start_bots` / `Start-Bots` are called.

### Adding a new metric (e.g., TPS, CPS)

TPS and CPS are already scaffolded in the harness as placeholder functions (`Measure-TPS` / `measure_tps`, `Measure-CPS` / `measure_cps`). To implement:

1. **TPS:** Paper ships spark (`/spark tps` or spark HTTP endpoint on :8181). Pumpkin/Neutron need their own metrics endpoints. Implement the probe function, parse the response, and return the value.

2. **CPS:** Vanilla/Paper use the [Chunky](https://github.com/PlayPro/Chunky) plugin (`chunky radius N`, `chunky start`, `chunky progress`). Pumpkin/Neutron need equivalent load mechanisms.

3. Update the JSON output structure and markdown template to include the new metric.

### Adding a new server type

1. Add the type to the `ValidateSet` / argument parser
2. Implement `start_server` / `Start-Server` for the new type
3. Implement `get_server_version` / `Get-ServerVersion`
4. Add config generation if needed (properties, TOML, etc.)
5. Update all TBD notes in the codebase

---

## CI integration

### GitHub Actions example

```yaml
name: Benchmark

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Setup Java 25
        uses: actions/setup-java@v4
        with:
          java-version: '25'
          distribution: temurin

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '18'

      - name: Install bot dependencies
        run: npm install
        working-directory: bench/bots/join-bench

      - name: Download server binaries
        run: |
          # TODO: download server binaries (vanilla, paper, pumpkin) from releases
          mkdir -p bench/servers/vanilla bench/servers/paper bench/servers/pumpkin

      - name: Build Neutron (cached release binary)
        run: cargo build --release -p neutron-cli

      - name: Run join-bench
        run: |
          ./bench/run.sh vanilla -n 10 --runs 5 --seed 1234567890123456789
          ./bench/run.sh neutron  -n 10 --runs 5 --seed 1234567890123456789

      - name: Upload benchmark results
        uses: actions/upload-artifact@v4
        with:
          name: bench-results
          path: bench/results/*.json
          retention-days: 30
```

### Regression detection

Compare `aggregate.join_p95_ms` and `aggregate.startup_ms` against baseline thresholds in CI:

```bash
#!/bin/bash
# Example: fail if p95 join latency exceeds 2000ms
json=$(cat bench/results/vanilla-10j-*.json)
p95=$(jq '.aggregate.join_p95_ms' <<< "$json")
if (( $(echo "$p95 > 2000" | bc -l) )); then
    echo "FAIL: p95 join latency ${p95}ms exceeds 2000ms threshold"
    exit 1
fi
```

### Schedule

The harness is designed for periodic runs:

| Frequency | Purpose | Command |
|---|---|---|
| Daily | Smoke test — verify harness works | Single run, N=5 |
| Weekly | Regression tracking against baselines | Full comparison (all 4 servers) |
| Per-PR | Gate on startup + join metrics | Compare against `main` baseline |

Store historical results in `bench/results/` and track trends over time. Commit the JSON files — they are the ground truth, not the markdown summary.
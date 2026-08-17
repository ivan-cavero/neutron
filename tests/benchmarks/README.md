# Benchmark Harness — neutron

Unified benchmark harness for Minecraft servers. Written entirely in Rust.

## Prerequisites

| Dependency | Required for | Minimum version |
| --- | --- | --- |
| **Rust nightly** | Building neutron-bot (azalea) | nightly |
| **Java 25** | Vanilla / Paper / Folia servers | 25 |
| **Server binaries** | Each server type | See `servers/` |

This directory is its **own Cargo workspace** (it is NOT part of the root
workspace) and pins **nightly** in `rust-toolchain.toml` — azalea requires it.
`rustup` resolves the toolchain from the **current directory**, not from
`--manifest-path`, so **always build from inside this directory**:

```bash
cd tests/benchmarks
cargo build --release
```

> Building from the repo root with `cargo build --release --manifest-path
> tests/benchmarks/Cargo.toml` fails: rustup picks up the root toolchain
> (stable) and azalea's build script aborts with "requires nightly Rust".

Server jars are **gitignored** — download them yourself (see `servers/README.md`).
The harness looks for `servers/<type>/server.jar` (vanilla/paper/folia) and
`servers/pumpkin/pumpkin` (or `.exe` on Windows); if missing it bails out with
a clear error before starting the benchmark.

## Quick start

```bash
cd tests/benchmarks
cargo build --release

# Run all scenarios for vanilla, small size
./target/release/neutron-bench run --server vanilla --size small

# Run only join-storm on paper, medium size, 3 iterations
./target/release/neutron-bench run --server paper --size medium --scenario join-storm --runs 3

# Compare results
./target/release/neutron-bench compare results/vanilla-small-*.json results/paper-small-*.json
```

## Architecture

```text
tests/benchmarks/
├── Cargo.toml                    # Workspace: neutron-bot + neutron-bench
├── rust-toolchain.toml           # nightly (required by azalea)
├── crates/
│   ├── neutron-bot/              # Bot library (scenarios)
│   │   └── src/
│   │       ├── client.rs         # Connection wrapper
│   │       ├── scenarios/        # Benchmark scenarios
│   │       │   ├── join_storm.rs     # N simultaneous bots
│   │       │   ├── distributed.rs    # 1 bot/second
│   │       │   ├── movement.rs       # Movement + jumping
│   │       │   ├── spread.rs         # Spread far apart
│   │       │   └── chunk_gen.rs      # Chunk generation
│   │       ├── metrics.rs        # Percentiles, averages
│   │       └── output.rs         # JSON result types
│   │
│   └── neutron-bench/            # CLI binary: harness + reports
│       └── src/
│           ├── main.rs           # CLI (clap)
│           ├── types.rs          # ServerType, Size, Scenario
│           ├── server.rs         # Lifecycle: start/stop/wait
│           ├── config.rs         # server.properties / config.toml generation
│           ├── harness.rs        # Main orchestration
│           ├── metrics.rs        # RSS, CPU, peak tracking (sysinfo)
│           ├── reporter.rs       # JSON + Markdown output
│           └── hardware.rs       # Hardware detection
│
├── servers/                      # Server binaries (gitignored)
│   ├── vanilla/server.jar
│   ├── paper/server.jar
│   ├── folia/server.jar
│   └── pumpkin/pumpkin
│
├── results/                      # Output: JSON + Markdown
└── logs/                         # Per-run logs
```

## CLI

### `neutron-bench run`

```bash
neutron-bench run \
  --server <vanilla|paper|folia|pumpkin> \
  --size <small|medium|large> \
  [--scenario <join-storm|distributed|movement|spread|chunk-gen>] \
  [--host 127.0.0.1] \
  [--port 25565] \
  [--runs 5] \
  [--seed 1234567890123456789] \
  [--warmup-secs 60] \
  [--duration 60] \
  [--results-dir results] \
  [--log-dir logs]
```

| Parameter | Default | Description |
| --- | --- | --- |
| `--server` | *(required)* | Server type |
| `--size` | *(required)* | Size: small(10), medium(100), large(1000) |
| `--scenario` | all | Specific scenario to run |
| `--runs` | 5 | Iterations per scenario |
| `--warmup-secs` | 60 | Idle warmup seconds |
| `--duration` | 60 | Scenario duration (movement/spread/chunk-gen) |

`--results-dir` and `--log-dir` are resolved relative to the workspace root
(`tests/benchmarks/`); absolute paths are used as-is.

### `neutron-bench compare`

```bash
neutron-bench compare results/vanilla-small-join-storm.json results/paper-small-join-storm.json
```

### `neutron-bench report`

```bash
neutron-bench report results/vanilla-small-join-storm.json
```

## Server sizes

| Size | Bots | Use case |
| --- | --- | --- |
| **small** | 10 | Personal server, friends |
| **medium** | 100 | Community server |
| **large** | 1000 | Massive server (F4+) |

## Scenarios

### 1. Join Storm
N bots connect simultaneously (<200ms total). Measures join latency (t0 → spawn).
**Metrics:** p50/p95/p99 join latency, startup time.

### 2. Distributed
1 bot connects per second for N seconds. Measures behavior under sustained load.
**Metrics:** global p50/p95/p99, latency curve per interval.

### 3. Movement
N bots spawned, move and jump within a 50-block radius. Alternates walk 2s → jump 1s.
**Metrics:** TPS, chunks received, RAM.

### 4. Spread
N bots spawned, each teleported far apart (>1000 blocks between them).
**Metrics:** chunk loading spike, RAM peak, TPS drop.

### 5. Chunk Generation
N bots walk in a straight line (X axis) at walk speed for 60s.
**Metrics:** CPS (total chunks/s), TPS p99, RAM peak.

## Benchmark matrix

| Server | Small (10) | Medium (100) | Large (1000) |
| --- | --- | --- | --- |
| Vanilla 26.2 | 5 scenarios | 5 scenarios | 5 scenarios |
| Paper | 5 scenarios | 5 scenarios | 5 scenarios |
| Folia | 5 scenarios | 5 scenarios | 5 scenarios |
| Pumpkin | 5 scenarios | 5 scenarios | 5 scenarios |

**Total: 60 configurations × N runs each**

## Output

### JSON
Written to `results/<id>.json` with structure:
```json
{
  "benchmark_id": "vanilla-small-join-storm-20260807-143022",
  "server": { "type": "vanilla", "version": "26.2" },
  "scenario": "join-storm",
  "size": "small",
  "n_bots": 10,
  "aggregate": {
    "startup_ms": 1880,
    "join": { "p50": 373, "p95": 406, "p99": 407 },
    "ram": { "idle_mb": 2287, "peak_mb": 2450 },
    "cpu": { "idle_pct": 24.1 }
  },
  "runs_detail": [...],
  "hardware": { "os": "...", "cpu": "...", "ram_gb": 32 }
}
```

### Markdown
Summary table + per-run detail, written to `results/<id>.md`.

## Servers

### vanilla
- **Binary:** `servers/vanilla/server.jar`
- **Runtime:** `java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui`
- **Config:** auto-generated `server.properties`

### paper
- **Binary:** `servers/paper/server.jar`
- **Runtime:** same JVM args as vanilla
- **Notes:** includes spark for TPS. Rate limit ~15/s → bots with throttle.

### folia
- **Binary:** `servers/folia/server.jar`
- **Runtime:** same JVM args
- **Notes:** threaded regions for scale.

### pumpkin
- **Binary:** `servers/pumpkin/pumpkin` (or `.exe`)
- **Runtime:** native, no JVM
- **Config:** auto-generated `config.toml`

## Metrics

| Metric | How it is measured |
| --- | --- |
| **Startup** | Regex `Done (Xs)!` in server log |
| **Join latency** | t0 (bot creation) → spawn (in world) |
| **RAM (RSS)** | sysinfo sampling every 1s |
| **CPU** | sysinfo sampling every 1s |
| **CPS** | chunks received / duration |
| **TPS** | spark HTTP (Paper/Folia) or estimate |

## Baselines (reference)

| Server | Startup | RAM idle | Join p50 |
| --- | --- | --- | --- |
| Vanilla 26.2 | 7-15 s | 0.9-1.8 GB | ~373 ms |
| Paper | 7-10 s | 1.1-2.2 GB | ~560 ms |
| Folia | 8-12 s | 1.5-2.5 GB | ~1695 ms |
| Pumpkin | 5-8 ms | ~100 MB | TBD |
| Neutron (target) | < 2 s | < 150 MB | < 2 s |

## Extending

### Adding a new scenario
1. Create `crates/neutron-bot/src/scenarios/<name>.rs`
2. Add a variant to `Scenario` in `types.rs`
3. Add a case in `harness.rs` and `main.rs`

### Adding a new server
1. Add a variant to `ServerType` in `types.rs`
2. Add an implementation in `server.rs` and `config.rs`
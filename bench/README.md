# Benchmark Harness — neon

Sistema unificado de benchmarks para servidores Minecraft. Escrito completamente en Rust.

## Quick start

```bash
cd bench
cargo build --release

# Run all scenarios for vanilla, small size
./target/release/neutron-bench run --server vanilla --size small

# Run only join-storm on paper, medium size, 3 iterations
./target/release/neutron-bench run --server paper --size medium --scenario join-storm --runs 3

# Compare results
./target/release/neutron-bench compare results/vanilla-small-*.json results/paper-small-*.json
```

## Arquitectura

```
bench/
├── Cargo.toml                    # Workspace: neutron-bot + neutron-bench
├── rust-toolchain.toml           # nightly (requerido por azalea)
├── crates/
│   ├── neutron-bot/              # Librería de bots (escenarios)
│   │   └── src/
│   │       ├── client.rs         # Wrapper de conexión
│   │       ├── scenarios/        # 5 escenarios de benchmark
│   │       │   ├── join_storm.rs     # N bots simultáneos
│   │       │   ├── distributed.rs    # 1 bot/segundo
│   │       │   ├── movement.rs       # Movimiento + salto
│   │       │   ├── spread.rs         # Esparcir lejos
│   │       │   └── chunk_gen.rs      # Generación de chunks
│   │       ├── metrics.rs        # Percentiles, promedios
│   │       └── output.rs         # Tipos de resultado JSON
│   │
│   └── neutron-bench/            # Binario CLI: harness + reportes
│       └── src/
│           ├── main.rs           # CLI (clap)
│           ├── types.rs          # ServerType, Size, Scenario
│           ├── server.rs         # Lifecycle: start/stop/wait
│           ├── config.rs         # Generación server.properties / config.toml
│           ├── harness.rs        # Orquestación principal
│           ├── metrics.rs        # RSS, CPU, peak tracking (sysinfo)
│           ├── reporter.rs       # JSON + Markdown output
│           └── hardware.rs       # Detección de hardware
│
├── servers/                      # Binarios de servidor
│   ├── vanilla/server.jar
│   ├── paper/server.jar
│   ├── folia/server.jar
│   └── pumpkin/pumpkin.exe
│
├── results/                      # Output: JSON + Markdown
└── logs/                         # Logs por run
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
  [--results-dir bench/results] \
  [--log-dir bench/logs]
```

| Parámetro | Default | Descripción |
|-----------|---------|-------------|
| `--server` | *(requerido)* | Tipo de servidor |
| `--size` | *(requerido)* | Tamaño: small(10), medium(100), large(1000) |
| `--scenario` | todos | Escenario específico a ejecutar |
| `--runs` | 5 | Iteraciones por escenario |
| `--warmup-secs` | 60 | Segundos de warmup idle |
| `--duration` | 60 | Duración del escenario (movement/spread/chunk-gen) |

### `neutron-bench compare`

```bash
neutron-bench compare results/vanilla-small-join-storm.json results/paper-small-join-storm.json
```

### `neutron-bench report`

```bash
neutron-bench report results/vanilla-small-join-storm.json
```

## Tamaños de servidor

| Tamaño | Bots | Caso de uso |
|--------|------|-------------|
| **small** | 10 | Server personal, amigos |
| **medium** | 100 | Server comunitario |
| **large** | 1000 | Server masivo (F4+) |

## Escenarios

### 1. Join Storm
N bots se conectan simultáneamente (<200ms total). Mide join latency (t0 → spawn).
**Métricas:** p50/p95/p99 de join latency, startup time.

### 2. Distributed
1 bot se conecta por segundo durante N segundos. Mide comportamiento bajo carga sostenida.
**Métricas:** p50/p95/p99 global, curva de latencia por intervalo.

### 3. Movement
N bots spawned, se mueven y saltan en radio de 50 bloques. Alterna walk 2s → jump 1s.
**Métricas:** TPS, chunks recibidos, RAM.

### 4. Spread
N bots spawned, cada uno teletransportado a posición lejana (>1000 bloques entre ellos).
**Métricas:** chunk loading spike, RAM peak, TPS drop.

### 5. Chunk Generation
N bots caminan en línea recta (eje X) a velocidad de caminata durante 60s.
**Métricas:** CPS (chunks/s total), TPS p99, RAM peak.

## Matriz de benchmarks

| Server | Small (10) | Medium (100) | Large (1000) |
|--------|------------|--------------|--------------|
| Vanilla 26.2 | 5 escenarios | 5 escenarios | 5 escenarios |
| Paper | 5 escenarios | 5 escenarios | 5 escenarios |
| Folia | 5 escenarios | 5 escenarios | 5 escenarios |
| Pumpkin | 5 escenarios | 5 escenarios | 5 escenarios |

**Total: 60 configuraciones × N runs cada una**

## Output

### JSON
Escrito a `bench/results/<id>.json` con estructura:
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
Tabla resumen + detalle por run, escrito a `bench/results/<id>.md`.

## Prerrequisitos

| Dependencia | Requerido para | Versión mínima |
|-------------|----------------|----------------|
| **Rust nightly** | Compilar neutron-bot (azalea) | nightly |
| **Java 25** | Vanilla / Paper / Folia servers | 25 |
| **Server binaries** | Cada tipo de server | Ver `servers/` |

## Servidores

### vanilla
- **Binary:** `servers/vanilla/server.jar`
- **Runtime:** `java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui`
- **Config:** `server.properties` auto-generado

### paper
- **Binary:** `servers/paper/server.jar`
- **Runtime:** Misma JVM args que vanilla
- **Notas:** Incluye spark para TPS. Rate limit ~15/s → bots con throttle.

### folia
- **Binary:** `servers/folia/server.jar`
- **Runtime:** Misma JVM args
- **Notas:** Threaded regions para escala.

### pumpkin
- **Binary:** `servers/pumpkin/pumpkin.exe`
- **Runtime:** Nativo, sin JVM
- **Config:** `config.toml` auto-generado

## Métricas

| Métrica | Cómo se mide |
|---------|--------------|
| **Startup** | Regex `Done (Xs)!` en log del servidor |
| **Join latency** | t0 (creación bot) → spawn (en mundo) |
| **RAM (RSS)** | muestreo sysinfo cada 1s |
| **CPU** | muestreo sysinfo cada 1s |
| **CPS** | chunks recibidos / duración |
| **TPS** | spark HTTP (Paper/Folia) o estimado |

## Baselines (referencia)

| Server | Startup | RAM idle | Join p50 |
|--------|---------|----------|----------|
| Vanilla 26.2 | 7-15 s | 0.9-1.8 GB | ~373 ms |
| Paper | 7-10 s | 1.1-2.2 GB | ~560 ms |
| Folia | 8-12 s | 1.5-2.5 GB | ~1695 ms |
| Pumpkin | 5-8 ms | ~100 MB | TBD |
| Neutron (target) | < 2 s | < 150 MB | < 2 s |

## Extending

### Agregar un nuevo escenario
1. Crear `crates/neutron-bot/src/scenarios/mi_escenario.rs`
2. Agregar variante a `Scenario` en `types.rs`
3. Agregar caso en `harness.rs` y `main.rs`

### Agregar un nuevo servidor
1. Agregar variante a `ServerType` en `types.rs`
2. Agregar implementación en `server.rs` y `config.rs`

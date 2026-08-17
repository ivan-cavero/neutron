# Neutron — Benchmarks: metodología y harness

> v0.3 · 8 ago 2026 · Regla del proyecto: **todo número publicado tiene metodología reproducible y datos crudos**.

## 1. Filosofía

1. Un benchmark sin metodología es marketing. Publicamos: hardware, software, versiones, comandos, datos crudos (`tests/benchmarks/results/*.json`) y tabla markdown autogenerada.
2. **Misma máquina, misma seed, mismo procedimiento** para vanilla, Paper, Folia y Pumpkin. Sin "condiciones especiales" por servidor.
3. Baselines verificados de la comunidad se citan con fuente y se REPRODUCEN en nuestra máquina antes de usarlos como referencia.
4. El benchmark es un artefacto de CI: si una PR regresiona una métrica clave, la PR no entra.

## 2. Arquitectura del harness (v2 — Rust)

El benchmark harness está escrito completamente en **Rust** (nightly, requerido por azalea).

```text
tests/benchmarks/
├── Cargo.toml                    # Workspace
├── rust-toolchain.toml           # nightly
├── crates/
│   ├── neutron-bot/              # Librería de bots (azalea real)
│   │   └── src/
│   │       ├── client.rs         # Conexión real + batched threads
│   │       ├── scenarios/        # 5 escenarios
│   │       ├── metrics.rs        # Percentiles
│   │       └── output.rs         # Tipos JSON
│   └── neutron-bench/            # CLI harness
│       └── src/
│           ├── main.rs           # CLI (clap)
│           ├── types.rs          # ServerType, Size, Scenario
│           ├── server.rs         # Lifecycle de servidor
│           ├── config.rs         # Generación config (server.properties, pumpkin.toml)
│           ├── harness.rs        # Orquestación principal
│           ├── metrics.rs        # RSS, CPU (sysinfo)
│           ├── tps.rs            # TPS via RCON
│           ├── rcon.rs           # Cliente RCON
│           ├── diskio.rs         # Disk I/O benchmark
│           ├── reporter.rs       # JSON + Markdown
│           └── hardware.rs       # Detección de hardware
├── servers/                      # Binarios
├── results/                      # Output
└── logs/                         # Logs
```

**Uso:**
```bash
cd tests/benchmarks && cargo build --release

# Todos los escenarios para vanilla, 10 bots
./target/release/neutron-bench run --server vanilla --size small

# Solo join-storm, paper, 100 bots
./target/release/neutron-bench run --server paper --size medium --scenario join-storm

# Comparar resultados
./target/release/neutron-bench compare results/*.json
```

## 3. Métricas y definiciones EXACTAS

| Métrica | Definición | Cómo se mide | Status |
|---|---|---|---|
| **Startup** | Tiempo desde spawn hasta "Done (Xs)!" | Regex en log, mediana | ✅ |
| **Join** | Latencia percibida por el cliente | Bot: t(createBot) → spawn. p50/p95/p99 | ✅ |
| **CPS** | Chunks generados por segundo | Bot camina 60s, cuenta chunks recibidos | ✅ |
| **TPS/MSPT** | Ticks por segundo / ms por tick | RCON: `spark tps` (Paper/Folia) | ✅ |
| **RAM (RSS)** | Footprint real del proceso | sysinfo, muestreo 1 Hz | ✅ |
| **CPU** | % de uso de la máquina | sysinfo, normalizado 0-100% | ✅ |
| **Disk I/O** | Velocidad de lectura/escritura | Bench local 64MB sequential + 4K IOPS | ✅ |

## 4. Escenarios de benchmark

| # | Escenario | Qué hace | Métricas clave |
|---|-----------|----------|----------------|
| 1 | **join-storm** | N bots simultáneos (<200ms) | Join p50/p95/p99, startup |
| 2 | **distributed** | 1 bot/segundo | Join por intervalo, TPS estable |
| 3 | **movement** | N bots moviéndose + saltando en radio 50 bloques | TPS, chunks, RAM |
| 4 | **spread** | N bots teletransportados >1000 bloques | Chunk loading spike, RAM peak |
| 5 | **chunk-gen** | N bots caminando en línea recta 60s | CPS total, TPS, RAM |

## 5. Tamaños de servidor

| Tamaño | Bots | Caso de uso |
|--------|------|-------------|
| **small** | 10 | Server personal |
| **medium** | 100 | Server comunitario |
| **large** | 1000 | Server masivo |

## 6. Matriz de benchmarks (4 servers × 3 tamaños × 5 escenarios = 60)

| Server | Small | Medium | Large |
|--------|-------|--------|-------|
| Vanilla 26.2 | ✅ | ✅ | ✅ |
| Paper 26.2 | ✅ TPS | ✅ TPS | ✅ TPS |
| Folia 26.2 | ✅ TPS | ✅ TPS | ✅ TPS |
| Pumpkin | ⚠️ bug protocolo | ⚠️ | ⚠️ |

## 7. Baselines verificados (agosto 2026)

### Join Storm (10 bots, p50 ms)
| Server | 10 bots | 100 bots | 1000 bots | TPS |
|--------|---------|----------|-----------|-----|
| Vanilla | 3,722 | 16,275 | 101,730 | N/A |
| Paper | 2,757 | 16,184 | 101,853 | 20.0 |
| Folia | 2,878 | 16,909 | 103,117 | 20.0 |
| Pumpkin | N/A | N/A | N/A | N/A |

### Recursos (10 bots, idle)
| Server | RAM idle (MB) | RAM peak (MB) | CPU peak (%) | Startup (ms) |
|--------|---------------|---------------|--------------|--------------|
| Vanilla | 2,362 | 2,382 | 1.5% | 8,223 |
| Paper | 2,454 | 2,494 | 50.0% | 8,669 |
| Folia | 2,413 | 2,429 | 6.2% | 8,659 |
| Pumpkin | 20 | 20 | 12.0% | 516 |

### Disk I/O (misma máquina)
| Métrica | Valor |
|---------|-------|
| Sequential Write | 3,600-3,800 MB/s |
| Sequential Read | 3,100-3,700 MB/s |
| Write IOPS (4K) | 120,000-150,000 |
| Read IOPS (4K) | 70,000-82,000 |

## 8. Limitaciones conocidas

1. **Pumpkin**: Bug upstream — dimension types incompatible con azalea (TAG_Long vs TAG_Int). Bots no pueden conectar.
2. **Join latency alto**: Azalea tiene más overhead que mineflayer. Números reales pero no comparables 1:1 con baseline anterior (mineflayer).
3. **1000 bots latencia**: Batched thread pool (50/batch) causa p50 ~100s. Es bottleneck del harness, no del server.
4. **TPS solo Paper/Folia**: Vanilla no tiene spark. Pumpkin no acepta conexiones.

## 9. Cómo agregar un nuevo escenario

1. Crear `tests/benchmarks/crates/neutron-bot/src/scenarios/mi_escenario.rs`
2. Agregar variante a `Scenario` en `types.rs`
3. Agregar función de lanzamiento en `client.rs`
4. Agregar caso en `harness.rs` y `main.rs`
5. Agregar parsing en `reporter.rs`

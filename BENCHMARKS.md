# Neutron — Benchmarks: metodología y harness

> v0.2 · 7 ago 2026 · Regla del proyecto: **todo número publicado tiene metodología reproducible y datos crudos**.

## 1. Filosofía

1. Un benchmark sin metodología es marketing. Publicamos: hardware, software, versiones, comandos, datos crudos (`bench/results/*.json`) y tabla markdown autogenerada.
2. **Misma máquina, misma seed, mismo procedimiento** para vanilla, Paper, Folia y Pumpkin. Sin "condiciones especiales" por servidor.
3. Baselines verificados de la comunidad se citan con fuente y se REPRODUCEN en nuestra máquina antes de usarlos como referencia.
4. El benchmark es un artefacto de CI: si una PR regresiona una métrica clave, la PR no entra.

## 2. Arquitectura del harness

El benchmark harness está escrito completamente en **Rust** (nightly, requerido por azalea).

```
bench/
├── Cargo.toml                    # Workspace
├── crates/
│   ├── neutron-bot/              # Librería de bots
│   │   └── src/scenarios/        # 5 escenarios
│   └── neutron-bench/            # CLI harness
│       └── src/
│           ├── main.rs           # CLI (clap)
│           ├── types.rs          # ServerType, Size, Scenario
│           ├── server.rs         # Lifecycle de servidor
│           ├── harness.rs        # Orquestación
│           ├── metrics.rs        # RSS, CPU (sysinfo)
│           └── reporter.rs       # JSON + Markdown
├── servers/                      # Binarios
├── results/                      # Output
└── logs/                         # Logs
```

**Uso:**
```bash
cd bench && cargo build --release
./target/release/neutron-bench run --server vanilla --size small
```

## 3. Métricas y definiciones EXACTAS

| Métrica | Definición | Cómo se mide |
|---|---|---|
| **Startup** | Tiempo desde spawn del proceso hasta "Done (Xs)!" | Regex en log, 5 runs, mediana |
| **Join** | Latencia percibida por el cliente | Bot: t(createBot) → spawn. p50/p95/p99 |
| **CPS** | Chunks generados por segundo, sostenido | Bot camina 60s, cuenta chunks recibidos |
| **TPS/MSPT** | Ticks por segundo / ms por tick | Paper/Folia: spark HTTP. Otros: estimado |
| **RAM (RSS)** | Footprint real del proceso | sysinfo, muestreo 1 Hz, 60s idle + carga |
| **CPU** | % de uso de la máquina | sysinfo, muestreo 1 Hz |

## 4. Tamaños de servidor

| Tamaño | Bots | Caso de uso |
|--------|------|-------------|
| **small** | 10 | Server personal |
| **medium** | 100 | Server comunitario |
| **large** | 1000 | Server masivo (F4+) |

## 5. Escenarios de benchmark

### Escenario 1: Join Storm
- **Descripción:** N bots se conectan simultáneamente (<200ms total)
- **Mide:** Join latency (t0 → spawn), p50/p95/p99
- **Config:** max_players = N, view-distance = 10

### Escenario 2: Distributed Join
- **Descripción:** 1 bot por segundo durante N segundos
- **Mide:** Join latency por intervalo, comportamiento bajo carga sostenida
- **Config:** max_players = N

### Escenario 3: Movement
- **Descripción:** N bots spawned, se mueven y saltan en radio 50 bloques
- **Acción:** Walk 2s → jump 1s → turn → repeat (60s)
- **Mide:** TPS, chunks recibidos, RAM

### Escenario 4: Spread
- **Descripción:** N bots spawned, cada uno teletransportado >1000 bloques
- **Mide:** Chunk loading spike, RAM peak, TPS drop

### Escenario 5: Chunk Generation
- **Descripción:** N bots caminan en línea recta 60s
- **Acción:** Walking speed (4.3 blocks/s) sin parar
- **Mide:** CPS total, CPS per-bot, TPS p99, RAM peak

## 6. Matriz de benchmarks

| Server | Small (10) | Medium (100) | Large (1000) |
|--------|------------|--------------|--------------|
| Vanilla 26.2 | 5 escenarios | 5 escenarios | 5 escenarios |
| Paper | 5 escenarios | 5 escenarios | 5 escenarios |
| Folia | 5 escenarios | 5 escenarios | 5 escenarios |
| Pumpkin | 5 escenarios | 5 escenarios | 5 escenarios |

**Total: 60 configuraciones × 5 runs = 300 runs por tanda completa**

## 7. Cómo levantar cada servidor

| Servidor | Comando | Notas |
|---|---|---|
| Vanilla 26.2 | `java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui` | Java 25 obligatorio |
| Paper | `java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui` | spark incluido, rate limit ~15/s |
| Folia | `java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui` | Threaded regions |
| Pumpkin | `./pumpkin` (nativo) | Sin JVM, config.toml |

## 8. Metodología estándar (cada run)

1. **Hardware fijo**: registrar CPU/RAM/SSD/SO
2. **Seed fija**: `1234567890123456789`, `online-mode=false`, `view-distance=10`
3. **Mundo limpio**: carpeta vacía en cada run
4. **Warmup**: 60s idle antes de medir
5. **N=5 runs** por escenario; reportar **mediana**
6. **Output**: `bench/results/<ID>.json` + `<ID>.md`

## 9. Baselines verificados (agosto 2026)

| Métrica | Vanilla | Paper | Pumpkin (self-reported) | Neutron (target) |
|---|---|---|---|---|
| Startup | 7-15 s | 7-10 s | ~5-8 ms | < 2 s |
| RAM idle | 0.9-1.8 GB | 1.1-2.2 GB | ~100 MB | < 150 MB |
| CPU idle | ~24% | ~20% | ~1.5% | TBD |
| CPS | 10.6-14.2 | 17.4-84.8 | no publicado | > 250 |
| TPS | 20 | 20 | 20 | 20.0 |
| Join p95 @100 | TBD | TBD | TBD | < 2 s |

## 10. Targets de Neutron

| Métrica | Target |
|---|---|
| Startup (mundo vacío → Done) | < 2 s |
| CPS overworld @16 hilos | > 250 sostenidos |
| RAM base idle | < 150 MB |
| RAM/jugador idle | < 1 MB |
| TPS @500 jugadores | 20.0, p99 tick < 25 ms |
| Join p95 @100 bots | < 2 s |
| Actualización de versión | main ≤ 7 días |

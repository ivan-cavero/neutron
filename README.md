# Neutron

Servidor de Minecraft Java Edition reimplementado desde cero en Rust. Multiplataforma (Windows/Linux/macOS x86-64/ARM64), paridad 1:1 con vanilla, plugins WASM/Lua seguros por construcción, y `main` siempre en la última versión de Minecraft.

**Estado**: PRE-ALPHA · F2d worldgen 97.84 % + servidor jugable · 14 ago 2026

## Cómo orientarte (qué leer cuándo)

| Necesitas | Documento |
|---|---|
| Saber qué es esto y sus objetivos | este README |
| Saber en qué punto estamos y qué sigue | STATE.md + runs/ (historial) |
| El plan completo (fases, bars, pipeline de versiones) | ROADMAP.md |
| Cómo está diseñado el servidor + evidencia verificada | ARCHITECTURE.md (Anexo A) |
| Cómo se miden los benchmarks | BENCHMARKS.md |
| Cómo trabajamos / lanzar el siguiente run | AGENTS.md — pi lo lee solo |

## Objetivos

1. **Extreme performance** — rendimiento medido y publicado con metodología reproducible (BENCHMARKS.md), no marketing.
2. **Security by construction** — plugins en sandbox WASM: un plugin nunca tumba el servidor.
3. **1:1 vanilla parity** — misma seed → mismo mundo; redstone, iluminación y spawns idénticos; verificado por checksums en CI.
4. **Version cadence** — `main` = última versión de Mojang en ≤ 7 días (pipeline D0-D4).

## Targets (a validar con BENCHMARKS.md)

| Métrica | Target |
|---|---|
| Startup (mundo vacío → `Done`) | < 2 s |
| Chunks/s @16 hilos | > 250 |
| RAM base | < 150 MB |
| RAM por jugador | < 1 MB |
| TPS @500 jugadores | 20.0, p99 < 25 ms |
| Join p95 @100 bots | < 2 s |
| Nueva versión de Mojang | main ≤ 7 días |

## Estructura del repo (hoy)

El diagrama de `ARCHITECTURE.md` describe el **objetivo** (cli, plugins WASM, Folia). El grafo real es más pequeño:

```
neutron/
├─ crates/
│  ├─ neutron-protocol/     # paquetes 26.2 (a mano)
│  ├─ neutron-world/        # Anvil / level.dat (aún no lo usa el server)
│  ├─ neutron-worldgen/     # overworld 26.2 (~98 % parity)
│  ├─ neutron-server/       # binario jugable: login + chunks
│  ├─ neutron-sim/          # luz / redstone / fluidos / spawn (tests, no cableado)
│  └─ neutron-bench-server/ # criterion
├─ tools/                   # golden-data · parity-check · vanilla-extract · java-probe
├─ bench/                   # workspace aparte: bots + jars de referencia
├─ runs/                    # historial de runs
└─ docs/                    # ADRs y notas (WHAT-NOT-TO-COMMIT)
```

## Quick start (dev)

```bash
# Servidor jugable (worldgen real, seed 12345, no 1:1 todavía)
cargo run --release -p neutron-server -- --seed 12345 --view-distance 8
# Cliente vanilla 26.2 → localhost:25565  (online-mode=false, Creative + vuelo)
# Estado de worldgen y gaps: crates/neutron-worldgen/WORLDGEN.md
```
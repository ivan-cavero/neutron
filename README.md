# Neutron

Servidor de Minecraft Java Edition reimplementado desde cero en Rust. Multiplataforma (Windows/Linux/macOS x86-64/ARM64), paridad 1:1 con vanilla, plugins WASM/Lua seguros por construcción, y `main` siempre en la última versión de Minecraft.

**Estado**: PRE-ALPHA · Documentación de diseño v0.2 · 5 ago 2026

## Objetivos

1. **Extreme performance** — rendimiento medido y publicado con metodología reproducible (BENCHMARKS.md), no marketing.
2. **Security by construction** — plugins en sandbox WASM: un plugin nunca tumba el servidor.
3. **1:1 vanilla parity** — misma seed → mismo mundo; redstone, iluminación y spawns idénticos; verificado por checksums en CI.
4. **Version cadence** — `main` = última versión de Mojang en ≤ 7 días (pipeline de extracción + codegen).

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

## Estructura del repo

```
neutron/
├─ crates/          # neutron-core · data · protocol · worldgen · world · sim · server · plugin · scripting · cli
├─ tools/           # mc-extract (jar → JSON) · codegen (JSON → Rust) · patch-bukkit (fase tardía)
├─ bench/           # harness de benchmarks + bots + results/
├─ docs/            # ADRs y documentación técnica
└─ *.md             # documentación del proyecto (abajo)
```

## Documentos

| Documento | Propósito |
|---|---|
| [RESEARCH.md](RESEARCH.md) | Evidencia verificada con fuentes (base de hechos del proyecto) |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Diseño técnico: capas, paridad, pipeline de versiones, plugins |
| [ROADMAP.md](ROADMAP.md) | Fases F0-F8 con bars y rondas (Gauntlet Loop) |
| [BENCHMARKS.md](BENCHMARKS.md) | Metodología de medición: qué medimos y cómo |
| [OPERATIONS.md](OPERATIONS.md) | Cómo trabajamos: Orca ADE + Gauntlet Loop + prompts por fase |

## Quick start (dev)

```bash
cargo run --release -p neutron-cli
# Conecta con un cliente vanilla 26.2 a localhost:25565 (online-mode=false en dev)
```
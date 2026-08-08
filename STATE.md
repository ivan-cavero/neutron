# STATE — Neutron

> Estado actual del proyecto. Se lee al empezar cada run y se actualiza al terminar.

## Fase actual
**F0 — Fundamentos y harness** (COMPLETADO ✅)

### Benchmark Harness v2 — Sistema completo en Rust

**Arquitectura**: 2 crates Rust (`neutron-bot` + `neutron-bench`) que reemplazan el harness PowerShell/Bash y los bots mineflayer/azalea sueltos.

**Commits recientes**:
```
7cd39e9  Fix TPS: RCON-based measurement works for Paper and Folia
86a21f5  Fix Pumpkin: encryption=false, compression=false
55fdf30  Full benchmark report: Vanilla/Paper/Folia/Pumpkin × 10/100/1000 bots
bc9f464  Fix 1000 bots: batched thread pool approach
7da2e76  Benchmark harness: Rust rewrite with real azalea bots
```

### Resultados de benchmark (join-storm, 2026-08-08)

| Server | 10 bots | 100 bots | 1000 bots | TPS |
|--------|---------|----------|-----------|-----|
| **Vanilla** | ✅ 10/10, p50=3722ms | ✅ 100/100, p50=16275ms | ✅ 1000/1000, p50=101730ms | N/A |
| **Paper** | ✅ 10/10, p50=2757ms | ✅ 100/100, p50=16184ms | ✅ 1000/1000, p50=101853ms | ✅ 20.0 |
| **Folia** | ✅ 9/10, p50=2878ms | ✅ 100/100, p50=16909ms | ✅ 1000/1000, p50=103117ms | ✅ 20.0 |
| **Pumpkin** | ⚠️ 0/10 | ⚠️ 0/10 | ⚠️ 0/10 | N/A |

### Métricas disponibles
- ✅ Startup time (regex "Done (Xs)!")
- ✅ Join latency p50/p95/p99 (azalea bots reales)
- ✅ RAM/RSS (sysinfo, muestreo 1Hz)
- ✅ CPU % (normalizado a 0-100%)
- ✅ CPS (chunks recibidos / duración)
- ✅ Disk I/O (read/write MB/s, IOPS)
- ✅ TPS via RCON (Paper/Folia: `spark tps`)
- ⚠️ Pumpkin: bug de protocolo (dimension types incompatible con azalea)

### Limitaciones conocidas
1. **Pumpkin**: Bug upstream — envía `height` como TAG_Long (8 bytes) pero azalea espera TAG_Int (4 bytes). Necesita fix en pumpkin-codegen.
2. **Join latency alto**: Azalea tiene más overhead que mineflayer. Números son reales pero no comparables 1:1 con baseline anterior.
3. **1000 bots**: Batched thread pool (50 bots/batch) — latencia alta pero funcional.

## Siguiente fase
F0 completada. Listo para F1 — Nucleo jugable (protocolo 26.2, mundo Anvil, fuzz, E2E).

## Ver
- `bench/results/FULL-BENCHMARK-REPORT.md` — reporte comparativo completo
- `bench/results/*.json` — resultados crudos
- `BENCHMARKS.md` — metodología actualizada
- `bench/README.md` — documentación del harness

# Neutron Benchmark Suite - Resultados

## Fecha: 2026-08-07
## Hardware: Windows 11, Java 25.0.3, 31GB RAM

---

## 1. JOIN STORM (bots entrando a la vez)

### Paper 26.2
| Bots | p50 (ms) | p95 (ms) | p99 (ms) |
|------|----------|----------|----------|
| 10   | 1,614    | 1,621    | 1,622    |
| 50   | 884      | 1,321    | 1,331    |
| 100  | 1,350    | 1,923    | 1,931    |

### Folia 26.2
| Bots | p50 (ms) | p95 (ms) | p99 (ms) |
|------|----------|----------|----------|
| 10   | 1,695    | 1,961    | 1,980    |
| 50   | 2,158    | 3,721    | 3,779    |
| 100  | 2,724    | 4,720    | 4,798    |

**Análisis**: Paper es más rápido que Folia para join storm. Folia tiene overhead por el scheduler multi-threaded.

---

## 2. IDLE (10 bots online, quietos)

| Servidor | RAM | CPU (s) | Threads | CPU System |
|----------|-----|---------|---------|------------|
| Paper    | 2,187 MB | 76.6s | 85 | 15.8% |
| Folia    | 2,190 MB | 62.0s | 68 | - |

---

## 3. STARTUP TIME

| Servidor | Startup (s) |
|----------|-------------|
| Vanilla 26.2 | 1.88 |
| Paper 26.2 | 6.56 |
| Folia 26.2 | 6.76 |

---

## 4. BASELINE ANTERIOR (10 bots)

| Servidor | Join p50 | Join p95 |
|----------|----------|----------|
| Vanilla 26.2 | 373ms | 406ms |
| Paper 26.2 | 3,332ms | 3,383ms |
| Folia 26.2 | 3,277ms | 3,678ms |

---

## 5. MÉTRICAS DE SISTEMA

| Métrica | Paper (idle) | Folia (idle) |
|---------|--------------|--------------|
| RAM | 2,187 MB | 2,190 MB |
| CPU (acumulado) | 76.6s | 62.0s |
| Threads | 85 | 68 |
| CPU System | 15.8% | - |
| Disk I/O | 0.1 MB/s | - |

---

## NOTAS

1. **CPS**: No medido localmente (requiere Chunky o método equivalente)
2. **TPS**: Spark no disponible en HTTP (Paper/Folia lo desactivan por defecto)
3. **Folia multithread**: Tiene 68 threads vs 85 de Paper, pero join latency es más alto
4. **Baseline C2ME** (1.21.10): Vanilla 10.6-14.2 cps, Paper 17.4-84.8 cps

---

## PRÓXIMOS PASOS

1. Habilitar spark HTTP para medir TPS
2. Medir CPS con Chunky o método equivalente
3. Ejecutar escenario "world generation" (bots volando)
4. Ejecutar escenario "spread" (bots en diferentes ubicaciones)
5. Comparar con Neutron cuando esté listo

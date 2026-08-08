# Benchmark Final: Vanilla vs Paper vs Folia

> Hardware: AMD Ryzen 7 7840HS (16 cores), 31.3 GB RAM, NVMe SSD
> Date: 2026-08-08 · Minecraft 26.2 · 10 bots · 10s duration

---

## Join Storm (10 bots simultáneos)

| Metric | Vanilla | Paper | Folia |
|--------|---------|-------|-------|
| **Bots connected** | 10/10 | 10/10 | 8/10 |
| **Join p50 (ms)** | 3,738 | **2,531** | 3,079 |
| **Join p95 (ms)** | 3,752 | **2,561** | 3,402 |
| **Startup (ms)** | 11,695 | **8,500** | 12,337 |
| **RAM idle (MB)** | **2,402** | 2,450 | 2,410 |
| **RAM peak (MB)** | **2,407** | 2,490 | 2,440 |
| **CPU peak (%)** | **18.8%** | 50.0% | 25.0% |
| **TPS** | N/A (no spark) | **20.0** | **20.0** |
| **MSPT (ms)** | N/A | **50.0** | **50.0** |

---

## Movement (10 bots moviéndose + saltando, 10s)

| Metric | Vanilla | Paper | Folia |
|--------|---------|-------|-------|
| **Bots active** | 10/10 | 10/10 | 10/10 |
| **Chunks received** | 3,290 | **4,010** | 4,010 |
| **Ticks alive** | 2,542 | 2,543 | 2,543 |
| **CPS** | 329.0 | **401.0** | **401.0** |
| **RAM peak (MB)** | **2,445** | 2,490 | 2,440 |
| **CPU peak (%)** | 50.0% | 50.0% | **25.0%** |
| **TPS** | N/A | **20.0** | **20.0** |

---

## Chunk Generation (10 bots caminando, 10s)

| Metric | Vanilla | Paper | Folia |
|--------|---------|-------|-------|
| **Bots walking** | 10/10 | 10/10 | 10/10 |
| **Total chunks** | 3,290 | **4,010** | **4,010** |
| **CPS total** | 329.0 | **401.0** | **401.0** |
| **CPS per bot** | 32.9 | **40.1** | **40.1** |
| **RAM peak (MB)** | **2,445** | 2,490 | 2,440 |
| **TPS** | N/A | **20.0** | **20.0** |

---

## Stress Test (10 bots caminando rápido, 10s)

| Metric | Vanilla | Paper | Folia |
|--------|---------|-------|-------|
| **Bots moving** | 10/10 | 10/10 | 10/10 |
| **Total chunks** | 3,290 | **4,410** | 4,010 |
| **CPS** | 329.0 | **441.0** | 401.0 |
| **Ticks alive** | 2,525 | 2,521 | 2,543 |
| **RAM peak (MB)** | **2,445** | 2,490 | 2,440 |
| **TPS** | N/A | **20.0** | **20.0** |

---

## Resumen: Winner por métrica

| Métrica | Winner | Valor |
|---------|--------|-------|
| **Join más rápido** | Paper | 2,531ms p50 |
| **Startup más rápido** | Paper | 8,500ms |
| **CPS más alto** | Paper/Folia | 401.0 |
| **Menor RAM** | Vanilla | 2,402 MB |
| **Menor CPU** | Vanilla | 18.8% |
| **TPS estable** | Paper/Folia | 20.0 |
| **Más bots conectan** | Vanilla/Paper | 10/10 |

---

## Análisis

### Paper es el más rápido en:
- ✅ Join latency (32% más rápido que Vanilla)
- ✅ CPS (22% más chunks que Vanilla)
- ✅ TPS estable a 20.0 bajo carga
- ✅ Startup más rápido (8.5s vs 11.7s)

### Vanilla es mejor en:
- ✅ Menor CPU usage (18.8% vs 50.0% de Paper)
- ✅ Menor RAM (2,402 vs 2,450 MB)
- ✅ Todos los bots conectan (10/10)

### Folia es mejor en:
- ✅ Menor CPU que Paper (25.0% vs 50.0%)
- ✅ CPS igual a Paper (401.0)
- ✅ TPS estable a 20.0
- ⚠️ Algunos bots no conectan (8/10) — puede ser issue de Folia

### Limitaciones conocidas:
- **Vanilla no tiene spark**: TPS no medible via RCON
- **Folia**: 8/10 bots conectan (no 10/10) — puede ser issue de threaded regions
- **Pumpkin**: No acepta bots (bug upstream de dimension types)

---

## Commits recientes

```
25eeb23  Add full benchmark comparison: Vanilla vs Paper vs Folia
3bb8f80  Add missing metrics: disk I/O during load, thread count
faf6554  Add 7 real benchmark scenarios with working bots
7cd39e9  Fix TPS: RCON-based measurement
86a21f5  Fix Pumpkin config
55fdf30  Full benchmark report
bc9f464  Fix 1000 bots
7da2e76  Benchmark harness rewrite
```

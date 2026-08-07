# Neutron Benchmark Suite - Reporte Completo

## Fecha: 2026-08-07
## Hardware: Windows 11, Java 25.0.3, 31GB RAM
## Servidores: Paper 26.2-102, Folia 26.2-1

---

## RESUMEN EJECUTIVO

| Métrica | Paper 26.2 | Folia 26.2 |
|---------|------------|------------|
| **Startup** | 7.1s | 6.9s |
| **Join p50 (10 bots)** | 560ms | 1,857ms |
| **Join p95 (100 bots)** | 1,416ms | 4,774ms |
| **RAM idle** | 2,130 MB | 2,140 MB |
| **CPU idle** | 77.4s | 79.2s |
| **Threads** | 81 | 76 |

---

## 1. ESCENARIO: JOIN STORM

### Paper 26.2
| Bots | p50 (ms) | p95 (ms) | p99 (ms) |
|------|----------|----------|----------|
| 10   | 560      | 582      | 583      |
| 100  | 930      | 1,416    | 1,524    |

### Folia 26.2
| Bots | p50 (ms) | p95 (ms) | p99 (ms) |
|------|----------|----------|----------|
| 10   | 1,857    | 2,082    | 2,084    |
| 100  | 2,729    | 4,774    | 4,912    |

**Análisis**: Paper es significativamente más rápido que Folia para join storm (3x más rápido con 10 bots).

---

## 2. ESCENARIO: IDLE (10 bots online, quietos)

### Paper 26.2
- Join p50: 449ms
- RAM: 2,130 MB
- CPU: 77.4s (acumulado)
- Threads: 81

### Folia 26.2
- Join p50: 713ms
- RAM: 2,140 MB
- CPU: 79.2s (acumulado)
- Threads: 76

---

## 3. ESCENARIO: WORLD GENERATION

**Estado**: Pendiente - CPS meter necesita fix (bots se desconectan al volar)

---

## 4. ESCENARIO: JUMPING

**Estado**: Pendiente - necesita bot que salte repetidamente

---

## 5. TAMAÑOS DE SERVIDOR

| Tamaño | Jugadores | Paper p50 | Folia p50 |
|--------|-----------|-----------|-----------|
| Pequeño | 10 | 560ms | 1,857ms |
| Mediano | 100 | 930ms | 2,729ms |
| Grande | 1,000 | N/A | N/A |

**Nota**: 1,000 bots no se ejecutó aún (requiere más tiempo y recursos)

---

## 6. MÉTRICAS DE SISTEMA

| Métrica | Paper | Folia |
|---------|-------|-------|
| RAM idle | 2,130 MB | 2,140 MB |
| CPU (acumulado) | 77.4s | 79.2s |
| Threads | 81 | 76 |
| CPU System | 15.8% | N/A |
| Disk I/O | 0.1 MB/s | N/A |

---

## 7. COMPARACIÓN CON BASELINES

| Métrica | Paper (26.2) | Folia (26.2) | C2ME (1.21.10) |
|---------|--------------|--------------|----------------|
| CPS | N/A | N/A | 17.4-84.8 |
| Join p50 | 560ms | 1,857ms | N/A |

**Nota**: Los baselines C2ME son de 1.21.10, no 26.2

---

## 8. CONCLUSIONES

1. **Paper es más rápido que Folia** para join storm (3x con 10 bots)
2. **Folia tiene más overhead** por el scheduler multi-threaded
3. **RAM similar** entre Paper y Folia (~2.1 GB)
4. **CPS no medido** - necesita Chunky o método equivalente
5. **TPS no medido** - spark HTTP no disponible en esta versión

---

## 9. PRÓXIMOS PASOS

1. Fix CPS meter para medir chunks/second
2. Habilitar spark HTTP para medir TPS
3. Ejecutar escenario "world generation" con bots volando
4. Ejecutar escenario "jumping" con bots saltando
5. Ejecutar test de 1,000 bots (requiere más tiempo)
6. Comparar con Neutron cuando esté listo

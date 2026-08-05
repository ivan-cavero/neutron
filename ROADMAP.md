# Neutron — Roadmap

> El progreso se mide en **BARS y RONDAS**, no en calendario. Un bar es una referencia real e innegociable (checksum, benchmark, server real) que un critic ciego inspecciona (Gauntlet Loop, ver OPERATIONS.md §2). El calendario con agentes es orientativo y se recalibra tras cada fase.

## 0. Cómo leer este roadmap

- **Bar**: lo que el critic compara contra nuestro artefacto (el "Call of Duty" de cada fase). No se discute: se cumple o no.
- **Rondas**: ciclos build → critic → fix. Sin cap arbitrario: se itera hasta que el bar gana, 2 rondas sin mejora, o presupuesto agotado.
- **Calendario**: estimación asumiendo agentes dedicados (opencode). Es secundario — el bar manda.

## 1. Cadencia de Mojang (verificada — RESEARCH.md §1)

| Versión | Tipo | Fecha |
|---|---|---|
| 1.21.11 "Mounts of Mayhem" | última 1.x (jar ofuscado, Java 21) | 9 dic 2025 |
| 26.1 "Tiny Takeover" | jar sin ofuscar, Java 25 | 24 mar 2026 |
| 26.2 "Chaos Cubed" | **versión objetivo de `main` hoy** | 16 jun 2026 |
| 26.3 | en snapshots | Q3 2026 |
| ~26.x | ~3 drops/año + hotfixes | continuo |

## 2. Fases

### F0 — Fundamentos y harness · 3-5 rondas · ~1-2 semanas
**Objetivo**: infraestructura del repo + primer baseline público.
**Bar**: un agente distinto al builder ejecuta `bench/run.ps1` desde cero en Windows y Linux y reproduce el baseline B0 (vanilla 26.2 / Paper / Pumpkin); 10 bots de join simultáneo sin kicks (p95 < 5 s); cps ±30% consistente con baselines publicados.
**Piezas**: harness + bots · CI/workspace · baseline B0 publicado. **Riesgo**: bajo.

### F1 — Núcleo jugable · 5-8 rondas · ~2-4 semanas
**Objetivo**: un jugador real entra, juega y el mundo persiste en Anvil vanilla.
**Bar**: bot vanilla 26.2 juega 10 min sin kick (E2E en CI); mundo guardado abre en vanilla y viceversa; fuzz del decode 1M inputs sin panic; startup < 2 s; RAM < 200 MB.
**Piezas**: protocolo 26.2 (login/play) · world v1 (Anvil, level.dat, carpetas vanilla) · pipeline de versiones v1 · E2E diario. **Riesgo**: medio.

### F2 — Worldgen paridad 1:1 · 8-12 rondas · ~4-8 semanas
**Objetivo**: misma seed → mismo mundo, verificado por checksum.
**Bar**: checksum xxHash64 idéntico a vanilla en 50 seeds golden (0 mismatches); cps > 250 @16 hilos reproducido; un mundo generado por Neutron abre en vanilla con el mismo terreno.
**Piezas**: golden data pipeline · density functions + noise + surface + carvers + features · estructuras fase 1 · bench de cps. **Riesgo**: medio-alto.

### F3 — Simulación vanilla · 10-16 rondas · ~6-12 semanas
**Objetivo**: bloques, fluidos, iluminación, redstone, spawns, survival.
**Bar**: suite dorada posicional 100% contra server vanilla real (bots); light arrays idénticos en 50 seeds; survival básica jugable por bot.
**Piezas**: iluminación (engine propio estilo Starlight) · redstone A (wire/torches/levers/doors) · B (comparators/repeaters/observers/hoppers/TNT) · C (pistons + QC + block swapping) · D (suite completa) · fluidos · spawns · survival. **Riesgo**: **ALTO** (redstone posicional — el mayor reto técnico).

### F4 — Escala 500-1000+ · 6-10 rondas · ~4-8 semanas
**Objetivo**: 500 jugadores estables; camino a 1000+.
**Bar**: 500 bots 60 min → TPS 20.0, p99 tick < 25 ms; RAM/jugador < 1 MB sobre base < 150 MB; sin regresión de cps/startup.
**Piezas**: diseño del scheduler por regiones (fan-out de 3 agentes con A/B) · optimizaciones hot path (arenas, buffers, sin locks) · stress 500 bots · memory profiling en CI. **Riesgo**: medio-alto.

### F5 — Mobs y AI · 10-16 rondas · ~6-12 semanas
**Objetivo**: comportamiento vanilla de mobs y combate completo.
**Bar**: E2E 20 min de survival; spot-checks automatizados (creeper explota, zombie quema al amanecer, enderman se teletransporta, dragon en loop de jefe); 50 mobs/chunk sin regresión de TPS.
**Piezas**: pasivos + trading · hostiles · jefes (Ender Dragon, luego wither) · combate (melee/arco/escudo/tridente/encantamientos) · pathfinding A*. **Riesgo**: **ALTO**.

### F6 — Plugins WASM + Lua · 8-12 rondas · ~4-8 semanas
**Objetivo**: ecosistema seguro por construcción; convertir lo convertible.
**Bar**: plugin WASM con panic no tumba el servidor; fuel 10M opcodes mata el plugin sin daño; hot reload sin reiniciar; 3 conversiones reales de plugins Bukkit simples; coste en hot path < 5 µs/tick.
**Piezas**: runtime wasmtime + WIT · API completa · Lua (mlua) · convertidor v1 · capa PatchBukkit-style v0 · docs. **Riesgo**: alto (expectativas de compat — comunicación honesta desde el inicio).

### F7 — Bedrock · 6-10 rondas · ~4-8 semanas
**Objetivo**: clientes Bedrock 26.x en el mismo mundo.
**Bar**: cliente Bedrock real juega 10 min; coexistencia Java+Bedrock verificada; TPS Java sin impacto.
**Piezas**: RakNet + login/play · play básico · mapeo de registries Java↔Bedrock · coexistencia. **Riesgo**: medio.

### F8 — 1.0 · 6-10 rondas · ~4-8 semanas
**Objetivo**: release estable, verificable y defendible.
**Bar**: parity suite completa verde en `main`; benchmarks públicos reproducibles en 2 máquinas; 72 h de uptime con 100 jugadores sin leak; fuzz 24 h limpio; binarios x86-64/ARM64 (Windows/Linux/macOS).
**Piezas**: fuzz + audits · benchmarks finales · docs + guía de migración · proceso de release. **Riesgo**: medio.

## 3. Timeline consolidado (estimación con agentes)

| Fase | Rondas est. | Calendario est. | Paralelo con |
|---|---|---|---|
| F0 | 3-5 | 1-2 semanas | — |
| F1 | 5-8 | 2-4 semanas | — |
| F2 | 8-12 | 4-8 semanas | — |
| F3 | 10-16 | 6-12 semanas | F4 |
| F4 | 6-10 | 4-8 semanas | F3 |
| F5 | 10-16 | 6-12 semanas | F6, F7 |
| F6 | 8-12 | 4-8 semanas | F5 |
| F7 | 6-10 | 4-8 semanas | F5 |
| F8 | 6-10 | 4-8 semanas | — |

**1.0 ≈ 6-10 meses** con agentes dedicados en paralelo (vs ~24 meses en modo clásico). Recalibrar tras F0 con datos reales.

*Hitos: mes 1 = baseline público · mes 2-3 = jugador real · mes 4-6 = paridad worldgen · mes 6-9 = redstone dorada + 500 jugadores · mes 6-12 = plugins WASM · mes 8-12 = 1.0.*

## 4. Pipeline de versiones D0-D4 (SLA: `main` ≤ 7 días tras release de Mojang)

| Día | Paso | Herramienta | Verificación |
|---|---|---|---|
| D0 | Detectar release de Mojang | webhook/CI | — |
| D1 | Extraer jar (sin ofuscar desde 26.1): registries, protocolo, worldgen, assets | `tools/mc-extract` | diff vs anterior; validación contra minecraft-data |
| D2 | Codegen → Rust tipado | `tools/codegen` | `cargo check` limpio, sin diffs manuales |
| D3 | Regenerar golden data (chunks por seed, contraptions) | harness | checksums xxHash64 |
| D4 | Parity suite + benchmarks + release `main` | CI + gate humano | parity 100%, benchmarks publicados |

## 5. Riesgos y mitigaciones

| Riesgo | Severidad | Mitigación |
|---|---|---|
| Paridad de redstone (posicional, QC, 1.21.2+) | CRÍTICO | Suite dorada posicional desde F3-A; comparación contra server real en cada PR |
| Paridad de mob AI | ALTO | Port desde jar sin ofuscar; oleadas; spot-checks automatizados |
| Expectativas de compat Bukkit | ALTO | Estrategia por capas honesta (F6); comunicación temprana |
| Escala 1000+ | MEDIO-ALTO | Scheduler regional con fan-out A/B (F4); stress continuo |
| Cadencia de Mojang | MEDIO | Pipeline D0-D4 desde F1; tests de regresión por versión |
| Scope creep | ALTO | Cada fase termina con su bar cumplido; backlog separado |
| Coste de agentes (tokens) | MEDIO | Presupuestos como guardrail; kill-switch; STATE.md |

## 6. Fuera de alcance

Combat 1.8 · mods Forge/Fabric · plugins Bukkit al 100% (solo por capas) · minigames custom · FPS de cliente (métrica de cliente, no de servidor — ver BENCHMARKS.md).
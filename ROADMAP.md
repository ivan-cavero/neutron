# Neutron — Roadmap completo

> v0.1 · 5 ago 2026 · Planificación F0-F8 hacia la 1.0. Cada fase termina con **evidencia publicada** (benchmark, parity test o release), nunca con "parece que funciona".

## 0. Reglas de planificación

1. **Cada fase tiene criterios de salida medibles** (Definition of Done) — no se avanza sin cumplirlos.
2. **Benchmarks y parity tests corren en CI** desde F0 — la regresión es un bug.
3. **Maker/checker**: el agente que implementa nunca se auto-verifica (ver ORCHESTRATION.md).
4. **Presupuesto**: cada fase tiene presupuesto de rounds/tokens; kill-switch al 100% (loop-engineering).
5. **Nada de "mil veces mejor" sin medirlo**: las claims se publican en BENCHMARKS.md con metodología reproducible.

## 1. Cadencia de Mojang (verificada) — el reloj del proyecto

| Versión | Tipo | Fecha | Notas |
|---|---|---|---|
| 1.21.11 "Mounts of Mayhem" | última 1.x | 9 dic 2025 | Último jar ofuscado, Java 21 |
| 26.1 "Tiny Takeover" | drop 2026 #1 | 24 mar 2026 | Primer jar sin ofuscar, Java 25 |
| 26.2 "Chaos Cubed" | drop 2026 #2 | 16 jun 2026 | **Versión objetivo de `main` hoy** |
| 26.3 | drop 2026 #3 | Q3 2026 (snapshots activos) | Preparar pipeline D0-D4 para el día 1 |
| ~26.x | drops 2027 | ~3/año | El pipeline corre 4-6 veces/año + hotfixes |

Fuentes: minecraft.net (numeración por año, 2 dic 2025), minecraft.wiki (version history), GamingOnLinux.

## 2. Fases

### F0 — Fundamentos y harness de benchmarks (semanas 1-4)

**Objetivo**: infraestructura y verdad de referencia. Nada de código de servidor sin saber medir.

**Entregables**:
- Repo cargo workspace (estructura del README §5), CI (fmt, clippy -D warnings, test, deny), `STATE.md`.
- `bench/` completo (BENCHMARKS.md implementado): levanta **vanilla 26.2**, **Paper última**, **Pumpkin nightly**; bots (mineflayer ≤1.21.11 / azalea para 26.x); mide startup (`Done (Xs)!`), join (login/spawn del bot), TPS (spark en Paper, logs en Pumpkin), RAM (RSS 1 Hz), CPU.
- **Publicar baseline verificado** en `bench/results/B0-*.md`: vanilla vs Paper vs Pumpkin en la misma máquina (nuestro hardware de referencia).
- Plantilla de worldgen: `neutron-core`, `neutron-data` esqueleto, `neutron-protocol` con status + login + keepalive (solo para que un bot haga ping).

**Criterios de salida**:
- AC0.1: `bench/run.ps1 --servers vanilla,paper,pumpkin --metrics all` corre de punta a punta en Windows y Linux.
- AC0.2: startup medido por regex `Done (Xs)!` con 5 runs → mediana reportada.
- AC0.3: 10 bots hacen join simultáneo en vanilla y Paper; p95 join < 5 s; sin kicks (online-mode=false, throttling de comandos).
- AC0.4: cps medido con Chunky (vanilla/Paper) y método propio (Pumpkin); consistente con baselines publicados ±30%.
- AC0.5: baseline B0 publicado con tabla markdown + JSON crudo.

**Riesgo**: bajo. **Presupuesto**: 4 semanas.

---

### F1 — Núcleo jugable (semanas 5-14)

**Objetivo**: un jugador real entra, se mueve, chatea, rompe/coloca bloques y el mundo persiste en Anvil.

**Entregables**:
- `mc-extract` v1 + codegen: registries + protocolo de 26.2 (y 26.3 si ya salió).
- Protocol: handshake → status → login (offline+online) → play: join, keepalive, chat, movement, chunk data (flat), block place/break, inventory básico, comandos (`/seed`, `/tp`, `/gamemode`, `/save`).
- `neutron-world` v1: chunk en memoria (palette), **Anvil .mca read/write**, `level.dat`, estructura de carpetas vanilla, `session.lock`.
- E2E diario con bot (Tarea S1): join → mover 100 bloques → colocar/romper → `/seed` → disconnect; TPS ≥ 19.9.
- Fuzzing del decode de paquetes (1M inputs sin panic).

**Criterios de salida**:
- AC1.1: bot vanilla 26.2 real entra y juega 10 min sin kick (harness E2E verde en CI).
- AC1.2: mundo guardado en Anvil → **abre en vanilla sin errores** (test de interoperabilidad bidireccional).
- AC1.3: startup < 2 s; RAM < 200 MB con mundo flat.
- AC1.4: fuzzer limpio; cero panics en logs (policy panic-free).

**Riesgo**: medio (protocolo 26.x, NBT). **Presupuesto**: 10 semanas.

---

### F2 — Worldgen paridad 1:1 (meses 4-7)

**Objetivo**: misma seed → **mismo mundo**, verificado por checksum. El corazón del claim.

**Entregables**:
- `neutron-worldgen`: density functions + noise (XORoshiro128 parity) + biome source + surface rules + carvers + placed features para overworld completo.
- Estructuras fase 1: stronghold, aldeas, ruinas, monumentos (orden por impacto técnico).
- **Test de oro**: golden data generado con server vanilla (50 seeds, chunks (0,0)-(15,15) de overworld) → checksums xxHash64 de bloques y biomas; CI compara contra Neutron.
- Bench de cps (criterion) vs referencia: objetivo > 250 cps @16 hilos.
- Datapacks de usuario: carga de `worldgen/` custom.

**Criterios de salida**:
- AC2.1: paridad 100% en las 50 seeds golden (0 mismatches de checksum).
- AC2.2: cps sostenido > 250 @16 hilos en hardware de referencia (vanilla ~14, Paper ~85, C2ME ~182 en 1.21.10 — benchmark publicado en `bench/results/F2-*.md`).
- AC2.3: estructura de carpetas 100% vanilla; un mundo generado por Neutron abre en vanilla con el mismo terreno.

**Riesgo**: medio-alto (features/estructuras son muchas). **Presupuesto**: 3-4 meses. *Mitigación: empezar con un subset de biomas y expandir; la CI de checksum marca el progreso sin ambigüedad.*

---

### F3 — Simulación vanilla (meses 6-12, en paralelo con F4)

**Objetivo**: comportamiento vanilla: bloques, fluidos, iluminación, redstone, spawns, survival básica.

**Entregables**:
- Iluminación: engine propio estilo Starlight con contrato "cualquier diferencia con vanilla es un bug".
- Redstone por fases:
  - **F3-A**: wire, torches, levers, buttons, doors, trapdoors — update order vanilla (PP W,E,N,S,D,U / NC W,E,D,U,N,S).
  - **F3-B**: comparators, repeaters, observers, hoppers, TNT.
  - **F3-C**: pistons + **quasi-connectivity** + block swapping (el punto más duro de vanilla).
  - **F3-D**: suite completa de "contraptions doradas" (100+ tests posicionales comparados contra server vanilla real).
- Fluidos (flujo determinista), spawns (ciclos/caps/pack spawning), XP, hambre, dormir, portales (nether/end dimensiones generadas y transitables).
- Persistencia de redstone/block entities en Anvil.

**Criterios de salida**:
- AC3.1: suite de contraptions doradas 100% verde, incluyendo tests posicionales (misma contraption en 5 posiciones distintas del mundo).
- AC3.2: iluminación: renders/snapshots de light data idénticos a vanilla en los 50 seeds golden (comparación automatizada de light arrays).
- AC3.3: survival básica jugable: minar → craft → comer → dormir → morir/revivir, todo con comportamiento vanilla verificado por bot.

**Riesgo**: **ALTO** (redstone es el mayor reto técnico del proyecto; vanilla es posicional). **Presupuesto**: 6 meses. *Mitigación: suite dorada desde F3-A (nunca construir sobre arena sin test); especialista dedicado; comparación contra server vanilla real en cada PR.*

---

### F4 — Escala: miles de jugadores (meses 8-14, paralelo a F3)

**Objetivo**: 500 jugadores simulados con p99 tick < 25 ms; camino a 1000+.

**Entregables**:
- Scheduler por regiones (single-writer por región, sync global mínimo) — benchmark de decisión A/B (tick global vs regional) publicado antes de adoptar.
- Optimizaciones: arenas, reuso de buffers, hot paths sin locks, batch de paquetes, netty-style outbox.
- Stress test: 500 bots (mineflayer/azalea distribuidos o simulación headless propia).
- Memory profiling (dhat/heaptrack) en CI para los 3 escenarios: idle / 100 / 500 jugadores.

**Criterios de salida**:
- AC4.1: 500 bots, view 10: TPS 20.0, p99 tick < 25 ms, sin degradación progresiva en 60 min.
- AC4.2: RAM por jugador idle < 1 MB sobre la base (< 150 MB).
- AC4.3: cps y startup no regresionan vs F2/F1 (benchmarks en CI).

**Riesgo**: medio-alto (paralelismo + determinismo). **Presupuesto**: 6 meses. *Mitigación: el determinismo dentro de región es contrato; nunca reordenar updates visibles.*

---

### F5 — Mobs y AI (meses 10-18, paralelo)

**Objetivo**: comportamiento vanilla de mobs: pasivos, hostiles, jefes (Ender Dragon como hito).

**Entregables**:
- Port de AI desde jar sin ofuscar (26.x): behavior por mob, pathfinding A* optimizado, targeting, damage/knockback/cooldown, proyectiles, XP orbs, villager trading.
- Spawns de mobs en chunk-gen y runtime con reglas vanilla (caps, luz, pack spawning).
- Combate completo (espadas/arcos/escudos/tridentes/encantamientos/estatus).

**Criterios de salida**:
- AC5.1: E2E: bot sobrevive 20 min en mundo survival; mobs aparecen, persiguen, atacan, mueren con drops vanilla.
- AC5.2: spot-checks de AI documentados (vídeo o log estructurado): creeper explota, zombie quema al amanecer, enderman se teletransporta, dragon hace loop de jefe.
- AC5.3: sin regresión de TPS con 50 mobs por chunk cargado (bench).

**Riesgo**: ALTO (AI es enorme y sutil). **Presupuesto**: 8 meses. *Mitigación: priorizar mobs críticos (zombie, creeper, esqueleto, enderman, villager, dragon); el resto por oleadas.*

---

### F6 — Plugins: WASM + Lua + API (meses 12-18)

**Objetivo**: ecosistema de plugins seguro y potente; convertir lo convertible.

**Entregables**:
- `neutron-plugin-api.wit` (eventos, comandos, world, entities, permissions) + runtime wasmtime (component model) + fuel/memory limits + hot reload + marketplace-ready manifest.
- Lua scripting (mlua) para plugins ligeros.
- **Conversor v1**: analizador estático Java→"convertible?" + recompilador bytecode→WASM para plugins puros (sin reflection) sobre API reimplementada.
- **PatchBukkit-style v0** (Fase B): runtime JVM embebido ejecutando jars Bukkit reales traducidos a eventos (bajo rendimiento, compat amplia) — evaluar contra el coste.
- Docs y ejemplos: plugin de referencia (evento → acción → chat) en Rust-WASM y Lua.

**Criterios de salida**:
- AC6.1: plugin WASM con panic interno → el servidor sigue vivo (test de crash automático).
- AC6.2: plugin con fuel limit 10M opcodes → kill sin daño; permisos denegados → no ejecuta.
- AC6.3: hot reload de plugin sin reiniciar server ni perder estado de jugadores.
- AC6.4: conversor v1: 3 plugins reales simples del ecosistema Bukkit convertidos y funcionando (casos documentados).
- AC6.5: benchmark: plugin WASM en hot path (p.ej. evento de daño) añade < 5 µs/tick.

**Riesgo**: alto (expectativas de compat). **Presupuesto**: 6 meses. *Mitigación: honestidad desde el README: "plugins nuevos = WASM/Lua; compat Bukkit = por capas y limitada".*

---

### F7 — Bedrock (paralelo, meses 14-20)

**Objetivo**: clientes Bedrock (26.x) conectan al mismo mundo.

**Entregables**: protocolo Bedrock (RakNet + login/play), mapeo de registries Java↔Bedrock, bridging de jugadores en el mismo mundo.
**Criterios de salida**: AC7.1: cliente Bedrock real entra y juega; AC7.2: jugadores Java y Bedrock coexisten en el mismo mundo; AC7.3: sin impacto en TPS Java (bench).
**Riesgo**: medio (protocolo distinto, registries divergentes). **Presupuesto**: 4-6 meses.

---

### F8 — 1.0 (meses 18-24)

**Objetivo**: release estable, verificable y defendible.

**Entregables**:
- Fuzzing continuo (24 h sin crash), panic-free audit, memory-leak audit (60 min idle estable).
- Benchmarks públicos reproducibles completos (BENCHMARKS.md final): vanilla/Paper/Pumpkin/Neutron en 2 máquinas (reference + budget).
- Documentación: guía de migración de mundos, config, plugins; binarios Windows/Linux/macOS × x86-64/ARM64.
- Post-mortem de parity: lista pública de desviaciones conocidas (si las hay) — transparencia.
- Release 1.0 + proceso de releases por versión de Mojang (SLA 7 días).

**Criterios de salida**: AC8.1-8.5: suite parity completa verde en la versión de `main`; benchmarks publicados; 72 h de uptime con 100 jugadores simulados sin leak; fuzzing limpio; docs completas.

---

## 3. Timeline consolidado

| Mes | F0 | F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 |
|---|---|---|---|---|---|---|---|---|---|
| 1 | ██ | | | | | | | | |
| 2-3 | | ████ | | | | | | | |
| 4-7 | | | ████████ | | | | | | |
| 6-12 | | | | █████████████ | | | | | |
| 8-14 | | | | | ████████████ | | | | |
| 10-18 | | | | | | ████████████████ | | | |
| 12-18 | | | | | | | ████████████ | | |
| 14-20 | | | | | | | | ████████ | |
| 18-24 | | | | | | | | | ████████████ |

*Hitos clave: mes 3 = jugador real en mundo flat · mes 7 = paridad de worldgen con checksum · mes 12 = redstone suite dorada · mes 14 = 500 jugadores · mes 18 = plugins WASM · mes 24 = 1.0.*

## 4. Pipeline de versiones (SLA: main ≤ 7 días tras release de Mojang)

Ver ARCHITECTURE.md §10 (D0-D4). En la práctica por drop (~3/año):

| Actividad | Tiempo | Dueño |
|---|---|---|
| D0-D1: detectar + extraer jar (sin ofuscar desde 26.1) | 1 día | pipeline CI |
| D2: codegen + compilar | 1 día | pipeline CI |
| D3: golden data nuevo (server vanilla headless) | 1-2 días | harness |
| D4: parity suite + benchmarks + release `main` | 1-2 días | CI + humano |
| **Total** | **4-7 días** | |

## 5. Riesgos y mitigaciones

| Riesgo | Severidad | Mitigación |
|---|---|---|
| Paridad de redstone (posicional, QC, 1.21.2+) | **CRÍTICO** | Suite dorada posicional desde F3-A; comparación contra server real en cada PR |
| Paridad de mob AI (enorme, sutil) | **ALTO** | Port desde jar sin ofuscar; oleadas por prioridad; spot-checks automatizados |
| Expectativas de compat Bukkit | **ALTO** | Estrategia por capas honesta (F6); comunicación temprana |
| Escala 1000+ jugadores | MEDIO-ALTO | Scheduler regional con benchmark A/B; stress desde F4 |
| Cadencia de Mojang (3 drops/año) | MEDIO | Pipeline D0-D4 desde F1; tests de regresión por versión |
| Scope creep ("y además...") | ALTO | Regla: cada fase termina con evidencia publicada; backlog separado |
| Equipo pequeño / burnout | MEDIO | Orquestación con agentes (ORCHESTRATION.md), presupuestos, STATE.md |

## 6. Fuera de alcance (explícito)

- Combat 1.8 (Pumpkin tampoco; no es vanilla actual).
- Mods Forge/Fabric (discutir en post-1.0; requeriría cargar código JVM — choca con "Rust nativo").
- Plugins Bukkit al 100%: solo por capas (F6), nunca el objetivo principal.
- Réplicas de servidores con lógica custom (Minigames tipo Hypixel): fuera, el foco es vanilla 1:1 + plugins.
- FPS del cliente: métrica de cliente, no del servidor (ver BENCHMARKS.md §2).
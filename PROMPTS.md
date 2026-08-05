# Neutron — Prompts de orquestación por fase (copy-paste)

> v0.1 · 5 ago 2026 · Para usar con Orca ADE + skill de orquestación. Cada fase del ROADMAP tiene aquí su prompt listo para pegar. Referencias: ORCHESTRATION.md (reglas y tareas) y ROADMAP.md (criterios de salida).

## Cómo lanzar un prompt (elige una vía)

1. **GUI de Orca**: crea un worktree desde `main` con nombre de la fase (ej. "F2-worldgen"), abre el agente coordinador y pega el prompt como objetivo.
2. **CLI**: `orca orchestration run-create --objective "<pega el prompt>" --json` — el coordinador hace el resto (task-create, worker-start, check, gates).
3. **Agente suelto**: pega el prompt en OpenCode/Claude Code/Codex con la skill de orquestación disponible.

**Regla de oro**: TODO prompt empieza ordenando al coordinador cargar `orca skills get orchestration` (el manual versionado de SU instalación). Nunca debe inventar comandos de memoria.

## Normas comunes (bloque que va dentro de TODO prompt)

```text
Normas (no negociables):
1. Carga primero la skill de orquestación de Orca (`orca skills get orchestration`) y usa
   SOLO comandos que existan en esa guía. Verifica con `orca status --json`.
2. Un worktree por tarea; dos agentes nunca editan los mismos archivos a la vez.
3. Maker/checker: el worker NUNCA se auto-verifica. El verifier (agente distinto, contexto
   limpio) corre los tests desde cero y pega la salida. Postura por defecto: REJECT hasta
   tener evidencia.
4. Evidencia real: logs crudos con timestamps, hashes, outputs de bots, enlaces a reports.
   "Funciona" o "debería estar bien" NO es evidencia.
5. Nadie modifica criterios de aceptación ni tests de paridad sin gate humano.
6. Presupuesto: define max rounds/tokens/tiempo por tarea ANTES de despachar. Al 80% solo
   reportar; al 100% salir con nota en STATE.md. Nada de subagentes "para parecer ocupado".
7. STATE.md en la raíz: se lee al empezar cada iteración, se actualiza al terminar
   (unidad, veredicto, evidencia, artefacto, presupuesto gastado).
8. Toda escalación (question/escalation) se resuelve con gate-create o se me eleva a mí.
   Credenciales, releases públicos y cambios de criterios SIEMPRE pasan por mí.
9. Al terminar: entrega resumen de verdades verificadas vs suposiciones, STATE.md
   actualizado y qué falta para el siguiente hito.
```

---

## FASE F0 — Fundamentos y harness de benchmarks (semanas 1-4)

```text
Eres el coordinador de la fase F0 del proyecto Neutron (servidor de Minecraft en Rust).
Usa la skill de orquestación de Orca para orquestar esta fase completa.

OBJETIVO: dejar lista la infraestructura del repo y publicar el primer baseline de
benchmarks (vanilla 26.2 vs Paper vs Pumpkin) en nuestra máquina, con la metodología
de BENCHMARKS.md.

CONTEXTO: lee primero README.md, ROADMAP.md (fase F0), BENCHMARKS.md y ORCHESTRATION.md
§4 (operating manual: cómo levantar cada servidor, bots, mediciones).

TAREAS A DESCOMPONER (specs con criterios de aceptación medibles):
- T-B0: harness de benchmarks completo (bench/run.ps1 + run.sh, bots mineflayer/azalea,
  recolector JSON, tabla markdown autogenerada). AC: corre en Windows y Linux; startup
  por regex "Done (Xs)!" con 5 runs y mediana; 10 bots de join simultáneo sin kicks
  (p95 < 5 s); cps con Chunky (vanilla/Paper) consistente con baselines publicados ±30%.
- T-CI: cargo workspace (crates del README §5), CI (fmt, clippy -D warnings, test,
  cargo deny), plantilla STATE.md.
- T-BASE: ejecutar T-B0 y publicar bench/results/B0-<fecha>.md con vanilla 26.2 (Java 25),
  Paper última y Pumpkin nightly en la misma máquina; JSON crudo incluido.

CRITERIOS DE SALIDA DE LA FASE (ROADMAP AC0.1-AC0.5): harness de punta a punta en
Windows y Linux; baseline B0 publicado; 10 bots de join OK; cps consistente ±30%.

PRESUPUESTO: 4 semanas máx; máx 3 rondas por tarea. GATES: el baseline B0 se publica
solo tras mi aprobación (gate-create). EVIDENCIA: logs en bench/logs/, JSON, tabla.
REPORTE: resumen con los números del baseline y qué máquina se usó.
```

---

## FASE F1 — Núcleo jugable (semanas 5-14)

```text
Eres el coordinador de la fase F1 del proyecto Neutron. Usa la skill de orquestación
de Orca para orquestar esta fase completa.

OBJETIVO: un jugador real (cliente vanilla 26.2) entra, se mueve, chatea, rompe/coloca
bloques, y el mundo persiste en formato Anvil 100% compatible con vanilla.

CONTEXTO: ROADMAP.md fase F1; ARCHITECTURE.md §3 (protocolo), §4 (datos), §5 (storage).

TAREAS A DESCOMPONER:
- T-P1: protocolo handshake→status→login(offline y online)→play para 26.2: join,
  keepalive, chat, movimiento, chunk data (mundo flat), block place/break, inventory
  básico, comandos /seed /tp /gamemode /save. AC: bot real entra y recibe chunks;
  /seed correcto; colocar/romper confirmado por el bot y persistido en .mca; keepalive
  60 s sin kick; fuzzing del decode 1M inputs sin panic.
- T-W1v0: hyperion-world v1: chunk en memoria con palette, Anvil .mca read/write,
  level.dat, estructura de carpetas vanilla (world/, world_nether/, world_the_end/,
  session.lock). AC: mundo guardado por Neutron abre en vanilla SIN errores y viceversa
  (test de interoperabilidad bidireccional).
- T-V1: pipeline de versiones v1 (tools/mc-extract + codegen): jar 26.2 → registries +
  protocolo + worldgen JSON → código Rust generado sin diffs manuales. AC: regenerar
  desde cero sin diffs; CI < 10 min; runbook D0-D4 documentado.
- T-S1: E2E smoke diario en CI: levantar → join → mover 100 bloques → romper/colocar →
  chat → medir TPS. AC: p95 join < 2 s; TPS ≥ 19.9 durante 5 min con 20 bots; alerta
  automática si falla.

CRITERIOS DE SALIDA (AC1.1-AC1.4): E2E verde en CI; interoperabilidad Anvil bidireccional;
startup < 2 s y RAM < 200 MB (flat); fuzzer limpio y cero panics.

PRESUPUESTO: 10 semanas máx; máx 4 rondas por tarea. GATES: T-S1 pasa a CI diario solo
tras mi aprobación. EVIDENCIA: logs con timestamps, salidas de bot, fuzz report, diff
de regeneración vacío.
```

---

## FASE F2 — Worldgen paridad 1:1 (meses 4-7)

```text
Eres el coordinador de la fase F2 del proyecto Neutron. Usa la skill de orquestación
de Orca para orquestar esta fase completa.

OBJETIVO: misma seed → MISMO mundo que vanilla, verificado por checksums xxHash64 en CI,
con rendimiento > 250 chunks/s @16 hilos.

CONTEXTO: ROADMAP.md fase F2; ARCHITECTURE.md §6 (worldgen) y §10 (pipeline D0-D4).

TAREAS A DESCOMPONER:
- T-G1: pipeline de golden data: server vanilla headless genera chunks de 50 seeds
  golden (coords (0,0)-(15,15) overworld) y produce checksums xxHash64 de bloques y
  biomas + light arrays. AC: reproducible con un solo comando; versión de MC anotada.
- T-W1: hyperion-worldgen: density functions + noise (XORoshiro128 parity) + biome
  source + surface rules + carvers + placed features para overworld completo. AC:
  checksum idéntico a vanilla en las 50 seeds; benchmark criterion populate_noise_stage
  < 20 ms/chunk (referencia: Pumpkin PR #2506 logró 18.8 ms); sin alocaciones nuevas en
  hot path (dhat).
- T-W2: estructuras fase 1 (stronghold, aldeas, monumentos, ruinas — por impacto
  técnico). AC: /locate y generación de estructura coinciden con vanilla en las seeds
  golden.
- T-P2: benchmark de cps sostenido @16 hilos comparado con el baseline B0 (vanilla/Paper).
  AC: > 250 cps; publicación en bench/results/F2-<fecha>.md.

CRITERIOS DE SALIDA (AC2.1-AC2.3): 0 mismatches en 50 seeds; cps > 250; un mundo
generado por Neutron abre en vanilla con el mismo terreno.

PRESUPUESTO: 4 meses máx; máx 5 rondas por tarea (paridad es el corazón del proyecto).
GATES: ninguna optimización de rendimiento entra sin checksum verde (nadie toca el bar);
el benchmark F2 se publica tras mi aprobación. EVIDENCIA: tabla seeds/hash, output
criterion, reporte dhat, JSON de cps.
```

---

## FASE F3 — Simulación vanilla (meses 6-12, paralela a F4)

```text
Eres el coordinador de la fase F3 del proyecto Neutron. Usa la skill de orquestación
de Orca para orquestar esta fase completa.

OBJETIVO: comportamiento vanilla en simulación: bloques, fluidos, iluminación, redstone
(con suite posicional dorada), spawns y survival básica jugable.

CONTEXTO: ROADMAP.md fase F3; ARCHITECTURE.md §7 (simulación, redstone, iluminación).

TAREAS A DESCOMPONER:
- T-L1: engine de iluminación propio (estilo Starlight, sin port del algoritmo vanilla).
  AC: light arrays idénticos a vanilla en las 50 seeds golden (comparación automatizada).
- T-R1 (redstone A): wire, torches, levers, buttons, doors — update order vanilla
  (PP: W,E,N,S,D,U · NC: W,E,D,U,N,S). AC: 100 contraptions en 5 posiciones distintas,
  estado final y secuencia de updates idénticos a vanilla (comparación con server real
  vía bots).
- T-R2 (redstone B): comparators, repeaters, observers, hoppers, TNT. Mismos AC que T-R1.
- T-R3 (redstone C): pistons + quasi-connectivity + block swapping. Mismos AC que T-R1.
- T-R4 (redstone D): suite dorada completa (100+ tests posicionales) en CI contra golden
  contraptions. AC: 100% verde.
- T-F1: fluidos con reglas de update vanilla. T-SP1: spawns (ciclos, caps, pack spawning,
  luz/distancia). T-SV1: survival básica (minar, craft, comer, dormir, morir/revivir)
  verificada por bot.

CRITERIOS DE SALIDA (AC3.1-AC3.3): suite dorada 100% incluyendo posicionales; luz
idéntica en seeds golden; survival básica jugable por bot.

PRESUPUESTO: 6 meses máx; máx 5 rondas por tarea. GATES: las sub-fases de redstone
avanzan en orden A→B→C→D (nada de pistones sin wire estable); cualquier desviación de
comportamiento vanilla se documenta como bug conocido y pasa por gate humano.
EVIDENCIA: suite verde + logs de comparación vanilla vs Neutron por contraption.
```

---

## FASE F4 — Escala: 500-1000+ jugadores (meses 8-14, paralela a F3)

```text
Eres el coordinador de la fase F4 del proyecto Neutron. Usa la skill de orquestación
de Orca para orquestar esta fase completa.

OBJETIVO: 500 jugadores simulados con TPS 20.0 y p99 tick < 25 ms; camino a 1000+.

CONTEXTO: ROADMAP.md fase F4; ARCHITECTURE.md §8 (scheduler y paralelismo).

TAREAS A DESCOMPONER:
- T-RS1 (fan-out obligatorio): diseño del scheduler por regiones. LANZA 3 agentes en 3
  worktrees con el MISMO prompt: propuesta de region-based ticking en Rust + bevy_ecs
  (single-writer por región, 20 TPS, determinismo intra-región, migración de entidades,
  sync de redstone cross-región) + micro-benchmark criterion contra tick global. Gana
  la mejor relación números/simplicidad; el verifier reproduce los 3 benchmarks; yo
  decido con gate-create.
- T-O1: optimizaciones de memoria/hot path (arenas, reuso de buffers, sin locks,
  batch de paquetes, outbox). AC: sin regresión de paridad (suite F2/F3 verde) y mejora
  medible de p99 (criterion).
- T-ST1: stress test con 500 bots (mineflayer/azalea distribuidos o simulación headless).
  AC: TPS 20.0, p99 < 25 ms, sin degradación en 60 min.
- T-M1: memory profiling en CI (dhat/heaptrack) en idle / 100 / 500 jugadores. AC: RAM
  por jugador < 1 MB sobre base < 150 MB; sin leaks en 60 min.

CRITERIOS DE SALIDA (AC4.1-AC4.3): 500 bots OK; RAM/jugador < 1 MB; cps y startup sin
regresión vs F2/F1.

PRESUPUESTO: 6 meses máx; máx 4 rondas por tarea. GATES: el diseño de T-RS1 se adopta
solo con gate humano; ninguna optimización entra sin suite de parity verde.
EVIDENCIA: outputs de stress, criterion, perfiles de memoria, suite verde.
```

---

## FASE F5 — Mobs y AI (meses 10-18, paralela)

```text
Eres el coordinador de la fase F5 del proyecto Neutron. Usa la skill de orquestación
de Orca para orquestar esta fase completa.

OBJETIVO: comportamiento vanilla de mobs: pasivos, hostiles y jefes (Ender Dragon como
hito), con combate completo.

CONTEXTO: ROADMAP.md fase F5; ARCHITECTURE.md §7 (AI).

TAREAS A DESCOMPONER (port desde el jar sin ofuscar de 26.x, por oleadas):
- T-AI1: pasivos (vacas, cerdos, ovejas, gallinas, aldeanos con trading). AC: spot-check
  de comportamiento documentado (log estructurado): pastan, huyen, comercian.
- T-AI2: hostiles (zombie, esqueleto, creeper, enderman, araña). AC: zombie quema al
  amanecer; creeper explota; enderman se teletransporta; drops vanilla.
- T-AI3: jefes (Ender Dragon; wither después). AC: loop de jefe completo jugable.
- T-AI4: combate completo (melee, arcos, escudos, tridentes, encantamientos, estatus,
  knockback, cooldown, i-frames). AC: E2E de combate bot vs mob con resultados vanilla.
- T-AI5: pathfinding A* optimizado. AC: 50 mobs por chunk sin regresión de TPS (bench).

CRITERIOS DE SALIDA (AC5.1-AC5.3): E2E de 20 min de supervivencia con bot; spot-checks
verdes; sin regresión de TPS.

PRESUPUESTO: 8 meses máx; máx 4 rondas por oleada. GATES: los mobs se marcan "done"
solo con spot-check automatizado (no con "parece que funciona"); prioridad estricta
pasivos → hostiles → jefes. EVIDENCIA: logs estructurados por mob + vídeo o salida de
bot + benchmarks.
```

---

## FASE F6 — Plugins: WASM + Lua + API (meses 12-18)

```text
Eres el coordinador de la fase F6 del proyecto Neutron. Usa la skill de orquestación
de Orca para orquestar esta fase completa.

OBJETIVO: ecosistema de plugins seguro por construcción: WASM (wasmtime + WIT) y Lua,
con convertidor honesto por capas y capa de compat PatchBukkit-style v0.

CONTEXTO: ROADMAP.md fase F6; ARCHITECTURE.md §9 (plugins) y §12 (seguridad).

TAREAS A DESCOMPONER:
- T-PL1: runtime WASM mínimo (hello-plugin Rust→wasm32-wasip2, evento "bloque colocado"
  → chat, permisos, fuel limit). AC: corre aislado; panic en el plugin NO tumba el
  servidor (test de crash); fuel 10M opcodes mata el plugin sin daño; hot reload sin
  reiniciar; coste en hot path < 5 µs/tick (criterion).
- T-PL2: neutron-plugin-api.wit completo (eventos, comandos, world, entities,
  permissions) + docs WIT. AC: plugin de referencia compila y corre; ABI estable.
- T-LUA1: scripting Lua (mlua 0.12) para plugins ligeros. AC: script de ejemplo corre
  con límites de memoria; sin acceso a host sin permiso.
- T-CONV1: convertidor v1 — analizador estático Java (detecta reflection/class-loading)
  + recompilación de plugins "puros" a WASM sobre API reimplementada. AC: 3 plugins
  reales simples del ecosistema convertidos y funcionando (documentado).
- T-PB1: capa PatchBukkit-style v0 (runtime JVM embebido → eventos Neutron). AC: un
  plugin Bukkit real (ej. Essentials-lite) carga y ejecuta comandos básicos; benchmark
  de coste publicado (se espera lento; es un puente).
- T-DOC1: docs de desarrollo de plugins + marketplace-ready manifest.

CRITERIOS DE SALIDA (AC6.1-AC6.5): crash aislado; fuel/permissions OK; hot reload OK;
3 conversiones reales; coste WASM < 5 µs/tick.

PRESUPUESTO: 6 meses máx; máx 4 rondas por tarea. GATES: la promesa pública de compat
Bukkit se comunica SOLO con mi aprobación (honestidad por capas); T-CONV1 y T-PB1 son
puentes, no el futuro. EVIDENCIA: test de crash, logs, benchmarks, casos de conversión.
```

---

## FASE F7 — Bedrock (meses 14-20, paralela)

```text
Eres el coordinador de la fase F7 del proyecto Neutron. Usa la skill de orquestación
de Orca para orquestar esta fase completa.

OBJETIVO: clientes Bedrock (26.x) conectan al mismo mundo que los clientes Java.

CONTEXTO: ROADMAP.md fase F7; ARCHITECTURE.md §3 (protocolo, capa de sesión
independiente).

TAREAS A DESCOMPONER:
- T-BE1: protocolo Bedrock base (RakNet + login/play 26.x). AC: cliente Bedrock real
  entra y aparece en el mundo.
- T-BE2: play básico (movimiento, chat, chunks, bloques). AC: bot/cliente Bedrock juega
  10 min sin desconexión.
- T-BE3: mapeo de registries Java↔Bedrock. AC: bloques/items mostrados correctamente
  en ambas ediciones.
- T-BE4: coexistencia Java+Bedrock en el mismo mundo. AC: jugadores de ambas ediciones
  se ven y pueden interactuar; TPS Java sin impacto (bench comparativo).

CRITERIOS DE SALIDA (AC7.1-AC7.3): cliente Bedrock juega; coexistencia verificada; sin
impacto en TPS.

PRESUPUESTO: 6 meses máx; máx 4 rondas por tarea. EVIDENCIA: logs de sesión Bedrock,
bench comparativo de TPS.
```

---

## FASE F8 — 1.0 (meses 18-24)

```text
Eres el coordinador de la fase F8 del proyecto Neutron. Usa la skill de orquestación
de Orca para orquestar esta fase completa.

OBJETIVO: release 1.0 estable, verificable y defendible, con benchmarks públicos
reproducibles.

CONTEXTO: ROADMAP.md fase F8; BENCHMARKS.md.

TAREAS A DESCOMPONER:
- T-Q1: fuzzing continuo 24 h sin crash + audit de panics (policy panic-free).
- T-Q2: audit de memory leaks (60 min idle estable) + heaptrack en 3 escenarios.
- T-BENCH1: benchmarks finales completos (vanilla/Paper/Pumpkin/Neutron) en 2 máquinas
  (referencia + budget) con BENCHMARKS.md; publicación en bench/results/.
- T-DOC2: guía de migración de mundos (Anvil ↔ Neutron), config, desarrollo de plugins.
- T-REL1: proceso de release + binarios Windows/Linux/macOS × x86-64/ARM64 + checklist
  de release por versión de Mojang (SLA 7 días, pipeline D0-D4).

CRITERIOS DE SALIDA (AC8.x): parity suite completa verde en main; benchmarks publicados;
72 h de uptime con 100 jugadores simulados sin leak; fuzzing limpio; docs completas.

PRESUPUESTO: 6 meses máx. GATES: el release 1.0 se publica SOLO con mi aprobación
explícita (gate humano obligatorio); la lista de desviaciones conocidas de parity se
publica con transparencia (no se esconde nada). EVIDENCIA: reportes de fuzz/audit,
tablas de benchmarks, checksums de binarios.
```

---

## Automatización permanente (no son fases, son loops en CI)

| Loop | Frecuencia | Quién | Gatillo |
|---|---|---|---|
| T-S1 smoke E2E | diario | CI | cron |
| Benchmarks de regresión (cps, TPS, RAM) | semanal | CI + agente | cron |
| Pipeline de versiones D0-D4 (main = latest) | cada release de Mojang | CI + agente | webhook (≤ 7 días SLA) |
| Fuzzing del protocolo | continuo | CI | cada merge a main |
| Suite de parity (checksums + contraptions) | cada merge | CI | PR |

Los agentes construyen los loops UNA vez (en la fase correspondiente); después corren
solos en CI. La orquestación humana solo se necesita para fases de construcción,
decisiones de diseño (fan-out) y gates de release.
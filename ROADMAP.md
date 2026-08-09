# Neutron — Roadmap

> El progreso se mide en **BARS y RONDAS**, no en calendario. Un bar es una referencia real e innegociable (checksum, benchmark, server real) que un critic ciego inspecciona (Gauntlet Loop, ver AGENTS.md §2).

## 0. Cómo leer este roadmap

- **Bar**: lo que el critic compara contra nuestro artefacto (el "Call of Duty" de cada fase). No se discute: se cumple o no.
- **Rondas**: ciclos build → critic → fix. Sin cap arbitrario: se itera hasta que el bar gana, 2 rondas sin mejora, o presupuesto agotado.
- **Prompt**: cada fase tiene un prompt completo listo para copiar y pegar en pi. Cada prompt detalla:
  - **Tareas** a distribuir entre subagentos (Agent tool)
  - **MCP tools a usar** (para investigación, búsqueda en internet/crates.io/minecraft docs)
  - **Tracking** con TodoWrite para cada unidad de trabajo
  - **Gates humanos** con AskUserQuestion
- **Cómo lanzar un run**: copia el `=== PROMPT F<NNN> ===` de la fase actual y pégalo en ZCode. ZCode leerá este ROADMAP.md automáticamente y ejecutará todo.

## 1. Cadencia de Mojang (verificada — ARCHITECTURE.md Anexo A §1)

| Versión | Tipo | Fecha |
|---|---|---|
| 1.21.11 "Mounts of Mayhem" | última 1.x (jar ofuscado, Java 21) | 9 dic 2025 |
| 26.1 "Tiny Takeover" | jar sin ofuscar, Java 25 | 24 mar 2026 |
| 26.2 "Chaos Cubed" | **versión objetivo de `main` hoy** | 16 jun 2026 |
| 26.3 | en snapshots | Q3 2026 |
| ~26.x | ~3 drops/año + hotfixes | continuo |

> **Pipeline D0-D4**: cuando Mojang saque una nueva versión, sigue el pipeline en §4 (Pipeline de versiones) para actualizar `main` en ≤ 7 días.

---

## 2. Fases — Prompts completos listos para copiar y pegar

Cada bloque `=== PROMPT F<NNN> ===` es un prompt completo que puedes copiar y pegar en ZCode. ZCode automáticamente:
1. Lee el contexto del proyecto (STATE.md, runs/, etc.)
2. Usa MCP tools para investigar referencias técnicas
3. Lanza subagentos (Agent tool) para cada pieza en paralelo
4. Usa TodoWrite para tracking de tareas
5. Ejecuta el Gauntlet Loop (builder → critic → fix → repetir)
6. Actualiza STATE.md y runs/run-NNN.md

---

### F0 — Fundamentos y harness ✅ COMPLETADO

**Objetivo**: infraestructura del repo + primer baseline público.
**Estado**: COMPLETADO — harness Rust funcional, 4 servidores, 5 escenarios, 84 configuraciones benchmarkeadas. Ver `bench/results/FULL-BENCHMARK-REPORT.md`.

---

### F1 — Núcleo jugable

**Objetivo**: un jugador real entra, juega y el mundo persiste en Anvil vanilla.
**Bar**: bot vanilla 26.2 juega 10 min sin kick (E2E en CI); mundo guardado abre en vanilla y viceversa; fuzz del decode 1M inputs sin panic; startup < 2 s; RAM < 200 MB.
**Piezas**: protocolo 26.2 (login/play) · world v1 (Anvil, level.dat, carpetas vanilla) · pipeline de versiones v1 · E2E diario. **Riesgo**: medio.

```text
=== PROMPT F1 — Copiar y pegar en ZCode ===

Eres el LEAD del proyecto Neutron. Lanza el Gauntlet Loop para la fase F1: Núcleo jugable.

## 📖 PASO 1 — Leer contexto

Lee TODOS estos archivos:
- STATE.md, AGENTS.md, ARCHITECTURE.md (lee completo, especialmente §3 Protocolo y §5 Mundo)
- BENCHMARKS.md, runs/run-001.md, ROADMAP.md

## 📋 PASO 2 — Tracking de tareas

Crea el tracking con TodoWrite:
```json
[
  {"content": "T1 — Protocolo 26.2: login/play packets (handshake, login, keep_alive, join_game, chat, position, spawn, chunk_data, block_update). Codec generado", "status": "in_progress", "priority": "high"},
  {"content": "T2 — Mundo Anvil: .mca read/write, level.dat, carpetas vanilla, session.lock, formato .hyp", "status": "pending", "priority": "high"},
  {"content": "T3 — Fuzz del decode: cargo-fuzz, 1M inputs, 0 panics. Reporte JSON", "status": "pending", "priority": "medium"},
  {"content": "T4 — E2E: bot 26.2 conecta, juega 10 min sin kick, mide startup < 2s, RAM < 200MB", "status": "pending", "priority": "high"}
]
```

## 🏗️ PASO 3 — Ejecutar en paralelo (T1 + T2)

Lanza T1 y T2 en paralelo con subagentos:

```json
// Subagente T1 — Protocolo
Agent(subagent_type="general-purpose", run_in_background=true,
  prompt="Implementa neutron-protocol para Minecraft 26.2: packets de login (handshake, login_start, login_success, encryption_request/response, set_compression), play (keep_alive, join_game, server_data, chat, player_position, set_default_spawn, chunk_data, block_update, synchronize_player_position). Usa tokio + bytes. Código generado, no a mano. Referencias: ARCHITECTURE.md §3, wiki.vg/Protocol")

// Subagente T2 — Mundo
Agent(subagent_type="general-purpose", run_in_background=true,
  prompt="Implementa neutron-world para Minecraft 26.2: lectura/escritura de Anvil .mca (region compuesta de chunks NBT), level.dat (NBT), estructura de carpetas world/world_nether/world_the_end/, session.lock, formato .hyp (zstd). Referencias: ARCHITECTURE.md §5, minecraft.wiki/Region_file_format")
```

Espera ambos resultados con TaskOutput.

## 🚧 PASO 4 — Gate humano: revisar protocolo

Pregunta al humano:
```json
AskUserQuestion(questions=[{
  "question": "¿El protocolo 26.2 es correcto? Revisa: login flow, packet IDs, chunk data format, compression. Ver ARCHITECTURE.md §3",
  "header": "Gate F1",
  "options": [
    {"label": "Aprobado", "description": "Protocolo correcto, continuar con fuzz y E2E"},
    {"label": "Requiere fixes", "description": "Hay problemas que corregir antes de continuar"}
  ]
}]
```

## 🏗️ PASO 5 — Ejecutar T3 + T4 (después del gate)

Si el gate es aprobado:

```json
// Subagente T3 — Fuzz
Agent(subagent_type="general-purpose", run_in_background=true,
  prompt="Implementa fuzz del decode de paquetes de protocolo para neutron-protocol: usa cargo-fuzz con arbitrary, genera 1M inputs aleatorios, verifica cero panics. Reporta en bench/results/fuzz-v1.json")

// Subagente T4 — E2E
Agent(subagent_type="general-purpose", run_in_background=true,
  prompt="Implementa el test E2E: bot vanilla 26.2 se conecta a neutron-cli, juega 10 min (moverse, romper, poner bloques, chatear), el servidor no lo kick. Usa mineflayer o azalea. Mide startup time y RAM")
```

## 🔄 PASO 6 — Gauntlet Loop (critic ciego por cada unidad)

Lanza un subagente critic con contexto limpio para CADA pieza:

**Critic Protocolo:**
```json
Agent(subagent_type="general-purpose",
  prompt="Eres el CRITIC ciego. Inspecciona crates/neutron-protocol/: ¿Los paquetes de login siguen el flujo wiki.vg? ¿El codec maneja compresión? ¿Packet IDs correctos para 26.2? ¿encryption_request/response funciona? ¿chunk_data tiene formato correcto (biomes, heightmaps, block entities)? Ejecuta cargo check, cargo test. Devuelve PASS o FAIL con el gap más grande.")
```

**Critic Mundo:**
```json
Agent(subagent_type="general-purpose",
  prompt="Eres el CRITIC ciego. Inspecciona crates/neutron-world/: ¿Lee y escribe .mca correctamente? ¿level.dat tiene campos correctos para 26.2? ¿Estructura de carpetas idéntica a vanilla? ¿session.lock funciona? Ejecuta cargo check, cargo test. Evidencia: mundo escrito por Neutron abre en vanilla.")
```

**Critic Fuzz:**
```json
Agent(subagent_type="general-purpose",
  prompt="Eres el CRITIC ciego. Inspecciona bench/results/fuzz-v1.json: ¿1M inputs ejecutados? ¿0 panics? ¿Inputs cubren todos los tipos de paquetes? Ejecuta: cargo fuzz run decode -- -runs=1000000")
```

**Critic E2E:**
```json
Agent(subagent_type="general-purpose",
  prompt="Eres el CRITIC ciego. Inspecciona logs del E2E y bench/results/startup-v1.json: ¿Bot 26.2 conectó y se mantuvo 10 min? ¿Startup < 2s? ¿RAM < 200MB? ¿El mundo persiste después de detener el servidor?")
```

Si FAIL → gap más grande → builder corrige → repetir critic. Si PASS → siguiente.
Si 2 rondas sin mejora → registrar en STATE.md.

## ✅ PASO 7 — Salida esperada

- `crates/neutron-protocol/` — crate funcional
- `crates/neutron-world/` — crate funcional con Anvil
- `bench/results/fuzz-v1.json` — fuzz report
- `bench/results/startup-v1.json` — startup benchmark
- `runs/run-002.md` — evidencia completa
- STATE.md actualizado a "F1: núcleo jugable"

Actualiza STATE.md y lanza el prompt de F2.

=== FIN PROMPT F1 ===
```

---

### F2 — Worldgen paridad 1:1

**Objetivo**: misma seed → mismo mundo, verificado por checksum.
**Bar**: checksum xxHash64 idéntico a vanilla en 50 seeds golden (0 mismatches); cps > 250 @16 hilos reproducido; un mundo generado por Neutron abre en vanilla con el mismo terreno.
**Piezas**: golden data pipeline · density functions + noise + surface + carvers + features · estructuras fase 1 · bench de cps. **Riesgo**: medio-alto.

```text
=== PROMPT F2 — Copiar y pegar en ZCode ===

Eres el LEAD del proyecto Neutron. F2: Worldgen paridad 1:1.

## 📖 PASO 1 — Leer contexto

ARCHITECTURE.md §6 (Worldgen) — leer COMPLETO. Prestar atención a:
- Pipeline de worldgen: noise → biome → surface → carvers → features → structures → spawn → light
- RNG: XORoshiro128
- Verificación: xxHash64 chunk checksum contra golden data
- Golden data pipeline descrita en §6
- DAG de chunks (3×3 neighbor dependency para features/structures)
- Datapack vanilla embebido para density functions/configs JSON

Lee también BENCHMARKS.md, STATE.md, runs/run-002.md.

## 📋 PASO 2 — Tracking de tareas

```json
[
  {"content": "T1 — Golden data: 50 seeds, xxHash64 de cada chunk, hashes.json", "status": "in_progress", "priority": "high"},
  {"content": "T2 — Noise + density functions + biomes", "status": "pending", "priority": "high"},
  {"content": "T3 — Surface rules + carvers + features (depende de T2)", "status": "pending", "priority": "high"},
  {"content": "T4 — Structures fase 1 (depende de T3)", "status": "pending", "priority": "medium"},
  {"content": "T5 — CPS benchmark (depende de T1)", "status": "pending", "priority": "medium"}
]
```

## 🏗️ PASO 3 — Ejecutar T1 + T2 en paralelo

```json
Agent(subagent_type="general-purpose", run_in_background=true,
  prompt="Crea tools/golden-data/ que: 1) levanta Vanilla 26.2 headless, 2) genera N chunks por seed, 3) calcula xxHash64 de cada chunk, 4) guarda en JSON con seed, chunk coords, hash. Para 50 seeds fijas. Incluye script tools/golden-data/generate.sh y tools/golden-data/hashes.json")

Agent(subagent_type="general-purpose", run_in_background=true,
  prompt="Implementa neutron-worldgen noise: 1) XORoshiro128 PRNG exacto (verificar contra vanilla con test), 2) Perlin noise 3D con octaves, 3) Density functions desde JSON del datapack vanilla, 4) Biome source con temperatura/humidity/continentalness/erosion/depth/weirdness. Paridad verificable con xxHash64 contra golden data")
```

Espera ambos, luego gate humano para golden data.

## 🚧 PASO 4 — Gate humano: golden data

AskUserQuestion: "¿Los golden data para 50 seeds son correctos? Revisar hashes.json"

## 🏗️ PASO 5 — Ejecutar T3, T4, T5

Si gate approved, lanza en cadena:
- T3 (surface) → T4 (structures)
- T5 (benchmark) independiente, después de T1

## 🔄 PASO 6 — Critic ciego

Lanza subagente critic para cada componente:
- **Critic noise**: compara chunks contra golden data, test de paridad, XORoshiro128 contra vanilla
- **Critic surface/features**: verifica surface rules, carvers, features. Checksum en 10 seeds
- **Critic structures**: verifica villages y strongholds contra vanilla
- **Critic benchmark**: ejecuta benchmark, verifica cps > 250

## 🎯 Bar

- [ ] 50 seeds golden: xxHash64 idéntico a vanilla — **0 mismatches**
- [ ] cps > 250 @16 hilos, reproducido en Windows y Linux
- [ ] Mundo generado por Neutron abre en vanilla con el mismo terreno

## ✅ Salida

- `tools/golden-data/` con 50+ seeds de referencia
- `crates/neutron-worldgen/` funcional con paridad 1:1
- `bench/results/cps-f2.json`
- `runs/run-003.md`
- STATE.md → "F2: worldgen paridad 1:1"

=== FIN PROMPT F2 ===
```

---

### F2d — Paridad 1:1 byte-identical

**Objetivo**: cerrar los gaps restantes para que same seed = chunks idénticos a vanilla.
**Bar**: xxHash64 idéntico en 5+ seeds entre Neutron y vanilla; verificación byte-level del NBT.
**Piezas**: cubic splines · RNG bootstrap · BlendedNoise seed · aquifer · surface rules. **Riesgo**: medio-alto.

```text
=== PROMPT F2d — Copiar y pegar en ZCode ===

Eres el LEAD del proyecto Neutron. F2d: Paridad 1:1 byte-identical con vanilla.

## 📖 Contexto
- LEE `tools/vanilla-extract/PARAMETERS.md` — TODOS los parámetros exactos de vanilla extraídos del jar decompilado
- LEE `crates/neutron-worldgen/src/` — código actual
- LEE `tools/parity-check/src/main.rs` — herramienta de comparación
- LEE `runs/run-005.md` — último run con gaps identificados
- LEE `STATE.md` — estado actual

## 🎯 Bar (criterios de aprobación)
- [ ] xxHash64 idéntico en 5+ seeds entre Neutron y vanilla
- [ ] Cubic splines exactas (TerrainProvider con splines anidados)
- [ ] RNG bootstrap order: un solo Xoroshiro128, mismo orden que vanilla
- [ ] BlendedNoise seed: seed derivado del world seed como vanilla
- [ ] Aquifer: water pockets (sea level 63) + lava pockets (Y=-54)
- [ ] Surface rules: condition-based (hole, steep, water check) como vanilla
- [ ] NBT byte-level verification: comparación de bytes, no solo hashes

## 🏗️ Tareas (lanzar en paralelo)

### T1 — Cubic splines exactas
Implementar `CubicSpline` con los puntos de control de PARAMETERS.md §8:
- overworldOffset spline (continentalness → erosion → weirdness)
- overworldFactor spline
- overworldJaggedness spline
- Nested splines (buildErosionOffsetSpline, buildErosionFactorSpline, etc.)
Referencia: `TerrainProvider.class` decompilado en `tools/vanilla-extract/server-classes/`

### T2 — RNG bootstrap order + BlendedNoise seed
- Fix: crear TODOS los noises de un solo Xoroshiro128 en el orden de vanilla
- Fix: derivar BlendedNoise seed del world seed (investigar en `BlendedNoise.class`)
- Referencia: `NoiseData.bootstrap()` y `NoiseRouterData.bootstrap()` decompilados

### T3 — Aquifer system
Implementar aquifer básico:
- Water pockets at sea level (Y=63)
- Lava pockets at Y=-54
- Fluid picker: air below MIN_Y*2, lava at -54, water at sea level
Referencia: `NoiseBasedChunkGenerator.createFluidPicker()` decompilado

### T4 — Surface rules condition-based
Reemplazar layer-based con condition-based como vanilla:
- `SurfaceRules.hole()` — check for cave/carver holes
- `SurfaceRules.steep()` — check for steep terrain
- `waterBlockCheck(-1, 0)` — water proximity check
- Biome-specific rules (desert=sand, badlands=terracotta, etc.)
Referencia: `SurfaceRuleData.overworldLike()` decompilado

### T5 — Parity verification
Re-ejecutar parity check con todos los fixes:
```bash
cargo run -p parity-check -- --seed 12345 --radius 8
cargo run -p parity-check -- --seed 67890 --radius 8
cargo run -p parity-check -- --seed 42 --radius 8
```
Verificar xxHash64 match. Si no matchea, identificar el chunk específico y diagnosticar.

## 🔄 Gauntlet Loop
1. Cada tarea se construye contra el bar (xxHash64 idéntico)
2. Critic ciego verifica con evidencia real (parity check output)
3. FAIL → gap más grande → fix → repetir
4. PASS cuando todos los chunks coinciden

## ✅ Salida esperada
- `crates/neutron-worldgen/src/` actualizado con parámetros exactos
- `bench/results/parity-F2d.json` — resultados de paridad
- `runs/run-006.md` — evidencia completa
- STATE.md → "F2d: paridad 1:1"

=== FIN PROMPT F2d ===
```

---

### F3 — Simulación vanilla

**Objetivo**: bloques, fluidos, iluminación, redstone, spawns, survival.
**Bar**: suite dorada posicional 100% contra server vanilla real (bots); light arrays idénticos en 50 seeds; survival básica jugable por bot.
**Piezas**: iluminación · redstone A/B/C/D · fluidos · spawns · survival. **Riesgo**: **ALTO** (redstone posicional — el mayor reto técnico).

```text
=== PROMPT F3 — Copiar y pegar en ZCode ===

Eres el LEAD del proyecto Neutron. F3: Simulación vanilla — **EL MAYOR RETO TÉCNICO DEL PROYECTO**.

## 📖 PASO 1 — Leer contexto COMPLETO

ARCHITECTURE.md §7 (Simulación) — leer ABSOLUTAMENTE TODO. Contiene:
- ECS con bevy_ecs (solo el crate, no Bevy completo)
- Tick loop a 20 TPS con scheduler por regiones
- Redstone: update order exacto PP: W,E,N,S,D,U · NC: W,E,D,U,N,S
- Quasi-connectivity (solo Java — "works as intended")
- Wire post-1.21.2: left-first, cómputo de potencia antes de updates
- Fluidos: flow mechanics exactas
- Iluminación: arrays de luz en chunks, engine voxel octree estilo Starlight
- Spawns: reglas de mob spawning vanilla

Lee también: BENCHMARKS.md, STATE.md, runs/run-003.md, AGENTS.md.

## 📋 PASO 2 — Tracking de tareas

```json
[
  {"content": "T1 — Iluminación: voxel octree, sky/block light, 50 seeds xxHash64", "status": "in_progress", "priority": "high"},
  {"content": "T2 — Redstone A: wire, torches, levers, doors. Update order exacto", "status": "pending", "priority": "high"},
  {"content": "T3 — Fluidos: water, lava, bubble columns, waterlogging", "status": "pending", "priority": "high"},
  {"content": "T4 — Spawns: mob spawning rules, light levels, despawning", "status": "pending", "priority": "medium"},
  {"content": "T5 — Redstone B: comparators, repeaters, observers, hoppers, TNT (depende T2)", "status": "pending", "priority": "high"},
  {"content": "T6 — Redstone C: pistons, QC, block swapping (depende T5)", "status": "pending", "priority": "high"},
  {"content": "T7 — Golden suite posicional (depende T1+T3+T6)", "status": "pending", "priority": "high"},
  {"content": "T8 — Survival básica (depende T7)", "status": "pending", "priority": "medium"}
]
```

## 🏗️ PASO 3 — FASE A: Independientes en paralelo

Lanza T1, T2, T3, T4 simultáneamente:

```json
Agent(run_in_background=true, prompt="Implementa neutron-sim lighting engine estilo Starlight: 1) Voxel octree, 2) Sky light propagation, 3) Block light propagation, 4) Dirty flag updates, 5) Light arrays idénticos a vanilla. Verificación: xxHash64 contra vanilla en 50 seeds.")

Agent(run_in_background=true, prompt="Implementa Redstone A en neutron-sim: 1) WIRE update order PP(W,E,N,S,D,U) y NC(W,E,D,U,N,S), post-1.21.2 left-first, 2) TORCHES burn-out, 3) LEVERS toggle, 4) DOORS open/close, 5) BLOCK UPDATES notify neighbors.")

Agent(run_in_background=true, prompt="Implementa fluidos en neutron-sim: 1) WATER source/flow/current, 2) LAVA 8 levels, 3) BUBBLE COLUMNS, 4) WATERLOGGING, 5) Fluid tick scheduling.")

Agent(run_in_background=true, prompt="Implementa mob spawning en neutron-sim: 1) Spawning rules light < 7, 2) Mob categories, 3) Pack spawning, 4) Despawning 32-128, 5) Mob caps.")
```

## 🚧 PASO 4 — Gate humano: Redstone A

Pregunta al humano:
"Redstone A: ¿el update order es correcto? ¿wire post-1.21.2 funciona? Prueba con circuitos reales"

## 🏗️ PASO 5 — FASE B: Redstone B (depende de A)

Si gate approved:
```json
Agent(run_in_background=true, prompt="Implementa Redstone B en neutron-sim, DEPENDE de Redstone A: 1) COMPARATORS subtraction/container, 2) REPEATERS delay 1-4 ticks + lock mode, 3) OBSERVERS, 4) HOPPERS cooldown 8 ticks, 5) TNT prime/fuse/explosion.")
```

Gate humano: "Redstone B: ¿funciona correctamente con A? Prueba: item sorter, TNT duper"

## 🏗️ PASO 6 — FASE C: Redstone C (depende de B)

```json
Agent(run_in_background=true, prompt="Implementa Redstone C (pistons) en neutron-sim, DEPENDE de Redstone B: 1) PISTONS extend/retract push limit 12, 2) STICKY PISTONS pull back, 3) QUASI-CONNECTIVITY (QC) Java-only BUD, 4) BLOCK SWAPPING, 5) BLOCK ENTITY MOVING preservar NBT. **Mecánica más compleja de Minecraft** — probar contra vanilla.")
```

Gate humano: "Redstone C (pistons + QC): ¿funciona QC exactamente como vanilla? ¿block swapping? Prueba: 2x2 piston door, flying machine, QC piston extender"

## 🏗️ PASO 7 — FASE D: Golden suite + Survival

Después de gates:
```json
Agent(run_in_background=true, prompt="Implementa golden suite posicional en bench/: 1) Script Vanilla vs Neutron misma seed, 2) Bots construyen contraptions idénticas, 3) Compara posición por posición, 4) Reporte JSON, 5) Contraptions: RSNOR, T-flip-flop, piston extender, item sorter, 2x2 piston door")

Agent(run_in_background=true, prompt="Implementa survival básica en neutron-sim: 1) HUNGER, 2) HEALTH, 3) DAMAGE invincibility frames, 4) CRAFTING 2x2/3x3, 5) INVENTORY, 6) EXPERIENCE.")
```

## 🔄 PASO 8 — Critic ciego (por cada sub-sistema)

Lanza subagente reviewer para cada componente. Evidencia real (logs, checksums). REJECT por defecto. Un solo gap por iteración.

## 🎯 Bar

- [ ] Suite dorada posicional 100% contra vanilla
- [ ] Light arrays xxHash64 idénticos en 50 seeds
- [ ] Survival básica jugable por bot
- [ ] Redstone: PP y NC update order exactos, QC como vanilla

## ✅ Salida

- `crates/neutron-sim/` con todos los subsistemas
- `bench/results/golden-f3.json`
- `runs/run-004.md`
- STATE.md → "F3: simulación vanilla"

=== FIN PROMPT F3 ===
```

## 🎯 PASO 3 — Bar con checkboxes detallados

El critic debe verificar CADA UNO de estos:

**Iluminación:**
- [ ] Light arrays en chunks: xxHash64 idéntico en 50 seeds
- [ ] Sky light propagation correcta (bloques translúcidos, altura)
- [ ] Block light propagation desde fuentes
- [ ] Dirty flag updates (un bloque cambia → luz se re-propaga)

**Redstone A:**
- [ ] Wire update order: PP (W,E,N,S,D,U) exacto
- [ ] Wire update order: NC (W,E,D,U,N,S) exacto
- [ ] Post-1.21.2 left-first wire prioritization
- [ ] Torch burn-out (8 cambios en 60 ticks)
- [ ] Lever toggle correcto
- [ ] Door open/close con redstone

**Redstone B:**
- [ ] Comparator: subtraction mode funciona
- [ ] Comparator: container-based (item count en chest = señal)
- [ ] Repeater: delay 1-4 ticks exactos
- [ ] Repeater: lock mode funciona
- [ ] Observer: block update detection genera pulse
- [ ] Hopper: item transfer con cooldown 8 ticks
- [ ] TNT: prime, fuse, explosion (daño a entidades + bloques)

**Redstone C:**
- [ ] Piston: extend/retract, push limit 12
- [ ] Sticky piston: pull back correcto
- [ ] QUASI-CONNECTIVITY: funciona exactamente como vanilla Java (BUD)
- [ ] Block swapping: sticky piston intercambia bloques correctamente
- [ ] Block entities movidos: chests/furnaces preservan NBT

**Fluidos:**
- [ ] Water flow: source, spread 1-7, current empuja entidades
- [ ] Lava flow: 8 niveles, source blocks
- [ ] Bubble columns: magma (down), soul sand (up)
- [ ] Waterlogging: bloques con agua dentro

**Spawns:**
- [ ] Hostile spawn: light level < 7
- [ ] Biome-specific mobs correctos
- [ ] Despawn mechanics (32-128 blocks)
- [ ] Mob caps respetados

**Golden Suite:**
- [ ] Suite dorada posicional 100% contra vanilla
- [ ] Redstone contraptions: RSNOR, T-flip-flop, piston extender, item sorter
- [ ] Reporte JSON con evidencias

**Survival:**
- [ ] Hunger: food bar, saturation, starvation damage
- [ ] Health: damage types, invincibility frames, armor
- [ ] Crafting: 2x2 y 3x3 con recipes de datapack vanilla
- [ ] Inventory: hotbar, armor, offhand, drops
- [ ] Experience: XP orbs, enchanting

## 🔄 PASO 4 — CRITIC ciego (para CADA sub-sistema)

Por cada worker, lanza un subagente **reviewer** con contexto limpio:

```text
Eres el CRITIC ciego. Vas a inspeccionar el artefacto REAL de:
[sub-sistema: iluminación / redstone-A / redstone-B / redstone-C / fluidos / spawns / golden-suite / survival]

PROCEDIMIENTO:
1. Ejecuta `cargo check` y `cargo test`
2. Si hay tests de paridad contra vanilla, ejecútalos
3. Si no hay tests, ejecuta el código manualmente:
   - Para redstone: construye un circuito, compara con vanilla real
   - Para iluminación: compara light arrays byte por byte
   - Para fluidos: fluye agua, compara con vanilla
4. Verifica EACH criterio del bar con evidencia (logs, checksums, screenshots)
5. Verdict: PASS solo si TODOS los criterios pasan con evidencia
6. FAIL → nombrar EXACTAMENTE una cosa, la más importante a corregir

DEFAULT = REJECT. No confíes en el reporte del builder.
```

Usa MCP tools para buscar referencias durante la crítica si algo no está claro.
Usa Computer Use si necesitas ver el comportamiento de vanilla directamente.

## ✅ PASO 5 — Salida esperada

Al final de F3:
- `crates/neutron-sim/` con todos los subsistemas:
  - `light/` — iluminación (voxel octree)
  - `redstone/` — redstone A+B+C
  - `fluids/` — water + lava + columns
  - `spawns/` — mob spawning
  - `survival/` — hunger, health, crafting, inventory
- `bench/results/golden-f3.json` — suite dorada posicional 100%
- `runs/run-004.md` — evidencia completa
- STATE.md → "F3: simulación vanilla"

Actualiza STATE.md y lanza el prompt de F4 (paralelo F3) o siguiente.

=== FIN PROMPT F3 ===
```

---

### F4 — Escala 500-1000+

**Objetivo**: 500 jugadores estables; camino a 1000+.
**Bar**: 500 bots 60 min → TPS 20.0, p99 tick < 25 ms; RAM/jugador < 1 MB sobre base < 150 MB; sin regresión de cps/startup.
**Piezas**: scheduler por regiones (fan-out de 3 agentes A/B) · hot path optimizations · stress 500 bots · memory profiling. **Riesgo**: medio-alto.

```text
=== PROMPT F4 — Copiar y pegar en ZCode ===

Eres el LEAD del proyecto Neutron. F4: Escala 500-1000+.

## 📖 Contexto
ARCHITECTURE.md §7 (ECS + scheduler por regiones) y §4 (Escala). Leer BENCHMARKS.md §7 (targets).

## 📋 Tracking
```json
[
  {"content": "T1 — Scheduler por regiones (Folia-style)", "status": "in_progress", "priority": "high"},
  {"content": "T2 — Hot path optimizations (arenas, lock-free)", "status": "pending", "priority": "high"},
  {"content": "T3 — Stress test 500 bots", "status": "pending", "priority": "high"},
  {"content": "T4 — Memory profiling (dhat/heaptrack)", "status": "pending", "priority": "medium"}
]
```

## 🏗️ Ejecución
T1 y T4 en paralelo. T2 después de T1. T3 después de T2.

```json
Agent(run_in_background=true, prompt="Implementa scheduler por regiones estilo Folia: fan-out de regiones a threads, lock-free con arenas. Referencia: ARCHITECTURE.md §7")
Agent(run_in_background=true, prompt="Implementa memory profiling: dhat/heaptrack para Neutron, mide RSS por jugador")
```

Gate → T2 (optimizations) → T3 (stress test 500 bots 60 min)

## 🎯 Bar
- [ ] 500 bots × 60 min → TPS 20.0, p99 tick < 25 ms
- [ ] RAM/jugador < 1 MB sobre base < 150 MB
- [ ] Sin regresión de cps/startup vs F3

## 🔄 Critic
Ejecuta stress test 500 bots, mide TPS cada 5s, reporta p99. Memory profile con dhat.

## ✅ Salida
- `crates/neutron-sim/scheduler/` funcional
- `bench/results/stress-f4.json`
- STATE.md → "F4: escala 500-1000+"

=== FIN PROMPT F4 ===
```

---

### F5 — Mobs y AI

**Objetivo**: comportamiento vanilla de mobs y combate completo.
**Bar**: E2E 20 min de survival; spot-checks (creeper explota, zombie quema al sol, enderman TP, dragon fight); 50 mobs/chunk sin regresión TPS.
**Piezas**: pasivos + trading · hostiles · jefes · combate · pathfinding A*. **Riesgo**: **ALTO**.

```text
=== PROMPT F5 — Copiar y pegar en ZCode ===

Eres el LEAD del proyecto Neutron. F5: Mobs y AI.

## 📖 Contexto
ARCHITECTURE.md §7 — ECS con bevy_ecs, entidades, componentes, systems (movimiento, AI, combate).
Lee: STATE.md, runs/run-005.md.

## 📋 Tracking
```json
[
  {"content": "T1 — Pathfinding A* engine", "status": "in_progress", "priority": "high"},
  {"content": "T2 — Mobs pasivos + trading", "status": "pending", "priority": "high"},
  {"content": "T3 — Mobs hostiles", "status": "pending", "priority": "high"},
  {"content": "T4 — Combate", "status": "pending", "priority": "high"},
  {"content": "T5 — Jefes: Ender Dragon + Wither", "status": "pending", "priority": "high"},
  {"content": "T6 — E2E survival 20 min", "status": "pending", "priority": "high"}
]
```

## 🏗️ Ejecución

**FASE 1 — AI engine (base de todo):**
```json
Agent(run_in_background=true, prompt="Implementa pathfinding A*: 1) Goal-oriented, 2) A* manhattan, 3) Path smoothing, 4) Obstacle avoidance, 5) Recalculation rate 5-20 ticks, 6) 50 mobs/chunk sin TPS drop")
```
Gate humano: "¿El pathfinding A* es correcto? 50 mobs en chunk, TPS sin drop"

**FASE 2 — Pasivos + hostiles + combate en paralelo:**
```json
Agent(run_in_background=true, prompt="Mobs pasivos: COW, PIG, SHEEP, CHICKEN, HORSE, VILLAGER con trading UI, breeding, professions")
Agent(run_in_background=true, prompt="Mobs hostiles: ZOMBIE (burn/reinforcements), SKELETON (strafe/arrows), SPIDER (climb/neutral sun), CREEPER (hiss/explode), SLIME (split), PHANTOM, ENDERMAN (TP)")
Agent(run_in_background=true, prompt="Combate: MELEE cooldown + sweep, BOW gravity, SHIELD, TRIDENT, ENCHANTMENTS sharpness/protection/fire_aspect")
```
Gate: "¿Creeper explota? ¿Zombie quema al sol? ¿Enderman se teleporta?"

**FASE 3 — Bosses:**
```json
Agent(run_in_background=true, prompt="Jefes: ENDER DRAGON (crystals, perch, breath, portal, egg) + WITHER (spawn, half-health rage, blue skulls)")
```

**FASE 4 — E2E:**
```json
Agent(run_in_background=true, prompt="E2E survival 20 min: bot azalea (Rust 26.x) que spawnea, se mueve, mata animales, craftea, come, sobrevive 20 min. Spot-checks: creeper, zombie, enderman. Reporte en bench/results/e2e-f5.json")
```

## 🎯 Bar
- [ ] E2E 20 min de survival sin crashes
- [ ] Spot-checks: creeper explota, zombie quema, enderman TP, dragon fight, wither rage
- [ ] 50 mobs/chunk sin regresión de TPS

## 🔄 Critic
Lanza reviewer para cada componente. Evidencia real.

## ✅ Salida
- Pathfinding A* funcional
- Mob behavior verificado contra vanilla
- `bench/results/e2e-f5.json`
- STATE.md → "F5: mobs y AI"

=== FIN PROMPT F5 ===
```

---

### F6 — Plugins WASM + Lua

**Objetivo**: ecosistema seguro por construcción.
**Bar**: WASM panic no tumba servidor; fuel 10M opcodes mata plugin sin daño; hot reload; 3 conversiones Bukkit; coste < 5 µs/tick.
**Piezas**: wasmtime + WIT · API · Lua (mlua) · converter · PatchBukkit · docs. **Riesgo**: alto.

```text
=== PROMPT F6 — Copiar y pegar en ZCode ===

Eres el LEAD del proyecto Neutron. F6: Plugins WASM + Lua.

## 📖 Contexto
ARCHITECTURE.md §9 (Plugins) + §1 (Seguridad por construcción).
Lee: STATE.md, runs/run-006.md.

## 📋 Tracking
```json
[
  {"content": "T1 — Runtime WASM (wasmtime + WIT + sandbox + hot reload)", "status": "in_progress", "priority": "high"},
  {"content": "T2 — Lua scripting (mlua + hooks + safety)", "status": "pending", "priority": "high"},
  {"content": "T3 — Convertidor Bukkit→WASM", "status": "pending", "priority": "medium"},
  {"content": "T4 — PatchBukkit v0", "status": "pending", "priority": "medium"},
  {"content": "T5 — Docs plugins", "status": "pending", "priority": "medium"}
]
```

## 🏗️ Ejecución

**FASE 1 — WASM + Lua en paralelo:**
```json
Agent(run_in_background=true, prompt="Implementa runtime WASM en neutron-plugin: 1) wasmtime + WIT, 2) Sandbox fuel 10M opcodes + memory limit, 3) Plugin API init/tick/event/command, 4) Host API set_block/get_block/send_message, 5) Panic → kill plugin only, 6) Hot reload .wasm")
Agent(run_in_background=true, prompt="Implementa Lua scripting en neutron-plugin: 1) mlua engine, 2) Hook before/after tick, block/player events, 3) Coroutine yield timeout, 4) Same host API que WASM")
```

**FASE 2 — Converter + Docs (después de WASM):**
```json
Agent(run_in_background=true, prompt="Implementa convertidor Bukkit→WASM en tools/: parse plugin.yml, transform listeners → WIT functions, generate .wasm stub. 3 ejemplos: EssentialsX motd, SimpleVote, WorldEdit wand")
Agent(run_in_background=true, prompt="Implementa PatchBukkit v0: event system, command system, scheduler parcial → WIT host calls")
Agent(run_in_background=true, prompt="Escribe docs de plugins en docs/plugins/: tutorial WASM, tutorial Lua, API reference, converting Bukkit, security guide")
```

## 🎯 Bar (SEGURIDAD es lo más importante)
- [ ] Plugin WASM panic → servidor sigue, solo plugin muere
- [ ] Fuel 10M opcodes → plugin termina, servidor intacto
- [ ] Memory limit → plugin muere, servidor intacto
- [ ] Hot reload: cambiar .wasm sin restart
- [ ] 3 plugins Bukkit convertidos reales
- [ ] Coste hot path < 5 µs/tick

## 🔄 Critic
- Sandbox: plugin panic → servidor sobrevive ✓
- Fuel: loop infinito → fuel termina plugin ✓
- Hot reload: cambia .wasm → recarga sin restart ✓
- Performance: criterion < 5 µs/tick ✓

## ✅ Salida
- `crates/neutron-plugin/` — WASM + Lua
- `tools/patch-bukkit/` — convertidor
- 3 plugins convertidos
- `docs/plugins/`
- STATE.md → "F6: plugins WASM + Lua"

=== FIN PROMPT F6 ===
```

---

### F7 — Bedrock

**Objetivo**: clientes Bedrock 26.x en el mismo mundo.
**Bar**: cliente Bedrock real juega 10 min; coexistencia Java+Bedrock; TPS Java sin impacto.
**Piezas**: RakNet + login/play · play básico · mapeo Java↔Bedrock · coexistencia. **Riesgo**: medio.

```text
=== PROMPT F7 — Copiar y pegar en ZCode ===

Eres el LEAD del proyecto Neutron. F7: Bedrock.

## 📖 Contexto
ARCHITECTURE.md §3 (Protocolo) — Bedrock: capa de sesión independiente, RakNet.
Lee: STATE.md, runs/run-007.md.

## 📋 Tracking
```json
[
  {"content": "T1 — RakNet + login/play", "status": "in_progress", "priority": "high"},
  {"content": "T2 — Registry mapping Java↔Bedrock", "status": "pending", "priority": "high"},
  {"content": "T3 — Play básico (movimiento, chat, bloques)", "status": "pending", "priority": "high"},
  {"content": "T4 — Coexistencia Java+Bedrock", "status": "pending", "priority": "high"}
]
```

## 🏗️ Ejecución (chain: cada tarea depende de la anterior)

```json
// T1 — RakNet
Agent(run_in_background=true, prompt="Implementa RakNet protocol en neutron-protocol-bedrock: open_connection, encapsulation, Login → PlayStatus, Play packets (level_chunk, move_player, inventory_content, chat, set_time)")

// Gate: "¿Conexión RakNet funciona? Prueba con Minecraft Bedrock real"
```

Si gate approved → T2 → T3 → T4 en cadena:
```json
Agent(prompt="Implementa registry mapping Java↔Bedrock: block ids, item ids, biome ids")
Agent(prompt="Implementa Play básico: movimiento, chat, bloques, inventario")
Agent(prompt="Implementa coexistencia Java+Bedrock: mismo mundo, mismo tick, TPS sin impacto")
```

## 🎯 Bar
- [ ] Cliente Bedrock real juega 10 min sin crashes
- [ ] Coexistencia: Java + Bedrock en el mismo mundo
- [ ] TPS Java sin impacto con Bedrock conectado

## 🔄 Critic
- Lanza cliente Bedrock real, verifica login/spawn/movimiento
- Conecta Java + Bedrock al mismo servidor, verifica mismo mundo
- Mide TPS con ambos conectados

## ✅ Salida
- `crates/neutron-protocol-bedrock/` funcional
- Cliente Bedrock real conectado (screenshot + logs)
- `bench/results/coexist-f7.json`
- STATE.md → "F7: Bedrock"

=== FIN PROMPT F7 ===
```

---

### F8 — 1.0

**Objetivo**: release estable, verificable y defendible.
**Bar**: parity suite completa verde en `main`; benchmarks reproducibles en 2 máquinas; 72 h uptime 100 jugadores; fuzz 24 h limpio; binarios x86-64/ARM64 (Win/Linux/Mac).
**Piezas**: fuzz + audits · benchmarks finales · docs + migración · proceso de release. **Riesgo**: medio.

```text
=== PROMPT F8 — Copiar y pegar en ZCode ===

Eres el LEAD del proyecto Neutron. F8: Release 1.0.

## 📖 Contexto
Lee TODO: STATE.md, runs/run-008.md, ROADMAP.md completo.
Asegúrate de que TODAS las fases F0-F7 están COMPLETADAS con STATE.md actualizado.

## 📋 Tracking
```json
[
  {"content": "T1 — Fuzz 24h + Security audit", "status": "in_progress", "priority": "high"},
  {"content": "T2 — Benchmarks finales (2 máquinas)", "status": "pending", "priority": "high"},
  {"content": "T3 — Uptime 72h con 100 jugadores", "status": "pending", "priority": "high"},
  {"content": "T4 — Binarios multiplataforma (x86-64/ARM64)", "status": "pending", "priority": "high"},
  {"content": "T5 — Docs + Migration guide + Changelog", "status": "pending", "priority": "medium"}
]
```

## 🏗️ Ejecución (TODO en paralelo)

```json
Agent(run_in_background=true, prompt="Fuzz 24h: cargo-fuzz decode 24h, cargo audit, review unsafe code, fuzzing del protocolo. Reporte en bench/results/fuzz-f8.json")
Agent(run_in_background=true, prompt="Benchmarks finales: reproducibles en 2 máquinas (Windows + Linux), same seed, same methodology. bench/results/final-f8.json")
Agent(run_in_background=true, prompt="Uptime test: 72h con 100 jugadores bots, medir memory leak, TPS sostenido. Reporte en bench/results/uptime-f8.json")
Agent(run_in_background=true, prompt="Binarios: x86-64/ARM64 para Windows (msi), Linux (deb/rpm), macOS (dmg). GitHub release workflow.")
Agent(run_in_background=true, prompt="Docs: README.md final, migration guide vanilla→Neutron, deployment guide, API reference, CHANGELOG.md")
```

## 🎯 Bar
- [ ] Parity suite 100% verde en `main`
- [ ] Benchmarks reproducibles en 2 máquinas
- [ ] 72h uptime, 100 jugadores, 0 memory leaks
- [ ] Fuzz 24h — 0 panics, 0 crashes
- [ ] Binarios: x86-64 + ARM64 para Win/Linux/Mac
- [ ] Docs completas

## 🔄 Critic
- Security audit: 0 unsafe en hot paths, fuel/memory limits, CVE check
- Benchmarks: reproducibilidad verificada
- Binarios: descargar, ejecutar, conectar cliente
- Docs: completas, sin typos, ejemplos funcionan

## ✅ Gate humano FINAL

Pregunta al humano:
"RELEASE GATE: ¿Estás seguro de que Neutron 1.0 está listo?
- Parity suite 100% verde?
- Benchmarks dentro de targets? (< 2s startup, > 250 cps, < 150MB RAM)
- 72h uptime sin leaks?
- Fuzz 24h limpio?
- Binarios en releases?
- Docs completas?"

Si APPROVED:
```bash
git tag v1.0.0
git push origin v1.0.0
```

## ✅ Salida
- `main` con parity suite 100% verde
- Binarios en GitHub Releases (v1.0.0)
- `docs/` completas
- STATE.md → "F8: RELEASED v1.0.0"
- CHANGELOG.md

=== FIN PROMPT F8 ===
```

---

## 3. Timeline consolidado

| Fase | Rondas est. | Puede ir en paralelo con |
|---|---|---|
| F0 | 3-5 | — |
| F1 | 5-8 | — |
| F2 | 8-12 | — |
| F3 | 10-16 | F4 |
| F4 | 6-10 | F3 |
| F5 | 10-16 | F6, F7 |
| F6 | 8-12 | F5 |
| F7 | 6-10 | F5 |
| F8 | 6-10 | — |

Recalibrar tras cada fase según la velocidad real.

## 4. Pipeline de versiones D0-D4 (SLA: `main` ≤ 7 días tras release de Mojang)

| Día | Paso | Herramienta | Verificación |
|---|---|---|---|
| D0 | Detectar release de Mojang | webhook/CI | — |
| D1 | Extraer jar: registries, protocolo, worldgen, assets | `tools/mc-extract` | diff vs anterior; minecraft-data |
| D2 | Codegen → Rust tipado | `tools/codegen` | `cargo check` limpio |
| D3 | Regenerar golden data (chunks por seed, contraptions) | harness | checksums xxHash64 |
| D4 | Parity suite + benchmarks + release `main` | CI + gate humano | parity 100% |

> **Importante**: El pipeline D0-D4 se implementa en F1 y se refina en cada fase subsecuente. Se lanza con su propio prompt (similar a los de las fases) cuando Mojang publique una nueva versión.

**Prompt rápido para actualización de versión:**
```text
Eres el LEAD. Pipeline D0-D4 para Minecraft <NUEVA_VERSION>. Sigue §4 del ROADMAP.md.
Herramientas: Agent, TodoWrite, Bash.
- D1: tools/mc-extract del jar nuevo
- D2: tools/codegen regenera Rust
- D3: golden data pipeline regenera hashes
- D4: parity suite + benchmarks + PR a main
Gate humano para el merge a main.
```

## 5. Riesgos y mitigaciones

| Riesgo | Severidad | Mitigación |
|---|---|---|
| Paridad de redstone (posicional, QC, 1.21.2+) | CRÍTICO | Suite dorada posicional desde F3-A; contra server real |
| Paridad de mob AI | ALTO | Port desde jar sin ofuscar; spot-checks automatizados |
| Expectativas de compat Bukkit | ALTO | Estrategia por capas honesta (F6); comunicación |
| Escala 1000+ | MEDIO-ALTO | Scheduler regional A/B (F4); stress continuo |
| Cadencia de Mojang | MEDIO | Pipeline D0-D4; tests de regresión |
| Scope creep | ALTO | Bar por fase; backlog separado |
| Coste de agentes (tokens) | MEDIO | Presupuestos guardrail; kill-switch; STATE.md |

## 6. Herramientas ZCode referencia

| Herramienta | Cuándo usarla |
|---|---|
| **Agent (subagente builder)** | Construir piezas en paralelo (`run_in_background: true`) |
| **Agent (subagente critic)** | CRITIC ciego con contexto limpio en cada ronda |
| **Agent (subagente Explore)** | Solo lectura: búsqueda de archivos, código, docs |
| **TodoWrite** | Tracking de tareas con dependencias y estado |
| **AskUserQuestion** | Gates humanos: aprobaciones, decisiones |
| **Bash** | Comandos: cargo build/test, java, git, etc. |
| **Read/Write/Edit** | Manipulación de archivos del proyecto |
| **TaskOutput** | Esperar resultado de subagentos async |
| **CronCreate** | Loops de automatización (smoke E2E, benchmarks) |
| **MCP tools** | Investigación: crates.io, docs de Minecraft, etc. |

## 7. Fuera de alcance

Combat 1.8 · mods Forge/Fabric · plugins Bukkit 100% (solo por capas) · minigames custom · FPS de cliente (ver BENCHMARKS.md)
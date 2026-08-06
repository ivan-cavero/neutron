# Neutron — Roadmap

> El progreso se mide en **BARS y RONDAS**, no en calendario. Un bar es una referencia real e innegociable (checksum, benchmark, server real) que un critic ciego inspecciona (Gauntlet Loop, ver AGENTS.md §2).

## 0. Cómo leer este roadmap

- **Bar**: lo que el critic compara contra nuestro artefacto (el "Call of Duty" de cada fase). No se discute: se cumple o no.
- **Rondas**: ciclos build → critic → fix. Sin cap arbitrario: se itera hasta que el bar gana, 2 rondas sin mejora, o presupuesto agotado.
- **Prompt**: cada fase tiene un prompt completo listo para copiar y pegar en pi. Cada prompt detalla:
  - **Skills a cargar** (gauntlet-loop, loop-engineering, orca-cli, orchestration, mcp-scripting)
  - **MCP tools a usar** (para investigación, búsqueda en internet/crates.io/minecraft docs)
  - **Orca CLI commands** (worktrees, terminales)
  - **Orca Orchestration** (task DAGs, worker_done, decision gates)
  - **Computer Use** (cuando se necesite interactuar con apps de escritorio)
- **Cómo lanzar un run**: copia el `=== PROMPT F<NNN> ===` de la fase actual y pégalo en pi. Pi leerá este ROADMAP.md automáticamente y ejecutará todo.

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

Cada bloque `=== PROMPT F<NNN> ===` es un prompt completo que puedes copiar y pegar en pi. Pi automáticamente:
1. Carga las skills necesarias (gauntlet-loop, loop-engineering, orca-cli, orchestration, mcp-scripting)
2. Lee el contexto del proyecto (STATE.md, runs/, etc.)
3. Usa MCP tools para investigar referencias técnicas
4. Usa Orca CLI para crear worktrees aislados
5. Usa Orca Orchestration para coordinar workers con DAGs
6. Ejecuta el Gauntlet Loop (builder → critic → fix → repetir)
7. Actualiza STATE.md y runs/run-NNN.md

---

### F0 — Fundamentos y harness

**Objetivo**: infraestructura del repo + primer baseline público.
**Bar**: un agente distinto al builder ejecuta `bench/run.ps1` desde cero en Windows y Linux y reproduce el baseline B0 (vanilla 26.2 / Paper / Pumpkin); 10 bots de join simultáneo sin kicks (p95 < 5 s); cps ±30% consistente con baselines publicados.
**Piezas**: harness + bots · CI/workspace · baseline B0 publicado. **Riesgo**: bajo.
**Estado actual**: Round 3 en curso (U1 PASS, U2 PASS, U3 in progress). Ver `workbench.md`.

```text
=== PROMPT F0 — Copiar y pegar en pi ===

Eres el LEAD del proyecto Neutron. Lanza el Gauntlet Loop para la fase F0.

## ⚙️ PASO 0 — Cargar skills y MCP tools

Carga las siguientes skills para tener todas las herramientas disponibles:

1. **gauntlet-loop**: para el loop build → critic → fix contra el bar
   - Lee los references/ del skill para prompt templates de LEAD/BUILDER/CRITIC
   - Lee references/domains/coding.md para el checklist de código
2. **loop-engineering**: para budgets, kill-switch, durable state
   - Lee state.md y budget.md references
3. **orca-cli**: para crear worktrees y gestionar terminales
   - Ejecuta `orca skills get orca-cli` para guía completa
4. **orchestration**: para task DAGs, worker_done, decision gates
   - Ejecuta `orca skills get orchestration` para guía completa
5. **mcp-scripting**: para hacer búsquedas con MCP tools
   - Úsalo para: buscar en crates.io, buscar en docs de Minecraft, buscar baselines publicados

Usa también MCP tools directamente:
- `mcp({ search: "minecraft 26.2 server download" })` — buscar URLs de descarga
- `mcp({ search: "paper 26.2 latest build" })` — buscar última build de Paper
- `mcp({ search: "pumpkin mc release latest" })` — buscar última release de Pumpkin
- `mcp({ search: "mineflayer example join multiple bots" })` — buscar ejemplos de bots
- `mcp({ search: "azalea rust minecraft bot crate" })` — buscar azalea en crates.io
- `mcp({ search: "cargo criterion benchmark example" })` — buscar ejemplos de benchmarks Rust
- `mcp({ describe: "xcodebuild_list_sims" })` — si necesitas interactuar con Xcode

Usa **Computer Use** (skill computer-use) si necesitas:
- Interactuar con la ventana de Orca
- Capturar pantalla de apps de escritorio
- Leer la UI de un servidor Minecraft levantado en una terminal

## 📖 PASO 1 — Leer contexto

Lee TODOS estos archivos antes de empezar:
- STATE.md → fase actual, último run, qué sigue
- AGENTS.md → reglas de trabajo, operating manual, formato de tarea
- ARCHITECTURE.md → cómo está diseñado el servidor
- BENCHMARKS.md → metodología de benchmarks, métricas exactas, baselines publicados
- workbench.md → progreso actual de F0 (Rounds anteriores, qué está PASS/FAIL)
- ROADMAP.md → (este archivo, leer la sección F0 completa)
- runs/README.md → formato de run
- README.md → contexto del proyecto

Usa MCP tools para INVESTIGAR referencias externas:
- Busca la última build de Paper 26.x (compatible con 26.2)
- Busca la última release de Pumpkin (binario oficial)
- Busca qué versión de Java 25 necesitas para Vanilla 26.2
- Busca ejemplos de mineflayer con conexión simultánea
- Busca el crate "azalea" para Rust bots en 26.x
- Busca baselines de cps publicados (C2ME benchmark methodology)

## 📝 PASO 2 — Crear estructura de runs

Crea la carpeta `runs/` si no existe y `runs/run-001.md` con:
- Objetivo: una frase clara y medible
- Bar: copiado del ROADMAP.md
- Tareas: 3-5 con AC, evidencia esperada y DoD (Definition of Done)
- Presupuesto orientativo (rondas, tokens aproximados)

Actualiza STATE.md para reflejar que F0 está en curso.

## 🏗️ PASO 3 — Orquestación con Orca

### 3a. Verificar que Orca está disponible
```bash
orca status --json
orca worktree ps --json
```

### 3b. Crear worktrees aislados (cada uno con su agente codex)

```bash
orca worktree create \
  --name f0-harness \
  --no-parent \
  --agent codex \
  --prompt "Construye bench/run.ps1 (PowerShell) y bench/run.sh (Bash) para Windows y Linux que: 1) levanta Vanilla 26.2 con Java 25, 2) ejecuta 10 bots de join, 3) mide cps, 4) produce JSON en bench/results/. Sigue la metodología de BENCHMARKS.md" \
  --setup run \
  --json

orca worktree create \
  --name f0-bots \
  --no-parent \
  --agent codex \
  --prompt "Construye bench/bots/join-bench/ con mineflayer (Node.js) que: 1) conecta 10 bots simultáneos a un servidor, 2) mide p50/p95/p99 de join time, 3) reporta JSON. Para versiones 1.20.2+ usa physicsEnabled:false hasta spawn. Para 26.2 usa azalea (Rust). Documenta en bench/bots/README.md" \
  --setup run \
  --json
```

Si `worktree create --agent` no funciona en tu versión de Orca:

```bash
# Crear worktree, luego terminal, luego enviar prompt
orca worktree create --name f0-harness --no-parent --setup run --json
ORCA_WT=$(orca worktree list --json | jq -r '.worktrees[] | select(.displayName=="f0-harness") | .id')
orca terminal create --worktree "id:${ORCA_WT}" --command "codex" --json
orca terminal send --text "Construye bench/run.ps1 ..." --enter
```

### 3c. Crear Run de Orquestación

```bash
orca orchestration run-create --objective "F0: harness + bots + baseline B0" --json
```

### 3d. Crear Tasks con dependencias

```bash
# T1: Harness (independiente)
T1=$(orca orchestration task-create \
  --spec "T1 - Harness de benchmarks: bench/run.ps1 + bench/run.sh que levanta Vanilla/Paper/Pumpkin, ejecuta tests de join y cps, produce JSON" \
  --deps '[]' \
  --json | jq -r '.id')

# T2: Bots (independiente)
T2=$(orca orchestration task-create \
  --spec "T2 - Bots multi-conexión: bench/bots/join-bench/ con 10 bots simultáneos, p50/p95/p99, mineflayer + azalea" \
  --deps '[]' \
  --json | jq -r '.id')

# T3: Docs del harness (depende de T1 y T2)
T3=$(orca orchestration task-create \
  --spec "T3 - bench/README.md: setup, ejecución, interpretación de resultados, referencias de baselines" \
  --deps "[\"$T1\",\"$T2\"]" \
  --json | jq -r '.id')

# T4: Baseline B0 (depende de T1, T2, T3)
T4=$(orca orchestration task-create \
  --spec "T4 - Baseline B0: ejecutar harness en Windows y Linux, publicar bench/results/baseline-B0.json con timestamps y hashes" \
  --deps "[\"$T1\",\"$T2\",\"$T3\"]" \
  --json | jq -r '.id')
```

### 3e. Lanzar workers

```bash
# T1 y T2 en paralelo
orca orchestration worker-start --task "$T1" --worktree new-child --name f0-harness-worker --agent codex --json
orca orchestration worker-start --task "$T2" --worktree new-child --name f0-bots-worker --agent codex --json
```

### 3f. Esperar resultados

```bash
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 900000 --json
```

Cuando recibas worker_done, procesa:
- Si PASS → revisa, haz worker-release, y lanza el siguiente worker en la DAG
- Si FAIL → registra findings, corrige, re-despacha
- Si question/decision_gate → responde o escalas al humano

## 🔄 PASO 4 — Gauntlet Loop

### 4a. Split (ya está hecho en el PASO 3: T1, T2, T3, T4)

### 4b. BUILDERS
Cada worker (codex) construye su artefacto real:
- Harness → `bench/run.ps1`, `bench/run.sh`
- Bots → `bench/bots/join-bench/`
- Docs → `bench/README.md`
- Baseline → `bench/results/baseline-B0.json`

Los builders NO se autoevalúan. NO modifican el bar.

### 4c. CRITIC (subagente con contexto limpio)

Para CADA unidad, lanza un subagente critic:
```text
Usa el subagente con agent="reviewer" (o agent="debugger" según toque) con contexto limpio.

El critic debe:
1. INSPECCIONAR el artefacto REAL — ejecutar el script, correr los tests, leer los logs
2. USAR MCP tools para verificar referencias (hashes, checksums)
3. USAR Computer Use si necesita ver la UI de una app
4. COMPARAR contra el bar usando evidencia objetiva
5. DEVOLVER PASS (con evidencia de cada criterio) o FAIL (con el gap más grande + fix concreto)
6. Si FAIL → exactamente UNA cosa, la más importante
```

**Reglas del critic**:
- Default = REJECT hasta tener evidencia
- NO confiar en el resumen del builder — inspeccionar el artefacto real
- Para código: ejecutar `cargo check`, `cargo test`, revisar logs de ejecución
- Para benchmarks: verificar que los números existen, tienen timestamps, son reproducibles
- NO cambiar el bar para que pase
- NO ignorar tests fallidos

### 4d. Loop
1. Si FAIL → el gap más grande → el builder corrige → repetir critic
2. Si PASS → siguiente tarea en la DAG
3. Si 2 rondas sin mejora → registrar en STATE.md y pasar a lo siguiente
4. Si presupuesto agotado → reportar y esperar al humano

### 4e. Gate humano
Antes de publicar baseline B0:
- Usa `orca orchestration gate-create` para pedir aprobación
- El humano revisa los números

```bash
orca orchestration gate-create \
  --task "$T4" \
  --question "¿Los resultados de baseline B0 son correctos y publicables? (min/max/per-bot JSON + markdown)" \
  --json
```

## 📊 PASO 5 — Seguimiento continuo

Después de CADA ronda:
1. Actualiza `workbench.md` con: ronda, verdict por unidad, evidencia, enlaces a artefactos
2. Actualiza el worktree principal:
   ```bash
   orca worktree set --worktree active --comment "F0 R<N>: <estado>"
   ```
3. Si una tarea bloquea a otra, actualiza el task status:
   ```bash
   orca orchestration task-update --id <task_id> --status blocked --json
   ```

## ✅ PASO 6 — Salida esperada

Al final de F0 debe existir:
- `bench/run.ps1` — funcional en Windows
- `bench/run.sh` — funcional en Linux/WSL
- `bench/bots/join-bench/` — scripts de bots con mineflayer y azalea
- `bench/bots/README.md` — documentación de bots
- `bench/README.md` — documentación del harness
- `bench/results/baseline-B0.json` — baseline público con timestamps y hashes
- `runs/run-001.md` — evidencia completa del run
- `STATE.md` — actualizado a "F0: baseline B0 publicado"
- `workbench.md` — round log completo

Al terminar, actualiza STATE.md:
```
Fase actual: F0
Estado: COMPLETED — baseline B0 publicado
Siguiente: F1 — Núcleo jugable
```

Y lanza el prompt de F1 a continuación.

=== FIN PROMPT F0 ===
```

---

### F1 — Núcleo jugable

**Objetivo**: un jugador real entra, juega y el mundo persiste en Anvil vanilla.
**Bar**: bot vanilla 26.2 juega 10 min sin kick (E2E en CI); mundo guardado abre en vanilla y viceversa; fuzz del decode 1M inputs sin panic; startup < 2 s; RAM < 200 MB.
**Piezas**: protocolo 26.2 (login/play) · world v1 (Anvil, level.dat, carpetas vanilla) · pipeline de versiones v1 · E2E diario. **Riesgo**: medio.

```text
=== PROMPT F1 — Copiar y pegar en pi ===

Eres el LEAD del proyecto Neutron. Lanza el Gauntlet Loop para la fase F1: Núcleo jugable.

## ⚙️ PASO 0 — Cargar skills y MCP tools

Carga estas skills (lee los archivos referenced dentro de cada skill):

1. **gauntlet-loop** → references/domains/coding.md, references/prompt-templates.md, references/running-the-loop.md
2. **loop-engineering** → state.md, budget.md
3. **orca-cli** → `orca skills get orca-cli`
4. **orchestration** → `orca skills get orchestration`
5. **mcp-scripting** → para investigar con MCP

Usa MCP tools para investigación:
- `mcp({ search: "minecraft 26.2 protocol packets login play" })` — investigar paquetes del protocolo
- `mcp({ search: "wiki.vg protocol 26.2" })` — buscar documentación del protocolo
- `mcp({ search: "cargo fuzz github tutorial" })` — cómo hacer fuzzing en Rust
- `mcp({ search: "minecraft anvil file format specification" })` — formato .mca
- `mcp({ search: "level.dat nbt structure minecraft" })` — estructura de level.dat
- `mcp({ search: "azalea crate protocol minecraft 26" })` — buscar crate azalea en crates.io
- `mcp({ search: "tokio bytes crate rust networking" })` — para networking
- `mcp({ search: "cargo criterion benchmark startup time" })` — para medir startup

## 📖 PASO 1 — Leer contexto

Lee TODOS estos archivos:
- STATE.md, AGENTS.md, ARCHITECTURE.md (lee completo, especialmente §3 Protocolo y §5 Mundo)
- BENCHMARKS.md, runs/run-001.md, ROADMAP.md

## 🏗️ PASO 2 — Orquestación con Orca

### 2a. Crear worktrees (un worktree por subsistema)

```bash
# Worktree para el protocolo (login + play)
orca worktree create --name f1-protocol --no-parent --agent codex --setup run \
  --prompt "Implementa neutron-protocol para Minecraft 26.2: packets de login (handshake, login_start, login_success, encryption_request/response, set_compression), play (keep_alive, join_game, server_data, chat, player_position, set_default_spawn, chunk_data, block_update, synchronize_player_position). Usa tokio + bytes. Código generado, no a mano. Referencias: ARCHITECTURE.md §3, wiki.vg/Protocol" \
  --json

# Worktree para el mundo (Anvil + level.dat + carpetas vanilla)
orca worktree create --name f1-world --no-parent --agent codex --setup run \
  --prompt "Implementa neutron-world para Minecraft 26.2: lectura/escritura de Anvil .mca (region compuesta de chunks NBT), level.dat (NBT), estructura de carpetas world/world_nether/world_the_end/, session.lock, formato .hyp (zstd). Referencias: ARCHITECTURE.md §5, minecraft.wiki/Region_file_format" \
  --json

# Worktree para fuzzing
orca worktree create --name f1-fuzz --no-parent --agent codex --setup run \
  --prompt "Implementa fuzz del decode de paquetes de protocolo para neutron-protocol: usa cargo-fuzz con arbitrary, genera 1M inputs aleatorios, verifica cero panics. Reporta en bench/results/fuzz-v1.json" \
  --json

# Worktree para E2E
orca worktree create --name f1-e2e --no-parent --agent codex --setup run \
  --prompt "Implementa el test E2E: bot vanilla 26.2 se conecta a neutron-cli, juega 10 min (moverse, romper, poner bloques, chatear), el servidor no lo kick. Usa mineflayer o azalea. Mide startup time y RAM" \
  --json
```

### 2b. Crear Run de Orquestación con DAG

```bash
orca orchestration run-create --objective "F1: núcleo jugable — protocolo 26.2 + mundo Anvil + fuzz + E2E" --json

# Tasks con DEPENDENCIAS
T_PROTO=$(orca orchestration task-create \
  --spec "T1 - Protocolo 26.2: login/play packets (handshake, login, keep_alive, join_game, chat, position, spawn, chunk_data, block_update). Codec generado" \
  --deps '[]' --json | jq -r '.id')

T_WORLD=$(orca orchestration task-create \
  --spec "T2 - Mundo Anvil: .mca read/write, level.dat, carpetas vanilla, session.lock, formato .hyp" \
  --deps '[]' --json | jq -r '.id')

T_FUZZ=$(orca orchestration task-create \
  --spec "T3 - Fuzz del decode: cargo-fuzz, 1M inputs, 0 panics. Reporte JSON" \
  --deps "[\"$T_PROTO\"]" --json | jq -r '.id')

T_E2E=$(orca orchestration task-create \
  --spec "T4 - E2E: bot 26.2 conecta, juega 10 min sin kick, mide startup < 2s, RAM < 200MB" \
  --deps "[\"$T_PROTO\",\"$T_WORLD\"]" --json | jq -r '.id')
```

### 2c. Lanzar workers

```bash
# T1 y T2 en paralelo (independientes)
W1=$(orca orchestration worker-start --task "$T_PROTO" --worktree new-child --name f1-proto-worker --agent codex --json | jq -r '.effects.dispatch.dispatch_id')
W2=$(orca orchestration worker-start --task "$T_WORLD" --worktree new-child --name f1-world-worker --agent codex --json | jq -r '.effects.dispatch.dispatch_id')

# Esperar T1 y T2
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 1800000 --json

# Gate humano: revisar el protocolo
orca orchestration gate-create --task "$T_PROTO" \
  --question "¿El protocolo 26.2 es correcto? Revisa: login flow, packet IDs, chunk data format, compression. Ver ARCHITECTURE.md §3" \
  --json

# Si gate approved → T3 (fuzz) y T4 (E2E)
W3=$(orca orchestration worker-start --task "$T_FUZZ" --worktree new-child --name f1-fuzz-worker --agent codex --json)
W4=$(orca orchestration worker-start --task "$T_E2E" --worktree new-child --name f1-e2e-worker --agent codex --json)

# Esperar T3 y T4
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json
```

## 🔄 PASO 3 — Gauntlet Loop (por cada unidad)

### CRITIC ciego para PROTOCOLO
```text
Lanza subagente reviewer con contexto limpio.
Inspecciona: crates/neutron-protocol/
- ¿Los paquetes de login siguen el flujo wiki.vg?
- ¿El codec maneja correctamente compresión?
- ¿Los packet IDs son correctos para 26.2?
- ¿Maneja correctamente encryption_request/response?
- ¿El chunk data packet tiene el formato correcto (biomes, heightmaps, block entities)?
Ejecuta: cargo check, cargo test
Evidencia: logs de conexión de un bot real
```

### CRITIC ciego para MUNDO
```text
Inspecciona: crates/neutron-world/
- ¿Lee y escribe .mca correctamente? (verificar con vanilla)
- ¿level.dat tiene los campos correctos para 26.2?
- ¿La estructura de carpetas es idéntica a vanilla?
- ¿session.lock funciona?
- ¿El formato .hyp es válido?
Ejecuta: cargo check, cargo test
Evidencia: mundo escrito por Neutron abre en vanilla
```

### CRITIC ciego para FUZZ
```text
Inspecciona: bench/results/fuzz-v1.json
- ¿1M inputs ejecutados?
- ¿0 panics?
- ¿Los inputs cubren todos los tipos de paquetes?
Ejecuta: cargo fuzz run decode -- -runs=1000000
Evidencia: log de fuzz
```

### CRITIC ciego para E2E
```text
Inspecciona: logs del E2E, bench/results/startup-v1.json
- ¿Bot 26.2 conectó y se mantuvo 10 min?
- ¿Startup < 2s? (mediana de 5 runs)
- ¿RAM < 200MB? (RSS medido con Get-Process / ps)
- ¿El mundo persiste después de detener el servidor?
Ejecuta: el test E2E desde cero
Evidencia: logs con timestamps, métricas
```

## ✅ PASO 4 — Salida esperada

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
=== PROMPT F2 — Copiar y pegar en pi ===

Eres el LEAD del proyecto Neutron. F2: Worldgen paridad 1:1.

## ⚙️ PASO 0 — Skills + MCP

Skills: gauntlet-loop, loop-engineering, orca-cli, orchestration, mcp-scripting.

MCP tools para investigación:
- `mcp({ search: "minecraft noise parameters perlin octaves 3d" })`
- `mcp({ search: "xorososhiro128 rust crate" })`
- `mcp({ search: "minecraft density functions json datapack 1.21" })`
- `mcp({ search: "cubiomes minecraft worldgen verification" })`
- `mcp({ search: "xxhash64 rust crate" })`
- `mcp({ search: "minecraft surface rules json format" })`
- `mcp({ search: "minecraft carvers cave noise" })`
- `mcp({ search: "pumpkin mc worldgen parity PR 2506" })`
- `mcp({ search: "rayon rust parallel iterator chunks" })`
- `mcp({ search: "cargo criterion chunks per second benchmark" })`
- Usa Computer Use para abrir el jar de vanilla y examinar archivos de worldgen/data

## 📖 PASO 1 — Leer contexto

ARCHITECTURE.md §6 (Worldgen) — leer COMPLETO. Prestar atención a:
- Pipeline de worldgen: noise → biome → surface → carvers → features → structures → spawn → light
- RNG: XORoshiro128
- Verificación: xxHash64 chunk checksum contra golden data
- Golden data pipeline descrita en §6
- DAG de chunks (3×3 neighbor dependency para features/structures)
- Datapack vanilla embebido para density functions/configs JSON

Lee también BENCHMARKS.md, STATE.md, runs/run-002.md.

## 🏗️ PASO 2 — Orquestación

### Worktrees

```bash
# Golden data pipeline (generar chunks de vanilla para 50 seeds, calcular xxHash64)
orca worktree create --name f2-golden --no-parent --agent codex --setup run \
  --prompt "Crea tools/golden-data/ que: 1) levanta Vanilla 26.2 headless, 2) genera N chunks por seed, 3) calcula xxHash64 de cada chunk, 4) guarda en JSON con seed, chunk coords, hash. Para 50 seeds fijas: 12345, 67890, ... (elegir 50 variadas). Incluye script tools/golden-data/generate.sh y tools/golden-data/hashes.json" \
  --json

# Noise / density functions
orca worktree create --name f2-noise --no-parent --agent codex --setup run \
  --prompt "Implementa neutron-worldgen noise: 1) XORoshiro128 PRNG exacto (verificar contra vanilla con test), 2) Perlin noise 3D con octaves, 3) Density functions desde JSON del datapack vanilla, 4) Biome source con temperatura/humidity/continentalness/erosion/depth/weirdness. Paridad verificable con xxHash64 contra golden data" \
  --json

# Surface + carvers + features
orca worktree create --name f2-surface --no-parent --agent codex --setup run \
  --prompt "Implementa neutron-worldgen surface rules, carvers (cave/abyssal/swamp_miasma), y placed features (trees, ores, flowers, lakes, springs). Carga config JSON del datapack vanilla. Verificar paridad contra golden data con xxHash64" \
  --json

# Structures
orca worktree create --name f2-structures --no-parent --agent codex --setup run \
  --prompt "Implementa structures fase 1 en neutron-worldgen: villages (generation attempts en región 32×32), strongholds (layout concéntrico). Sigue el algoritmo de cubiomes. Usa seeded random para determinar posición exacta" \
  --json

# CPS benchmark
orca worktree create --name f2-bench --no-parent --agent codex --setup run \
  --prompt "Implementa bench de cps (chunks/second) para Neutron: genera N chunks con 16 hilos (rayon), mide tiempo total, reporta cps sostenido. Usa criterion. Sigue metodología de BENCHMARKS.md. Reporta en bench/results/cps-f2.json" \
  --json
```

### DAG de Orquestación

```bash
orca orchestration run-create --objective "F2: worldgen paridad 1:1" --json

T_GOLDEN=$(orca orchestration task-create --spec "T1 - Golden data: 50 seeds, xxHash64 de cada chunk, hashes.json" --deps '[]' --json | jq -r '.id')
T_NOISE=$(orca orchestration task-create --spec "T2 - Noise + density functions + biomes" --deps '[]' --json | jq -r '.id')
T_SURFACE=$(orca orchestration task-create --spec "T3 - Surface rules + carvers + features" --deps "[\"$T_NOISE\"]" --json | jq -r '.id')
T_STRUCT=$(orca orchestration task-create --spec "T4 - Structures fase 1" --deps "[\"$T_SURFACE\"]" --json | jq -r '.id')
T_BENCH=$(orca orchestration task-create --spec "T5 - CPS benchmark" --deps "[\"$T_GOLDEN\"]" --json | jq -r '.id')
```

### Workers

```bash
# T1 (golden) y T2 (noise) en paralelo
orca orchestration worker-start --task "$T_GOLDEN" --worktree new-child --name f2-golden-worker --agent codex --json
orca orchestration worker-start --task "$T_NOISE" --worktree new-child --name f2-noise-worker --agent codex --json

# Esperar
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 1800000 --json
# Gate humano: revisar golden data
orca orchestration gate-create --task "$T_GOLDEN" --question "¿Los golden data para 50 seeds son correctos? Revisar hashes.json" --json

# Continuar con T3, T4, T5
orca orchestration worker-start --task "$T_SURFACE" --worktree new-child --name f2-surface-worker --agent codex --json
orca orchestration worker-start --task "$T_BENCH" --worktree new-child --name f2-bench-worker --agent codex --json
```

## 🎯 Bar (criterios de aprobación — el critic VERIFICA cada uno)

- [ ] 50 seeds golden: checksum xxHash64 idéntico a vanilla — **0 mismatches**
- [ ] cps > 250 @16 hilos, reproducido en Windows y Linux — **verificado por benchmark independiente**
- [ ] Un mundo generado por Neutron abre en vanilla con el mismo terreno — **verificación visual + checksum**

## 🔄 CRITIC ciego

Para CADA componente, lanza subagente reviewer:

**Critic noise**: Compara chunks contra golden data. Ejecuta test de paridad. Verifica XORoshiro128 contra vanilla.

**Critic surface/features**: Verifica surface rules, carvers, features. Checksum contra golden data en 10 seeds.

**Critic structures**: Verifica posición de villages y strongholds contra vanilla (cubiomes o manual).

**Critic benchmark**: Ejecuta benchmark, verifica cps > 250. Compara con metodología BENCHMARKS.md.

Reglas: REJECT por defecto. Evidencia real (logs, hashes). Un solo gap por iteración.

## ✅ Salida

- `tools/golden-data/` con 50+ seeds de referencia
- `crates/neutron-worldgen/` funcional con paridad 1:1
- `bench/results/cps-f2.json`
- `runs/run-003.md`
- STATE.md → "F2: worldgen paridad 1:1"

=== FIN PROMPT F2 ===
```

---

### F3 — Simulación vanilla

**Objetivo**: bloques, fluidos, iluminación, redstone, spawns, survival.
**Bar**: suite dorada posicional 100% contra server vanilla real (bots); light arrays idénticos en 50 seeds; survival básica jugable por bot.
**Piezas**: iluminación · redstone A/B/C/D · fluidos · spawns · survival. **Riesgo**: **ALTO** (redstone posicional — el mayor reto técnico).

```text
=== PROMPT F3 — Copiar y pegar en pi ===

Eres el LEAD del proyecto Neutron. F3: Simulación vanilla — **EL MAYOR RETO TÉCNICO DEL PROYECTO**.

## ⚙️ PASO 0 — Skills + MCP

Carga: gauntlet-loop, loop-engineering, orca-cli, orchestration, mcp-scripting.

MCP tools para INVESTIGACIÓN (cada sub-sistema necesita su propia investigación):

**Iluminación:**
- `mcp({ search: "paper starlight lighting engine voxel octree" })` — arquitectura Starlight
- `mcp({ search: "minecraft light level propagation sky block" })` — mecánica de luz vanilla
- `mcp({ search: "crate starlight lighting rust voxel" })` — buscar crates Rust de iluminación
- `mcp({ search: "minecraft light array format chunk NBT" })` — formato de luz en chunks

**Redstone** (la parte más crítica):
- `mcp({ search: "minecraft redstone wire update order PP NC 1.21" })` — orden de updates post-1.21.2
- `mcp({ search: "minecraft quasi-connectivity QC piston java edition" })` — QC exacto
- `mcp({ search: "minecraft 1.21 redstone experiments left-first" })` — left-first wire prioritization
- `mcp({ search: "pumpkin redstone implementation github" })` — cómo lo hizo Pumpkin
- `mcp({ search: "minecraft redstone comparator container subtraction" })` — comparators
- `mcp({ search: "minecraft piston block swapping mechanics" })` — block swapping con pistons
- `mcp({ search: "minecraft observer block update detection" })` — observers
- `mcp({ search: "minecraft hopper item transfer tick timing" })` — hoppers

**Fluidos:**
- `mcp({ search: "minecraft water flow mechanics source spread" })`
- `mcp({ search: "minecraft lava flow 8 levels vs 7" })`
- `mcp({ search: "minecraft bubble columns magma soul sand" })`

**Spawns:**
- `mcp({ search: "minecraft mob spawning algorithm light level" })`
- `mcp({ search: "minecraft despawn mechanics 32 128 blocks" })`

**ECS/Survival:**
- `mcp({ search: "bevy_ecs crate rust tutorial getting started" })`
- `mcp({ search: "minecraft hunger health damage mechanics" })`
- `mcp({ search: "minecraft crafting recipes 3x3 grid" })`

Usa **Computer Use** para:
- Levantar un servidor vanilla 26.2 real y probar redstone contraptions manualmente
- Comparar visualmente el comportamiento de redstone entre vanilla y Neutron
- Verificar light arrays con un mod como MiniHUD

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

## 🏗️ PASO 2 — Orquestación con DAG profundo

### Worktrees (8 total, se lanzan según dependencias)

```bash
# === FASE 2a: Workers independientes ===

# Iluminación
orca worktree create --name f3-light --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **neutron-sim lighting engine** estilo Starlight:
  1) Voxel octree para almacenamiento eficiente de luz
  2) Sky light propagation desde bloques translúcidos
  3) Block light propagation desde fuentes (torches, glowstone, lava)
  4) Dirty flag updates cuando un chunk cambia
  5) Light arrays idénticos a vanilla en chunk NBT
  Verificación: xxHash64 de light arrays contra vanilla en 50 seeds.
  Referencia: github.com/PaperMC/Starlight, minecraft.wiki/Light'

# Redstone A — básico
orca worktree create --name f3-redstone-A --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **Redstone A** (básico) en neutron-sim:
  1) REDSTONE WIRE: update order exacto PP (W,E,N,S,D,U) y NC (W,E,D,U,N,S)
     Post-1.21.2: left-first wire prioritization (Redstone Experiments)
  2) REDSTONE TORCHES: lit/unlit state, burn-out después de 8 cambios/60 ticks
  3) LEVERS: toggle on right-click, solid attach, powered state
  4) DOORS: open/close con redstone, double doors, correct block states
  5) BLOCK UPDATES: notify neighbors en orden correcto
  Pruebas: same redstone circuit → same behavior en vanilla y Neutron'

# Fluidos
orca worktree create --name f3-fluids --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **fluidos** en neutron-sim:
  1) WATER: source blocks, flow (spread level 1-7), current (flowing water empuja entidades)
  2) LAVA: source blocks, flow (8 levels vs 7 de water), no empuja entidades
  3) BUBBLE COLUMNS: magma block (suction down), soul sand (push up)
  4) WATERLOGGING: bloques que pueden contener agua (stairs, slabs, fences)
  5) Fluid tick scheduling: same priority queue como vanilla'

# Spawns
orca worktree create --name f3-spawns --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **mob spawning** en neutron-sim:
  1) Spawning rules: light level < 7 for hostiles, biome-specific mobs
  2) Mob categories: hostile, passive, ambient, water, water_ambient
  3) Pack spawning: groups of mobs spawn together
  4) Despawning: >128 blocks instant, 32-128 random, <32 stays
  5) Mob caps: global y por player, same como vanilla'

# === FASE 2b: Dependen de Redstone A ===

# Redstone B — avanzado (depende de Redstone A)
orca worktree create --name f3-redstone-B --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **Redstone B** (avanzado) en neutron-sim, DEPENDE de Redstone A:
  1) COMPARATORS: subtraction mode, container-based (chest, furnace, hopper)
     Redstone comparison based on item count / stack size
  2) REPEATERS: delay 1-4 ticks, lock mode (locked por otro repeater)
  3) OBSERVERS: block update detection, pulse generation on state change
  4) HOPPERS: item transfer between containers, cooldown (8 ticks), push/pull
  5) TNT: prime (redstone trigger), fuse timer, explosion (destroy blocks + damage)
  Verificar interacciones wire→comparator→repeater→observer→hopper'

# === FASE 2c: Dependen de Redstone B ===

# Redstone C — pistons (depende de Redstone B)
orca worktree create --name f3-redstone-C --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **Redstone C** (pistons) en neutron-sim, DEPENDE de Redstone B:
  1) PISTONS: extend/retract, push limit 12 blocks
  2) STICKY PISTONS: pull back (mismo limite 12), sticky block adhesion
  3) QUASI-CONNECTIVITY (QC): Java-only. Piston activado por redstone 1 block arriba
     y 1 block a los lados. QC = BUD (block update detector) behavior
  4) BLOCK SWAPPING: sticky piston intercambia bloques en ciertos casos
  5) BLOCK ENTITY MOVING: chests, furnaces, hoppers movidos por pistons
     (preservar NBT data durante el movimiento)
  **Esta es la mecánica más compleja de todo Minecraft** — probar contra vanilla'

# === FASE 2d: Integración ===

# Golden suite posicional (depende de light + redstone-C + fluids)
orca worktree create --name f3-golden-suite --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **golden suite posicional** en bench/:
  1) Script que levanta Vanilla 26.2 y Neutron con la misma seed
  2) Ejecuta bots que construyen redstone contraptions idénticas
  3) Compara posición por posición, estado a estado
  4) Reporta bench/results/golden-f3.json con 100% match o gaps
  5) Incluye contraptions: RSNOR (redstone NOR gate), T-flip-flop,
     piston extender, item sorter, 2x2 piston door, creeper farm'

# Survival (depende de golden suite)
orca worktree create --name f3-survival --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **survival básica** en neutron-sim:
  1) HUNGER: food bar, saturation, exhaustion, starvation damage
  2) HEALTH: hearts, damage (fall, suffocation, drowning, fire, mob)
  3) DAMAGE: invincibility frames (0.5s), armor reduction, enchantment protection
  4) CRAFTING: 2x2 grid (inventory) y 3x3 grid (crafting table)
     Recipes cargados desde JSON del datapack vanilla
  5) INVENTORY: hotbar, armor slots, offhand, item picking, dropping
  6) EXPERIENCE: XP orbs, enchanting, repair cost'
```

### DAG de Tasks

```bash
orca orchestration run-create --objective "F3: simulación vanilla" --json

# FASE 2a — Independientes
T_LIGHT=$(orca orchestration task-create --spec "T1 - Iluminación: voxel octree, sky/block light, 50 seeds xxHash64 match" --deps '[]' --json | jq -r '.id')
T_RA=$(orca orchestration task-create --spec "T2 - Redstone A: wire, torches, levers, doors. Update order exacto" --deps '[]' --json | jq -r '.id')
T_FLUID=$(orca orchestration task-create --spec "T3 - Fluidos: water, lava, bubble columns, waterlogging" --deps '[]' --json | jq -r '.id')
T_SPAWN=$(orca orchestration task-create --spec "T4 - Spawns: mob spawning rules, light levels, despawning" --deps '[]' --json | jq -r '.id')

# Lanzar FASE 2a en paralelo
W_LIGHT=$(orca orchestration worker-start --task "$T_LIGHT" --worktree new-child --name f3-light-worker --agent codex --json | jq -r '.effects.dispatch.dispatch_id')
W_RA=$(orca orchestration worker-start --task "$T_RA" --worktree new-child --name f3-RA-worker --agent codex --json | jq -r '.effects.dispatch.dispatch_id')
W_FLUID=$(orca orchestration worker-start --task "$T_FLUID" --worktree new-child --name f3-fluid-worker --agent codex --json)
W_SPAWN=$(orca orchestration worker-start --task "$T_SPAWN" --worktree new-child --name f3-spawn-worker --agent codex --json)

# Esperar FASE 2a
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json

# GATE HUMANO: Redstone A es CRÍTICO — revisión obligatoria
orca orchestration gate-create --task "$T_RA" --question "Redstone A (wire, torches, levers, doors): ¿el update order es correcto? ¿wire post-1.21.2 funciona? Prueba con circuitos reales" --json

# FASE 2b — Dependen de Redstone A
T_RB=$(orca orchestration task-create --spec "T5 - Redstone B: comparators, repeaters, observers, hoppers, TNT" --deps "[\"$T_RA\"]" --json | jq -r '.id')

# Lanzar FASE 2b
W_RB=$(orca orchestration worker-start --task "$T_RB" --worktree new-child --name f3-RB-worker --agent codex --json)

# Esperar
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json

# GATE HUMANO: Redstone B también es crítico
orca orchestration gate-create --task "$T_RB" --question "Redstone B (comparators, repeaters, observers, hoppers, TNT): ¿funciona correctamente con A? Prueba: item sorter, TNT duper" --json

# FASE 2c — Depende de Redstone B
T_RC=$(orca orchestration task-create --spec "T6 - Redstone C: pistons, QC, block swapping, block entity moving" --deps "[\"$T_RB\"]" --json | jq -r '.id')

# Lanzar
W_RC=$(orca orchestration worker-start --task "$T_RC" --worktree new-child --name f3-RC-worker --agent codex --json)

# Esperar
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json

# GATE HUMANO: PISTONS = el pico más alto de riesgo técnico
orca orchestration gate-create --task "$T_RC" --question "Redstone C (pistons + QC): ¿funciona QC exactamente como vanilla? ¿block swapping? ¿piston movable block entities? Prueba: 2x2 piston door, flying machine, QC piston extender" --json

# FASE 2d — Golden suite (depende de light + redstone-C + fluids)
T_GOLDEN=$(orca orchestration task-create --spec "T7 - Golden suite posicional: bots comparan vanilla vs Neutron, redstone contraptions, reporte JSON" --deps "[\"$T_LIGHT\",\"$T_RC\",\"$T_FLUID\"]" --json | jq -r '.id')
T_SURVIVAL=$(orca orchestration task-create --spec "T8 - Survival básica: hunger, health, damage, crafting, inventory" --deps "[\"$T_GOLDEN\"]" --json | jq -r '.id')

# Lanzar golden suite
W_GOLDEN=$(orca orchestration worker-start --task "$T_GOLDEN" --worktree new-child --name f3-golden-worker --agent codex --json)

# Esperar
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json

# Lanzar survival
W_SURVIVAL=$(orca orchestration worker-start --task "$T_SURVIVAL" --worktree new-child --name f3-survival-worker --agent codex --json)

# Esperar
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json
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
=== PROMPT F4 — Copiar y pegar en pi ===

Eres el LEAD del proyecto Neutron. F4: Escala 500-1000+.

## ⚙️ Skills + MCP
gauntlet-loop, loop-engineering, orca-cli, orchestration, mcp-scripting

MCP: `mcp({ search: "minecraft folia region scheduler architecture" })`, `{ search: "rust lock free data structures arena allocator" }`, `{ search: "cargo criterion benchmark hot path" }`, `{ search: "rust memory profiling dhat heaptrack" }`.

## 📖 Contexto
ARCHITECTURE.md §7 (ECS + scheduler por regiones) y §4 (Escala). Leer BENCHMARKS.md §7 (targets).

## 🏗️ Orquestación

Worktrees: f4-scheduler (Folia-style), f4-optimizations (arenas, lock-free), f4-stress (500 bots), f4-profiling (dhat/heaptrack).

DAG: scheduler → optimizations → stress; profiling paralelo.

## 🎯 Bar
- [ ] 500 bots × 60 min → TPS 20.0, p99 tick < 25 ms
- [ ] RAM/jugador < 1 MB sobre base < 150 MB
- [ ] Sin regresión de cps/startup vs F3

## 🔄 CRITIC
Ejecuta stress test de 500 bots, mide TPS cada 5s, reporta p99. Compara con targets de BENCHMARKS.md. Memory profile con dhat.

=== FIN PROMPT F4 ===
```

---

### F5 — Mobs y AI

**Objetivo**: comportamiento vanilla de mobs y combate completo.
**Bar**: E2E 20 min de survival; spot-checks (creeper explota, zombie quema al sol, enderman TP, dragon fight); 50 mobs/chunk sin regresión TPS.
**Piezas**: pasivos + trading · hostiles · jefes · combate · pathfinding A*. **Riesgo**: **ALTO**.

```text
=== PROMPT F5 — Copiar y pegar en pi ===

Eres el LEAD del proyecto Neutron. F5: Mobs y AI.

## ⚙️ Skills + MCP

Skills: gauntlet-loop, loop-engineering, orca-cli, orchestration, mcp-scripting.

MCP tools para investigación:
- `mcp({ search: "minecraft villager trading system mechanics" })`
- `mcp({ search: "minecraft zombie behavior pathfinding" })`
- `mcp({ search: "minecraft creeper explosion radius damage" })`
- `mcp({ search: "minecraft ender dragon fight phases" })`
- `mcp({ search: "minecraft wither boss fight mechanics" })`
- `mcp({ search: "minecraft enderman teleport mechanics" })`
- `mcp({ search: "rust A* pathfinding crate" })`
- `mcp({ search: "bevy_ecs AI behavior tree rust" })`
- `mcp({ search: "minecraft sword damage attack cooldown" })`
- `mcp({ search: "minecraft bow arrow mechanics" })`
- `mcp({ search: "minecraft enchantment mechanics sharpness protection" })`
- Computer Use: para probar mob behavior en vanilla real

## 📖 Contexto
Lee ARCHITECTURE.md §7 (Simulación) — ECS con bevy_ecs, entidades, componentes (posición, salud, AI state, inventario), systems (movimiento, AI, combate).
Lee también: STATE.md, runs/run-005.md.

## 🏗️ Orquestación

### Worktrees

```bash
# Pathfinding A* engine (base para todos los mobs)
orca worktree create --name f5-ai --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **pathfinding A*** en neutron-sim:
  1) Goal-oriented: entity quiere llegar a un destino
  2) A* algorithm con heurística manhattan
  3) Path smoothing (evitar zigzag en pasillos)
  4) Obstable avoidance (no atravesar bloques sólidos)
  5) Performance: path recalculation rate (cada 5-20 ticks según tipo mob)
  6) 50 mobs/chunk sin TPS drop significativo'

# Mobs pasivos
orca worktree create --name f5-passives --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **mobs pasivos** en neutron-sim:
  1) COW: wander, moo, drop beef/leather on death, breed (wheat)
  2) PIG: wander, drop porkchop, breed (carrot), saddled (rideable?)
  3) SHEEP: wander, eat grass blocks (regrow wool), shear, dyeable
  4) CHICKEN: wander, lay eggs, drop feather/chicken, breed (seeds)
  5) HORSE: wander, tameable, rideable, inventory (saddle + armor)
  6) VILLAGER: schedule-based AI (work/sleep/wander), professions, trading
     GUI (trade interface), gossip, iron golem summoning'

# Mobs hostiles
orca worktree create --name f5-hostiles --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **mobs hostiles** en neutron-sim:
  1) ZOMBIE: chase player, attack, burn in sunlight, drown, break doors (hard)
     Reinforcements (zombie can call more zombies)
  2) SKELETON: strafe player, shoot arrows, burn in sunlight
  3) SPIDER: climb walls, leap attack, become neutral in sunlight
  4) CREEPER: hiss + explode (3s fuse), charged (lightning), block damage
  5) SLIME: split on death (size 1→2→4), hop movement
  6) PHANTOM: spawn after 3+ sleepless nights, dive attack
  7) ENDERMAN: neutral until eye contact, teleport on hit/water/sunlight
     Pick up blocks randomly'

# Combate
orca worktree create --name f5-combat --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **combate** en neutron-sim:
  1) MELEE: sword/axe attack damage según material, attack cooldown (1.9+)
     Sweep attack (sword), critical hits (falling)
  2) BOW: charge time, arrow trajectory (gravity), enchantments (power, flame)
  3) SHIELD: blocking (reduce damage 100% frontal), cooldown after hit
  4) TRIDENT: melee + ranged (loyalty, riptide, channeling)
  5) ENCHANTMENTS: sharpness, protection, fire_aspect, knockback, power, flame
     Anvil + enchanting table mechanics'

# Jefes
orca worktree create --name f5-bosses --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **jefes** en neutron-sim:
  1) ENDER DRAGON (FASE 1): fight mechanics (crystals heal dragon, perch, breath)
     Portal spawn, exit portal, egg drop
     AI: fly pattern, charge attack, fireball defense
  2) WITHER (FASE 2): spawn (soul sand + wither skulls), half-health rage
     AI: shoot heads (blue = wither effect), break blocks, dash'

# E2E
orca worktree create --name f5-e2e --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **E2E survival 20 min** en bench/:
  Crea bot en azalea (Rust 26.x) que:
  1) Spawnea en survival mode
  2) Se mueve, encuentra animales, los mata
  3) Recoge drops, abre inventario
  4) Craftea items (2x2 grid)
  5) Come cuando tiene hambre
  6) Sobrevive 20 minutos sin morir
  7) Spot-checks: creeper explota → daño recibido, zombie quema al sol, etc.
  Reporta en bench/results/e2e-f5.json'
```

### DAG

```bash
orca orchestration run-create --objective "F5: mobs y AI" --json

# FASE 1 — AI engine (base de todo)
T_AI=$(orca orchestration task-create --spec "T1 - Pathfinding A* engine" --deps '[]' --json | jq -r '.id')

# Lanzar AI primero
orca orchestration worker-start --task "$T_AI" --worktree new-child --name f5-ai-worker --agent codex --json
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 1800000 --json
orca orchestration gate-create --task "$T_AI" --question "¿El pathfinding A* es correcto? Prueba: 50 mobs en chunk, TPS sin drop, pathfinding goal-oriented" --json

# FASE 2 — Pasivos + hostiles + combate en paralelo
T_PASSIVES=$(orca orchestration task-create --spec "T2 - Mobs pasivos + trading" --deps "[\"$T_AI\"]" --json | jq -r '.id')
T_HOSTILES=$(orca orchestration task-create --spec "T3 - Mobs hostiles" --deps "[\"$T_AI\"]" --json | jq -r '.id')
T_COMBAT=$(orca orchestration task-create --spec "T4 - Combate" --deps '[]' --json | jq -r '.id')

# Lanzar en paralelo
orca orchestration worker-start --task "$T_PASSIVES" --worktree new-child --name f5-passives-worker --agent codex --json
orca orchestration worker-start --task "$T_HOSTILES" --worktree new-child --name f5-hostiles-worker --agent codex --json
orca orchestration worker-start --task "$T_COMBAT" --worktree new-child --name f5-combat-worker --agent codex --json

orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json

# GATE HUMANO para mobs
orca orchestration gate-create --task "$T_HOSTILES" --question "¿Creeper explota? ¿Zombie quema al sol? ¿Enderman se teleporta? Prueba cada uno manualmente" --json

# FASE 3 — Bosses (depende de hostiles + combat)
T_BOSSES=$(orca orchestration task-create --spec "T5 - Jefes: Ender Dragon, Wither" --deps "[\"$T_HOSTILES\",\"$T_COMBAT\"]" --json | jq -r '.id')

# Lanzar
orca orchestration worker-start --task "$T_BOSSES" --worktree new-child --name f5-bosses-worker --agent codex --json
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json

# FASE 4 — E2E (depende de todo)
T_E2E=$(orca orchestration task-create --spec "T6 - E2E survival 20 min" --deps "[\"$T_PASSIVES\",\"$T_HOSTILES\",\"$T_BOSSES\",\"$T_COMBAT\"]" --json | jq -r '.id')

orca orchestration worker-start --task "$T_E2E" --worktree new-child --name f5-e2e-worker --agent codex --json
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json
```

## 🎯 Bar
- [ ] E2E 20 min de survival sin crashes
- [ ] Spot-checks automatizados:
  - [ ] Creeper explota y causa daño (área + bloques)
  - [ ] Zombie quema al amanecer
  - [ ] Enderman se teleporta al recibir daño/agua/sol
  - [ ] Dragon fight: fases, crystals, breath attack, perch
  - [ ] Wither: spawn, half-health rage
- [ ] 50 mobs/chunk sin regresión de TPS

## 🔄 CRITIC

Lanza reviewer para CADA worker:
- pathfinding: ejecuta 50 mobs en chunk, mide TPS, verifica A*
- pasivos: verifica trading UI, breeding, shearing
- hostiles: verifica cada comportamiento (creeper explode, zombie burn, etc.)
- combat: verifica damage calculation, attack cooldown, enchantments
- bosses: fight completo Ender Dragon (no necesitas matarlo, solo verificar mecánicas)
- e2e: 20 min de survival completo

## ✅ Salida
- Pathfinding A* funcional
- Mob behavior verificado contra vanilla
- Ender Dragon + Wither con fight mechanics
- `bench/results/e2e-f5.json` — 20 min survival
- STATE.md → "F5: mobs y AI"

=== FIN PROMPT F5 ===
```

---

### F6 — Plugins WASM + Lua

**Objetivo**: ecosistema seguro por construcción.
**Bar**: WASM panic no tumba servidor; fuel 10M opcodes mata plugin sin daño; hot reload; 3 conversiones Bukkit; coste < 5 µs/tick.
**Piezas**: wasmtime + WIT · API · Lua (mlua) · converter · PatchBukkit · docs. **Riesgo**: alto.

```text
=== PROMPT F6 — Copiar y pegar en pi ===

Eres el LEAD del proyecto Neutron. F6: Plugins WASM + Lua.

## ⚙️ Skills + MCP

Skills: gauntlet-loop, loop-engineering, orca-cli, orchestration, mcp-scripting.

MCP tools:
- `mcp({ search: "wasmtime rust component model tutorial" })`
- `mcp({ search: "WIT interface types wasm" })`
- `mcp({ search: "mlua rust crate docs" })`
- `mcp({ search: "bukkit plugin java event system" })`
- `mcp({ search: "wasm fuel limit sandbox rust" })`
- `mcp({ search: "pumpkin mc plugin system" })`
- `mcp({ search: "hot reload rust shared library" })`
- `mcp({ search: "cargo criterion microbenchmark nanoseconds" })`

## 📖 Contexto
Lee ARCHITECTURE.md §9 (Plugins) + §1 (Seguridad por construcción).
Lee STATE.md, runs/run-006.md.

## 🏗️ Orquestación

```bash
# Runtime WASM
orca worktree create --name f6-wasm --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **runtime WASM** en neutron-plugin:
  1) wasmtime + WIT component model
  2) Sandbox: fuel limit (10M opcodes), memory limit (probar límites)
  3) Plugin API: init(), tick(), event(event_data), command(cmd, args)
  4) Host API: set_block(x,y,z,block_id), get_block(x,y,z), send_message(player,msg),
     get_player_position(player), set_time(time), spawn_entity(entity_type,pos)
  5) Safety: if plugin panics → kill plugin ONLY, not the server
  6) Hot reload: watch .wasm file, reload on change sin restart'

# Lua
orca worktree create --name f6-lua --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **Lua scripting** en neutron-plugin:
  1) mlua engine, API equivalente a WASM
  2) Hook: before/after tick, block events, player events
  3) Safety: coroutine yield timeout, no infinite loops
  4) Same host API que WASM: set_block, get_block, send_message, etc.'

# Convertidor Bukkit
orca worktree create --name f6-converter --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **convertidor Bukkit→WASM** en tools/:
  1) Parse plugin.yml (name, main, commands, permissions)
  2) Transform Java Bukkit listener methods → WIT functions
  3) Generate .wasm stub from Bukkit plugin
  4) Soporte para: onEnable(), onDisable(), onCommand(), onPlayerJoin(), onBlockBreak()
  5) 3 conversiones de ejemplo: EssentialsX motd, SimpleVote, WorldEdit wand'

# PatchBukkit layer
orca worktree create --name f6-bukkit --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **PatchBukkit v0** en tools/:
  1) Bukkit API surface parcial (event system, command system, scheduler)
  2) Traduce llamadas Bukkit comunes → WIT host calls
  3) NO intentar compatibilidad 100% — solo lo que se pueda convertir'

# Docs
orca worktree create --name f6-docs --no-parent --agent codex --setup run --json \
  --prompt 'Escribe **docs de plugins** en docs/plugins/:
  1) Writing a WASM plugin: tutorial paso a paso
  2) Writing a Lua plugin: tutorial paso a paso
  3) API reference: host functions, events, permissions
  4) Converting Bukkit plugins: guía + ejemplos
  5) Security: sandbox, fuel limits, best practices'
```

### DAG
```bash
orca orchestration run-create --objective "F6: plugins WASM + Lua" --json

T_WASM=$(orca orchestration task-create --spec "T1 - Runtime WASM" --deps '[]' --json | jq -r '.id')
T_LUA=$(orca orchestration task-create --spec "T2 - Lua scripting" --deps '[]' --json | jq -r '.id')

# Lanzar WASM + Lua en paralelo
orca orchestration worker-start --task "$T_WASM" --worktree new-child --name f6-wasm-worker --agent codex --json
orca orchestration worker-start --task "$T_LUA" --worktree new-child --name f6-lua-worker --agent codex --json

orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json

T_CONVERTER=$(orca orchestration task-create --spec "T3 - Convertidor Bukkit" --deps "[\"$T_WASM\"]" --json | jq -r '.id')
T_BUKKIT=$(orca orchestration task-create --spec "T4 - PatchBukkit v0" --deps "[\"$T_WASM\"]" --json | jq -r '.id')
T_DOCS=$(orca orchestration task-create --spec "T5 - Docs" --deps "[\"$T_WASM\",\"$T_LUA\"]" --json | jq -r '.id')

# Lanzar en paralelo
orca orchestration worker-start --task "$T_CONVERTER" --worktree new-child --name f6-converter-worker --agent codex --json
orca orchestration worker-start --task "$T_BUKKIT" --worktree new-child --name f6-bukkit-worker --agent codex --json
orca orchestration worker-start --task "$T_DOCS" --worktree new-child --name f6-docs-worker --agent codex --json

orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json
```

## 🎯 Bar (SEGURIDAD es lo más importante)
- [ ] Plugin WASM panic → servidor sigue funcionando, solo plugin muere
- [ ] Fuel 10M opcodes → plugin termina, servidor intacto
- [ ] Memory limit → plugin excede → plugin muere, servidor intacto
- [ ] Hot reload: cambiar .wasm sin restart server
- [ ] 3 plugins Bukkit convertidos reales (EssentialsX motd, SimpleVote, WorldEdit wand)
- [ ] Coste en hot path < 5 µs/tick (benchmark con criterion)

## 🔄 CRITIC
- Sandbox: escribe plugin que hace panic, verifica servidor sobrevive
- Fuel: escribe plugin con loop infinito, verifica fuel termina el plugin
- Hot reload: cambia .wasm, verifica que se recarga sin restart
- Converter: convierte 3 plugins reales, verifica funcionan
- Performance: benchmark criterion < 5 µs/tick

## ✅ Salida
- `crates/neutron-plugin/` — WASM + Lua runtimes
- `tools/patch-bukkit/` — convertidor + capa compat
- 3 plugins convertidos funcionales
- `docs/plugins/` — guías y API reference
- STATE.md → "F6: plugins WASM + Lua"

=== FIN PROMPT F6 ===
```

---

### F7 — Bedrock

**Objetivo**: clientes Bedrock 26.x en el mismo mundo.
**Bar**: cliente Bedrock real juega 10 min; coexistencia Java+Bedrock ; TPS Java sin impacto.
**Piezas**: RakNet + login/play · play básico · mapeo Java↔Bedrock · coexistencia. **Riesgo**: medio.

```text
=== PROMPT F7 — Copiar y pegar en pi ===

Eres el LEAD del proyecto Neutron. F7: Bedrock.

## ⚙️ Skills + MCP

Skills: gauntlet-loop, loop-engineering, orca-cli, orchestration, mcp-scripting, computer-use.

MCP tools:
- `mcp({ search: "raknet protocol specification" })`
- `mcp({ search: "rust raknet crate implementation" })`
- `mcp({ search: "minecraft bedrock protocol 26 login" })`
- `mcp({ search: "bedrock java block id mapping" })`
- `mcp({ search: "bedrock java item id mapping" })`
- `mcp({ search: "bedrock java biome id mapping" })`
- `mcp({ search: "azalea bedrock protocol" })`
- Computer Use: para instalar y conectar Minecraft Bedrock real (Windows 10/11)

## 📖 Contexto
ARCHITECTURE.md §3 (Protocolo) — Bedrock: F7, capa de sesión independiente, RakNet.
Lee STATE.md, runs/run-007.md.

## 🏗️ Orquestación (chain: cada worker depende del anterior)

```bash
# FASE 1: RakNet + login/play
orca worktree create --name f7-raknet --no-parent --agent codex --setup run --json \
  --prompt 'Implementa **RakNet protocol** en neutron-protocol-bedrock:
  1) RakNet connection: open_connection, connection_request, new_incoming_connection
  2) Encapsulation: frame types, reliability, ordering channels
  3) Login: Login packet → ServerLoginResponse → PlayStatus
     Auth: offline (xuid generado) + online (xbox live — fase 2)
  4) Play: level_chunk, move_player, player_auth_input, inventory_content,
     text (chat), set_time, set_difficulty'

orca orchestration run-create --objective "F7: Bedrock" --json

T_RAKNET=$(orca orchestration task-create --spec "T1 - RakNet + login/play" --deps '[]' --json | jq -r '.id')

orca orchestration worker-start --task "$T_RAKNET" --worktree new-child --name f7-raknet-worker --agent codex --json
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json
orca orchestration gate-create --task "$T_RAKNET" --question "¿Conexión RakNet funciona? Prueba con Minecraft Bedrock real" --json

# FASE 2: Registry mapping Java ↔ Bedrock
T_REGISTRY=$(orca orchestration task-create --spec "T2 - Registry mapping Java↔Bedrock" --deps "[\"$T_RAKNET\"]" --json | jq -r '.id')

orca orchestration worker-start --task "$T_REGISTRY" --worktree new-child --name f7-registry-worker --agent codex --json
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json

# FASE 3: Play básico (movimiento, chat, bloques, inventario)
T_PLAY=$(orca orchestration task-create --spec "T3 - Play básico" --deps "[\"$T_REGISTRY\"]" --json | jq -r '.id')

orca orchestration worker-start --task "$T_PLAY" --worktree new-child --name f7-play-worker --agent codex --json
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json

# FASE 4: Coexistencia Java + Bedrock
T_COEXIST=$(orca orchestration task-create --spec "T4 - Coexistencia Java+Bedrock" --deps "[\"$T_PLAY\"]" --json | jq -r '.id')

orca orchestration worker-start --task "$T_COEXIST" --worktree new-child --name f7-coexist-worker --agent codex --json
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 3600000 --json
```

### Orca para tests con Computer Use
```bash
# Después de T1 (RakNet), USA COMPUTER USE para probar:
# 1. Abre Minecraft Bedrock desde Windows Start
# 2. Conecta a localhost:19132
# 3. Verifica login exitoso
# 4. Lee logs del servidor para confirmar conexión
```

## 🎯 Bar
- [ ] Cliente Bedrock real juega 10 min sin crashes
- [ ] Coexistencia: Java y Bedrock en el mismo mundo simultáneamente
- [ ] TPS Java sin impacto con Bedrock conectado (mismo chunk, mismo tick)

## 🔄 CRITIC
- Lanza un cliente Bedrock real (Computer Use)
- Verifica login, spawn, movimiento, rotura/colocación de bloques
- Conecta un cliente Java 26.2 y un cliente Bedrock 26.x al mismo servidor
- Verifica que ven el mismo mundo
- Mide TPS con ambos conectados, compara con solo Java

## ✅ Salida
- `crates/neutron-protocol-bedrock/` funcional
- Cliente Bedrock real conectado y jugando (evidencia: screenshot + logs)
- `bench/results/coexist-f7.json` — métricas de coexistencia
- STATE.md → "F7: Bedrock"

=== FIN PROMPT F7 ===
```

---

### F8 — 1.0

**Objetivo**: release estable, verificable y defendible.
**Bar**: parity suite completa verde en `main`; benchmarks reproducibles en 2 máquinas; 72 h uptime 100 jugadores; fuzz 24 h limpio; binarios x86-64/ARM64 (Win/Linux/Mac).
**Piezas**: fuzz + audits · benchmarks finales · docs + migración · proceso de release. **Riesgo**: medio.

```text
=== PROMPT F8 — Copiar y pegar en pi ===

Eres el LEAD del proyecto Neutron. F8: Release 1.0.

## ⚙️ Skills + MCP

Skills: gauntlet-loop, loop-engineering, orca-cli, orchestration, mcp-scripting, security-auditor.

MCP tools:
- `mcp({ search: "rust cargo audit security vulnerabilities" })`
- `mcp({ search: "cargo criterion benchmark best practices" })`
- `mcp({ search: "rust cross compilation windows linux macos" })`
- `mcp({ search: "github actions release workflow rust" })`
- `mcp({ search: "minecraft server migration guide vanilla to paper" })`
- Computer Use: para hacer release builds y verificación manual

## 📖 Contexto
Lee TODO: STATE.md, runs/run-008.md, ROADMAP.md completo.
Asegúrate de que TODAS las fases F0-F7 están COMPLETADAS con STATE.md actualizado.

## 🏗️ Orquestación (TODO paralelo)

```bash
orca orchestration run-create --objective "F8: Release 1.0" --json

# T1: Fuzz + Security audit
T_FUZZ=$(orca orchestration task-create --spec "T1 - Fuzz 24h + Security audit: cargo-fuzz decode 24h, cargo audit, review unsafe code, fuzzing del protocolo" --deps '[]' --json | jq -r '.id')

# T2: Benchmarks finales
T_BENCH=$(orca orchestration task-create --spec "T2 - Benchmarks finales: reproducibles en 2 máquinas (Windows + Linux), same seed, same methodology. bench/results/final-f8.json" --deps '[]' --json | jq -r '.id')

# T3: Uptime 72h
T_UPTIME=$(orca orchestration task-create --spec "T3 - Uptime test: 72h con 100 jugadores bots, medir memory leak, TPS sostenido" --deps '[]' --json | jq -r '.id')

# T4: Binarios multiplataforma
T_BINARIES=$(orca orchestration task-create --spec "T4 - Binarios: x86-64/ARM64 para Windows (msi), Linux (deb/rpm), macOS (dmg). GitHub release" --deps '[]' --json | jq -r '.id')

# T5: Docs + Migration guide
T_DOCS=$(orca orchestration task-create --spec "T5 - Docs: README.md final, migration guide vanilla→Neutron, deployment guide, API reference, changelog" --deps '[]' --json | jq -r '.id')

# Lanzar TODO en paralelo
orca orchestration worker-start --task "$T_FUZZ" --worktree new-child --name f8-fuzz-worker --agent codex --json
orca orchestration worker-start --task "$T_BENCH" --worktree new-child --name f8-bench-worker --agent codex --json
orca orchestration worker-start --task "$T_UPTIME" --worktree new-child --name f8-uptime-worker --agent codex --json
orca orchestration worker-start --task "$T_BINARIES" --worktree new-child --name f8-binaries-worker --agent codex --json
orca orchestration worker-start --task "$T_DOCS" --worktree new-child --name f8-docs-worker --agent codex --json

# Esperar a todos los workers
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 86400000 --json
```

## 🎯 Bar
- [ ] Parity suite completa 100% verde en `main`
- [ ] Benchmarks reproducibles en 2 máquinas (Windows + Linux)
- [ ] 72h de uptime con 100 jugadores — 0 memory leaks
- [ ] Fuzz 24h — 0 panics, 0 crashes
- [ ] Binarios: x86-64 + ARM64 para Windows, Linux, macOS
- [ ] Docs completas: README, migration guide, deployment guide, changelog

## 🔄 CRITIC

Usa **security-auditor** como subagente para:
- Verificar 0 unsafe code en hot paths
- Verificar fuel/memory limits en plugins
- Verificar fuzz 24h sin crashes
- Revisar CVE list para dependencias

Usa **reviewer** para:
- Ejecutar benchmarks, verificar reproducibilidad
- Verificar binarios: descargar, ejecutar, conectar cliente
- Revisar docs: están completas, sin typos, ejemplos funcionan

## ✅ Gate humano FINAL

```bash
orca orchestration gate-create \
  --task "$T_BENCH" \
  --question "RELEASE GATE: ¿Estás seguro de que Neutron 1.0 está listo?
  - Parity suite 100% verde?
  - Benchmarks dentro de targets? (< 2s startup, > 250 cps, < 150MB RAM)
  - 72h uptime sin leaks?
  - Fuzz 24h limpio?
  - Binarios en releases/?
  - Docs completas?
  Responde APPROVED o DENIED con razón."
  --json
```

Si gate = APPROVED:
```bash
git tag v1.0.0
git push origin v1.0.0
# Subir binarios a GitHub Releases
```

## ✅ Salida
- `main` con parity suite 100% verde
- Binarios release en GitHub Releases (v1.0.0)
- Docs completas en `docs/`
- STATE.md → "F8: RELEASED v1.0.0"
- CHANGELOG.md con historial completo

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
Skills: orca-cli, orchestration.
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

## 6. Herramientas y skills referencia

| Herramienta/Skill | Cuándo usarla |
|---|---|
| **gauntlet-loop** | Siempre. Es el core del proyecto. Build → critic → fix |
| **loop-engineering** | Siempre. Budget, kill-switch, durable STATE.md |
| **orca-cli** | Siempre que se necesiten worktrees, terminales, handoffs |
| **orchestration** | Siempre que haya múltiples workers supervisados (DAGs, gates) |
| **mcp-scripting** | Para investigación con MCP tools (crates.io, docs, etc.) |
| **computer-use** | Para interactuar con apps de escritorio (server vanilla, clientes) |
| **MCP search** | Para buscar documentación, referencias, librerías |
| **MCP describe** | Para inspeccionar MCP tools disponibles |
| **subagent (reviewer)** | Para el CRITIC ciego en cada ronda |
| **subagent (debugger)** | Para root cause de bugs |
| **subagent (worker)** | Para construir piezas en paralelo |

## 7. Fuera de alcance

Combat 1.8 · mods Forge/Fabric · plugins Bukkit 100% (solo por capas) · minigames custom · FPS de cliente (ver BENCHMARKS.md)
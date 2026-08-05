# Neutron — Operaciones

> Cómo trabajamos: Orca ADE + Gauntlet Loop. v0.2 · 5 ago 2026. Este documento fusiona los antiguos ORCHESTRATION.md y PROMPTS.md.

## 1. Modelo de trabajo

Un humano (dueño del proyecto) + agentes coordinados por Orca ADE. El control de calidad es un **Gauntlet Loop** (verificado, §2): cada pieza se construye contra un **bar** real e innegociable y la juzga un **critic ciego** con contexto fresco. El loop-engineering (§3) es el contenedor: presupuestos como freno, estado durable y gates humanos.

```
TÚ (dueño)
  └─ LEAD (coordinador de fase: descompone, despacha, cobra evidencia)
       ├─ BUILDER (worktree A) ── pieza 1
       ├─ BUILDER (worktree B) ── pieza 2        (paralelo si no se solapan)
       └─ CRITIC (contexto limpio) ── inspecciona el artefacto REAL contra el bar
            PASS → merge vía CI verde
            FAIL → el gap más grande → al BUILDER → nueva ronda
```

## 2. Gauntlet Loop (el método de calidad — verificado)

**Origen**: Matt Shumer, "How to Run a Gauntlet Loop" (somethingbig.ai/gauntlet-loop, jul 2026), repo `mshumer/Claude-of-Duty`. Verificación cruzada: Decrypt, ThePromptIndex, We0. Ver RESEARCH.md §7.

**Pilares**:
1. **Un bar que el agente no puede esquivar con palabras.** La forma más fuerte: igualar o superar algo real — checksum de vanilla, benchmark, server real, test suite. Puede ser aspiracional: no tiene que ser alcanzable (ver lección abajo). Para Neutron, **vanilla es nuestro "Call of Duty"**.
2. **Se da el objetivo, no la implementación.** El lead dice qué debe ser verdad al terminar; el builder elige la ruta.
3. **El lead divide.** Piezas mínimas construibles y juzgables por separado; en paralelo si no se solapan.
4. **El builder nunca se autoevalúa.** El critic (agente distinto, contexto limpio, sin historia del builder) inspecciona el artefacto REAL: tests, logs, JSON, benchmarks — no el resumen. Blind A/B cuando se pueda (sin saber cuál es cuál).
5. **FAIL → el gap más grande.** El critic devuelve UNA cosa, la más importante; el builder la corrige. Ronda nueva con critic nuevo.
6. **Sin cap arbitrario de rondas.** Se para cuando: el bar gana · 2 rondas seguidas sin mejora · presupuesto agotado · lo paras tú.

**Lección del propio Shumer**: su critic nunca ganó contra Call of Duty real (3.59 → 5+/10). Eso es correcto: un bar inalcanzable tira el trabajo hacia arriba. El bar no se negocia: se negocia cuándo parar.

**Roles**: LEAD (orquesta, no construye) · BUILDER (construye, no se califica) · CRITIC (ciego, contexto fresco, juzga el artefacto real) · SMOOTHER (integra piezas separadas, resuelve conflictos, no rediseña).

## 3. Loop engineering (el contenedor)

- **workbench.md** (o STATE.md en la raíz): se lee al empezar cada iteración; se actualiza al terminar (unidad, PASS/FAIL, evidencia del critic, artefacto, presupuesto gastado). Es la memoria fuera del contexto del modelo.
- **Presupuestos como guardrail, no como cap de rondas**: cada tarea define max tokens / tiempo estimado ANTES de despachar. Al 80% solo reportar; al 100% salir con nota en workbench.md. Nada de subagentes "para parecer ocupado".
- **Worktrees**: una tarea por worktree; el merge a `main` solo pasa con critic PASS + CI verde.
- **Gates humanos**: credenciales, releases públicos, licencia y cambios de criterios/bars → `gate-create` o escalación. El loop no se auto-aprueba.
- **Kill-switch**: al 100% de presupuesto, el worker sale y reporta; el dueño decide si recarga.

## 4. Orca ADE (verificado — RESEARCH.md §7)

**Orca**: Agent Development Environment open-source (MIT) de Stably AI (YC). No es un modelo: lanza agentes CLI (OpenCode, Claude Code, Codex...) cada uno en su propio git worktree, con terminal, editor y diffs. Incluye CLI de orquestación: Run (namespace durable), Task (spec + dependencias + estado), Dispatch (intento en un terminal), Message (worker_done/escalation/question/heartbeat), Decision gates.

- Web: onorca.dev · Docs: onorca.dev/docs · Repo: github.com/stablyai/orca
- **Resolver el CLI**: variable `ORCA_CLI_COMMAND` si existe; dev checkout con `ORCA_DEV_REPO_ROOT` → `orca-dev`; Linux fuera de Orca → `orca-ide` (NUNCA `orca` desnudo en Linux: es el screen reader); Windows o dentro de Orca → `orca`.
- **Regla de oro**: `orca skills get orchestration` ANTES de orquestar — la superficie de comandos cambia entre releases; el repo de skills es la fuente versionada. Verificar con `orca status --json`.

Flujo base (verificar en la guía de tu versión):

```
orca orchestration run-create --objective "<objetivo>" --json
orca orchestration task-create --spec "<spec>" --task-title "<título>" --json
orca orchestration worker-start --task <taskId> --worktree <worktree> --agent <agente> --json
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 900000 --json
orca orchestration send --type worker_done --task-id <taskId> --dispatch-id <dispatchId> --outcome succeeded --json
orca orchestration gate-create --task <taskId> --question "<pregunta>" --options '["a","b"]' --json
```

Contrato con los workers: `worker_done` exactamente una vez (incluso en fallo) con task+dispatch IDs; heartbeat en trabajos largos; `ask` para preguntas bloqueantes; `@all/@idle/@<agente>` para direccionar.

## 5. Reglas no negociables

1. **El bar no se toca.** Criterios de aceptación y tests de paridad solo cambian con gate humano. Cambiar un test para que pase = trampa.
2. **Builder ≠ critic.** El que construye nunca se autoevalúa; el critic (contexto limpio) inspecciona el artefacto real; postura por defecto: REJECT hasta tener evidencia.
3. **Evidencia real, no afirmaciones.** Logs crudos con timestamps, salidas de comandos, checksums, salidas de bots, enlaces a reports. "Funciona" no es evidencia; se pega en `worker_done`.
4. **Sin cap arbitrario de rondas.** Se itera hasta que el bar gana, 2 rondas sin mejora, presupuesto agotado (guardrail de tokens/tiempo) o decisión humana. Kill-switch al 100%.
5. **Estado durable.** workbench.md/STATE.md se lee al empezar y se actualiza al terminar cada iteración.
6. **Isolación.** Una tarea por worktree; merge a `main` solo con critic PASS + CI verde.
7. **Gates humanos.** Credenciales, releases, licencia y criterios → escalar. El loop no se auto-aprueba.
8. **Regla de oro de Neutron.** Ninguna tarea de código se da por terminada sin su benchmark o parity test asociado en CI.

## 6. Operating manual (lo que todo agente debe saber antes de tocar servidores)

### Levantar servidores de referencia
- **Vanilla 26.2**: requiere **Java 25** (26.1+). `java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui` con `eula.txt=true`, `online-mode=false`, `level-seed=<fija>`, `view-distance=10`. Esperar la línea `Done (Xs)!` (regex `Done \(.*\)!`).
- **Paper**: última build (verificar soporte 26.x en el momento). Spark incluido: `/spark tps`, `/spark health`. **Rate limit de comandos ~15/s** → throttle de bots (sleep ≥ 80 ms entre comandos).
- **Pumpkin nightly**: binario de releases oficiales; config.toml (`online_mode = false`). No tiene Chunky → medir cps con método propio documentado.
- **Neutron**: `cargo run --release -p neutron-cli`; `neutron bench` para métricas.

### Bots (verificación E2E)
- **mineflayer** (Node.js): maduro, 1.8→1.21.11. `createBot({host,port,username,auth:'offline'})`; eventos `login` y `spawn` para medir join. **Quirk 1.20.2+**: `physicsEnabled: false` hasta `spawn` o kick. Sin proxies en tests.
- **azalea** (Rust): trackea 26.x — usar para 26.2.
- Verificación de mundo: `/seed` (debe coincidir), `/locate` para estructuras, exportar regiones `.mca` y comparar checksums xxHash64 contra golden data de vanilla.

### Métricas
Startup: regex `Done (Xs)!`; join: timestamps de login/spawn del bot; cps: Chunky (vanilla/Paper) o contador propio; TPS: spark o endpoint propio; RAM: RSS por OS (Linux `ps`, Windows `Get-Process`).

## 7. Plantilla de spec de tarea

```json
{
  "task-title": "T-X: <título>",
  "bar": "<la referencia real e innegociable: test, checksum, benchmark, server real>",
  "depends_on": [],
  "budget": { "max_tokens": 150000, "timeout_min": 120 },
  "spec": "<qué debe ser verdad al terminar, medible; contexto: archivos/URLs verificadas; restricciones>",
  "evidencia": "<logs crudos, hashes, outputs que se pegarán en worker_done>",
  "dod": "<qué ejecuta el critic desde cero para dar PASS>"
}
```

## 8. Work packages grandes (specs listas para desplegar)

### B0 — Harness de benchmarks (F0)
**Objetivo**: harness que mide vanilla/Paper/Pumpkin/Neutron con la metodología de BENCHMARKS.md.
**Alcance**: `bench/` completo (run.ps1/run.sh, bots join-bench, recolector JSON, tabla markdown autogenerada) + baseline B0 publicado.
**AC**: B0-1 corre de punta a punta en Windows y Linux; B0-2 startup vía regex `Done (Xs)!`, 5 runs, mediana; B0-3 10 bots join simultáneo en vanilla y Paper sin kicks, p95 < 5 s; B0-4 cps con Chunky (vanilla/Paper) consistente con baselines publicados ±30%; B0-5 JSON validado + tabla generada.
**Evidencia**: logs crudos en `bench/logs/`, JSON, tabla. **DoD**: el critic ejecuta el harness desde cero y reproduce B0-3/B0-4.

### P1 — Protocolo login/play 26.2 (F1)
**Objetivo**: handshake→status→login(offline)→play: join, keepalive, chat, movimiento, chunks (flat), block place/break.
**AC**: P1-1 bot real entra y recibe chunks; P1-2 `/seed` correcto; P1-3 colocar/romper confirmado por el bot y persistido en `.mca`; P1-4 keepalive 60 s sin kick; P1-5 fuzzing decode 1M inputs sin panic.
**Evidencia**: logs con timestamps, salida del bot, fuzz report. **DoD**: E2E S1 verde + fuzz limpio en CI.

### W1 — Worldgen paridad: overworld (F2)
**Objetivo**: density functions + noise + surface rules con checksums idénticos a vanilla.
**AC**: W1-1 hash xxHash64 de chunks (0,0)-(7,7) idéntico a vanilla en 50 seeds golden (CI); W1-2 benchmark `populate_noise_stage` < 20 ms/chunk (referencia: Pumpkin PR #2506 logró 18.8 ms); W1-3 sin alocaciones nuevas en hot path (dhat).
**Evidencia**: tabla seeds/hash, output criterion, reporte dhat. **DoD**: el critic regenera golden data con server vanilla y ejecuta la suite de checksum de cero.

### R1 — Redstone wire+torches parity (F3-A)
**Objetivo**: dust, torches, levers, buttons, doors con update order vanilla (PP: W,E,N,S,D,U / NC: W,E,D,U,N,S).
**AC**: R1-1 100 contraptions en 5 posiciones distintas → estado final y secuencia de updates idénticos a vanilla (comparación contra server vanilla real con bots); R1-2 sin diferencias posicionales.
**Evidencia**: suite verde + logs de comparación vanilla vs Neutron. **DoD**: suite R1 completa en CI contra golden contraptions.

### V1 — Pipeline de versiones D0-D4 (F1/F2)
**Objetivo**: `mc-extract` descarga jar 26.2 → registries+protocolo+worldgen JSON → codegen → compila sin diffs manuales.
**AC**: V1-1 JSON validado contra minecraft-data; V1-2 codegen regenera `neutron-data` y `neutron-protocol` sin diffs manuales; V1-3 CI completo < 10 min; V1-4 runbook D0-D4 con tiempos reales.
**Evidencia**: artefactos generados, diff vacío, timings. **DoD**: el critic borra los crates generados y los regenera desde cero.

### S1 — E2E smoke diario (F1)
**Objetivo**: test de humo automatizado diario: levantar, join, mover 100 bloques, romper/colocar, chat, medir TPS.
**AC**: S1-1 verde en CI diario; S1-2 p95 join < 2 s; S1-3 TPS ≥ 19.9 durante 5 min con 20 bots; S1-4 alerta automática si falla.
**Evidencia**: reporte CI + métricas JSON. **DoD**: 7 días consecutivos verdes sin intervención humana.

### PL1 — Plugin WASM mínimo (F6)
**Objetivo**: hello-plugin Rust→`wasm32-wasip2` que reacciona a "bloque colocado" y responde en chat, con permisos y fuel limit.
**AC**: PL1-1 corre en runtime wasmtime aislado; PL1-2 panic en el plugin NO tumba el servidor (test de crash automático); PL1-3 fuel 10M opcodes mata el plugin sin daño; PL1-4 hot reload sin reiniciar; PL1-5 coste en hot path < 5 µs/tick (criterion).
**Evidencia**: test de crash + logs + benchmark. **DoD**: el critic reproduce el crash test y el benchmark desde cero.

### Fan-out (misma tarea, 3 agentes, worktrees)
Para problemas de diseño donde queremos competencia real (p.ej. el region scheduler de F4): 3 agentes en 3 worktrees con el MISMO prompt entregan diseño + micro-benchmark criterion contra la alternativa base; el critic reproduce los 3 benchmarks; el humano decide con gate-create. Gana la mejor relación números/simplicidad.

## 9. Prompts por fase (copy-paste para el coordinador)

**Cómo lanzar**: GUI de Orca (worktree desde `main` + pegar como objetivo) · CLI (`orca orchestration run-create --objective "..."`) · agente suelto con la skill de orquestación.

**Normas comunes (bloque que va dentro de TODO prompt — no negociables)**:
1. Carga primero `orca skills get orchestration` y usa SOLO comandos de esa guía; verifica con `orca status --json`.
2. Un worktree por tarea; dos agentes nunca editan los mismos archivos a la vez.
3. Gauntlet Loop: el builder nunca se autoevalúa; el critic (contexto limpio) inspecciona el artefacto real; postura por defecto REJECT.
4. Evidencia real pegada en `worker_done` (logs crudos, hashes, outputs de bots).
5. El bar no se toca sin gate humano.
6. Sin cap arbitrario de rondas: se itera hasta que el bar gana, 2 rondas sin mejora, o presupuesto agotado. Al 100% salir con nota en workbench.md.
7. workbench.md se lee al empezar cada iteración y se actualiza al terminar.
8. Toda escalación se resuelve con gate-create o se eleva al dueño. Credenciales, releases públicos y cambios de criterios SIEMPRE pasan por él.
9. Reporte final: verdades verificadas vs suposiciones, workbench.md actualizado, qué falta para el siguiente hito.

---

### FASE F0 — Fundamentos y harness · bar: baseline reproducible por agente distinto en Windows y Linux; 10 bots p95 < 5 s; cps ±30%

```text
Eres el LEAD de la fase F0 de Neutron (servidor de Minecraft en Rust). Usa la skill
de orquestación de Orca y ejecuta un Gauntlet Loop completo (OPERATIONS.md §2-§5).

OBJETIVO: infraestructura del repo + primer baseline público de benchmarks
(vanilla 26.2 / Paper / Pumpkin) con la metodología de BENCHMARKS.md.

BAR (innegociable; lo inspecciona un critic ciego con contexto limpio):
- Un agente distinto al builder ejecuta bench/run.ps1 desde cero en Windows y Linux
  y reproduce el baseline B0.
- 10 bots de join simultáneo sin kicks, p95 < 5 s.
- cps ±30% consistente con los baselines publicados (C2ME).

SPLIT (piezas mínimas mejorables y calificables por separado; sugeridas):
harness + bots · CI/workspace · baseline B0 publicado.

CONTEXTO: README.md, ROADMAP.md (F0), BENCHMARKS.md, OPERATIONS.md §6.

PRESUPUESTO (guardrail): ~150k tokens por tarea; 1-2 semanas de calendario
orientativas (el bar manda, no la fecha). GATES: el baseline B0 se publica solo
con mi aprobación (gate-create). EVIDENCIA: logs en bench/logs/, JSON validado,
tabla markdown. REPORTE: números del baseline, máquina usada, round log PASS/FAIL.
```

### FASE F1 — Núcleo jugable · bar: bot vanilla 26.2 juega 10 min sin kick (E2E CI); mundo Anvil abre en vanilla y viceversa; fuzz 1M inputs sin panic; startup < 2 s; RAM < 200 MB

```text
Eres el LEAD de la fase F1 de Neutron. Usa la skill de orquestación de Orca y
ejecuta un Gauntlet Loop completo (OPERATIONS.md §2-§5).

OBJETIVO: un jugador real (cliente vanilla 26.2) entra, se mueve, chatea, rompe/
coloca bloques, y el mundo persiste en Anvil 100% compatible con vanilla.

BAR: bot vanilla 26.2 juega 10 min sin kick (E2E en CI); mundo guardado por Neutron
abre en vanilla SIN errores y viceversa; fuzz del decode 1M inputs sin panic;
startup < 2 s; RAM < 200 MB (mundo flat).

SPLIT (sugeridas): T-P1 protocolo login/play 26.2 (join, keepalive, chat, movimiento,
chunks flat, block place/break, /seed /tp /gamemode /save) · T-W1v0 world v1 (chunk
en memoria con palette, Anvil .mca read/write, level.dat, carpetas vanilla,
session.lock) · T-V1 pipeline de versiones v1 (mc-extract + codegen, sin diffs
manuales, CI < 10 min, runbook D0-D4) · T-S1 E2E smoke diario (p95 join < 2 s,
TPS ≥ 19.9 con 20 bots, alerta automática).

CONTEXTO: ROADMAP.md (F1); ARCHITECTURE.md §3, §4, §5.

PRESUPUESTO: ~150k tokens por tarea; 2-4 semanas orientativas. GATES: T-S1 pasa a
CI diario solo con mi aprobación. EVIDENCIA: logs con timestamps, salidas de bot,
fuzz report, diff de regeneración vacío.
```

### FASE F2 — Worldgen paridad 1:1 · bar: checksum xxHash64 idéntico en 50 seeds golden (0 mismatches); cps > 250 @16 hilos reproducido; mundo generado abre en vanilla con el mismo terreno

```text
Eres el LEAD de la fase F2 de Neutron. Usa la skill de orquestación de Orca y
ejecuta un Gauntlet Loop completo (OPERATIONS.md §2-§5).

OBJETIVO: misma seed → MISMO mundo que vanilla, verificado por checksums xxHash64
en CI, con rendimiento > 250 chunks/s @16 hilos.

BAR: checksum xxHash64 idéntico a vanilla en 50 seeds golden (0 mismatches);
benchmark cps > 250 @16 hilos reproducido; un mundo generado por Neutron abre en
vanilla con el mismo terreno.

SPLIT (sugeridas): T-G1 golden data pipeline (server vanilla headless genera
chunks de 50 seeds, checksums de bloques/biomas/light arrays, reproducible con un
comando, versión de MC anotada) · T-W1 density functions + noise (XORoshiro128
parity) + biome source + surface rules + carvers + placed features (criterion
populate_noise_stage < 20 ms/chunk; sin alocaciones nuevas en hot path) ·
T-W2 estructuras fase 1 (stronghold, aldeas, monumentos, ruinas; /locate coincide)
· T-P2 benchmark cps sostenido @16 hilos vs baseline B0.

CONTEXTO: ROADMAP.md (F2); ARCHITECTURE.md §6, §10.

PRESUPUESTO: ~150k tokens por tarea; 4-8 semanas orientativas. GATES: ninguna
optimización entra sin checksum verde (nadie toca el bar); el benchmark F2 se
publica con mi aprobación. EVIDENCIA: tabla seeds/hash, output criterion, dhat.
```

### FASE F3 — Simulación vanilla · bar: suite dorada posicional 100% contra server vanilla real; light arrays idénticos en 50 seeds; survival básica por bot

```text
Eres el LEAD de la fase F3 de Neutron. Usa la skill de orquestación de Orca y
ejecuta un Gauntlet Loop completo (OPERATIONS.md §2-§5).

OBJETIVO: comportamiento vanilla en simulación: bloques, fluidos, iluminación,
redstone (con suite posicional dorada), spawns y survival básica jugable.

BAR: suite dorada posicional 100% contra server vanilla real (bots); light arrays
idénticos a vanilla en las 50 seeds golden; survival básica jugable por bot.

SPLIT (sugeridas, en orden): T-L1 iluminación (engine propio estilo Starlight) ·
T-R1 redstone A (wire, torches, levers, buttons, doors; 100 contraptions en 5
posiciones, update order PP: W,E,N,S,D,U / NC: W,E,D,U,N,S) · T-R2 redstone B
(comparators, repeaters, observers, hoppers, TNT) · T-R3 redstone C (pistons +
quasi-connectivity + block swapping) · T-R4 redstone D (suite dorada completa en
CI) · T-F1 fluidos · T-SP1 spawns (ciclos, caps, pack spawning, luz/distancia) ·
T-SV1 survival básica (minar, craft, comer, dormir, morir/revivir).

CONTEXTO: ROADMAP.md (F3); ARCHITECTURE.md §7.

PRESUPUESTO: ~150k tokens por tarea; 6-12 semanas orientativas. GATES: las
sub-fases de redstone avanzan en orden A→B→C→D; cualquier desviación de vanilla
se documenta como bug conocido y pasa por gate humano. EVIDENCIA: suite verde +
logs de comparación vanilla vs Neutron por contraption.
```

### FASE F4 — Escala 500-1000+ · bar: 500 bots 60 min → TPS 20.0, p99 tick < 25 ms; RAM/jugador < 1 MB; sin regresión de cps/startup

```text
Eres el LEAD de la fase F4 de Neutron. Usa la skill de orquestación de Orca y
ejecuta un Gauntlet Loop completo (OPERATIONS.md §2-§5).

OBJETIVO: 500 jugadores simulados con TPS 20.0 y p99 tick < 25 ms; camino a 1000+.

BAR: 500 bots 60 min → TPS 20.0, p99 < 25 ms; RAM/jugador < 1 MB sobre base
< 150 MB; sin regresión de cps/startup (suite F2/F3 verde).

SPLIT (sugeridas): T-RS1 diseño del scheduler por regiones (FAN-OUT OBLIGATORIO:
3 agentes, 3 worktrees, mismo prompt — region-based ticking en Rust + bevy_ecs,
single-writer por región, determinismo intra-región, migración de entidades, sync
de redstone cross-región + micro-benchmark criterion vs tick global; gana la mejor
relación números/simplicidad; el critic reproduce los 3; yo decido con gate-create)
· T-O1 optimizaciones hot path (arenas, reuso de buffers, sin locks, batch de
paquetes; sin regresión de paridad, mejora medible de p99) · T-ST1 stress 500 bots
(TPS 20.0, p99 < 25 ms, 60 min sin degradación) · T-M1 memory profiling en CI
(dhat/heaptrack) idle/100/500 jugadores.

CONTEXTO: ROADMAP.md (F4); ARCHITECTURE.md §8.

PRESUPUESTO: ~150k tokens por tarea; 4-8 semanas orientativas. GATES: el diseño
de T-RS1 se adopta solo con gate humano; ninguna optimización entra sin suite de
parity verde. EVIDENCIA: outputs de stress, criterion, perfiles de memoria.
```

### FASE F5 — Mobs y AI · bar: E2E 20 min survival; spot-checks (creeper explota, zombie quema, enderman teletransporta, dragon loop); 50 mobs/chunk sin regresión TPS

```text
Eres el LEAD de la fase F5 de Neutron. Usa la skill de orquestación de Orca y
ejecuta un Gauntlet Loop completo (OPERATIONS.md §2-§5).

OBJETIVO: comportamiento vanilla de mobs: pasivos, hostiles y jefes (Ender Dragon
como hito), con combate completo.

BAR: E2E 20 min de survival con bot; spot-checks automatizados (creeper explota,
zombie quema al amanecer, enderman se teletransporta, dragon en loop de jefe);
50 mobs/chunk sin regresión de TPS.

SPLIT (sugeridas, por oleadas, port desde el jar sin ofuscar): T-AI1 pasivos
(vacas, cerdos, ovejas, gallinas, aldeanos con trading) · T-AI2 hostiles (zombie,
esqueleto, creeper, enderman, araña; drops vanilla) · T-AI3 jefes (Ender Dragon,
wither después) · T-AI4 combate completo (melee, arcos, escudos, tridentes,
encantamientos, estatus, knockback, cooldown, i-frames) · T-AI5 pathfinding A*
optimizado.

CONTEXTO: ROADMAP.md (F5); ARCHITECTURE.md §7.

PRESUPUESTO: ~150k tokens por tarea; 6-12 semanas orientativas. GATES: un mob se
marca done solo con spot-check automatizado (no con "parece que funciona");
prioridad estricta pasivos → hostiles → jefes. EVIDENCIA: logs estructurados por
mob + salida de bot + benchmarks.
```

### FASE F6 — Plugins WASM + Lua · bar: panic aislado; fuel 10M mata el plugin sin daño; hot reload; 3 conversiones reales; coste < 5 µs/tick

```text
Eres el LEAD de la fase F6 de Neutron. Usa la skill de orquestación de Orca y
ejecuta un Gauntlet Loop completo (OPERATIONS.md §2-§5).

OBJETIVO: ecosistema de plugins seguro por construcción: WASM (wasmtime + WIT) y
Lua, con convertidor honesto por capas y capa de compat PatchBukkit-style v0.

BAR: plugin WASM con panic no tumba el servidor; fuel 10M opcodes mata el plugin
sin daño; hot reload sin reiniciar; 3 conversiones reales de plugins Bukkit
simples; coste en hot path < 5 µs/tick.

SPLIT (sugeridas): T-PL1 runtime WASM mínimo (crash aislado, fuel, hot reload,
criterion) · T-PL2 neutron-plugin-api.wit completo (eventos, comandos, world,
entities, permissions; ABI estable) · T-LUA1 scripting Lua (mlua 0.12, límites de
memoria, sin acceso al host sin permiso) · T-CONV1 convertidor v1 (analizador
estático Java detecta reflection/class-loading; recompilación de plugins puros a
WASM; 3 plugins reales convertidos) · T-PB1 capa PatchBukkit-style v0 (runtime
JVM embebido → eventos Neutron; Essentials-lite carga y ejecuta comandos básicos;
coste publicado, se espera lento: es un puente) · T-DOC1 docs de desarrollo.

CONTEXTO: ROADMAP.md (F6); ARCHITECTURE.md §9, §12.

PRESUPUESTO: ~150k tokens por tarea; 4-8 semanas orientativas. GATES: la promesa
pública de compat Bukkit se comunica SOLO con mi aprobación (honestidad por
capas); T-CONV1 y T-PB1 son puentes, no el futuro. EVIDENCIA: test de crash, logs,
benchmarks, casos de conversión.
```

### FASE F7 — Bedrock · bar: cliente Bedrock juega 10 min; coexistencia Java+Bedrock verificada; TPS Java sin impacto

```text
Eres el LEAD de la fase F7 de Neutron. Usa la skill de orquestación de Orca y
ejecuta un Gauntlet Loop completo (OPERATIONS.md §2-§5).

OBJETIVO: clientes Bedrock (26.x) conectan al mismo mundo que los clientes Java.

BAR: cliente Bedrock real juega 10 min sin desconexión; coexistencia Java+Bedrock
verificada (se ven e interactúan); TPS Java sin impacto (bench comparativo).

SPLIT (sugeridas): T-BE1 protocolo Bedrock base (RakNet + login/play 26.x) ·
T-BE2 play básico (movimiento, chat, chunks, bloques) · T-BE3 mapeo de registries
Java↔Bedrock · T-BE4 coexistencia.

CONTEXTO: ROADMAP.md (F7); ARCHITECTURE.md §3.

PRESUPUESTO: ~150k tokens por tarea; 4-8 semanas orientativas. EVIDENCIA: logs de
sesión Bedrock, bench comparativo de TPS.
```

### FASE F8 — 1.0 · bar: parity suite completa verde en main; benchmarks públicos reproducibles en 2 máquinas; 72 h uptime 100 jugadores sin leak; fuzz 24 h limpio; binarios x86-64/ARM64

```text
Eres el LEAD de la fase F8 de Neutron. Usa la skill de orquestación de Orca y
ejecuta un Gauntlet Loop completo (OPERATIONS.md §2-§5).

OBJETIVO: release 1.0 estable, verificable y defendible, con benchmarks públicos
reproducibles.

BAR: parity suite completa verde en main; benchmarks públicos reproducibles en
2 máquinas; 72 h de uptime con 100 jugadores sin leak; fuzz 24 h limpio; binarios
Windows/Linux/macOS × x86-64/ARM64.

SPLIT (sugeridas): T-Q1 fuzzing continuo 24 h sin crash + audit de panics ·
T-Q2 audit de memory leaks (60 min idle estable) + heaptrack en 3 escenarios ·
T-BENCH1 benchmarks finales (vanilla/Paper/Pumpkin/Neutron) en 2 máquinas con
BENCHMARKS.md; publicación en bench/results/ · T-DOC2 guía de migración de mundos,
config, desarrollo de plugins · T-REL1 proceso de release + binarios + checklist
por versión de Mojang (SLA 7 días, D0-D4).

CONTEXTO: ROADMAP.md (F8); BENCHMARKS.md.

PRESUPUESTO: ~150k tokens por tarea; 4-8 semanas orientativas. GATES: el release
1.0 se publica SOLO con mi aprobación explícita; la lista de desviaciones
conocidas de parity se publica con transparencia. EVIDENCIA: reportes de fuzz/
audit, tablas de benchmarks, checksums de binarios.
```

## 10. Loops de automatización (no son fases: corren solos en CI)

| Loop | Frecuencia | Quién | Gatillo |
|---|---|---|---|
| T-S1 smoke E2E | diario | CI | cron |
| Benchmarks de regresión (cps, TPS, RAM) | semanal | CI + agente | cron |
| Pipeline de versiones D0-D4 (main = latest) | cada release de Mojang | CI + agente | webhook (SLA ≤ 7 días) |
| Fuzzing del protocolo | continuo | CI | cada merge a main |
| Suite de parity (checksums + contraptions) | cada merge | CI | PR |

Los agentes construyen los loops UNA vez (en su fase); después corren solos. La orquestación humana solo se necesita para fases de construcción, decisiones de diseño (fan-out) y gates de release.

## 11. Checklist antes de cada run

- [ ] `orca status --json` OK y `orca skills get orchestration` leído (superficie de comandos actual).
- [ ] Repo en git con `main` estable; worktree por tarea.
- [ ] workbench.md/STATE.md actualizado con el estado del último run.
- [ ] Specs con bar escrito (plantilla §7) — sin bar no hay tarea.
- [ ] Presupuesto definido (tokens/tiempo como guardrail; sin cap de rondas).
- [ ] Hardware de bench libre (nadie más corriendo benchmarks).
- [ ] Evidencia esperada definida (qué logs/hashes/outputs se pegarán en worker_done).
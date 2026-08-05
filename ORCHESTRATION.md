# Neutron — Orquestación con Orca ADE: prompts, tareas y reglas para agentes

> v0.1 · 5 ago 2026 · Cómo el equipo (humano + agentes) construye Neutron con Orca ADE.

## 1. Qué es Orca ADE (verificado)

**Orca** es un Agent Development Environment open-source (MIT) de **Stably AI** (Y Combinator). No es un modelo: es la *coordinación* — lanza agentes CLI (Claude Code, Codex, **OpenCode**, Grok, etc.) cada uno en su propio **git worktree** aislado, con terminal, editor y diffs. Incluye CLI de orquestación estructurada.

- Web: https://www.onorca.dev · Docs: https://www.onorca.dev/docs · Repo: https://github.com/stablyai/orca
- Modelo mental: *"Every task gets its own git worktree, its own agent terminal"* → crear → trabajar → revisar diff → ship → archivar.
- Orquestación CLI (experimental, activar en Settings): **Run** (namespace durable), **Task** (spec + dependencias + estado), **Dispatch** (intento de tarea en un terminal), **Message** (worker_done / escalation / question / heartbeat), **Decision gates** (pregunta que bloquea hasta resolver).

## 2. Setup del proyecto

1. `git init` + push del repo (Orca necesita un repo real para worktrees).
2. En Orca: añadir repo → crear worktree por tarea → elegir agente (OpenCode recomendado si usas este entorno; Claude Code/Codex alternativos).
3. Habilitar Orca CLI y orquestación experimental.
4. **SIEMPRE antes de orquestar**: ejecutar `orca skills get orchestration` — la superficie de comandos cambia entre releases; este repo de skills es la fuente versionada. Verificar con `orca status --json`.

Flujo base (documentado por Orca; adaptar a la versión instalada):

```
orca orchestration run-create --objective "<objetivo>" --json
orca orchestration task-create --spec "<spec con criterios>" --task-title "<título>" --json
orca orchestration worker-start --task <taskId> --worktree <worktree> --agent <agente> --json
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 900000 --json
orca orchestration send --type worker_done --task-id <taskId> --dispatch-id <dispatchId> --outcome succeeded --json
orca orchestration gate-create --task <taskId> --question "<pregunta>" --options '["a","b"]' --json
```

Contrato con los workers: enviar `worker_done` exactamente una vez (incluso en fallo), incluir task+dispatch IDs, heartbeat en trabajos largos, `ask` para preguntas bloqueantes, `@all/@idle/@<agente>` para direccionar.

## 3. Reglas del proyecto para AGENTES (no negociables)

Tomadas de loop-engineering (maker/checker, STATE.md, presupuestos, gates):

1. **Maker/checker**: el agente que implementa NUNCA se auto-verifica. Un verifier distinto (contexto limpio) revisa el artefacto real: corre los tests él mismo, pega la salida, y su postura por defecto es **REJECT hasta que haya evidencia**.
2. **Evidencia real, no afirmaciones**: "funciona" no es evidencia. Evidencia = logs crudos con timestamps, salidas de comandos, checksums, hashes de archivos, salidas de bots, enlaces a reports (spark/Chunky). La evidencia se pega en el mensaje de `worker_done`.
3. **STATE.md en la raíz**: al empezar cada iteración se lee; al terminar se actualiza (unidad, veredicto, evidencia, artefacto, gasto de presupuesto). Es la memoria fuera del contexto del modelo.
4. **Presupuesto con kill-switch**: cada tarea define max rounds / max tokens / max tiempo ANTES de empezar. Al 80% → solo reportar. Al 100% → salir con nota en STATE.md. Nada de "spawnear subagentes para parecer ocupado".
5. **Nadie toca el bar**: los criterios de aceptación y los tests de paridad solo se cambian con gate humano (verifier + humano). Cambiar un test para que pase = trampa.
6. **Isolación**: cada tarea en su worktree/branch; el merge a `main` solo pasa por el verifier y CI verde.
7. **Gates humanos**: credenciales, publicar releases, cambiar licencia, cambiar criterios → escalar (`escalation` / `gate-create`). El loop no se auto-aprueba.
8. **Regla de oro de Neutron**: ninguna tarea de código se da por terminada sin su benchmark o parity test asociado en CI (ver ROADMAP §0).

## 4. Operating manual para agentes (lo que todo agente debe saber antes de tocar servidores)

### Levantar servidores de referencia
- **Vanilla 26.2**: requiere **Java 25** (26.1+). `java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui` con `eula.txt=true`, `online-mode=false`, `level-seed=<fija>`, `view-distance=10`. Esperar la línea `Done (Xs)!` (regex `Done \(.*\)!`).
- **Paper**: última build (verificar soporte 26.x en el momento). Spark incluido: `/spark tps`, `/spark health`. **Rate limit de comandos ~15/s** → throttle de bots (sleep ≥ 80 ms entre comandos).
- **Pumpkin nightly**: binario de releases oficiales; config.toml (`online_mode = false`). No tiene Chunky → medir cps con método propio documentado.
- **Neutron**: `cargo run --release -p neutron-cli`; `neutron bench` para métricas.

### Bots (verificación E2E)
- **mineflayer** (Node.js): maduro, 1.8→1.21.11. `createBot({host,port,username,auth:'offline'})`; eventos `login` y `spawn` para medir join. **Quirk 1.20.2+**: `physicsEnabled: false` hasta `spawn` o kick. Sin proxies en tests.
- **azalea** (Rust): trackea 26.x (mc26.1+) — usar para 26.2.
- Verificación de mundo: `/seed` (debe coincidir), `/locate` para estructuras, exportar chunks (guardar región `.mca`) y comparar checksums xxHash64 contra golden data de vanilla.
- "Ver lo que hay": el bot puede consultar bloques (`blockAt` en mineflayer), chatear, moverse; para evidencia visual usar screenshots del servidor-cliente solo si es imprescindible (el harness prioriza datos estructurados).

### Métricas
- Startup: regex `Done (Xs)!`; join: timestamps de login/spawn del bot; cps: Chunky (vanilla/Paper) o contador propio; TPS: spark o endpoint propio; RAM: RSS por OS (Linux `ps`, Windows `Get-Process`).

> Para el prompt copy-paste de cada fase del roadmap (F0-F8) ve a **[PROMPTS.md](PROMPTS.md)**.

## 5. Plantillas de prompts

### 5.1 Prompt de coordinador (run-level, en español)

```
Eres el coordinador del proyecto Neutron. Objetivo de esta run: <OBJETIVO>.

1. Descompón en tareas con task-create (spec con criterios de aceptación medibles
   y dependencias explícitas entre tareas).
2. Despacha con worker-start asignando el agente adecuado a cada tarea
   (una tarea por worktree; sin solapamientos de archivos).
3. Espera resultados con check --wait (worker_done, escalation, question).
4. Ante una question/escalation: resuélvela con gate-create o escalándola a mí;
   nunca la ignores.
5. Reglas: (a) ninguna tarea se marca done sin evidencia pegada (logs, hashes,
   outputs de bots); (b) nadie modifica criterios de aceptación ni tests de
   paridad sin gate humano; (c) al final entrega: resumen de verdades verificadas
   vs suposiciones, STATE.md actualizado, y qué falta para el siguiente hito.
```

### 5.2 Plantilla de tarea (spec genérica)

```
Tarea: <título>
Objetivo: <qué debe ser verdad al terminar, en una frase medible>
Contexto: <archivos/librerías/fuentes relevantes — pegar URLs verificadas>
Restricciones: <crates permitidos, estilo, no tocar X, presupuesto de rounds/tokens>
Criterios de aceptación (todos medibles):
- AC1: <comando o test concreto + umbral>
- AC2: <...>
Evidencia requerida en worker_done: <logs crudos, hashes, outputs>
Definition of done: <test/benchmark que el verifier correrá de cero>
```

## 6. Tareas grandes (work packages) listas para desplegar

### B0 — Harness de benchmarks (F0)
**Objetivo**: el harness que mide vanilla/Paper/Pumpkin/Neutron con la metodología de BENCHMARKS.md.
**Alcance**: `bench/` completo (run.ps1/run.sh, bots join-bench, recolector JSON, tabla markdown autogenerada) + baseline B0 publicado.
**AC**: B0-1: corre de punta a punta en Windows y Linux; B0-2: startup vía regex `Done (Xs)!`, 5 runs, mediana; B0-3: 10 bots join simultáneo en vanilla y Paper sin kicks, p95 < 5 s; B0-4: cps con Chunky (vanilla/Paper) consistente con baselines publicados ±30%; B0-5: JSON validado + tabla markdown generada.
**Evidencia**: logs crudos en `bench/logs/`, JSON, tabla.
**DoD**: el verifier ejecuta el harness desde cero y reproduce B0-3/B0-4.

### P1 — Protocol login/play 26.2 (F1)
**Objetivo**: handshake→status→login(offline)→play: join, keepalive, chat, movimiento, chunks (flat), block place/break.
**AC**: P1-1: bot real entra y recibe chunks; P1-2: `/seed` responde correcto; P1-3: colocar/romper bloque confirmado por el bot y persistido en `.mca`; P1-4: keepalive 60 s sin kick; P1-5: fuzzing decode 1M inputs sin panic.
**Evidencia**: logs con timestamps, salida del bot, fuzz report.
**DoD**: E2E S1 verde + fuzz limpio en CI.

### W1 — Worldgen paridad: overworld (F2)
**Objetivo**: density functions + noise + surface rules con checksums idénticos a vanilla.
**AC**: W1-1: hash xxHash64 de chunks (0,0)-(7,7) idéntico a vanilla en 50 seeds golden (CI); W1-2: benchmark criterion `populate_noise_stage` < 20 ms/chunk (referencia: Pumpkin PR #2506 logró 18.8 ms); W1-3: sin alocaciones nuevas en hot path (dhat).
**Evidencia**: tabla seeds/hash, output criterion, reporte dhat.
**DoD**: verifier regenera golden data con server vanilla y ejecuta la suite de checksum de cero.

### R1 — Redstone wire+torches parity (F3-A)
**Objetivo**: dust, torches, levers, buttons, doors con update order vanilla (PP: W,E,N,S,D,U / NC: W,E,D,U,N,S).
**AC**: R1-1: 100 contraptions de test en 5 posiciones distintas → estado final y secuencia de updates idénticos a vanilla (comparación contra server vanilla real con bots); R1-2: sin diferencias posicionales (el mismo test en 5 coordenadas da el mismo resultado que vanilla en esas coordenadas).
**Evidencia**: suite verde + logs de comparación vanilla vs Neutron.
**DoD**: suite R1 completa en CI comparando contra golden contraptions.

### V1 — Pipeline de versiones D0-D4 (F1/F2)
**Objetivo**: `mc-extract` descarga jar 26.2 → registries+protocolo+worldgen JSON → codegen → compila sin diffs manuales.
**AC**: V1-1: `cargo run -p mc-extract -- --version 26.2 --out data/26.2` produce JSON validado contra minecraft-data; V1-2: codegen regenera `neutron-data` y `neutron-protocol` sin diffs manuales; V1-3: CI completo < 10 min; V1-4: runbook documentado (D0-D4 con tiempos reales).
**Evidencia**: artefactos generados, diff vacío, timings.
**DoD**: verifier borra los crates generados y los regenera desde cero con el pipeline.

### S1 — E2E smoke diario (F1)
**Objetivo**: test de humo automatizado diario: levantar, join, mover 100 bloques, romper/colocar, chat, medir TPS.
**AC**: S1-1: verde en CI diario; S1-2: p95 join < 2 s; S1-3: TPS ≥ 19.9 durante 5 min con 20 bots; S1-4: alerta automática si falla.
**Evidencia**: reporte CI + métricas JSON.
**DoD**: 7 días consecutivos verdes sin intervención humana.

### PL1 — Plugin WASM mínimo (F6)
**Objetivo**: hello-plugin Rust→`wasm32-wasip2` que reacciona a "bloque colocado" y responde en chat, con permisos y fuel limit.
**AC**: PL1-1: corre en runtime wasmtime aislado; PL1-2: panic dentro del plugin NO tumba el servidor (test de crash automático); PL1-3: fuel limit 10M opcodes mata el plugin sin daño; PL1-4: hot reload sin reiniciar; PL1-5: coste en hot path < 5 µs/tick (criterion).
**Evidencia**: test de crash + logs + benchmark.
**DoD**: verifier reproduce el crash test y el benchmark desde cero.

## 7. Ejemplo de fan-out (misma tarea, 3 agentes, worktrees)

Para problemas de diseño donde queremos competencia real (p.ej. el region scheduler de F4):

```
Problema: diseñar el scheduler por regiones de Neutron para 1000+ jugadores.
Cada agente (worktree propio, mismo prompt):
  1. Propón una arquitectura de region-based ticking en Rust + bevy_ecs:
     single-writer por región, 20 TPS global, determinismo intra-región,
     migración de entidades entre regiones, sync de redstone cross-región.
  2. Entrega: docs/adr/region-scheduler.md + micro-benchmark criterion
     comparando tu propuesta contra tick global (mismo escenario sintético).
  3. Criterios: p99 tick, RAM por región, complejidad (líneas de código),
     riesgos identificados. Evidencia: benchmark criterion + diseño.
Gana el que tenga mejores números con el diseño más simple; el verifier
reproduce los benchmarks de los 3 y el humano decide con gate-create.
```

## 8. Checklist antes de cada run de orquestación

- [ ] `orca status --json` OK y `orca skills get orchestration` leído (superficie de comandos actual).
- [ ] Repo en git con `main` estable; worktree por tarea.
- [ ] STATE.md actualizado con el estado del último run.
- [ ] Criterios de aceptación escritos (plantilla §5.2) — sin ACs no hay tarea.
- [ ] Presupuesto definido (rounds/tokens/timeout).
- [ ] Hardware de bench libre (nadie más corriendo benchmarks).
- [ ] Evidencia esperada definida (qué logs/hashes/outputs se pegarán en worker_done).
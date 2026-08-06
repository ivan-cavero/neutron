# AGENTS.md — Neutron: cómo trabajamos

> v0.4 · 5 ago 2026 · **pi lee este archivo automáticamente** al trabajar en el repo. Trabajo 100% con pi por ahora.

## 0. DÓNDE SE ESCRIBE TODO (regla de carpeta — no negociable)

**La única carpeta de trabajo es la raíz de este repositorio: el directorio de trabajo actual, donde vive este AGENTS.md.** Ahí se ejecuta todo y ahí vive el código, los documentos y los runs. No se asume ninguna ruta absoluta fija: la ruta concreta es la del equipo en el momento de ejecutar.

- Prohibido crear, escribir o editar archivos fuera del directorio de trabajo actual: carpetas de herramientas externas (tipo `...\orca\workspaces\...`), carpetas temporales, otras unidades.
- **EXCEPCIÓN**: los **worktrees de Git gestionados por Orca** (`orca worktree create`) están permitidos porque son clonaciones ligeras del mismo repositorio, gestionadas por Orca, y no contienen datos externos. Siempre que uses `orca worktree create`, estás dentro de la regla.
- Si encuentras trabajo en una ruta externa, se trae al proyecto y se borra lo externo.
- Si algo no se puede hacer en el proyecto (por ejemplo, una herramienta exige otra ruta), se pregunta al humano antes de tocar nada.

## 1. Modelo de trabajo

### Flujo normal (sin Orca)
Un agente pi hace de **LEAD**: lee el estado, genera el run (con el prompt de §6), ejecuta las tareas y entrega evidencia. Todo el trabajo ocurre en la carpeta del proyecto (regla §0). La calidad la asegura un **Gauntlet Loop**: cada pieza se construye contra un **bar** y la juzga un **critic** lanzado como subagente con contexto limpio.

```
LEAD (pi)
  ├─ construye cada pieza (builder)
  └─ critica (subagente con contexto limpio, inspecciona lo real)
       PASS → siguiente pieza
       FAIL → el gap más grande → reconstruir → repetir
```

### Flujo con Orca (multi-agente, worktrees, orchestration)
Cuando se necesita más potencia, se usan **Orca CLI** y **Orca Orchestration** para distribuir el trabajo entre múltiples agentes y terminales:

```
LEAD (pi en worktree main)
  ├── ORCA: create worktree + launch codex → worker harness
  ├── ORCA: create worktree + launch codex → worker bots
  ├── ORCA: create worktree + launch codex → worker benchmarks
  └── ORCA: orchestration run + tasks + DAG
        ├── check --wait worker_done
        ├── gate-create (gate humano)
        └── worker-release
```

#### Orca CLI (gestión de worktrees y terminales)
- **Worktrees**: cada builder tiene su propio worktree aislado
  - `orca worktree create --name <task> --no-parent --agent codex --setup run`
  - `orca worktree set --worktree active --comment "F0: <estado>"`
- **Terminals**: cada worker tiene su terminal
  - `orca terminal create --worktree active --command "codex"`
  - `orca terminal send --text "<prompt>" --enter`
  - `orca terminal read --terminal <handle>`

#### Orca Orchestration (coordinación multi-agente)
- **Runs**: namespace para la fase actual
  - `orca orchestration run-create --objective "<objective>"`
- **Tasks**: unidades de trabajo con dependencias
  - `orca orchestration task-create --spec "<spec>" --deps '[]'`
- **Workers**: agentes que ejecutan tasks
  - `orca orchestration worker-start --task <id> --worktree new-child`
  - `orca orchestration check --wait --types worker_done,escalation,question`
- **Decision gates**: aprobaciones humanas
  - `orca orchestration gate-create --task <id> --question "¿Aprobado?"`
  - `orca orchestration gate-resolve --id <gate_id> --resolution "approved"`

#### Prompt para lanzar un run con Orca
Copia el prompt completo de la fase desde **ROADMAP.md** (§2, Fases). Cada prompt incluye:
1. El objetivo y el bar
2. La secuencia de comandos Orca CLI para crear worktrees
3. La secuencia de Orca Orchestration para tasks y workers
4. Las reglas del Gauntlet Loop (builder + critic)

> **Nota**: Los worktrees de Orca son worktrees de Git gestionados por Orca, no carpetas externas. Son parte del proyecto y están permitidos por la regla §0 porque Orca los gestiona de forma centralizada.

## 2. Gauntlet Loop (lo esencial)

- **Bar**: referencia real e innegociable — checksum de vanilla, benchmark, server real, test suite. Vanilla es nuestro "Call of Duty": no se negocia, se cumple o no. Puede ser inalcanzable: eso es correcto, tira el trabajo hacia arriba.
- **Builder nunca se autoevalúa**: el critic (subagente, contexto limpio, sin la historia del builder) inspecciona el artefacto REAL — logs, JSON, tests ejecutados por él mismo — no el resumen.
- **FAIL → el gap más grande**: el critic devuelve UNA cosa, la más importante; se corrige y se repite.
- **Sin cap arbitrario de rondas**: se para cuando el bar gana, 2 rondas sin mejora, o presupuesto agotado.
- Origen verificado: Matt Shumer, "How to Run a Gauntlet Loop" (jul 2026) — ARCHITECTURE.md (Anexo A, §7).

## 3. Reglas no negociables

1. **El bar no se toca.** Criterios y tests de paridad solo cambian con tu aprobación. Cambiar un test para que pase = trampa.
2. **Builder ≠ critic.** Postura por defecto del critic: REJECT hasta tener evidencia.
3. **Evidencia real, no afirmaciones.** Logs crudos con timestamps, hashes, salidas de bots, enlaces a reports. "Funciona" no es evidencia.
4. **Presupuesto como guardrail.** Tokens/tiempo estimado por tarea antes de empezar; al 80% solo reportar; al 100% salir con nota en el run.
5. **Regla de oro de Neutron.** Ninguna tarea de código se da por terminada sin su benchmark o parity test asociado en CI.
6. **Gates humanos.** Releases, credenciales y cambios de criterios pasan por ti.

## 4. Operating manual (lo esencial)

- **Vanilla 26.2**: Java 25. `java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui` con `eula.txt=true`, `online-mode=false`, `level-seed=<fija>`, `view-distance=10`. Arranque = línea `Done (Xs)!`.
- **Paper**: última build (verificar soporte 26.x). Spark incluido (`/spark tps`). Rate limit de comandos ~15/s → throttle de bots (sleep ≥ 80 ms).
- **Pumpkin nightly**: binario de releases oficiales; `config.toml` con `online_mode = false`. No tiene Chunky → cps con método propio.
- **Neutron**: `cargo run --release -p neutron-cli`.
- **Bots**: mineflayer (Node, ≤ 1.21.11; quirk 1.20.2+: `physicsEnabled: false` hasta `spawn`) · azalea (Rust, 26.x — usar para 26.2).
- **Métricas**: startup regex `Done (Xs)!` · join timestamps de login/spawn · cps Chunky (vanilla/Paper) o contador propio · TPS spark/endpoint · RAM RSS por OS.

## 5. Formato de tarea (markdown simple)

```markdown
### T1 — <título>
- Qué: <qué debe ser verdad al terminar, medible>
- AC: <criterios concretos con umbrales>
- Evidencia: <logs, hashes, salidas que se pegarán>
- DoD: <qué ejecuta el critic desde cero para dar PASS>
```

## 6. Cómo lanzar un run

### Método 1: Prompt completo desde ROADMAP.md ★ RECOMENDADO

1. Abre **ROADMAP.md** → §2 Fases
2. Copia el bloque `=== PROMPT F<NNN> ===` de la fase que toca **COMPLETO**
3. Pégalo en pi
4. Pi automáticamente:
   - Carga todas las skills necesarias (gauntlet-loop, loop-engineering, orca-cli, orchestration, mcp-scripting)
   - Lee STATE.md, workbench.md, runs/
   - Usa MCP tools para investigar referencias externas
   - Crea worktrees aislados con `orca worktree create`
   - Crea Run de Orquestación con DAG de tasks
   - Lanza workers en paralelo
   - Ejecuta el Gauntlet Loop (builder → critic → fix)
   - Actualiza workbench.md, STATE.md, runs/run-NNN.md

### Método 2: Prompt mínimo (para runs simples sin Orca)

```text
Eres el LEAD del proyecto Neutron. Prepara el siguiente run de trabajo.

PASO 1 — Lee el estado:
- STATE.md → fase actual y progreso
- runs/README.md y runs/run-*.md → historial
- ROADMAP.md → fase actual, su bar completo
- README.md, AGENTS.md → contexto y reglas
- workbench.md → rondas anteriores

PASO 2 — Genera:
- Si el bar de la fase actual está cumplido (evidencia en historial) → avanza la fase y plantea el siguiente
- Si no → genera run-NNN.md con objetivo, bar, tareas (1-5 con AC, evidencia y DoD)

PASO 3 — Registra:
- Crea runs/run-NNN.md, actualiza STATE.md
- Termina con: "Eres el LEAD de este run. Ejecuta el Gauntlet Loop: construye cada tarea y lanza un subagente critic con contexto limpio. No te autoevalúes."
```

### Método 3: Manual (tú controlas cada paso)

1. Tú decides qué run toca y me lo dices
2. Ejemplo: "Lanza F0. El bar es: baseline B0 reproducible, 10 bots p95 < 5 s, cps ±30%"
3. Pi ejecuta el Gauntlet Loop (builder → critic → fix → repetir)
4. Tú supervisás los gates humanos (releases, credenciales, cambios de bar)

### Referencia rápida de comandos Orca

| Acción | Comando |
|--------|--------|
| Ver estado | `orca status --json` |
| Listar worktrees | `orca worktree ps --json` |
| Crear worktree | `orca worktree create --name X --no-parent --agent codex --prompt "..." --setup run --json` |
| Actualizar comentario | `orca worktree set --worktree active --comment "F0: estado" --json` |
| Crear Run | `orca orchestration run-create --objective "..." --json` |
| Crear Task | `orca orchestration task-create --spec "..." --deps '[]' --json` |
| Lanzar worker | `orca orchestration worker-start --task <id> --worktree new-child --agent codex --json` |
| Esperar worker | `orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 900000 --json` |
| Gate humano | `orca orchestration gate-create --task <id> --question "..." --json` |
| Liberar worker | `orca orchestration worker-release --dispatch <id> --json` |

## 7. Loops de automatización (corren solos en CI)

| Loop | Frecuencia | Gatillo |
|---|---|---|
| Smoke E2E (levantar, join, mover, romper, chat, TPS) | diario | cron |
| Benchmarks de regresión (cps, TPS, RAM) | semanal | cron |
| Pipeline de versiones D0-D4 (main = última de Mojang, ≤ 7 días) | cada release | webhook |
| Fuzzing del protocolo | continuo | cada merge a main |
| Suite de parity (checksums + contraptions) | cada merge | PR |

Los agentes construyen cada loop UNA vez (en su fase); después corre solo.
# AGENTS.md — Neutron: cómo trabajamos

> v0.7 · 9 ago 2026 · **ZCode lee este archivo automáticamente** al trabajar en el repo.

## 0. DÓNDE SE ESCRIBE TODO (regla de carpeta — no negociable)

**La única carpeta de trabajo es la raíz de este repositorio: el directorio de trabajo actual, donde vive este AGENTS.md.** Ahí se ejecuta todo y ahí vive el código, los documentos y los runs. No se asume ninguna ruta absoluta fija: la ruta concreta es la del equipo en el momento de ejecutar.

- Prohibido crear, escribir o editar archivos fuera del directorio de trabajo actual: carpetas de herramientas externas, carpetas temporales, otras unidades.
- Si encuentras trabajo en una ruta externa, se trae al proyecto y se borra lo externo.
- Si algo no se puede hacer en el proyecto (por ejemplo, una herramienta exige otra ruta), se pregunta al humano antes de tocar nada.

## 1. Modelo de trabajo

### Flujo normal (ZCode)
Un agente ZCode hace de **LEAD**: lee el estado, genera el run (con el prompt de §6), ejecuta las tareas y entrega evidencia. Todo el trabajo ocurre en la carpeta del proyecto (regla §0). La calidad la asegura un **Gauntlet Loop**: cada pieza se construye contra un **bar** y la juzga un **critic** lanzado como subagente con contexto limpio.

```
LEAD (pi)
  ├─ construye cada pieza (builder)
  └─ critica (subagente con contexto limpio, inspecciona lo real)
       PASS → siguiente pieza
       FAIL → el gap más grande → reconstruir → repetir
```

### Flujo multi-agente (ZCode)
Cuando se necesita más potencia, ZCode distribuye el trabajo entre **subagentos** usando la herramienta `Agent`:

```
LEAD (pi)
  ├── Agent(subagente "builder-harness") → construye harness
  ├── Agent(subagente "builder-bots") → construye bots
  ├── Agent(subagente "builder-bench") → construye benchmarks
  └── TodoWrite → tracking de tareas + dependencias
        ├── TaskOutput → esperar resultado de cada subagente
        ├── AskUserQuestion → gates humanos
        └── Actualizar STATE.md
```

#### Herramientas ZCode para multi-agente
- **Subagentos** (tool `Agent`): cada builder/critic corre en contexto aislado
  - `Agent(subagent_type="general-purpose", prompt="...")` — construye o critica
  - `Agent(subagent_type="Explore", prompt="...") — solo lectura (búsqueda)`
  - `run_in_background: true` — ejecuta async, notificación al terminar
- **Tracking** (tool `TodoWrite`): cada tarea tiene estado (pending/in_progress/completed)
- **Gates humanos** (tool `AskUserQuestion`): preguntas directas al humano
- **Bash**: comandos directos (cargo build, java, etc.)

#### Cómo lanzar un run con ZCode
Copia el prompt completo de la fase desde **ROADMAP.md** (§2, Fases). Cada prompt incluye:
1. El objetivo y el bar
2. Las tareas a distribuir entre subagentos
3. Las reglas del Gauntlet Loop (builder + critic)

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
- **Neutron**: `cargo run --release -p neutron-server -- --seed 12345 --view-distance 8`. (`neutron-cli` no existe todavía.)
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
4. ZCode automáticamente:
   - Lee STATE.md, workbench.md, runs/
   - Usa MCP tools para investigar referencias externas
   - Lanza subagentos (Agent tool) para cada pieza en paralelo
   - Usa TodoWrite para tracking de tareas
   - Ejecuta el Gauntlet Loop (builder → critic → fix)
   - Actualiza workbench.md, STATE.md, runs/run-NNN.md

### Método 2: Prompt mínimo (runs simples)

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
2. Ejemplo: "Lanza F1. El bar es: protocolo 26.2 funcional, mundo Anvil, fuzz 1M inputs, E2E 10 min"
3. ZCode ejecuta el Gauntlet Loop (builder → critic → fix → repetir)
4. Tú supervisás los gates humanos (releases, credenciales, cambios de bar)

### Referencia rápida de herramientas ZCode

| Acción | Herramienta |
|--------|------------|
| Subagente builder | `Agent(subagent_type="general-purpose", prompt="...")` |
| Subagente lector | `Agent(subagent_type="Explore", prompt="...")` |
| Tracking de tareas | `TodoWrite(todos=[...])` |
| Gate humano | `AskUserQuestion(questions=[...])` |
| Bash (build, test, run) | `Bash(command="...")` |
| Leer archivos | `Read(file_path="...")` |
| Escribir archivos | `Write(file_path="...", content="...")` |
| Editar archivos | `Edit(file_path="...", old_string="...", new_string="...")` |
| Esperar subagente async | `TaskOutput(task_id="...", block=true)` |

### Ejemplos prácticos (de sesiones reales)

#### Lanzar 2 tareas en paralelo
```python
# T1 y T2 son independientes → lanzar ambos async
Agent(run_in_background=true, prompt="Implementa crate A: ...")
Agent(run_in_background=true, prompt="Implementa crate B: ...")
# ZCode notifica cuando cada uno termina
```

#### Lanzar 4+ tareas en paralelo (modo productivo)
```python
# Para fases grandes, lanzar todo lo independiente junto
Agent(run_in_background=true, prompt="T1: noise calibration ...")
Agent(run_in_background=true, prompt="T2: NBT format fix ...")
Agent(run_in_background=true, prompt="T3: cave carving ...")
Agent(run_in_background=true, prompt="T4: biome source ...")
# Cada uno trabaja en archivos distintos (no se pisan)
```

#### Gate humano entre fases
```python
# Después de que terminan los subagentes, preguntar al humano
AskUserQuestion(questions=[{
  "question": "¿Aprobás T1 y T2 para continuar?",
  "header": "Gate F1",
  "options": [
    {"label": "Aprobado", "description": "Continuar con Siguiente paso"},
    {"label": "Requiere fixes", "description": "Hay problemas que corregir"}
  ]
}]
```

#### Tracking de tareas
```python
TodoWrite(todos=[
  {"content": "T1 — Protocolo 26.2", "status": "completed", "priority": "high"},
  {"content": "T2 — Mundo Anvil", "status": "in_progress", "priority": "high"},
  {"content": "T3 — Fuzz decode", "status": "pending", "priority": "medium"},
])
```

#### Patrón típico de un run completo
```
1. Leer STATE.md, ROADMAP.md, runs/
2. Crear tracking con TodoWrite
3. Lanzar tareas independientes en paralelo (Agent run_in_background)
4. Esperar resultados (ZCode notifica automáticamente)
5. Gate humano (AskUserQuestion)
6. Lanzar siguientes tareas (con dependencias)
7. Crític ciego: lanzar subagente con contexto limpio para verificar
8. Actualizar STATE.md, crear runs/run-NNN.md
9. Guardar memories
```

#### Crític ciego (Gauntlet Loop)
```python
# El critic NUNCA ve el código del builder — contexto limpio
Agent(subagent_type="general-purpose",
  prompt="Eres el CRITIC ciego. Inspecciona crates/neutron-worldgen/:
  1. Ejecuta cargo test
  2. Compara resultados contra el bar
  3. Devuelve PASS o FAIL con el gap más grande
  REJECT por defecto hasta tener evidencia.")
```

## 7. Loops de automatización (ZCode CronCreate)

| Loop | Frecuencia | Gatillo |
|---|---|---|
| Smoke E2E (levantar, join, mover, romper, chat, TPS) | diario | `CronCreate` |
| Benchmarks de regresión (cps, TPS, RAM) | semanal | `CronCreate` |
| Pipeline de versiones D0-D4 (main = última de Mojang, ≤ 7 días) | cada release | manual |
| Fuzzing del protocolo | continuo | cada merge a main |
| Suite de parity (checksums + contraptions) | cada merge | PR |

Los agentes construyen cada loop UNA vez (en su fase); después corre solo via `CronCreate`.
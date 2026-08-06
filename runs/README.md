# Runs — Historial de ejecuciones

> Cada run se registra aquí con su objetivo, tareas, evidencia y resultado. Formato: `run-NNN.md`.

## Formato de run

Cada `run-NNN.md` sigue esta estructura:

```markdown
# run-NNN — <título>

## Objetivo
Una frase con lo que debe ser verdad al terminar.

## Bar
Criterios medibles (del ROADMAP.md).

## Tareas
### T1 — <título>
- Qué: medible
- AC: criterios concretos
- Evidencia: logs, hashes, salidas
- DoD: qué ejecuta el critic

### T2 — ...
...

## Evidencia
(Se pegan aquí los logs crudos con timestamps, hashes, salidas de bots, etc.)

## Resultado
PASS / FAIL (parcial) / BLOCKED

## Rounds
- R1: T1 PASS, T2 FAIL (motivo)
- R2: T2 corregido → PASS
```

## Historial

| Run | Fase | Resultado | Fecha |
|---|---|---|---|
| run-000 | Fundación | TERMINADO | 5 ago 2026 |
| run-001 | F0 | | |
| run-002 | F0 | | |
| ... | ... | ... | ... |

## Cómo generar un run

Copia el prompt de la fase correspondiente desde **ROADMAP.md** (sección 2, "Fases") y pégalo en pi. Pi:

1. Lee STATE.md → decide qué run toca
2. Crea `runs/run-NNN.md` con el formato arriba
3. Lanza el Gauntlet Loop (builder → critic → fix → repetir)
4. Actualiza STATE.md y el historial cuando termine

### Orquestación (qué usa pi detrás de cámaras)

Pi usa **Orca CLI** y **Orca Orchestration** para:

- **Worktrees**: cada builder corre en su propio worktree aislado
  - `orca worktree create --name <task> --no-parent --agent codex --setup run`
- **Terminals**: cada worker tiene su terminal gestionada por Orca
  - `orca terminal create --worktree id:<repoId>::<path> --command "codex"`
  - `orca terminal send --text "<prompt>" --enter`
  - `orca terminal read --terminal <handle>`
- **Orquestación**: DAG de tasks, worker_done, decision gates
  - `orca orchestration run-create --objective "<objective>"`
  - `orca orchestration task-create --spec "<spec>" --deps '["task_a"]'`
  - `orca orchestration worker-start --task <task_id> --worktree new-child`
  - `orca orchestration check --wait --types worker_done,escalation,question`
  - `orca orchestration gate-create --task <task_id> --question "¿Aprobado?"`
- **Seguimiento**: actualizar workbench.md y comentarios del worktree
  - `orca worktree set --worktree active --comment "F0: <estado>"`
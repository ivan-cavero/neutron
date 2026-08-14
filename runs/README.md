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
| run-041 | F2d R41 | FAIL (bar 1:1) · 121/121 BB 1:1; roll 0.467; ALL 97.84% | 14 ago 2026 |
| run-040 | F2d R40 | FAIL (bar 1:1) · generateBox; roll 0.029; ALL 97.28% | 14 ago 2026 |
| run-039 | F2d R39 | FAIL (bar 1:1) · 116/121 BB; roll 0.996→0.112 (catalyst sí); ALL 97.27% | 14 ago 2026 |
| run-038 | F2d R38 | FAIL (bar 1:1) · mineshaft (4,-1) 4 piezas XZ 1:1; roll sigue 0.996 | 14 ago 2026 |
| run-037 | F2d R37 | FAIL (bar 1:1) · 98.48%; roll 0.996→0.30 si se abre la mineshaft vecina | 14 ago 2026 |
| run-036 | F2d R36 | FAIL (bar 1:1) · **98.48%**; ChargeCursor 1:1 cueva; cat 2=2 | 14 ago 2026 |
| run-035 | F2d R35 | FAIL (bar 1:1) · 98.35%; plano 1:1; sculk 643/518; Y=-32 213 | 14 ago 2026 |
| run-034 | F2d R34 | FAIL (bar 1:1) · 98.40%; sculk 330→382; tick 3 mata cursores | 14 ago 2026 |
| run-033 | F2d R33 | FAIL (bar 1:1) · 98.41%; i=0 pos 1:1, catalyst_roll 0.701 | 14 ago 2026 |
| run-032 | F2d R32 | FAIL (bar 1:1) · 98.41%; shuffle 1:1; capa Y=-32 | 14 ago 2026 |
| run-031 | F2d R31 | FAIL (bar 1:1) · 98.41%; sculk 187→330 | 14 ago 2026 |
| run-030 | F2d R30 | FAIL (bar 1:1) · 98.33% / BASE 99.69%; OCEAN_FLOOR + stream | 14 ago 2026 |
| run-029 | F2d R29 | FAIL (bar 1:1) · 97.65% / BASE 99.65%; andesite 1:1 | 14 ago 2026 |
| run-028 | F2d R28 | FAIL (bar 1:1) · 97.02%; BiomeFilter; van upper en 28 chunks | 14 ago 2026 |
| run-027 | F2d R27 | FAIL (bar 1:1) · 97.02%; andesite_upper diag | 14 ago 2026 |
| run-026 | F2d R26 | FAIL (bar 1:1) · block 94→97%, BASE 99% | 14 ago 2026 |
| run-025 | F2d R25 | FAIL (bar 1:1) · block match 85→94% | 14 ago 2026 |
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
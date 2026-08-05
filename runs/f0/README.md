# RUN F0 — Fundamentos y harness de benchmarks

> Fecha: 5 ago 2026 · Repo: C:\Users\ivang\neutron · Rama base: main · ROADMAP F0 (bar + rondas)
> Fuentes de verdad: OPERATIONS.md §9 (prompt F0), ROADMAP.md (F0), BENCHMARKS.md (metodología), OPERATIONS.md §2-§6 (Gauntlet Loop, reglas, operating manual)

## 0. Pre-flight (obligatorio, 60 s)

1. Resolver el CLI de Orca (regla de OPERATIONS.md §4):
   - Si existe la variable `ORCA_CLI_COMMAND` → úsala.
   - Si estás en un dev checkout con `ORCA_DEV_REPO_ROOT` → `orca-dev`.
   - En Linux fuera de Orca → `orca-ide` (NUNCA `orca` desnudo en Linux: es el screen reader).
   - En Windows o dentro de Orca → `orca`.
2. `orca status --json` → confirma que responde.
3. `orca skills get orchestration` → LEE la guía completa de TU versión. Los flags de abajo son los verificados en ago 2026; si esta versión los cambió, manda.

## 1. Crear el run (objective)

```powershell
$obj = Get-Content -Raw .\runs\f0\objective.txt
orca orchestration run-create --objective $obj --json
```

Alternativa GUI: nuevo Run en Orca → pega el contenido de `objective.txt` → lanza el coordinador en un worktree desde `main`.

## 2. El coordinador descompone y despacha (specs de referencia en tasks/)

El coordinador DEBE crear estas 3 tareas (specs listas en `tasks/*.json`):

| Tarea | Archivo spec | Depende de | Agente sugerido |
|---|---|---|---|
| T-B0: Harness de benchmarks | tasks/t-b0.json | — | opencode |
| T-CI: Workspace + CI + STATE.md | tasks/t-ci.json | — | opencode |
| T-BASE: Baseline B0 publicado | tasks/t-base.json | T-B0, T-CI | opencode |

```powershell
orca orchestration task-create --task-title "T-B0: Harness de benchmarks" --spec (Get-Content -Raw .\runs\f0\tasks\t-b0.json) --json
orca orchestration task-create --task-title "T-CI: Workspace + CI + STATE.md" --spec (Get-Content -Raw .\runs\f0\tasks\t-ci.json) --json
orca orchestration task-create --task-title "T-BASE: Baseline B0 publicado" --spec (Get-Content -Raw .\runs\f0\tasks\t-base.json) --json

orca orchestration worker-start --task <taskId-T-B0> --worktree f0-t-b0 --agent opencode --json
orca orchestration worker-start --task <taskId-T-CI> --worktree f0-t-ci --agent opencode --json
# T-BASE se despacha cuando T-B0 y T-CI estén done
```

*(Si tu versión usa `--run-id <id>`, añádelo a task-create. Verifícalo en la guía que cargaste en el paso 0.)*

## 3. Esperar resultados

```powershell
orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 900000 --json
```

- Timeout de 15 min por check; si expira, re-ejecuta el mismo check.
- En `worker_done`: exigir evidencia (logs crudos, JSON, tabla). Sin evidencia → devolver al worker.
- En `escalation`/`question`: resolver con `gate-create` o elevar al humano.
- Heartbeat: workers de tareas largas (T-B0) deben mandar heartbeat periódico; si se silencian > 30 min, escalar.

## 4. Gauntlet Loop y gates humanos de F0 (no negociables)

- Cada tarea se itera contra su bar (ACs): builder → critic ciego (contexto limpio) que inspecciona el artefacto REAL → FAIL con el gap más grande → repetir. Sin cap arbitrario de rondas: para cuando el bar gana, 2 rondas sin mejora, o presupuesto agotado.
- El baseline B0 se publica SOLO tras aprobación humana (gate-create con el humano).
- Nadie modifica BENCHMARKS.md ni los AC sin gate humano.
- Los workers no se auto-verifican: el critic (agente distinto) re-ejecuta desde cero y pega la salida.

## 5. Criterios de salida de la fase (ROADMAP F0 — el bar)

- [ ] Harness de punta a punta en Windows y Linux (`bench/run.ps1` + `bench/run.sh`).
- [ ] Startup medido por regex `Done (Xs)!`, 5 runs, mediana.
- [ ] 10 bots de join simultáneo en vanilla y Paper sin kicks, p95 < 5 s.
- [ ] cps medido con Chunky (vanilla/Paper) + método propio (Pumpkin), consistente con baselines ±30%.
- [ ] Baseline B0 publicado en `bench/results/B0-<fecha>.md` (vanilla 26.2 / Paper / Pumpkin) con JSON crudo y hardware documentado.
- [ ] Un agente distinto al builder reproduce B0 desde cero en Windows y Linux (parte del bar).

## 6. Reporte final que debe entregar el coordinador

1. Resumen de verdades verificadas vs suposiciones. 2. STATE.md actualizado. 3. Tabla del baseline. 4. Qué falta para F1 (riesgos detectados con evidencia).
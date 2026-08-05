# STATE — RUN F0 · Neutron

> Durable state del Gauntlet Loop F0. Se lee al empezar cada iteración; se actualiza al terminar (OPERATIONS.md §3).

## Run

- Run Orca: `run_5791497557d0` (creado 5 ago 2026 20:14 UTC)
- Coordinador: terminal `term_e8f5dbdd` (lead-f0-console, pane estable)
- Rama base: `main` @ `08f73fb` (notes del LEAD commiteadas)
- Presupuesto global de fase (guardrail): 3 tareas × specs (T-B0 200k tok/120 min · T-CI 100k tok/90 min · T-BASE 150k tok/180 min)

## Tareas

| Tarea | Task ID | Dispatch | Estado | Ronda | Critic | Presupuesto usado |
|---|---|---|---|---|---|---|
| T-B0: Harness de benchmarks | task_49e175bf0333 | ctx_6f1f933e8be2 | dispatched (r1) | 1/… | pendiente | 0% |
| T-CI: Workspace + CI + STATE.md | task_da6370d2127e | ctx_0900f6b9c7be | dispatched (r1) | 1/… | pendiente | 0% |
| T-BASE: Baseline B0 publicado | task_a2ebf2a248e9 | — (deps T-B0+T-CI) | pending | — | pendiente | 0% |

## Bar de la fase (no negociable)

1. Agente distinto al builder ejecuta `bench/run.ps1` desde cero en Windows y Linux y reproduce el baseline B0.
2. 10 bots join simultáneo sin kicks, p95 < 5 s.
3. cps ±30% consistente con baselines publicados.

## Entorno verificado (LEAD, 5 ago 2026)

- Windows win32 · Node 24.18.0 · cargo 1.96.0 · **Java 21 (falta Java 25 → vanilla 26.2)** · **sin distro WSL (Linux pendiente de resolver)**
- Detalles: `runs/f0/lead/notes.md`

## Decisión del LEAD

- `STATE.md` raíz lo gestiona el LEAD (evita conflicto de merge con T-CI): T-CI entrega la plantilla en `docs/STATE.template.md` para cumplir AC CI-3.

## Log de rondas

| Ronda | Tarea | Builder | Verdict critic | Gap | Evidencia |
|---|---|---|---|---|---|
| 1 | T-B0, T-CI | opencode ×2 (paralelo) | pendiente | — | — |

# STATE — Neutron

> Estado actual del proyecto. Se lee al empezar cada run y se actualiza al terminar.

## Fase actual
**F0 — Fundamentos y harness** (Round 3 en curso)
- Bar: baseline B0 reproducible por un agente distinto; 10 bots join p95 < 5 s; cps ±30% vs baselines publicados.
- Progreso: U1 (harness) ✅ PASS | U2 (bots) ✅ PASS | U3 (docs) 🔄 IN PROGRESS | U4 (baseline B0) ⏳ PENDIENTE
- Ver `workbench.md` para el detalle de rondas.

## Últimos runs
| Run | Fecha | Resultado |
|-----|-------|-----------|
| 000 | 5 ago 2026 | Fundación — terminado |
| 001 | en curso | F0: harness + bots + baseline B0 |

## Herramientas disponibles
- **pi**: LEAD + builder principal; critic como subagente con contexto limpio
- **Orca CLI**: worktrees aislados por builder (`orca worktree create`)
- **Orca Orchestration**: DAG de tasks, worker_done, decision gates
- **MCP tools**: investigación de referencias externas
- **Computer Use**: interacción con apps de escritorio

## Presupuesto acumulado
N/A (fase de documentación + harness)

## Historial
- `runs/run-*.md` — detalle de cada run
- Método: AGENTS.md §2 (Gauntlet Loop) · ROADMAP.md §2 (prompts por fase)
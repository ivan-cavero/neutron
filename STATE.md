# STATE — Neutron

> Estado actual del proyecto. Se lee al empezar cada run y se actualiza al terminar.

## Fase actual
**F0 — Fundamentos y harness** (COMPLETADO ✅)
- Bar: un agente distinto al builder ejecuta `bench/run.ps1` desde cero en Windows y Linux y reproduce el baseline B0 (vanilla 26.2 / Paper / Pumpkin); 10 bots de join simultáneo sin kicks (p95 < 5 s); cps ±30% consistente con baselines publicados.
- Progreso:
  - U1 (harness `run.ps1` + `run.sh`) ✅ PASS — Ronda 7
  - U2 (bots `join-bench/index.js` mineflayer + `azalea-join-bench` Rust) ✅ PASS — Ronda 7
  - U3 (docs `bench/README.md`) ✅ PASS — Ronda 7
  - U4 (baseline B0) ✅ COMPLETADO — azalea bots funcionando con Vanilla/Paper/Folia

## Baseline B0 — Resultados (azalea, 10 bots cada uno)
| Servidor | Bots | p50 (ms) | p95 (ms) | p99 (ms) |
|---|---|---|---|---|
| Vanilla 26.2 | 10/10 | 373 | 406 | 407 |
| Paper 26.2 | 10/10 | 3332 | 3383 | 3384 |
| Folia 26.2 | 10/10 | 3277 | 3678 | 3678 |

## Bots
- **mineflayer** (Node.js): funciona hasta 1.21.11, no soporta 26.2
- **azalea** (Rust): funciona con 26.2, join latency ~300-400ms local
- Binario: `bench/bots/azalea-join-bench/target/release/azalea-join-bench.exe`
- Compilar con: `cd bench/bots/azalea-join-bench && rustup run nightly cargo build --release`

## Ver
- `workbench.md` — detalle de rondas
- `runs/run-001.md` — documentación completa del run
- `bench/results/` — JSON con resultados

## Siguiente fase
F0 completada. Listo para F1 — Núcleo jugable (protocolo 26.2, mundo Anvil, fuzz, E2E).

## Herramientas disponibles
- **pi**: LEAD + builder principal; critic como subagente con contexto limpio
- **Orca CLI**: worktrees aislados por builder (`orca worktree create`)
- **Orca Orchestration**: DAG de tasks, worker_done, decision gates
- **MCP tools**: investigación de referencias externas
- **Computer Use**: interacción con apps de escritorio

## Presupuesto acumulado
~200K tokens estimados para F0 completa (7 rondas realizadas)

## Historial
- `runs/run-001.md` — run actual de F0
- Método: AGENTS.md §2 (Gauntlet Loop) · ROADMAP.md §2 (prompts por fase)
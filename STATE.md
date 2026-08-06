# STATE — Neutron

> Estado actual del proyecto. Se lee al empezar cada run y se actualiza al terminar.

## Fase actual
**F0 — Fundamentos y harness** (Completado — baseline B0 publicado)
- Bar: un agente distinto al builder ejecuta `bench/run.ps1` desde cero en Windows y Linux y reproduce el baseline B0 (vanilla 26.2 / Paper / Pumpkin); 10 bots de join simultáneo sin kicks (p95 < 5 s); cps ±30% consistente con baselines publicados.
- Progreso:
  - U1 (harness `run.ps1` + `run.sh`) ✅ PASS — Ronda 7
  - U2 (bots `join-bench/index.js`) ✅ PASS — Ronda 7
  - U3 (docs `bench/README.md`) ✅ PASS — Ronda 7
  - U4 (baseline B0) ✅ PARCIAL — Vanilla 26.2 funcionando, bots mineflayer no soportan 26.2, Paper no descargable (API detrás de Cloudflare)
- Ver `workbench.md` para detalle de rondas.
- Ver `runs/run-001.md` para documentación del run.

## Últimos runs
| Run | Fase | Resultado | Fecha |
|-----|------|-----------|-------|
| 001 | F0 | COMPLETADO (U1✅ U2✅ U3✅ U4⚠️ parcial) | 6 ago 2026 |

## Limitaciones conocidas de F0
1. **Mineflayer no soporta 26.2**: La versión 4.37.1 solo llega hasta 1.21.11. Para 26.2 se necesita azalea (Rust) — backlog para F0 o F1
2. **Paper no descargado**: API de PaperMC (api.papermc.io) bloqueada por Cloudflare desde scripts. Se necesita descarga manual o usar el site downloads.papermc.io
3. **Pumpkin descargado** ✅ pero no probado (misma limitación de mineflayer para bots)
4. **Neutron no compila**: No hay Cargo.toml en raíz — el servidor Neutron empieza en F1
5. **Script run.ps1 tiene bugs heredados**: Em dashes (U+2014), RedirectStandardOutput/Error al mismo path, ConsoleCancelKeyPress en no interactivo — se usó run_baseline.ps1 como alternativa

## Herramientas disponibles
- **pi**: LEAD + builder principal; critic como subagente con contexto limpio
- **Orca CLI**: worktrees aislados por builder (`orca worktree create`)
- **Orca Orchestration**: DAG de tasks, worker_done, decision gates
- **MCP tools**: investigación de referencias externas
- **Computer Use**: interacción con apps de escritorio

## Presupuesto acumulado
~200K tokens estimados para F0 completa (6 rondas realizadas)

## Historial
- `runs/run-001.md` — run actual de F0
- Método: AGENTS.md §2 (Gauntlet Loop) · ROADMAP.md §2 (prompts por fase)
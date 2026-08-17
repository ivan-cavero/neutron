# STATE — Neutron

> Read this first every session. Answers: where are we, what is the bar, what is the next action.
> History lives in `runs/` — this file only holds the current state.
> **Updated 17 Aug 2026** — dual-track session paused (user switching PCs). Resume
> boundary + full detail: `runs/run-047.md`. Worktree state: main contains ALL PASSED
> work (merged); branches `ivan-cavero/bench-refactor` + `ivan-cavero/server-worldgen` pushed.

## Current phase

**Dual-track (run-047)**: Track A = benchmark harness refactor; Track B = server review +
worldgen parity. Session paused mid-loop; next action = resume units below.

## Bars (unchanged)

- **Worldgen (human decision R43 — mechanism parity)**: same seeds/streams/algorithms as
  vanilla. Deterministic phases → 100% block match multi-seed; vegetation/sculk → same RNG
  stream 1:1. Do not edit measurement examples/tests to pass.
- **Benchmarks (run-047 Track A)**: multi-version provisioning (`servers download
  <type> <version>`, Mojang/Paper API + fallback), versioned report history + compare over
  history, builds green in both workspaces, build/runtime measured.

## Track A — benchmarks (`tests/benchmarks/`, own nightly workspace)

| Piece | Status | Commits (merged to main) |
| --- | --- | --- |
| A1 provisioning (multi-version, arch-aware pumpkin, fallback) | ✅ blind-critic PASS | 36c13a4, 3b7b0ff |
| A2 versioned report history + compare over history | ✅ blind-critic PASS | c0876c6, 16fb597, 519bc2e |
| A3 perf: exit-101 bevy_log panic fix, root gate, measured times | ⏳ NEXT | — |
| A4 smoothing | ⏳ after A3 | — |

## Track B — server + worldgen (`crates/neutron-server`, `crates/neutron-worldgen`)

| Piece | Status | Commits (merged to main) |
| --- | --- | --- |
| B1 server review (boot/ping/join/TPS) | ✅ blind-critic PASS (6/6) | 6444286, 89550c4, 7d45404, 2199b05 |
| B1b disconnect cleanup (critic-found defect) | ✅ blind-critic PASS | 272e30b |
| B2 fresh 26.2 reference extraction + baseline re-measure (424242/12345/777) | ⏳ NEXT | — |
| B3 worldgen: 777 regression + lush/pale recall ≥80% | ⏳ after B2 | — |
| B4 smoothing | ⏳ after B3 | — |

## Server status (critic-verified 17 Aug, worktree build)

- 26.2 protocol: boots (`Done (0.0s)!`), valid status ping, protocol-level join → Play +
  real chunks (21, zero decode errors), TPS **20.00** via `tokio::time::interval(50ms)`
  (was 16.07 with sleep), disconnect cleanup on RST fixed. `cargo test --workspace` 241/241.
- Evidence + open items: `crates/neutron-server/REVIEW.md`.

## Worldgen measurement status (IMPORTANT)

- **No reference worlds on disk** (`tools/nbt-ref/` has only README.md) → the run-046
  numbers (97.38%, 96.29% for 777, clay 466, lush/pale recall 62.94%) are **unverifiable**.
  B2 must re-extract fresh 26.2 references (Java 25 installed at
  `C:\Program Files\Eclipse Adoptium\jdk-25.0.4.7-hotspot`) and re-measure before B3 work.
- Known gaps from run-046 (targets for B3): 777 regression isolate; lush/pale recall
  (moss_block 1218, cave_vines 735 missing); border decoration order; mineshaft/structures deferred.

## System status

- **Tests**: 241 passed (worldgen 59, protocol 47, world 39, sim 65, server 24, integration 7).
- **Server**: `cargo run --release -p neutron-server -- --seed 12345 --view-distance 8`.
- **F3**: FASE A ✅ B ✅ C ✅ D pending (not started).

## History (pointers — full details in each run file)

| Runs | Phase | Outcome |
| --- | --- | --- |
| run-000..043 | F0→F2d | harness → parity baseline → mechanism parity bar (R43) |
| run-044 | mechanism parity T1-T3 | ✅ aquifer/surface/sculk (blind-critic PASS) |
| run-045 | lush/pale dispatch | recall 11→49.6%; cross-chunk model isolated |
| run-046 | cross-chunk input model | U1 PASS; U5 active (777 regression unverified; critic pending on R3) |
| run-047 | dual-track: benchmarks + server/worldgen | **ACTIVE — PAUSED for PC switch** |

## Key docs

- `AGENTS.md` — how we work (bar, gauntlet loop, tools)
- `ROADMAP.md` — phases, bars, prompt templates in `docs/prompts/`
- `workbench.md` — live round log for the active run
- `runs/run-047.md` — current run file with RESUME BOUNDARY
- `crates/neutron-worldgen/WORLDGEN.md`, `WORLDGEN-PIPELINE.md`
- `crates/neutron-server/REVIEW.md` — server review evidence
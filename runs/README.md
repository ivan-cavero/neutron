# Runs — execution history

> Every run is recorded here: objective, bar, tasks, evidence, outcome. Format: `run-NNN.md`.
> **This file is the single source of truth for "how to run a run"** (AGENTS.md §6).

## Run template

Every `run-NNN.md` follows this structure (see `run-001.md` for a good example):

```markdown
# Run NNN — <title>

## Objective
One sentence: what must be true when done.

## Bar
Measurable criteria (from ROADMAP.md). Include the multi-seed ratchet:
no regression on ANY seed in the current bar (e.g. 12345/424242/777).

## Tasks
### T1 — <title>
- What: measurable
- AC: concrete criteria with thresholds (+ ratchet: no regression on other seeds)
- Evidence: raw logs, hashes, outputs
- DoD: what the blind critic runs from scratch to give PASS

### T2 — ...
...

## Evidence
(Raw logs with timestamps, hashes, bot outputs, links to reports.)

## Result
PASS / FAIL (partial) / BLOCKED

## Rounds
- R1: T1 PASS, T2 FAIL (reason)
- R2: T2 fixed → PASS
```

## PASS discipline (mandatory)

- A **PASS** verdict in a run file requires **blind-critic evidence**: an independent
  subagent with clean context re-ran the measurement from scratch and inspected the
  real artifact (not the builder's summary).
- Builder-verified work is labeled **"builder-verified"**, never PASS.
- Every round re-measures ALL seeds in the bar (ratchet). A regression on any seed
  is a FAIL.

## How to launch a run

1. Read `STATE.md` → decide which run is next (bar not met → same run continues).
2. Create `runs/run-NNN.md` with the template above.
3. Track units with `todo`; launch builders via `subagent` (parallel, background).
4. Gauntlet Loop: builder → blind critic (`subagent`, clean context) → fix → repeat.
5. Update `workbench.md` (live round log) and `STATE.md` (state, not history) when done.

## History

| Run | Phase | Result | Date |
| --- | --- | --- | --- |
| run-046 | F2d cross-chunk input model | **ACTIVE** — R3 committed 91862d4, critic pending | 16 Aug 2026 |
| run-045 | F2d lush/pale dispatch | recall 11→49.6%; 424242 97.28%; cross-chunk model isolated | 16 Aug 2026 |
| run-044 | F2d mechanism parity | ✅ T1-T3 PASS (blind critic); T4 → run-045 | 15-16 Aug 2026 |
| run-043 | F2d R43 | new bar: mechanism parity (human gate); vanilla1 reference poisoned | 15 Aug 2026 |
| run-042 | F2d freeze + join | worldgen frozen; 26.2 server serves real chunks | 14 Aug 2026 |
| run-041 | F2d R41 | FAIL (bar 1:1) · 121/121 BB 1:1; ALL 97.84% | 14 Aug 2026 |
| run-040 | F2d R40 | FAIL (bar 1:1) · generateBox; ALL 97.28% | 14 Aug 2026 |
| run-039 | F2d R39 | FAIL (bar 1:1) · 116/121 BB; ALL 97.27% | 14 Aug 2026 |
| run-038 | F2d R38 | FAIL (bar 1:1) · mineshaft (4,-1) 4 pieces XZ 1:1 | 14 Aug 2026 |
| run-037 | F2d R37 | FAIL (bar 1:1) · 98.48%; HORIZONTAL N,E,S,W | 14 Aug 2026 |
| run-036 | F2d R36 | FAIL (bar 1:1) · 98.48%; ChargeCursor cave 1:1 | 14 Aug 2026 |
| run-035 | F2d R35 | FAIL (bar 1:1) · 98.35%; flat floor 1:1 | 14 Aug 2026 |
| run-034 | F2d R34 | FAIL (bar 1:1) · 98.40%; sculk 330→382 | 14 Aug 2026 |
| run-033 | F2d R33 | FAIL (bar 1:1) · 98.41%; first sculk patch 1:1 | 14 Aug 2026 |
| run-032 | F2d R32 | FAIL (bar 1:1) · 98.41%; shuffle 1:1 | 14 Aug 2026 |
| run-031 | F2d R31 | FAIL (bar 1:1) · 98.41%; sculk 187→330 | 14 Aug 2026 |
| run-030 | F2d R30 | FAIL (bar 1:1) · 98.33% / BASE 99.69% | 14 Aug 2026 |
| run-029 | F2d R29 | FAIL (bar 1:1) · 97.65%; andesite 1:1 | 14 Aug 2026 |
| run-028 | F2d R28 | FAIL (bar 1:1) · 97.02%; BiomeFilter | 14 Aug 2026 |
| run-027 | F2d R27 | FAIL (bar 1:1) · 97.02%; andesite_upper diag | 14 Aug 2026 |
| run-026 | F2d R26 | FAIL (bar 1:1) · block 94→97% | 14 Aug 2026 |
| run-025 | F2d R25 | FAIL (bar 1:1) · block match 85→94% | 14 Aug 2026 |
| run-024..000 | F2d R24..F0 | early parity + harness | 5-14 Aug 2026 |

> **Historical runs (run-000..run-043) are in Spanish** — they are superseded by
> `STATE.md` + the recent runs. Translate only when a run becomes active again.

## Orchestration (pi)

- **Subagents**: `subagent` tool — builders (parallel, `async`), blind critics
  (foreground, clean context), Explore (read-only).
- **Tracking**: `todo` tool — one item per unit with status and dependencies.
- **Human gates**: `ask_user_question` — releases, credentials, bar changes.
- **Waiting**: `subagent_wait` — block until an async subagent finishes.
- **Research**: `web_search` / `fetch_content` for crates.io, minecraft docs, vanilla sources.

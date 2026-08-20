# Runs — archive

> **20 Aug 2026:** this folder is **history**, not a launch checklist.
> Do **not** open `run-NNN.md` to start worldgen work. Method: `AGENTS.md` v2.
> Facts: `STATE.md`. Optional: drop a short evidence note here *after* a dump or a % move.
>
> Older runs (001–059) stay as a paper trail of false levers. Format below is
> what those files used; it is not required for new work.

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

### Worktree creation (cache sharing — run-049 lesson)

Each git worktree has its OWN `target/` (cargo does not share caches across
worktrees), so a new worktree starts with a COLD build (~15-20 min release) —
this burned the run-049 first fan-out (30-min default subagent timeout died on
builds, not on work). Two rules:

1. **Seed the worktree target with hardlinks from main** (instant, no cargo
   lock contention — cargo replaces files on rebuild, safe with hardlinks):
   ```bash
   git worktree add -b <branch> <path>
   cd <path> && cp -al <main>/target .  # or: cp -al <main>/target target
   ```
   Do this BEFORE launching the builder so its first build is warm.
2. **Pass an explicit budget**: subagent default timeout is 30 min — too short
   for research-heavy units (order derivation, multi-port rounds). Set
   `maxRuntimeMs` ≥ 5400000 (90 min) for builders; keep critics at 30-45 min
   (they verify, they don't build new logic).
3. (Optional) Refs/jars are gitignored runtime data — symlink them from the
   main checkout instead of copying (run-049: `ln -sfn <main>/tools/nbt-ref/vanilla-fresh-* <wt>/tools/nbt-ref/`).


## Version bump run (D0-D4) — Mojang releases a new version

> Trigger: a Mojang release (webhook/CI on D0). Goal: `main` on the new version in
> ≤ 7 days with parity re-measured. Same gauntlet discipline as any run: bar +
> ratchet + blind critic. The plan lives in `ROADMAP.md` §4 + `ARCHITECTURE.md` §10;
> this template is the operational checklist.

```markdown
# Run NNN — D0-D4 for Minecraft <X.Y>
## Objective
main on <X.Y>, parity re-measured, old-version data intact.
## Bar
- Parity suite green on the NEW version's references (ratchet on all seeds).
- No fixes from the old version regressed silently (diff review).
## Tasks
### T1 — D1: code diff vs previous version
- `cargo run -p mc-decompiler -- download <X.Y>` + `decompile <X.Y>`
- `cargo run -p mc-decompiler -- diff <X.Y> <prev>` (full + per-class: worldgen, redstone, protocol)
- Evidence: diff output; list of changed classes to port.
### T2 — D1b: worldgen DATA diff (detection of data-only changes)
- Re-extract `worldgen/**` JSON + `biome_params.bin` from the new jar
  (tool: mc-decompiler `extract-data` once it exists — today: manual, see STATE gap B4)
- Diff vs `crates/neutron-worldgen/src/data/worldgen/`:
  new/removed/changed biome, configured_feature, placed_feature, noise settings.
- Evidence: `diff -r` output; count of changed features.
### T3 — D2: ports + codegen
- Port changed classes (dispatch coverage test must stay green: every biome →
  placed → configured feature type dispatches or is whitelisted).
### T4 — D3: re-provision references (versioned!)
- `tools/nbt-ref/vanilla-fresh-<X.Y>-<seed>/` for all bar seeds (B2 recipe in the
  previous run file). Old-version refs stay on disk untouched (never delete).
- Re-run determinism spot-check (7 runs of one seed, border noise map) — cheap, 6 min.
### T5 — D4: full parity suite + release
- All parity examples + probes + `cargo test --workspace` + benchmarks.
- Evidence: one report file per suite, pushed with the bump commit.
## Result
PASS (all bars, blind critic) / FAIL (list gaps) / BLOCKED
```

**Detection rules (what must fail loudly on a bump):**
- Dispatch coverage test: a new configured-feature type → red test at D2, not at D4.
- Data integrity test: any placed/configured JSON reference that does not resolve → red.
- FeatureSorter indices: re-verified against the jar probe (they change silently).
- Golden hashes per version: parity suite re-run against NEW refs only; old refs are
  the regression check (same seed, old version, old parity must still hold).

## History

| Run | Phase | Result | Date |
| --- | --- | --- | --- |
| run-048 | F2d resume: benchmarks + worldgen | **ACTIVE** — A-track DONE; B2 PASS; B3 re-derived on main (parallel agent), recall 57.14% (bar NOT met); B4 next | 18 Aug 2026 |
| run-047 | dual-track: benchmarks refactor + server review | A1/A2 PASS, B1/B1b PASS (merged); A3/B2/B3 → run-048 | 17 Aug 2026 |
| run-046 | F2d cross-chunk input model | U1 PASS; U5 R3 (777 regression, recall 62.94% claim — unverified) | 16 Aug 2026 |
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

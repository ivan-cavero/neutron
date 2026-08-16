# STATE — Neutron

> Read this first every session. Answers: where are we, what is the bar, what is the next action.
> History lives in `runs/` — this file only holds the current state.

## Current phase

**F2d run-046 — cross-chunk input model (WorldGenRegion)** — ACTIVE

## Bar (human decision R43 — mechanism parity)

Same seeds/streams/algorithms as vanilla. Deterministic phases → 100% block match multi-seed;
vegetation/sculk → same RNG stream 1:1. **Do not edit measurement examples/tests to pass.**

### run-046 acceptance criteria (intact)

| # | Criterion | Latest (R3, 16 Aug, LEAD-measured) | Status |
| --- | --- | --- | --- |
| 1 | clay 424242 → ~497 (lush/pale missing ≤20%, recall ≥80%) | clay **466** ✓ · recall **62.94%** (missing 37%) | ❌ |
| 2 | border diffs 12345 down ≥30% (from 20.89pp baseline) | **-7.5%** (19.33pp) | ❌ |
| 3 | REGION 424242 ≥97.28% · 12345 ≥97.75% (no regression) | **97.38%** ✓ · **97.94%** ✓ | ✅ |
| 4 | `cargo test --workspace` green · worldgen 59/59 | **241/241 green** | ✅ |
| 5 | **Multi-seed ratchet: 777 no regression** | **96.29%** (baseline ~99.4%) | ❌ REGRESSION |

## Last measurement (16 Aug 2026, `region_parity`/`clay_overlap`/`lush_pale_parity`)

| seed | REGION 3×3 ALL | Note |
| --- | --- | --- |
| 12345 (6,-2) | 97.94% | bar ✓ |
| 424242 (0,0) | 97.38% | bar ✓ |
| 777 (0,0) | **96.29%** | **regression vs ~99.4% — investigate first** |

## Next action

1. **Isolate the 777 regression**: U5 cross-chunk model vs R3 tree changes (bisect by stash).
2. Port missing lush/pale placement: **MossVegetationFeature + CaveVineFeature** (moss_block 1218,
   cave_vines 735 missing) — the biggest recall gap.
3. Launch the **blind critic for R3** (working tree committed as `91862d4`, critic pending).

## Open gaps (short list — details in runs/)

- Lush/pale recall 62.94% (bar ≥80%): moss_block, cave_vines, clay placement
- Border diffs: neighbor decoration order still off (decoration region model)
- 777 regression: unisolated
- Mineshaft postProcess (rails, cobweb), other structures (villages, stronghold) — deferred
- F3 FASE D (golden suite posicional + survival) — not started

## System status

- **Server**: 26.2 protocol joinable (Configuration + known packs), serves real chunks.
  Spawn = heightmap (0,0). `cargo run --release -p neutron-server -- --seed 12345 --view-distance 8`
- **Tests**: 241 passed (worldgen 59, protocol 47, world 39, sim 65, server 24, integration 7)
- **F3**: FASE A (light/redstone/fluids/spawns) ✅ · B (comparators/repeaters/observers/hoppers/TNT) ✅ ·
  C (pistons/QC/block swapping) ✅ · D pending

## History (pointers — full details in each run file)

| Runs | Phase | Outcome |
| --- | --- | --- |
| run-000..001 | F0 harness | ✅ |
| run-002..006 | F1/F2/F2d early | ✅ baseline |
| run-007..023 | F2d R3-R23 | terrain/density/surface/ores |
| run-024..032 | F2d R24-R32 | RNG, ores, sculk, trees |
| run-033..043 | F2d R33-R43 | sculk patches; R43 = new bar (mechanism parity) |
| run-044 | mechanism parity T1-T3 | ✅ aquifer/surface/sculk (blind-critic PASS) |
| run-045 | lush/pale dispatch | recall 11→49.6%; cross-chunk model isolated |
| run-046 | cross-chunk input model | **ACTIVE** — R3 committed, critic pending |

## Key docs

- `AGENTS.md` — how we work (bar, gauntlet loop, tools)
- `ROADMAP.md` — phases, bars, prompt templates in `docs/prompts/`
- `workbench.md` — live round log for the active run
- `crates/neutron-worldgen/WORLDGEN.md` — worldgen freeze, metrics, gaps
- `crates/neutron-worldgen/WORLDGEN-PIPELINE.md` — pipeline + findings

# STATE — Neutron

> Read this first every session. Answers: where are we, what is the bar, what is the next action.
> History lives in `runs/` — this file only holds the current state.
> **Updated 19 Aug 2026** — run-050 closed: order hypothesis REFUTED, cave-biome root cause found.

## Current phase

**run-050 (closed, 19 Aug)**: pull limpio a origin/main; refs 424242 provisionados
en esta máquina (spawn chunk (0,0)); baseline reproducido (REGION 97.34% · recall
58.43% · clay 411/435). El plan run-049 (orden como lever) fue **refutado con
evidencia** — el orden es run-dependiente (PC-2 vs esta máquina) y mueve ±1pp.
**Causa raíz nueva**: la clasificación de cave-biomes difiere de vanilla (2.43%
de celdas 4×4×4, todas en secciones de cueva y −48..96): Neutron dice
`pale_garden` donde vanilla tiene `lush_caves` → el filtro `minecraft:biome` de
lush_caves_clay/moss/vines rechaza los draws → gaps de clay (22%) + moss (~19%) +
vines (~8%) = 34% del recall. Ver `runs/run-050.md`.

**run-051 (19 Aug)**: el cave-biome era un FALSO POSITIVO — el comparador de
run-050 usaba el grid almacenado vs el voronoi; con la comparación correcta
(voronoi, lo que usa el filtro `minecraft:biome`) hay 0 mismatches. Verificado
exacto: RNG streams (62 draws de clay idénticos), voronoi, environment_scan,
feature unions, biome_id_to_name. **El desync real está dentro de
`place_vegetation_patch`** (xz_radius muestreado 2× vs 1×?, orden del set,
depth/chances) + el terreno (97.34%). Clay: Neutron coloca +53 células sobre el
terreno vanilla; generate full 612 vs 435.

**Próximo (run-052 T1)**: diff línea a línea de `place_vegetation_patch` vs
`VegetationPatchFeature` de vanilla (consumo RNG del xz_radius, orden del set,
depth, extra_edge/bottom chances), y lo mismo para los trees. Objetivo:
clay_overlap 192/435 → ~435/435, luego recall.

## Bars (unchanged)

- **Worldgen (human decision R43 — mechanism parity)**: same seeds/streams/algorithms as
  vanilla. Deterministic phases → 100% block match multi-seed; vegetation/sculk → same RNG
  stream 1:1. Do not edit measurement examples/tests to pass.
- **Benchmarks (run-047/048 Track A)**: multi-version provisioning, versioned report
  history + compare, builds green both workspaces, build/runtime measured. **MET.**

## Track A — benchmarks (`tests/benchmarks/`, own nightly workspace) — DONE

| Piece | Status | Commits (in main) |
| --- | --- | --- |
| A1 provisioning (multi-version, arch-aware pumpkin, fallback) | ✅ PASS | 36c13a4, 3b7b0ff |
| A2 versioned report history + compare over history | ✅ PASS | c0876c6, 16fb597, 519bc2e |
| A3 perf: exit-101 fix (OnceLock logger + LogPlugin disable), root gate `./bench`, measured times | ✅ PASS | 3e13bfe, 144b288, a77dc86, 12de2d9 (+smoothing 0067fe2, 522ecae, cc59594) |
| A4 smoothing | ✅ DONE | merged to main (ff) |

## Track B — worldgen parity (`crates/neutron-worldgen`) — B3 in progress

| Piece | Status | Evidence / commits |
| --- | --- | --- |
| B1 server review + B1b disconnect fix | ✅ PASS (run-047) | merged in main |
| B2 fresh 26.2 references + baseline (424242/12345/777) | ✅ PASS (blind critic) | `runs/run-048-evidence-baseline.txt` |
| B3 777 regression + lush/pale recall ≥80% | 🔨 builder DONE on THIS PC (main 355c3d5, 3 commits); **critic PENDING (none on this PC)**; **bar NOT met (recall 57.14%)** | commits 7fcfd06/e72a87e/355c3d5 (main, NOT pushed) |
| B4 vegetation gap closure | ⏳ next | design in workbench R4 log |

## B3 results (LEAD re-measured on THIS PC, main @ 355c3d5 — builder claims not blindly verified)

- **777 regression root-caused** (e72a87e): `climate_at_block` used `peaksAndValleys`
  on ridge noise; vanilla 26.2 uses RAW ridge noise (probe −0.8113). → 777
  96.29 → **98.31 %**, lifts every seed. Also fixed: `random_offset` sampling order
  (xz,xz,y) and `vegetation_patch` Java HashSet order (355c3d5, `JavaBlockPosSet`).
- **Live measurements (LEAD ran the examples, 20:0x +02:00, fresh refs on disk)**:
  424242 **97.36 %** (≥97.28 ✓) · 12345 **97.81 %** (≥97.75 ✓) · 777 **98.31 %**
  (ratchet ✓; "~99.4 %" historical claim still NOT reproducible). Lush/pale recall
  **57.14 %** (PC-2's 57.43 % claim NOT reproducible here — close but different).
  clay 411/497 · tests 241/241 (15 suites, LEAD re-ran).
- **Residual gap (B4 design)**: 8 572 lush/pale cells missing in 424242 3×3:
  pale_oak_leaves 2178 + pale_oak_log 1190 (trees ~39 %), clay 1965, moss_block 983,
  pale_hanging_moss 519, cave_vines_plant+head 730, moss carpets 377, big_dripleaf
  220, azalea 109. Two root-cause families found:
  1. **Dispatch no-ops**: `minecraft:multiface_growth` (glow_lichen),
     `minecraft:root_system` (rooted_azalea_tree), `minecraft:vines`
     (classic_vines_cave_feature) fall into `_ => {}` silence.
  2. **RNG-stream desync within features**: center-chunk counts ≈ vanilla
     (leaves 392 vs 396, clay_overlap example) but 3×3 positions ≠ → trees/patches
     accept at different attempts, shifting the stream for later attempts.
     Order experiments (ring-first) are noise until this is fixed.
- Evidence: live example outputs in this session (region_parity ×3, lush_pale_parity,
  clay_overlap); `runs/run-048-evidence-B3.txt` (PC-2) does NOT exist on this PC.

## D0-D4 detection infra (built 18 Aug 2026, this session — PRE-COMMIT)

Two pieces from the B4/version-bump design are now REAL, not paper (user asked
to build them):

1. **`mc-decompiler extract-data <version>`** (tools/, human-approved): extracts
   `data/minecraft/worldgen/**` JSON from a server JAR (bundler-safe, in-memory,
   no `.extracted` litter) and semantically diffs it against
   `crates/neutron-worldgen/src/data/worldgen`. Evidence (run on this PC):
   - vs **26.2** jar: 963 files extracted; **MATCH 654, CHANGED 0, JAR-ONLY 309
     (carvers/structure/template_pool/presets — not ported by design),
     CRATE-ONLY 1** (`noise_settings_overworld.json` — jar path is
     `noise_settings/overworld.json`; rename in the crate, semantically equal).
     → crate data is 100% in sync with the 26.2 jar.
   - vs **26.3-snapshot-8** jar (already on disk): **CHANGED 302, JAR-ONLY 703
     (incl. new biome `dappled_forest`), CRATE-ONLY 227** → the D0-D4 T2
     detection works end-to-end.
2. **Dispatch coverage test** (`crates/neutron-worldgen/tests/dispatch_coverage.rs`):
   every placed → configured feature reachable from the overworld FeatureSorter
   must dispatch, be in the step-6 batch (`features.rs`), or be whitelisted.
   First run found **43 orphans / 23 types**; after verification against the
   source: 14 types implemented (10 dispatch + 4 batch), **30 confessed no-ops
   whitelisted with reasons** (geode, bamboo, disk@step4 ice_patch,
   ore_infested@step7, speleothem_cluster, block_blob, fossil, freeze_top_layer,
   spike, iceberg, kelp, lake, large_dripstone, monster_room, root_system,
   sea_pickle, seagrass, sequence/sulfur_pool, vines, multiface_growth@glow_lichen…).
   **GREEN** (`cargo test --workspace` 242/242). A new feature type in 26.3 →
   red at D2 naming the exact feature.
   - Bonus findings (real gaps the test surfaced): `ice_patch` (disk) at step 4
     and `ore_infested` at step 7 are OUTSIDE the step-6 batch — new entries in
     the confessed-gaps list with reasons.

Update the whitelist + reason whenever a type is ported; the test fails on
whitelist drift (both additions and removals).

## RESUME BOUNDARY (current machine — read first)

1. **Push UNBLOCKED**: `git push origin main` succeeded (up to 63d3e3b). main == origin/main, 0/0 ahead/behind. Contents pushed: all commits up to the T4 blue_ice port + vanilla_spawn experiment + deco_stream_probe + tree-type fix + T4 ports. Working tree: CLEAN.
2. **B3 critic verdict does not exist on this machine** (PC-2 async run 44205f88 did not
   travel). LEAD re-derived the measurements live on 18 Aug 20:0x +02:00 (region_parity
   ×3 + lush_pale_parity + clay_overlap + full test suite) → **expect FAIL on recall**
   (57.14 % < 80 %); ratchet ✓ (777 98.31 % > 96.29 baseline).
3. **Runtime data is on disk on THIS machine** (gitignored): references
   `tools/nbt-ref/vanilla-fresh-{424242,12345,777}/` (529 chunks each, verified by B2
   critic). Jar at `tools/mc-decompiler/jars/server-26.2.jar` (60,894,273 B). Re-provision
   on any other PC with the B2 recipe in `runs/run-048.md`.
5. **Next actions**:
   - **T3 order**: complete the full 9-chunk vanilla decoration order (the subagent got
     partial for (0,0) — (-1,0),(0,-1) before (0,0) only; the other 8 chunks' orders
     need derivation). This is THE lever for the tree desync (39% of the 8,572 gap).
   - **T3c**: apply the order + the water filter fix (column_water_depth with correct
     OCEAN_FLOOR) TOGETHER — they are a package. Measure recall (target: +5pp toward 80%).
   - **T3d**: clay gap (411 vs 497) — currently terrain-coupled; investigate with order fixed.
   - **T4 remaining**: desert_well, ice_spike, speleothem×2, fossil×2, freeze_top_layer,
     large_dripstone, monster_room, lake_lava, sulfur_pool, ice_patch, ore_infested
     (whitelist 19 → ~5). Measure each (recall risk: glow_lichen was -0.81pp).
   - **T5**: the dominant terrain mismatches — the order fix is the primary lever; the
     tree-type bug is DONE (+0.27pp 777). Climate is exact. The remaining families
     (clay, sculk, ores, carvers) are terrain-coupled.
6. **Ownership**: LEAD owns STATE/workbench/runs/. Worldgen source = builder-owned.
   `tools/` = human-owned. `tests/benchmarks/**` = Track A (closed).

## Worldgen measurement status

- References on disk (THIS PC): `tools/nbt-ref/vanilla-fresh-{424242,12345,777}/` (529
  chunks each, hash-mode blocks, verified by B2 critic). 12345 spawn center = (6,-2); its
  (0,0) chunk is an air proto-chunk (invalid measurement target).
- Baseline (B2 PASS): REGION 424242 97.27% · 12345 97.79% · 777 96.29% · recall 53.03% ·
  clay 466 (vanilla 493).
- **Now (19 Aug, main 63d3e3b)**: 424242 **97.32%** · 12345 **97.81%** · 777 **98.58%**
  (tree-type fix +0.27pp) · recall **57.50%** · clay **411** (vanilla 497). All region
  bars ✓, recall/clay bars ✗. Coverage: 168 features, 17 dispatched, 19 confessed.

## System status

- **Tests**: 243 passed root workspace (47 protocol, 7 integration, 24 server, 65 sim, 39 world, 59 worldgen, 1 dispatch_coverage) — verified 19 Aug.
- **Server**: `cargo run --release -p neutron-server -- --seed 12345 --view-distance 8` (B1 PASS in run-047).
- **F3**: FASE A ✅ B ✅ C ✅ D pending (not started).

## History (pointers — full details in each run file)

| Runs | Phase | Outcome |
| --- | --- | --- |
| run-000..043 | F0→F2d | harness → parity baseline → mechanism parity bar (R43) |
| run-044 | mechanism parity T1-T3 | ✅ aquifer/surface/sculk (blind-critic PASS) |
| run-045 | lush/pale dispatch | recall 11→49.6%; cross-chunk model isolated |
| run-046 | cross-chunk input model | U1 PASS; U5 R3 (777 regression, recall 62.94% claim — unverified) |
| run-047 | dual-track benchmarks + server/worldgen | A1/A2 PASS, B1/B1b PASS (merged); A3/B2/B3 pending |
| run-048 | resume on new PC | **ACTIVE** — Track A DONE; B2 PASS; B3 re-derived, tree-type fix (+0.27pp 777), order partial (T3a-b), T4 ports (whitelist 19), recall 57.50% bar NOT met; B4 in progress |

## Key docs

- `AGENTS.md` — how we work (bar, gauntlet loop, tools)
- `ROADMAP.md` — phases, bars, prompt templates in `docs/prompts/`
- `workbench.md` — live round log for the active run
- `runs/run-048.md` — current run file with RESUME BOUNDARY + B2 recipe
- `runs/run-048-evidence-baseline.txt` — B2 evidence (B3 evidence file exists only on PC-2)
- `crates/neutron-worldgen/WORLDGEN.md`, `WORLDGEN-PIPELINE.md`
- `crates/neutron-server/REVIEW.md` — server review evidence
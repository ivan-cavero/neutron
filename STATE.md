# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 24 Aug 2026 (Linux box).**

## Now

Worldgen 1:1 vs vanilla **26.2**.

| Seed | region ALL | notes |
| --- | --- | --- |
| 424242 | **98.06%** r=1 (this Linux machine; histogram: trees ~5.8k, veg patches ~3k, grass/vines ~2k) | mineshafts 1:1 + ores + spiral order |
| 424242 SCAN | **97.98%** over all 81 full-status ref chunks (spawn area) — the global audit number | PARITY_SCAN=1 |
| 12345 | 98.45% center (6,-2) — Windows box only, ref not here | spawn = (6,-2): authentic wavefront |
| 777 | 99.43% chunk (0,0) — Windows box only | |

- Closed earlier (git log has evidence): spiral origin order · ore skipped-draw
  fix · mineshaft side-exit · step 3 ON · trapezoid dispatcher.
- lush/pale recall 60.66%.

## Perf

- Single-chunk gen 11.5 s. `NEUTRON_STEP_TIMING=1` per-phase ms.

## Next (one question)

**METER SWITCHED (24 Aug): region_parity + `PARITY_LEDGER=<csv>` is the road
to 100%. ProbeDecorate stays as logic debugger ONLY — its replay fidelity vs
the real world is ~27% (4035/5558 mismatches on 424242 cc=(0,0), 81678
writes, 0 errors), so it cannot rank remaining work; don't burn sessions on
its positional jitter. Ledger = cell-exact TSV (x,y,z,class,core/border,
vanilla,neutron) + GAPS ranking with cum % + e.g./bbox per gap. Whole-ref
audit: `PARITY_SCAN=1 PARITY_LEDGER=g.csv cargo run -r -p neutron-worldgen
--example region_parity -- <seed> 0 0 0 <regiondir>` (PARITY_SCAN=N samples
every Nth chunk). Coverage = full-status chunks in the ref (spawn area;
pregenerate more in-game to widen). New MC version =
`tools/nbt-ref/new-mc-version.sh <ver> <seed>` → rerun scan.**

Top gaps FULL SCAN 424242 (161200 cells = missing 2.02%; cum % of gap):
dark_oak leaves+log ±19% · pale_oak leaves+log ±17% (tree placement/shape =
#1 family overall ~36%) · moss_block/clay/stone swaps ~14% (lush clay+moss
patch placement) · short_grass/tall_grass/leaf_litter ±6% (vegetation_patch
HashSet — known dead-end) · oak_leaves ±4% · cave_vines ±4% ·
pale_hanging_moss ±4% · coal_ore wrong ~2.4%. WORST chunks ring (-1..0,-3)
and (1,3..4).

Probe pipeline facts kept: getRandom=worldgen_region_random factory per
origin; isStateAtPosition handler gap killed ALL trees silently; OreFeature
section.setBlockState needs mirrorToSection+sync (`|sync`); decorationSeed =
CHUNK coords; biome_id_to_name +27 names.

**Validation harness**: `biome_grid_parity` example (BGP_SETS=1 prints sets):
ref-vs-ours quart grid = 99.98% (3 quarts), SETS identical → BIOME GRID
EXONERATED as cause.

- Writer attribution (kept): sculk_vein ~99% from charge cascade.
- Honest baseline (Windows box, 12345 cc=(6,-2)): 367/773 sculk cells.
  Refs 12345/777 NOT on this machine — only 424242.
- Attempt-level tracing: `M|ATT|patch|...`, `NEUTRON_SCULK_ATT`,
  `NEUTRON_SCULK_TRACE_W`, `PROBE_WATCH`.

Chunk decoration order as FIXED sequence (vanilla = async ticket scheduler;
spiral is the best static approximation — sweep-verified) · cave-biome
stored-grid vs voronoi · vegetation_patch HashSet · noodle sign · carvers write
cave_air (overworld carvers write plain AIR; refs' cave_air = structures) ·
geode outer-layer branch reachable · freeze gate as router-temperature · worm
start-Y desync · placeGround already-ground in surface set · mineshaft
set_large_feature_seed missing salt · step-6 sorter order (34/34 exact) ·
OreFeature blob math (line-exact, flow-simulated; desync was skipped-draw
early-out) · trapezoid dedicated-path formula · ore attempt>=2 "drift" (was
probe model missing blob draws — ProbeOreFlow now exact for discard 0/1) ·
biome-grid divergence as feature-flip cause (99.98% match, sets identical,
biome_grid_parity) · biome first-seen ORDER as probe-vs-ref seed shuffler
(LinkedHashSet+captured order: fidelity unchanged 25.15%).

## This machine (Linux box — Windows artifacts NOT here)

```
424242  tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region
jar     tools/nbt-ref/vanilla-fresh-424242/versions/26.2/server-26.2.jar (+ libraries/)
javap   decompile single classes via tools/mc-decompiler/vendor/vineflower.jar
NO:     refs 12345/777 · decompiled src tree · ProbeDecorate.class history
```

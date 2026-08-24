# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 24 Aug 2026 (Linux box).**

## Now

Worldgen 1:1 vs vanilla **26.2**.

| Seed | region ALL | notes |
| --- | --- | --- |
| 424242 | **98.06%** r=1 (this Linux machine; histogram: trees ~5.8k, veg patches ~3k, grass/vines ~2k) | mineshafts 1:1 + ores + spiral order |
| 12345 | 98.45% center (6,-2) — Windows box only, ref not here | spawn = (6,-2): authentic wavefront |
| 777 | 99.43% chunk (0,0) — Windows box only | |

- Closed earlier (git log has evidence): spiral origin order · ore skipped-draw
  fix · mineshaft side-exit · step 3 ON · trapezoid dispatcher.
- lush/pale recall 60.66%.

## Perf

- Single-chunk gen 11.5 s. `NEUTRON_STEP_TIMING=1` per-phase ms.

## Next (one question)

**ORACLE PIPELINE FIXED (24 Aug, Linux box) — now captures trees + ores +
decorators; probe-vs-REF fidelity is the open front (~25% on origin (0,0)).**
Probe fixes this session (ProbeDecorate.java, javap/vineflower-verified):
(1) `getRandom`=worldgen_region_random factory per origin; (2) missing
`isStateAtPosition` handler killed ALL trees silently — fixed (429 logs /
2253 leaves now); (3) OreFeature writes via section.setBlockState directly —
mirrorToSection + syncSectionsToStore (`|sync` tag) capture them; (4)
PaleMossDecorator needs Registry-typed lookup + getLevel chain
(makeServerLevel); (5) decorationSeed = CHUNK coords; (6) biome_id_to_name
completed +27 names ("_ => plains" collapse poisoned export remap).

**Validation harness**: `biome_grid_parity` example (BGP_SETS=1 prints sets):
ref-vs-ours quart grid = 99.98% (3 quarts), SETS identical (dark_forest,
deep_dark, lush_caves, pale_garden) → BIOME GRID EXONERATED as cause.

**Open mystery**: with seeds formula javap-verified (`seed+index+10000*step`),
same dec, same feature list/order — probe blobs still land near-but-not-on
ref/neutron positions (coal probe∩neutron = 1/48; e.g. probe (1,101,3) vs
neutron/ref (0,101,3)). Off-by-a-few pattern suggests a draw-count divergence
early in the modifier chain or in Feature.place entry. Next session: diff
raw draw streams for ore_coal_upper @origin(0,0) between ProbeDecorate
(PALE_TRACE-style) and neutron rng_echo from setFeatureSeed onward.
NOTE: region_parity/ORACLE numbers vs ref are unaffected by probe issues —
they read the ref world directly.

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

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

- Closed earlier (see git log for evidence): decoration origin order =
  setInitialSpawn spiral · ore skipped-draw fix (`y < -64`) · mineshaft SOUTH
  side-exit `maxZ()-3` · step 3 wired ON · trapezoid dispatcher.
- lush/pale recall 60.66%. Remaining composition (12345 center): trees ~700,
  sculk ~400, surface veg ~180, clay patches (424242) ~300, misc ~100.

## Perf

- Single-chunk gen 11.5 s (noise+surface = 10 s of it; carvers 79 ms;
  decoration ~1.7 s). `NEUTRON_STEP_TIMING=1` per-phase/per-step ms.

## Next (one question)

**PROBE FIXES (24 Aug, Linux box): oracle now runs clean on 424242.**
1. ProbeDecorate `getRandom` was unimplemented → pale_moss_patch (VegetationPatch
   step 9, gif 28) THREW per origin; ref missing all moss-patch writes.
   Vanilla truth (javap): WorldGenRegion.random =
   `randomState.getOrCreateRandomFactory("minecraft:worldgen_region_random")
   .at(centerChunk.getWorldPosition())` — PER-ORIGIN. Implemented in probe;
   errors 7→0, writes 9817→9870, TAGCHECK=true.
2. ProbePaleFlow was still TAG-BLIND (no bindBlockTags) → MOSS_REPLACEABLE
   dead → placeGroundPatch placed nothing → run-061's "java surface set
   EMPTY / ±2 draws residual" was an ARTIFACT. Fixed: calls
   ProbeDecorate.bindBlockTags(). PATCHTEST now PLACEs (passed=3/41 real
   terrain; before: 41×NOREPL).
3. region_parity: PARITY_HISTO=1 prints diff-class histogram.

**424242 r=1 = 98.06% (9 chunks, this machine). Histogram of the missing
~1.9%**: pale oak trees ~5.8k cells (log/leaves position+shape), lush/pale
VegetationPatches ~3k (moss/clay↔stone both directions), grass/vines ~2k,
sculk negligible here. Center-chunk oracle diff vs fixed reference:
40 cells, ALL VegetationPatch-family (clay/moss/cave_vines/grass).

- **Flat-text isolated test is VOID for gates**: dump_terrain TEXT format has
  no biomes → FROMDUMP consults generator climate → our side fails biome gate
  at pos #1 while fakeLevel java forces pale_garden. Streams themselves are
  IDENTICAL through seeding + pos#1 (next(31) values equal). Isolated tree
  work must use biome-carrying dumps (NDEC1 / PRETERRAIN over refs).
- Sculk honest baseline (367/773 @12345 cc=(6,-2)) stands but was measured on
  Windows box; refs 12345/777 NOT on this machine — only 424242.
- Attempt-level tracing: `M|ATT|patch|...` write-log markers,
  `NEUTRON_SCULK_ATT` / `NEUTRON_SCULK_TRACE_W`, `PROBE_WATCH`.
Next steps: (1) sculk attempt #0 diff vs corrected reference — needs 12345 ref
(Windows box) or copy it here; (2) VegetationPatch chain on 424242 via clean
oracle: first diverging origin/cell of the 40-cell diff, then trees.## Dead (do not reopen without a new two-sided dump)

Chunk decoration order as FIXED sequence (vanilla = async ticket scheduler;
spiral is the best static approximation — sweep-verified) · cave-biome
stored-grid vs voronoi · vegetation_patch HashSet · noodle sign · carvers write
cave_air (overworld carvers write plain AIR; refs' cave_air = structures) ·
geode outer-layer branch reachable · freeze gate as router-temperature · worm
start-Y desync · placeGround already-ground in surface set · mineshaft
set_large_feature_seed missing salt · step-6 sorter order (34/34 exact) ·
OreFeature blob math (line-exact, flow-simulated; desync was skipped-draw
early-out) · trapezoid dedicated-path formula · ore attempt>=2 "drift" (was
probe model missing blob draws — ProbeOreFlow now exact for discard 0/1).

## This machine (Linux box — Windows artifacts NOT here)

```
424242  tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region
jar     tools/nbt-ref/vanilla-fresh-424242/versions/26.2/server-26.2.jar (+ libraries/)
javap   decompile single classes via tools/mc-decompiler/vendor/vineflower.jar
NO:     refs 12345/777 · decompiled src tree · ProbeDecorate.class history
```

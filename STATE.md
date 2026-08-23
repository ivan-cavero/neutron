# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 23 Aug 2026 (sesión 2).**

## Now

Worldgen 1:1 vs vanilla **26.2**. HEAD: mineshaft SOUTH maxZ()-3 fix + step 3 ON
+ trapezoid dispatcher fix.

| Seed | region ALL | notes |
| --- | --- | --- |
| 424242 | **97.96%** r=1 (+0.26 vs 97.70) | mineshafts 1:1 + ores draw-flow fix |
| 12345 | **98.36%** center (6,-2) r=1 (+0.13 vs 98.23) | center chunk 98.31 / BASE 99.82 |
| 777 | 99.38% chunk (0,0) only full chunk on disk | |

- **Ores fixed**: root cause = out-of-world Y samples (`y < -64`) hit an early
  `continue` before place_ore_blob, skipping ~69 draws vanilla consumes.
  Verified attempt counts per feature vs ProbeOrePositions.java (new probe:
  modifier-chain model count/rarity/square/uniform/trapezoid for step 6).
  Center-chunk ore mismatches (iron/redstone/diamond) eliminated.
- **Mineshaft layout 1:1**: SOUTH corridor side-exits at `maxZ() - 3`
  (MineshaftPieces.java:233/:244). All referenced starts BB-IDENTICAL vs NBT
  (14/14 both seeds). Probes kept: `examples/ms_layout.rs`,
  `examples/sorter6.rs`, `ProbeSorter6.java`, `ProbeOrePositions.java`.
- Step 3 (monster_room/fossil) wired ON. TrapezoidHeight fixed in dispatcher.
- lush/pale recall 60.66% (was 60.96 — deep-ore fix shifted some lush gates;
  net region gain positive).
- Remaining composition (12345 center): trees ~700 (cascade), sculk ~400,
  surface veg (leaf_litter/grass) ~180, clay patches 424242 ~300, misc ~100.

## Perf (this machine, release)

- Biome union memoized per origin; quart grids stored in RegionBuf.
  Single-chunk gen 50 s → 11.5 s (noise+surface = 10 s of it; carvers 79 ms;
  decoration ~1.7 s). `NEUTRON_STEP_TIMING=1` per-phase/per-step ms.

## Next (one question)

Border-cell spill (28 non-cascade left, ALL at chunk edges: granite/diorite
blob overlap + clay/gravel disks + stray coal at lx=0 / lz=0-1). Likely lever:
origin decoration ORDER vs vanilla ChunkTracker wavefront (now that everything
else is 1:1, last-writer-wins at borders is exposed). Try NEUTRON_DECO_CUSTOM_
ORDER sweeps per-border-origin before/after center. Then trees/sculk cascade
shrinks further.

## Dead (do not reopen without a new two-sided dump)

Chunk decoration order · cave-biome stored-grid vs voronoi · vegetation_patch
HashSet · noodle sign · carvers write cave_air (overworld carvers write plain
AIR; refs' cave_air = structures) · geode outer-layer branch reachable · freeze
gate as router-temperature · worm start-Y desync · placeGround already-ground
in surface set · mineshaft set_large_feature_seed missing salt (vanilla uses no
salt for structure generation either) · step-6 sorter order (34/34 exact) ·
OreFeature blob math (line-exact; desync was the skipped-draw early-out) ·
trapezoid dedicated-path formula (was already correct).

## This machine

```
424242  tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region
12345   tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region
777     tools/nbt-ref/vanilla-fresh-777/world/dimensions/minecraft/overworld/region
jar     tools/mc-decompiler/jars/server-26.2.jar
java    tools/mc-decompiler/output/26.2/src
javacp  tools/nbt-ref/vanilla-fresh-12345/versions/26.2/server-26.2.jar + libraries/
```

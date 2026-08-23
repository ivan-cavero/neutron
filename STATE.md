# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 23 Aug 2026 (sesión 2).**

## Now

Worldgen 1:1 vs vanilla **26.2**. HEAD: mineshaft SOUTH maxZ()-3 fix + step 3 ON
+ trapezoid dispatcher fix.

| Seed | region ALL | notes |
| --- | --- | --- |
| 424242 | **97.90%** r=1 (+0.20 vs 97.70) | mineshafts now 1:1 |
| 12345 | **98.28%** center (6,-2) r=1 (+0.05 vs 98.23) | geodes + honest CAVE_AIR |
| 777 | 99.38% chunk (0,0) only full chunk on disk | |

- **Mineshaft layout 1:1**: root cause = SOUTH corridor side-exits at
  `maxZ()` instead of `maxZ() - 3` (MineshaftPieces.java:233/:244; the EAST
  maxX-3 quirk was ported, the mirrored SOUTH one was not). All referenced
  starts now BB-IDENTICAL vs vanilla NBT (14/14 across both seeds).
  Probe kept: `examples/ms_layout.rs` (diffs structures.starts Children BBs;
  References longs are `(z<<32)|x`, NOT ChunkPos.asLong). Step-6 sorter probe:
  `examples/sorter6.rs` — our order matches vanilla 34/34.
- Step 3 (monster_room/fossil) wired and stays ON: uniform-height sampling is
  correct (`randomBetweenInclusive`); earlier "wrong rooms" were cascade from
  the mineshaft bug.
- TrapezoidHeight fixed in dispatcher (sampling.rs): vanilla =
  min + betweenInclusive(0, range-plateauStart) + betweenInclusive(0,
  plateauStart); was (a+b)/2. Dedicated ore path already had it right.
- Ore blob mismatches remain (~150 cells/region, e.g. extra redstone / missing
  diamond blobs): discrete step-6 OreFeature blobs, NOT the noise veinifier
  (OreVeinifier port verified line-exact). Next probe: blob start positions.
- Remaining composition (12345 center): trees ~800 (cascade), sculk ~400,
  ores ~80, leaf_litter/grass ~90, clay patches 424242 ~300.

## Perf (this machine, release)

- Biome union memoized per origin; quart grids stored in RegionBuf.
  Single-chunk gen 50 s → 11.5 s (noise+surface = 10 s of it; carvers 79 ms;
  decoration ~1.7 s). `NEUTRON_STEP_TIMING=1` per-phase/per-step ms.

## Next (one question)

Ore blob placement parity: diff our blob starts vs vanilla for ore_redstone /
ore_diamond in one chunk (ProbeOreBlob pattern), then sculk spreader cascade,
then tree draw-column base closure.

## Dead (do not reopen without a new two-sided dump)

Chunk decoration order · cave-biome stored-grid vs voronoi · vegetation_patch
HashSet · noodle sign · carvers write cave_air (overworld carvers write plain
AIR; refs' cave_air = structures) · geode outer-layer branch reachable · freeze
gate as router-temperature · worm start-Y desync · placeGround already-ground
in surface set · mineshaft set_large_feature_seed missing salt (vanilla uses no
salt for structure generation either) · step-6 sorter order (34/34 exact).

## This machine

```
424242  tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region
12345   tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region
777     tools/nbt-ref/vanilla-fresh-777/world/dimensions/minecraft/overworld/region
jar     tools/mc-decompiler/jars/server-26.2.jar
java    tools/mc-decompiler/output/26.2/src
javacp  tools/nbt-ref/vanilla-fresh-12345/versions/26.2/server-26.2.jar + libraries/
```

Visual diff without the game: `tools/neutron-map` (map/tree/biomes/feature —
see its README). First full-region run: 91/91 full chunks match, 0 differ.

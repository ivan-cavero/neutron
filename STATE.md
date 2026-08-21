# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 21 Aug 2026.**

## Now

Worldgen 1:1 vs vanilla **26.2**. HEAD pending: clay-pool dripleaf (`BlockId` + waterlogged veg on the water cell).

| Seed | region ALL | notes |
| --- | --- | --- |
| 424242 | **97.71%** (was ~97.34) | re-extracted ref `vanilla-fresh-424242`, forceload chunks [-2,2]² |
| 777 | ~98.58% | ridge-noise raw fix was the real jump |
| 12345 | ~97.8% when Status=full | skip if proto-chunk |

- lush/pale recall ~57% (bar ≥80% — **not** the next knob).
- clay full `(0,0)`: Neutron 575 vs vanilla 509 (`clay_overlap` iso 517/509, xz overlap 96/134).
- water y[0,16): Neutron **(0,0)=4 (0,1)=96** vs vanilla **15 + 97**. Probe cells **13/22 water**.
- trees `(0,0)` 424242: 51 vs 37 pale trunks. RNG 1:1. Extra accepts = terrain.
- open (air+cave_air) `(0,0)`: Neutron doFill y0..16 **2023** vs vanilla final **1900**; y-16..-1 Neutron **0** vs vanilla **151 cave_air**. `(0,1)` y0..16 2957 vs 2932 (close). Carvers: `CARVE_BAND` cell=write=none=**0** in y[-16,16) for `(0,0)`/`(0,1)`.

Benchmarks track: **done**. Server: joinable.

## Next (one question)

`(0,1)` water is done (96 vs 97). `(0,0)` 4 vs 15 is extra doFill air at y0..16 (+123 vs vanilla final) + already-clay skip. `placeGround` return-true overshoots (waterlogged-only 49; clay+pool 18 / region 97.67%; all patches 18 / 97.68%). Do not poke `placeGround` again.

Next dump: extra Neutron air cells in `(0,0)` y0..16 vs vanilla (doFill density at those coords). Parallel: why `CARVE_BAND` cell=0 while vanilla has 151 cave_air in `(0,0)` y[-16,-1] (worm path / `can_reach`, not aquifer-None).

## Dead (do not reopen without a new two-sided dump)

Chunk decoration order · cave-biome stored-grid vs voronoi · `vegetation_patch` HashSet · noodle sign (compared seed 12345 vs 424242) · `getInterpolatedNoiseValue` helper as if it were `doFill` · T4 feature ports as the 424242 recall lever · classic carvers as the writer of 424242 `(0,0)`/`(0,1)` y[-16,16) water · worm start-Y desync · `SpringFeature` as the writer of those floor cells · `nextInt(2)` as `RandomBooleanSelectorFeature` pick · missing dripleaf `BlockId` as the extra `(0,1)` water (111→96).

## This machine

```
424242  tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region
12345   tools/nbt-ref/vanilla-fresh-12345/dimensions/minecraft/overworld/region
777     tools/nbt-ref/vanilla-fresh-777/dimensions/minecraft/overworld/region
jar     tools/mc-decompiler/jars/server-26.2.jar
java    tools/mc-decompiler/output/26.2/src
```

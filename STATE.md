# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 21 Aug 2026.**

## Now

Worldgen 1:1 vs vanilla **26.2**. HEAD pending: clay-pool dripleaf (`BlockId` + waterlogged veg on the water cell).

| Seed | region ALL | notes |
| --- | --- | --- |
| 424242 | **97.71%** r=1 (9 ch); **97.31%** r=2 (25 ch); **96.95%** center (1,-1) r=1 | forceload [-2,2]²; not one-cell fitted |
| 12345 | **97.62%** chunk (0,0) Status=full | vanilla1; 9-chunk 96.33% includes proto neighbours — skip those |
| 777 | no `.mca` on this machine | path in STATE is stale |

- lush/pale recall **61.81%** (dripleaf helped; bar ≥80% — not the next knob).
- clay full `(0,0)`: Neutron 575 vs vanilla 509. `lush_caves_clay` draws **1:1** vs `ProbeClayDraws` (gi=29).
- water y[0,16): Neutron **(0,0)=4 (0,1)=96** vs vanilla **15 + 97**.
- trees `(0,0)` 424242: 51 vs 37 pale trunks. RNG 1:1.
- doFill `(0,0)` y0..16 vs vanilla final: extra air over vegetation/clay=122, over solid=**0**. Terrain matches. `placeGround` already-ground still overshoots %.
- classic carvers: starts 1:1 vs `ProbeCarveStartY`. `CARVE_BAND` cell=0; vanilla worms also **write 0** into `(0,0)`/`(0,1)` y[-16,16).
- `minecraft:multiface_growth` (glow_lichen) is a no-op. Ports with/without spread: 97.71→**97.67/97.62** and `(0,0)` water 4→1. Do not ship until jar-accurate.

Benchmarks track: **done**. Server: joinable.

## Next (one question)

`(0,1)` water done. `(0,0)` 4 vs 15 = extra lush clay (575 vs 509) on **matching** doFill, then pool skip on already-clay. Do not poke `placeGround`. Do not chase doFill extra air (it was grass).

Next: extra clay on matching terrain — glow_lichen/cave_vines before clay (step 9 order) vs vanilla counts in `(0,0)`. Closed port: `MultifaceGrowthFeature` with jar defaults (floor=false, spread 0.5, search 20) + threshold filter.

## Dead (do not reopen without a new two-sided dump)

Chunk decoration order · cave-biome stored-grid vs voronoi · `vegetation_patch` HashSet · noodle sign (compared seed 12345 vs 424242) · `getInterpolatedNoiseValue` helper as if it were `doFill` · T4 feature ports as the 424242 recall lever · classic carvers as the writer of 424242 `(0,0)`/`(0,1)` y[-16,16) water · worm start-Y desync · `SpringFeature` as the writer of those floor cells · `nextInt(2)` as `RandomBooleanSelectorFeature` pick · missing dripleaf `BlockId` as the extra `(0,1)` water (111→96) · extra doFill air in `(0,0)` y0..16 (was grass/clay vs vanilla final).

## This machine

```
424242  tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region
12345   tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region
777     (missing)
jar     tools/mc-decompiler/jars/server-26.2.jar
java    tools/mc-decompiler/output/26.2/src
```

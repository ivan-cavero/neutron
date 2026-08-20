# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 20 Aug 2026.**

## Now

Worldgen 1:1 vs vanilla **26.2**. HEAD `1d8d26e` (carver → aquifer `getCarveState`; 0 pp on region/clay).

| Seed | region ALL | notes |
| --- | --- | --- |
| 424242 | ~97.34% | stuck since run-048 (~97.27). Measure this one. |
| 777 | ~98.58% | ridge-noise raw fix was the real jump |
| 12345 | ~97.8% when Status=full | skip if proto-chunk |

- lush/pale recall ~57% (bar ≥80% — **not** the next knob).
- clay 411 vs vanilla ~435–497 on 424242.
- water: Neutron ~0 cave water in y 0..16; vanilla chunk `(0,1)` seed 424242 has ~95 water.
- trees chunk `(0,0)` 424242: Neutron 51 pale trunks vs vanilla 37. **RNG stream 1:1**. Extra accepts = terrain.

Benchmarks track: **done**. Server: joinable.

## Next (one question)

**Dumps 20 Aug** (`evidence/dofill/`):

- Vanilla doFill (BeardifierMarker): water cells density +0.0037..+0.006, `getInterpolatedState=null` (solid). Same sign as Neutron.
- Vanilla `computeSubstance(ctx,0.0)` at those cells: **air**, not water. If a carver visits, it would write air.
- Neutron carvers: 33 starts with Y in [-32,0), but **0 writes** in chunks (0,0)/(0,1) in that band. Writes in those chunks are y[-48,-32) air (deepslate) only. Neighbor chunks *do* get y[-32,0) air. Water=0 from carvers.

`carveEllipsoid` Y bound now matches vanilla (`worldY > minY`). region 424242 still **97.34%**. `CARVE_BAND_CELL=0`: no ellipsoid even *enters* y[-16,16) in (0,0)/(0,1). Mineshafts add 0 air there. Next: worm path vs vanilla start Y (not aquifer-none). Still no feature ports.

## Dead (do not reopen without a new two-sided dump)

Chunk decoration order · cave-biome stored-grid vs voronoi · `vegetation_patch` HashSet · noodle sign (compared seed 12345 vs 424242) · `getInterpolatedNoiseValue` helper as if it were `doFill` · T4 feature ports as the 424242 recall lever.

## This machine

```
424242  tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region
12345   tools/nbt-ref/vanilla-fresh-12345/dimensions/minecraft/overworld/region
777     tools/nbt-ref/vanilla-fresh-777/dimensions/minecraft/overworld/region
jar     tools/mc-decompiler/jars/server-26.2.jar
java    tools/mc-decompiler/output/26.2/src
```

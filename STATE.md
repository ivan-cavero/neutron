# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 28 Aug 2026 (Linux box), session 2.**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical 524-chunk world.

| Measurement | Value |
| --- | --- |
| SCAN 525, 28 Aug s2 (9d58a2e) | **98.85%**, ledger **594,312** cells |
| Session delta | 599,711 → 594,312 (**−5,399**) |
| SCAN 528, seed **12345** (28 Aug) | ticket_sim **98.45%** (no regression) |
| SCAN 527, seed **777** (28 Aug) | ticket_sim **98.41%** (no regression) |
| 12345 window (6,-2) r=2 | 98.47 → **98.49%** (fallen_tree port) |

Meter speedup (6ae05e2): worker pool (cores−2, `PARITY_WORKERS`), streaming
compare, NBT prefetch, per-worker persistent NoiseCache. Full SCAN ~24 min
→ **~4 min**, 2 cores free, output identical.

## Closed 28 Aug (git log has evidence)

- **FallenTreeFeature PORTED** (77b27a2): was unknown-type no-op. Stump +
  horizontal log + canPlaceEntireFallenLog (gap ≤2) + trunk_vine +
  attached_to_logs decorators, RNG order exact. Ledger −1,022.
- **validTreePos = full 26.2 replaceable_by_trees tag** (013a17a): tall_grass,
  ferns, flowers, bush/dead_bush, dry grass, seagrass, roots, vine now free —
  they had IDs and REJECTED trees vanilla replaces-and-places. +
  CountPlacement count 0 = empty stream (trees_plains 0w19/1w1). −1,334.
- **minSurfaceLevel = vanilla bilinear** (312ed67): 16x16 cell-corner lerp of
  quart-quantized preliminary surface levels, f32 alphas, floor — was
  per-block eval + trunc. −2,464.
- **steep = one-directional** (9d58a2e): south≥north+4 else west≥east+4,
  chunk-clamped — was symmetric + self-row deltas. −579.
- **Carve geometry PROVEN identical** (b81b047): ProbeCarveTrace (real
  CaveWorldCarver, stone ProtoChunk, real aquifer) vs NEUTRON_CARVE_TRACE:
  56/56 source streams identical on target (1,-1). (17,96,-5): density
  bit-exact (+0.00944026 both), aquifer both=air, ZERO ellipsoids cover it;
  production matches ref there per meter.
- **Aquifer exonerated** (16 probe cells + carve path). **ocean/cold_ocean
  have NO carvers in vanilla** — our port carves from every source (no
  effect this seed: 0 ocean sources region-wide; fix pending).
- **Tree displacement mechanism** (tree_trunks_dump): ≥97% of missing trunk
  bases have a same-type extra within 8 blocks, offsets scattered → CASCADE.

## Standing causal map

Streams align draw-for-draw when inputs match. Remaining ledger: trees ~44%
(cascade from gate-input flips), lush/sculk ~13% (scene microdiffs; port
draw-exact), pale garden ~3.5%. All converge on 1-cell terrain diffs
(surface rules fixed today — biggest single win).

## Next

1. **BUILD THE VANILLA PRE-DECORATION DUMPER** (the decisive tool): Java
   probe running the REAL pipeline up to carvers — fillFromNoise +
   buildSurface + applyCarvers over a 5x5 (ProtoChunk + real NoiseChunk;
   stubs pattern from ProbeCarveTrace/ProbeDecorate) — dump NDEC-style.
   Diff cell-level vs our export_predecorate. Every diff = a concrete fix
   target (surface rule application, steps 1-8, ticket_sim residual).
   Rationale: stripped-final replays CANNOT reconstruct the pre-tree scene
   (trees/patches modify the terrain they sit on — proven repeatedly:
   below-trunk dirt, patch moss, probe replay artifacts). All terrain
   components verified bit-exact individually: density, aquifer substance,
   cave-carve geometry (56/56 + 59/59 sources), surface_depth, canyon y-range
   (no canyon reaches the audited cells).
2. Fix ProbeTreeFirstFlip dump loader (document stateful semantics).
3. Ocean/cold_ocean carver-list gating (coastal seeds).
4. Waterlogged clay-pool top-fill per-column cascade ((34,5,13)-type).
5. Ruined portal loot tables (out of metric). AGENTS.md ref paths for
   12345/777 DO have `world/` prefix (stale doc).
6. `cargo test --workspace` before any push.

## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5 s
cold, ~5 s warm. Rust 1.98 · Temurin 25 · vineflower 1.12 (src at
tools/mc-decompiler/output/26.2/src) · probe classpath jar at
tools/nbt-ref/vanilla-fresh-424242/versions/26.2/. Playbook: docs/PARITY.md.

# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 1 Sep 2026 (Linux box), session 3.**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical 524-chunk world.

| Measurement | Value |
| --- | --- |
| SCAN 525, 1 Sep (evidence session, f99effe) | **98.90%**, ledger **568,429** cells |
| SCAN 525, 31 Aug (placement-fix session) | 98.86%, ledger 586,152 cells |
| f99effe delta | 583,032 → 568,429 (**−14,603**, 7x7 window + ref-footprint filter) |
| SCAN 528, seed **12345** (28 Aug) | ticket_sim **98.45%** (no regression) |
| SCAN 527, seed **777** (28 Aug) | ticket_sim **98.41%** (no regression) |
| Chunk (-14,-14) window r=0, 1 Sep | **99.08%** (baseline, unchanged) |

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
- **Nested-count placement pipeline** (31 Aug): `count` modifiers are now a
  RepeatingPlacement fan-out per stream position (rarity/biome filters run per
  base position, then the next count replicates survivors) instead of the old
  count-product loop. `wildflowers_birch_forest` (count:3→rarity 1/2→count:64)
  placed 54 blocks where vanilla places 0 — the rarity was consumed 192 times
  instead of 3, desyncing `trees_birch` downstream (the dark_oak gap root
  cause). Ledger −5,489.
- **Placement acceptance fixes** (31 Aug, after SCAN 588,823): three root
  causes of the dark_oak canopy cascade found via per-origin RNG diffing:
  (1) `TrapezoidInt.sample` for plateau=0 symmetric ranges is
  `nextInt(max+1) - nextInt(max+1)`, not the average of two uniforms — the
  average biased offsets toward the middle and shifted every wildflower/tree
  offset; (2) `parse_heightmap_kind` lowercased the heightmap name so
  `MOTION_BLOCKING` no longer fell back to OCEAN_FLOOR (which ignores water —
  plants were placed over water); (3) the generic `simple_block` branch placed
  without `canSurvive`, planting wildflowers on air over water where vanilla
  rejects. With all three, origin (-240,-224) wildflowers match vanilla 22=22
  exactly and the dark_oak tree cascade largely aligns (5/9 origins now
  identical). Ledger −2,671.

## Standing causal map (rewritten 1 Sep)

Tree-gap ledger attribution (SCAN 525 with `--writers`, /tmp/before_ledger.csv):
**87-89% of ALL tree-gap cells sit in the chunk BORDER zone** (dark_oak
42692/5543 border/core, pale_oak 25346/4075, tree-writer 149177/22560).
Missing canopy spread over 350 chunks at 300-400 cells each — diffuse stream
cascade, not a single cluster. Terrain writer 245k, tree 172k,
vegetation_patch 59k, simple_block 38k, ore 18k.

**dark_oak boundary objective was a ghost (1 Sep, PROVEN)**: the handoff's
"origins (-224,-240)/(-224,-208) place 0 logs vs vanilla 9/32" came from
ProbeTreeAttempts, whose per-origin replay order is ROW-MAJOR (center runs
5th, after (-14,-15)) — NOT the ref-world order. Ref world has NO trunk at
the probe's (-214,-225) base (0 logs y68-70, only y71+ fragments from other
trees); the only trunk the probe's n0 could see a canopy over, (-215,-223),
IS present in the ref and matches neutron's center draw-1 ACCEPT at y=68
(52 trunks). Per-origin "van=N" figures were per-ORIGIN totals across the
3x3 region, not target-chunk counts.

**Sim order validated against mined ore precedence (1 Sep)**: deco_pairs CSV
(45k mined ore-overwrite pairs, winner=later) constrains the (-14,-14) 5x5
window with 7 pairs; sim satisfies 6/7. The violated pair (-13,-14)<(-14,-14),
76 votes, was A/B tested twice via NEUTRON_DECO_CUSTOM_ORDER: both reorders
REGRESS chunk (-14,-14) parity (99.08 → 99.03 / 98.95). The 76-vote pair is
tainted by the very cascade it was mined from (contested-cell replay uses
neutron heightfield; tree-cascade windows corrupt it). Single-pair reorders
are DEAD as a lever; the remaining border gap needs the full constraint set
or a different mechanism.

Prior root cause stands: `wildflowers_birch_forest` (gif 22) acceptance still
differs (45 extra per origin), each wildflower raises WORLD_SURFACE in
`surface_water_depth_filter` and desyncs the NEXT origin's stream.
`place_below_trunk` dirt in trunks is CORRECT (vanilla trunkSetter adds it).

## Next

1. **Wildflower acceptance diff (gif 22)**: root of the border cascade —
   find why `dark_forest_vegetation`'s own placement accepts 45 extra
   wildflowers per origin (neutron heightmap/biome at draw time vs vanilla).
   Two-sided draw dump per origin, same method as the 31 Aug session.
2. **Border-zone mechanism**: 87% of tree-gap cells are in the chunk border.
   After (1), re-check whether the 7x7 window order still mispredicts any
   mined pair en masse (full-CSV constraint fit per window, not single pairs).
3. Ocean/cold_ocean carver-list gating (coastal seeds).
4. Waterlogged clay-pool top-fill per-column cascade ((34,5,13)-type);
   worst chunk on SCAN 525 is (2,9) (stone→clay 486, moss/water swaps).
5. Ruined portal loot tables (out of metric). AGENTS.md ref paths for
   12345/777 DO have `world/` prefix (stale doc).
6. `cargo test --workspace` before any push.

## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5 s
cold, ~5 s warm. Rust 1.98 · Temurin 25 · vineflower 1.12 (src at
tools/mc-decompiler/output/26.2/src) · probe classpath jar at
tools/nbt-ref/vanilla-fresh-424242/versions/26.2/. Playbook: docs/PARITY.md.

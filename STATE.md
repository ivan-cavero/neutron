# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 28 Aug 2026 (Linux box).**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical 524-chunk world (`new-mc-version.sh`).

| Measurement | Value |
| --- | --- |
| SCAN 525, 28 Aug (77b27a2) | **98.842%**, ledger **597,355** cells (was 599,711) |
| SCAN 528, seed **12345** fresh (28 Aug) | ticket_sim **98.45%** (= 27 Aug, no regression) |
| SCAN 527, seed **777** fresh (28 Aug) | ticket_sim **98.41%** (= 27 Aug, no regression) |
| 12345 window (6,-2) r=2 | 98.47 → **98.49%** (fallen_tree port) |

Meter speedup (6ae05e2): worker pool (cores−2 default, `PARITY_WORKERS`
overrides), streaming compare, vanilla NBT prefetch thread, per-worker
persistent NoiseCache over contiguous coord blocks. Full SCAN ~24 min →
**~4 min**, 2 cores free, output byte-identical, ledger identical.

## Closed 28 Aug (git log has evidence)

- **FallenTreeFeature PORTED** (77b27a2): was unknown-type no-op. Stump +
  horizontal log + `canPlaceEntireFallenLog` (validTreePos walk, non-sturdy
  gap ≤2) + trunk_vine + attached_to_logs decorators, RNG order exact.
  First flip at (2,0) gif17 draw5 now ACCEPTs. Ledger −1,022.
- **validTreePos = full 26.2 replaceable_by_trees tag** (013a17a): tall_grass,
  ferns, large_fern, flowers, bush/firefly/dead_bush, dry grass, seagrass,
  roots, vine/glow_lichen now free — previously (now-ID'd blocks) they
  REJECTED trees vanilla replaces-and-places. + CountPlacement count 0 =
  empty stream (trees_plains 0w19/1w1). Ledger −1,334.
- **Tree displacement mechanism proven** (tree_trunks_dump): missing trunk
  bases are ≥97% matched by a same-type extra within 8 blocks, offsets
  scattered ±3 → CASCADE, not different sets. First-flip oracle
  (tree_first_flip + ProbeTreeFirstFlip): flips are GATE-INPUT (terrain cell
  under draw), STREAM (decorator-cell drift inside earlier trees), or the
  port gaps above. Port is draw-exact when terrain matches (16/16 draws).
- **Lush caves: port is draw-exact** (moss_terrain_stream + lush_chain_dump
  re-run): 0 never-attempted cells; 79/94 TERRAIN-bucket cells in (0,-1)
  lost to per-column gates of `VegetationPatchFeature.placeGroundPatch/
  placeGround` evaluated on a scene already diverged upstream; 15/94 died at
  environment_scan; biome gate 0. Root = carver-edge + aquifer/water
  microdiffs (chain dump: step-6 `van_pre=air neu_pre=stone`).

## Standing causal map (updated)

Streams align draw-for-draw when inputs match (proven for trees, lush,
canyon). The remaining ledger is: (1) trees ~46% — cascade from gate-input
terrain flips; (2) lush/sculk clay+moss+cave_vines ~13% — scene microdiffs;
(3) pale garden patch ~3.5%. All three converge on **base-terrain parity:
carver edges + aquifer/water cells** (BASE is 99.64%; the missing cells sit
exactly on these features' gates).

## Next

1. **Cave-carver tunnel RNG parity** (the single surviving root): our density
   is solid where the final world is air at surface gate cells — e.g.
   (17,96,-5) neu density +0.0094 (solid) but carved; vanilla keeps
   grass_block. Raw pre-feature carve jitter = 419 cells / 25-chunk window
   (214 miss + 205 extra, 55% boundary-pair within Chebyshev 2, y med −13);
   everything else in van_air/solid classes is feature-block flips.
   Tool path: carve_from_chunk tunnel walk vs ProbeCarveHits; next dump =
   tunnel-cell trace for source chunks carving chunk (1,-1).
2. Waterlogged clay-pool top-fill: (34,5,13) clay-identical, van water vs
   neu clay at surface cell — per-column cascade inside the patch.
3. Fallen tree polish: sideways axis props if metric evolves.
4. Ruined portal: loot tables (out of metric).
5. AGENTS.md ref paths for 12345/777 DO have `world/` prefix (stale doc).
6. `cargo test --workspace` before any push.

## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5 s
cold, ~5 s with warm neighbor cache. Rust 1.98 · Temurin 25 · vineflower
1.12 (src at tools/mc-decompiler/output/26.2/src) · probe classpath jar at
tools/nbt-ref/vanilla-fresh-424242/versions/26.2/. Playbook:
docs/PARITY.md.

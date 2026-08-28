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

## Closed 28 Aug (git log has evidence) — session 2

- **minSurfaceLevel = vanilla bilinear** (312ed67): 16x16 surface-cell corner
  lerp of quart-quantized preliminary surface levels, f32 alphas, floor —
  was per-block direct eval with trunc. Ledger −2,464.
- **steep = vanilla one-directional** (0c7386b+): south>=north+4 else
  west>=east+4, chunk-clamped — was symmetric incl. self-row deltas.
  Ledger −579. Cumulative today: 599,711 → **594,312** (98.85%).
- **Carve geometry PROVEN identical** (b81b047): ProbeCarveTrace (real
  CaveWorldCarver over stone ProtoChunk + real aquifer) vs NEUTRON_CARVE_TRACE:
  56/56 source streams bit-identical for target (1,-1). The (17,96,-5) cell:
  density bit-exact (+0.00944026 both), aquifer both=air, ZERO ellipsoids
  cover it — earlier "our carver opened it" was wrong; that column matches in
  production per the meter ledger.
- **Aquifer exonerated twice** (16 probe cells + carve-path); **ocean/cold_ocean
  have NO carvers in vanilla** — our port carves from every source (does not
  affect this seed: 0 ocean sources region-wide; fix pending for coasts).
- **tree_first_flip replay harness UNRELIABLE** (documented, not yet fixed):
  ProbeTreeFirstFlip dump loader reads phantom pale_oak_leaves (index bug);
  build_stripped_buffer strips pale oak family while the OCEAN_FLOOR
  heightmap path in the replay yields y values inconsistent with the stripped
  scene; the "first diver at draw N" table is void until the oracle is
  rebuilt on the real pipeline. Trunk-base displacement itself is real
  (meter ledger: pale_oak leaves 30k/31k missing/extra).

## Next

1. **Rebuild the tree first-flip oracle on the REAL pipeline**: per-origin
   NEUTRON_TRACE_TREES on generate_chunk_cached (region random intact) vs
   ref trunk bases via the meter-grade loader; align draw-for-draw only on
   origins whose chunk matches vanilla at base (BASE 99.7%+).
2. Fix ProbeTreeFirstFlip dump loader index bug (phantom leaves).
3. Ocean/cold_ocean carver-list gating (coastal seeds).
4. Waterlogged clay-pool top-fill per-column cascade (34,5,13-type cells).
5. `cargo test --workspace` before any push.

## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5 s
cold, ~5 s with warm neighbor cache. Rust 1.98 · Temurin 25 · vineflower
1.12 (src at tools/mc-decompiler/output/26.2/src) · probe classpath jar at
tools/nbt-ref/vanilla-fresh-424242/versions/26.2/. Playbook:
docs/PARITY.md.

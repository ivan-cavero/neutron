# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 1 Sep 2026 (Linux box), session 4.**

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

## Closed (git log has full evidence)

- 77b27a2 FallenTreeFeature port (−1,022) · 013a17a replaceable_by_trees
  validTreePos + count-0 streams (−1,334) · 312ed67 bilinear minSurfaceLevel
  (−2,464) · 9d58a2e one-directional steep (−579) · b81b047 carve geometry
  proven bit-exact · 8c22a40 nested-count pipeline (−5,489) · 615443c
  TrapezoidInt / heightmap-parse / canSurvive (−2,671) · 6da2859 wavefront
  ticket sim · f99effe 7x7 window + ref-footprint filter (−14,603).

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

1. **Border-zone mechanism (PRIMARY)**: 87% of tree-gap cells are in the
   chunk border. Origin spillover-state divergence, not feature-port bugs.
   ProbeFullDecorate now supports `gif=N` RNG-stream capture (real
   placeWithBiomeCheck + full draw log) — use it per origin as the oracle.
   Next probe: diff the full vanilla run vs neutron per-origin trees_birch
   draws under IDENTICAL origin order (row-major), then isolate which
   spillover write each divergent gate saw.
2. **Wildflowers gif 22 closed as independent root (1 Sep)**: neutron's
   fan matches vanilla 15/16 positions for origin (-240,-240) (first copy
   (-219,69,-236) EXACT); residual per-copy diffs trace to origin-order
   spillover state, not the port. Block-level ledger impact: 38 cells.
   Old "neutron 67 vs vanilla 22" figure was pre-31-Aug-fix and per-origin
   totals, not ref-derived. Do not chase.
3. Ocean/cold_ocean carver-list gating (coastal seeds).
4. Waterlogged clay-pool top-fill per-column cascade; worst chunk on
   SCAN 525 is (2,9) (stone→clay 486, moss/water swaps).
5. Ruined portal loot tables (out of metric). AGENTS.md ref paths for
   12345/777 DO have `world/` prefix (stale doc).
6. `cargo test --workspace` before any push.

## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5 s
cold, ~5 s warm. Rust 1.98 · Temurin 25 · vineflower 1.12 (src at
tools/mc-decompiler/output/26.2/src) · probe classpath jar at
tools/nbt-ref/vanilla-fresh-424242/versions/26.2/. Playbook: docs/PARITY.md.

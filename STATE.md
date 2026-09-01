# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 1 Sep 2026 (Linux box), session 5.**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical 524-chunk world.

| Measurement | Value |
| --- | --- |
| SCAN 525, 1 Sep s5 (matching_fluids fix 25c4708) | **98.90%**, ledger **568,109** cells (−320) |
| SCAN 525, 1 Sep s3-s4 (evidence, f99effe) | 98.90%, ledger 568,429 cells |
| seed **12345** ratchet, 1 Sep | **98.54%** (was 98.45% 28 Aug — improved) |
| seed **777** ratchet, 1 Sep | **98.61%** (was 98.41% 28 Aug — improved) |
| Chunk (-14,-14) window r=0, 1 Sep | **99.09%** (was 99.08%) |

Meter speedup (6ae05e2): worker pool (cores−2, `PARITY_WORKERS`), streaming
compare, NBT prefetch, per-worker persistent NoiseCache. Full SCAN ~24 min
→ **~4 min**, 2 cores free, output identical.

## Closed (git log has full evidence)

- 77b27a2 FallenTreeFeature port (−1,022) · 013a17a replaceable_by_trees
  validTreePos + count-0 streams (−1,334) · 312ed67 bilinear minSurfaceLevel
  (−2,464) · 9d58a2e one-directional steep (−579) · b81b047 carve geometry
  proven bit-exact · 8c22a40 nested-count pipeline (−5,489) · 615443c
  TrapezoidInt / heightmap-parse / canSurvive (−2,671) · 6da2859 wavefront
  ticket sim · f99effe 7x7 window + ref-footprint filter (−14,603) ·
  25c4708 matching_fluids predicate (−320; see Next 1).

## Standing causal map (rewritten 1 Sep s5)

**Phantom firefly bushes SOLVED (25c4708)**: `matching_fluids` predicate
was missing from `eval_block_predicate` (fell into `_ => true`), so the
near-water gate of `patch_firefly_bush_near_water` passed unconditionally.
40 phantom bushes per 7x7 window (vanilla: 0); each raises WORLD_SURFACE
above OCEAN_FLOOR, so `surface_water_depth_filter` then rejected
vanilla-accepted trees on those columns. Proven by draw-index diff:
vanilla trees_birch n=8 ACCEPT (-219,-226,y=68) vs neutron REJECT y=0 at
the identical stream index after 119 matching draws.

Tree-gap ledger attribution: **87-89% of tree-gap cells sit in the chunk
BORDER zone** (dark_oak 42692/5543 border/core, pale_oak 25346/4075,
tree-writer 149177/22560); 350 chunks affected — spillover-gate divergence,
not feature-port bugs. Remaining writers: vegetation_patch 59k,
simple_block 38k, ore 18k, block_column 18k.

**dark_oak boundary objective was a ghost (1 Sep, PROVEN)**: the handoff's
"origins (-224,-240)/(-224,-208) place 0 logs vs vanilla 9/32" came from
ProbeTreeAttempts, whose per-origin replay order is ROW-MAJOR (center runs
5th) — NOT the ref-world order. Ref world has NO trunk at the probe's
(-214,-225) base; the (-215,-223) trunk IS in the ref and matches
neutron's center draw-1 ACCEPT at y=68. "van=N" figures were per-origin
totals across the 3x3 region, not target-chunk counts.

**Sim order validated against mined ore precedence (1 Sep)**: deco_pairs
CSV constrains the (-14,-14) 5x5 window with 7 pairs; sim satisfies 6/7.
The violated pair A/B regressed twice (99.08 → 99.03 / 98.95). Single-pair
reorders are DEAD as a lever.

Wildflowers gif 22: fan matches vanilla 15/16 positions for origin
(-240,-240) (first copy (-219,69,-236) EXACT); residual diffs are
origin-order spillover state. Ledger impact 38 cells. Do not chase.

## Next

1. **Border-zone mechanism (PRIMARY)**: continue the per-origin stream
   diff (ProbeFullDecorate gif=N oracle) for the remaining gate
   divergences — next writers: vegetation_patch 59k, simple_block 38k.
2. Ocean/cold_ocean carver-list gating (coastal seeds).
3. Waterlogged clay-pool top-fill per-column cascade; worst chunk on
   SCAN 525 is (2,9) (stone→clay 486, moss/water swaps).
4. Ruined portal loot tables (out of metric). AGENTS.md ref paths for
   12345/777 DO have `world/` prefix (stale doc).
5. `cargo test --workspace` before any push.

## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5 s
cold, ~5 s warm. Rust 1.98 · Temurin 25 · vineflower 1.12 (src at
tools/mc-decompiler/output/26.2/src) · probe classpath jar at
tools/nbt-ref/vanilla-fresh-424242/versions/26.2/. Playbook: docs/PARITY.md.
Probe rebuild: javac -cp "<libs-recursive>:<server.jar>" -d
tools/worldgen-probe/bin src/ProbeFullDecorate.java src/ProbeDecorate.java
src/ProbeTreeFirstFlip.java src/ProbePaleFlow.java; run with `gif=N` arg
for RNG-stream capture of one placed feature.

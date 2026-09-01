# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 1 Sep 2026 (Linux box), session 6.**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical 524-chunk world.

| Measurement | Value |
| --- | --- |
| SCAN 525, 1 Sep s5 (matching_fluids fix 25c4708) | **98.90%**, ledger **568,109** cells (−320) |
| seed **12345** ratchet, 1 Sep | **98.54%** (was 98.45% 28 Aug — improved) |
| seed **777** ratchet, 1 Sep | **98.61%** (was 98.41% 28 Aug — improved) |
| Chunk (-14,-14) window r=0 | **99.09%** |
| Chunk (2,9) window r=0 | 96.9% (worst; lush clay patches) |

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
  25c4708 matching_fluids predicate (−320; ratchet improved both seeds).

**Phantom firefly bushes SOLVED (25c4708)**: `matching_fluids` predicate
was missing from `eval_block_predicate` (`_ => true`), so the near-water
gate of `patch_firefly_bush_near_water` passed unconditionally. 40 phantom
bushes per 7x7 window (vanilla: 0); each raises WORLD_SURFACE above
OCEAN_FLOOR so `surface_water_depth_filter` rejected vanilla-accepted
trees on those columns. Proven: vanilla trees_birch n=8 ACCEPT
(-219,-226,y=68) vs neutron REJECT y=0 at the identical stream index
after 119 matching draws.

**dark_oak boundary objective was a ghost (1 Sep, PROVEN)**: the handoff's
"origins (-224,-240)/(-224,-208) place 0 logs vs vanilla 9/32" came from
ProbeTreeAttempts, whose per-origin replay order is ROW-MAJOR (center runs
5th) — NOT the ref-world order. Ref world has NO trunk at the probe's
(-214,-225) base. "van=N" figures were per-origin totals, not target-chunk
counts. Sim window order validated 6/7 against mined ore precedence;
the violated pair A/B regressed twice — single-pair reorders are DEAD.

## Standing causal map (1 Sep s6)

Tree-gap attribution: **87-89% of tree-gap cells sit in the chunk BORDER
zone** (dark_oak 42692/5543 border/core, pale_oak 25346/4075); 350 chunks.
Remaining writers: vegetation_patch 59k, simple_block 38k, ore 18k,
block_column 18k.

**NEXT TARGET (1 Sep s6, evidence on disk, no fix yet)**:
lush_caves_clay (gif 29) in chunk (2,9) — neutron 2758 clay writes vs
vanilla 1116 from origin (32,144) alone; ledger swaps stone→clay 1478,
moss→water 252, clay→water 303. Streams MATCH through 290 draws (17 bases'
in_square/height raws — raw height = y+64, uniform 0..320, above_bottom 0 —
plus every selector boolean identical). Divergence: vanilla's waterlogged
patch floods a long run of interior columns (float-run of per-column rolls
continues) while neutron's `surface_pts=interior` set comes up short →
dry moss/clay where vanilla has water. Suspect: `patch_is_exposed`
checks `is_face_sturdy(neighbor)` as a whole block, vanilla checks the
SPECIFIC opposite face `isFaceSturdy(dir.getOpposite())` — fluids/leaves/
dripleaf neighbours are sturdy on NO face but may be sturdy elsewhere;
also verify surface_pts iteration order (vanilla java HashSet hash order)
for the vegetation rolls — the first divergent roll is the patch's 6th.

Wildflowers gif 22: fan matches vanilla 15/16 positions for origin
(-240,-240); residual diffs are origin-order spillover state. Ledger
impact 38 cells. Do not chase.

## Next

1. **Waterlogged patch interior/exposure (PRIMARY)**: implement
   face-specific sturdiness in `patch_is_exposed` (vanilla
   `isFaceSturdy(dir.getOpposite())` per direction), then re-diff the
   gif=29 stream for origin (2,9) — oracle on disk:
   /tmp/opencode/stream_clay.log, /tmp/neu_clay_draws3.txt,
   dump /tmp/opencode/eo_c29.ndec (probe rebuild recipe in Perf).
2. Ocean/cold_ocean carver-list gating (coastal seeds).
3. Ruined portal loot tables (out of metric). AGENTS.md ref paths for
   12345/777 DO have `world/` prefix (stale doc).
4. `cargo test --workspace` before any push.

## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5 s
cold, ~5 s warm. Rust 1.98 · Temurin 25 · vineflower 1.12 (src at
tools/mc-decompiler/output/26.2/src) · probe classpath jar at
tools/nbt-ref/vanilla-fresh-424242/versions/26.2/. Playbook: docs/PARITY.md.
Probe rebuild: javac -cp "<all library jars>:<server.jar>" -d
tools/worldgen-probe/bin src/ProbeFullDecorate.java src/ProbeDecorate.java
src/ProbeTreeFirstFlip.java src/ProbePaleFlow.java; run with `gif=N` arg
for RNG-stream capture of one placed feature.

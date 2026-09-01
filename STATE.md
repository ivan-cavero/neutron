# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 1 Sep 2026 (Linux box), session 9.**

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
5th) — NOT the ref-world order. Sim window order validated 6/7 against
mined ore precedence; the violated pair A/B regressed twice — single-pair
reorders are DEAD as a lever.

## Standing causal map (1 Sep s9)

Tree-gap attribution: **87-89% of tree-gap cells sit in the chunk BORDER
zone**; 350 chunks affected. Remaining writers: vegetation_patch 59k,
simple_block 38k, ore 18k, block_column 18k.

**Waterlogged patch interior**: exposure rule itself PROVEN correct —
base-17 instrumentation: origin (39,84,145), r=5 (vanilla r=5), interior
62 all flooded ≈ vanilla's 68 water cells. The `+1` radius (vegetation.rs
:423-424, run-045 hack) A/B tested: removal REGRESSES 96.9→96.20% — keep.

**lush_caves_clay (gif 29) — biome-check divergence (PRIMARY, isolated
1 Sep s9)**: per-origin clay/water table for chunk (2,9)'s 9 origins
(vanilla vs neutron): (2,8) = vanilla **0 clay / 0 bases passed** vs
NEUTRON 1281 clay; neutron ~2x clay per origin elsewhere (1762/565,
2618/1317, 2758/1095). Root: the `minecraft:biome` placement check
verdicts differ — vanilla rejects ALL 62 bases of origin (2,8), neutron
accepts ~20. Chunk (2,8) sits at a lush/surface biome boundary.
Evidence on disk: /tmp/opencode/stream_clay_c29.log (vanilla gif=29
STREAM captures, per-origin draws), /tmp/opencode/van_28_bases.txt (the
62 base positions of origin (2,8), x|y|z), /tmp/opencode/pool_dump_c29.log
(neutron [pool] per-base triples), pool_dump2_c29.log (NSET with pass
origins).

## Next

1. **Biome-check divergence for lush_caves_clay (PRIMARY)**: for origin
   (2,8)'s 62 base positions (van_28_bases.txt), compare vanilla's
   biome@position (3D voronoi, quart resolution) vs neutron
   `biome_name_at`/`biome_id_at_block` at the same points. Suspects:
   quart-vs-block sampling, y-shift at biome boundaries, or the scan
   landing differing because terrain in chunk (2,8) differs (terrain was
   only proven bit-exact in the (-14,-14) window — verify with
   dump_terrain for (2,9) window first).
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

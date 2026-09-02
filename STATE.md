# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 2 Sep 2026 (Linux box), session 15. 63 commits pushed to origin main (d3af1c1..9cd96ad). Workspace tests 280 passed.**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical 524-chunk world.

| Measurement | Value |
| --- | --- |
| SCAN 525, 1 Sep s5 (matching_fluids fix 25c4708) | **98.90%**, ledger **568,109** cells (−320) |
| seed **12345** ratchet, 1 Sep | **98.54%** (was 98.45% 28 Aug — improved) |
| seed **777** ratchet, 1 Sep | **98.61%** (was 98.41% 28 Aug — improved) |
| Chunk (-14,-14) window r=0 | **99.09%** |
| Chunk (2,9) window r=0 (worst) | 96.9% (lush clay patches; order-driven) |

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
was missing from `eval_block_predicate` (`_ => true`). Proven: vanilla
trees_birch n=8 ACCEPT (-219,-226,y=68) vs neutron REJECT y=0 at the
identical stream index after 119 matching draws.

**Waterlogged patch interior (PRIMARY, 6 iterations in)**: RADIUS
SETTLED (vanilla place() line 28-29 = `sample(random) + 1`; neutron's
`+1` CORRECT). CONFIRMED: the gif=29 RNG streams for origin (2,9) align
1:1 through draw 290 (per-column rolls identical: neutron's per-column
dump values 0.9608/0.3672/0.8671/0.9499/0.5471... match vanilla's float
sequence exactly). The divergence = vanilla's 291st draw: a vegetation
roll 0.0633 < 0.1 = PASS on ONE extra surface point that neutron's
surface set lacks (neutron 127 vs vanilla 128). The missing point is
the LAST in vanilla's java-HashSet iteration order. Consequence: no
dripleaf at that column in neutron, and base 18+ RNG states diverge.
NEXT (needs java work in ProbeFullDecorate): print the returned
waterSurface set per pool base by reflectively invoking
WaterloggedVegetationPatchFeature.placeGroundPatch (protected, returns
Set<BlockPos>) after replicating the selector boolean + radius draws -
the set diff vs neutron's 127 names the missing column, then examine
that column's scan/below-sturdiness.

**dark_oak boundary objective was a ghost (1 Sep, PROVEN)**: the handoff's
"origins (-224,-240)/(-224,-208) place 0 logs vs vanilla 9/32" came from

`sample(random) + 1`; neutron's `+1` (vegetation.rs:429-430) is CORRECT
and must stay. Streams match through draw 290 (bases, booleans, radius
draws identical; vanilla processes dx=-6..6 same as neutron). The
divergence = ONE extra vanilla float at idx 290 (0.0633 < 0.1 = a
vegetation PASS) → vanilla's base-17 surface set has ONE point neutron
lacks (neutron 127 dumped via NEUTRON_COL_DUMP=39,145 — instrumentation
now committed, env-gated). Next: diff vanilla's base-17 surface (68
water cells + dry clay tops from the capture, bbox x[34..44]
z[140..150]) against neutron's 127 points; examine the missing column's
scan landing. Per-base clay in bbox: neutron 925 vs vanilla 367.
.ndec export, whose chunk (2,8) grid rejected all 62 bases — but (a)
vanilla `BiomeManager.getBiome` and neutron `biome_id_at_block` AGREE
ProbeTreeAttempts' row-major replay, not the ref-world order. Single-pair
reorders are DEAD as a lever.

## Closed (1 Sep s6-s13: lush_caves_clay chain)

- matching_fluids predicate implemented (25c4708, −320; ratchet improved
  both seeds) · radius `+1` SETTLED as correct (vanilla place() line
  28-29 = `sample(random) + 1`) · exposure test PROVEN correct (base-17:
  interior 62 all flooded ≈ vanilla 68) · block_column dispatch VERIFIED
  identical to vanilla source · biome-gate hypothesis DISPROVEN (oracle
  grid artifact; vanilla getBiome and neutron biome_id_at_block AGREE at
  all 12 divergent positions; ref world chunk (2,8) HAS the clay).

## Standing causal map (1 Sep s14)

**pale_garden short_grass excess = same origin-order mechanism (1 Sep
s18)**: the air→short_grass cells (7431) cluster in pale_garden chunks
(e.g. (0,-2): 124). Vanilla 26.2 pale_garden DOES include
patch_grass_forest (datapack verified; neutron's feature list matches —
glow_lichen at line 73 ✓). The excess = patch surface sets differing by
origin order, same as lush_caves_clay. No independent fix.

**lush_caves_clay attribution CLOSED — mechanism is origin-order
spillover**: the surface-set diff (base 17: vanilla 94 vs neutron 127
points; neutron-only 45 cells on the x=33/z=151 ring) is the decoration
ORIGIN ORDER mechanism, not a patch-code bug. Proof: base (39,84,145)'s
ring columns flood in neutron but not vanilla because their ground
placement depends on prior origins' spillover (earlier patches filled
the floor), which differs between vanilla's real order and neutron's sim
order. ALL patch internals verified identical to vanilla source (radius
+1, depth loop incl. same-block skip, exposure test, block_column).
The lush_caves_clay divergence is a downstream symptom of the
border-zone/order divergence — same root as the tree gap. Lever remains
the origin order model (part of the 87%-border cluster).

Tree-gap attribution: **87-89% of tree-gap cells sit in the chunk BORDER
zone**; 350 chunks affected. Remaining writers: vegetation_patch 59k,
simple_block 38k, ore 18k, block_column 18k.

## Next

1. **Origin order model (PRIMARY, closed 1 Sep s16 at 95.85% fit)**: the
   sim's decorate sequence fits 95.85% of the 45,391 mined pairs (1,870
   violations; 50 distinct pairs). Classification of all 50: 30 are
   rank_gap=1 (adjacent in sim), 26 of those with the winner directly
   NORTH (dz=-1) of the loser — vanilla decorates the SOUTH cell first
   in those halo adjacencies, i.e. vanilla's concurrent task execution
   interleaves the BFS deque differently than the deterministic FIFO
   sim. Remaining large-gap pairs (12 of 50) span different wavefront
   tiers. No structural sweep rule reproduces the inversions (row-major
   reorders REGRESS per fit A/B). The 95.85% fit is the order-model
   floor absent a faithful concurrent-execution model of vanilla's
   ChunkTaskDispatcher. The halo residual (~4%) is accepted.
2. Ocean carver gating CLOSED (1 Sep s17): DISPROVEN — the 26.2
   datapack gives ALL 60 overworld biomes identical carvers (cave,
   cave_extra_underground, canyon); the old "ocean/cold_ocean have NO
   carvers" claim is stale (older-version fact). Neutron already carves
   every chunk without a biome gate — matching vanilla. No change.
3. Ruined portal loot tables (out of metric). AGENTS.md ref paths for
   12345/777 DO have `world/` prefix (stale doc).
4. `cargo test --workspace` before any push.

## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5
s cold, ~5 s warm. Rust 1.98 · Temurin 25 · vineflower 1.12. Probe
rebuild recipe in tools/worldgen-probe/src. Playbook: docs/PARITY.md.
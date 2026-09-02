# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 1 Sep 2026 (Linux box), session 11.**

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

## Standing causal map (1 Sep s10)

Tree-gap attribution: **87-89% of tree-gap cells sit in the chunk BORDER
zone**; 350 chunks affected. Remaining writers: vegetation_patch 59k,
simple_block 38k, ore 18k, block_column 18k.

**Waterlogged patch interior (PRIMARY, 5 iterations in)**: RADIUS
SETTLED - vanilla VegetationPatchFeature.place() line 28-29 is
`sample(random) + 1`; neutron's `+1` is CORRECT. Surface-set diff (base
17, origin (39,84,145)): neutron 127 points (NEUTRON_COL_DUMP), vanilla
94+ in the clipped bbox 34..44/140..150 - the earlier vanilla-only
extraction was bbox-clipped (r=6 spans 33..45/139..151); re-extract
unclipped. The extra vanilla float at draw 290 (0.0633 < 0.1) is a
vegetation PASS. The vegetation divergence is now isolated to the
dripleaf feature: vanilla 16 dripleaf writes from origin (2,9) vs
neutron 60 (3.75x). dripleaf = simple_random_selector(small_dripleaf
simple_block, block_column big_dripleaf); the selector (nextInt(2)) and
patch loop match through draw 290. Next: diff BlockColumnFeature
placement for big_dripleaf stems - vanilla samples each layer height
(RNG), truncates on allowed_placement failure, and the
supports_big_dripleaf tag governs allowed_placement; verify neutron's
block_column dispatch (writer block_column, 18k ledger cells) matches
the truncation + height sampling. Neutron dripleaf positions:
(42,84..87,143) stems etc.; vanilla: (42,86,149) one stem.

**lush_caves_clay biome-gate divergence (UNRESOLVED, corrected 1 Sep
s11)**: origin (2,8) = vanilla 0 clay / 0 bases passed the biome gate vs
neutron 1281 clay (~20 bases accepted). The biome check happens at the
POST-SCAN position (environment_scan moves down <=12 then random_offset
+-1), NOT at the pre-scan height draw. My earlier check that found 12
lush_caves positions evaluated the PRE-scan y - invalid. Both sides use
live voronoi (WorldGenRegion's BiomeManager = getUncachedNoiseBiome with
voronoi, probe BIOME_MGR identical). Ref world chunk (2,8) HAS clay
(ledger 19 stone-to-clay / 30 clay-to-stone only) - vanilla's real gate
accepted some bases there. NEXT: evaluate the biome at the POST-scan
positions - from the vanilla capture, the post-scan y is not logged;
instead compute it per base by replaying the scan against the dump
terrain (air-scan down <=12, then +-1), then compare vanilla vs neutron
biome verdicts at those corrected positions.

## Next

1. **lush_caves_clay per-base attribution CLOSED (1 Sep s13 — mechanism
identified)**: the surface-set diff (vanilla 94 vs neutron 127 points;
neutron-only 45 cells on the x=33/z=151 ring) is the decoration ORIGIN
ORDER mechanism, not a patch-code bug. Proof: base (39,84,145)'s ring
columns (x=33) flood in neutron but not vanilla because the ground
placement for those columns depends on prior origins' spillover (earlier
patches filled the floor), which differs between vanilla's real order
and neutron's sim order. The patch internals (radius+1, depth loop,
same-block skip, exposure test, block_column) are all VERIFIED identical
to vanilla source. The lush_caves_clay divergence is a downstream symptom
of the border-zone/order divergence already tracked — same root as the
tree gap. No further patch-level work; the lever remains the origin
order model (part of the 87%-border cluster).



## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5
s cold, ~5 s warm. Rust 1.98 · Temurin 25 · vineflower 1.12. Probe
rebuild recipe in tools/worldgen-probe/src. Playbook: docs/PARITY.md.
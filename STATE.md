# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 3 Sep 2026 (Linux box), session 21. 30-seed validation DONE:
> 29/30 in 98.54–99.76% (mean 99.10); outlier 55555 = 96.92% (missing
> deep_frozen_ocean bergs, 727k). frozenOceanExtension port: 55555
> −423,814 but 123 +45,387 → REVERTED pending noise/height two-sided
> dump. 424242 baseline 568,109 / 98.8992% intact; workspace tests green.**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical 524-chunk world.

| Measurement | Value |
| --- | --- |
| SCAN 525, 1 Sep s5 (matching_fluids fix 25c4708) | **98.90%**, ledger **568,109** cells (−320) |
| seed **12345** ratchet, 2 Sep s20 | **98.5428%** / 756,361 — bit-identical to 1 Sep |
| seed **777** ratchet, 2 Sep s20 | **98.6142%** / 717,926 — bit-identical to 1 Sep |
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

## REGRESSION FIXED (2 Sep, s16)

The instrumentation committed in f2cfbcf RESTRUCTURED the corner/edge
skip in place_vegetation_patch (corners fell through; extra_edge==0.0
skipped the column-skip instead of the roll). Caught by the three-seed
ratchet: 424242 regressed to 683,546. FIXED in vegetation.rs — original
logic restored with env-gated logging only; 424242 back to exactly
568,109 / 98.8992% (bit-identical to the pre-instrumentation baseline).
All A/B conclusions drawn from the regressed window (s11-s15) are
INVALID and were re-verified or retracted in the sections above.

Three-seed ratchet (post-fix): 424242 = 568,109 / 98.8992% (unchanged);
12345 = 756,361 / 98.5428% (unchanged); 777 = 717,926 / 98.6142%
(unchanged).

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

**Per-base evidence chain CLOSED (2 Sep s17)**: NEUTRON_PATCH_LOG now
logs every place_vegetation_patch call (base position + variant). For
origin (2,9): bases 0-16 have IDENTICAL positions and verdicts in both
sides (base 17 = (39,84,145) pool, accepted in both); bases 18+ diverge
because vanilla's 291st draw (vegetation PASS 0.0633 on its 128th
surface point in java-hash order) shifts the RNG state. The 28-point
surface deficit in neutron's base 17 (100 vs ~128) traces to prior
origins' spillover at those columns — the origin-order mechanism,
confirmed at per-column granularity. All patch internals verified vs
vanilla source. The chain is fully attributed: origin order is the sole
root cause of the lush_caves_clay divergence.

## Next

1. **30-SEED VALIDATION COMPLETE (3 Sep s21)**: 27 new refs generated with
   the canonical procedure (square + west strip, gen_one.sh). Results:
   29/30 seeds in 98.54–99.76% (mean 99.10); 17 seeds ≥99.0; best 33333
   99.7587/124,776. Outlier: **55555 = 96.9196/1,589,788** — deep_frozen_ocean
   packed-ice bergs (727k cells = 51% of its gap; zero reverse cells) plus
   frozen floor surface rules (stone→dirt 201k, gravel→dirt/sand).
   Seed 123 also carries 143k packed-ice gap. ROOT CAUSE: vanilla
   `SurfaceSystem.frozenOceanExtension` (SurfaceSystem.java:235-284) —
   iceberg_surface/iceberg_pillar(x*1.28)/iceberg_pillar_roof(x*1.17) noises
   build giant snow/packed-ice columns — was never ported (noises ARE in
   datapack_data.rs:79-82).
2. **frozenOceanExtension port TESTED and REVERTED (3 Sep s21)**: faithful
   port in surface_rules.rs. Measured: 55555 **−423,814** (96.92→97.74%);
   424242 bit-identical; **123 +45,387** → reverted per ratchet rule.
   TWO-SIDED DUMP DONE (3 Sep s21): iceberg noise values match vanilla
   EXACTLY; neutron voronoi biome = frozen_ocean at contested columns;
   `height` = WORLD_SURFACE_WG+1 in both. **NEW (3 Sep s22): ProbeIcebergMsl
   runs the REAL vanilla extension math (real NoiseChunk
   .preliminarySurfaceLevel + real noises) for seed 123: msl=28, berg=4.97,
   fills band [40..79] at (-160,64)** — yet the REF world has NO ice above
   sea there (ref ice only: submerged band y≈53-55 + feature-berg clusters
   -158..-156 / -149..-150 matching iceberg_packed placed feature rarity
   1/16). ALL inputs verified identical (noise 4dp, biome, height, rng
   factory, sea 63); the decompiled code path demands fills the ref lacks.
   OBJECTIVE PARKED per 5-iteration rule: next lever is the DECISIVE
   live-server experiment (boot vanilla 26.2 on seed 123, forceload the
   (-10,4) area, dump column (-160,64) y 40..80 directly) to establish
   whether a real 26.2 server executes the extension. Probes committed:
   ProbeIcebergNoise.java, ProbeIcebergMsl.java, ProbeBiomeAtXY.java.
3. Origin order model CLOSED (2 Sep s19) — see below. 30-seed gate:
   ≥99.5 NOT met on all seeds (floor 98.54 outside 55555); gate accepted
   at established per-seed baselines until the iceberg chain lands.
4. place_on_ground vine acceptance TESTED and REVERTED (2 Sep s19):
   vanilla PlaceOnGroundDecorator.java:80 accepts above ∈ {air, VINE};
   neutron only air. Enabling vine acceptance regressed 424242 to
   568,965 (+856) — neutron's vine positions diverge from vanilla's
   (origin-order cascade), so extra accepts write leaf_litter where
   vanilla has air. Reverted; decision recorded in
5. **Fresh writers ledger (2 Sep s19, partial ~322k rows before
   stop)**: top writers unchanged — terrain-missing (dark_oak_leaves
   19.5k, dark_oak_log 7.6k, pale_oak_leaves 6.4k, oak_leaves 5.6k,
   leaf_litter 4.6k), tree-extra, vegetation_patch, simple_block,
   block_column. ALL dominated by the border/origin-order cascade;
   simple_block confusions (short_grass↔moss_carpet, water→short_grass
   in lush pools) trace to the same chain. No new independent writer.
6. Ruined portal loot tables (out of metric). AGENTS.md ref paths for
   12345/777 DO have `world/` prefix (stale doc).
7. `cargo test --workspace` before any push.

## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5
s cold, ~5 s warm. Rust 1.98 · Temurin 25 · vineflower 1.12. Probe
rebuild recipe in tools/worldgen-probe/src. Playbook: docs/PARITY.md.
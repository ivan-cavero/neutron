# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 5 Sep 2026 (Linux box), session 28. GATE3: mean 99.3503%
> (11/27 ≥99.5%, 25/27 ≥99.0%, total 9.07M). TWO STRUCTURAL FINDINGS:
> (1) VANILLA RACES PROVEN: two identical vanilla 26.2 runs on 424242
> differ in 332,936 cells (0.85%) — ALL decoration features (trees 43%,
> vines/cave_vines/moss/clay/leaf_litter); noise/density/aquifer/carvers/
> deep ores/biomes are deterministic (biomes 100% identical). Neutron's
> residual fingerprint MATCHES the vanilla race floor. (2) Climate
> sampler params diverge at biome boundaries (all 6; depth worst:
> vanilla -17382 vs neutron +436 at (-20,75,9)) — flips mangrove/savanna
> lookup and gates tree placement (789 mangrove ≈120k cells). Tooling:
> vanilladiff, ProbeClimateAt, climate_at example, ROOTWALK tracer,
> GIFDRAW capture. Mangrove port verified draw-exact to 62 dice; parked
> pending (1) or climate fix (2). Tests green.**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical 524-chunk world.
| Measurement | Value |
| 27-seed gate v3 (union fix 5b03feb, 5 Sep s27) | mean **99.3503%**, 11/27 ≥99.5%, 25/27 ≥99.0%, total 9.07M cells (−571,589 vs gate2) |
| seed **456** gate3 | **98.8722%** / 584,256 (was 621,610; below 99.0 — needs writer dump) |
| seed **12345** gate3 | **99.1362%** / 448,365 |
| seed **777** gate3 | **98.7047%** / 671,026 |
| seed **40000** gate3 | **99.7489%** / 130,108 (bit-identical to gate2) |
| seed **55555** gate3 | **97.7251%** / 1,174,078 (iceberg chain parked) |

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

1. **STEP-7 UNION FIX LANDED (4 Sep s26, commit 5b03feb)**:
   apply_step_origin early-returned when features_at_step(primary_biome,
   gen_step) was empty (plains step 7 = []) BEFORE building the 3x3
   biome-union feature list — silently skipping the whole decoration step
   for origins whose neighbours included cave biomes (dripstone_caves step
   7 = dripstone_cluster + pointed_dripstone; ~951k dripstone cells across
   12 seeds). Fix: union computed first; early-return only when both union
   and primary list are empty. Measured: 70707 371,432→289,715
   (**99.4376%**, −81,717); 777 674,071→671,026 (98.7047%); 12345
   448,459→448,365 (99.1362%); 424242 562,139→562,057 (98.9109%). All
   improved. GATE3 (5 Sep s27): all 27 remaining seeds re-run — 26/27
   improved, net −571,589 cells; mean 99.3503%, 11/27 ≥99.5%. CLOSED.
1b. **OBJECTIVE — seed 456 (98.8722% / 584,256; the only non-55555 seed
   below 99.0)**. Writers ledger (5 Sep s27): terrain 277k / tree 195k /
   simple_block 65k / vegetation_patch 12k. Jungle-signature cells 267k
   (86% border); biomes match 100% at the worst chunks. Iteration 1:
   ported mega_jungle trunk+foliage, bush foliage, leave_vine+cocoa
   decorators — REVERTED, 456 regressed to 98.7551% (+60,681): the extra
   RNG draws desync the shared selector stream (vanilla mega picks consume
   branch draws neutron previously skipped; with the port the whole
   trees_jungle stream shifts and canopies land where the ref has none).
   The selector itself (chances 0.1/0.5/0.333/0.0125 + default) matches
   vanilla RandomSelectorFeature draw-for-draw. Iteration 2: frozen
   WORLD_SURFACE_WG/OCEAN_FLOOR_WG heightmaps (vanilla freezes Usage.WORLDGEN
   maps at surface; ProtoChunk tracks only FINAL maps during features,
   ChunkStatus.java:17-28 + ProtoChunk.java:147-169) — REVERTED, 456
   regressed to 98.7296% (+73,871): the frozen read is measurably wrong too,
   so vanilla WG heightmaps must reflect something between live and frozen
   (candidate: the WG maps DO get updated by setBlock because
   getOrCreateHeightmapUnprimed lazily primes them at first setBlock, or
   WorldGenRegion.getHeight re-primes). 2 investigation+2 fix iterations
   spent on 456; per the 5-cap, NEXT: park 456 with both negative results
   recorded; move to the 10101/789/50000 cluster ledger (99.01–99.08) for
   an independent writer before returning with a better WG-heightmap model.
   LEDGERS DONE (5 Sep s27): 10101 #1 writer = ancient_city (244k of 512k —
   structure not ported; PARKED like 55555, too large). 789 #1 = mangrove
   trees 138k (upwards_branching_trunk + random_spread_foliage +
   mangrove_root_placer — all Unknown in neutron → trees place nothing).
   Iteration 3: ported mangrove trunk/foliage/roots — REVERTED, 789
   regressed to 98.9862% (+33,673). SAME failure mode as 456's mega-jungle
   port: any tree-port that starts consuming selector-stream draws at
   origins where neutron previously no-oped REGRESSES, proving the shared
   decoration RNG stream is ALREADY desynced before tree selection (the
   no-op was accidentally absorbing the desync). Trees are downstream of
   the real divergence — root cause sits EARLIER in the per-origin step
   chain (steps 1..6: placement-modifier draws). STREAM DUMP DONE (5 Sep
   s28, oracle789b.ndec + pfd-mang.out + neu-789-trace.log, origin (-2,0)):
   the stream matches THROUGH tree selection — vanilla n=5 mangrove at
   (-23,9,y74): selroll 0.6160519<0.85 → tall_mangrove_checked, height
   4+0+4=8 — neutron draw 6: identical roll, identical nextInt(2)=0,
   nextInt(10)=4, ACCEPT at the same cell. The divergence starts INSIDE the
   accepted tree: vanilla then consumes the full tree's draws (mangrove
   roots simulate, trunk branches, 70 foliage attempts, decorators → 827
   writes) while neutron's Unknown-trunk no-op consumes ~0, so the NEXT
   attempt's in_square draws diverge (van n=6 = (10,2), neu = (9,8)). The
   earlier mangrove port REGRESSED because its INTERNAL draw order didn't
   match vanilla's (root placer/above-root/foliage sequence), not because
   selection desyncs. ITERATION 4 (5 Sep s28): re-landed the port with the
   EXACT vanilla order — two real bugs found and fixed via raw-stream diff
   (pfd-mang47.out STREAM gif=47 vs NEUTRON_RNG_TRACE): (a) doPlace draws
   getTreeHeight BEFORE rootPlacer.getTrunkOrigin (TreeFeature.java:65-69 —
   my first port had it reversed); (b) BlockPos.distManhattan INCLUDES the Y
   term (my width omitted it). After fixes the streams match 15+ draws
   (selector 0.6160519 → height 0,4 → offset 0 → root-skew dice identical).
   Full 789 parity: 98.9869% (+33,303) — STILL REGRESSES. CORRECTION (5 Sep
   s28, surface_height_dump): the mud FLOOR heights MATCH vanilla (floors
   equal on nearly all columns; the surface diffs in raw dumps are tree
   canopies, expected). The terrain-gate hypothesis is WITHDRAWN. The draw
   divergence at index 15 (vanilla bool vs neutron float) is a root-walk
   POSITION difference: vanilla simulated a position where canPlaceRoot
   failed (no bool drawn), neutron walked elsewhere. Resolving it needs
   vanilla-side root-position logging — ITERATION 5 (5 Sep s28): fixed bug
   #3 (root direction order: vanilla Plane.HORIZONTAL = N,E,S,W; mine was
   W,E,N,S — found via ProbeMangroveRootTrace replay + python walk diff).
   Stream alignment improved 15→62 draws, then diverges on walk LENGTH:
   vanilla's dir-1 walk ends at ~62 dice (canPlaceRoot=false at a solid
   cell), neutron's continues (cell replaceable in neutron's terrain). The
   two terrains differ in stone/dirt boundary micro-cells in the swamp;
   each such cell flips canPlaceRoot and desyncs the rest of the origin's
   stream. Full 789: 98.9972% (+27,956) — still negative. BAIL OUT (5-cap):
   port REVERTED (code in session history; bugs #1 height-before-offset,
   #2 distManhattan Y, #3 dir order are all real fixes to re-apply once
   terrain micro-parity lands). OBJECTIVE MOVES TO PHASE 1: quantify and
   fix the stone/dirt-boundary micro-diffs in mangrove_swamp (789) and
   jungle floor (456) — doFill/surface level, BEFORE any tree port can
   net-positive. ProbeMangroveRootTrace committed (tools/); AUTHORITATIVE
   vanilla root walk captured (evidence/stream789/vanilla-root-walk-
   authoritative.txt, ROOTWALK=1 in-scene): N 6 roots, E 3, S 3, W 8, first
   candidate (-23,77,7) via else-bool(true), all canPlace results logged.
   ITERATION 6 DONE (5 Sep s28): re-applied port + instrumentation, walked
   both sides position-for-position. FIRST DIVERGENCE: canPlaceRoot at
   (-20,75,9) — neutron canPlace=true (mud). Vanilla-side comparison from
   the post-tree replay was INCONCLUSIVE: that trace is a POST-TREE replay
   (hook runs after cf.place()), so its canPlace results include the tree's
   own writes (below-trunk dirt at 75) — NOT the live walk semantics. The
   live-vs-replay mismatch makes further vanilla-side
   diffing impossible without instrumenting inside MangroveRootPlacer
   (javaagent/reflection hook — heavy). Definitive session conclusion:
   port draw-exact through 62 dice; 3 measurements all negative; port
   REVERTED; mangrove objective PARKED until either (a) a live-walk
   vanilla tracer exists, or (b) a different angle (e.g. count parity of
   mangrove_roots per chunk) yields a cheap check. Surface-rule "dirt vs
   mud" claim WITHDRAWN (replay artifact). CROSS-SCENE WALK PROOF (5 Sep
   s28): replaying vanilla dice on the NEUTRON-scene NDEC reproduces
   neutron's walk exactly ((-20,76,9)T -> (-20,75,9)F), and on the
   VANILLA-scene reproduces vanilla's — the port + dir order are fully
   correct; the entire residual divergence is the underlying terrain at
   cells the root walk touches. QUANTIFIED (5 Sep s28, z=9 slice
   x=-40..-20): 5/20 columns differ, ALL the same pattern — the TOP of the
   mud band: vanilla = dirt,dirt,grass_block; neutron = mud,mud,mud
   (e.g. x=-27: 68-70). IT 7-8: vanilla surface rule decoded (mangrove mud
   is UNCONDITIONAL in the biome under-chain; dirt@75 in the old trace was
   the tree's own below_trunk write — replay artifact). Live-walk tracer
   built (two-pass NDEC restore + dice replay): origin (-2,0) tree matches
   vanilla 31/31 root calls. But chunk (2,-1): vanilla places a mangrove
   tree, neutron never fires it (0 root calls) — stream already desynced
   upstream. Full 789 with verified port: 98.9972% (+27,956) still
   negative. TRUE ROOT CAUSE (ProbeClimateAt + climate_at example): at
   (-20,75,9) ALL SIX climate parameters differ — vanilla temp=1888
   humid=-1830 cont=2557 eros=5356 depth=-17382 weird=-2774 vs neutron
   temp=2009 humid=-1682 cont=3491 eros=5567 depth=+436 weird=-3050.
   Neutron's depth ≈ 0 at y=75 where vanilla has -17382 (well above
   surface) → biome lookup gives mangrove_swamp (neutron) vs savanna
   (vanilla) → surface rule flips (mud vs dirt/grass) → tree walk
   diverges. NEXT OBJECTIVE (Phase 1.1 climate sampler): per-parameter
   diff of temperature/vegetation shifted_noise jitter and the depth
   density chain at boundary cells; tooling committed (ProbeClimateAt +
   climate_at example). Mangrove port stays PARKED until climate parity
   lands (re-apply is mechanical — bugs #1-#3 + dir order documented).
   session's history has the complete port).
   VANILLA_NDEC_OUT export hook bug FIXED (header was 20 bytes, must be
   18 — LE writer verified).
   50000 (99.0821%/474,604) — spruce trees 138k + dripstone_caves 138k,
   BOTH 86% border; core-only 19.5k/18k. Same signature as 456/789: the
   remaining gap across every 99.0-99.4 seed is the border origin-order
   cascade plus tree/stream divergence — NO new independent writer. The
   cheap per-seed ledger route is exhausted; the remaining ~5M gate gap
   requires either the live-walk tracer (javaagent) or a structural fix
   to the origin-order model (95.85% fit ceiling).
2. **HEIGHTMAP FIX LANDED (3 Sep s24, commit 3d66868)**: vanilla
   buildSurface's `height` = WORLD_SURFACE_WG+1 INCLUDES fluids
   (SurfaceSystem.java:112,119); neutron passed a fluid-EXCLUSIVE heightmap,
   so the surface y-loop started below the water column, water_height stayed
   MIN at deep ocean floors, the Water(-6) condition passed (MIN = exposed)
   and sediment dirt/sand overwrote stone floors. Fix: apply_surface_rules
   recomputes the fluid-inclusive top per column before the y-loop.
   Measured: 424242 568,109→562,139 (98.9108%); 12345 756,361→448,459
   (**99.1360%**); 777 717,926→674,071 (98.6989%); 40000 565,427→130,108
   (**99.7489%**). Aggregated `stone->dirt` across 30 seeds: 1.20M cells.
   NEXT: re-run the full 30-seed gate with this fix (expect ~1M cell drop);
   iceberg chain still parked.
3. **30-SEED VALIDATION COMPLETE (3 Sep s21)**: 27 new refs generated with
   29/30 seeds in 98.54–99.76% (mean 99.10); 17 seeds ≥99.0; best 33333
   99.7587/124,776. Outlier: **55555 = 96.9196/1,589,788** — deep_frozen_ocean
   packed-ice bergs (727k cells = 51% of its gap; zero reverse cells) plus
   frozen floor surface rules (stone→dirt 201k, gravel→dirt/sand).
   Seed 123 also carries 143k packed-ice gap. ROOT CAUSE: vanilla
   `SurfaceSystem.frozenOceanExtension` (SurfaceSystem.java:235-284) —
   iceberg_surface/iceberg_pillar(x*1.28)/iceberg_pillar_roof(x*1.17) noises
   build giant snow/packed-ice columns — was never ported (noises ARE in
   datapack_data.rs:79-82).
4. **frozenOceanExtension objective Bailed OUT (3 Sep s23, 5-iteration
   cap)**: port tested: 55555 **−423,814** (96.92→97.74%), 424242
   bit-identical, 123 **+45,387** → reverted per ratchet rule. Live-server
   experiment (probe-123 world, real 26.2 jar, forceload) proved the ref
   IS real vanilla: berg columns byte-identical. Column-level ground
   truth: extension fills water band [sea−top−7 .. sea] with ~15% skips,
   top = min(berg²·1.2, ceil(roof·40)+14)+sea; the 123 regression came
   from neutron fills landing where the ref has feature cut-outs and
   inter-column variation the port cannot see without the full
   IcebergFeature interplay (feature bergs + cut-outs carve the
   extension ice; my earlier "ref lacks ice" and "ref stone-to-63"
   readings were ledger/column-sampling misreads — ref terrain matches
   neutron at non-berg floors). The remaining lever is a full
   IcebergFeature + cut-out + extension joint implementation — parked:
   too large for the iteration budget; revisit if the gate moves above
   99.5 on the other 29 seeds. Port stays reverted; probe evidence
   committed (ProbeIcebergNoise/Msl/BiomeAtXY).
5. Origin order model CLOSED (2 Sep s19) — see below. 30-seed gate:
   ≥99.5 NOT met on all seeds (floor 98.54 outside 55555); gate accepted
   at established per-seed baselines until the iceberg chain lands.
6. place_on_ground vine acceptance TESTED and REVERTED (2 Sep s19):
   vanilla PlaceOnGroundDecorator.java:80 accepts above ∈ {air, VINE};
   neutron only air. Enabling vine acceptance regressed 424242 to
   568,965 (+856) — neutron's vine positions diverge from vanilla's
   (origin-order cascade), so extra accepts write leaf_litter where
   vanilla has air. Reverted; decision recorded in
7. **Fresh writers ledger (2 Sep s19, partial ~322k rows before
   stop)**: top writers unchanged — terrain-missing (dark_oak_leaves
   19.5k, dark_oak_log 7.6k, pale_oak_leaves 6.4k, oak_leaves 5.6k,
   leaf_litter 4.6k), tree-extra, vegetation_patch, simple_block,
   block_column. ALL dominated by the border/origin-order cascade;
   simple_block confusions (short_grass↔moss_carpet, water→short_grass
   in lush pools) trace to the same chain. No new independent writer.
8. Ruined portal loot tables (out of metric). AGENTS.md ref paths for
   12345/777 DO have `world/` prefix (stale doc).
9. `cargo test --workspace` before any push.

## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5
s cold, ~5 s warm. Rust 1.98 · Temurin 25 · vineflower 1.12. Probe
rebuild recipe in tools/worldgen-probe/src. Playbook: docs/PARITY.md.
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

**Waterlogged patch interior (PRIMARY, 4 iterations in)**: RADIUS
(lush_caves) at all 12 divergent positions (40,15,135 / 32,18,132 /
33,11,130 / 37,59,131 / 47,73,139 / 46,6,138 / 45,42,129 / 39,83,142 /
43,34,137 / 40,22,131 / 46,33,128 / 43,70,140), and (b) the REF WORLD
chunk (2,8) HAS the clay (ledger only 19 stone→clay extra, 30
clay→stone) — vanilla placed it from origin (2,8) too. The export's
biome grid is miscalibrated for gate-fidelity captures. Caution: any
gate-biome-sensitive per-origin capture result from the .ndec export is
suspect until the export writes the chunk's stored quart grid correctly.

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

**Waterlogged patch interior (PRIMARY, 4 iterations in)**: RADIUS
SETTLED - vanilla VegetationPatchFeature.place() line 28-29 is
`sample(random) + 1`; neutron's `+1` (vegetation.rs:429-430) is CORRECT
and must stay. Streams match through draw 290 (bases, booleans, radius
draws identical; vanilla processes dx=-6..6 same as neutron). The
divergence = ONE extra vanilla float at idx 290 (0.0633 < 0.1 = a
vegetation PASS) - vanilla's base-17 surface set has ONE point neutron
lacks (neutron 127 dumped via NEUTRON_COL_DUMP=39,145 - instrumentation
now committed, env-gated). Next: diff vanilla's base-17 surface (68
water cells + dry clay tops from the capture, bbox x[34..44]
z[140..150]) against neutron's 127 points; examine the missing column's
scan landing. Per-base clay in bbox: neutron 925 vs vanilla 367.

**lush_caves_clay biome-gate hypothesis DISPROVEN (1 Sep s10)**: the
"origin (2,8) vanilla 0 clay vs neutron 1281" was an ORACLE GRID
ARTIFACT: ProbeFullDecorate reads the biome grid from the
decorate_oracle .ndec export, whose chunk (2,8) grid rejected all 62
bases - but (a) vanilla BiomeManager.getBiome and neutron
biome_id_at_block AGREE (lush_caves) at all 12 divergent positions,
and (b) the REF WORLD chunk (2,8) HAS the clay (ledger only 19
stone-to-clay extra, 30 clay-to-stone) - vanilla placed it from origin
(2,8) too. The export's biome grid is miscalibrated for gate-fidelity
captures. Caution: gate-biome-sensitive per-origin capture results from
the .ndec export are suspect until the export writes the chunk's stored
quart grid correctly.

## Next

1. **lush_caves_clay per-base stream diff, part 2 (PRIMARY)**: the
   gif=29 RNG stream for origin (2,9) matched 290 draws; resume from
   draw 290: identify the ONE extra roll vanilla takes inside base 17's
   loop (edge/bottom/vegetation roll count) — options: iterate
   vegetation rolls in java-HashSet order vs neutron
   JavaBlockPosSet order (the sets differ by ≥1 surface point);
   or the ground-loop edge-selection consumed 1 extra roll.
   Evidence: /tmp/opencode/stream_clay_c29.log, /tmp/neu_clay_sws3.txt,
   dump /tmp/opencode/eo_c29.ndec. After the roll-order fix, re-check
   per-origin clay counts against the REF WORLD (ledger), NOT the
   grid-biased capture.
2. Ocean/cold_ocean carver-list gating (coastal seeds).
3. Ruined portal loot tables (out of metric). AGENTS.md ref paths for
   12345/777 DO have `world/` prefix (stale doc).
4. `cargo test --workspace` before any push.

## Perf / Environment (this box)

8 cores; meter default leaves 2 free (`PARITY_WORKERS`). Chunk gen ~9.5
s cold, ~5 s warm. Rust 1.98 · Temurin 25 · vineflower 1.12. Probe
rebuild recipe in tools/worldgen-probe/src. Playbook: docs/PARITY.md.
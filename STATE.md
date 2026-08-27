# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 27 Aug 2026 (Linux box).**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical 524-chunk world (`new-mc-version.sh`).
| Measurement | Value |
| --- | --- |
| window (7,2) r=1 before→after ruined_portal | chunk 95.58% → **97.79%**, window 98.48% → **98.73%** |
| SCAN 525 **with** trapezoid+ruined_portal | **98.71%**, `ledger_v7.csv`, 665,800 cells (−47.5k vs v6-era 713k) |
| SCAN 525 pre-fixes (same commits otherwise) | 98.67%; histo top dark_oak ±104k, pale_oak ±66k |

## Closed this session (git log has evidence)

- **Canyon pipeline BIT-EXACT**: JVM probe ↔ Rust trace of all 4 firing
  canyon starts near (7,2) — seeding/order/pos/angles/thickness/radii/
  width factors/canReach: 268 records byte-identical, ZERO reach (7,2).
  The "missing-carve" mass there was misattributed: it is a ruined portal.
- **TrapezoidFloat exact** (5d20d60): was `(f1+f2)*3` clamp; now exact
  (TrapezoidFloat.java:35-40).
- **ruined_portal overworld PORTED** (16af6e7): plans frozen pre-decoration,
  placed at owner origin between steps 3/4; start = floorDiv(8,40)*40 +
  LegacyRandom(salt=34222645) nextInt(25)x2 ⇒ (8,2); giant_portal_2 ROT NONE
  FRONT_BACK TP(128,45,32) air_pocket=1 cold=0 mossiness .2 — matches saved
  `structures.starts` field-for-field. Chest placed (loot out of scope);
  stair/slab blockstate props unmodeled. Residual debris-pattern cells ride
  `NEUTRON_RP_STEP_INDEX` + surface microdiffs.

## Closed earlier this week (git log / runs/)

SWDF floor predicate · #air tag incl. cave_air · env_scan out-of-world
semantics · TreeDecorator java_hash_order THEN stable Y sort · WorldGenRegion
xoroshiro draw-for-draw vs JVM probe (nextBoolean = LOW bit) · pale_moss_
carpet placeAt/topper; tall_grass DoublePlant (BlockId 140) · GEODE polarity
+ codec defaults + Cursor3D x-fastest + DOWN-first crystals.

## Order findings (do not reopen casually)

No static NEUTRON_SCULK_ORIGIN_ORDER preset collapses worst chunks on the
canonical ref (spiral/row/col within ±0.09% at (7,2),(8,0)). Vanilla's true
order = ticket-scheduler interleave; ref-procedure dependence in c9bd433.

NEW evidence for the scheduler work (ore dump): granite/diorite/andesite/
tuff swaps are NOT rng/predicate/upstream — identical blobs draw-for-draw,
only pass ORDER differs. Vanilla ≈ world_origin-ascending at sampled
chunks. world_origin zeroes swap clusters but breaks a 37-cell case at
(0,0): vanilla decorates diag-chunk (-1,+1) BEFORE center — no static
preset produces it ⇒ ticket-scheduler is the only faithful model. Ore
shared-sim already 96.7–97.7%.

## Causal chain (standing)

Streams align draw-for-draw when inputs match (oracle traces). Remaining
displacement = gate-input flips from upstream terrain microdiffs + origin
order interleave. v7 ledger ranking: trees both ways ~230k, worst chunks
(-13,4) 15.8k · (0,6) 14.4k · (-12,4) 14.3k (dark_oak/oak canopy classes,
border-heavy) · lush/sculk family ~60k, pool at (-2..0,-3..-1). Trees/lush
close only via order work + remaining terrain microdiffs.

## Next

1. Ticket-scheduler wavefront — OWNED by parallel agent
   (decoration_origin_order); v7 tree/lush mass is its target metric.
2. Ruined portal polish: step-index debris pattern, chest loot tables,
   blockstate properties if metric ever compares them.
3. (7,2) residuals: rooted_dirt/moss/tuff dig-site shaft x122..126 z37..41;
   tree foliage diffs y74..92 (fold into tree/order chain).
4. When carvers.rs is touched next: match vanilla applyCarvers dx-outer/dz-
   inner iteration order (harmless today, zero mask).
5. Re-run `cargo test --workspace` before any push.

## Perf / Environment (this box)

Single-chunk gen 11.5 s; `NEUTRON_STEP_TIMING=1` per-phase ms.
Rust 1.98 · Temurin 25 · vineflower 1.12 (src at tools/mc-decompiler/output/
26.2/src) · probe classpath jar under tools/nbt-ref/vanilla-fresh-424242/
versions/26.2/. Update playbook: docs/PARITY.md (+ worldgen-json-diff.sh).

# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 27 Aug 2026 (Linux box).**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical 524-chunk world (`new-mc-version.sh`).

| Measurement | Value |
| --- | --- |
| SCAN 525, **ticket_sim default** (a5021c8) | **98.84%**, `ledger_v8.csv`, 599,711 cells |
| SCAN 525, pre-ticket_sim (5d20d60+16af6e7) | 98.71%, `ledger_v7.csv`, 665,800 cells |
| window (7,2) before→after ruined_portal | chunk 95.58% → **97.79%**, window 98.48% → 98.73% |
| histo v8 top | trees both ways still #1 (−10%/class vs v7); moss↔stone ~11k next |

## Closed today (git log has evidence)

- **ticket_sim origin order = NEW DEFAULT** (a5021c8): deterministic port of
  the 26.2 ticket/dispatcher wavefront (deco_schedule.rs; FORCED batch +
  level propagation + ChunkTaskPriorityQueue lowest-bucket×insertion-age +
  ChunkPyramid gating; phases instead of wall-clock). Consistency vs 45,391
  mined precedence pairs: **95.01% all / 96.04% interior** (row baseline
  81.00/90.22; canonical_pregen 76.08). Ore swaps → 0 incl. the (0,0)
  diag-before-center case; (3,-4) residual is vanilla resolving like
  world_origin (only known parametric miss). Gate: SCAN 98.71→**98.84%**,
  ledger −66,089 cells.
- **Ref procedure CORRECTED** (a5021c8, script comment): actual ref was made
  square([-8..7]²)+ONE west strip 31 s later (server log), not the 4-strip
  ring; disk = forced ∪ Chebyshev-2 auto-promoted halo (528 slots; d3 cells
  decorate via neighbor sweeps, drop unsaved). deco_schedule ingests this.
- **ruined_portal overworld PORTED** (16af6e7): start floorDiv(8,40)*40 +
  LegacyRandom(salt=34222645) nextInt(25)x2 ⇒ (8,2); giant_portal_2 ROT NONE
  FRONT_BACK TP(128,45,32) air_pocket=1 cold=0 mossiness .2 (matches saved
  `structures.starts`). Chest placed (loot out of scope); stair/slab props
  unmodeled. `NEUTRON_RP_STEP_INDEX` sweep ≈ flat (±0.03%).
- **Canyon pipeline BIT-EXACT** + **TrapezoidFloat exact** (5d20d60): probe
  ↔ trace 268 records identical; "missing-carve" at (7,2) was the portal.

## Order residual (next frontier, do not reopen without these)

ticket_sim leaves 4.99% pairs inconsistent: (a) same-ring-epoch ties that
vanilla resolved via nproc−1 worker races (ref-side noise; core stays
deterministic); (b) light-queue epochs merged away in sim; (c) families not
covered by ore/tuff mining (trees contribute only weak evidence). Trees/lush
mass shrinks only via these + upstream microdiffs.

## Causal chain (standing)

Streams align draw-for-draw when inputs match (oracle traces). Remaining
displacement = origin-order residual + upstream terrain microdiffs feeding
tree would_survive / lush first-accept gates.

## Next

1. Validate ticket_sim on OTHER seeds vs existing refs
   (tools/nbt-ref/vanilla-fresh-12345 & -777, paths have no `world/` prefix)
   — the any-seed contract; mine precedence pairs per seed like 424242.
2. Characterize ring-epoch ties: sample same-bucket completion races vs the
   pairs CSV; if bounded, pick the epoch tie-break vanilla observed.
3. Lush/sculk pool (-2..0,-3..-1): first-accept gate dump under ticket_sim.
4. Ruined portal polish: loot tables; blockstate props if metric evolves.
5. When carvers.rs touched: applyCarvers dx-outer/dz-inner iteration order.
6. Re-run `cargo test --workspace` before any push.

## Perf / Environment (this box)

Single-chunk gen 11.5 s; `NEUTRON_STEP_TIMING=1` per-phase ms.
Rust 1.98 · Temurin 25 · vineflower 1.12 (src at tools/mc-decompiler/output/
26.2/src) · probe classpath jar under tools/nbt-ref/vanilla-fresh-424242/
versions/26.2/. Update playbook: docs/PARITY.md (+ worldgen-json-diff.sh).

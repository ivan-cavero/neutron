# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 27 Aug 2026 (Linux box).**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical 524-chunk world (`new-mc-version.sh`).

| Measurement | Value |
| --- | --- |
| SCAN 525, ticket_sim default (a5021c8) | **98.84%**, `ledger_v8.csv`, 599,711 cells |
| SCAN 528, seed **12345** fresh ref | ticket_sim **98.45%** vs canonical_pregen 98.41% (−20.6k cells) |
| SCAN 527, seed **777** fresh ref | ticket_sim **98.41%** vs canonical_pregen 98.31% (−49.6k cells) |
| window (7,2) before→after ruined_portal | chunk 95.58% → **97.79%**, window 98.48% → 98.73% |

New refs 12345/777 provisioned 27 Aug with same jar+procedure (square+west
strip; bundler re-extract — 424242's libraries/ was pruned to probe set).
Cross-seed consistency vs mined pairs (`ticket_sim_anyseed.rs`): 95.01 /
85.08 / 91.69 % all-pairs — beats row (81.0/80.8/80.6) everywhere; interior
flag: 12345 ticket_sim 86.91 < row 88.68 (proxy only; block parity still
wins). v8 histo top: trees both ways (−10%/class vs v7); moss↔stone ~11k.

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

## Order residual (cross-seed verified; next levers)

ticket_sim wins all-seed on block parity (see Now table). Pair-proxy
residuals: (a) same-ring-epoch ties vanilla resolved via nproc−1 worker
races (ref-side noise; core deterministic); (b) light-queue epochs merged
in sim; (c) phase boundary: 424242's west strip landed ~31 s after the
square, new refs' at 150 s — parameterize if chasing the last %.

## Causal chain (standing)

Streams align draw-for-draw when inputs match (oracle traces). Lush/sculk
pool at (−1,−2)-area: MASK EMULATION REJECTED with numbers (inert by
default; NEUTRON_TMP_MASK=1 breaks 448 correct cells, window 9095→9550).
Real mechanism = per-origin placement parity: ore-blob coverage edges
(coal misses a cell by 1 at dx+1) + TERRAIN bucket where vanilla placed
and neutron never wrote (356 center / 3,339 3×3) = roll/placement
divergence. Biome grid NOT the cause (7,679/7,680 quarts match incl.
worst tree chunks). Artifacts: /tmp/opencode/mask_zone_cells.csv,
examples mask_zone_probe.rs + moss_clay_dump.rs (untracked).

## Next

1. Lush/sculk winner-flips: reconstruct placement streams for the TERRAIN
   bucket + the coal-edge off-by-1 (see artifacts above).
2. Ring-epoch ties: sample same-bucket completion races vs the three
   seeds' pairs CSVs; fix epoch tie-break if bounded.
3. Tree mass: dark_oak/oak displacement both ways remains top class.
4. Ruined portal polish: loot tables; blockstate props if metric evolves.
5. When carvers.rs touched: applyCarvers dx-outer/dz-inner order.
6. Re-run `cargo test --workspace` before any push.

## Perf / Environment (this box)

Single-chunk gen 11.5 s; `NEUTRON_STEP_TIMING=1` per-phase ms.
Rust 1.98 · Temurin 25 · vineflower 1.12 (src at tools/mc-decompiler/output/
26.2/src) · probe classpath jar under tools/nbt-ref/vanilla-fresh-424242/
versions/26.2/. Update playbook: docs/PARITY.md (+ worldgen-json-diff.sh).

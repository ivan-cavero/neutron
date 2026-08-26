# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 26 Aug 2026 (Linux box).**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1`
+ `PARITY_LEDGER=<csv>`. Ref = canonical concentric-square 524-chunk world
(`new-mc-version.sh`; old corner-quadrant refs are dead for measurement).

| Measurement | Value |
| --- | --- |
| 424242 r=1 center window | 98.10% → **98.18%** (SWDF/air-tag/env_scan/decorator-order/region-RNG/carpet) |
| 424242 SCAN 524 chunks (pre-geode-fix) | 98.43%, ledger 806k cells (`ledger_v3.csv`) |
| 424242 SCAN 524 chunks (post-geode-fix) | **98.52%**, ledger 764,064 cells (`ledger_v4.csv`) — geode fix recovered ~42k cells |

## Closed this session (git log has evidence)

- **SWDF floor predicate**: OCEAN_FLOOR = `blocks_motion()` only; leaf_litter
  carpets raise WS above OF ⇒ depth 1 rejects tree attempts like vanilla
  (predicates.rs `column_water_depth`).
- **#air tag** includes cave_air (matching_block_tag shortcut + is_in_tag).
- **env_scan out-of-world semantics**: immediate fail after leaving build
  height, no final target re-check; top bound exclusive (`>= WORLD_TOP`).
  Both call sites (feature_dispatch/mod.rs, feature_ports/sequence.rs).
- **TreeDecorator.Context order**: java_hash_order FIRST on raw add order,
  THEN stable Y sort of logs AND leaves (trunks sort was missing after hash).
- **WorldGenRegion.random ported**: xoroshiro factory chain
  `main.fromHashOf("minecraft:worldgen_region_random").forkPositional().at(minCorner)`
  — validated draw-for-draw vs JVM probe (`region_random_dump` example +
  ProbeRegionRandom). NOTE: XoroshiroRandomSource.nextBoolean = `(next&1)!=0`
  (LOW bit override), NOT the interface default top-bit form.
- **pale_moss_carpet placeAt** + topper dice from region random; BASE/topper
  split as internal ids PaleMossCarpet/PaleMossCarpetTopper (same name out);
  tall_grass DoublePlant (new BlockId::TallGrass=140, lower+upper cells,
  below∈#dirt + air-above gate).
- **GEODE root cause** (was ~29k cells): cell-pass branch polarity INVERTED
  (layer chain ran when shell<outerCrust ⇒ only crack-air ever wrote) +
  missing codec defaults distribution_points UniformInt(3,4)/point_offset(1,2)
  + Cursor3D x-fastest pass order + Direction.values() DOWN-first crystal
  faces. Stream alignment proven identical to JVM probe first (trace shows
  same origin/points/crack_size); polarity fix recovered -205 cells in chunk
  (-11,-1) alone. gif=2/step=2 CONFIRMED origin-invariant (agentF sweep).

## Order findings (do not reopen casually)

No static NEUTRON_SCULK_ORIGIN_ORDER preset collapses worst chunks on the
canonical ref (spiral/row/col within ±0.09% at (7,2),(8,0)). Vanilla's true
order = ticket-scheduler interleave; ref-procedure dependence documented in
commit c9bd433. coal_ore/gold_buried residual (~14k+ cells) is order-
sensitivity of discard>0 ores (agentD: row order matched chunk (-10,6)
exactly on the OLD ref).

## Gap ranking (v3 ledger, pre-geode-fix; recount on v4)

trees displaced both ways ~250k · lush moss/clay-on-stone swaps ~60k (terrain
desync poisoned attempts — agentB: divergence onset == first-ACCEPT index;
corr(base-mismatch,lush-mismatch)=0.637) · granite/diorite/andesite mutual
swaps ~20k · coal_ore 14k · short_grass/tall_grass/leaf_litter ~33k.

## Next

1. ~~Land v4~~ DONE: **98.52% / 764k cells**.
2. Lush swaps need BASE convergence inside caves (carver-level), prerequisite
   for moss/clay/cave_vines recall.
3. Tree displacement: needs ticket-wavefront simulation or per-area order
   detection — biggest single lever left (~30% of gap). air-family rows are
   mostly tree displacement both directions (agentF-style attribution).
4. gold_buried residual rides along with any order work.
5. Re-run `cargo test --workspace` before any push.

## Perf

Single-chunk gen 11.5 s. `NEUTRON_STEP_TIMING=1` per-phase ms.

## Environment (this box)

Rust 1.98 · Temurin 25 · vineflower 1.12 (src at tools/mc-decompiler/output/
26.2/src) · probe classpath jar under tools/nbt-ref/vanilla-fresh-424242/
versions/26.2/. Update playbook: docs/PARITY.md (+ worldgen-json-diff.sh).

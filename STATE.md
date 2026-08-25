# STATE — Neutron

> Facts only. History: `runs/` (archive). Method: `AGENTS.md` v2.
> **Updated 26 Aug 2026 (Linux box, env rebuilt from scratch).**

## Now

Worldgen 1:1 vs vanilla **26.2**. Meter = `region_parity` + `PARITY_SCAN=1` +
`PARITY_LEDGER=<csv>` (cell-exact TSV + GAPS ranking). Coverage = full-status
chunks in the ref region dir.

| Ref | Parity | Notes |
| --- | --- | --- |
| 424242 (484-ch ref, corner-quadrant pregen) | spiral default **98.32%**, row order **~98.3%** (scan pending) | tree family dominates gap |
| 424242 (concentric-square pregen — canonical) | re-baseline pending | matches neutron `spiral` wavefront |

## Ref procedure is part of the measurement (26 Aug finding)

Decoration order embedded in a ref world depends on HOW chunks were loaded;
neutron's origin order (`decoration_origin_order`, default `spiral`) must match it.

- Vanilla truth: chunk C's features see spillover (trees, leaf_litter carpets)
  only from origins that decorated BEFORE C; gates read that state live:
  SurfaceWaterDepthFilter (world_surface − ocean_floor > 0 rejects carpeted
  cells) and would_survive (rejects on canopies). One gate flip desyncs the
  whole step-9 RNG cascade → whole-tree displacement (~40% overplacement when
  center decorates first against a row-pregenerated ref).
- Old refs (81 chunks) = one centered forceload square → center-out ticket
  wavefront ≈ `spiral` ✓. A 4-corner-quadrant pregen instead embeds per-row
  sweeps → mismatched with spiral.
- Canonical procedure now scripted: `tools/nbt-ref/new-mc-version.sh <ver>
  <seed>` = RCON boot + ONE centered 16×16-chunk forceload square (256 =
  vanilla command cap), settle, then one outer ring in 4 strips ≤256 each,
  save-all flush, stop. Keep this stable across future refs/versions.

## Environment (this box — rebuilt 25 Aug)

Rust 1.98 · Temurin 25 · vineflower 1.12 (decompiled src at
`tools/mc-decompiler/output/26.2/src`) · refs via new-mc-version.sh.
Probe classpath jar: `tools/nbt-ref/<name>/versions/26.2/server-26.2.jar`.
Vanilla quirks hit: `randomTickSpeed` gamerule gone; forceload area cap 256
chunks; RCON responses can lag minutes on big forceloads (fire-and-forget +
poll world files works); `pause-when-empty-seconds=0` needed while pregenning.

## Oracle fixes (26 Aug)

- **ProbeDecorate seeded decoration with CHUNK coords** — vanilla
  `setDecorationSeed` takes the chunk MIN-CORNER BLOCK coords
  (`ChunkGenerator.java:327`). All old replay numbers were seed-desynced
  (the "~27% fidelity" was this, not physics).
  `tools/worldgen-probe/src/ProbeTreeAttempts.java` (block coords) reproduces
  the ref exactly: pale_garden_vegetation 0/16 attempts survive (all dark_forest
  biome), positions identical to neutron's draws cell-for-cell.
- Neutron RNG/position chain verified exact vs Java for origin (112,32):
  same 32 first positions both features; divergence starts at GATES reading
  cross-origin state (SWDF drop on prior-origin litter at (126,79,35)).

## Gap ranking FULL SCAN 424242 (800k cells = 1.68% missing, corner-quadrant ref)

lush moss/clay/stone swaps ~18% · dark_oak leaves+log ~15%+ · pale_oak ~18%+
· short_grass/tall_grass/leaf_litter ~10% · cave_vines ~4% · coal_ore ~3.6%
· oak ~3%. WORST chunks: (7,2),(2,10),(8,0),(3,10),(-1,-2).

## Closed earlier (git log has evidence)

spiral origin order sweep · ore skipped-draw fix · mineshaft side-exit ·
step 3 ON · trapezoid dispatcher · trunk/foliage placer line-exact ports
(DarkOakTrunkPlacer lean+branches, foliage skip logic, Plane.HORIZONTAL order)
· placement chain (count→in_square→SWDF(0)→heightmap OF→biome) JSON-exact ·
place_on_ground predicates now all-leaves faithful (commit 864cae3) ·
vegetation patch placeGround return semantics (already-ground joins surface
set) · would_survive = SUPPORTS_VEGETATION below only · OCEAN_FLOOR heightmap
= blocksMotion() only, no fluid term (26.2).

## Perf

Single-chunk gen 11.5 s. `NEUTRON_STEP_TIMING=1` per-phase ms.

## Next (one question)

Re-baseline against the canonical concentric-square ref (regen running);
expect tree-family gap to collapse if wavefront theory holds. Then rerun
PARITY_SCAN ledger and re-rank. Lush clay/moss swap family (#1 by cells,
scattered single-cell jitter at many Y bands) is next in line after trees —
needs its own two-sided dump (probe `VegetationPatchFeature` surface scan on
identical inputs). Do NOT trust old 12345/777 Windows-box numbers against
new-procedure refs without regenerating those refs too.

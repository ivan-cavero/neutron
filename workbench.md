# F2d run-046 — Cross-chunk input model · gauntlet workbench

> Active loop since 16 Aug 2026. **Bar (human decision R43, run-045)**:
> **mechanism parity** — same seeds/streams/algorithms as vanilla; deterministic
> phases → 100% block match multi-seed; vegetation/sculk → same RNG stream 1:1.
> See `runs/run-045.md` (baseline) and `runs/run-046.md`.

**Budget:** no round cap; stop only if (a) bar wins, (b) 2 rounds without
improvement, (c) human stops it.

## Baseline (run-045 close)

| seed | REGION 3×3 ALL | dominant gap |
| --- | --- | --- |
| 12345 (6,-2) | 97.75 % | input-model (border) + vegetation |
| 424242 (0,0) | **97.28 %** | clay inflated 840 vs 497 + lush/pale missing |
| 777 (0,0) | ~99.4 % | residuals |

## run-046 bar (AC from run-045, untouchable)

- [ ] clay 840 → **~497** (lush/pale missing ≤20%, recall ≥80%) on 424242
- [ ] `region_parity` border diffs down **≥30%** on 12345; cores no regression
- [ ] REGION 424242 ≥ **97.28%** and 12345 ≥ **97.75%** (no regression)
- [ ] **777 no regression** (multi-seed ratchet, added 16 Aug)
- [ ] `cargo test --workspace` green; worldgen 59/59
- [ ] Bar untouchable: do not edit measurement examples/tests

## Units

| # | Unit | Files | Bar (per unit) |
|---|------|-------|----------------|
| U1 | Clean build (clay_probe WIP broken) | `crates/neutron-worldgen/Cargo.toml`, `examples/clay_probe.rs` | `cargo test --workspace` exit 0, 59/59, registered examples compile |
| U5 | Cross-chunk input model (run-046) | `generator.rs`, `region_buf.rs`, `feature_dispatch.rs`, decoration scheduler | full run-046 bar (above) |

## Round log

| Round | U1 | U5 | Notes |
|-------|----|----|-------|
| R0 (16 Aug) | 🔴 no compile | — | `clay_probe.rs:116` `sections` missing; run-045 WIP |

### R0 details

- `cargo test --workspace` → exit 1: `error[E0425]: cannot find value 'sections'`
  in `crates/neutron-worldgen/examples/clay_probe.rs:116` (example registered
  in Cargo.toml with `autoexamples = false`).
- 8 warnings in neutron-server (dead fields: compression_threshold, game_mode,
  current_tick, get_player_info...).

| R1 (16 Aug) | ✅ **PASS** (blind critic) | — | cargo test exit 0, 59/59 worldgen, 25 measurement examples compile, src/ untouched. run-045 WIP committed (0689ff8) after human gate |
| R1 (16 Aug) | — | 🔍 full investigation | Vanilla model verified (ChunkGenerator.java L263-341, WorldGenRegion.java, ChunkPyramid L20): origin-major, center first, neighbors in CARVERS, masking. Gaps C1-C8 + plan D. **Builder U5 pending** |
| R2 (16 Aug) | — | 🔨 builder U5 reports | clay 840→**466** (≈497 ✓); REGION 12345 97.94% ✓; cores improve; but REGION 424242 97.20% (-0.08 ❌), border -7.5% (needs ≥30% ❌), lush/pale recall 48% (pre-existing ❌). Gap diagnosed: pale oaks 0/167 overlap (steps 1-5/8 not ported alter state at step 9). **Critic pending** |
| R2 (16 Aug) | — | ❌ **FAIL** (blind critic) | Reproduced everything from scratch: tests 59/59 ✓, REGION 12345 97.94% ✓, clay 466 ✓, hygiene ✓. FAILS: REGION 424242 97.20% (regression), border -7.5%, lush/pale recall 48.27% (missing 51.73%). **Biggest gap: vegetation decoration placement** (pale oaks 3936 missing, clay 2126, moss 1300) → drags REGION 424242. Fix: port full FancyTreeFeature (2×2 trunk, canopy, branches) + lush_caves_clay placement chain. **→ Builder R3** |
| R3 (16 Aug) | — | 🔨 builder R3 (working tree) | FancyTreeFeature pale oak (2×2 trunk, canopy, branches) + TreeDecorator sort + creaking_heart/pale_moss + lush_caves_clay placement chain. **Measured (LEAD, 16 Aug):** REGION 424242 **97.38%** ✓ (bar ≥97.28), REGION 12345 **97.94%** ✓ (bar ≥97.75), clay **466** ✓ (~497), lush/pale recall **62.94%** (bar ≥80 ❌, was 48.27), border -7.5% (bar ≥30 ❌), **777 96.29%** (baseline ~99.4 ❌ REGRESSION). tests 59/59 ✓. **Bar NOT met**: recall <80%, border unchanged, 777 regresses. **Critic pending** |

## Status

**R3 committed (91862d4): recall lush/pale 48→62.94%, 424242 97.38%, 12345 97.94%,
clay 466 ✓ — bar NOT met (recall <80%, border -7.5%, 777 regression 96.29%).
Blind critic pending. Next: isolate 777 regression + port MossVegetation/CaveVine.**

*Last update: 16 Aug 2026 — R3 (LEAD-measured)*

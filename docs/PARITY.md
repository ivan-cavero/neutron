# PARITY — keeping worldgen 1:1 across Minecraft updates

> Method contract: `AGENTS.md`. Live facts: `STATE.md`. This file is the
> update-day playbook + the vanilla↔neutron component map.

## 1. The three pillars

1. **Canonical reference worlds** — `tools/nbt-ref/new-mc-version.sh <ver>
   <seed>` downloads the server jar, boots it headless with RCON and
   pregenerates a **centered 16×16-chunk forceload square plus one outer
   ring** (concentric wavefront). The decoration ORDER embedded in a ref
   world depends on how chunks were loaded; this procedure is part of the
   measurement and must not change casually.
2. **Cell-exact meter** — `neutron-parity` generates neutron chunks and
   compares every block (and optionally every quart biome) against the ref
   `.mca` files with one strict decoder:
   ```bash
   # fast window (9 chunks, ~90 s); headline metric is CORE%
   cargo run --release -p neutron-parity -- \
     --ref tools/nbt-ref/<ref>/world/dimensions/minecraft/overworld/region \
     --seed <seed> --center 0,0 --radius 1 --json out.json
   # whole-ref audit with per-cell ledger (~30 min)
   cargo run --release -p neutron-parity -- --ref <regiondir> \
     --seed <seed> --scan 1 --ledger ledger.csv --json out.json
   ```
   Guarantees: decode errors are fatal (never compare garbage), proto chunks
   are counted and skipped, unknown vanilla names are flagged as UNMAPPED
   version drift instead of silently diffing as air, reports are
   byte-identical for identical inputs (BTreeMap + total sort keys), and the
   ledger keeps the historical `x,y,z,class,zone,vanilla,neutron` CSV format.
   Exit codes for gating: `--min-core PCT` -> 1 below threshold,
   `--strict` -> 2 on unmapped names or missing ref chunks.
   The legacy `region_parity` example still exists for continuity but the
   CLI above is canonical.
3. **JVM oracle** — `tools/worldgen-probe/src/Probe*.java` run the REAL
   vanilla feature classes against neutron-exported terrain dumps
   (draw-for-draw RNG traces). When Rust and Java disagree on identical
   input, the probe says which draw diverges first.

## 2. Update-day runbook (new MC version)

```bash
# 0. pin everything at the last green commit; record old parity % in STATE.md

# 1. canonical ref for the new version (also downloads the new jar)
tools/nbt-ref/new-mc-version.sh 26.3 424242

# 2. DATA-level diff: exactly which datapack JSONs changed
tools/worldgen-json-diff.sh \
  tools/nbt-ref/vanilla-fresh-424242/versions/26.2/server-26.2.jar \
  tools/nbt-ref/vanilla-fresh-424242/versions/26.3/server-26.3.jar

# 3. decompile the new sources for code-level diffs
cargo run -p mc-decompiler -- download <newver>
cargo run -p mc-decompiler -- decompile <newver>
# output lands in tools/mc-decompiler/output/<newver>/src

# 4. re-point probes at the new jar (classpath anchor:
#    tools/nbt-ref/vanilla-fresh-424242/versions/<newver>/server-<newver>.jar
#    + its libraries/) and rerun the ones covering changed families
#    (ProbeTreeAttempts, ProbeOreFlow, ProbeDecorate…)

# 5. run the version-drift tripwires — they name exactly which blocks or
#    biomes the new version added/renamed before you touch anything:
cargo test -p neutron-parity --test version_tripwires

# 6. update registries (tripwires tell you which):
#    a) embedded JSON copies under crates/neutron-worldgen/src/data/
#       (only the ones step 2 listed)
#    b) NEW/RENAMED BLOCKS: surface.rs BlockId enum + from_name +
#       vanilla_name/block_name (+ server protocol_data remap)
#    c) NEW/RENAMED BIOMES: biome/source.rs biome_id consts +
#       feature_dispatch/predicates.rs biome_id_to_name +
#       feature_catalog.rs OVERWORLD_BIOME_ORDER (shifts ALL global
#       FeatureSorter indices!) + multi_noise parameter list points
#       in src/data/biome_params.bin — NOTE: no repack tool exists yet;
#       repacking is manual today (known gap)
#    d) port code changes via the map below

# 7. measure: fast window first, then full scan with ledger; fix by gap
#    rank; commit per closed family with the % move in the message
```

Triage rule of thumb: a changed `placed_feature`/`configured_feature` JSON is
usually a data-only fix (copy + maybe index shifts); a changed Java feature
class needs a two-sided dump before touching Rust (AGENTS.md §6).

## 3. Component map (vanilla class → neutron module)

| Vanilla (26.2) | Neutron | Notes |
| --- | --- | --- |
| `RandomState`, `XoroshiroRandomSource` | `src/rng.rs`, `src/worldgen.rs` | xoroshiro128++, factory hashing (MD5) |
| `PositionalRandomFactory` | `src/positional.rs` | `Mth.getSeed`, `at()`, `fromHashOf().forkPositional()` |
| `WorldGenRegion.random` | `RegionBuf::region_random` + `WorldgenState::region_random` | one stream per origin pass, `worldgen_region_random` factory |
| `ChunkGenerator.applyBiomeDecoration` | `feature_dispatch/mod.rs::apply_step_origin` | decorationSeed = chunk min-corner BLOCK coords |
| `FeatureSorter` / global indices | `src/feature_catalog.rs` | per-step global feature indices + salts |
| placement modifiers (`PlacedFeature`) | `feature_dispatch/mod.rs::place_placed_feature_step` (+ `feature_ports/sequence.rs`) | lazy per-attempt chain, env_scan/SWDF semantics |
| block predicates / tags | `feature_dispatch/predicates.rs` | `is_in_tag`, heightmap kinds, `blocks_motion` |
| `SurfaceWaterDepthFilter` | `predicates.rs::column_water_depth` | WS − OF(=blocksMotion only) |
| `VegetationPatchFeature` | `feature_dispatch/vegetation.rs` | lush/pale patches, placeGround surface set |
| `SimpleBlockFeature` | `feature_dispatch/mod.rs` simple_block arm | incl. DoublePlant + `MossyCarpetBlock.placeAt` |
| `TreeFeature`, trunk/foliage placers | `src/tree/*` | java_hash.rs = HashSet iteration order sim |
| `TreeDecorator.Context` | `tree/mod.rs` (hash order → stable Y sort) | logs AND leaves |
| `PaleMossDecorator` | `tree/decorators.rs` | shuffledCopy, hanger rolls |
| `OreFeature` + trapezoid heights | `src/features.rs` | blob walk line-exact; discard consumes RNG |
| carvers | `src/carvers.rs` | write air/cave_air per overworld config |
| aquifer / density / noise router | `src/aquifer.rs`, `src/density/`, `src/noise.rs`, `src/worldgen.rs` | doFill stage |
| sculk spreading | `src/sculk/` | charge cascade ≈99% writer attribution |
| multiface growth | `src/multiface_spreader.rs` | sculk_vein, glow lichen |

## 4. Version-drift tripwires (`cargo test -p neutron-parity`)

- `ref_block_palettes_fully_mapped` — every block name in on-disk refs must
  resolve through `BlockId::from_name`. Current porting debt is an explicit
  allowlist in that test; a NEW unmapped name fails with its exact name.
- `ref_biome_palettes_fully_mapped` — every biome in refs must resolve via
  `biome_id_to_name`.
- `block_id_tables_consistent` — `block_name()`/`vanilla_name()`/`from_name()`
  stay in sync (first run caught missing mushroom-block arms).

## 5. Rules that keep parity honest

- Never edit measurement examples/tests to make a number pass.
- A hypothesis without a two-sided dump (seed + coords + `Class.method`
  lines both sides) does not get implemented.
- Commit when a family matches or the % moves; run `cargo test
  -p neutron-worldgen` for worldgen-only commits, `--workspace` before push.
- Refs are gitignored; regenerate them only via the canonical script.

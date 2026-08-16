# How Minecraft 26.2 world generation works (and where we are)

> 15 Aug 2026 · Written for Neutron from the decompiled sources of the real jar
> (`tools/vanilla-extract/decompiled/`). Each phase cites the Java class that
> implements it and the Rust module that ports it.

## The full pipeline (what happens when you request a chunk)

Each chunk goes through a **chain of states** (`ChunkStatus`). Each state requires
the 3×3 neighbors to have reached the previous state. The order:

```
EMPTY → STRUCTURE_STARTS → STRUCTURE_REFERENCES → BIOMES → NOISE
      → SURFACE → CARVERS → FEATURES (liquid/hot/cool/etc.) → INITIALIZE_LIGHT → LIGHT → SPAWN
```

When the server needs a chunk (a player sees it), it climbs the chain by
dependencies: requesting chunk (6,-2) at LIGHT means generating the 8 neighbors up
to INITIALIZE_LIGHT, and outer rings up to earlier states. **That is what our
`RegionBuf` 3×3 replicates.**

### Phase 1 — BIOMES: `ChunkGenerator.createBiomes`

- Source: `ChunkGenerator.java` → `BiomeResolver` from `Climate.Sampler`.
- Climate is 6 noises (temperature, vegetation, continentalness, erosion, depth,
  weirdness) → 7498-point table (overworld parameter list).
- Biome choice: the point with highest `fitness` (product of deltas).
- Smooth borders: **voronoi at 1:4 scale** (4×4×4 blocks per biome cell) with
  `obfuscateSeed` (SHA-256 of the seed).
- Rust: `biome/` (multi-noise + voronoi). Params in `data/biome_params.bin`.

### Phase 2 — NOISE: terrain shape

- Source: `NoiseChunk.java`, `NoiseRouter.java`, `RandomState.java`.
- A **NoiseRouter** = tree of density functions (noise + operations). Derived from
  the seed with `RandomState.create(...)` — each noise has its own sub-seed derived
  in fixed order (`NoiseData.bootstrap`): this makes terrain **100% deterministic
  per chunk**.
- The router produces one density per 4×8×4 cell, then **interpolates** to blocks
  (`NoiseInterpolator`) with lerp over cells.
- On final density: `finalDensity < 0` = solid. The aquifer (`NoiseBasedAquifer`)
  decides water/lava in holes; `OreVeinifier` decides copper/iron veins +
  tuff/granite.
- Rust: `density.rs`, `noise.rs`, `aquifer.rs`, `ore_vein.rs`.

### Phase 3 — SURFACE: surface rules

- Source: `NoiseChunk.java` + datapack `surface_rules` of the noise_settings.
- Condition tree (steep, hole, water, y-checks, badlands…) deciding
  grass/dirt/stone/terracotta per column, using `surfaceDepth` (a noise).
- **Bedrock**: `vertical_gradient` + positional RNG
  (`PositionalRandomFactory` with `Mth.getSeed(x,y,z)`) — verified 758/758
  bit-exact.
- Rust: `surface.rs`, `surface_rules.rs`.

### Phase 4 — CARVERS: caves and canyons

- Source: `CaveWorldCarver.java`, `CanyonWorldCarver.java`.
- Per chunk, N attempts are drawn (`setLargeFeatureWithSalt`); each cave is a
  "worm" of ellipsoids with width noise. Deterministic per chunk.
- Rust: `carvers.rs`.

### Phase 5 — STRUCTURES: `StructureStart` per chunk

- Source: `ChunkGenerator.createStructures` → each `Structure` decides if the chunk
  has a start (RNG by salt) and builds pieces (`MineShaftPieces`…).
- Pieces are written in `applyBiomeDecoration` (step 3 of the step loop).
- Rust: `mineshaft.rs` (121/121 bounding boxes bit-exact). Villages, strongholds,
  trial chambers: **not ported**.

### Phase 6 — FEATURES (decoration): the hard step

- Source: `ChunkGenerator.applyBiomeDecoration` (~line 263).
- For the chunk being decorated, with `origin` = NW corner of the chunk:
  1. `setDecorationSeed(seed, originX, originZ)` → one seed per chunk.
  2. Collect the biomes present in the 3×3 (`possibleBiomes`).
  3. For each step (0..10): the step's features present in those biomes, ordered by
     **global FeatureSorter index** (106 features in step 9).
  4. Per feature: `setFeatureSeed(decorSeed, globalIndex, step)` → the feature's own
     RNG stream. Then `PlacedFeature.placeWithContext`: a **lazy sequence** of
     modifiers (count → in_square → filters → height) producing positions, and per
     position the feature runs (e.g. `TreeFeature` consumes ~50 stream values).
- Features **read and write outside the chunk** (a tree crosses borders) through a
  `WorldGenRegion`.
- Rust: `feature_dispatch.rs` + `feature_catalog.rs` (FeatureSorter indices verified
  against the jar with `ProbeFeatureOrder`) + per-feature ports.

## THE FINDING (15 Aug 2026): vanilla 26.2 determinism map

**Experiment:** 7 runs of the real server (same jar, seed 12345, identical
procedure), comparing blocks of chunk (6,-2):

| Result | Runs | Diffs |
| --- | --- | --- |
| 100.00 % identical | 4/7 | 0 |
| 99.98 % | 2/7 | 15 |
| 99.05 % | 1/7 | 938 |

**Spatial distribution of diffs (run with 15):** 13/15 at distance ≤1 from the
chunk border, **0 in the core** (dist ≥5). The 938 of the extreme run are also
100% vegetation.

### The model (empirically verified)

1. **Everything driven by a per-chunk seeded stream is 100% deterministic**:
   feature positions (`setFeatureSeed` → in_square), structures and their chests
   (mineshaft/dungeon/…), ores, sculk, bedrock, full terrain. Vanilla always
   reproduces this — like a mineshaft chest is always in the same spot with the
   same seed.
2. **The only variation source** is PARALLEL decoration: a tree whose canopy
   crosses the border reads the neighbor chunk's state (heightmap / `would_survive`),
   which depends on whether the neighbor already decorated. The thread scheduler
   decides the order → most runs repeat the typical order (identical chunk), rarely
   changes (±15 border cells), very rarely collides hard (±938 vegetation cells).
3. **The chunk core (≥5 blocks from the border) was identical in all 7 runs.**

### Practical consequence for Neutron

- Mechanism parity (human decision): same seeds/streams/algorithms.
- Verify CORE chunks against any fresh reference: must be 100% (except real port
  gaps).
- Border diffs are vanilla noise: measure, but don't chase cell-by-cell against ONE
  run.
- Multi-chunk comparison (not a single chunk) separates signal (port gaps,
  deterministic and reproducible) from noise (borders).

## Status per phase (measured against fresh worlds, seed 12345, chunk (6,-2))

| Phase | Determinism | Neutron state | Real gap |
| --- | --- | --- | --- |
| Noise/shape | ✅ deterministic | ~99.6 % dens_shape | residual interpolators |
| Surface + bedrock | ✅ | bedrock 758/758 exact | dirt/grass swap collateral of trees |
| Carvers | ✅ | full port | initial widthFactors |
| Ores (step 6) | ✅ | andesite/diorite 1:1 | ~180 positional cells (iron/redstone/diamond) |
| Tuff/ore veins | ✅ | partial | ~69 border cells |
| Mineshaft | ✅ | 121/121 BB bit-exact | postProcess (rails/cobweb) |
| Sculk (step 7) | ✅ (3 noise cells in 2 runs) | volume ok | **~325 position cells** |
| Vegetation (step 9) | ❌ stochastic run to run | 2 trees vs ~6 | real count/position gap + irreducible noise |
| Other structures | ✅ | not ported | villages, strongholds, … |

## What "1:1" means — DECIDED (human gate, 15 Aug 2026)

> "I want it to behave like vanilla. If X is not deterministic in vanilla, use the
> same mechanism of making it random as vanilla."

1. **Mechanism parity**: same seeds → same RNG streams → same per-feature
   algorithms, bit by bit.
2. **Deterministic phases** (terrain, carvers, ores, structures, sculk, bedrock) →
   100% block match multi-seed against fresh references.
3. **Vegetation** → same per-chunk stream (verified with Java probes against the
   jar); our serial order produces a valid output of vanilla's output space.
4. **Multi-seed**: parity is measured on N seeds × their spawn chunks with
   `tools/nbt-ref/multiseed.py`.

## Quick verification

```bash
# generate a fresh reference (arbitrary seed)
cd tools/nbt-ref && mkdir vanilla-fresh-<seed> && cd vanilla-fresh-<seed>
cp ../vanilla1/server.jar . && echo eula=true > eula.txt
printf 'level-seed=<seed>\nonline-mode=false\nview-distance=5\n' > server.properties
(sleep 75; echo stop; sleep 20) | java -Xms1G -Xmx1G -jar server.jar nogui

# parity against that reference
cargo run --release -p neutron-worldgen --example block_parity -- <seed> <cx> <cz> <region-dir>

# compare two vanilla worlds against each other (stochastic noise)
cargo run --release -p neutron-worldgen --example compare_worlds -- <a.mca> <b.mca>
```

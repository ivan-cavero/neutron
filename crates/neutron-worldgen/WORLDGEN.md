# Worldgen — freeze point (F2d R41) + R43 correction

> Updated 15 Aug 2026. Read **`WORLDGEN-PIPELINE.md`** first (mandatory): vanilla
> determinism map (R43 finding) and the **mechanism parity** bar decision. The
> numbers in this table were measured against the `vanilla1` reference, now known
> to be **poisoned** for decoration.

**How it is measured now** (fresh references, multi-chunk, multi-seed):

```bash
# 9 chunks around (6,-2) with core/border breakdown
cargo run --release -p neutron-worldgen --example region_parity -- 12345 6 -2 1

# multi-seed (generates fresh references and measures)
python tools/nbt-ref/multiseed.py 12345 777 424242
```

R43 baseline (3×3 region, fresh references): 12345 → ALL 97.73 %; 424242 → ALL
89.07 % (aquifer + desert surface + lush caves + pale garden). Core chunks must be
100 % in deterministic phases — see PIPELINE.

## What's missing (R44 priority, real deterministic gap)

1. **Aquifer/water** — 424242: 7149 air→water cells (deterministic). ✅ fixed run-044
2. **Desert/beach surface rules** — 424242: ~1500 cells. ✅ fixed run-044
3. **Lush caves features** (moss/clay/cave_vines) + **pale garden** — in progress (run-045/046)
4. **Sculk positions** — 12345: ~325 cells. ✅ mechanism closed run-044
5. **Positional ores + tuff** — 12345: ~250 cells.
6. Vegetation: stream parity (mechanism), not absolute block-match.

## How it looks on the server

`neutron-server` generates real chunks (not superflat) with this pipeline.
Default seed: **12345** (the bar's). Spawn = heightmap at (0, 0) + 1.

```bash
cargo run --release -p neutron-server -- --seed 12345 --view-distance 8
# Vanilla 26.2 client → localhost:25565 (online-mode=false)
# Creative + fly. Terrain is the current worldgen, not 1:1 yet.
```

Go to chunk `(6, -2)` (approx x=96, z=-32) to see mineshaft + deep dark.

## Modules

| File | Role |
| --- | --- |
| `generator.rs` | Orchestrates the chunk (3×3 region + features). `Send` (`Arc` density). |
| `biome/` | Multi-noise + voronoi. Params in `data/biome_params.bin` (7594 points). |
| `density.rs` / `worldgen.rs` | Noise router + markers (`DF = Arc<DFNode>`) |
| `surface.rs` / `surface_rules.rs` | Internal `BlockId` + surface JSON + `vanilla_name` |
| `carvers.rs` | Caves / canyon |
| `mineshaft.rs` | Structure pieces 26.2 |
| `features.rs` | OreFeature + rarity |
| `sculk.rs` | ChargeCursor + vein |
| `feature_dispatch.rs` / `tree.rs` | Step 9 (JSON dispatch + TreeFeature CFR) |

`BlockId` is **internal** (Air=0, Stone=1, Dirt=10, …). The server translates it
to 26.2 protocol block-state IDs in `neutron-server`. (`vegetation.rs` approx was
removed in R43 — JSON dispatch is the only path.)

## R41 history (against `vanilla1` reference — poisoned for decoration)

Pipeline `ChunkGenerator::generate_chunk`: noise+aquifer+veins+surface →
carvers → mineshafts → step 6 ores → step 7 sculk → step 9 vegetation.

| Metric (chunk 6,-2) | R41 value |
| --- | --- |
| ALL | 97.84 % · BASE 99.34 % · dens_shape ~99.6 % |
| Bedrock 758/758 · Andesite 1424 = vanilla · Mineshaft 121/121 BB bit-exact |
| Sculk volume 917 vs 518 · ChargeCursor 1:1 flat |

R41→R44 pending: mineshaft postProcess (rails/cobweb), sculk positions,
TreeFeature stream, positional ores, other structures, carver widthFactors.

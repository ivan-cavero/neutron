# Worldgen — punto de congelación (F2d R41) + corrección R43

> Actualizado 15 ago 2026. Ver **`WORLDGEN-PIPELINE.md`** (obligatorio): mapa
> de determinismo de vanilla (hallazgo R43) y la decisión de bar por
> **paridad de mecanismo**. Los números de esta tabla se midieron contra la
> referencia `vanilla1`, hoy sabida **viciada** para decoración.

**Cómo se mide ahora** (referencias frescas, multi-chunk, multi-seed):

```bash
# 9 chunks alrededor de (6,-2) con desglose core/border
cargo run --release -p neutron-worldgen --example region_parity -- 12345 6 -2 1

# multi-seed (genera referencias frescas y mide)
python tools/nbt-ref/multiseed.py 12345 777 424242
```

Baseline R43 (region 3×3, referencias frescas): 12345 → ALL 97.73 %;
424242 → ALL 89.07 % (aquifer + surface desierto + lush caves + pale garden).
Chunks core deben ser 100 % en fases deterministas — ver PIPELINE.

## Qué falta (prioridad R44, gap real determinista)

1. **Aquifer/agua** — 424242: 7149 celdas air→water (determinista).
2. **Surface rules desierto/playa** — 424242: ~1500 celdas.
3. **Lush caves features** (moss/clay/cave_vines) + **pale garden**.
4. **Sculk posiciones** — 12345: ~325 celdas.
5. **Ores posicionales + tuff** — 12345: ~250 celdas.
6. Vegetación: paridad de stream (mecanismo), no block-match absoluto.

## Cómo se ve en el servidor

`neutron-server` genera chunks reales (no superflat) con esta pipeline.
Seed por defecto: **12345** (el del bar). Spawn = heightmap en (0, 0) + 1.

```bash
cargo run --release -p neutron-server -- --seed 12345 --view-distance 8
# Cliente vanilla 26.2 → localhost:25565 (online-mode=false)
# Creative + vuelo. El terreno es el worldgen actual, no 1:1.
```

Ir a chunk `(6, -2)` (aprox. x=96, z=-32) para ver mineshaft + deep dark.

## Módulos

| Archivo | Rol |
|---|---|
| `generator.rs` | Orquesta el chunk (3×3 region + features). `Send` (`Arc` density). |
| `biome/` | Multi-noise + voronoi. Params en `data/biome_params.bin` (7498 puntos). |
| `density.rs` / `worldgen.rs` | Noise router + markers (`DF = Arc<DFNode>`) |
| `surface.rs` / `surface_rules.rs` | `BlockId` interno + surface JSON + `vanilla_name` |
| `carvers.rs` | Cuevas / cañón |
| `mineshaft.rs` | Structure pieces 26.2 |
| `features.rs` | OreFeature + rarity |
| `sculk.rs` | ChargeCursor + vein |
| `feature_dispatch.rs` / `tree.rs` | Step 9 (dispatch JSON + TreeFeature CFR) |

`BlockId` es **interno** (Air=0, Stone=1, Dirt=10, …). El servidor lo traduce a
block-state IDs de protocolo 26.2 en `neutron-server`. (`vegetation.rs`
aproximado fue eliminado en R43 — el dispatch JSON es el único camino.)

## Histórico R41 (contra referencia `vanilla1` — viciada para decoración)

Pipeline `ChunkGenerator::generate_chunk`: noise+aquifer+veins+surface →
carvers → mineshafts → step 6 ores → step 7 sculk → step 9 vegetación.

| Métrica (chunk 6,-2) | Valor R41 |
|---|---|
| ALL | 97.84 % · BASE 99.34 % · dens_shape ~99.6 % |
| Bedrock 758/758 · Andesite 1424 = vanilla · Mineshaft 121/121 BB bit-exact |
| Sculk volume 917 vs 518 · ChargeCursor 1:1 plano |

Pendiente R41→R44: mineshaft postProcess (raíles/cobweb), sculk posiciones,
TreeFeature stream, ores posicionales, otras estructuras, carver widthFactors.

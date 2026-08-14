# Worldgen — punto de congelación (F2d R41)

> 14 ago 2026. La generación **no es 1:1**. El bar sigue siendo 100 % block
> match en seed `12345`, chunk `(6, -2)`. Este archivo congela el estado para
> poder **jugar el mundo** en el servidor mientras el parity queda pendiente.

## Qué hay hoy

Pipeline de `ChunkGenerator::generate_chunk` (overworld 26.2):

1. Noise + aquifer + ore veins + surface rules (datapack JSON)
2. Carvers (cuevas + cañón)
3. Estructuras: **mineshafts** (start, árbol de piezas, `generateBox`, `placeSupport`)
4. Step 6 — ores (`OreFeature`)
5. Step 7 — sculk (`ChargeCursor` + `MultifaceSpreader`)
6. Step 9 — vegetación (árboles, hierba, leaf litter)

| Métrica (seed 12345, chunk 6,-2) | Valor | Notas |
|---|---|---|
| ALL (nombre de bloque) | **97.84 %** (96177 / 98304) | run-041 |
| BASE (sin veg) | **99.34 %** | residual = ores + shape |
| dens_shape | ~99.55–99.67 % | noodle / interpolators |
| Bedrock | 758 / 758 | positional RNG |
| Andesite | 1424 = vanilla | RarityFilter `nextFloat` |
| Mineshaft start `(4,-1)` | **121 / 121 BB bit-exact** | Crossing N/S/E + EAST `maxX-3` |
| Primer parche sculk `(98,-43,-23)` | roll **0.467** (catalyst sí) | cueva aún incompleta |
| Sculk volume | 917 vs van 518 | de más: mineshaft air residual |
| ChargeCursor (suelo plano) | 1:1 ticks 1–2 | no basta en cueva real |

## Qué falta (prioridad para volver a 1:1)

No tocar el bar. El siguiente gap que mueve celdas, en este orden:

1. **Mineshaft `postProcess`** — raíles, cobweb, `generateMaybeBox` con el RNG del structure start. 10 `cave_air` residuales en r≤15 del parche mueven el roll a 0.269.
2. **`Room.generateUpperHalfSphere`** + `isInInvalidLocation`.
3. **Sculk de más** — baja cuando el air de la mineshaft vecina `(5,-2)` / `(5,-1)` coincide.
4. **TreeFeature** — extra `dark_oak_leaves` (~240 air→leaves).
5. **BASE residual** — ores posicionales, emerald/magma sin `BlockId`.
6. **Otras estructuras** — no hay villages, strongholds, trial chambers, etc.
7. **Carver `widthFactors`** — port inicial; no es el residual dominante.

Criterio para retomar: un critic ciego ejecuta `block_parity` contra un mundo vanilla fresco (no golden viejo) y compara contra la tabla de arriba.

## Cómo se verifica

```bash
# Parity del chunk bar (necesita un mundo vanilla seed 12345 pregenerado)
cargo run --release -p neutron-worldgen --example block_parity

# Un solo chunk, seed 12345
cargo test -p neutron-worldgen --release
```

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
| `surface.rs` / `surface_rules.rs` | `BlockId` interno + surface JSON |
| `carvers.rs` | Cuevas / cañón |
| `mineshaft.rs` | Structure pieces 26.2 |
| `features.rs` | OreFeature + rarity |
| `sculk.rs` | ChargeCursor + vein |
| `feature_dispatch.rs` / `tree.rs` / `vegetation.rs` | Step 9 |
| `examples/` | Sondas de parity (`autoexamples = false`; solo 6 se compilán por defecto) |

`BlockId` es **interno** (Air=0, Stone=1, Dirt=10, …). El servidor lo traduce a
block-state IDs de protocolo 26.2 en `neutron-server`.

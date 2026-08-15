# Cómo funciona la generación de mundo de Minecraft 26.2 (y dónde estamos)

> 15 ago 2026 · Escrito para Neutron a partir de las fuentes decompiladas del jar
> real (`tools/vanilla-extract/decompiled/`). Cada fase cita la clase Java que
> la implementa y el módulo Rust que la porta.

## El pipeline completo (qué pasa cuando pides un chunk)

Cada chunk pasa por una **cadena de estados** (`ChunkStatus`). Cada estado
requiere que los vecinos 3×3 hayan alcanzado el estado anterior. El orden:

```
EMPTY → STRUCTURE_STARTS → STRUCTURE_REFERENCES → BIOMES → NOISE
      → SURFACE → CARVERS → FEATURES (liquid/hot/cool/etc.) → INITIALIZE_LIGHT → LIGHT → SPAWN
```

Cuando el servidor necesita un chunk (jugador lo ve), sube la cadena por
dependencias: pedir el chunk (6,-2) en estado LIGHT implica generar los 8
vecinos hasta INITIALIZE_LIGHT, y los anillos exteriores hasta estados
previos. **Eso es lo que replica nuestro `RegionBuf` 3×3.**

### Fase 1 — BIOMES: `ChunkGenerator.createBiomes`

- Fuente: `ChunkGenerator.java` → `BiomeResolver` desde `Climate.Sampler`.
- El clima es 6 ruidos (temperature, vegetation, continentalness, erosion,
  depth, weirdness) → tabla de 7498 puntos (parameter list del overworld).
- Elección de bioma: el punto con mayor `fitness` (producto de deltas).
- Bordes suaves: **voronoi a escala 1:4** (4×4×4 bloques por celda de bioma)
  con `obfuscateSeed` (SHA-256 del seed).
- Rust: `biome/` (multi-noise + voronoi). Params en `data/biome_params.bin`.

### Fase 2 — NOISE: la forma del terreno

- Fuente: `NoiseChunk.java`, `NoiseRouter.java`, `RandomState.java`.
- Un **NoiseRouter** = árbol de density functions (noise + operaciones).
  Se deriva del seed con `RandomState.create(...)` — cada noise tiene su
  propio sub-seed derivado en orden fijo (`NoiseData.bootstrap`): esto hace
  el terreno **100 % determinista por chunk**.
- El router produce una densidad por celda 4×8×4 (cell) y luego se
  **interpola** a bloques (`NoiseInterpolator`) con lerp sobre cells.
- Sobre la densidad final: `finalDensity < 0` = sólido. El aquifer
  (`NoiseBasedAquifer`) decide agua/lava en huecos; `OreVeinifier`
  decide vetas de copper/iron + tuff/granite.
- Rust: `density.rs`, `noise.rs`, `aquifer.rs`, `ore_vein.rs`.

### Fase 3 — SURFACE: reglas de superficie

- Fuente: `NoiseChunk.java` + datapack `surface_rules` del noise_settings.
- Árbol de condiciones (steep, hole, water, y-checks, bandlands…) que decide
  grass/dirt/stone/terracota por columna, usando `surfaceDepth` (un noise).
- **Bedrock**: `vertical_gradient` + RNG posicional
  (`PositionalRandomFactory` con `Mth.getSeed(x,y,z)`) — verificado 758/758
  bit-exact.
- Rust: `surface.rs`, `surface_rules.rs`.

### Fase 4 — CARVERS: cuevas y cañones

- Fuente: `CaveWorldCarver.java`, `CanyonWorldCarver.java`.
- Por chunk se sortean N intentos (`setLargeFeatureWithSalt`); cada cueva es
  un "gusano" de elipsoides con ruido de anchura. Determinista por chunk.
- Rust: `carvers.rs`.

### Fase 5 — ESTRUCTURAS: `StructureStart` por chunk

- Fuente: `ChunkGenerator.createStructures` → cada `Structure` decide si el
  chunk tiene start (RNG por salt) y construye piezas (`MineShaftPieces`…).
- Las piezas se escriben en `applyBiomeDecoration` (paso 3 del step loop).
- Rust: `mineshaft.rs` (121/121 bounding boxes bit-exact). Villages,
  strongholds, trial chambers: **no portadas**.

### Fase 6 — FEATURES (decoración): el paso complicado

- Fuente: `ChunkGenerator.applyBiomeDecoration` (línea ~263).
- Para el chunk que se decora, con `origin` = esquina NW del chunk:
  1. `setDecorationSeed(seed, originX, originZ)` → un seed por chunk.
  2. Se reunen los biomas presentes en el 3×3 (`possibleBiomes`).
  3. Por cada step (0..10): los features del step presentes en esos biomas,
     ordenados por **índice global FeatureSorter** (106 features en step 9).
  4. Por feature: `setFeatureSeed(decorSeed, índiceGlobal, step)` → stream
     de RNG propio del feature. Luego `PlacedFeature.placeWithContext`:
     una **secuencia lazy** de modificadores (count → in_square → filtros →
     altura) que produce posiciones, y por cada posición el feature corre
     (p.ej. `TreeFeature` consume ~50 valores del stream).
- Los features **leen y escriben fuera del chunk** (un árbol cruza bordes)
  a través de un `WorldGenRegion`.
- Rust: `feature_dispatch.rs` + `feature_catalog.rs` (índices FeatureSorter
  verificados contra el jar con `ProbeFeatureOrder`) + ports por feature.

## EL HALLAZGO (15 ago 2026): mapa de determinismo de vanilla 26.2

**Experimento:** 7 corridas del servidor real (mismo jar, seed 12345,
procedimiento idéntico), comparando bloques del chunk (6,-2):

| Resultado | Corridas | Diffs |
|---|---|---|
| 100.00 % idéntico | 4/7 | 0 |
| 99.98 % | 2/7 | 15 |
| 99.05 % | 1/7 | 938 |

**Distribución espacial de los diffs (corrida con 15):** 13/15 a distancia
≤1 del borde del chunk, **0 en el core** (dist ≥5). Los 938 de la corrida
extrema también son 100 % vegetación.

### El modelo (verificado empíricamente)

1. **Todo lo que decide un stream con seed por chunk es 100 % determinista**:
   posiciones de features (`setFeatureSeed` → in_square), estructuras y sus
   cofres (mineshaft/dungeon/…), ores, sculk, bedrock, terreno completo.
   Vanilla reproduce esto SIEMPRE — igual que un cofre de mineshaft siempre
   está en el mismo sitio con la misma seed.
2. **La única fuente de variación** es la decoración PARALELA: un árbol cuyo
   canopy cruza el borde lee el estado del chunk vecino (heightmap /
   `would_survive`), que depende de si el vecino ya decoró. El scheduler de
   threads decide el orden → en la mayoría de corridas el orden típico se
   repite (chunk idéntico), rara vez cambia (±15 celdas en el borde),
   muy rara vez colisiona fuerte (±938 celdas de vegetación).
3. **El core del chunk (≥5 bloques del borde) salió idéntico en las 7
   corridas.**

### Consecuencia práctica para Neutron

- Paridad de mecanismo (decisión del humano): mismos seeds/streams/algoritmos.
- Verificar chunks NÚCLEO contra cualquier referencia fresca: deben ser
  100 % (salvo gaps reales del port).
- Los diffs de borde son ruido vanilla: medir, pero no perseguir celda a
  celda contra UNA corrida.
- La comparación multi-chunk (no un solo chunk) separa señal (gaps del port,
  deterministas y reproducibles) de ruido (bordes).

## Estado por fase (medido contra mundos frescos, seed 12345, chunk (6,-2))

| Fase | Determinismo | Estado Neutron | Gap real |
|---|---|---|---|
| Noise/shape | ✅ determinista | ~99.6 % dens_shape | interpoladores residuales |
| Surface + bedrock | ✅ | bedrock 758/758 exact | dirt/grass swap colateral de árboles |
| Carvers | ✅ | port completo | widthFactors inicial |
| Ores (step 6) | ✅ | andesite/diorite 1:1 | ~180 celdas posicionales (iron/redstone/diamond) |
| Tuff/ore veins | ✅ | parcial | ~69 celdas frontera |
| Mineshaft | ✅ | 121/121 BB bit-exact | postProcess (raíles/cobweb) |
| Sculk (step 7) | ✅ (3 celdas de ruido en 2 corridas) | volumen ok | **~325 celdas de posición** |
| Vegetación (step 9) | ❌ estocástico corrida a corrida | 2 árboles vs ~6 | gap real de count/posición + ruido irreducible |
| Otras estructuras | ✅ | no portadas | villages, strongholds, … |

## Qué significa "1:1" — DECIDIDO (gate humano, 15 ago 2026)

> "Quiero que se comporte como vanilla. Si en vanilla X cosa no es
> determinista, usar el mismo mecanismo de hacerlo random que en vanilla."

1. **Paridad de mecanismo**: mismos seeds → mismos streams de RNG → mismos
   algoritmos por feature, bit a bit.
2. **Fases deterministas** (terreno, carvers, ores, estructuras, sculk,
   bedrock) → 100 % block match multi-seed contra referencias frescas.
3. **Vegetación** → mismo stream por chunk (verificado con probes Java
   contra el jar); nuestro orden serial produce una salida válida del
   espacio de salidas de vanilla.
4. **Multi-seed**: la paridad se mide en N seeds × sus chunks de spawn con
   `tools/nbt-ref/multiseed.py`.

## Verificación rápida

```bash
# generar referencia fresca (seed arbitraria)
cd tools/nbt-ref && mkdir vanilla-fresh-<seed> && cd vanilla-fresh-<seed>
cp ../vanilla1/server.jar . && echo eula=true > eula.txt
printf 'level-seed=<seed>\nonline-mode=false\nview-distance=5\n' > server.properties
(sleep 75; echo stop; sleep 20) | java -Xms1G -Xmx1G -jar server.jar nogui

# parity contra esa referencia
cargo run --release -p neutron-worldgen --example block_parity -- <seed> <cx> <cz> <region-dir>

# comparar dos mundos vanilla entre sí (ruido estocástico)
cargo run --release -p neutron-worldgen --example compare_worlds -- <a.mca> <b.mca>
```

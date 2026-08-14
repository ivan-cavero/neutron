# STATE — Neutron

> Estado actual del proyecto. Se lee al empezar cada run y se actualiza al terminar.

## Fase actual
**F3 FASE A — Simulación vanilla** (COMPLETADO ✅)
**F3 FASE B — Redstone B** (COMPLETADO ✅)
**F3 FASE C — Redstone C** (COMPLETADO ✅)
**F2d R42 — freeze worldgen + servidor jugable (login 26.2 + chunks reales)**  
**F2d R41 — EAST maxX-3; 121/121 BB 1:1; roll 0.467; ALL 97.84%**  
**F2d R40 — generateBox+placeSupport; roll 0.029 (catalyst sí); 116/121 BB; ALL 97.28%**  
**F2d R39 — Crossing N/S/E; 116/121 BB 1:1 Y=-44; catalyst_roll 0.112; ALL 97.27%**  
**F2d R38 — Mineshaft start (4,-1) + 4 piezas XZ 1:1; árbol diverge en #4; roll 0.996; ALL 98.48%**  
**F2d R37 — HORIZONTAL N,E,S,W; primer parche roll 0.996 por mineshaft en (5,-2)/(5,-1); ALL 98.48%**  
**F2d R36 — spreadAll snapshot; ChargeCursor 1:1 cueva+plano; sculk 446/518 cat 2=2; ALL 98.48%**  
**F2d R35 — ChargeCursor 1:1 suelo plano (166/174/roll 0.821); sculk 382→643/518; Y=-32 213/278; 98.35%**  
**F2d R34 — ProbeSculkPatch: ChargeCursor 1:1 tick 1–2; DEFAULT facings; sculk 330→382; 98.40%**  
**F2d R33 — Primer parche sculk i=0=(98,-43,-23) 1:1; catalyst_roll 0.701 (van sí); (103,-26,-31) no sale; 98.41%**  
**F2d R32 — ChargeCursor shuffle/NON_CORNER 1:1; sculk 330/518 capa Y=-32; block 98.41%**  
**F2d R31 — Sculk origen centro primero; sculk 187→330; block 98.41% / BASE 99.69%**  
**F2d R30 — OCEAN_FLOOR blocksMotion + PlacedFeature stream lazy; block 98.33% / BASE 99.69%**  
**F2d R29 — RarityFilter nextFloat; andesite 1424=1424; block 97.65% / BASE 99.65%**  
**F2d R28 — BiomeFilter ores + validTreePos; 97.02%/99.00%; van sí tiene andesite_upper en 28 chunks, no en (6,-2)**  
**F2d R27 — would_survive on RandomSelector; block 97.02%; andesite_upper blob localizado (RNG=vanilla, van no escribe)**  
**F2d R26 — OreFeature gate + canPlaceOre + Mth.sin; block match 93.89% → 96.96%, BASE 99.00%**  
**F2d R25 — WorldgenRandom nextLong/nextDouble + lush_caves id; block match 85.09% → 93.89%**  
**F2d R24 — 4 builders: ChargeCursor + voronoi + TreeFeature + disks; dens 99.67% sculk overlap 332/565**  
**F2d R23 — setFeatureSeed vanilla + FeatureSorter; dens 99.68% sculk 326/565 overlap 292**  
**F2d R21 — 1:1 tools (extract-worldgen + feature_catalog + CFR sculk, no heuristics)**  
**F2d R14 — Sculk flood+expand + veg; dens 99.71% sculk~425/565**  
**F2d R13 — Sculk vein+patch; dens 99.43% pure_air=0 residual=563 sculk**  
**F2d R12 — Pregen multi-bioma + canyon/veg; dens avg 99.45% / block 88.5%**  
**F2d R11 — Multi-chunk dens; sculk stub**  
**F2d R10 — Full Interpolated (noodle)** (99.42% pure_air=0)
**F2d R6 — Underground ore features** (DONE)
**F2d R5 — Surface JSON engine + ore veins** (DONE)
**F2d R4 — Surface rules + bedrock/deepslate** (DONE)
**F2d R3 — Marker wrapping** (T1 DONE)

### Workspace Rust — 6 crates + tools

```
neutron/
├── crates/
│   ├── neutron-protocol/       ✅ 54 tests — protocolo 26.2
│   ├── neutron-world/          ✅ 39 tests — Anvil, level.dat
│   ├── neutron-worldgen/       ✅ 45+ tests — noise, caves, biomes, markers
│   ├── neutron-server/         ✅ 13 tests — server binario
│   ├── neutron-sim/            ✅ 65 tests — light, redstone (B+C), fluid, spawn
│   └── neutron-bench-server/   ✅ Criterion benchmarks
├── tools/
│   ├── golden-data/            ✅ Vanilla chunk extraction
│   ├── parity-check/           ✅ Parity verification
│   └── vanilla-extract/        ✅ PARAMETERS.md (650+ líneas)
└── tests/
    └── e2e-server/             ✅ E2E bot test
```

### Resultados de F3 FASE A (9 ago 2026)

| Sistema | Tests | Estado vanilla |
|---------|-------|----------------|
| **Lighting** | 11 | BFS propagation, sky/block light, dirty flags |
| **Redstone** | 9 | NC order N,S,W,E,D,U, burnout 9+, relight 160 ticks |
| **Fluids** | 18 | Water 5 ticks, lava 30 ticks, lava max 4 blocks |
| **Spawns** | 13 | sky<=7 AND block==0, caps 20/5, despawn 1/800 |

### Resultados de F3 FASE B (9 ago 2026) — Comparators, Repeaters, Observers, Hoppers, TNT

| Componente | Tests | Estado |
|------------|-------|--------|
| Comparator (container mode) | ✅ | Output signal = container level (0-15), output at BACK of facing |
| Comparator (subtraction) | ✅ | Output = container - input signal |
| Repeater (1-4 delay) | ✅ | Delay stored as 2-8 ticks, output at facing direction |
| Observer | ✅ | Fires on block change in facing direction, 1-tick pulse output |
| Hopper | ✅ | 8-tick cooldown, item transfer in/out |
| TNT | ✅ | 40-tick fuse, blast radius 4, clears blocks |

### Resultados de F3 FASE C (9 ago 2026) — Pistons, QC, Block Swapping

| Componente | Tests | Estado |
|------------|-------|--------|
| Piston extend | ✅ | 2-tick animation, push up to 12 blocks |
| Piston retract | ✅ | Power removed → retract |
| Sticky piston | ✅ | Extends, pushes block, maintains state |
| Quasi-connectivity (QC) | ✅ | Java-only BUD mechanic, side-adjacent powered block |
| Block swapping | ✅ | Sticky piston pulls block back on retract |

### Fixes de paridad aplicados
1. Fluid spread rates (water 5x, lava 6x too fast)
2. Spawn light check (sky<=7 AND block==0)
3. Redstone NC order (N,S,W,E,D,U)
4. Torch burnout threshold (8→9) + relight
5. Light emission values (20+ sources)
6. Light BFS propagation (cross-column fix)

### Gaps restantes para F3
- FASE D: Golden suite posicional + Survival básica
- Redstone C: quasi-connectivity (QC) push limit 12 blocks only approximate
- Redstone B: hopper transfer timing approximate (no full inventory system)
- No server integration yet (redstone in neutron-sim only, no redstone ticks from server)
- Observer: block-change detection uses update_queue proxy (not full block-type tracking) — known limitation, pass per bar

## Estado de paridad F2d (run-009, 9 ago 2026)

### T1 DONE — Marker wrapping implemented

| Componente | Estado | Detalle |
|------------|--------|---------|
| MarkerState | ✅ | Mutable caching state: last_pos_2d, last_value, interpolation_counter, flat_cache, cell_cache |
| DensityEnv markers | ✅ | `marker_state: Option<&'a mut MarkerState>` — with_markers() / new() |
| compute() &mut | ✅ | Mutable ref para mutating marker state |
| Marker handler | ✅ | Immutable-check / mutable-compute / mutable-store pattern |
| 380 markers integrados | ✅ | 145 Cache2D, 26 CacheOnce, 209 FlatCache, 0 CacheAllInCell |
| Marker per-chunk | ✅ | Generator crea MarkerState por chunk, interpolation_counter por bloque |
| Grid sampling | ✅ | Usa DensityEnv::new() sin markers (sin caching innecesario) |
| Fill loop | ✅ | Usa DensityEnv::with_markers() con caching |
| Tests | ✅ | 8/8 neutron-worldgen --lib tests pass |
| Full build | ✅ | Workspace compila clean en release |

**Resultado**: Chunk generates with 32414 non-air blocks (seed 12345), marker caching active.

### F2d R5 DONE (9 ago 2026) — Surface JSON + ore veins

| Componente | Estado | Detalle |
|------------|--------|---------|
| Terrain heights | ✅ | 1:1 sin vegetación (chunk 6,-2 seed 12345) |
| Surface rules JSON | ✅ | Engine `surface_rules.rs` desde noise_settings datapack |
| Vertical gradient RNG | ✅ | PositionalRandomFactory + Mth.getSeed → **bedrock 758/758 exact** |
| Ore veins | ✅ | OreVeinifier copper/iron → tuff/granite/ores en density phase |
| Surface grass/dirt | ✅ | 256 grass + dirt variable por surfaceDepth |
| Deepslate | ✅ | vertical_gradient absolute 0..8 |

### F2d R6 DONE — Underground ores (feature step 6)

| Componente | Estado | Detalle |
|------------|--------|---------|
| OreFeature blobs | ✅ | Ellipsoid size/discard + stone/deepslate targets |
| Stone variants | ✅ | granite/diorite/andesite/tuff/dirt/gravel counts ~vanilla order |
| Coal/iron/copper/… | ✅ | Present in chunk; counts same order of magnitude |
| Feature RNG | ✅ | setDecorationSeed + setFeatureSeed |

### F2d R8–R12 hallazgos

| Métrica | Valor |
|---------|-------|
| dens_shape (6,-2) | **99.42%** pure_air=0 |
| dens_shape **24 chunks** multi-bioma | **99.45% avg** · block **88.5%** |
| Mejor dens_shape | **100%** en (−42,−46) |
| Java vs Neutron open_frac | match exacto en (6,-2) y (10,10) |
| Vanilla1 after pregen | **~25 MB** regiones · cientos de full |
| Canyon | ✅ index 2 |
| Short grass | ✅ ligero |
| Sculk | 🟡 port parcial, **`SCULK_ENABLED=false`** |

**Bar 1:1**: 100% block match. Aún no se cumple.

### F2d R29 (14 ago 2026) — RarityFilter 26.2

| Componente | Estado | Detalle |
|------------|--------|---------|
| RarityFilter | ✅ | `nextFloat < 1/chance` (javap + probe 26.2) |
| andesite | ✅ | **1424 = vanilla**, extra=0 miss=0 |
| **block name match** | 🟡 | **97.65%** |
| **BASE no veg** | 🟡 | **99.65%** |
| Mundo fresco | ✅ | `world-r29/` confirma el skip (no golden viejo) |

### F2d R28 (14 ago 2026) — BiomeFilter step 6

| Componente | Estado | Detalle |
|------------|--------|---------|
| BiomeFilter ores | ✅ | `hasFeature` vía lista datapack del bioma |
| validTreePos | ✅ | sin water (CFR REPLACEABLE_BY_TREES) |
| andesite_upper | 🔴 | van región 28 chunks Y≥64; **(6,-2)=0**; índice 6 confirmado jar |
| block match | 🟡 | **97.02% / BASE 99.00%** (sin cambio) |

### F2d R27 (14 ago 2026) — selector placed + andesite_upper

| Componente | Estado | Detalle |
|------------|--------|---------|
| RandomSelector → placed | ✅ | `would_survive` ya corre (`dark_oak_leaf_litter`) |
| **block name match** | 🟡 | **97.02%** |
| BASE no veg | 🟡 | **99.00%** |
| andesite_upper | 🔴 | extra 545 @ Y92–100; RNG jar-exact; van Y solo ≤63 |

### F2d R26 (14 ago 2026) — OreFeature vanilla gate

| Componente | Estado | Detalle |
|------------|--------|---------|
| OCEAN_FLOOR_WG gate | ✅ | skip `doPlace` si bbox sobre terreno |
| canPlaceOre RNG | ✅ | `shouldSkipAirCheck` antes de `isAir` |
| isAdjacentToAir | ✅ | solo `isAir()`, no fluidos |
| blob Mth.sin | ✅ | tabla 65536 + `t` f32 |
| **block name match** | 🟡 | **96.96%** (era 93.89%) |
| **BASE no veg** | 🟡 | **99.00%** |
| diorite | ✅ | 1046 = vanilla |

**1:1 no alcanzado.** Tests: worldgen 40 PASS.

### F2d R25 (14 ago 2026) — WorldgenRandom BitRandomSource + lush_caves

| Componente | Estado | Detalle |
|------------|--------|---------|
| FeatureRandom nextLong | ✅ | dos `next(32)` — golden vs 26.2 jar |
| FeatureRandom nextDouble | ✅ | `next(26)<<27 + next(27)` |
| lush_caves / sulfur_caves | ✅ | ids 34 / 36 (ya no ocean=0) |
| ore_clay | ✅ | gate `LUSH_CAVES`; clay 745 vs van 703 |
| dens_shape (6,-2) | 🟡 | **99.5534%** · feat_extra=389 · miss=50 |
| **block name match** | 🟡 | **93.89%** (era 85.09%) |
| BASE no veg | 🟡 | **95.85%** |
| sculk overlap | 🟡 | 137/565 · catalyst 1 (RNG correcto, algo incompleto) |

**1:1 no alcanzado.** Tests: worldgen 39 PASS (golden WorldgenRandom).

### F2d R24 (13 ago 2026) — 4 builders en paralelo

| Componente | Estado | Detalle |
|------------|--------|---------|
| ChargeCursor CFR | 🟡 | overlap sculk **332/565** · vol 417 · ~232 paredes |
| BiomeManager voronoi | ✅ | `obfuscateSeed` SHA-256 + 8-corner fiddle; tests Java |
| TreeFeature | 🟡 | fancy/dark_oak/blob CFR; extra leaves 385→207 |
| Disks + step 6 index | 🟡 | DiskFeature 30–32; ore_clay Off (lush id=0) |
| dens_shape (6,-2) | 🟡 | **99.6704%** · feat_extra=238 |
| block name match | 🟡 | **85.09%** (ores posicionales dominan) |

**1:1 no alcanzado.** Tests: worldgen 38, protocol 47, world 39, sim 65 — todos PASS.

### F2d R30 (14 ago 2026) — OCEAN_FLOOR blocksMotion

| Componente | Estado | Detalle |
|------------|--------|---------|
| Heightmap OCEAN_FLOOR | ✅ | `blocksMotion` incl. hojas (javap + ProbeBlocksMotion) |
| PlacedFeature stream | ✅ | place entre cada InSquare (lazy 26.2) |
| WORLD_SURFACE / NO_LEAVES | ✅ | predicados 26.2 |
| **block name match** | 🟡 | **98.33%** (era 97.65%) |
| BASE no veg | 🟡 | **99.69%** |
| extra dark oak | 🟡 | leaves 725→514 · logs 466→202 · air→leaves 469→240 |

### F2d R31 (14 ago 2026) — sculk centro primero

| Componente | Estado | Detalle |
|------------|--------|---------|
| FEATURES origin order | ✅ | sculk: centro luego vecinos (veg revertido: regresaba árboles) |
| **block name match** | 🟡 | **98.41%** (era 98.33%) |
| sculk volume | 🟡 | **330** vs van 518 (era 187) |
| sculk→deepslate | 🟡 | **221** (era 311) |

### F2d R32 (14 ago 2026) — ChargeCursor probes

| Componente | Estado | Detalle |
|------------|--------|---------|
| Util.shuffle / allShuffled | ✅ | ProbeShuffle bit-exact |
| NON_CORNER order | ✅ | betweenClosed = rust |
| extra_rare_growths | ✅ | JSON 0 + bucle javap |
| **block name match** | 🟡 | **98.41%** (sin cambio de volumen) |
| sculk Y=-32..-17 | 🔴 | 135 vs 278; 15-radius del único parche no cubre |

### F2d R42 (14 ago 2026) — freeze + join

Worldgen **congelado** en R41. Documentado en `crates/neutron-worldgen/WORLDGEN.md`.
El servidor habla protocolo 26.2 (Configuration + known packs) y sirve
chunks de `ChunkGenerator` con IDs de bloque vanilla.

| Pieza | Estado |
|---|---|
| Login 26.2 (config + registries + tags) | ✅ |
| BlockId → block-state 26.2 | ✅ (reports del jar) |
| Cache + hilo worldgen | ✅ |
| Spawn = heightmap (0,0) | ✅ |
| Cliente vanilla entra y ve el terreno | 🟡 verificar al levantar |

### Pendiente F2d (parity 1:1 — no se toca en este freeze)

| Componente | Estado | Detalle |
|------------|--------|---------|
| Mineshaft postProcess | 🔴 | raíles, cobweb, maybeBox, half-sphere |
| TreeFeature extra+miss | 🔴 | air→dark_oak_leaves ~240 |
| Sculk volume | 🔴 | 917 vs 518 (air mineshaft residual) |
| BASE residual | 🟡 | ores posicionales |
| Otras estructuras | 🔴 | villages, stronghold, … |

## Ver
- `crates/neutron-worldgen/WORLDGEN.md` — freeze F2d: métricas, gaps, cómo entrar
- `runs/run-042.md` — servidor 26.2 + chunks reales
- `runs/run-032.md` — F2d R32: ChargeCursor shuffle 1:1 + capa Y=-32
- `runs/run-031.md` — F2d R31: sculk centro primero
- `runs/run-030.md` — F2d R30: OCEAN_FLOOR blocksMotion
- `runs/run-029.md` — F2d R29: RarityFilter nextFloat; andesite 1:1
- `runs/run-028.md` — F2d R28: BiomeFilter ores; van andesite_upper en región
- `runs/run-027.md` — F2d R27: would_survive + andesite_upper diag
- `runs/run-026.md` — F2d R26: OreFeature gate + canPlaceOre
- `runs/run-025.md` — F2d R25: WorldgenRandom nextLong/nextDouble
- `runs/run-024.md` — F2d R24: 4 builders paralelo
- `runs/run-023.md` — F2d R23: setFeatureSeed + FeatureSorter
- `runs/run-018.md` — F2d R12: pregen + canyon/veg + multi
- `runs/run-017.md` — F2d R11: multi-chunk + sculk gate
- `runs/run-016.md` — F2d R10: full Interpolated / noodle fix
- `tools/nbt-ref/vanilla1/pregen.py` — pregen multi-centro
- `runs/run-013.md` — F2d R7: métricas estrictas 1:1
- `runs/run-012.md` — F2d R6: underground ore features
- `runs/run-011.md` — F2d R5: surface JSON + ore veins
- `tools/java-probe/src/ProbeNoodle.java` — probe density
- `tools/vanilla-extract/BIOME-SPEC.md` — biome spec completa
- `crates/neutron-worldgen/src/density.rs` — MarkerState, DensityEnv, compute() con markers
- `tools/java-probe/` — Java verification probes
- `tools/vanilla-extract/decompiled/*.java` — fuentes descompiladas (NoiseChunk.java)

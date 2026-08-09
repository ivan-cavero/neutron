# STATE — Neutron

> Estado actual del proyecto. Se lee al empezar cada run y se actualiza al terminar.

## Fase actual
**F3 FASE A — Simulación vanilla** (COMPLETADO ✅)
**F3 FASE B — Redstone B** (COMPLETADO ✅)
**F3 FASE C — Redstone C** (COMPLETADO ✅)
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

### Pendiente F2d R13+ (camino a 100%)

| Componente | Estado | Detalle |
|------------|--------|---------|
| SculkSpreader bit-exact | 🔴 | cierra feat_extra deep_dark |
| Trees / full vegetation | 🔴 | |
| OreFeature bit-exact | 🔴 | |
| Canyon widthFactors exact | 🟡 | port inicial OK |
| Disks / surface extras | ⏸️ | |

## Ver
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

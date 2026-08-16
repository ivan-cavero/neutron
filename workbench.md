# F2d run-046 — Modelo de input cross-chunk · gauntlet workbench

> Loop activo desde 16 ago 2026. **Bar (decisión humana R43, run-045)**:
> **paridad de mecanismo** — mismos seeds/streams/algoritmos que vanilla;
> fases deterministas → 100 % block match multi-seed; vegetación/sculk →
> mismo stream 1:1. Ver `runs/run-045.md` (baseline) y `runs/run-046.md`.

**Budget:** sin cap de rondas; stop solo si (a) bar gana, (b) 2 rondas sin
mejora, (c) humano frena.

## Baseline (cierre run-045)

| seed | REGION 3×3 ALL | gap dominante |
| --- | --- | --- |
| 12345 (6,-2) | 97.75 % | input-model (borde) + vegetación |
| 424242 (0,0) | **97.28 %** | clay inflado 840 vs 497 + lush/pale missing |
| 777 (0,0) | ~99.4 % | residuales |

## Bar del run-046 (AC, del run-045, intocable)

- [ ] clay 840 → **~497** (missing lush/pale ≤ 20 %, recall ≥ 80 %) en 424242
- [ ] border diffs de `region_parity` bajan **≥ 30 %** en 12345; cores sin regresión
- [ ] REGION 424242 ≥ **97.28 %** y 12345 ≥ **97.75 %** (sin regresión)
- [ ] `cargo test --workspace` verde; tests 59/59 worldgen
- [ ] Bar intocable: no editar examples/tests de medición

## Units

| # | Unit | Archivos | Bar (per unit) |
|---|------|----------|----------------|
| U1 | Build limpio (clay_probe WIP roto) | `crates/neutron-worldgen/Cargo.toml`, `examples/clay_probe.rs` | `cargo test --workspace` exit 0, 59/59 tests, examples registrados compilan |
| U5 | Modelo de input cross-chunk (run-046) | `generator.rs`, `region_buf.rs`, `feature_dispatch.rs`, scheduler de decoración | Bar del run-046 completo (arriba) |

## Round log

| Round | U1 | U5 | Notas |
|-------|----|----|-------|
| R0 (16 ago) | 🔴 no compila | — | `clay_probe.rs:116` `sections` no existe; WIP run-045 |

### R0 details

- `cargo test --workspace` → exit 1: `error[E0425]: cannot find value 'sections'`
  en `crates/neutron-worldgen/examples/clay_probe.rs:116` (example registrado
  en Cargo.toml con `autoexamples = false`).
- 8 warnings en neutron-server (campos muertos: compression_threshold,
  game_mode, current_tick, get_player_info...).

| R1 (16 ago) | ✅ **PASS** (critic ciego) | — | cargo test exit 0, 59/59 worldgen, 25 examples de medición compilan, src/ no tocado. WIP run-045 commiteado (0689ff8) tras gate humano |
| R1 (16 ago) | — | 🔍 investigación completa | Modelo vanilla verificado (ChunkGenerator.java L263-341, WorldGenRegion.java, ChunkPyramid L20): origin-major, centro primero, vecinos en CARVERS, masking. Brechas C1-C8 + plan D. **Builder U5 pendiente** |
| R2 (16 ago) | — | 🔨 builder U5 reporta | clay 840→**466** (≈497 ✓); REGION 12345 97.94 % ✓; cores mejoran; pero REGION 424242 97.20 % (-0.08 ❌), border -7.5 % (necesita ≥30 % ❌), lush/pale recall 48 % (pre-existente ❌). Gap diagnosticado: pale oaks 0/167 solape (steps 1-5/8 no portados alteran el estado en step 9). **Critic pendiente** |
| R2 (16 ago) | — | ❌ **FAIL** (critic ciego) | Reprodujo todo desde cero: tests 59/59 ✓, REGION 12345 97.94 % ✓, clay 466 ✓, higiene ✓. FALLA: REGION 424242 97.20 % (regresión), border -7.5 %, recall lush/pale 48.27 % (missing 51.73 %). **Gap más grande: posición de decoración vegetal** (pale oaks 3936 missing, clay 2126, moss 1300) → arrastra REGION 424242. Fix: portar FancyTreeFeature completa (trunk 2×2, canopy, ramas) + placement chain de lush_caves_clay. **→ Builder R3** |
| R3 (16 ago) | — | 🔨 builder R3 (working tree) | FancyTreeFeature pale oak (trunk 2×2, canopy, ramas) + TreeDecorator sort + creaking_heart/pale_moss + lush_caves_clay placement chain. **Medido (LEAD, 16 ago):** REGION 424242 **97.38 %** ✓ (bar ≥97.28), REGION 12345 **97.94 %** ✓ (bar ≥97.75), clay **466** ✓ (~497), recall lush/pale **62.94 %** (bar ≥80 ❌, era 48.27), border -7.5 % (bar ≥30 ❌), **777 96.29 %** (baseline ~99.4 ❌ REGRESIÓN). tests 59/59 ✓. **Bar NO cumplido**: recall <80 %, border sin mejora, 777 regresa. **Critic pendiente** |

## Estado

**R3 WIP (working tree): recall lush/pale 48→62.94 %, 424242 97.38 %, 12345 97.94 %, clay 466 ✓ — bar NO cumplido (recall <80 %, border -7.5 %, 777 regresión 96.29 %). Commit checkpoint hecho; critic ciego pendiente.**

*Última actualización: 16 ago 2026 — R3 (medido por LEAD)*

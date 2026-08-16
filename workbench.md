# F2d run-046 — Modelo de input cross-chunk · gauntlet workbench

> Loop activo desde 16 ago 2026. **Bar (decisión humana R43, run-045)**:
> **paridad de mecanismo** — mismos seeds/streams/algoritmos que vanilla;
> fases deterministas → 100 % block match multi-seed; vegetación/sculk →
> mismo stream 1:1. Ver `runs/run-045.md` (baseline) y `runs/run-046.md`.

**Budget:** sin cap de rondas; stop solo si (a) bar gana, (b) 2 rondas sin
mejora, (c) humano frena.

## Baseline (cierre run-045)

| seed | REGION 3×3 ALL | gap dominante |
|---|---|---|
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

| R1 (16 ago) | 🔨 builder reporta fix (WIP) | — | clay_probe: `}` prematuro cerró scope de `sections`; fix de scoping, sin tocar mediciones. Evidencia `runs/run-046-evidence-build.txt`. **Critic pendiente** |
| R1 (16 ago) | — | 🔍 investigación completa | Modelo vanilla verificado (ChunkGenerator.java L263-341, WorldGenRegion.java, ChunkPyramid L20): origin-major, centro primero, vecinos en CARVERS, masking. Brechas C1-C8 + plan D. **Builder U5 pendiente** |

## Estado

**U1: esperando critic ciego. U5: prompt listo (plan de investigación D).**

*Última actualización: 16 ago 2026 — R1*
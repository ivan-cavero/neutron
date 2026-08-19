# Plan de acción — post-mortem run-049 (19 Aug)

## Qué pasó (honesto)
- 0 commits de paridad en main. 5 commits de infra/tooling (Cargo fix, workbench,
  state, README, tooling T3).
- 3 fan-outs de builders fallaron por timeout (30 min default del subagent que no
  se propagó a pesar de pedir 50-90 min).
- Los builders divagaron en investigación en vez de producir — tareas demasiado
  abiertas para el presupuesto.

## Hallazgos reales de hoy (de los builders, verificados)
1. **T3 tooling completo y commiteado** (rama b4-t3-order, 4 commits):
   - water filter fix (OCEAN_FLOOR = blocks_motion) — el fix documentado del root cause
   - deco_stream_probe: strip modes + full step-9 stream + trunk compare
   - strip-center-trees (quita solo logs/leaves, mantiene moss spillover)
   - Con after6+center-trees strip: 5/16 draws rejected = ratio ~1/4 (coincide con finding)
2. **T4 hallazgo clave**: los 5 ports candidatos (monster_room, ice_spike, freeze_top,
   desert_well, ice_patch) NO afectan la medición 424242 — no hay cobblestone/snow/ice/
   sandstone en la región medida. Los ports NO son la palanca del recall.
3. **La palanca real del recall es el ORDEN** (T3): trees = 39% del gap de 8,572 celdas.

## Plan (próximos pasos)
### P1 — Derivar el orden REAL de generación de vanilla (hoy, LEAD)
- El orden de chunks alrededor del spawn viene del scheduler (ChunkMap/ServerChunkCache):
  cómo se promueven los chunks a FEATURES. Decompilar con javap:
  - ServerChunkCache.getChunkFuture / ChunkMap.scheduleGenerationTask
  - DistanceManager / ChunkTaskDispatcher: orden de procesamiento
  - El orden de decoración del 3×3 = el orden en que cada chunk se vuelve centro
- Si el scheduler es demasiado complejo: correr el servidor vanilla 26.2 con el seed
  y observar el orden real (log de generación o timestamps en region files).
- Output: el orden total de los 9 chunks (permutación) para seed 424242.

### P2 — Implementar orden + water filter en main (después de P1)
- decoration_origin_order: modo 'full_order' con la permutación derivada.
- Water filter fix (ya listo en b4-t3-order).
- Medir: region_parity ×3 (ratchet) + lush_pale_parity recall + clay.
- Target: recall 57.96% → +5pp hacia 80%.

### P3 — T4 ports (después de P2, si el recall no llega a 80%)
- Los ports son correctos pero no mueven 424242. Hacerlos para completar la whitelist
  (19→~5) con medición por-port, sin forzar regresiones.

### P4 — Ratchet + crítico ciego
- Re-medir todos los seeds, cargo test --workspace, crítico ciego sobre el merge.

## Regla aprendida
- Los builders necesitan tareas CERRADAS (output concreto, pasos verificables),
  no investigación abierta. Para investigación: LEAD la hace o la divide en
  micro-pasos con checkpoint por paso.
- El timeout del subagent default (30 min) NO se sobrescribe con maxRuntimeMs en
  todos los paths — verificar antes de relanzar.

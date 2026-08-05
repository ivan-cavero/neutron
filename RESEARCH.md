# Neutron — Base de investigación verificada

> Documento de evidencia. Todo hecho fue verificado en fuentes primarias el **5 ago 2026**. Se actualiza en cada fase (pipeline D0-D4, benchmarks nuevos). Formato: hecho — fuente — confianza.

## 1. Minecraft: cadencia y versiones

1. Mojang cambió a numeración por año el 2 dic 2025. **No existe "1.22"**. — minecraft.net (new-version-numbering-system) — ALTA
2. 26.1 "Tiny Takeover" (24 mar 2026): primer jar **sin ofuscar**, requiere Java 25. — minecraft.net — ALTA
3. 26.2 "Chaos Cubed" (16 jun 2026): versión actual; 26.3 en snapshots (Q3 2026). — minecraft.wiki (version history) — ALTA
4. Cadencia: ~3 drops/año + hotfixes (1.21.5→1.21.11 en 2025; 26.1, 26.2, 26.3 en 2026). — minecraft.wiki — ALTA

## 2. Ecosistema Rust de servidores

5. **Pumpkin** (referente): ~10.6k★, GPL-3.0, solo nightly (sin 1.0); worldgen casi completo (biomas/terreno/carvers), estructuras parciales, redstone temprana (pistones rotos), iluminación OK, plugins WASM (wasmtime + WIT), PatchBukkit en desarrollo. — github.com/Pumpkin-MC/Pumpkin, issues #449 #36 #1402 — ALTA
6. Pumpkin 1.0 prometida "en 2026", retrasada desde 2025. — r/rust (feb 2026) — ALTA
7. Valence activo (framework, Bevy ECS, sin server completo); Feather inactivo desde 2024; FerrumC activo (rechaza compat Bukkit explícitamente); Oxide activo. — repos oficiales — ALTA

## 3. Referencias de rendimiento

8. **C2ME** (1.21.10, metodología rigurosa: seed fija, tmpfs, warmup): vanilla 10.6-14.2 cps, Paper 17.4-84.8, C2ME 22.6-182.4 según hilos; **vanilla no escala > ~14 cps**. — gist.github.com/ishland — ALTA
9. **Pumpkin self-reported** (su doc avisa que la comparación es injusta): startup ~8 ms vs 7-8 s Paper; RAM 0.4-27 MB vs 1-2 GB; CPU 1.5% vs 20-26%. **Sin chunks/s publicados**. — docs.pumpkinmc.org/about/benchmarks — MEDIA
10. Test comunitario (jul 2026): confirmó startup/RAM de Pumpkin; encontró entidades rotas. — YouTube kxTZb0FYiTU — MEDIA
11. Pumpkin PR #2506: `populate_noise_stage` 43.1 → 18.8 ms/chunk (−56%) con **paridad bit-for-bit verificada por checksums xxHash64**. — github.com/Pumpkin-MC/Pumpkin/pull/2506 — ALTA

## 4. Paridad vanilla

12. Worldgen determinista y data-driven (datapack `worldgen/`); cubiomes reproduce biomas/estructuras por seed. — minecraft.wiki (World_seed, World_generation), cubiomes — ALTA
13. Redstone: orden de updates **PP: W,E,N,S,D,U / NC: W,E,D,U,N,S**; quasi-connectivity solo Java; 1.21.2 cambió el wire (left-first); comportamiento posicional. — minecraft.wiki (Block_update, Redstone_mechanics, 1.21.2) — ALTA
14. Iluminación: Starlight demostró **salida idéntica con engine distinto y más rápido**; 1.20 adoptó sus ideas. — PaperMC/Starlight (TECHNICAL_DETAILS.md) — ALTA
15. Spawns: ciclos (hostil/tick, pasivo/400 ticks), caps (monster 70, creature 10...), pack spawning ±5, reglas de luz/distancia (24/32/128). — minecraft.wiki (Mob_spawning) — ALTA
16. Mob AI **hardcoded** en Java (no data-driven como Bedrock). — minecraft.wiki (Behavior_pack) — ALTA

## 5. Stack Rust

17. **wasmtime**: runtime de referencia (component model, WASI preview 2); Pumpkin probó Extism y migró a wasmtime. — wasmtime.dev, pumpkin issue #662 — ALTA
18. **mlua 0.12** (jul 2026): Lua 5.1-5.5/LuaJIT/Luau. — crates.io/mlua — ALTA
19. **bevy_ecs**: solo el crate ECS (sin el engine); lo usan Valence, FerrumC, Azalea. — repos — ALTA
20. **redb**: KV embeddable activo (ACID); sled en modo mantenimiento. — github.com/cberner/redb — ALTA

## 6. Plugins Java → WASM (límites verificados)

21. TeaVM/JWebAssembly: sin reflection ni class-loading por defecto; CheerpJ = JVM-en-WASM orientada a browser; FerrumC descarta compat Bukkit. **Conclusión: compat Bukkit solo por capas** (API nativa → convertidor para plugins puros → capa PatchBukkit-style). — teavm.org, CheerpJ blog, ferrumc README — ALTA

## 7. Orca ADE + Gauntlet Loop

22. **Orca**: Stably AI, MIT, onorca.dev; worktrees por tarea + orquestación CLI (run/task/dispatch/worker_done/decision gates); soporta OpenCode, Claude Code, Codex. — onorca.dev/docs, github.com/stablyai/orca — ALTA
23. **Gauntlet Loop**: Matt Shumer, "How to Run a Gauntlet Loop" (somethingbig.ai/gauntlet-loop, jul 2026), repo mshumer/Claude-of-Duty. Núcleo: split → build → blind critic → repeat contra un bar real; sin cap arbitrario de rondas. — somethingbig.ai, Decrypt, ThePromptIndex, We0 — ALTA
24. Lección del propio Shumer: el bar puede ser **inalcanzable** (su critic nunca ganó a CoD real: 3.59 → 5+/10). El bar tira del trabajo hacia arriba; no se negocia. — somethingbig.ai — ALTA

## 8. Herramientas de medición

25. **spark** (TPS/salud, incluido en Paper 1.21+; Timings deprecado) · **Chunky** (chunks/s) · **mineflayer** (bots ≤ 1.21.11) · **azalea** (bots Rust, trackea 26.x) · marcador de arranque = línea `Done (Xs)!` · RSS por OS (no heap JVM). — spark.lucko.me, Chunky wiki, repos mineflayer/azalea — ALTA

## Mantenimiento

Actualizar este documento: (a) en cada release de Mojang (pipeline D0-D4); (b) tras cada benchmark publicado; (c) cuando cambie el estado de Pumpkin (nuestra referencia). Toda afirmación nueva entra solo con fuente y fecha.
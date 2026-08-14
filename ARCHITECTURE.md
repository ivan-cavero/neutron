# Neutron — Arquitectura

> v0.1 · 5 ago 2026 · Documento vivo: cada decisión marcada con `[ADR]` se mueve a `docs/adr/` cuando se implementa.
> Todas las decisiones técnicas se basan en investigación verificada (Anexo A al final).

## 1. Principios de diseño

1. **Cero panics en runtime** — política panic-free en código de producción (Pumpkin está eliminando ~70 `unwrap/expect` como blocker de su 1.0; lo hacemos desde el día 1: `#![forbid(unsafe_code)]` en la mayoría de crates, error handling tipado).
2. **Determinismo como contrato** — misma seed + misma versión = mismo mundo y mismo comportamiento. Verificado por checksums en CI, no por opinión.
3. **Sin GC, sin sorpresas** — memoria explícita, arenas y pooling; perfiles de memoria medidos en CI.
4. **Medir antes de optimizar** — ningún "optimización" entra sin benchmark (criterion) y sin comparación contra la referencia verificada.
5. **Paridad por tests, no por fe** — cada subsistema tiene un "test de oro" contra vanilla (hash de chunks, secuencia de updates de redstone, resultado de iluminación).
6. **Seguridad por construcción** — plugins en sandbox WASM con fuel/memory limits; el servidor nunca cae por un plugin.
7. **`main` = última versión de Minecraft** — la cadencia de versiones es una feature de primera clase (pipeline D0-D4, ver §10).
8. **Estructura de carpetas del mundo 100% vanilla** — `world/`, `world_nether/`, `world_the_end/`, `level.dat`, `region/*.mca`, `session.lock`: un mundo de Neutron abre en vanilla y viceversa.

## 2. Vista general

### Hoy (agosto 2026) — grafo real

```
  neutron-server (binario)
       │  login 26.2 + 1 worker worldgen + LRU de chunks
       ├── neutron-protocol     paquetes 26.2 escritos a mano
       └── neutron-worldgen     overworld; DF = Arc (Send); no 1:1

  neutron-world     Anvil listo, no usado por el server
  neutron-sim       luz / redstone / fluidos / spawn — solo tests
```

### Objetivo (sin implementar todavía)

```
                        ┌──────────────────────┐
                        │    neutron-cli      │  binario, config, comandos
                        └──────────┬───────────┘
                                   │
        ┌──────────────────────────▼──────────────────────────┐
        │                   neutron-server                   │
        │  ┌─────────────┐  ┌──────────────┐  ┌────────────┐  │
        │  │ networking  │  │  scheduler   │  │ observab.  │  │
        │  │ (tokio)     │  │ (por región) │  │ tracing+   │  │
        │  │ sesiones    │  │ single-writer│  │ metrics    │  │
        │  └──────┬──────┘  └──────┬───────┘  └────────────┘  │
        └─────────┼────────────────┼──────────────────────────┘
                  │                │
     ┌────────────▼────────────────▼─────────────┐   ┌───────────────────────┐
     │          neutron-sim (bevy_ecs)          │   │   neutron-plugin     │
     │  entidades · redstone · iluminación       │   │  wasmtime (component  │
     │  fluidos · spawns · mob AI · física       │   │  model, WIT) + mlua   │
     └────────────┬────────────────┬─────────────┘   └───────────────────────┘
                  │                │
     ┌────────────▼──────┐  ┌──────▼─────────────────┐
     │  neutron-world   │  │   neutron-worldgen    │  ← neutron-data
     │  chunks · Anvil   │  │   paridad 1:1          │    (datos generados)
     │  .hyp · region IO │  │   checksums xxHash64   │
     └────────────┬──────┘  └────────────────────────┘
                  │
     ┌────────────▼────────────┐
     │    neutron-protocol    │  ← generado por tools/mc-extract + codegen
     │   paquetes multi-versión│
     └─────────────────────────┘
```

## 3. Protocolo y networking

- **Java Edition: versión actual de Mojang (26.2 hoy)** como objetivo primario; mapa de IDs de paquetes **multi-versión generado** (como el `MultiVersionJavaPacket` de Pumpkin) para aceptar clientes N-1..N-3.
- **Bedrock**: fase F7 (protocolo 26.x + RakNet). Prioridad baja al inicio; no condiciona la arquitectura (capa de sesión independiente).
- Networking con **tokio + bytes**: cero-copia donde el hot path lo permita, outbox por jugador, batch de paquetes de chunks (rate-limit por jugador como el `player-max-chunk-send-rate` de Paper).
- Seguridad de protocolo: validación estricta de todo input, límites de tamaño, rate limits por conexión, y **fuzzing** (`cargo-fuzz` + `arbitrary`) del decode desde F1.
- `[ADR-P1]` codec generado (como Basalt/Aero: generan paquetes desde JSON de minecraft-data) vs codec a mano → **generado**, con override manual cuando Mojang rompe patrones.

## 4. Datos y registries (capa `neutron-data`)

- Todo dato (bloques, items, biomas, registries, worldgen, protocolo) se **extrae del jar de Mojang** (desde 26.1 el jar no está ofuscado: extracción directa; para 1.21.x usar el mapa de ofuscación como los demás) y se **genera código Rust tipado** en build time.
- El worldgen de vanilla es data-driven (datapack `worldgen/` embebido en el jar desde 1.19.3): **embebemos el datapack vanilla** como recurso y lo cargamos en runtime — así los cambios de Mojang en noise/biomas/features llegan como datos, no como reimplementación.
- Soporte de **datapacks de usuario** (los mundos vanilla los usan): parser de codecs y registries dinámicos desde F2.

## 5. Mundo y storage (capa `neutron-world`)

- Chunk interno: **palette compacta** (1-15 bits/bloque), heightmaps, biomes, block entities, light data — estructura de memoria propia (no NBT) para el hot path.
- **Persistencia 1:1 con vanilla**:
  - Lectura/escritura de **Anvil `.mca`** (NBT) — mundos intercambiables con vanilla/Paper.
  - `level.dat` (NBT) con los campos correctos para cada versión.
  - `world/`, `world_nether/`, `world_the_end/`, `session.lock`, estructura de carpetas idéntica.
  - Formato propio **`.hyp`** (zstd, ~50-95% más pequeño, como el `.linear` de Pumpkin) opcional vía config, con conversor idéntico.
- I/O asíncrono fuera del tick loop; cola de escritura con flush periódico; guardado incremental (region dirty flags).
- `[ADR-W1]` redb como KV para estado global (players.dat, scores, metadata) — activo y ACID; sled descartado (modo mantenimiento). Para regiones: archivos `.mca`/`.hyp` + índice en memoria.

## 6. Worldgen (capa `neutron-worldgen`) — paridad 1:1

Pipeline por chunk (verificado contra el flujo real de vanilla):

```
noise (density functions) → biome source → surface rules → carvers
→ placed features → structures → spawn de mobs en generación → iluminación
```

- **Determinismo**: mismo seed + misma versión = mismo mundo (verificado: minecraft.wiki/World_seed; determinismo demostrado por cubiomes y por el PR #2506 de Pumpkin con checksums).
- RNG: reimplementación exacta del PRNG de Mojang (**XORoshiro128** — Pumpkin ya lo hizo y verificó paridad de seed).
- Funciones de densidad: port 1:1 de los algoritmos (Perlin/octaves 3D) + carga del JSON del datapack vanilla.
- **Paralelismo**: pool dedicado (rayon o tokio con DAG de dependencias — como el DAG de Pumpkin para dependencias cross-chunk) generando chunks en paralelo; los chunks vecinos (3×3) son dependencia para features/estructuras.
- **Verificación de paridad en CI**: checksum **xxHash64** del bloque final (y del mapa de biomas) por chunk, comparado contra golden data de vanilla (generado una vez por versión con un server vanilla headless). 50+ seeds golden por versión. Esto es el "test de oro" — la misma técnica que verificó Pumpkin en su PR #2506.
- Estructuras: port por fases (fuertes → aldeas → ancient city → resto) con los "generation attempts" por región de 32×32 (mecánica verificada por cubiomes).

## 7. Simulación (capa `neutron-sim`) — comportamiento vanilla

- **ECS con `bevy_ecs`** — IMPORTANTE: usamos SOLO el crate `bevy_ecs` (la librería de Entity Component System: almacén de entidades + scheduler de systems), **NO el engine Bevy completo** (sin renderer, sin ventanas, sin assets — un servidor no pinta nada). Verificado: Valence, FerrumC y Azalea lo usan en producción; es el estándar de facto.
- **Qué NO es ECS**: los chunks (arrays densos con palettes), la redstone y los fluidos son simulación por tiles con estructuras de datos propias (§5, redstone en §7). El ECS cubre SOLO la capa de entidades: componentes (posición, salud, AI state, inventario) + systems (movimiento, AI, combate) ejecutados en paralelo por el scheduler por regiones. La capa está aislada en `neutron-sim`: si los benchmarks de F4 lo desaconsejan, se puede sustituir por `hecs` o almacén custom sin tocar el resto.
- **Tick a 20 TPS** con scheduler por regiones (estilo Folia): cada región (p.ej. 8×8 chunks) tiene su propio loop de tick con **single-writer**; sync global solo donde hace falta (redstone cross-región, física de jugadores). Esto es lo que permite escalar a 1000+ jugadores sin un hilo único.
- **Redstone** (el mayor reto, verificado): subsistema dedicado con
  - orden de updates exacto de vanilla: **PP: W, E, N, S, D, U · NC: W, E, D, U, N, S** (minecraft.wiki/Block_update);
  - **quasi-connectivity** (solo Java — "works as intended");
  - comportamiento de wire post-1.21.2 (Redstone Experiments: left-first, cómputo de potencia antes de updates);
  - **tests posicionales**: las mismas contraptions en posiciones distintas del mundo (la vanilla es position-dependent — Paper issue #7725);
  - suite de "contraptions doradas" comparada contra un server vanilla real con bots (no solo unit tests).
- **Iluminación**: engine propio estilo Starlight (propagar niveles, skylight dedicado, gestión stateless de light sections para generación paralela) con el contrato de Starlight: "cualquier diferencia con vanilla es un bug". 1.20 demostró que vanilla puede adoptar estas ideas → paridad alcanzable.
- **Fluidos**: flujo determinista con las reglas de update de vanilla (mismo orden de ticks de fluido).
- **Spawns**: ciclo hostil cada tick / pasivo cada 400, caps (monster 70, creature 10, ambient 15…), pack spawning triangular ±5, reglas de luz y distancia (24/32/128 bloques) — todo según minecraft.wiki/Mob_spawning. Spawns de chunk-gen incluidos (seed-derived).
- **Mob AI**: port desde el jar sin ofuscar (26.1+); behavior por mob con prioridad: pasivos → hostiles → jefes; pathfinding A* con optimizaciones (Pumpkin ya demostró mejoras grandes en su A*).
- **Physics/combate**: knockback, cooldown, i-frames, proyectiles (verificado en Pumpkin #1404 como referencia de alcance).

## 8. Scheduler y paralelismo

- **Regla de oro**: un escritor por región; lectura concurrente permitida con `RwLock` por chunk o versioning.
- Chunk pipeline: generación en paralelo (DAG), carga/descarga asíncrona, envío al cliente con rate-limit.
- Hot paths sin locks: arenas de memoria, reuso de buffers (el PR #2506 de Pumpkin: −56% tiempo de noise con optimización de buffers; nosotros lo hacemos desde el diseño).
- Networking fuera del tick (tokio); los paquetes se encolan en el outbox del jugador.
- Determinismo vs paralelismo: el orden de ejecución *dentro* de una región es fijo (lista ordenada de entidades/updates); el paralelismo nunca reordena updates visibles (contrato de parity).

## 9. Plugins: WASM + Lua (capa `neutron-plugin`)

- **Runtime: wasmtime** (verificado: runtime de referencia de la Bytecode Alliance, component model + WASI preview 2 estable; Pumpkin probó Extism y migró a wasmtime — nosotros podemos usar Extism como capa ergonómica *sobre* wasmtime si aporta, decisión en F6).
- Formato: **componentes WASM (`wasm32-wasip2`)** con interface **WIT** (`neutron-plugin-api.wit`): ABI estable entre versiones del servidor → un plugin compilado una vez corre en todas las versiones futuras (esto es lo que resuelve el problema de "mi plugin se rompe en cada release").
- **Seguridad**: sandbox de memoria por store, **fuel limits** (opcodes por tick), memory limits, capacidades explícitas por plugin (permisos: chat, world access, network…), hot reload, y aislamiento: panic/crash del plugin ≠ crash del servidor.
- **Lua (mlua 0.12)** para scripting ligero de confianza (eventos simples, comandos, GUI) — ergonomía alta, sin recompilar.
- **API más potente que Bukkit, más simple**: eventos con tipado fuerte, sistema de entidades vía bevy_ecs (queries, systems), comandos declarativos, y un modelo de permisos built-in (Bukkit lo resolvió con terceros como LuckPerms).
- **Conversor de plugins existentes — honesto** (verificado: TeaVM/JWebAssembly prohíben reflection/class-loading; CheerpJ es una JVM-en-WASM para browser; FerrumC descarta compat Bukkit):
  1. **Fase A**: analizador estático que detecta plugins "convertibles" (sin reflection, sin internals de CraftBukkit) → recompila el *bytecode puro* a WASM sobre nuestra API reimplementada (subconjunto del API Bukkit en Rust/WIT). Funciona para una fracción pequeña pero real de plugins simples.
  2. **Fase B**: capa de compat PatchBukkit-style (como la que Pumpkin está construyendo) que ejecuta jars Bukkit reales en un runtime JVM embebido, traducido a eventos de Neutron — lento, pero compatible.
  3. Mensaje claro a la comunidad: los plugins nuevos se escriben nativos (Rust/WASM o Lua). La "herramienta de conversión" es un puente, no el futuro.

## 10. Pipeline de versiones (la feature "días, no semanas")

Flujo D0-D4, automatizado y con SLA ≤ 7 días tras release de Mojang:

| Día | Paso | Herramienta | Verificación |
|---|---|---|---|
| D0 | Mojang publica release (ej. 26.3) | webhook/CI detecta | — |
| D1 | Descargar jar + extraer: registries, protocolo, worldgen datapack, assets | `tools/mc-extract` (jar sin ofuscar desde 26.1; para 1.21.x usar mapping) | diff de registries vs versión anterior; validación contra minecraft-data |
| D2 | Codegen: paquetes, block states, biomes, density functions JSON → Rust | `tools/codegen` | `cargo check` limpio, sin diffs manuales |
| D3 | Regenerar golden data con server vanilla (chunks por seed, contraptions) | harness | checksums xxHash64 |
| D4 | Correr suite de parity completa + benchmarks; release de `main` | CI | parity 100%, benchmarks publicados |

- `main` = siempre la última; las versiones anteriores se mantienen como **protocolo multi-versión** (N-1..N-3) pero simulación solo latest (como vanilla: no hay soporte de simulación para viejas).
- Cadencia de Mojang verificada: ~3 drops/año desde 2025 + hotfixes (1.21.5→1.21.11 en 2025; 26.1, 26.2, 26.3 en 2026) — el pipeline se ejecuta ~4-6 veces/año + hotfixes puntuales.
- Referencia del ecosistema: el pipeline de **minecraft-data** (PrismarineJS) ya hace esto en JS (proto.yml por versión + auto-updater + extractor Fabric); Valence también tiene extractor. Nosotros lo hacemos nativo en Rust con salida tipada.

## 11. Observabilidad

- `tracing` + métricas (prometheus-style) exportadas por endpoint interno.
- Endpoint de profiling propio (equivalente a spark: TPS, tick durations min/max/avg/p99, CPU, memoria, disk).
- CLI de bench integrado (`neutron bench ...`) para medir sin bots externos cuando sea posible.
- Todo benchmark publicado en `bench/results/` (ver BENCHMARKS.md).

## 12. Seguridad

- Protocolo: validación estricta, límites, fuzzing continuo.
- Plugins: sandbox WASM (fuel, memoria, capacidades) — "Security by construction" no es una capa, es el diseño.
- Panic-free en release; `cargo deny` para supply chain; vendoring de dependencias críticas.
- Anti-cheat server-side básico (movimiento, velocidad, noclip) como servicio interno (plugins pueden ampliarlo).

## 13. Stack de dependencias (verificado ago 2026)

| Crate | Uso | Estado verificado |
|---|---|---|
| tokio | networking, async I/O | estándar |
| bevy_ecs | simulación de entidades (solo el crate ECS, sin el engine) | usado por Valence/FerrumC/Azalea |
| wasmtime | runtime de plugins WASM | referencia de la Bytecode Alliance; elegido por Pumpkin |
| mlua 0.12 | scripting Lua (5.1-5.5/LuaJIT/Luau) | activo (jul 2026) |
| redb | KV embeddable (estado global) | activo; sled descartado |
| rayon | paralelismo de chunks (o pool propio con DAG) | estándar |
| xxhash-rust | checksums de paridad | estándar |
| serde + simdnbt | NBT (Anvil, level.dat) | — |
| criterion | micro-benchmarks | estándar |
| clap, tracing, anyhow/thiserror, bytes, flate2/zstd | infra | estándar |

## 14. Decisiones abiertas (ADR pendientes)

- Licencia: MIT o Apache-2.0 (recomendado) vs GPL-3.0 (como Pumpkin). Decisión del dueño.
- Extism como capa sobre wasmtime: sí/no (F6).
- Formato `.hyp` vs solo Anvil: .hyp como opt-in (F3).
- Region size y granularidad del scheduler (F4, con benchmark de decisión).


## Anexo A — Evidencia verificada (5 ago 2026)

> Base de hechos que anclan las decisiones de este documento y del roadmap. Formato: hecho — fuente — confianza. Se actualiza en cada fase (pipeline D0-D4, benchmarks nuevos).

### 1. Minecraft: cadencia y versiones

1. Mojang cambió a numeración por año el 2 dic 2025. **No existe "1.22"**. — minecraft.net (new-version-numbering-system) — ALTA
2. 26.1 "Tiny Takeover" (24 mar 2026): primer jar **sin ofuscar**, requiere Java 25. — minecraft.net — ALTA
3. 26.2 "Chaos Cubed" (16 jun 2026): versión actual; 26.3 en snapshots (Q3 2026). — minecraft.wiki (version history) — ALTA
4. Cadencia: ~3 drops/año + hotfixes (1.21.5→1.21.11 en 2025; 26.1, 26.2, 26.3 en 2026). — minecraft.wiki — ALTA

### 2. Ecosistema Rust de servidores

5. **Pumpkin** (referente): ~10.6k★, GPL-3.0, solo nightly (sin 1.0); worldgen casi completo (biomas/terreno/carvers), estructuras parciales, redstone temprana (pistones rotos), iluminación OK, plugins WASM (wasmtime + WIT), PatchBukkit en desarrollo. — github.com/Pumpkin-MC/Pumpkin, issues #449 #36 #1402 — ALTA
6. Pumpkin 1.0 prometida "en 2026", retrasada desde 2025. — r/rust (feb 2026) — ALTA
7. Valence activo (framework, Bevy ECS, sin server completo); Feather inactivo desde 2024; FerrumC activo (rechaza compat Bukkit explícitamente); Oxide activo. — repos oficiales — ALTA

### 3. Referencias de rendimiento

8. **C2ME** (1.21.10, metodología rigurosa: seed fija, tmpfs, warmup): vanilla 10.6-14.2 cps, Paper 17.4-84.8, C2ME 22.6-182.4 según hilos; **vanilla no escala > ~14 cps**. — gist.github.com/ishland — ALTA
9. **Pumpkin self-reported** (su doc avisa que la comparación es injusta): startup ~8 ms vs 7-8 s Paper; RAM 0.4-27 MB vs 1-2 GB; CPU 1.5% vs 20-26%. **Sin chunks/s publicados**. — docs.pumpkinmc.org/about/benchmarks — MEDIA
10. Test comunitario (jul 2026): confirmó startup/RAM de Pumpkin; encontró entidades rotas. — YouTube kxTZb0FYiTU — MEDIA
11. Pumpkin PR #2506: `populate_noise_stage` 43.1 → 18.8 ms/chunk (−56%) con **paridad bit-for-bit verificada por checksums xxHash64**. — github.com/Pumpkin-MC/Pumpkin/pull/2506 — ALTA

### 4. Paridad vanilla

12. Worldgen determinista y data-driven (datapack `worldgen/`); cubiomes reproduce biomas/estructuras por seed. — minecraft.wiki (World_seed, World_generation), cubiomes — ALTA
13. Redstone: orden de updates **PP: W,E,N,S,D,U / NC: W,E,D,U,N,S**; quasi-connectivity solo Java; 1.21.2 cambió el wire (left-first); comportamiento posicional. — minecraft.wiki (Block_update, Redstone_mechanics, 1.21.2) — ALTA
14. Iluminación: Starlight demostró **salida idéntica con engine distinto y más rápido**; 1.20 adoptó sus ideas. — PaperMC/Starlight (TECHNICAL_DETAILS.md) — ALTA
15. Spawns: ciclos (hostil/tick, pasivo/400 ticks), caps (monster 70, creature 10...), pack spawning ±5, reglas de luz/distancia (24/32/128). — minecraft.wiki (Mob_spawning) — ALTA
16. Mob AI **hardcoded** en Java (no data-driven como Bedrock). — minecraft.wiki (Behavior_pack) — ALTA

### 5. Stack Rust

17. **wasmtime**: runtime de referencia (component model, WASI preview 2); Pumpkin probó Extism y migró a wasmtime. — wasmtime.dev, pumpkin issue #662 — ALTA
18. **mlua 0.12** (jul 2026): Lua 5.1-5.5/LuaJIT/Luau. — crates.io/mlua — ALTA
19. **bevy_ecs**: solo el crate ECS (sin el engine); lo usan Valence, FerrumC, Azalea. — repos — ALTA
20. **redb**: KV embeddable activo (ACID); sled en modo mantenimiento. — github.com/cberner/redb — ALTA

### 6. Plugins Java → WASM (límites verificados)

21. TeaVM/JWebAssembly: sin reflection ni class-loading por defecto; CheerpJ = JVM-en-WASM orientada a browser; FerrumC descarta compat Bukkit. **Conclusión: compat Bukkit solo por capas** (API nativa → convertidor para plugins puros → capa PatchBukkit-style). — teavm.org, CheerpJ blog, ferrumc README — ALTA

### 7. Orca ADE + Gauntlet Loop

22. **Orca**: Stably AI, MIT, onorca.dev; worktrees por tarea + orquestación CLI; soporta pi, Claude Code, Codex. — onorca.dev/docs, github.com/stablyai/orca — ALTA
23. **Gauntlet Loop**: Matt Shumer, "How to Run a Gauntlet Loop" (somethingbig.ai/gauntlet-loop, jul 2026), repo mshumer/Claude-of-Duty. Núcleo: split → build → blind critic → repeat contra un bar real; sin cap arbitrario de rondas. — somethingbig.ai, Decrypt, ThePromptIndex, We0 — ALTA
24. Lección del propio Shumer: el bar puede ser **inalcanzable** (su critic nunca ganó a CoD real: 3.59 → 5+/10). El bar tira del trabajo hacia arriba; no se negocia. — somethingbig.ai — ALTA

### 8. Herramientas de medición

25. **spark** (TPS/salud, incluido en Paper 1.21+; Timings deprecado) · **Chunky** (chunks/s) · **mineflayer** (bots ≤ 1.21.11) · **azalea** (bots Rust, trackea 26.x) · marcador de arranque = línea `Done (Xs)!` · RSS por OS (no heap JVM). — spark.lucko.me, Chunky wiki, repos mineflayer/azalea — ALTA

### Mantenimiento

Actualizar este anexo: (a) en cada release de Mojang (pipeline D0-D4); (b) tras cada benchmark publicado; (c) cuando cambie el estado de Pumpkin (nuestra referencia). Toda afirmación nueva entra solo con fuente y fecha.
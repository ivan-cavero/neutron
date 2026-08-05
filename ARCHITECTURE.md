# Neutron — Arquitectura

> v0.1 · 5 ago 2026 · Documento vivo: cada decisión marcada con `[ADR]` se mueve a `docs/adr/` cuando se implementa.
> Todas las decisiones técnicas se basan en investigación verificada (fuentes al final).

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

- **ECS con bevy_ecs** (verificado: Valence, FerrumC y Azalea lo usan en producción; es el estándar de facto en Rust gamedev).
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
| bevy_ecs | simulación de entidades | usado por Valence/FerrumC/Azalea |
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

## Fuentes principales

1. Pumpkin issues #449 (roadmap 1.0), #36 (worldgen), #1402 (redstone), #1404 (combat), PR #2506 (perf+parity worldgen) — github.com/Pumpkin-MC/Pumpkin
2. Pumpkin docs benchmarks — docs.pumpkinmc.org/about/benchmarks · blog.pumpkinmc.org
3. minecraft.wiki: World_seed, World_generation, Block_update, Redstone_mechanics, Tutorial:Quasi-connectivity, Mob_spawning, Data_pack, Java_Edition_1.21.2, Java_Edition_26.1
4. PaperMC/Starlight (TECHNICAL_DETAILS.md) + gist de Spottedleaf (1.20 light rewrite)
5. cubiomes — github.com/Cubitect/cubiomes
6. C2ME benchmarks — gist.github.com/ishland
7. Valence — github.com/valence-rs/valence · FerrumC — github.com/ferrumc-rs/ferrumc
8. TeaVM docs · CheerpJ blog · JWebAssembly — límites Java→WASM
9. wasmtime/wasmi/extism/mlua/redb — repos oficiales
10. minecraft-data (PrismarineJS) — pipeline de datos por versión
11. minecraft.net — versionado por año, 26.1 unobfuscated, 26.2
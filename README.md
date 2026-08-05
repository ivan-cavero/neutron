# Neutron

> El servidor de Minecraft más rápido jamás visto — escrito 100% en Rust.
> **Working title** · Documento de diseño v0.1 · 5 de agosto de 2026 · Estado: PRE-ALPHA (fase de diseño/investigación)

Neutron es un servidor de Minecraft Java Edition (y más adelante Bedrock) reimplementado desde cero en Rust. No es un wrapper ni un fork: es código original, con tres obsesiones:

1. **Extreme performance** — rendimiento *medido, publicado y reproducible*, no marketing.
2. **Security by construction** — plugins en WebAssembly sandboxed: un plugin nunca puede tumbar el servidor ni tocar memoria ajena.
3. **1:1 vanilla parity** — misma seed → mismo mundo. Misma redstone. Misma iluminación. Mismos spawns. Verificado con tests de checksum, no con fe.
4. **Version cadence** — la rama `main` es SIEMPRE la última versión de Minecraft. Actualizar a una versión nueva toma **días, no semanas** (pipeline de extracción de datos + generación de código).

---

## 1. El nombre

**Neutron** — la partícula sin carga eléctrica: densa, veloz, estable; el corazón de la fisión. Encaja con la marca: máxima densidad de rendimiento, sin ruido, sin fricción. Suena igual en español e inglés.

| Nombre | Pros | Contras | Veredicto |
|---|---|---|---|
| **Neutron** | Densidad, física, sin colisión seria en el nicho | Crate `neutron` en crates.io (client de Pulsar, muerto) → nuestros crates van como `neutron-*` | ✅ **Elegido** |
| Hyperion | Titán de la luz, evocador | **PILLADO en el mismo nicho**: `hyperion-mc/hyperion`, un game engine de Minecraft en Rust (Bevy ECS) para eventos masivos | Descartado |
| Quasar | Astronómico, "rápido" | Colisión con proyectos JS/Go conocidos | Descartado |
| Cinder | Ceniza, fuego | Sonido parecido a "Cider" | Descartado |
| Aether | Cielo, mítica | Colisión con el mod Aether | Descartado |

*Verificado el 5 ago 2026: no existe ningún **servidor** de Minecraft llamado Neutron. Colisiones menores en otros dominios: plugin de utilidades para Velocity (Crypnotic, 2019), launcher viejo de MCPE (NeutronLauncher), un cliente Minecraft llamado NeutronMC, el crate `neutron` muerto (Pulsar client 0.0.2) y la blockchain Neutron (neutron.org). Pendiente antes del release público: confirmar disponibilidad de org de GitHub y dominio.*
## 2. Realidad verificada (agosto 2026) — la investigación de la que partimos

Todo lo siguiente fue verificado en fuentes primarias el 5 de agosto de 2026. Fuentes al final de este documento.

| # | Hecho verificado | Fuente | Implicación para Neutron |
|---|---|---|---|
| 1 | La versión actual de Minecraft Java es **26.2 "Chaos Cubed"** (16 jun 2026). **No existe "1.22"**: desde el 2 dic 2025 Mojang usa numeración por año (26.1, 26.2…). 26.3 está en snapshots (Q3 2026) | minecraft.net, minecraft.wiki | `main` = 26.2 hoy; planificar para 26.3 en Q3 2026 |
| 2 | Desde **26.1** (24 mar 2026) el jar ya **no está ofuscado** y requiere Java 25 | minecraft.net | Extraer datos y portar código es MUCHO más fácil que antes |
| 3 | **Pumpkin** es el referente Rust actual: ~10.6k★, GPL-3.0, **solo nightly (sin 1.0)**, worldgen casi completo (biomas, terreno, carvers), **estructuras a medias**, **redstone temprana (pistones rotos)**, iluminación OK, plugins WASM (wasmtime + WIT), PatchBukkit en desarrollo, 1.0 prometida "en 2026" (ya retrasada desde 2025) | GitHub Pumpkin (issues #449, #36, #1402), blog.pumpkinmc.org | Es nuestro benchmark de competencia y de referencia técnica |
| 4 | Benchmarks oficiales de Pumpkin (ellos mismos avisan que la comparación es *injusta*: menos features): startup ~8 ms vs 7-8 s Paper, RAM 0.4-27 MB vs 1-2 GB, CPU 1.5% vs 20-26%. Un test comunitario (jul 2026) confirmó parcialmente los números, pero encontró entidades rotas. **No existen benchmarks públicos de chunks/s de Pumpkin** | docs.pumpkinmc.org/about/benchmarks, YouTube (kxTZb0FYiTU) | Nosotros publicaremos chunks/s desde el día 1: es nuestra ventaja diferencial |
| 5 | Referencia de generación de chunks (1.21.10, metodología C2ME): vanilla ~10-14 chunks/s, Paper ~17-85, C2ME ~23-182 según hilos. Vanilla **no escala** por encima de ~14 cps aunque le des 80 hilos | gist de ishland (C2ME), modrinth C2ME | Objetivo realista: >250 cps @16 hilos |
| 6 | Pumpkin PR #2506 (jul 2026): `populate_noise_stage` 43.1 ms → 18.8 ms/chunk (−56%) con **paridad bit-for-bit verificada por checksums xxHash64** | GitHub PR #2506 | Adoptamos exactamente esa técnica de verificación: checksums en CI |
| 7 | La paridad de worldgen es alcanzable: el worldgen de vanilla es **data-driven** (datapack `worldgen/`) y determinista; cubiomes reproduce biomas/estructuras por seed | minecraft.wiki, cubiomes | Estrategia: embeker el datapack vanilla + portar algoritmos + test de oro por seed |
| 8 | La redstone es EL riesgo #1: es **posicional** (orden de updates PP: W,E,N,S,D,U / NC: W,E,D,U,N,S), quasi-connectivity solo en Java, y 1.21.2 cambió el comportamiento del wire | minecraft.wiki (Redstone_mechanics, Block_update, 1.21.2) | Fase dedicada con suite de "contraptions doradas" posicionales |
| 9 | Iluminación: Starlight demostró que se puede lograr **salida idéntica con un engine distinto y más rápido** (y 1.20 adoptó sus ideas) | PaperMC/Starlight | Engine propio con paridad verificada por tests, no port del algoritmo vanilla |
| 10 | "Convertir plugins Bukkit a WASM" **mágicamente NO es viable**: los plugins dependen de reflection/class loading que TeaVM/JWebAssembly prohíben; CheerpJ es una JVM-en-WASM orientada a browser. FerrumC (server Rust) lo descarta explícitamente | teavm.org, CheerpJ blog, ferrumc README | Estrategia por capas: API nativa → convertidor estático para plugins "puros" → capa de compat PatchBukkit-style (fase tardía) |
| 11 | Stack Rust verificado: **wasmtime** (runtime WASM de referencia; Pumpkin probó Extism y migró a wasmtime), **mlua 0.12** (Lua 5.1-5.5/LuaJIT/Luau, jul 2026), **bevy_ecs** (lo usan Valence, FerrumC y Azalea), **redb** (KV embeddable activo; sled en modo mantenimiento) | repos/crates oficiales | Base tecnológica decidida (detalle en ARCHITECTURE.md) |
| 12 | **Orca ADE** (Stably AI, MIT, onorca.dev) es real: ADE desktop con worktrees por tarea y CLI de orquestación (run-create, task-create, worker-start, check, worker_done, decision gates). Soporta OpenCode, Claude Code, Codex, etc. | onorca.dev/docs, github.com/stablyai/orca | Es nuestra herramienta de orquestación (ver ORCHESTRATION.md) |
| 13 | Herramientas de medición verificadas: **spark** (TPS/salud, incluido en Paper), **Chunky** (chunks/s), **mineflayer** (bots, hasta 1.21.11), **azalea** (bots Rust, trackea 26.1), marcador de arranque = línea `Done (Xs)!` | docs spark, Chunky wiki, repos mineflayer/azalea | Base del harness de benchmarks (ver BENCHMARKS.md) |

---

## 3. Objetivos de rendimiento (TARGETS — a validar, no promesas)

| Métrica | Target Neutron | Referencia verificada |
|---|---|---|
| Startup (mundo vacío → `Done`) | **< 2 s** | Paper 7-15 s; Pumpkin ~8 ms (no precarga mundo) |
| Chunks/s overworld sostenidos @16 hilos | **> 250** | vanilla ~14, Paper ~85, C2ME ~182 (1.21.10) |
| RAM base (idle, mundo vacío) | **< 150 MB** | Paper 1-2 GB; Pumpkin ~100 MB (self-reported) |
| RAM por jugador idle | **< 1 MB** | Paper ~100-200 MB/jugador |
| TPS | **20.0 estable, p99 tick < 25 ms con 500 jugadores** | — |
| Join de jugador (evento spawn del bot) | **< 2 s con 100 joins simultáneos** | — |
| Actualización a nueva versión de Mojang | **main ≤ 7 días tras el release** | otros: semanas/meses |

---

## 4. Neutron vs Pumpkin (honestidad total)

| Dimensión | Pumpkin (ago 2026) | Neutron (objetivo) |
|---|---|---|
| Estado | Nightly, 1.0 "en 2026" | Pre-alpha, 1.0 2027-2028 |
| Worldgen | Biomas+terreno OK, estructuras parciales | Mismo enfoque + checksums de paridad en CI desde F2 |
| Redstone | Temprana (wire OK, pistones rotos) | Fase dedicada con suite de contraptions doradas |
| Plugins | WASM (wasmtime, WIT), PatchBukkit en camino | WASM + Lua + convertidor honesto por capas |
| Benchmarks | Self-reported, sin chunks/s publicados | Públicos, reproducibles, en CI desde F0 |
| Versiones | Java multi-versión + Bedrock WIP | `main` = latest, pipeline de días |
| Licencia | GPL-3.0 | Decisión pendiente (MIT o Apache-2.0 recomendado; todo código es original, sin copiar) |

*No copiamos código de Pumpkin: referenciamos datos públicos (wiki, papers, jars sin ofuscar desde 26.1) y reimplementamos.*

---

## 5. Estructura del repo (objetivo)

```
neutron/
├─ crates/
│  ├─ neutron-core/        # tipos base, registries, ids, utilidades
│  ├─ neutron-data/        # datos GENERADOS (bloques, items, biomas, worldgen JSON)
│  ├─ neutron-protocol/    # paquetes + codec (generado, multi-versión)
│  ├─ neutron-worldgen/    # port de worldgen vanilla (paridad 1:1, checksums)
│  ├─ neutron-world/       # chunks, storage (Anvil .mca + formato propio), region I/O
│  ├─ neutron-sim/         # tick, entidades (bevy_ecs), redstone, iluminación, fluidos, AI
│  ├─ neutron-server/      # runtime: scheduler por regiones, sesiones, networking
│  ├─ neutron-plugin/      # runtime WASM (wasmtime) + API WIT + Lua (mlua)
│  └─ neutron-cli/         # binario principal
├─ tools/
│  ├─ mc-extract/           # extracción jar → JSON (pipeline de versiones, D0-D2)
│  ├─ codegen/              # JSON → código Rust
│  └─ patch-bukkit/         # capa de compat Bukkit/Spigot/Paper (fase tardía, F6)
├─ bench/                   # harness de benchmarks + bots + resultados/
├─ docs/                    # ADRs y documentación técnica
├─ README.md
├─ ARCHITECTURE.md          # arquitectura completa
├─ ROADMAP.md               # roadmap completo F0-F8
├─ BENCHMARKS.md            # metodología y harness de medición
└─ ORCHESTRATION.md         # Orca ADE + prompts + tareas para agentes
```

---

## 6. Documentos clave

- **[ARCHITECTURE.md](ARCHITECTURE.md)** — capas, scheduling, paridad, pipeline de versiones, plugins WASM, stack de dependencias (todo con fuentes).
- **[ROADMAP.md](ROADMAP.md)** — fases F0-F8 con criterios de salida medibles, timeline, riesgos y mitigaciones.
- **[BENCHMARKS.md](BENCHMARKS.md)** — cómo medimos: startup, join, chunks/s, TPS, RAM. Baselines verificados y targets.
- **[ORCHESTRATION.md](ORCHESTRATION.md)** — Orca ADE, reglas para agentes, prompts de ejemplo y tareas grandes listas para desplegar.

---

## 7. Quick start (dev)

```bash
# Prerequisitos: Rust stable (edition 2024), Node.js 20+ (bots), Java 25 (vanilla 26.x de referencia)
git clone <repo> && cd neutron
cargo run --release -p neutron-cli
# Conecta con un cliente vanilla 26.2 a localhost:25565 (online-mode=false por defecto en dev)
```

---

## 8. Fuentes verificadas (selección — todas consultadas el 5 ago 2026)

1. Minecraft versionado por año: https://www.minecraft.net/en-us/article/minecraft-new-version-numbering-system
2. Versión 26.1 (jar sin ofuscar, Java 25): https://www.minecraft.net/en-us/article/minecraft-java-edition-26-1
3. Historial de versiones Java: https://minecraft.wiki/w/Java_Edition_version_history
4. Pumpkin repo + releases: https://github.com/Pumpkin-MC/Pumpkin · https://github.com/Pumpkin-MC/Pumpkin/releases
5. Pumpkin roadmap 1.0: https://github.com/Pumpkin-MC/Pumpkin/issues/449
6. Pumpkin worldgen tracking: https://github.com/Pumpkin-MC/Pumpkin/issues/36
7. Pumpkin redstone tracking: https://github.com/Pumpkin-MC/Pumpkin/issues/1402
8. Pumpkin benchmarks (self-reported): https://docs.pumpkinmc.org/about/benchmarks
9. Pumpkin PR paridad+perf worldgen: https://github.com/Pumpkin-MC/Pumpkin/pull/2506
10. C2ME benchmarks de chunks/s: https://gist.github.com/ishland/6eb0dd0af4216ffffd340ea994dc5796
11. Paridad redstone (update order, QC): https://minecraft.wiki/w/Block_update · https://minecraft.wiki/w/Redstone_mechanics
12. Starlight (iluminación con salida idéntica): https://github.com/PaperMC/Starlight
13. cubiomes (determinismo worldgen): https://github.com/Cubitect/cubiomes
14. TeaVM (límites Java→WASM): https://teavm.org/docs/intro/overview.html · FerrumC: https://github.com/ferrumc-rs/ferrumc
15. wasmtime / wasmi / mlua / bevy_ecs / redb: repos oficiales y crates.io
16. Orca ADE: https://www.onorca.dev/docs · https://github.com/stablyai/orca
17. mineflayer: https://github.com/PrismarineJS/mineflayer · azalea: https://github.com/azalea-rs/azalea
18. spark: https://spark.lucko.me/docs/Command-Usage · Chunky: https://github.com/pop4959/Chunky
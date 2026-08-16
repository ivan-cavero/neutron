# Neutron

Servidor de Minecraft Java Edition reimplementado desde cero en Rust. Multiplataforma
(Windows/Linux/macOS x86-64/ARM64), paridad 1:1 con vanilla, plugins WASM/Lua seguros por
construcción, y `main` siempre en la última versión de Minecraft.

**Estado**: PRE-ALPHA · worldgen F2d activo (paridad de mecanismo, run-046) · servidor 26.2 jugable

## Qué es este proyecto

1. **Extreme performance** — rendimiento medido y publicado con metodología reproducible (BENCHMARKS.md), no marketing.
2. **Security by construction** — plugins en sandbox WASM: un plugin nunca tumba el servidor.
3. **1:1 vanilla parity** — misma seed → mismo mundo; redstone, iluminación y spawns idénticos; verificado por checksums en CI.
4. **Version cadence** — `main` = última versión de Mojang en ≤ 7 días (pipeline D0-D4).

---

## Cómo se trabaja en este proyecto con la AI (LEER)

Este repo está diseñado para que un agente (pi, opencode o zcode) trabaje sobre él con
**estado en disco, no en memoria de chat**. El método es genérico — sirve para worldgen,
redstone, protocolo, tools, lo que sea — y escala porque cada sesión reconstruye su
contexto desde archivos, no desde la conversación anterior.

### El método: Gauntlet Loop

```
LEAD → divide el objetivo en piezas gradeables
  ├─ BUILDER construye cada pieza
  └─ CRITIC (subagente, contexto limpio) inspecciona el artefacto REAL contra el bar
       PASS → siguiente pieza · FAIL → el gap más grande → reconstruir → repetir
```

Reglas no negociables: el **bar** es una referencia real (checksum, benchmark, server
vanilla) que nunca se edita para que un test pase · el **builder nunca se autoevalúa** ·
**ratchet**: cada ronda re-mide TODOS los seeds, una regresión es FAIL · **commits
incrementales**: cada pieza probada se commitea sola, nunca mega-commits.

### Mapa de archivos (qué es cada uno, quién lo toca)

| Archivo | Qué es | Quién lo lee | Quién lo escribe | Cuándo |
| --- | --- | --- | --- | --- |
| `AGENTS.md` | Contrato universal: cómo se trabaja, bar, loop, límites, tools | todo agente, al empezar | humano + LEAD | cuando cambia el método |
| `STATE.md` | **Estado real** (≤80 líneas): fase, bar único, última medición, próxima acción, gaps | todo agente, al empezar | LEAD, al cerrar cada run | cada run |
| `workbench.md` | Round log VIVO del run activo: ronda actual, PASS/FAIL por unidad | LEAD + quien supervisa | LEAD, tras cada ronda | cada ronda |
| `runs/run-NNN.md` | Evidencia de cada run: objetivo, bar, tareas, logs, rounds | critic ciego + quien audita | LEAD | cada run |
| `runs/README.md` | Plantilla de run + disciplina de PASS + cómo lanzar | LEAD | LEAD | cuando cambia la plantilla |
| `ROADMAP.md` | Fases + bars + links (índice, no prompts) | LEAD | humano + LEAD | cuando cambia el plan |
| `docs/prompts/*.md` | Prompts de fase listos para copiar a pi | LEAD | LEAD | al lanzar una fase |
| `ARCHITECTURE.md` | Diseño del servidor + evidencia verificada | quien diseña | humano | cuando cambia el diseño |

**Reglas de estado** (contra el "estado falso"):

- Al empezar sesión, **auditar STATE.md contra la evidencia real** (git log, runs/,
  logs): si una afirmación no tiene archivo de evidencia, se re-mide, no se confía.
- El estado lo escribe quien tiene la evidencia, nunca se copia de resúmenes ajenos.
- Un PASS exige critic ciego con evidencia; lo verificado por el builder se etiqueta
  "builder-verified", nunca PASS.
- **Resume test**: el sistema funciona si podés matar la sesión, retomarla, y el próximo
  agente retoma solo desde disco.

### Harness (pi / opencode / zcode)

El harness principal es **pi** (con plugins). `AGENTS.md` es el contrato universal: todos
los harness mainstream lo leen. Los nombres de herramientas de `AGENTS.md` §7 son de pi
(`subagent`, `todo`, `ask_user_question`); opencode/zcode mapean esos roles a sus propias
herramientas — los roles importan, no los nombres. La delegación de subagentes (builder
vs critic con contexto limpio) y la búsqueda en internet (`web_search`/`fetch_content`)
funcionan igual en los tres.

### Skills

Cargar **solo las skills del proyecto que apliquen a la tarea** (p. ej. buenas prácticas
de Rust para una tarea de worldgen, gauntlet-loop para un run). No cargar skills que no
tengan que ver con la tarea. Viven en el directorio de skills del harness.

---

## Cómo orientarte (qué leer cuándo)

| Necesitas | Documento |
| --- | --- |
| Saber qué es esto y cómo se trabaja con la AI | este README |
| Saber en qué punto estamos y qué sigue | STATE.md |
| El plan (fases, bars, pipeline de versiones) | ROADMAP.md (prompts en docs/prompts/) |
| Cómo está diseñado el servidor + evidencia | ARCHITECTURE.md (Anexo A) |
| Cómo se miden los benchmarks | BENCHMARKS.md |
| Cómo trabajar / lanzar un run | AGENTS.md + runs/README.md |

## Targets (a validar con BENCHMARKS.md)

| Métrica | Target |
| --- | --- |
| Startup (mundo vacío → `Done`) | < 2 s |
| Chunks/s @16 hilos | > 250 |
| RAM base | < 150 MB |
| RAM por jugador | < 1 MB |
| TPS @500 jugadores | 20.0, p99 < 25 ms |
| Join p95 @100 bots | < 2 s |
| Nueva versión de Mojang | main ≤ 7 días |

## Estructura del repo (hoy)

El diagrama de `ARCHITECTURE.md` describe el **objetivo** (cli, plugins WASM, Folia). El grafo real es más pequeño:

```
neutron/
├─ crates/
│  ├─ neutron-protocol/     # paquetes 26.2 (a mano)
│  ├─ neutron-world/        # Anvil / level.dat (aún no lo usa el server)
│  ├─ neutron-worldgen/     # overworld 26.2 — el foco actual de parity
│  ├─ neutron-server/       # binario jugable: login + chunks
│  ├─ neutron-sim/          # luz / redstone / fluidos / spawn (tests, no cableado)
│  └─ neutron-bench-server/ # criterion
├─ tools/                   # golden-data · parity-check · vanilla-extract · java-probe
├─ bench/                   # workspace aparte: bots + jars de referencia
├─ runs/                    # historial de runs (run-NNN.md)
├─ docs/prompts/            # prompts de fase para pi
└─ docs/                    # ADRs y notas
```

## Quick start (dev)

```bash
# Servidor jugable (worldgen real, seed 12345)
cargo run --release -p neutron-server -- --seed 12345 --view-distance 8
# Cliente vanilla 26.2 → localhost:25565  (online-mode=false, Creative + vuelo)
# Estado de worldgen y gaps: STATE.md + crates/neutron-worldgen/WORLDGEN.md
```

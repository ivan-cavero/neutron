# Neutron — Benchmarks: metodología y harness

> v0.1 · 5 ago 2026 · Regla del proyecto: **todo número publicado tiene metodología reproducible y datos crudos**. Esto es lo que nos diferencia de los benchmarks self-reported.

## 1. Filosofía

1. Un benchmark sin metodología es marketing. Publicamos: hardware, software, versiones, comandos, datos crudos (`bench/results/*.json`) y tabla markdown autogenerada.
2. **Misma máquina, misma seed, mismo procedimiento** para vanilla, Paper, Pumpkin y Neutron. Sin "condiciones especiales" por servidor.
3. Baselines verificados de la comunidad se citan con fuente y se REPRODUCEN en nuestra máquina antes de usarlos como referencia.
4. El benchmark es un artefacto de CI: si una PR regresiona una métrica clave, la PR no entra.

## 2. Métricas y definiciones EXACTAS

| Métrica | Definición | Cómo se mide |
|---|---|---|
| **Startup** | tiempo desde spawn del proceso hasta que el server está jugable | regex de log: `Done (Xs)!` (marcador estándar usado por launchers; socket abierto ≠ jugable). 5 runs, mediana. |
| **Join (jugador)** | latencia percibida por el cliente | bot (mineflayer/azalea): `t(createBot)` → evento `login` (handshake) y evento `spawn` (mundo listo). p50/p95/p99 con N bots. |
| **Chunks/s (cps)** | chunks generados por segundo, sostenido | vanilla/Paper: **Chunky** (`chunky radius N`, `chunky start`, `chunky progress` da cps). Pumpkin/Neutron: contador propio del servidor o método equivalente con la misma carga (mismo radio, mismo seed, misma altura). |
| **TPS / MSPT** | ticks por segundo / ms por tick (20 TPS = 50 ms/tick) | Paper: **spark** (`/spark tps`, `/spark health` — Timings v2 está deprecado desde 1.21). Pumpkin: logs/métricas propias. Neutron: endpoint de métricas. Umbrales: <40 ms/tick sano, 40-50 marginal, >50 lag. |
| **RAM (RSS)** | footprint real del proceso (no heap JVM) | muestreo OS cada 1 s (Linux: `ps`/`smaps`; Windows: `Get-Process` WorkingSet64) durante 60 s idle y durante carga. Para JVM: `-Xms=-Xmx` + `-XX:+AlwaysPreTouch` para que el heap reportado sea real (Aikar). |
| **CPU** | % de uso respecto a la máquina | muestreo OS 1 Hz; reportar idle y pico (el pico inicial de carga de chunks se reporta aparte). |

**Nota sobre FPS**: los FPS son métrica de CLIENTE (render), no del servidor. La métrica server-side equivalente es TPS/MSPT. Si algún día queremos FPS de cliente, se necesita un cliente headless real (fuera de alcance v1 del harness).

## 3. Metodología estándar (cada run)

1. **Hardware fijo**: registrar CPU/RAM/SSD/SO/kernel. Idealmente la misma máquina para toda una tanda comparativa.
2. **Seed fija** (p.ej. `1234567890123456789`) en `server.properties` / config equivalente; `online-mode=false`; `view-distance=10`; `simulation-distance=10`.
3. **Mundo limpio**: carpeta de mundo vacía en cada run (o `tmpfs` en Linux, como la metodología C2ME).
4. **Warmup**: 60 s idle antes de medir (JIT en Java, caches en Rust).
5. **N=5 runs** por escenario; reportar **mediana** (y min/max).
6. **Escenarios**:
   - `idle`: 0 jugadores, mundo vacío.
   - `gen`: generación de chunks pesada (Chunky, radio fijo, p.ej. 1024 bloques = 16.384 chunks).
   - `join`: N bots en ráfaga (10, 50, 100) midiendo login/spawn.
   - `load`: 10-100 bots quietos 60 min → TPS/RAM/leaks.
   - `stress`: 500 bots (solo cuando el servidor lo soporte; F4+).
7. **Salida**: `bench/results/<ID>-<fecha>.json` + tabla markdown autogenerada + logs crudos en `bench/logs/`.

## 4. Harness (`bench/`)

```
bench/
├─ run.ps1 / run.sh        # orquesta: levanta servidor → bots → métricas → JSON
├─ servers/
│  ├─ vanilla/             # server.jar 26.2 + Java 25 + scripts de arranque
│  ├─ paper/               # última build Paper (verificar soporte 26.x en el momento)
│  ├─ pumpkin/             # nightly de Pumpkin (releases oficiales)
│  └─ neutron/            # cargo run --release -p neutron-cli
├─ bots/
│  ├─ join-bench/          # Node.js + mineflayer (≤1.21.11) o azalea (26.x)
│  └─ verify-world/        # checksum de chunks, /seed, /locate
└─ results/                # JSON + markdown autogenerado (commiteado)
```

Reglas de fairness:
- Ningún plugin/mod de optimización en vanilla/Paper (sin C2ME, sin Lithium, sin mods). Paper con su config por defecto.
- Misma carga exacta: mismo radio de gen, mismos bots, misma seed.
- Pumpkin corre con su config por defecto (documentar si se cambia algo).
- Si un servidor no soporta algo (p.ej. Chunky no corre en Pumpkin), usar método equivalente y DOCUMENTARLO en el resultado.

## 5. Cómo levantar cada servidor (operating manual)

| Servidor | Comando (Linux) | Notas |
|---|---|---|
| Vanilla 26.2 | `java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui` | **Java 25 obligatorio** (26.1+). `eula.txt=true`, `online-mode=false`, `level-seed=...`. Esperar `Done (Xs)!` |
| Paper | descargar build de downloads.papermc.io; mismo arranque | spark incluido: `/spark health`, `/spark tps`; rate limit de comandos ~15/s → bots con throttle |
| Pumpkin nightly | binario oficial (win/linux/mac) | config.toml: `online_mode = false`, seed; sin método Chunky → contador propio |
| Neutron | `cargo run --release -p neutron-cli` | endpoint de métricas + modo `neutron bench` |

Bots: **mineflayer** (Node.js, maduro, 1.8→1.21.11) y **azalea** (Rust, trackea 26.x). Quirks conocidos a documentar en el harness:
- En 1.20.2+ desactivar física del bot hasta el evento `spawn` (`physicsEnabled: false`) o puede ser kickeado.
- No usar proxies (Velocity/Bungee) en los tests de join directo.
- Paper rate-limita comandos (~15/s): throttle con `sleep(80ms)` entre comandos.

## 6. Baselines verificados (agosto 2026) — nuestro punto de partida

| Métrica | Vanilla | Paper | C2ME | Pumpkin (self-reported) | Fuente |
|---|---|---|---|---|---|
| Startup | 7-15 s | 7-10 s | — | ~5-8 ms (sin precargar mundo) | docs.pumpkinmc.org/about/benchmarks |
| RAM idle | 0.9-1.8 GB | 1.1-2.2 GB | — | ~100 MB (0.4-27 MB sin jugadores en su tabla) | docs.pumpkinmc.org + test comunitario jul 2026 |
| CPU idle | ~24% | ~20% | — | ~1.5% | docs.pumpkinmc.org |
| cps (1.21.10, seed fija, tmpfs) | 10.6-14.2 | 17.4-84.8 (escala hasta ~8 hilos) | 22.6-182.4 (escala a 80 hilos) | **no publicado** | gist de ishland (C2ME) |
| TPS | 20 | 20 | 20 | 20 (objetivo roadmap) | — |

*Ojo: los números de Pumpkin son self-reported y su propia doc avisa que la comparación es injusta (menos features). Los de C2ME son la metodología más rigurosa publicada (seed fija, tmpfs, warmup, N runs).*

## 7. Targets de Neutron (a validar — ver README §3)

| Métrica | Target |
|---|---|
| Startup (mundo vacío → `Done`) | < 2 s |
| cps overworld @16 hilos | > 250 sostenidos |
| RAM base idle | < 150 MB |
| RAM/jugador idle | < 1 MB |
| TPS @500 jugadores | 20.0, p99 tick < 25 ms |
| Join p95 @100 bots | < 2 s |
| Actualización de versión | main ≤ 7 días |

## 8. Plantilla de resultados (se rellena en cada run)

```markdown
# Benchmark <ID> — <fecha> — <hardware>
SO: <os/kernel> · CPU: <modelo> · RAM: <GB> · Disco: <tipo> · Java: <versión> · Rust: <versión>
Seed: <seed> · View: 10 · Sim: 10 · online-mode: false · Mundo: <limpio/tmpfs>
Warmup: 60 s · Runs: 5 (mediana)

| Servidor | Versión | Startup | RAM idle | RAM 100j | CPU idle | cps | TPS p99 | Join p50 | Join p95 |
|---|---|---|---|---|---|---|---|---|---|
| Vanilla | 26.2 | | | | | | | | |
| Paper | <build> | | | | | | | | |
| Pumpkin | nightly <hash> | | | | | | | | |
| Neutron | <commit> | | | | | | | | |
```

*Notas de desviación: (documentar cualquier excepción a la metodología)*
*Logs: bench/logs/<ID>/* · *JSON: bench/results/<ID>.json*
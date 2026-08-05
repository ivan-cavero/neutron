# AGENTS.md — Neutron: cómo trabajamos

> v0.4 · 5 ago 2026 · **opencode lee este archivo automáticamente** al trabajar en el repo. Trabajo 100% con opencode por ahora.

## 0. DÓNDE SE ESCRIBE TODO (regla de carpeta — no negociable)

**La única carpeta de trabajo es la raíz de este repositorio: la carpeta en la que estamos trabajando en este momento (el directorio de trabajo actual, donde vive este AGENTS.md).** Ahí se ejecuta todo y ahí vive el código, los documentos y los runs. No se asume ninguna ruta absoluta fija: la ruta concreta es la del equipo en el momento de ejecutar.

- Prohibido crear, escribir o editar archivos fuera del directorio de trabajo actual: worktrees, carpetas de herramientas externas (tipo ...\orca\workspaces\...), carpetas temporales, otras unidades.
- Si encuentras trabajo en una ruta externa, se trae al proyecto y se borra lo externo.
- Si algo no se puede hacer en el proyecto (por ejemplo, una herramienta exige otra ruta), se pregunta al humano antes de tocar nada.

## 1. Modelo de trabajo

Un agente opencode hace de **LEAD**: lee el estado, genera el run (con el prompt de §7), ejecuta las tareas y entrega evidencia. Todo el trabajo ocurre en la carpeta del proyecto (regla §0): nada de worktrees ni carpetas externas. La calidad la asegura un **Gauntlet Loop**: cada pieza se construye contra un **bar** y la juzga un **critic** lanzado como subagente con contexto limpio.

```
LEAD (opencode)
  ├─ construye cada pieza (builder)
  └─ critica (subagente con contexto limpio, inspecciona lo real)
       PASS → siguiente pieza
       FAIL → el gap más grande → reconstruir → repetir
```

## 2. Gauntlet Loop (lo esencial)

- **Bar**: referencia real e innegociable — checksum de vanilla, benchmark, server real, test suite. Vanilla es nuestro "Call of Duty": no se negocia, se cumple o no. Puede ser inalcanzable: eso es correcto, tira el trabajo hacia arriba.
- **Builder nunca se autoevalúa**: el critic (subagente, contexto limpio, sin la historia del builder) inspecciona el artefacto REAL — logs, JSON, tests ejecutados por él mismo — no el resumen.
- **FAIL → el gap más grande**: el critic devuelve UNA cosa, la más importante; se corrige y se repite.
- **Sin cap arbitrario de rondas**: se para cuando el bar gana, 2 rondas sin mejora, o presupuesto agotado.
- Origen verificado: Matt Shumer, "How to Run a Gauntlet Loop" (jul 2026) — ARCHITECTURE.md (Anexo A, §7).

## 3. Reglas no negociables

1. **El bar no se toca.** Criterios y tests de paridad solo cambian con tu aprobación. Cambiar un test para que pase = trampa.
2. **Builder ≠ critic.** Postura por defecto del critic: REJECT hasta tener evidencia.
3. **Evidencia real, no afirmaciones.** Logs crudos con timestamps, hashes, salidas de bots, enlaces a reports. "Funciona" no es evidencia.
4. **Presupuesto como guardrail.** Tokens/tiempo estimado por tarea antes de empezar; al 80% solo reportar; al 100% salir con nota en el run.
5. **Regla de oro de Neutron.** Ninguna tarea de código se da por terminada sin su benchmark o parity test asociado en CI.
6. **Gates humanos.** Releases, credenciales y cambios de criterios pasan por ti.

## 4. Operating manual (lo esencial)

- **Vanilla 26.2**: Java 25. `java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui` con `eula.txt=true`, `online-mode=false`, `level-seed=<fija>`, `view-distance=10`. Arranque = línea `Done (Xs)!`.
- **Paper**: última build (verificar soporte 26.x). Spark incluido (`/spark tps`). Rate limit de comandos ~15/s → throttle de bots (sleep ≥ 80 ms).
- **Pumpkin nightly**: binario de releases oficiales; `config.toml` con `online_mode = false`. No tiene Chunky → cps con método propio.
- **Neutron**: `cargo run --release -p neutron-cli`.
- **Bots**: mineflayer (Node, ≤ 1.21.11; quirk 1.20.2+: `physicsEnabled: false` hasta `spawn`) · azalea (Rust, 26.x — usar para 26.2).
- **Métricas**: startup regex `Done (Xs)!` · join timestamps de login/spawn · cps Chunky (vanilla/Paper) o contador propio · TPS spark/endpoint · RAM RSS por OS.

## 5. Formato de tarea (markdown simple)

```markdown
### T1 — <título>
- Qué: <qué debe ser verdad al terminar, medible>
- AC: <criterios concretos con umbrales>
- Evidencia: <logs, hashes, salidas que se pegarán>
- DoD: <qué ejecuta el critic desde cero para dar PASS>
```

## 6. PROMPT PARA GENERAR RUNS (copy-paste en opencode)

```text
Eres el LEAD del proyecto Neutron. Prepara el siguiente run de trabajo.

PASO 1 — Lee el estado actual (todo el contexto):
- STATE.md (fase actual y bar actual)
- runs/README.md (formato de run) y runs/run-*.md (historial completo:
  qué se hizo en cada run, con qué resultado y evidencia)
- ROADMAP.md (fases, bars, piezas de cada fase)
- README.md (contexto del proyecto)
- AGENTS.md (este documento: normas, operating manual, formato de tarea)

PASO 2 — Decide el siguiente run:
- Si ya hay un run de la fase actual en curso → resúmelo y NO dupliques.
- Si el bar de la fase actual está cumplido (evidencia en el historial)
  → avanza la fase en STATE.md y plantea el run de la siguiente.
- Si no → genera el siguiente run con el formato de runs/README.md:
  objetivo (una frase), bar, tareas (1-5 con AC, evidencia y DoD) y
  presupuesto orientativo.

PASO 3 — Registra y entrega:
- Crea runs/run-<NNN>.md (siguiente número correlativo) y actualiza STATE.md.
- Termina con: qué run creaste, por qué, y cómo lanzarlo (pegar su objetivo
  en opencode añadiendo: "Eres el LEAD de este run. Ejecuta el Gauntlet Loop:
  construye cada tarea y lanza un subagente critic con contexto limpio que
  inspeccione el artefacto real. No te autoevalúes. Al terminar actualiza
  runs/run-NNN.md y STATE.md.").
```

## 7. Loops de automatización (corren solos en CI)

| Loop | Frecuencia | Gatillo |
|---|---|---|
| Smoke E2E (levantar, join, mover, romper, chat, TPS) | diario | cron |
| Benchmarks de regresión (cps, TPS, RAM) | semanal | cron |
| Pipeline de versiones D0-D4 (main = última de Mojang, ≤ 7 días) | cada release | webhook |
| Fuzzing del protocolo | continuo | cada merge a main |
| Suite de parity (checksums + contraptions) | cada merge | PR |

Los agentes construyen cada loop UNA vez (en su fase); después corre solo.
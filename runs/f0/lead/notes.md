# RUN F0 — Notas del LEAD (contexto de entorno verificado, 5 ago 2026)

Verificado por el LEAD (no por los builders) antes del primer dispatch:

## Hardware/SO de la máquina de referencia

- Windows (win32), repo en `C:\Users\ivang\neutron` (también `F:\Hyperion`, `C:\Users\ivang\orca\...` existen como otros worktrees de Orca, NO usar).
- Node v24.18.0, npm 11.4.2 — OK para bots mineflayer/azalea.
- cargo/rustc 1.96.0 — OK para el workspace.
- Java: **openjdk 21.0.7 Temurin** — **INSUFICIENTE**: vanilla 26.2 exige Java 25 (26.1+). El builder DEBE instalar un JDK 25 (p.ej. Temurin 25) o declarar la desviación. Java 21 está en PATH; el harness debe usar JAVA25_HOME o ruta explícita.
- **WSL: NO hay distro Linux** (solo `podman-machine-default`, Stopped). El bar de F0 pide correr en Windows Y Linux. El builder de T-B0 debe evaluar opciones reales (instalar distro WSL, contenedor, o máquina remota) ANTES de implementar run.sh, y si no hay Linux disponible, escalar con `ask` — NUNCA dar el AC B0-1 por cumplido sin evidencia en Linux real.
- Disco: registrar modelo/tipo (SSD/HDD) al publicar el baseline (BENCHMARKS.md §3.1).

## Reglas del LEAD para todos los workers

1. Worktree propio: T-B0 → `f0-t-b0`, T-CI → `f0-t-ci` (creados por el LEAD con worker-start; nunca editar el otro worktree ni main directamente).
2. Presupuestos (guardrail): T-B0 200k tokens / 120 min; T-CI 100k / 90 min; T-BASE 150k / 180 min. Heartbeat si el trabajo es largo. Al 100%: salir con nota.
3. Evidencia: pegar logs crudos con timestamps en `worker_done`; guardar logs en `bench/logs/` (T-B0/T-BASE).
4. `eula.txt=true` está permitido (uso local de testing). No publicar nada fuera del repo sin gate humano.
5. Descargas: Mojang server.jar 26.2, Paper (builds.papermc.io), Pumpkin nightly (GitHub releases) — rutas a fijar en `bench/servers/` (BENCHMARKS.md §4). Documentar la URL + hash (sha256) de cada artefacto en el JSON del baseline.
6. Seed fija del proyecto para benchmarks: `1234567890123456789` (BENCHMARKS.md §3.2) salvo que el harness documente otra.
7. El builder NUNCA se autoevalúa: al terminar, `worker_done` con evidencia y esperar. El critic (agente distinto, contexto limpio) ejecutará el DoD desde cero.

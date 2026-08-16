# AGENTS.md — Neutron: how we work

> v1.0 · 16 Aug 2026 · Read automatically by the coding agent (pi) at task start.
> Agent-facing files are in English. Human docs (README.md) may stay in Spanish.

## 0. Working directory rule

The only working directory is the repository root (where this file lives). Never create,
write, or edit files outside it. If work exists on an external path, bring it into the
project. If something cannot be done inside the project, ask the human first.

## 1. What this project is

Neutron is a Minecraft server in Rust targeting **vanilla 26.2**. The hardest goal is
**1:1 world generation parity**: same seed → byte-identical world vs vanilla. Current
parity work is in `crates/neutron-worldgen` (see `STATE.md`).

## 2. How we work: Gauntlet Loop

```
LEAD (pi)
  ├─ splits the goal into independently gradeable pieces
  ├─ BUILDER builds each piece
  └─ CRITIC (subagent, clean context) inspects the REAL artifact
       PASS → next piece
       FAIL → the single biggest gap → rebuild → repeat
```

- **Bar**: a real, non-negotiable reference (checksum, benchmark, vanilla server, test
  suite). Never edited to make a test pass — that is cheating. An unreachable bar is
  correct: it pulls work upward.
- **Builder never grades itself.** The critic is a subagent with clean context that
  inspects the real artifact (logs, JSON, tests it runs itself) — never the builder's
  summary. Default stance: REJECT until evidence.
- **Ratchet**: every round must re-measure ALL seeds in the current bar (multi-seed
  regression gate). A regression on any seed is a FAIL.
- **Stop**: bar wins, 2 rounds without improvement, or budget exhausted. Record what
  is still below the bar.
- Evidence = raw logs with timestamps, hashes, bot outputs, links. "It works" is not
  evidence.

## 3. Commands

```bash
cargo test --workspace          # all tests (must be green before commit)
cargo test -p neutron-worldgen  # worldgen tests (59)
cargo build --release           # full release build
# server (26.2, joinable):
cargo run --release -p neutron-server -- --seed 12345 --view-distance 8
# parity measurement (release examples, see runs/run-046.md for args):
cargo run --release -p neutron-worldgen --example region_parity -- 424242 0 0 1 <region_dir>
cargo run --release -p neutron-worldgen --example clay_overlap -- 424242 0 0 1 <region_dir>
cargo run --release -p neutron-worldgen --example lush_pale_parity -- 424242 0 0 1 <region_dir>
python tools/nbt-ref/multiseed.py # multi-seed parity sweep
```

## 4. Project structure

```
crates/
  neutron-protocol/   # 26.2 packets, hand-written
  neutron-world/      # Anvil + level.dat (not wired to server)
  neutron-worldgen/   # overworld; 1:1 parity work lives here
  neutron-server/     # the playable binary
  neutron-sim/        # light/redstone/fluids/spawns test engines (not wired)
  neutron-bench-server/ # criterion
tools/
  java-probe/         # Java verification probes vs vanilla jar
  nbt-ref/            # vanilla reference worlds + multiseed.py (runtimes gitignored)
  vanilla-extract/    # decompiled vanilla sources (local, gitignored)
tests/e2e-server/     # bot E2E
runs/                 # run history (run-NNN.md) + README with template
docs/prompts/         # phase prompt templates (from ROADMAP.md)
```

## 5. Operating manual

- **Vanilla 26.2**: Java 25. `java -Xms2G -Xmx2G -XX:+AlwaysPreTouch -jar server.jar nogui`,
  `eula.txt=true`, `online-mode=false`, fixed `level-seed`, `view-distance=10`. Startup
  marker = line `Done (Xs)!`.
- **Paper**: latest build (verify 26.x support). Spark included (`/spark tps`). Command
  rate limit ~15/s → bots need sleep ≥80 ms.
- **Pumpkin nightly**: official release binaries; `config.toml` with `online_mode = false`.
  No Chunky → own cps counter.
- **Neutron**: `cargo run --release -p neutron-server -- --seed 12345 --view-distance 8`.
- **Bots**: mineflayer (Node, ≤1.21.11; quirk 1.20.2+: `physicsEnabled: false` until
  spawn) · azalea (Rust, 26.x — use for 26.2).
- **Metrics**: startup `Done (Xs)!` regex · join timestamps · cps via Chunky (vanilla/
  Paper) or own counter · TPS via spark/endpoint · RAM RSS by OS.
- **Reference worlds**: `tools/nbt-ref/vanilla-fresh-*` (gitignored, re-extract with
  `multiseed.py`/pregen scripts).

## 6. Running a run

**Single source of truth: `runs/README.md`** (template, how to launch, orchestration).
Read it before creating a run. Summary:

1. Read `STATE.md` → decide which run is next (bar not met → same run continues).
2. Create `runs/run-NNN.md` with the template (objective, bar, tasks with
   What/AC/Evidence/DoD).
3. Track units with `todo`; launch builders via `subagent` (parallel, background).
4. Gauntlet Loop: builder → blind critic (`subagent` with clean context) → fix → repeat.
5. Update `workbench.md` (round log) and `STATE.md` (state, not history) when done.

## 7. pi tools (this harness)

| Tool | Use |
| --- | --- |
| `subagent` | builder / blind critic / Explore (read-only) subagents, async or foreground |
| `todo` | task tracking with status and dependencies |
| `ask_user_question` | human gates (releases, credentials, bar changes) |
| `bash` / `read` / `write` / `edit` | commands and file manipulation |
| `subagent_wait` | block until an async subagent finishes |
| `web_search` / `fetch_content` | research (crates.io, minecraft docs, vanilla sources) |

## 8. Git workflow

- Commit in small, descriptive units. Always run `cargo test --workspace` first.
- **Never commit**: `target/`, `bench/results/` dumps, vanilla runtimes
  (`tools/nbt-ref/vanilla-*/`), jars, `tools/vanilla-extract/` extract, `logs/`, `tmp*`.
- A PASS in a run file requires blind-critic evidence. Builder-verified work is labeled
  "builder-verified", never PASS.
- When in doubt about a bar or a boundary, ask the human — do not improvise a new bar.

## 9. Boundaries (never do)

- Edit measurement examples or tests to make the bar pass.
- Touch `tools/` while the human is refactoring it (in progress, Aug 2026).
- Modify `STATE.md` history or run files retroactively (append only).
- Commit secrets, jars, worlds, or bulk runtime data.
- Declare a task done without its parity test or benchmark (golden rule).

## 10. Task format

```markdown
### T1 — <title>
- What: <what must be true at the end, measurable>
- AC: <concrete criteria with thresholds>
- Evidence: <logs, hashes, outputs to paste>
- DoD: <what the critic runs from scratch to give PASS>
```

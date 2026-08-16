# AGENTS.md — Neutron: how we work

> v1.1 · 16 Aug 2026 · Read automatically by the coding agent at task start.
> All docs are in English (README included).
> **Harness-agnostic**: this file is the universal contract. Primary harness is **pi**
> (with plugins); opencode/zcode also work. Tool names in §7 are pi-specific — each
> harness maps them to its own (see README.md §AI workflow).

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
- **Reviewer/verifier**: before trusting state, an independent read-only agent audits
  the claims against repository evidence (git log, run files, test output). If the
  builder's critic missed something, the reviewer catches it. Every session starts
  with this audit (§2.5).
- **Stop**: bar wins, 2 rounds without improvement, or budget exhausted. Record what
  is still below the bar.
- Evidence = raw logs with timestamps, hashes, bot outputs, links. "It works" is not
  evidence.

## 2.5 Session start: audit the state (resume boundary)

**Never trust STATE.md or workbench.md on faith.** A state file can be stale, wrong,
or written by an agent that misjudged. Before doing anything, verify it against
repository evidence:

1. `git status --short` + `git log --oneline -10` — does the working tree match what
   STATE.md claims (committed WIP, uncommitted changes)?
2. Re-read the latest `runs/run-NNN.md` + `workbench.md` round log — do the claimed
   numbers match the evidence files (logs, JSON, test output) they cite?
3. If a claim is unverifiable (no evidence file, no log, no test run), **flag it and
   re-measure** — do not build on it. State written by an agent that holds no evidence
   is a hypothesis, not a fact.
4. Only then choose the next action. If the audit contradicts STATE.md, fix STATE.md
   first (append a correction, never rewrite history).

This is the **resume test**: the system works when you can kill a session, resume,
and the next agent picks up correctly from disk alone — no chat memory needed.

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

## 5.5 Parallel agents & concurrency

Multiple agents may run on this repo at once (parallel sessions, or subagents).
Without rules they clobber each other — this happened Aug 2026: a parallel agent
overwrote `ARCHITECTURE.md` and deleted docs while the main session worked. Rules:

1. **Ownership map** — declare who owns what BEFORE parallel work starts:
   - Human: `tools/` (refactor in progress, Aug 2026) — agents never touch it.
   - LEAD: `STATE.md`, `workbench.md`, `runs/` (single writer, append-only).
   - Builder: only the files of its assigned unit (from the run file).
   - Critic/Reviewer: read-only — never writes.
2. **One writer per file**: if a file is not yours, don't edit it. Two agents
   editing the same file = guaranteed clobber.
3. **Worktree isolation (preferred)**: parallel agents work in separate git
   worktrees; the LEAD merges. Same-tree parallel work only when ownership is
   disjoint (different dirs).
4. **Commit early**: commit each proven increment so others can pull. Uncommitted
   changes are invisible to everyone else — don't hold them long.
5. **If you see foreign uncommitted work**: stop, don't overwrite it, ask the
   human who owns it.

## 6. Running a run

**Single source of truth: `runs/README.md`** (template, how to launch, orchestration).
Read it before creating a run. Summary:

0. **Audit the state first** (§2.5) — verify STATE.md against evidence before trusting it.
1. Read `STATE.md` → decide which run is next (bar not met → same run continues).
2. Create `runs/run-NNN.md` with the template (objective, bar, tasks with
   What/AC/Evidence/DoD).
3. Track units with `todo`; launch builders via `subagent` (parallel, background).
4. Gauntlet Loop: builder → blind critic (`subagent` with clean context) → fix → repeat.
5. **Commit proven increments as you go** (§8) — never wait until the end.
6. Update `workbench.md` (round log) and `STATE.md` (state, not history) when done.

## 7. Tools (harness-dependent)

Primary harness: **pi** (with plugins). Other harnesses (opencode, zcode) map these
roles to their own tools — the roles are what matter, not the names.

| Role | pi tool |
| --- | --- |
| builder / blind critic / Explore (read-only) subagents | `subagent` |
| task tracking with status and dependencies | `todo` |
| human gates (releases, credentials, bar changes) | `ask_user_question` |
| commands and file manipulation | `bash` / `read` / `write` / `edit` |
| block until an async subagent finishes | `subagent_wait` |
| research (crates.io, minecraft docs, vanilla sources) | `web_search` / `fetch_content` |

## 8. Git workflow

- **Commit incrementally**: commit each proven unit as it passes (`cargo test` green +
  measurement evidence), with a descriptive message. Never a mega-commit at the end —
  a 47-file commit is un-bisectable. If a unit is WIP and unproven, commit it on a
  branch or label it clearly in the message (e.g. "WIP: ...").
- Always run `cargo test --workspace` before committing.
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
- Trust a state claim without evidence (§2.5) — build on verified state only.

## 9.5 Skills

Load project skills only when needed and only the relevant ones (e.g. Rust/gauntlet
best practices for a worldgen task). Do not load skills that don't apply to the task.
Skills live in the harness's skill directory (pi: `~/.pi/agent/skills/`).

## 10. Task format

```markdown
### T1 — <title>
- What: <what must be true at the end, measurable>
- AC: <concrete criteria with thresholds>
- Evidence: <logs, hashes, outputs to paste>
- DoD: <what the critic runs from scratch to give PASS>
```

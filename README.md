# Neutron

A Minecraft Java Edition server reimplemented from scratch in Rust. Multi-platform
(Windows/Linux/macOS x86-64/ARM64), 1:1 vanilla parity, secure-by-construction
WASM/Lua plugins, and `main` always on the latest Minecraft version.

**Status**: PRE-ALPHA · worldgen F2d active (mechanism parity, run-046) · playable 26.2 server

## What this project is

1. **Extreme performance** — measured and published with reproducible methodology (BENCHMARKS.md), not marketing.
2. **Security by construction** — plugins in a WASM sandbox: a plugin never takes down the server.
3. **1:1 vanilla parity** — same seed → same world; redstone, lighting and spawns identical; verified by CI checksums.
4. **Version cadence** — `main` = latest Mojang version in ≤ 7 days (D0-D4 pipeline).

---

## How AI work happens in this project (READ)

This repo is designed for an agent (pi, opencode or zcode) to work on it with
**state on disk, not in chat memory**. The method is generic — it works for worldgen,
redstone, protocol, tools, anything — and scales because each session rebuilds its
context from files, not from the previous conversation.

### The method: Gauntlet Loop

```
LEAD → splits the goal into gradeable pieces
  ├─ BUILDER builds each piece
  └─ CRITIC (subagent, clean context) inspects the REAL artifact against the bar
       PASS → next piece · FAIL → the biggest gap → rebuild → repeat
```

Non-negotiable rules: the **bar** is a real reference (checksum, benchmark, vanilla
server) that is never edited to make a test pass · the **builder never grades itself** ·
**ratchet**: every round re-measures ALL seeds, a regression is FAIL · **incremental
commits**: each proven piece is committed alone, never mega-commits.

### File map (what each file is, who touches it)

| File | What it is | Who reads it | Who writes it | When |
| --- | --- | --- | --- | --- |
| `AGENTS.md` | Universal contract: how we work, bar, loop, boundaries, tools | every agent, at start | human + LEAD | when the method changes |
| `STATE.md` | **Real state** (≤80 lines): phase, single bar, last measurement, next action, gaps | every agent, at start | LEAD, at run close | every run |
| `workbench.md` | LIVE round log of the active run: current round, per-unit PASS/FAIL | LEAD + whoever supervises | LEAD, after each round | every round |
| `runs/run-NNN.md` | Evidence of each run: objective, bar, tasks, logs, rounds | blind critic + auditors | LEAD | every run |
| `runs/README.md` | Run template + PASS discipline + how to launch | LEAD | LEAD | when the template changes |
| `ROADMAP.md` | Phases + bars + links (index, not prompts) | LEAD | human + LEAD | when the plan changes |
| `docs/prompts/*.md` | Phase prompts ready to paste into pi | LEAD | LEAD | when launching a phase |
| `ARCHITECTURE.md` | Server design + verified evidence | whoever designs | human | when the design changes |

**State rules** (against "false state"):

- At session start, **audit STATE.md against real evidence** (git log, runs/, logs):
  if a claim has no evidence file, re-measure it — don't trust it.
- State is written by whoever holds the evidence, never copied from someone else's
  summaries.
- A PASS requires blind-critic evidence; builder-verified work is labeled
  "builder-verified", never PASS.
- **Resume test**: the system works if you can kill the session, resume, and the next
  agent picks up from disk alone.

### Harness (pi / opencode / zcode)

The primary harness is **pi** (with plugins). `AGENTS.md` is the universal contract:
all mainstream harnesses read it. The tool names in `AGENTS.md` §7 are pi's
(`subagent`, `todo`, `ask_user_question`); opencode/zcode map those roles to their own
tools — the roles matter, not the names. Subagent delegation (builder vs blind critic
with clean context) and web research (`web_search`/`fetch_content`) work the same in
all three.

### Multiple agents at once

The repo supports parallel agents with **ownership rules** (AGENTS.md §5.5): each agent
touches only its own files, shared state (STATE/workbench/runs) is written only by the
LEAD (append-only), and worktree isolation is preferred. Without these rules, two agents
in the same tree clobber each other — it happened in Aug 2026 (a parallel agent
overwrote ARCHITECTURE.md). If you see foreign uncommitted work, don't overwrite it:
ask who owns it.

### Skills

Load **only the project skills that apply to the task** (e.g. Rust best practices for a
worldgen task, gauntlet-loop for a run). Don't load skills unrelated to the task. They
live in the harness's skill directory.

---

## Orientation (what to read when)

| You need | Document |
| --- | --- |
| What this is and how AI work happens | this README |
| Where we are and what's next | STATE.md |
| The plan (phases, bars, version pipeline) | ROADMAP.md (prompts in docs/prompts/) |
| How the server is designed + evidence | ARCHITECTURE.md (Annex A) |
| How benchmarks are measured | BENCHMARKS.md |
| How to work / launch a run | AGENTS.md + runs/README.md |

## Targets (to validate with BENCHMARKS.md)

| Metric | Target |
| --- | --- |
| Startup (empty world → `Done`) | < 2 s |
| Chunks/s @16 threads | > 250 |
| Base RAM | < 150 MB |
| RAM per player | < 1 MB |
| TPS @500 players | 20.0, p99 < 25 ms |
| Join p95 @100 bots | < 2 s |
| New Mojang version | main ≤ 7 days |

## Repo structure (today)

The diagram in `ARCHITECTURE.md` describes the **goal** (cli, WASM plugins, Folia). The real graph is smaller:

```
neutron/
├─ crates/
│  ├─ neutron-protocol/     # 26.2 packets (hand-written)
│  ├─ neutron-world/        # Anvil / level.dat (not yet used by the server)
│  ├─ neutron-worldgen/     # 26.2 overworld — the current parity focus
│  ├─ neutron-server/       # playable binary: login + chunks
│  ├─ neutron-sim/          # light / redstone / fluids / spawn (tests, not wired)
│  └─ neutron-bench-server/ # criterion
├─ tools/                   # golden-data · parity-check · vanilla-extract · java-probe
├─ bench/                   # separate workspace: bots + reference jars
├─ runs/                    # run history (run-NNN.md)
├─ docs/prompts/            # phase prompts for pi
└─ docs/                    # ADRs and notes
```

## Quick start (dev)

```bash
# Playable server (real worldgen, seed 12345)
cargo run --release -p neutron-server -- --seed 12345 --view-distance 8
# Vanilla 26.2 client → localhost:25565  (online-mode=false, Creative + fly)
# Worldgen state and gaps: STATE.md + crates/neutron-worldgen/WORLDGEN.md
```

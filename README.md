# Neutron

A Minecraft Java Edition server reimplemented from scratch in Rust. Multi-platform
(Windows/Linux/macOS x86-64/ARM64), 1:1 vanilla parity, secure-by-construction
WASM/Lua plugins, and `main` always on the latest Minecraft version.

**Status**: PRE-ALPHA · worldgen F2d active (mechanism parity, run-048) · playable 26.2 server

## What this project is

1. **Extreme performance** — measured and published with reproducible methodology (BENCHMARKS.md), not marketing.
2. **Security by construction** — plugins in a WASM sandbox: a plugin never takes down the server.
3. **1:1 vanilla parity** — same seed → same world; redstone, lighting and spawns identical; verified by CI checksums.
4. **Version cadence** — `main` = latest Mojang version in ≤ 7 days (D0-D4 pipeline).

---

## How AI work happens in this project (READ)

**Fan out closed dumps/ports in parallel; one writer patches after.** Full contract:
`AGENTS.md` v2. Facts: `STATE.md`. `runs/` and `workbench.md` are archive.

Do not fan-out three “investigate water/trees/clay” agents (same gap). Do not open a
run file to start work. Bar = vanilla 26.2. Worldgen order: doFill → surface →
carvers → features.

| File | What it is |
| --- | --- |
| `AGENTS.md` | How we work |
| `STATE.md` | Current numbers + next dump (≤80 lines) |
| `runs/` | Archive of past attempts |
| `ROADMAP.md` | Phases |
| `ARCHITECTURE.md` | Server design |

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
├─ tools/                   # mc-decompiler · worldgen-probe · ref-extract · neutron-hash · chunk-dump · nbt-ref (human-owned)
├─ tests/benchmarks/        # separate workspace: bots + reference jars (nightly)
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

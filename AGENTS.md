# AGENTS.md — Neutron: how we work

> v2.0 · 20 Aug 2026 · Read at task start. Docs in English.
> Default harness can be pi, Grok, Cursor, Codex — this file is the contract, not a pi playbook.

## 0. Working directory

Repository root only. Never write outside it. If work exists on an external path, bring it in or ask.

## 1. What this is

Minecraft server in Rust, vanilla **26.2**. Hardest goal: **1:1 worldgen** (`crates/neutron-worldgen`). Facts: `STATE.md`. Java sources: `tools/mc-decompiler/output/26.2/src`.

## 2. How we work

The harness (pi, Grok, …) **should** fan out agents. That is the point. What burned weeks
was not fan-out — it was three open “investigate water/trees/clay” agents on the **same
causal chain**, plus run files, worktrees, and 30-minute timeouts.

`runs/` is an archive. `workbench.md` is frozen. No `run-NNN.md` to start work.

**Loop**

1. Split the stuck point into **independent closed dumps or closed ports** (see §6).
2. Fan them out in parallel. Each agent owns disjoint files and answers one question
   with seed + `(x,y,z)` + vanilla `Class.method` (decompile line) + Neutron counterpart.
3. No dump of both sides = no finding. Agents do not write `STATE.md`.
4. The parent merges the dumps, then **one writer** patches. Re-dump the cell.
   `region_parity` on 424242. Commit if the cell matches or the % moved.

**Worldgen phase order — do not skip down:**

```
doFill (interpolated density + aquifer → block)
  → surface → carvers → structures → features (ores, sculk, clay, trees, …)
```

Water, clay, and trees are **one problem** until doFill + carvers match. Fan-out
*dumps of different stages* (doFill ∥ carvers ∥ “does this Y band even open”). Do **not**
fan-out three feature ports against that gap. Feature ports and decoration order stay
frozen until the dumps match.

A “lever” without a two-sided dump of the same seed, coords, and code path is a
hypothesis. Hypotheses do not go in `STATE.md`.

**Do not:**

- Open a new `runs/run-NNN.md` to start work.
- Give an agent “investigate why there is no water” / “look at the Java”.
- `cargo test --workspace` on every experiment (only before commit / push).
- Cold git worktrees for a dump (15–20 min cargo tax). Same checkout, disjoint files.
- Trust `STATE.md` history, `workbench.md`, or a previous agent's “THE lever”.

## 3. Commands

```bash
# fast (inner loop)
cargo test -p neutron-worldgen
cargo run --release -p neutron-worldgen --example dofill_cells
cargo run --release -p neutron-worldgen --example water_ref_scan -- \
  tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region 0 0 --list
cargo run --release -p neutron-worldgen --example region_parity -- 424242 0 0 1 \
  tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region

# before commit / push
cargo test --workspace

# server
cargo run --release -p neutron-server -- --seed 12345 --view-distance 8

# other seeds (paths differ: 12345/777 have no `world/` prefix)
#   tools/nbt-ref/vanilla-fresh-12345/dimensions/minecraft/overworld/region
#   tools/nbt-ref/vanilla-fresh-777/dimensions/minecraft/overworld/region
```

Do not re-extract vanilla refs if the region `.mca` files are already on disk. Do not boot a vanilla server to answer a density question — use `tools/worldgen-probe` against the jar.

## 4. Layout

```
crates/neutron-worldgen/   # 1:1 parity lives here
crates/neutron-server/     # playable binary
tools/mc-decompiler/       # 26.2 sources + jar
tools/nbt-ref/             # vanilla reference worlds (gitignored)
tools/worldgen-probe/      # Java dumps vs the jar
runs/                      # archive only
```

## 5. Vanilla / refs

- Java 25. Jar: `tools/mc-decompiler/jars/server-26.2.jar`.
- Probe classpath: extracted `tools/nbt-ref/vanilla-fresh-424242/versions/26.2/server-26.2.jar` + `libraries/`.
- A proto-chunk (`Status=structure_starts`) is not a measurement target. Skip that seed/chunk.

## 6. Parallelism (this is the harness — use it)

Fan-out is default when the pieces **do not share a writer file** and **do not share a
causal chain**.

| Fan out | Do not fan out |
| --- | --- |
| Closed dump A vs dump B (doFill table ∥ carver table) | “Find the root cause of water” × 3 |
| Two feature ports that do not share a module, **after** terrain dumps match | trees ∥ clay ∥ water (same gap) |
| Java probe in `tools/worldgen-probe/src/ProbeFoo.java` ∥ Rust example `examples/foo.rs` | Two agents editing `density.rs` / `carvers.rs` / `Cargo.toml` |
| Closed port: “implement `IceSpikeFeature` from this Java file; `dispatch_coverage` green” | Open research, worktrees, critics of research |

Rules:

1. Parent names the question, the files, and the evidence path **before** spawn.
2. One writer per file. If you see foreign uncommitted work, stop.
3. Same git checkout (warm `target/`). Worktrees only when two agents must edit the
   same crate for real implementations, and then seed `target/` with hardlinks.
4. Agents never write `STATE.md` / `AGENTS.md`. Parent merges dumps, then patches.
5. `tools/`: adding a dump probe is allowed. Do not refactor the tool tree.

## 7. Git

- Commit a proven cell-fix or a real % move, not a new diagnostic example every time.
- `cargo test -p neutron-worldgen` green for worldgen-only commits; `cargo test --workspace` before push.
- Never commit: `target/`, jars, `tools/nbt-ref/vanilla-*/`, decompiled dumps, `tools/worldgen-probe/bin/`, `logs/`.
- Never edit measurement examples/tests to make a bar pass.

## 8. STATE.md

≤ 80 lines. Current numbers, the next dump, dead hypotheses. Rewrite the facts section when they change; do not append novels. History stays in `runs/` if someone wants it.

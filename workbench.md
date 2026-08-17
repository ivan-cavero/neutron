# Neutron — dual-track gauntlet workbench

> Live round log. LEAD: pi (main checkout). Two parallel tracks in Orca worktrees.
> Bar is untouchable; builders never grade themselves; critics are fresh per round.

## Goal

1. **Track A (bench-refactor)**: refactor + improve the benchmark harness — fast,
   reliable multi-version server provisioning (jar download + local fallback),
   versioned report history, and a build/run that isn't painfully slow.
2. **Track B (server-worldgen)**: review the neutron server (test it properly) and
   continue world generation parity work (run-046 continuation).

## Bar (verbatim)

**Track A** (from previous workbench P1-P4 + user additions):
1. `bench/` no longer exists in the repo root; everything lives in `tests/benchmarks/`.
2. `cargo test --workspace` green in root workspace AND benchmarks workspace; no
   regression in the 59 worldgen tests.
3. `neutron-bench servers download <type> <version>` provisions jars (auto-download
   from Mojang manifest / Paper API + local fallback) into a central managed dir with
   a **multi-version layout**; zero tools reference `bench/servers` (`grep -r "bench/servers" tools/` = 0).
4. Every run writes a timestamped, versioned report into `tests/benchmarks/results/history/`;
   `compare` works over the history.
5. Benchmarks workspace builds in release from the repo root (toolchain resolved);
   build time measured before/after; benchmark wall-clock budget documented.

**Track B** (run-046 bar, human decision R43 — mechanism parity):
- Same seeds/streams/algorithms as vanilla. Deterministic phases → 100% block match
  multi-seed; vegetation/sculk → same RNG stream 1:1. **Do not edit measurement
  examples/tests to pass.**
- U5 AC: clay 424242 → ~497 (lush/pale missing ≤20%); border diffs ≥30% down on
  12345; REGION 424242 ≥97.28%, 12345 ≥97.75%; **777 no regression** (ratchet);
  tests 59/59.
- Server: boots, joinable, serves real chunks, TPS stable (smoke evidence).

## Budget

3 rounds/unit max, 2 tracks in parallel, 1 smoothing pass per track, then merge+push.
Stop: bar met, or 2 consecutive rounds with no improvement, or user says stop.

## Parallelism / ownership map

| Track | Worktree (branch) | Files owned | Builder | Critic |
| --- | --- | --- | --- | --- |
| A | `C:/Users/ivang/orca/workspaces/neutron/bench-refactor` (`ivan-cavero/bench-refactor`) | `tests/benchmarks/**` | gauntlet-builder | gauntlet-critic (fresh) |
| B | `C:/Users/ivang/orca/workspaces/neutron/server-worldgen` (`ivan-cavero/server-worldgen`) | `crates/neutron-server/**`, `crates/neutron-worldgen/**` | gauntlet-builder | gauntlet-critic (fresh) |
| LEAD | main checkout | `STATE.md`, `workbench.md`, `runs/` | — | — |

Rules: one writer per file; A and B touch disjoint dirs; lead merges branches into
main and pushes incrementally. `tools/` = human-owned, never touched.

## Units

| Unit | Scope | Verdict | Evidence | Artifact |
| --- | --- | --- | --- | --- |
| A1 | Multi-version server provisioning (`servers download <type> <version>`, Mojang/Paper API + local fallback, central dir, ref-extract decoupled) | — | | |
| A2 | Versioned report history + `compare` over history | — | | |
| A3 | Perf: root build works, build/runtime time before/after | — | | |
| B1 | Server review + bot smoke test (boot, join, chunks, TPS) | — | | |
| B2 | Reference extraction (Java 25) + baseline re-measure (424242/12345/777) | — | | |
| B3 | Worldgen: 777 regression isolate + lush/pale recall ≥80% | — | | |

## Round log

- R0 (LEAD): audit §2.5 — repo clean @ c566ece, in sync with origin/main. No stash
  (workbench claim stale). `tools/nbt-ref/` has NO reference worlds → run-046 numbers
  unverifiable, flagged. No jars, Java 21 only → Java 25 installed via winget
  (Temurin 25.0.4.7, user-approved). No target dirs (cold builds everywhere — the
  "slow benchmarks" baseline). Orca worktrees created:
  `bench-refactor` + `server-worldgen` (branch `ivan-cavero/*`). Workbench rewritten.
- R1: launching A1 builder + B1 builder in parallel (async).

## Open questions for the human

- (none)